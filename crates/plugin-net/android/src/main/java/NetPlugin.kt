package dev.mwt.net

import android.app.Activity
import android.content.Context
import android.net.wifi.WifiManager
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import app.tauri.Logger
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class HexArgs {
  lateinit var hex: String
}

@InvokeArg
class MulticastArgs {
  var on: Boolean = false
}

/**
 * LAN sync on Android: the multicast lock mDNS needs, and a non-exportable
 * AES key in the Android Keystore (StrongBox when the phone has one) that
 * wraps the device's sync key at rest.
 */
@TauriPlugin
class NetPlugin(private val activity: Activity) : Plugin(activity) {
  private val alias = "mwt-sync-device-key"
  private var lock: WifiManager.MulticastLock? = null

  @Command
  fun multicast(invoke: Invoke) {
    val args = invoke.parseArgs(MulticastArgs::class.java)
    val wifi = activity.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    if (args.on) {
      if (lock == null) {
        lock = wifi.createMulticastLock("mwt-sync").apply { setReferenceCounted(false) }
      }
      lock?.acquire()
    } else {
      lock?.release()
    }
    invoke.resolve(JSObject())
  }

  private fun key(): SecretKey {
    val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    (store.getKey(alias, null) as? SecretKey)?.let { return it }
    val spec = { strongBox: Boolean ->
      KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .apply { if (strongBox && Build.VERSION.SDK_INT >= 28) setIsStrongBoxBacked(true) }
        .build()
    }
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
    return try {
      generator.init(spec(true))
      generator.generateKey()
    } catch (ex: Exception) {
      Logger.info("no StrongBox, using the TEE: ${ex.message}")
      generator.init(spec(false))
      generator.generateKey()
    }
  }

  @Command
  fun wrap(invoke: Invoke) {
    try {
      val plain = unhex(invoke.parseArgs(HexArgs::class.java).hex)
      val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.ENCRYPT_MODE, key())
      val sealed = cipher.iv + cipher.doFinal(plain)
      invoke.resolve(JSObject().put("hex", hex(sealed)))
    } catch (ex: Exception) {
      invoke.reject("wrap failed: ${ex.message}")
    }
  }

  @Command
  fun unwrap(invoke: Invoke) {
    try {
      val sealed = unhex(invoke.parseArgs(HexArgs::class.java).hex)
      val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed.copyOfRange(0, 12)))
      val plain = cipher.doFinal(sealed.copyOfRange(12, sealed.size))
      invoke.resolve(JSObject().put("hex", hex(plain)))
    } catch (ex: Exception) {
      invoke.reject("unwrap failed: ${ex.message}")
    }
  }

  private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }

  private fun unhex(text: String): ByteArray =
    ByteArray(text.length / 2) { index -> text.substring(index * 2, index * 2 + 2).toInt(16).toByte() }
}
