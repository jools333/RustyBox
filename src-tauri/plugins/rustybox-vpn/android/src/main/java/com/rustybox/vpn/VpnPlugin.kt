package com.rustybox.vpn

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class StartVpnArgs {
    lateinit var config: String
}

@InvokeArg
class OpenUrlArgs {
    var url: String? = null
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

    @Command
    fun getOrCreateMasterKey(invoke: Invoke) {
        try {
            val key = loadOrCreateMasterKeyBytes()
            val ret = JSObject()
            ret.put("key", Base64.encodeToString(key, Base64.NO_WRAP))
            invoke.resolve(ret)
        } catch (e: Exception) {
            invoke.reject("Failed to access Android Keystore: ${e.message}")
        }
    }

    private fun getOrCreateKeystoreKey(): SecretKey {
        val alias = "rustybox_master_key_v1"
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val existing = keyStore.getKey(alias, null) as? SecretKey
        if (existing != null) return existing

        val keyGen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        val spec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build()
        keyGen.init(spec)
        return keyGen.generateKey()
    }

    private fun loadOrCreateMasterKeyBytes(): ByteArray {
        val prefs = activity.getSharedPreferences("rustybox_secure_prefs", Context.MODE_PRIVATE)
        val encB64 = prefs.getString("master_key_enc", null)
        val ivB64 = prefs.getString("master_key_iv", null)
        val ksKey = getOrCreateKeystoreKey()

        if (encB64 != null && ivB64 != null) {
            try {
                val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                val iv = Base64.decode(ivB64, Base64.NO_WRAP)
                cipher.init(Cipher.DECRYPT_MODE, ksKey, GCMParameterSpec(128, iv))
                val plain = cipher.doFinal(Base64.decode(encB64, Base64.NO_WRAP))
                if (plain.size == 32) return plain
            } catch (_: Exception) {
                // Keystore entry invalidated or corrupted; regenerate below
            }
        }

        val masterKey = ByteArray(32)
        SecureRandom().nextBytes(masterKey)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, ksKey)
        val encrypted = cipher.doFinal(masterKey)
        prefs.edit()
            .putString("master_key_enc", Base64.encodeToString(encrypted, Base64.NO_WRAP))
            .putString("master_key_iv", Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
            .apply()
        return masterKey
    }

    @Command
    fun openUrl(invoke: Invoke) {
        val args = invoke.parseArgs(OpenUrlArgs::class.java)
        val url = args.url
        if (url.isNullOrBlank()) {
            invoke.reject("URL must not be empty")
            return
        }
        try {
            val intent = Intent(Intent.ACTION_VIEW, android.net.Uri.parse(url)).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "Failed to open URL")
        }
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
