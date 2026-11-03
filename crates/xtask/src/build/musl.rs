//! The static (musl) xtask the build containers run: they are Ubuntu 22.04 and archlinux, a
//! glibc binary built on the host does not run in both. The same file is mounted read-only at
//! `/usr/local/bin/xtask` (`Env::run_ci`, `Env::run_arch`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::env::Env;
use crate::exec::{Cmd, Ctx};
use crate::graph::{Outcome, run_cmd};

pub const TARGET: &str = "x86_64-unknown-linux-musl";
/// Relative to the repository root.
const TARGET_DIR: &str = ".cache/xtask-musl";
/// Where the binary is mounted in a container.
pub const MOUNT: &str = "/usr/local/bin/xtask";

pub fn task_name() -> String {
    "build:xtask".into()
}

/// Where `cargo build -p xtask --target <musl>` puts the binary.
fn built_path(root: &Path) -> PathBuf {
    root.join(TARGET_DIR).join(TARGET).join("debug/xtask")
}

/// Whether `rustup target list --installed` (one target per line) has the target.
fn installed(list: &str, target: &str) -> bool {
    list.lines().any(|line| line.trim() == target)
}

/// The `-v` argument that mounts the binary into a container.
pub fn mount_args(binary: &Path) -> Vec<String> {
    vec!["-v".into(), format!("{}:{MOUNT}:ro", binary.display())]
}

/// The `build:xtask` task: finds or builds the musl xtask and tells `Env` where it is.
pub async fn prepare(env: Arc<Env>, ctx: Ctx) -> Outcome {
    // A musl xtask (CI downloads one) is the binary to mount as it is.
    let binary = if cfg!(target_env = "musl") {
        match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => return Outcome::Fail(format!("cannot find the running xtask: {error}")),
        }
    } else {
        match build(&ctx).await {
            Ok(path) => path,
            Err(outcome) => return outcome,
        }
    };
    ctx.note(&format!("static xtask: {}", binary.display()));
    let _ = env.set_musl_xtask(binary);
    Outcome::Ok
}

async fn build(ctx: &Ctx) -> Result<PathBuf, Outcome> {
    let listed = ctx
        .capture("rustup", &["target", "list", "--installed"])
        .await
        .unwrap_or_default();
    if !installed(&listed, TARGET) {
        let add = Cmd::new("rustup").args(["target", "add", TARGET]);
        match run_cmd(ctx, &add).await {
            Outcome::Ok => {}
            other => return Err(other),
        }
    }
    let cargo = Cmd::new("cargo").args([
        "build",
        "-p",
        "xtask",
        "--locked",
        "--target",
        TARGET,
        "--target-dir",
        TARGET_DIR,
    ]);
    match run_cmd(ctx, &cargo).await {
        Outcome::Ok => Ok(built_path(&ctx.root)),
        other => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_targets_are_matched_by_line() {
        let list = "x86_64-unknown-linux-gnu\nx86_64-unknown-linux-musl\n";
        assert!(installed(list, TARGET));
        assert!(!installed("x86_64-unknown-linux-gnu\n", TARGET));
        assert!(!installed("", TARGET));
    }

    #[test]
    fn binary_is_mounted_read_only() {
        let args = mount_args(Path::new("/r/.cache/xtask-musl/t/debug/xtask"));
        assert_eq!(args[0], "-v");
        assert_eq!(
            args[1],
            "/r/.cache/xtask-musl/t/debug/xtask:/usr/local/bin/xtask:ro"
        );
    }

    #[test]
    fn built_binary_is_under_the_target_triple() {
        assert_eq!(
            built_path(Path::new("/r")),
            Path::new("/r/.cache/xtask-musl/x86_64-unknown-linux-musl/debug/xtask")
        );
    }
}
