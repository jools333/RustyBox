package com.rustybox.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Intent
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import io.nekohasekai.libbox.BridgeOptions
import io.nekohasekai.libbox.BridgeSession
import io.nekohasekai.libbox.CommandServer
import io.nekohasekai.libbox.CommandServerHandler
import io.nekohasekai.libbox.ConnectionOwner
import io.nekohasekai.libbox.InterfaceUpdateListener
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.LocalDNSTransport
import io.nekohasekai.libbox.NeighborUpdateListener
import io.nekohasekai.libbox.NetworkInterfaceIterator
import io.nekohasekai.libbox.OverrideOptions
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.PlatformUser
import io.nekohasekai.libbox.SetupOptions
import io.nekohasekai.libbox.ShellSession
import io.nekohasekai.libbox.StringIterator
import io.nekohasekai.libbox.SystemProxyStatus
import io.nekohasekai.libbox.TunOptions
import io.nekohasekai.libbox.WIFIState

class RustyBoxVpnService : VpnService() {
    companion object {
        const val ACTION_START = "com.rustybox.vpn.START"
        const val ACTION_STOP = "com.rustybox.vpn.STOP"
        const val EXTRA_CONFIG = "config"
        private const val CHANNEL_ID = "rustybox_vpn_channel"
        private const val NOTIFICATION_ID = 1

        @Volatile
        var isRunning: Boolean = false
            private set

        @Volatile
        var lastError: String? = null
            private set
    }

    private var commandServer: CommandServer? = null
    private var tunFd: ParcelFileDescriptor? = null
    private var defaultNetworkCallback: ConnectivityManager.NetworkCallback? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                val config = intent.getStringExtra(EXTRA_CONFIG) ?: return START_NOT_STICKY
                startForegroundNotification()
                Thread {
                    startBox(config)
                }.start()
            }
            ACTION_STOP -> {
                Thread {
                    stopBox()
                    stopForeground(STOP_FOREGROUND_REMOVE)
                    stopSelf()
                }.start()
            }
        }
        return START_STICKY
    }

    private fun startForegroundNotification() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "RustyBox VPN",
                NotificationManager.IMPORTANCE_LOW
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager?.createNotificationChannel(channel)
        }
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        val notification = builder
            .setContentTitle("RustyBox VPN")
            .setContentText("VPN service is active")
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .build()
        startForeground(NOTIFICATION_ID, notification)
    }

    @Synchronized
    private fun startBox(configJson: String) {
        stopBox()
        lastError = null
        try {
            val setupOptions = SetupOptions().apply {
                basePath = filesDir.absolutePath
                workingPath = filesDir.absolutePath
                tempPath = cacheDir.absolutePath
                fixAndroidStack = true
            }
            Libbox.setup(setupOptions)

            val platform = object : PlatformInterface {
                override fun localDNSTransport(): LocalDNSTransport? = null

                override fun openTun(options: TunOptions): Int {
                    val mtu = if (options.mtu > 0) options.mtu else 1500
                    val builder = Builder()
                        .setSession("RustyBox")
                        .setMtu(mtu)
                        .addAddress("172.19.0.1", 30)
                        .addAddress("fdfe:dcba:9876::1", 126)
                        .addRoute("0.0.0.0", 0)
                        .addRoute("::", 0)
                        .addDnsServer("1.1.1.1")
                        .addDisallowedApplication(packageName)

                    val pfd = builder.establish()
                        ?: throw IllegalStateException("VpnService.Builder.establish() returned null")
                    tunFd = pfd
                    return pfd.fd
                }

                override fun autoDetectInterfaceControl(fd: Int) {
                    val ok = protect(fd)
                    if (!ok) {
                        android.util.Log.e("RustyBoxVPN", "protect($fd) returned false!")
                    }
                }

                override fun usePlatformAutoDetectInterfaceControl(): Boolean = true
                override fun useProcFS(): Boolean = false
                override fun findConnectionOwner(
                    ipProtocol: Int,
                    sourceAddress: String?,
                    sourcePort: Int,
                    destinationAddress: String?,
                    destinationPort: Int
                ): ConnectionOwner = ConnectionOwner().apply {
                    userId = -1
                }

                override fun underNetworkExtension(): Boolean = false
                override fun includeAllNetworks(): Boolean = false
                override fun clearDNSCache() {}
                override fun readWIFIState(): WIFIState? = null
                override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener?) {
                    val cm = getSystemService(ConnectivityManager::class.java) ?: return
                    fun notifyNetwork(network: Network?) {
                        if (network == null) {
                            listener?.updateDefaultInterface("", -1, false, false)
                            return
                        }
                        val lp = cm.getLinkProperties(network) ?: return
                        val ifName = lp.interfaceName ?: return
                        val caps = cm.getNetworkCapabilities(network)
                        val isExpensive = caps?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == false
                        val ifIndex = try {
                            java.net.NetworkInterface.getByName(ifName)?.index ?: -1
                        } catch (_: Exception) {
                            -1
                        }
                        if (ifIndex >= 0) {
                            listener?.updateDefaultInterface(ifName, ifIndex, isExpensive, false)
                        }
                    }

                    notifyNetwork(cm.activeNetwork)

                    val callback = object : ConnectivityManager.NetworkCallback() {
                        override fun onAvailable(network: Network) {
                            notifyNetwork(network)
                        }

                        override fun onLinkPropertiesChanged(network: Network, linkProperties: LinkProperties) {
                            notifyNetwork(network)
                        }

                        override fun onCapabilitiesChanged(network: Network, networkCapabilities: NetworkCapabilities) {
                            notifyNetwork(network)
                        }

                        override fun onLost(network: Network) {
                            notifyNetwork(cm.activeNetwork)
                        }
                    }
                    defaultNetworkCallback = callback
                    try {
                        cm.registerDefaultNetworkCallback(callback)
                    } catch (_: Exception) {
                    }
                }

                override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener?) {
                    val cb = defaultNetworkCallback ?: return
                    defaultNetworkCallback = null
                    try {
                        val cm = getSystemService(ConnectivityManager::class.java)
                        cm?.unregisterNetworkCallback(cb)
                    } catch (_: Exception) {
                    }
                }

                override fun getInterfaces(): NetworkInterfaceIterator {
                    val result = mutableListOf<io.nekohasekai.libbox.NetworkInterface>()
                    try {
                        val cm = getSystemService(ConnectivityManager::class.java)
                        val activeCaps = cm?.activeNetwork?.let { cm.getNetworkCapabilities(it) }
                        val activeIfName = cm?.activeNetwork?.let { cm.getLinkProperties(it)?.interfaceName }
                        val netIfs = java.net.NetworkInterface.getNetworkInterfaces()
                        while (netIfs != null && netIfs.hasMoreElements()) {
                            val nif = netIfs.nextElement()
                            if (!nif.isUp) continue
                            val addrs = nif.interfaceAddresses.mapNotNull { ia ->
                                val host = ia.address?.hostAddress?.substringBefore('%') ?: return@mapNotNull null
                                "$host/${ia.networkPrefixLength}"
                            }
                            if (addrs.isEmpty()) continue
                            val libIf = io.nekohasekai.libbox.NetworkInterface().apply {
                                index = nif.index
                                name = nif.name
                                mtu = try { nif.mtu } catch (_: Exception) { 1500 }
                                flags = (if (nif.isUp) 1 else 0) or (if (nif.isLoopback) 8 else 64)
                                addresses = stringIteratorOf(addrs)
                                type = if (nif.name == activeIfName && activeCaps != null) {
                                    when {
                                        activeCaps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> Libbox.InterfaceTypeWIFI
                                        activeCaps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> Libbox.InterfaceTypeCellular
                                        activeCaps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> Libbox.InterfaceTypeEthernet
                                        else -> Libbox.InterfaceTypeOther
                                    }
                                } else {
                                    Libbox.InterfaceTypeOther
                                }
                            }
                            result.add(libIf)
                        }
                    } catch (_: Exception) {
                    }
                    var idx = 0
                    return object : NetworkInterfaceIterator {
                        override fun hasNext(): Boolean = idx < result.size
                        override fun next(): io.nekohasekai.libbox.NetworkInterface? =
                            if (idx < result.size) result[idx++] else null
                    }
                }
                override fun sendNotification(notification: io.nekohasekai.libbox.Notification?) {}
                override fun cancelNotification(identifier: String?, typeID: Int) {}
                override fun startNeighborMonitor(listener: NeighborUpdateListener?) {}
                override fun closeNeighborMonitor(listener: NeighborUpdateListener?) {}
                override fun registerMyInterface(name: String?) {}
                override fun usePlatformShell(): Boolean = false
                override fun checkPlatformShell() {}
                override fun openShellSession(
                    user: PlatformUser?,
                    command: String?,
                    environ: StringIterator?,
                    term: String?,
                    rows: Int,
                    cols: Int
                ): ShellSession? = null

                override fun lookupUser(username: String?): PlatformUser? = null
                override fun lookupSFTPServer(): String = ""
                override fun readSystemSSHHostKey(): String = ""
                override fun tailscaleHostname(): String = ""
                override fun usePlatformBridge(): Boolean = false
                override fun createBridge(options: BridgeOptions?): BridgeSession? = null
            }

            val handler = object : CommandServerHandler {
                override fun serviceStop() {
                    stopBox()
                    stopForeground(STOP_FOREGROUND_REMOVE)
                    stopSelf()
                }

                override fun serviceReload() {}
                override fun getSystemProxyStatus(): SystemProxyStatus? = null
                override fun setSystemProxyEnabled(enabled: Boolean) {}
                override fun triggerNativeCrash() {}
                override fun writeDebugMessage(message: String?) {}
                override fun connectSSHAgent(): Int = -1
            }

            val server = Libbox.newCommandServer(handler, platform)
            server.start()
            server.startOrReloadService(configJson, OverrideOptions())
            commandServer = server
            isRunning = true
        } catch (e: Exception) {
            lastError = e.message ?: e.toString()
            isRunning = false
            stopBox()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
    }

    private fun stringIteratorOf(items: List<String>): StringIterator = object : StringIterator {
        private var idx = 0
        override fun len(): Int = items.size
        override fun hasNext(): Boolean = idx < items.size
        override fun next(): String = if (idx < items.size) items[idx++] else ""
    }

    @Synchronized
    private fun stopBox() {
        try {
            commandServer?.closeService()
        } catch (_: Exception) {
        }
        try {
            commandServer?.close()
        } catch (_: Exception) {
        }
        commandServer = null
        try {
            tunFd?.close()
        } catch (_: Exception) {
        }
        tunFd = null
        isRunning = false
    }

    override fun onDestroy() {
        stopBox()
        super.onDestroy()
    }
}
