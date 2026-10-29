//! One build task per platform: the same scripts and images as GitLab CI (`scripts/ci/build-*.sh`).

use std::sync::Arc;

use super::env::Env;
use crate::exec::{Cmd, Ctx, which};
use crate::graph::Outcome;

/// Set for the platform builds: the frontend is built once on the host (`build:frontend`), so
/// tauri-config.sh puts an empty `beforeBuildCommand` into the overlay. GitLab CI never sets it.
pub const PREBUILT_FRONTEND: &str = "MWT_PREBUILT_FRONTEND";
/// Where tauri-config.sh writes the overlay: one file per lane, builds run side by side.
pub const OVERLAY_VAR: &str = "TAURI_CI_CONFIG";
/// build-android.sh leaves the committed icons alone when they are a private copy.
pub const ICONS_VAR: &str = "MWT_ICONS_ISOLATED";

fn overlay(lane: &str) -> String {
    format!(".cache/tauri-ci-{lane}.json")
}

/// `-e` arguments of a container run for a platform build.
fn tauri_args(lane: &str) -> Vec<String> {
    vec![
        "-e".into(),
        format!("{PREBUILT_FRONTEND}=1"),
        "-e".into(),
        format!("{OVERLAY_VAR}={}", overlay(lane)),
    ]
}

fn quoted(env: &Env) -> String {
    format!("'{}' '{}'", env.version, env.out)
}

pub async fn linux(env: Arc<Env>, ctx: Ctx) -> Outcome {
    if env.native {
        let cmd = Cmd::new("scripts/ci/build-linux.sh")
            .args([&env.version, &env.out])
            .env("RELEASE_CHANNEL", &env.channel)
            .env(PREBUILT_FRONTEND, "1")
            .env(OVERLAY_VAR, overlay("linux"));
        return crate::graph::run_cmd(&ctx, &cmd).await;
    }
    let image = match env.ci_image(&ctx, "linux").await {
        Ok(image) => image,
        Err(outcome) => return outcome,
    };
    let script = format!("scripts/ci/build-linux.sh {}", quoted(&env));
    match env.run_ci("linux", &image, &script, tauri_args("linux")) {
        Ok(cmd) => env.finish(&ctx, "linux", cmd).await,
        Err(outcome) => outcome,
    }
}

pub async fn android(env: Arc<Env>, ctx: Ctx) -> Outcome {
    let image = match env.ci_image(&ctx, "android").await {
        Ok(image) => image,
        Err(outcome) => return outcome,
    };
    // `tauri icon` rewrites the committed icons while linux and windows read them: android works
    // on a private copy mounted over the folder.
    let icons = ctx.root.join(".cache/xtask/android-icons");
    if let Err(error) = crate::guard::copy_dir(&ctx.root.join("crates/app/icons"), &icons) {
        return Outcome::Fail(format!("cannot copy the icons: {error}"));
    }
    let mut more = tauri_args("android");
    more.extend([
        "-v".into(),
        format!("{}:/src/crates/app/icons", icons.display()),
        "-e".into(),
        format!("{ICONS_VAR}=1"),
    ]);
    let script = format!(
        "source scripts/ci/setup-android.sh; scripts/ci/build-android.sh {}",
        quoted(&env)
    );
    match env.run_ci("android", &image, &script, more) {
        Ok(cmd) => env.finish(&ctx, "android", cmd).await,
        Err(outcome) => outcome,
    }
}

pub async fn windows(env: Arc<Env>, ctx: Ctx) -> Outcome {
    let image = match env.windows_image(&ctx).await {
        Ok(image) => image,
        Err(outcome) => return outcome,
    };
    let script = format!("scripts/local/build-windows-cross.sh {}", quoted(&env));
    match env.run_ci("windows", &image, &script, tauri_args("windows")) {
        Ok(cmd) => env.finish(&ctx, "windows", cmd).await,
        Err(outcome) => outcome,
    }
}

pub async fn pacman(env: Arc<Env>, ctx: Ctx) -> Outcome {
    let script = format!("scripts/ci/build-pacman.sh {}", quoted(&env));
    match env.run_arch("pacman", &script, &[]) {
        Ok(cmd) => env.finish(&ctx, "pacman", cmd).await,
        Err(outcome) => outcome,
    }
}

pub async fn flatpak(env: Arc<Env>, ctx: Ctx) -> Outcome {
    if env.native && which("flatpak-builder").is_some() {
        let cmd = Cmd::new("scripts/ci/build-flatpak.sh")
            .args([&env.version, &env.out])
            .env("RELEASE_CHANNEL", &env.channel);
        return crate::graph::run_cmd(&ctx, &cmd).await;
    }
    // flatpak-builder needs bubblewrap.
    let script = format!("scripts/ci/build-flatpak.sh {}", quoted(&env));
    match env.run_arch("flatpak", &script, &["--privileged"]) {
        Ok(cmd) => env.finish(&ctx, "flatpak", cmd).await,
        Err(outcome) => outcome,
    }
}
