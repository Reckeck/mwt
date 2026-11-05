package dev.mwt.net

import android.Manifest
import android.app.Activity
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import app.tauri.Logger
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import androidx.appcompat.app.AppCompatActivity
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

@InvokeArg
class BackgroundArgs {
  var armed: Boolean = false
  var wake: Boolean = false
  var channel: String = "Sync"
  var title: String = "Sync"
  var text: String = ""
  var lock: String = "Lock"
  var stop: String = "Stop"
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
  /** Sync runs: going to the background starts [SyncService]. */
  @Volatile private var armed = false
  @Volatile private var inBackground = false
  private var serviceOn = false

  override fun onPause(activity: AppCompatActivity) {
    inBackground = true
    if (armed) startService()
  }

  override fun onResume(activity: AppCompatActivity) {
    inBackground = false
    stopService()
  }

  private fun startService() {
    if (serviceOn) return
    try {
      SyncService.start(activity.applicationContext)
      serviceOn = true
    } catch (ex: Exception) {
      Logger.error("cannot start background sync: ${ex.message}")
    }
  }

  /** Idempotent: the service ignores a stop it was never asked to run. */
  private fun stopService() {
    SyncService.stop(activity.applicationContext)
    serviceOn = false
  }

  /** Sync started or stopped here, or the notification text changed. */
  @Command
  fun background(invoke: Invoke) {
    val args = invoke.parseArgs(BackgroundArgs::class.java)
    SyncService.texts = SyncService.Texts(args.channel, args.title, args.text, args.lock, args.stop)
    SyncService.wake = args.wake
    armed = args.armed
    activity.runOnUiThread {
      if (!armed) {
        stopService()
      } else if (inBackground) {
        if (serviceOn) SyncService.refresh() else startService()
      }
    }
    invoke.resolve(JSObject())
  }

  /** Whether the app is in the background, and the button pressed since the last poll. */
  @Command
  fun poll(invoke: Invoke) {
    val action = SyncService.pending
    SyncService.pending = null
    invoke.resolve(JSObject().put("background", inBackground).put("action", action))
  }

  /** The system's language, for texts made outside the window. */
  @Command
  fun language(invoke: Invoke) {
    invoke.resolve(JSObject().put("language", java.util.Locale.getDefault().language))
  }

  /** Android 13+: the background sync notification needs this permission. */
  @Command
  fun askNotifications(invoke: Invoke) {
    if (Build.VERSION.SDK_INT >= 33 &&
      activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
    ) {
      activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 7318)
    }
    invoke.resolve(JSObject())
  }

  /** Whether the background notification can show: the permission (13+) and the app's own switch. */
  @Command
  fun notificationsAllowed(invoke: Invoke) {
    val granted = Build.VERSION.SDK_INT < 33 ||
      activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
    val manager = activity.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    invoke.resolve(JSObject().put("allowed", granted && manager.areNotificationsEnabled()))
  }

  /** The system page of this app's notification settings. */
  @Command
  fun openNotificationSettings(invoke: Invoke) {
    try {
      val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
        .putExtra(Settings.EXTRA_APP_PACKAGE, activity.packageName)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      activity.startActivity(intent)
      invoke.resolve(JSObject())
    } catch (ex: Exception) {
      invoke.reject("cannot open the notification settings: ${ex.message}")
    }
  }

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
