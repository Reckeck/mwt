//! Does the app still compile for every platform it ships on? Clippy, no
//! bundles. Linux is the workspace `clippy` task of the check graph.

use std::path::Path;

use crate::checks::lane;
use crate::exec::{Cmd, Ctx, ExecEnd, which};
use crate::graph::{Class, Outcome, Task};

const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";
const ANDROID_TARGET: &str = "aarch64-linux-android";
const LINT: [&str; 3] = ["--", "-D", "warnings"];

pub const PLATFORMS: [&str; 4] = ["linux", "windows", "android", "macos"];

/// Task name for a platform name.
pub fn task_name(platform: &str) -> Option<String> {
    match platform {
        "linux" => Some("clippy".to_string()),
        "windows" | "android" | "macos" => Some(format!("platform:{platform}")),
        _ => None,
    }
}

pub fn tasks(_root: &Path) -> Vec<Task> {
    vec![
        Task::func("platform:android", Class::Heavy, &[], |ctx| async move {
            android(&ctx).await
        }),
        Task::func("platform:windows", Class::Heavy, &[], |ctx| async move {
            windows(&ctx).await
        }),
        Task::func("platform:macos", Class::Heavy, &[], |ctx| async move {
            macos(&ctx).await
        }),
    ]
}

fn skip(ctx: &Ctx, reason: &str) -> Outcome {
    ctx.note(&format!("skipped: {reason}"));
    Outcome::Skip(reason.to_string())
}

async fn finish(ctx: &Ctx, cmd: Cmd) -> Outcome {
    crate::graph::run_cmd(ctx, &cmd).await
}

async fn rust_target_installed(ctx: &Ctx, target: &str) -> bool {
    ctx.capture("rustup", &["target", "list", "--installed"])
        .await
        .is_some_and(|out| out.lines().any(|line| line.trim() == target))
}

async fn windows(ctx: &Ctx) -> Outcome {
    if which("cargo-xwin").is_none() {
        return skip(ctx, "no cargo-xwin (cargo install --locked cargo-xwin)");
    }
    if !rust_target_installed(ctx, WINDOWS_TARGET).await {
        return skip(
            ctx,
            &format!("no Rust target (rustup target add {WINDOWS_TARGET})"),
        );
    }
    let cmd = Cmd::new("cargo")
        .args([
            "xwin",
            "clippy",
            "-p",
            "trade-app",
            "--lib",
            "--target",
            WINDOWS_TARGET,
            "--target-dir",
        ])
        .arg(lane(&ctx.root, "xwin"))
        .args(LINT);
    finish(ctx, cmd).await
}

/// The NDK's clang for C dependencies built by build scripts.
fn ndk_env(ndk: &str) -> Option<Vec<(&'static str, String)>> {
    let bin = Path::new(ndk).join("toolchains/llvm/prebuilt/linux-x86_64/bin");
    let clang = bin.join("aarch64-linux-android24-clang");
    clang.is_file().then(|| {
        let clang = clang.display().to_string();
        vec![
            ("CC_aarch64_linux_android", clang.clone()),
            (
                "AR_aarch64_linux_android",
                bin.join("llvm-ar").display().to_string(),
            ),
            ("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", clang),
        ]
    })
}

async fn android(ctx: &Ctx) -> Outcome {
    let ndk = std::env::var("ANDROID_NDK_HOME")
        .or_else(|_| std::env::var("NDK_HOME"))
        .unwrap_or_default();
    if !ndk.is_empty()
        && rust_target_installed(ctx, ANDROID_TARGET).await
        && let Some(env) = ndk_env(&ndk)
    {
        let mut cmd = Cmd::new("cargo")
            .args([
                "clippy",
                "-p",
                "trade-app",
                "--lib",
                "--target",
                ANDROID_TARGET,
                "--target-dir",
            ])
            .arg(lane(&ctx.root, "android"))
            .args(LINT);
        cmd.env
            .extend(env.into_iter().map(|(key, value)| (key.to_string(), value)));
        return finish(ctx, cmd).await;
    }
    android_in_container(ctx).await
}

pub fn image_tag(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(".gitlab-ci.yml")).ok()?;
    text.lines().find_map(|line| {
        let rest = line.strip_prefix("  CI_IMAGE_TAG: \"")?;
        Some(rest.split('"').next()?.to_string())
    })
}

async fn android_in_container(ctx: &Ctx) -> Outcome {
    let engine = std::env::var("CONTAINER_ENGINE")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            which("podman")
                .or_else(|| which("docker"))
                .map(|path| path.display().to_string())
        });
    let Some(engine) = engine else {
        return skip(
            ctx,
            "no Android NDK on the host (ANDROID_NDK_HOME) and no podman/docker for the ci-android image",
        );
    };
    let Some(tag) = image_tag(&ctx.root) else {
        return Outcome::Fail("no CI_IMAGE_TAG in .gitlab-ci.yml".into());
    };
    let registry = std::env::var("CI_REGISTRY_IMAGE")
        .unwrap_or_else(|_| "registry.gitlab.com/reckeck/trade-app".into());
    let image = format!("{registry}/ci-android:{tag}");
    match ctx
        .succeeds(&Cmd::new(&engine).args(["image", "inspect", &image]))
        .await
    {
        Ok(Some(true)) => {}
        Ok(Some(false)) => match ctx
            .succeeds(&Cmd::new(&engine).args(["pull", "-q", &image]))
            .await
        {
            Ok(Some(true)) => {}
            Ok(Some(false)) => {
                return skip(
                    ctx,
                    &format!(
                        "no ci-android image {image} (mise run build:all -- android builds it)"
                    ),
                );
            }
            Ok(None) => return Outcome::Cancelled,
            Err(error) => return Outcome::Fail(format!("{error:#}")),
        },
        Ok(None) => return Outcome::Cancelled,
        Err(error) => return Outcome::Fail(format!("{error:#}")),
    }
    // Same volumes as `cargo xtask build`, so the compiled crates are reused.
    let checkout = checkout_hash(&ctx.root);
    let network = std::env::var("CONTAINER_NETWORK").unwrap_or_else(|_| "host".into());
    let cmd = Cmd::new(&engine)
        .args(["run", "--rm", "--init", &format!("--network={network}")])
        .args(["-v", &format!("{}:/src", ctx.root.display()), "-w", "/src"])
        .args([
            "-v",
            &format!("mwt-build-{checkout}-android-target:/src/.rust-target"),
        ])
        .args(["-v", &format!("mwt-build-{checkout}-android-home:/root")])
        // setup-linux.sh runs `pnpm install`: into its own volume, or it relinks the host's
        // node_modules to the container store while vitest is running next to it.
        .args([
            "-v",
            &format!("mwt-build-{checkout}-android-node-modules:/src/node_modules"),
        ])
        .args([
            "-e",
            "CI_PROJECT_DIR=/src",
            "-e",
            "CI=true",
            "-e",
            "CI_SKIP_GUI_LIBS=1",
        ])
        .args(crate::sccache::container_mount())
        .args([image.as_str(), "bash", "-c", IN_CONTAINER]);
    finish(ctx, cmd).await
}

const IN_CONTAINER: &str = r#"set -eo pipefail
source scripts/ci/setup-linux.sh; source scripts/ci/setup-android.sh
ndk="${ANDROID_NDK_HOME:-$NDK_HOME}"; bin="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang" AR_aarch64_linux_android="$bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
cargo clippy -p trade-app --lib --target aarch64-linux-android -- -D warnings"#;

/// First 8 hex digits of the sha256 of the root path, as `cargo xtask build` names its volumes.
pub fn checkout_hash(root: &Path) -> String {
    use std::process::{Command, Stdio};
    let mut child = match Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return "local".into(),
    };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(root.display().to_string().as_bytes());
    }
    let out = child
        .wait_with_output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    out.chars().take(8).collect()
}

pub const MAC_EXCLUDES: [&str; 11] = [
    ".git",
    ".jj",
    ".rust-target",
    "node_modules",
    ".cache",
    "release",
    "dist",
    "crates/app/gen",
    ".flatpak-build",
    ".flatpak-repo",
    ".flatpak-builder",
];

async fn macos(ctx: &Ctx) -> Outcome {
    if cfg!(target_os = "macos") {
        let cmd = Cmd::new("cargo")
            .args(["clippy", "-p", "trade-app", "--lib", "--target-dir"])
            .arg(lane(&ctx.root, "macos"))
            .args(LINT);
        return finish(ctx, cmd).await;
    }
    let Some(host) = std::env::var("MACOS_BUILD_HOST")
        .ok()
        .filter(|value| !value.is_empty())
    else {
        return skip(
            ctx,
            "macOS compiles on a Mac only; set MACOS_BUILD_HOST=user@mac to check there over ssh",
        );
    };
    let remote = std::env::var("MACOS_BUILD_DIR").unwrap_or_else(|_| "mwt-build".into());
    let mut rsync = Cmd::new("rsync").args(["-a", "--delete"]);
    for exclude in MAC_EXCLUDES {
        rsync = rsync.args([format!("--exclude={exclude}")]);
    }
    rsync = rsync.args(["./", &format!("{host}:{remote}/")]);
    match ctx.exec(&rsync).await {
        Ok(ExecEnd::Done(0)) => {}
        Ok(ExecEnd::Done(code)) => return Outcome::Fail(format!("rsync exited with {code}")),
        Ok(ExecEnd::Cancelled) => return Outcome::Cancelled,
        Err(error) => return Outcome::Fail(format!("{error:#}")),
    }
    let script =
        format!("cd '{remote}' && bash -lc 'cargo clippy -p trade-app --lib -- -D warnings'");
    finish(ctx, Cmd::new("ssh").args([host.as_str(), script.as_str()])).await
}
