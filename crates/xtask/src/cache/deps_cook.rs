//! `cargo xtask deps-cook <lane>`: compile only the dependencies of a lane, so sccache holds them
//! (the job `deps`; replaces deps-cook.sh). cargo-chef rebuilds a skeleton of the workspace (our
//! crates are empty stubs, manifests and Cargo.lock are real), so the feature set of every
//! dependency equals the real build's. It runs in a scratch copy: `cook` writes its stubs into the
//! tree it runs in.
//!
//! Each lane cooks what that lane's job compiles:
//!   check-linux     cargo xtask check --exclude @frontend --exclude @platforms: cargo test/run
//!                   (dev build) and clippy, whole workspace
//!   clippy-android  xtask (dev build, host) + clippy -p trade-app for aarch64-linux-android
//!   clippy-macos    xtask (dev build, host) + clippy -p trade-app
//!   clippy-windows  clippy -p trade-app (no xtask there, see ci.yml)
//!
//! The text of THIS file is part of every lane key (lane_key.rs), so editing a recipe renews caches.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::{Scratch, run_checked};
use crate::output::{Mode, Output, OutputArgs};

/// One `cargo chef cook` call.
#[derive(Debug, PartialEq, Eq)]
pub struct Cook {
    pub target_dir: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

fn cook(target_dir: String, args: &[&str], env: &[(String, String)]) -> Cook {
    Cook {
        target_dir,
        args: args.iter().map(ToString::to_string).collect(),
        env: env.to_vec(),
    }
}

/// The NDK's clang for C dependencies built by build scripts (as xtask does).
pub fn ndk_env(ndk: &str) -> Vec<(String, String)> {
    let bin = format!("{ndk}/toolchains/llvm/prebuilt/linux-x86_64/bin");
    let clang = format!("{bin}/aarch64-linux-android24-clang");
    vec![
        ("CC_aarch64_linux_android".to_string(), clang.clone()),
        (
            "AR_aarch64_linux_android".to_string(),
            format!("{bin}/llvm-ar"),
        ),
        (
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER".to_string(),
            clang,
        ),
    ]
}

/// The cook calls of a lane. `target` is the real job's target dir, to the byte: crates with a
/// build script have OUT_DIR, and with it this path, in their sccache key.
pub fn plan(lane: &str, target: &str, ndk: Option<&str>) -> Result<Vec<Cook>> {
    let sub = |name: &str| {
        format!(
            "{target}{}lanes{}{name}",
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        )
    };
    Ok(match lane {
        "check-linux" => vec![
            cook(target.to_string(), &["--workspace", "--all-targets"], &[]),
            cook(
                sub("clippy"),
                &["--clippy", "--workspace", "--all-targets"],
                &[],
            ),
        ],
        "clippy-android" => {
            let ndk = ndk.context("no Android NDK (ANDROID_NDK_HOME or NDK_HOME)")?;
            let env = ndk_env(ndk);
            vec![
                cook(target.to_string(), &["-p", "xtask"], &env),
                cook(
                    sub("android"),
                    &[
                        "--clippy",
                        "-p",
                        "trade-app",
                        "--target",
                        "aarch64-linux-android",
                    ],
                    &env,
                ),
            ]
        }
        "clippy-macos" => vec![
            cook(target.to_string(), &["-p", "xtask"], &[]),
            cook(sub("macos"), &["--clippy", "-p", "trade-app"], &[]),
        ],
        "clippy-windows" => vec![cook(
            target.to_string(),
            &["--clippy", "-p", "trade-app"],
            &[],
        )],
        _ => bail!("unknown lane: {lane}"),
    })
}

fn grouped() -> bool {
    Output::from_args(&OutputArgs::default()).mode == Mode::Github
}

fn extract_head(root: &Path, into: &Path) -> Result<()> {
    let mut archive = Command::new("git")
        .args(["archive", "HEAD"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .spawn()
        .context("cannot start git archive")?;
    let pipe = archive.stdout.take().context("git archive has no stdout")?;
    run_checked(
        Command::new("tar")
            .arg("-x")
            .arg("-C")
            .arg(into)
            .stdin(pipe),
    )?;
    let status = archive.wait()?;
    if !status.success() {
        bail!("git archive HEAD failed: {status}");
    }
    Ok(())
}

/// cargo-chef is installed when `cargo chef` does not answer; the lane's sccache statistics end the run.
pub fn run(root: &Path, lane: &str) -> Result<()> {
    let chef_present = Command::new("cargo")
        .args(["chef", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !chef_present {
        super::install_chef::run()?;
    }
    cook_all(root, lane)?;
    run_checked(Command::new("sccache").arg("--show-stats"))
}

fn cook_all(root: &Path, lane: &str) -> Result<()> {
    let target = root.join(".rust-target").display().to_string();
    let ndk = std::env::var("ANDROID_NDK_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("NDK_HOME")
                .ok()
                .filter(|value| !value.is_empty())
        });
    let cooks = plan(lane, &target, ndk.as_deref())?;
    let work = Scratch::new("deps-cook")?;
    // In a job container the checkout belongs to another user.
    run_checked(
        Command::new("git")
            .args(["config", "--global", "--add", "safe.directory"])
            .arg(root),
    )?;
    extract_head(root, work.path())?;
    // A flag, not CARGO_TARGET_DIR: sccache hashes CARGO_* variables, so the variable would make
    // every key differ from the real build. chef needs the flag: it does not read .cargo/config.toml.
    run_checked(
        Command::new("cargo")
            .args(["chef", "prepare", "--recipe-path", "recipe.json"])
            .current_dir(work.path())
            .env_remove("CARGO_TARGET_DIR"),
    )?;
    let group = grouped();
    for item in cooks {
        let shown = item.args.join(" ");
        if group {
            println!("::group::cargo chef cook {shown}");
        }
        let result = run_checked(
            Command::new("cargo")
                .args([
                    "chef",
                    "cook",
                    "--recipe-path",
                    "recipe.json",
                    "--target-dir",
                ])
                .arg(&item.target_dir)
                .args(&item.args)
                .envs(item.env.iter().map(|(key, value)| (key, value)))
                .current_dir(work.path())
                .env_remove("CARGO_TARGET_DIR"),
        );
        if group {
            println!("::endgroup::");
        }
        result?;
    }
    Ok(())
}
