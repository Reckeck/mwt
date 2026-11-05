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
import android.os.PowerManager
import app.tauri.Logger

/**
 * Keeps vault sync running while the app is in the background: a foreground
 * service with a notification (devices, «Заблокировать», «Остановить») and a
 * Wi-Fi lock. The buttons only leave a word for Rust, which polls it.
 */
class SyncService : Service() {
  private var wifiLock: WifiManager.WifiLock? = null
  private var wakeLock: PowerManager.WakeLock? = null
  /** [startForeground] succeeded for this instance. */
  private var foreground = false

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    instance = this
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    // First, before anything that can throw: a start by startForegroundService
    // must reach startForeground, also for a stop that arrives right behind it.
    if (!foreground) {
      try {
        val minimal = minimal()
        if (Build.VERSION.SDK_INT >= 29) {
          startForeground(NOTIFICATION_ID, minimal, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        } else {
          startForeground(NOTIFICATION_ID, minimal)
        }
        foreground = true
      } catch (ex: Exception) {
        // Not allowed right now (started from the background): sync simply
        // pauses with the app, as without the service. Nothing to undo.
        Logger.error("background sync refused: ${ex.message}")
        requested = false
        stopSelf()
        return START_NOT_STICKY
      }
    }
    when (intent?.action) {
      ACTION_QUIT -> {
        quit()
        stopSelf(startId)
        return START_NOT_STICKY
      }
      ACTION_LOCK -> pending = "lock"
      ACTION_STOP -> pending = "stop"
    }
    refreshNow()
    return START_NOT_STICKY
  }

  override fun onDestroy() {
    if (instance === this) instance = null
    release()
    super.onDestroy()
  }

  private fun quit() {
    release()
    if (Build.VERSION.SDK_INT >= 24) {
      stopForeground(STOP_FOREGROUND_REMOVE)
    } else {
      @Suppress("DEPRECATION")
      stopForeground(true)
    }
    foreground = false
  }

  /** Redraw the notification from [texts] and follow [wake]; no new start needed. */
  @Synchronized
  fun refreshNow() {
    try {
      val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
      manager.notify(NOTIFICATION_ID, build())
      holdWifi()
      if (wake) holdCpu() else releaseCpu()
    } catch (ex: Exception) {
      Logger.error("cannot refresh background sync: ${ex.message}")
    }
  }

  @Synchronized
  private fun release() {
    wifiLock?.let { if (it.isHeld) it.release() }
    wifiLock = null
    releaseCpu()
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

  /** A partial wake lock while a link is live; asking again is harmless. */
  private fun holdCpu() {
    val lock = wakeLock ?: (getSystemService(Context.POWER_SERVICE) as PowerManager)
      .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mwt:sync")
      .apply { setReferenceCounted(false) }
      .also { wakeLock = it }
    if (!lock.isHeld) lock.acquire()
  }

  private fun releaseCpu() {
    wakeLock?.let { if (it.isHeld) it.release() }
  }

  /** What [startForeground] shows until [refreshNow] draws the full one. */
  private fun minimal(): Notification = builder()
    .setSmallIcon(applicationInfo.icon)
    .setContentTitle(texts.title)
    .setOngoing(true)
    .build()

  private fun builder(): Notification.Builder {
    val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    if (Build.VERSION.SDK_INT >= 26) {
      manager.createNotificationChannel(
        NotificationChannel(CHANNEL, texts.channel, NotificationManager.IMPORTANCE_LOW)
      )
    }
    return if (Build.VERSION.SDK_INT >= 26) {
      Notification.Builder(this, CHANNEL)
    } else {
      @Suppress("DEPRECATION")
      Notification.Builder(this)
    }
  }

  private fun build(): Notification {
    val builder = builder()
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
    private const val ACTION_QUIT = "dev.mwt.net.QUIT"

    @Volatile var texts = Texts()

    /** At least one link is live: hold the CPU wake lock. */
    @Volatile var wake = false

    /** The running service, to redraw its notification without a new start. */
    @Volatile private var instance: SyncService? = null

    /** [start] was called and no [stop] since. */
    @Volatile private var requested = false

    fun refresh() {
      instance?.refreshNow()
    }

    /** The button pressed last («lock» or «stop»), taken by Rust. */
    @Volatile var pending: String? = null

    fun start(context: Context) {
      requested = true
      try {
        launch(context, null)
      } catch (ex: Exception) {
        requested = false
        throw ex
      }
    }

    /**
     * Idempotent. A stop is a command the service handles after its
     * `startForeground`, so it is safe right behind a start.
     */
    fun stop(context: Context) {
      if (!requested) return
      requested = false
      try {
        launch(context, ACTION_QUIT)
      } catch (ex: Exception) {
        context.stopService(Intent(context, SyncService::class.java))
      }
    }

    private fun launch(context: Context, action: String?) {
      val intent = Intent(context, SyncService::class.java).setAction(action)
      if (Build.VERSION.SDK_INT >= 26) context.startForegroundService(intent) else context.startService(intent)
    }
  }
}
