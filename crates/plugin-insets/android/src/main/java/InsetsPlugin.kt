package dev.mwt.insets

import android.app.Activity
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import kotlin.math.max

@InvokeArg
class BarArgs {
  var dark: Boolean = false
}

/**
 * The system bars, the cutout and the keyboard as CSS variables on the page
 * (`--native-inset-*` in px, `--native-ime` 1/0): Android's WebView reports
 * `env(safe-area-inset-*)` as zero, so the page cannot see them otherwise.
 * The insets are only read, never consumed: the WebView still lays out
 * edge to edge.
 */
@TauriPlugin
class InsetsPlugin(private val activity: Activity) : Plugin(activity) {
  private var webView: WebView? = null
  /** top, right, bottom, left in CSS px. */
  private var last = floatArrayOf(0f, 0f, 0f, 0f)
  private var imeShown = false

  override fun load(webView: WebView) {
    super.load(webView)
    this.webView = webView
    WindowCompat.setDecorFitsSystemWindows(activity.window, false)
    ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      val density = activity.resources.displayMetrics.density
      last = floatArrayOf(
        bars.top / density,
        bars.right / density,
        max(bars.bottom, ime.bottom) / density,
        bars.left / density,
      )
      imeShown = insets.isVisible(WindowInsetsCompat.Type.ime())
      push()
      insets
    }
    ViewCompat.requestApplyInsets(webView)
  }

  private fun push() {
    val (top, right, bottom, left) = last.toList()
    val js = "(function(){var s=document.documentElement.style;" +
      "s.setProperty('--native-inset-top','${top}px');" +
      "s.setProperty('--native-inset-right','${right}px');" +
      "s.setProperty('--native-inset-bottom','${bottom}px');" +
      "s.setProperty('--native-inset-left','${left}px');" +
      "s.setProperty('--native-ime','${if (imeShown) 1 else 0}');})()"
    activity.runOnUiThread { webView?.evaluateJavascript(js, null) }
  }

  @Command
  fun apply(invoke: Invoke) {
    activity.runOnUiThread { webView?.let { ViewCompat.requestApplyInsets(it) } }
    push()
    invoke.resolve()
  }

  @Command
  fun setBarStyle(invoke: Invoke) {
    val args = invoke.parseArgs(BarArgs::class.java)
    activity.runOnUiThread {
      val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
      controller.isAppearanceLightStatusBars = !args.dark
      controller.isAppearanceLightNavigationBars = !args.dark
    }
    invoke.resolve()
  }

  @Command
  fun moveToBack(invoke: Invoke) {
    activity.runOnUiThread { activity.moveTaskToBack(true) }
    invoke.resolve()
  }
}
