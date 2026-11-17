package com.rustybox.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import io.nekohasekai.libbox.BoxService
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.TunOptions

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

    private var boxService: BoxService? = null
    private var tunFd: ParcelFileDescriptor? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                val config = intent.getStringExtra(EXTRA_CONFIG) ?: return START_NOT_STICKY
                startForegroundNotification()
                startBox(config)
            }
            ACTION_STOP -> {
                stopBox()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
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

    private fun startBox(configJson: String) {
        stopBox()
        lastError = null
        try {
            val platform = object : PlatformInterface {
                override fun openTun(options: TunOptions): Int {
                    val builder = Builder()
                        .setSession("RustyBox")
                        .setMtu(options.mtu)
                        .addAddress("172.19.0.1", 30)
                        .addRoute("0.0.0.0", 0)
                        .addDnsServer("1.1.1.1")
                        .addDisallowedApplication(packageName)

                    val pfd = builder.establish()
                        ?: throw IllegalStateException("VpnService.Builder.establish() returned null")
                    tunFd = pfd
                    return pfd.fd
                }

                override fun autoDetectInterfaceControl(fd: Int) {
                    protect(fd)
                }

                override fun usePlatformAutoDetectInterfaceControl(): Boolean = true
                override fun useProcFS(): Boolean = false
                override fun findConnectionOwner(
                    ipProtocol: Int,
                    sourceAddress: String?,
                    sourcePort: Int,
                    destinationAddress: String?,
                    destinationPort: Int
                ): Int = -1

                override fun packageNameByUid(uid: Int): String = ""
                override fun uidByPackageName(packageName: String?): Int = -1
                override fun underNetworkExtension(): Boolean = false
                override fun includeAllNetworks(): Boolean = false
                override fun clearDNSCache() {}
                override fun readWIFIState(): io.nekohasekai.libbox.WIFIState? = null
                override fun startDefaultInterfaceMonitor(listener: io.nekohasekai.libbox.InterfaceUpdateListener?) {}
                override fun closeDefaultInterfaceMonitor(listener: io.nekohasekai.libbox.InterfaceUpdateListener?) {}
                override fun getInterfaces(): io.nekohasekai.libbox.NetworkInterfaceIterator? = null
                override fun sendNotification(notification: io.nekohasekai.libbox.Notification?) {}
                override fun writeLog(message: String?) {}
            }

            val service = Libbox.newService(configJson, platform)
            service.start()
            boxService = service
            isRunning = true
        } catch (e: Exception) {
            lastError = e.message ?: e.toString()
            isRunning = false
            stopBox()
        }
    }

    private fun stopBox() {
        try {
            boxService?.close()
        } catch (_: Exception) {
        }
        boxService = null
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
