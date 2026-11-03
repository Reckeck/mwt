//! The native Windows build (a Windows host: the GitHub runner or a developer machine): NSIS
//! installer, one MSI per language (WiX runs on Windows only) and a portable zip with the bare
//! exe. Rust, node and pnpm come from mise; MSVC must already be installed, it is only checked.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use super::env::Env;
use super::targets::{self, with_names};
use crate::exec::{Cmd, Ctx, which};
use crate::graph::{Outcome, run_cmd};

mod collect;

#[cfg(test)]
mod tests;

/// Whether `vswhere ... -property installationPath` found an installation with the C++ tools.
fn msvc_found(vswhere_stdout: &str) -> bool {
    !vswhere_stdout.trim().is_empty()
}

fn vswhere_path() -> Option<PathBuf> {
    let base = std::env::var_os("ProgramFiles(x86)")?;
    Some(Path::new(&base).join("Microsoft Visual Studio/Installer/vswhere.exe"))
}

/// A program by its installed name: on Windows tools such as pnpm are `.cmd` shims, which
/// `CreateProcess` does not find from a bare name.
fn tool(name: &str) -> String {
    which(name).map_or_else(|| name.to_string(), |path| path.display().to_string())
}

/// The environment of the build: pnpm store and cargo target inside the working copy, sccache
/// when it is on PATH. The channel's names and release profile come with `with_names`.
fn build_env(root: &Path, sccache: Option<&Path>) -> Vec<(String, String)> {
    let cache = root.join(".cache");
    let mut env = vec![
        ("CARGO_INCREMENTAL".to_string(), "0".to_string()),
        (
            "CARGO_TARGET_DIR".to_string(),
            root.join(".rust-target").display().to_string(),
        ),
        // The store goes in the Actions cache; pnpm reads npm_config_* settings itself.
        (
            "npm_config_store_dir".to_string(),
            cache.join("pnpm").display().to_string(),
        ),
    ];
    if let Some(path) = sccache {
        env.extend([
            ("RUSTC_WRAPPER".to_string(), path.display().to_string()),
            (
                "SCCACHE_DIR".to_string(),
                cache.join("sccache").display().to_string(),
            ),
            ("SCCACHE_CACHE_SIZE".to_string(), "3G".to_string()),
            // The server must outlive the long LTO tail of the last crate: asleep after 600 s
            // it would lose its statistics (and the stores of the crates still compiling).
            ("SCCACHE_IDLE_TIMEOUT".to_string(), "0".to_string()),
        ]);
    }
    env
}

fn with_env(mut cmd: Cmd, env: &[(String, String)]) -> Cmd {
    for (name, value) in env {
        cmd = cmd.env(name, value);
    }
    cmd
}

async fn check_msvc(ctx: &Ctx) -> Result<(), Outcome> {
    let found = match vswhere_path() {
        Some(vswhere) if vswhere.is_file() => ctx
            .capture(
                &vswhere.display().to_string(),
                &[
                    "-latest",
                    "-products",
                    "*",
                    "-requires",
                    "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                    "-property",
                    "installationPath",
                ],
            )
            .await
            .is_some_and(|out| msvc_found(&out)),
        _ => false,
    };
    if found {
        Ok(())
    } else {
        Err(Outcome::Fail(
            "MSVC build tools (Visual Studio C++ x64 tools) not found: install them, xtask does \
             not (the GitHub windows-latest image has them)"
                .into(),
        ))
    }
}

async fn step(ctx: &Ctx, cmd: &Cmd) -> Result<(), Outcome> {
    match run_cmd(ctx, cmd).await {
        Outcome::Ok => Ok(()),
        other => Err(other),
    }
}

pub async fn native(env: Arc<Env>, ctx: Ctx) -> Outcome {
    match build(&env, &ctx).await {
        Ok(()) => Outcome::Ok,
        Err(outcome) => outcome,
    }
}

async fn build(env: &Env, ctx: &Ctx) -> Result<(), Outcome> {
    if !cfg!(windows) {
        return Err(Outcome::Fail(
            "the native Windows build needs a Windows host (use a container build on Linux)".into(),
        ));
    }
    check_msvc(ctx).await?;
    let sccache = which("sccache");
    let vars = build_env(&env.root, sccache.as_deref());
    let pnpm = |args: &[&str]| {
        with_names(
            with_env(Cmd::new(&tool("pnpm")).args(args), &vars),
            env.channel,
        )
    };
    step(
        ctx,
        &pnpm(&["install", "--frozen-lockfile", "--prefer-offline"]),
    )
    .await?;
    let config = targets::overlay(env, "windows", false, false)?;
    let app_bin = main_binary(&config).map_err(|error| Outcome::Fail(format!("{error:#}")))?;
    let target = env.root.join(".rust-target");
    // Bundles of an earlier build (the other channel names them differently) would be
    // collected as well.
    let _ = std::fs::remove_dir_all(target.join("release/bundle"));
    let mut tauri = vec![
        "tauri",
        "build",
        "--ci",
        "--bundles",
        "nsis,msi",
        "--config",
        &config,
    ];
    // Updater artifacts (.sig) only with the signing key: tauri fails the build when
    // createUpdaterArtifacts is on and the key is missing.
    let signing = std::env::var("TAURI_SIGNING_PRIVATE_KEY").is_ok_and(|key| !key.is_empty());
    let sign_config = env
        .root
        .join(".cache/tauri-updater.json")
        .display()
        .to_string();
    if signing {
        std::fs::write(
            &sign_config,
            r#"{"bundle":{"createUpdaterArtifacts":true}}"#,
        )
        .map_err(|error| Outcome::Fail(format!("cannot write {sign_config}: {error}")))?;
        tauri.extend(["--config", &sign_config]);
    }
    step(ctx, &pnpm(&tauri)).await?;
    let out = env.root.join(&env.out);
    collect::run(&target.join("release"), &out, &env.version, &app_bin)
        .map_err(|error| Outcome::Fail(format!("{error:#}")))?;
    if let Some(path) = sccache {
        let _ = run_cmd(
            ctx,
            &with_env(
                Cmd::new(&path.display().to_string()).arg("--show-stats"),
                &vars,
            ),
        )
        .await;
    }
    Ok(())
}

/// `mainBinaryName` of the Tauri overlay: the channel's binary.
fn main_binary(overlay_path: &str) -> Result<String> {
    let text =
        std::fs::read_to_string(overlay_path).with_context(|| format!("read {overlay_path}"))?;
    binary_name(&text)
}

fn binary_name(overlay: &str) -> Result<String> {
    let json: serde_json::Value = serde_json::from_str(overlay)?;
    match json["mainBinaryName"].as_str() {
        Some(name) if !name.is_empty() => Ok(name.to_string()),
        _ => bail!("no mainBinaryName in the Tauri overlay"),
    }
}
