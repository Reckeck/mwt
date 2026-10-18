package dev.mwt.net

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import app.tauri.Logger

/**
 * Keeps vault sync running while the app is in the background: a foreground
 * service with a notification (devices, «Заблокировать», «Остановить») and a
 * Wi-Fi lock. The buttons only leave a word for Rust, which polls it.
 */
class SyncService : Service() {
  private var wifiLock: WifiManager.WifiLock? = null

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    when (intent?.action) {
      ACTION_LOCK -> pending = "lock"
      ACTION_STOP -> pending = "stop"
      else -> {
        try {
          val notification = build()
          if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
          } else {
            startForeground(NOTIFICATION_ID, notification)
          }
          holdWifi()
        } catch (ex: Exception) {
          // Not allowed right now (started from the background): sync simply
          // pauses with the app, as without the service.
          Logger.error("background sync refused: ${ex.message}")
          stopSelf()
        }
      }
    }
    return START_NOT_STICKY
  }

  override fun onDestroy() {
    wifiLock?.let { if (it.isHeld) it.release() }
    wifiLock = null
    super.onDestroy()
  }

  @Suppress("DEPRECATION")
  private fun holdWifi() {
    if (wifiLock != null) return
    val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    wifiLock = wifi.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "mwt-sync").apply {
      setReferenceCounted(false)
      acquire()
    }
  }

  private fun build(): Notification {
    val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    if (Build.VERSION.SDK_INT >= 26) {
      manager.createNotificationChannel(
        NotificationChannel(CHANNEL, texts.channel, NotificationManager.IMPORTANCE_LOW)
      )
    }
    val builder = if (Build.VERSION.SDK_INT >= 26) {
      Notification.Builder(this, CHANNEL)
    } else {
      @Suppress("DEPRECATION")
      Notification.Builder(this)
    }
    val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
      PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE)
    }
    return builder
      .setSmallIcon(applicationInfo.icon)
      .setContentTitle(texts.title)
      .setContentText(texts.text)
      .setOngoing(true)
      .setContentIntent(open)
      .addAction(action(ACTION_LOCK, texts.lock, 1))
      .addAction(action(ACTION_STOP, texts.stop, 2))
      .build()
  }

  @Suppress("DEPRECATION")
  private fun action(name: String, label: String, code: Int): Notification.Action {
    val intent = Intent(this, SyncService::class.java).setAction(name)
    val pending = PendingIntent.getService(this, code, intent, PendingIntent.FLAG_IMMUTABLE)
    return Notification.Action.Builder(applicationInfo.icon, label, pending).build()
  }

  data class Texts(
    val channel: String = "Sync",
    val title: String = "Sync",
    val text: String = "",
    val lock: String = "Lock",
    val stop: String = "Stop",
  )

  companion object {
    private const val CHANNEL = "mwt-sync"
    private const val NOTIFICATION_ID = 7317
    private const val ACTION_LOCK = "dev.mwt.net.LOCK"
    private const val ACTION_STOP = "dev.mwt.net.STOP"

    @Volatile var texts = Texts()

    /** The button pressed last («lock» or «stop»), taken by Rust. */
    @Volatile var pending: String? = null

    fun start(context: Context) {
      val intent = Intent(context, SyncService::class.java)
      if (Build.VERSION.SDK_INT >= 26) context.startForegroundService(intent) else context.startService(intent)
    }

    fun stop(context: Context) {
      context.stopService(Intent(context, SyncService::class.java))
    }
  }
}
