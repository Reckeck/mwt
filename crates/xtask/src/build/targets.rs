//! One build task per platform: the same scripts and images as GitLab CI (`scripts/ci/build-*.sh`).

use std::sync::Arc;

use super::env::Env;
use crate::channel::Channel;
use crate::exec::{Cmd, Ctx, which};
use crate::graph::Outcome;
use crate::tauri_config;

/// The overlay the build scripts pass to `tauri build --config`: written here, one file per lane
/// (builds run side by side). The frontend is built once on the host (`build:frontend`), so the
/// overlay has an empty `beforeBuildCommand`.
pub const OVERLAY_VAR: &str = "MWT_TAURI_CONFIG";
/// build-android.sh leaves the committed icons alone when they are a private copy.
pub const ICONS_VAR: &str = "MWT_ICONS_ISOLATED";

/// Writes the lane's overlay on the host; the path as the build script sees it: the working
/// copy's own path natively, `/src/...` in a container.
pub fn overlay(env: &Env, lane: &str, android: bool, container: bool) -> Result<String, Outcome> {
    let rel = format!(".cache/tauri-ci-{lane}.json");
    let params = tauri_config::Params {
        channel: env.channel,
        android,
        version: &env.version,
        version_code: env.version_code,
        prebuilt_frontend: true,
    };
    let path = tauri_config::write(&env.root, &rel, &params)
        .map_err(|error| Outcome::Fail(format!("{error:#}")))?;
    Ok(if container {
        format!("/src/{rel}")
    } else {
        path.display().to_string()
    })
}

/// `-e` arguments of a container run for a platform build.
fn tauri_args(env: &Env, lane: &str, android: bool) -> Result<Vec<String>, Outcome> {
    let path = overlay(env, lane, android, true)?;
    Ok(vec!["-e".into(), format!("{OVERLAY_VAR}={path}")])
}

/// The channel's names for a native script run.
pub fn with_names(mut cmd: Cmd, channel: Channel) -> Cmd {
    for (name, value) in channel.env() {
        cmd = cmd.env(name, value);
    }
    cmd
}

fn quoted(env: &Env) -> String {
    format!("'{}' '{}'", env.version, env.out)
}

pub async fn linux(env: Arc<Env>, ctx: Ctx) -> Outcome {
    if env.native {
        let config = match overlay(&env, "linux", false, false) {
            Ok(path) => path,
            Err(outcome) => return outcome,
        };
        let cmd = with_names(
            Cmd::new("scripts/ci/build-linux.sh").args([&env.version, &env.out]),
            env.channel,
        )
        .env(OVERLAY_VAR, &config);
        return crate::graph::run_cmd(&ctx, &cmd).await;
    }
    let image = match env.ci_image(&ctx, "linux").await {
        Ok(image) => image,
        Err(outcome) => return outcome,
    };
    let script = format!("scripts/ci/build-linux.sh {}", quoted(&env));
    let more = match tauri_args(&env, "linux", false) {
        Ok(more) => more,
        Err(outcome) => return outcome,
    };
    match env.run_ci("linux", &image, &script, more) {
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
    let mut more = match tauri_args(&env, "android", true) {
        Ok(more) => more,
        Err(outcome) => return outcome,
    };
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
    let more = match tauri_args(&env, "windows", false) {
        Ok(more) => more,
        Err(outcome) => return outcome,
    };
    match env.run_ci("windows", &image, &script, more) {
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
        let cmd = with_names(
            Cmd::new("scripts/ci/build-flatpak.sh").args([&env.version, &env.out]),
            env.channel,
        );
        return crate::graph::run_cmd(&ctx, &cmd).await;
    }
    // flatpak-builder needs bubblewrap.
    let script = format!("scripts/ci/build-flatpak.sh {}", quoted(&env));
    match env.run_arch("flatpak", &script, &["--privileged"]) {
        Ok(cmd) => env.finish(&ctx, "flatpak", cmd).await,
        Err(outcome) => outcome,
    }
}
