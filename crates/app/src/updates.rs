//! Self-update. Installs that can replace themselves (AppImage, NSIS/MSI,
//! macOS .app) check the signed manifest through `tauri-plugin-updater`,
//! download in the background and apply on exit (or on "Restart now");
//! every other install (deb/rpm, Pacman, Flatpak, Android) only learns that a
//! newer version exists and is pointed at the right place (`UpdateOffer`).
//!
//! The state mutex is held for short, synchronous sections only, never
//! across an `.await` (AGENTS.md rule 3).

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use trade_core::ipc_contract::{UpdatePhase, UpdateStatusDto};
use trade_core::updates::{
    AppVersion, Channel, InstallKind, fetch_manifest, manifest_url, offer_from_manifest,
};

use crate::commands::app_settings::{config_dir, current_settings, save};

pub const STATE_EVENT: &str = "update-state";

#[derive(Default)]
struct Inner {
    phase: Option<UpdatePhase>,
    /// A downloaded, verified package waiting for the exit.
    #[cfg(desktop)]
    pending: Option<(tauri_plugin_updater::Update, Vec<u8>)>,
}

#[derive(Default)]
pub struct UpdateState {
    inner: Mutex<Inner>,
}

impl UpdateState {
    fn phase(&self) -> UpdatePhase {
        let guard = self.inner.lock().expect("update state");
        guard.phase.clone().unwrap_or(UpdatePhase::Idle)
    }
}

pub fn channel() -> Channel {
    channel_of(crate::app_id::get())
}

fn channel_of(identifier: &str) -> Channel {
    if identifier.starts_with("dev.") {
        Channel::Dev
    } else {
        Channel::Prod
    }
}

pub fn detect_install_kind() -> InstallKind {
    #[cfg(target_os = "android")]
    {
        InstallKind::Android
    }
    #[cfg(not(target_os = "android"))]
    {
        detect_desktop_kind()
    }
}

#[cfg(all(desktop, target_os = "linux"))]
fn detect_desktop_kind() -> InstallKind {
    use tauri::utils::config::BundleType;
    use tauri::utils::platform::bundle_type;
    if std::env::var_os("APPIMAGE").is_some() {
        return InstallKind::AppImage;
    }
    if std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists() {
        return InstallKind::Flatpak;
    }
    match bundle_type() {
        Some(BundleType::Deb) => InstallKind::Deb,
        Some(BundleType::Rpm) => InstallKind::Rpm,
        Some(BundleType::AppImage) => InstallKind::AppImage,
        // A package built outside the Tauri bundler (the AUR one) lives under /usr.
        _ if std::env::current_exe().is_ok_and(|exe| exe.starts_with("/usr")) => {
            InstallKind::Pacman
        }
        _ => InstallKind::Unknown,
    }
}

#[cfg(all(desktop, target_os = "windows"))]
fn detect_desktop_kind() -> InstallKind {
    use tauri::utils::config::BundleType;
    match tauri::utils::platform::bundle_type() {
        Some(BundleType::Nsis) => InstallKind::WindowsNsis,
        Some(BundleType::Msi) => InstallKind::WindowsMsi,
        _ => InstallKind::Unknown,
    }
}

#[cfg(all(desktop, target_os = "macos"))]
fn detect_desktop_kind() -> InstallKind {
    InstallKind::MacApp
}

#[cfg(not(any(
    all(desktop, target_os = "linux"),
    all(desktop, target_os = "windows"),
    all(desktop, target_os = "macos"),
    target_os = "android"
)))]
fn detect_desktop_kind() -> InstallKind {
    InstallKind::Unknown
}

/// The manifest to read; a debug build may point `MWT_UPDATE_URL` at a local file server.
fn manifest_location(channel: Channel) -> String {
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("MWT_UPDATE_URL") {
        return url;
    }
    manifest_url(channel).to_owned()
}

pub fn status(app: &AppHandle) -> UpdateStatusDto {
    let settings = current_settings(app);
    UpdateStatusDto {
        phase: app.state::<UpdateState>().phase(),
        current_version: app.package_info().version.to_string(),
        install_kind: detect_install_kind(),
        channel: channel(),
        auto_update: settings.auto_update(),
        last_check: settings.last_update_check.clone(),
    }
}

fn set_phase(app: &AppHandle, phase: UpdatePhase) {
    app.state::<UpdateState>()
        .inner
        .lock()
        .expect("update state")
        .phase = Some(phase);
    let _ = app.emit(STATE_EVENT, status(app));
}

/// Whether an automatic check has anything to do right now.
pub fn auto_check_due(app: &AppHandle) -> bool {
    matches!(
        app.state::<UpdateState>().phase(),
        UpdatePhase::Idle | UpdatePhase::UpToDate | UpdatePhase::Error { .. }
    )
}

fn stamp_last_check(app: &AppHandle) {
    let dir = config_dir(app);
    let mut settings = current_settings(app);
    settings.last_update_check = Some(chrono::Utc::now().to_rfc3339());
    if let Err(err) = save(&dir, &settings) {
        eprintln!("updates: cannot save the check time: {err}");
    }
}

/// One check. A manual one reports failures; an automatic one stays silent
/// (offline is normal) and leaves the previous state.
pub async fn run_check(app: AppHandle, manual: bool) {
    match app.state::<UpdateState>().phase() {
        UpdatePhase::Checking | UpdatePhase::Downloading { .. } => return,
        UpdatePhase::Ready { .. } if !manual => return,
        _ => {}
    }
    let before = app.state::<UpdateState>().phase();
    set_phase(&app, UpdatePhase::Checking);
    let kind = detect_install_kind();
    let result = if kind.can_self_update() {
        check_self_update(&app).await
    } else {
        check_notice(&app, kind).await
    };
    stamp_last_check(&app);
    match result {
        Ok(phase) => set_phase(&app, phase),
        Err(message) if manual => set_phase(&app, UpdatePhase::Error { message }),
        Err(message) => {
            eprintln!("updates: automatic check failed: {message}");
            set_phase(&app, before);
        }
    }
}

/// Notice-only installs: read our manifest, decide, never download.
async fn check_notice(app: &AppHandle, kind: InstallKind) -> Result<UpdatePhase, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("multi-wallet-tracker")
        .build()
        .map_err(|err| err.to_string())?;
    let channel = channel();
    let manifest = fetch_manifest(&client, &manifest_location(channel))
        .await
        .map_err(|err| err.to_string())?;
    let current = AppVersion::parse(&app.package_info().version.to_string())
        .map_err(|err| err.to_string())?;
    let offer =
        offer_from_manifest(&manifest, &current, channel, kind).map_err(|err| err.to_string())?;
    Ok(
        offer.map_or(UpdatePhase::UpToDate, |offer| UpdatePhase::Available {
            offer,
        }),
    )
}

#[cfg(desktop)]
async fn check_self_update(app: &AppHandle) -> Result<UpdatePhase, String> {
    use tauri_plugin_updater::UpdaterExt;
    let channel = channel();
    let url = manifest_location(channel)
        .parse::<tauri::Url>()
        .map_err(|err| err.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|err| err.to_string())?
        // The same rule as the notice path: prod skips pre-releases, nothing older is offered.
        .version_comparator(move |current, remote| {
            match (
                AppVersion::parse(&current.to_string()),
                AppVersion::parse(&remote.version.to_string()),
            ) {
                (Ok(current), Ok(offered)) => {
                    trade_core::updates::is_newer(&current, &offered, channel)
                }
                _ => false,
            }
        })
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|err| err.to_string())?;
    let Some(update) = updater.check().await.map_err(|err| err.to_string())? else {
        return Ok(UpdatePhase::UpToDate);
    };
    let version = update.version.clone();
    set_phase(app, UpdatePhase::Downloading { percent: 0 });
    let mut received: u64 = 0;
    let mut last_percent: u8 = 0;
    let progress_app = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                received += chunk as u64;
                let percent = total.filter(|total| *total > 0).map_or(0, |total| {
                    (received.saturating_mul(100) / total).min(100) as u8
                });
                if percent != last_percent {
                    last_percent = percent;
                    set_phase(&progress_app, UpdatePhase::Downloading { percent });
                }
            },
            || {},
        )
        .await
        .map_err(|err| err.to_string())?;
    app.state::<UpdateState>()
        .inner
        .lock()
        .expect("update state")
        .pending = Some((update, bytes));
    Ok(UpdatePhase::Ready { version })
}

#[cfg(not(desktop))]
async fn check_self_update(_app: &AppHandle) -> Result<UpdatePhase, String> {
    Ok(UpdatePhase::UpToDate)
}

/// Install the downloaded package, if any. On Windows a successful install
/// exits the process; elsewhere the new files are used at the next start.
#[cfg(desktop)]
pub fn apply_pending(app: &AppHandle) -> Result<bool, String> {
    let pending = app
        .state::<UpdateState>()
        .inner
        .lock()
        .expect("update state")
        .pending
        .take();
    let Some((update, bytes)) = pending else {
        return Ok(false);
    };
    update.install(bytes).map_err(|err| err.to_string())?;
    Ok(true)
}

#[cfg(not(desktop))]
pub fn apply_pending(_app: &AppHandle) -> Result<bool, String> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_identifiers_are_the_dev_channel() {
        assert_eq!(channel_of("dev.mwt.desktop"), Channel::Dev);
        assert_eq!(channel_of("dev.mwt.app"), Channel::Dev);
        assert_eq!(channel_of("mwt.desktop"), Channel::Prod);
        assert_eq!(channel_of("mwt.app"), Channel::Prod);
    }
}
