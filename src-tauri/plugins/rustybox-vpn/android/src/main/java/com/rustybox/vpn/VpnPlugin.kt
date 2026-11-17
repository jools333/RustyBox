package com.rustybox.vpn

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class StartVpnArgs {
    lateinit var config: String
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingConfig: String? = null

    @Command
    fun startVpn(invoke: Invoke) {
        val args = invoke.parseArgs(StartVpnArgs::class.java)
        val prepareIntent = VpnService.prepare(activity)
        if (prepareIntent != null) {
            pendingConfig = args.config
            startActivityForResult(invoke, prepareIntent, "onVpnPermissionResult")
        } else {
            launchVpnService(args.config)
            val ret = JSObject()
            ret.put("started", true)
            invoke.resolve(ret)
        }
    }

    @ActivityCallback
    fun onVpnPermissionResult(invoke: Invoke, result: ActivityResult) {
        val config = pendingConfig
        pendingConfig = null
        if (result.resultCode == Activity.RESULT_OK && config != null) {
            launchVpnService(config)
            val ret = JSObject()
            ret.put("started", true)
            invoke.resolve(ret)
        } else {
            invoke.reject("VPN permission denied by user")
        }
    }

    @Command
    fun stopVpn(invoke: Invoke) {
        val intent = Intent(activity, RustyBoxVpnService::class.java).apply {
            action = RustyBoxVpnService.ACTION_STOP
        }
        activity.startService(intent)
        val ret = JSObject()
        ret.put("stopped", true)
        invoke.resolve(ret)
    }

    @Command
    fun getVpnStatus(invoke: Invoke) {
        val ret = JSObject()
        ret.put("running", RustyBoxVpnService.isRunning)
        RustyBoxVpnService.lastError?.let { ret.put("error", it) }
        invoke.resolve(ret)
    }

    private fun launchVpnService(config: String) {
        val intent = Intent(activity, RustyBoxVpnService::class.java).apply {
            action = RustyBoxVpnService.ACTION_START
            putExtra(RustyBoxVpnService.EXTRA_CONFIG, config)
        }
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
            activity.startForegroundService(intent)
        } else {
            activity.startService(intent)
        }
    }
}
