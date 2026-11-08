//! Android biometric quick unlock (H7B): an AES-256-GCM key in the Android
//! Keystore that works only after a `BiometricPrompt` gesture (Class 3 / STRONG),
//! used through a `CryptoObject` so the prompt is bound to the key.
//!
//! The Kotlin side is called from Rust only (`run_mobile_plugin`); no command is
//! exposed to the webview. The plugin keeps its handle in a process-wide slot so
//! the biometric backend (a unit struct without an `AppHandle`) can reach it.
//!
//! Off Android every call answers that it is not supported.

use serde::{Deserialize, Serialize};
use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "dev.mwt.biometric";

pub type Result<T> = std::result::Result<T, String>;

/// How a prompt ended, as the Kotlin side reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    /// The user dismissed the prompt (negative button, back, cancel).
    Cancelled,
    /// The key is gone or was invalidated by a new biometric enrollment.
    Invalidated,
    /// No hardware or no enrolled strong biometric.
    Unavailable,
    Failed,
}

#[derive(Deserialize)]
struct Outcome {
    status: Status,
    hex: Option<String>,
}

#[derive(Deserialize)]
struct Availability {
    available: bool,
}

#[derive(Serialize)]
struct KeyArgs<'a> {
    alias: &'a str,
}

#[derive(Serialize)]
struct SealArgs<'a> {
    alias: &'a str,
    hex: String,
}

type Caller = Box<dyn Fn(&str, serde_json::Value) -> Result<serde_json::Value> + Send + Sync>;

static CALLER: std::sync::OnceLock<Caller> = std::sync::OnceLock::new();

fn call<T: serde::de::DeserializeOwned, P: Serialize>(command: &str, payload: P) -> Result<T> {
    let caller = CALLER
        .get()
        .ok_or_else(|| "the biometric plugin is not registered (Android only)".to_string())?;
    let payload = serde_json::to_value(payload).map_err(|err| err.to_string())?;
    let value = caller(command, payload)?;
    serde_json::from_value(value).map_err(|err| err.to_string())
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

/// A strong biometric is enrolled and the hardware is there. Does not prompt.
pub fn available() -> Result<bool> {
    call::<Availability, _>("available", ()).map(|out| out.available)
}

/// Create a fresh key under `alias` (replacing any) and encrypt `secret` with it
/// after a biometric prompt. Returns `iv || ciphertext`.
pub fn encrypt(alias: &str, secret: &[u8]) -> Result<(Status, Vec<u8>)> {
    seal("encrypt", alias, secret)
}

/// Decrypt `iv || ciphertext` with the key under `alias` after a biometric prompt.
/// `Status::Invalidated` also means the key was deleted.
pub fn decrypt(alias: &str, blob: &[u8]) -> Result<(Status, Vec<u8>)> {
    seal("decrypt", alias, blob)
}

fn seal(command: &str, alias: &str, input: &[u8]) -> Result<(Status, Vec<u8>)> {
    let out: Outcome = call(
        command,
        SealArgs {
            alias,
            hex: hex(input),
        },
    )?;
    let data = match (out.status, out.hex) {
        (Status::Ok, Some(text)) => unhex(&text)?,
        (Status::Ok, None) => return Err("no data in the answer".into()),
        _ => Vec::new(),
    };
    Ok((out.status, data))
}

/// Delete the key under `alias`; a missing key is fine.
pub fn delete_key(alias: &str) -> Result<()> {
    call::<serde::de::IgnoredAny, _>("deleteKey", KeyArgs { alias }).map(|_| ())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("biometric")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "BiometricPlugin")?;
                let caller: Caller = Box::new(move |command, payload| {
                    handle
                        .run_mobile_plugin(command, payload)
                        .map_err(|err| err.to_string())
                });
                CALLER.set(caller).ok();
            }
            #[cfg(not(target_os = "android"))]
            let _ = api;
            Ok(())
        })
        .build()
}
