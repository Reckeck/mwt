//! Android side of LAN vault sync (docs/sync.md):
//!
//! - the Wi-Fi multicast lock, without which Android drops the mDNS packets
//!   devices find each other with;
//! - a foreground service that keeps sync running in the background, with
//!   a notification and a Wi-Fi lock;
//! - wrapping the device's sync key with a non-exportable AES key of the
//!   Android Keystore (StrongBox when the phone has one), so the key at rest
//!   cannot be read from the app's files.
//!
//! Off Android every call answers that it is not supported.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "dev.mwt.net";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Serialize, Deserialize)]
struct Hex {
    hex: String,
}

#[derive(Serialize)]
struct Multicast {
    on: bool,
}

/// The background sync notification (`Net::background`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Background {
    /// Sync runs: going to the background starts the foreground service.
    pub armed: bool,
    /// At least one link is live: the service holds a partial wake lock.
    pub wake: bool,
    pub channel: String,
    pub title: String,
    pub text: String,
    pub lock: String,
    pub stop: String,
}

#[derive(Deserialize)]
struct Language {
    language: String,
}

/// What [`Net::poll`] reports.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Poll {
    pub background: bool,
    /// The notification button pressed since the last poll: `lock` or `stop`.
    pub action: Option<String>,
}

/// `app.net()`.
pub struct Net<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    _runtime: PhantomData<fn() -> R>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hex".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| {
            text.get(at..at + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(|| "bad hex".to_string())
        })
        .collect()
}

impl<R: Runtime> Net<R> {
    /// Hold (or let go of) the multicast lock while sync is on.
    pub fn multicast(&self, on: bool) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("multicast", Multicast { on })
            .map(|_| ())
    }

    /// Arm (or disarm) background sync, with the notification's texts.
    pub fn background(&self, state: &Background) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("background", state.clone())
            .map(|_| ())
    }

    /// Whether the app is in the background, and the button pressed.
    pub fn poll(&self) -> Result<Poll> {
        self.run("poll", ())
    }

    /// The system's language code (`ru`, `en`, …).
    pub fn language(&self) -> Result<String> {
        let out: Language = self.run("language", ())?;
        Ok(out.language)
    }

    /// Ask for the notification permission (Android 13+), without waiting.
    pub fn ask_notifications(&self) -> Result<()> {
        self.run::<serde::de::IgnoredAny, _>("askNotifications", ())
            .map(|_| ())
    }

    /// `secret` sealed with the Keystore key (IV + AES-GCM).
    pub fn wrap(&self, secret: &[u8]) -> Result<Vec<u8>> {
        let out: Hex = self.run("wrap", Hex { hex: hex(secret) })?;
        unhex(&out.hex)
    }

    /// The inverse of [`Self::wrap`].
    pub fn unwrap(&self, wrapped: &[u8]) -> Result<Vec<u8>> {
        let out: Hex = self.run("unwrap", Hex { hex: hex(wrapped) })?;
        unhex(&out.hex)
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

pub trait NetExt<R: Runtime> {
    fn net(&self) -> &Net<R>;
}

impl<R: Runtime, T: Manager<R>> NetExt<R> for T {
    fn net(&self) -> &Net<R> {
        self.state::<Net<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("net")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let net = Net {
                handle: api.register_android_plugin(PLUGIN_IDENTIFIER, "NetPlugin")?,
                _runtime: PhantomData,
            };
            #[cfg(not(target_os = "android"))]
            let net = {
                let _ = api;
                Net::<R> {
                    _runtime: PhantomData,
                }
            };
            app.manage(net);
            Ok(())
        })
        .build()
}
