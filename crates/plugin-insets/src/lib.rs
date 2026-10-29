//! Android system bars and the webview (docs/architecture.md):
//!
//! - the status bar, the navigation bar, a display cutout and the keyboard
//!   become CSS variables `--native-inset-top|right|bottom|left` (px) and
//!   `--native-ime` (1 while the keyboard is up), since Android's WebView
//!   gives `env(safe-area-inset-*)` as zero;
//! - the bar icons follow the app's theme (dark icons on a light page);
//! - «minimise» for the system Back on the home screen.
//!
//! Off Android every call answers that it is not supported.

use std::marker::PhantomData;

use serde::Serialize;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Runtime};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "dev.mwt.insets";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Serialize)]
struct Bars {
    dark: bool,
}

/// `app.insets()`.
pub struct Insets<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    _runtime: PhantomData<fn() -> R>,
}

impl<R: Runtime> Insets<R> {
    /// Write the last known insets into the page again (a fresh page has none).
    pub fn apply(&self) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("apply", ())
            .map(|_| ())
    }

    /// Light icons on a dark page, dark ones on a light page.
    pub fn set_bar_style(&self, dark: bool) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("setBarStyle", Bars { dark })
            .map(|_| ())
    }

    /// Send the app to the background, as Home would.
    pub fn move_to_back(&self) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("moveToBack", ())
            .map(|_| ())
    }

    #[cfg(target_os = "android")]
    fn run<T: serde::de::DeserializeOwned, P: Serialize>(
        &self,
        command: &str,
        payload: P,
    ) -> Result<T> {
        self.handle
            .run_mobile_plugin(command, payload)
            .map_err(|err| err.to_string())
    }

    #[cfg(not(target_os = "android"))]
    fn run<T, P: Serialize>(&self, command: &str, _payload: P) -> Result<T> {
        Err(format!("{command}: Android only"))
    }
}

pub trait InsetsExt<R: Runtime> {
    fn insets(&self) -> &Insets<R>;
}

impl<R: Runtime, T: Manager<R>> InsetsExt<R> for T {
    fn insets(&self) -> &Insets<R> {
        self.state::<Insets<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("insets")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let insets = Insets {
                handle: api.register_android_plugin(PLUGIN_IDENTIFIER, "InsetsPlugin")?,
                _runtime: PhantomData,
            };
            #[cfg(not(target_os = "android"))]
            let insets = {
                let _ = api;
                Insets::<R> {
                    _runtime: PhantomData,
                }
            };
            app.manage(insets);
            Ok(())
        })
        // Each loaded page starts without the variables: hand them over again.
        // The hook runs on the Android UI thread, and `run_mobile_plugin` waits
        // for that same thread to run the Kotlin side: called here, it deadlocks
        // (black screen, then «isn't responding»). So it goes off the UI thread.
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let app = webview.app_handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = app.insets().apply();
                });
            }
        })
        .build()
}
