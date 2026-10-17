package dev.mwt.saf

import android.app.Activity
import android.content.Intent
import android.net.Uri
import androidx.activity.result.ActivityResult
import app.tauri.Logger
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class CreateArgs {
  lateinit var name: String
  var mime: String? = null
}

@InvokeArg
class UriArgs {
  lateinit var uri: String
}

/**
 * Document pickers whose read/write grant survives a restart
 * (ACTION_OPEN_DOCUMENT / ACTION_CREATE_DOCUMENT + takePersistableUriPermission).
 * A cancelled picker resolves with no `uri` instead of rejecting.
 */
@TauriPlugin
class SafPlugin(private val activity: Activity) : Plugin(activity) {
  private val readWrite =
    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

  @Command
  fun openDocument(invoke: Invoke) {
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
      addCategory(Intent.CATEGORY_OPENABLE)
      type = "*/*"
      addFlags(readWrite or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
    }
    startActivityForResult(invoke, intent, "documentPicked")
  }

  @Command
  fun createDocument(invoke: Invoke) {
    val args = invoke.parseArgs(CreateArgs::class.java)
    val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
      addCategory(Intent.CATEGORY_OPENABLE)
      type = args.mime ?: "application/octet-stream"
      putExtra(Intent.EXTRA_TITLE, args.name)
      addFlags(readWrite or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
    }
    startActivityForResult(invoke, intent, "documentPicked")
  }

  @ActivityCallback
  fun documentPicked(invoke: Invoke, result: ActivityResult) {
    val out = JSObject()
    val data = result.data
    val uri: Uri? = if (result.resultCode == Activity.RESULT_OK) data?.data else null
    if (uri != null) {
      // Keep only what was granted; a provider without persistable grants
      // still works until the app restarts.
      val granted = (data?.flags ?: 0) and readWrite
      try {
        activity.contentResolver.takePersistableUriPermission(uri, granted)
      } catch (ex: SecurityException) {
        Logger.error("no persistable grant for $uri: ${ex.message}")
      }
      out.put("uri", uri.toString())
    }
    invoke.resolve(out)
  }

  @Command
  fun releasePermission(invoke: Invoke) {
    val args = invoke.parseArgs(UriArgs::class.java)
    try {
      activity.contentResolver.releasePersistableUriPermission(Uri.parse(args.uri), readWrite)
    } catch (ex: SecurityException) {
      Logger.error("release ${args.uri}: ${ex.message}")
    }
    invoke.resolve()
  }
}
