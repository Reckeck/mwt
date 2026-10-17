//! Android Storage Access Framework pickers that keep their grant.
//!
//! `tauri-plugin-dialog` opens files with `ACTION_GET_CONTENT`, whose read
//! grant ends with the activity: fine for a one-off copy, useless for a
//! document the app writes to after every save. Here the document is picked
//! with `ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT` and the grant is made
//! persistable, so the vault's external copy survives a restart. Reading and
//! writing then go through `tauri-plugin-fs` (`app.fs().open(url, …)`).
//!
//! Off Android every call answers that it is not supported.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "dev.mwt.saf";

/// MIME type a new vault document is created with (`.wallet` has none).
const VAULT_MIME: &str = "application/octet-stream";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize)]
struct Picked {
    uri: Option<String>,
}

#[derive(Serialize)]
struct CreateArgs<'a> {
    name: &'a str,
    mime: &'a str,
}

#[derive(Serialize)]
struct UriArgs<'a> {
    uri: &'a str,
}

/// Access to the pickers: `app.saf()`.
pub struct Saf<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    _runtime: PhantomData<fn() -> R>,
}

impl<R: Runtime> Saf<R> {
    /// An existing document, readable and writable; `None` when cancelled.
    pub fn open_document(&self) -> Result<Option<String>> {
        self.run::<Picked, _>("openDocument", ())
            .map(|picked| picked.uri)
    }

    /// A new document named `name`; `None` when cancelled.
    pub fn create_document(&self, name: &str) -> Result<Option<String>> {
        let args = CreateArgs {
            name,
            mime: VAULT_MIME,
        };
        self.run::<Picked, _>("createDocument", args)
            .map(|picked| picked.uri)
    }

    /// Give back the long-lived grant of `uri`.
    pub fn release(&self, uri: &str) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("releasePermission", UriArgs { uri })
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
        Err(format!(
            "{command}: the Storage Access Framework exists on Android only"
        ))
    }
}

pub trait SafExt<R: Runtime> {
    fn saf(&self) -> &Saf<R>;
}

impl<R: Runtime, T: Manager<R>> SafExt<R> for T {
    fn saf(&self) -> &Saf<R> {
        self.state::<Saf<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("saf")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let saf = Saf {
                handle: api.register_android_plugin(PLUGIN_IDENTIFIER, "SafPlugin")?,
                _runtime: PhantomData,
            };
            #[cfg(not(target_os = "android"))]
            let saf = {
                let _ = api;
                Saf::<R> {
                    _runtime: PhantomData,
                }
            };
            app.manage(saf);
            Ok(())
        })
        .build()
}
