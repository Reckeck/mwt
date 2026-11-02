//! macOS: Tauri cannot build for it from another OS. On a Mac the script runs here, otherwise
//! on `MACOS_BUILD_HOST=user@mac` over ssh (rsync there and back), otherwise the task is skipped.

use std::sync::Arc;

use super::env::Env;
use super::targets::{OVERLAY_VAR, overlay, with_names};
use crate::exec::{Cmd, Ctx, ExecEnd};
use crate::graph::{Outcome, run_cmd};

/// What the machine offers; read once when the graph is built.
#[derive(Clone, Debug)]
pub struct MacFacts {
    pub is_mac: bool,
    /// `MACOS_BUILD_HOST`.
    pub host: Option<String>,
    /// `MACOS_BUILD_DIR`.
    pub dir: String,
}

impl MacFacts {
    pub fn detect() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        Self {
            is_mac: cfg!(target_os = "macos"),
            host: var("MACOS_BUILD_HOST"),
            dir: var("MACOS_BUILD_DIR").unwrap_or_else(|| "mwt-build".into()),
        }
    }

    /// Runs on this machine, where the frontend built once on the host is at hand.
    pub fn local(&self) -> bool {
        self.is_mac
    }
}

pub const SKIP_REASON: &str =
    "macOS builds on a Mac only; set MACOS_BUILD_HOST=user@mac to build there over ssh";

pub async fn build(env: Arc<Env>, facts: MacFacts, ctx: Ctx) -> Outcome {
    if facts.is_mac {
        let config = match overlay(&env, "macos", false, false) {
            Ok(path) => path,
            Err(outcome) => return outcome,
        };
        let cmd = with_names(
            Cmd::new("scripts/ci/build-macos.sh").args([&env.version, &env.out]),
            env.channel,
        )
        .env(OVERLAY_VAR, &config);
        return run_cmd(&ctx, &cmd).await;
    }
    let Some(host) = facts.host else {
        ctx.note(&format!("skipped: {SKIP_REASON}"));
        return Outcome::Skip(SKIP_REASON.into());
    };
    let remote = facts.dir;
    // dist is not copied: the Mac builds its own frontend (beforeBuildCommand).
    let mut rsync = Cmd::new("rsync").args(["-a", "--delete"]);
    for exclude in crate::platforms::MAC_EXCLUDES {
        rsync = rsync.args([format!("--exclude={exclude}")]);
    }
    let rsync = rsync.args(["./", &format!("{host}:{remote}/")]);
    // The Mac builds its own frontend and writes its own overlay (build-macos.sh), from the
    // channel's names.
    let names: String = env
        .channel
        .env()
        .iter()
        .chain(env.channel.build_env_here().iter())
        .map(|(name, value)| format!("{name}='{value}' "))
        .collect();
    let script = format!(
        "cd '{remote}' && {names}bash -lc \"scripts/ci/build-macos.sh '{}' release\"",
        env.version
    );
    let ssh = Cmd::new("ssh").args([host.as_str(), script.as_str()]);
    let fetch = Cmd::new("rsync")
        .args([
            "-a",
            &format!("{host}:{remote}/release/"),
            &format!("{}/", env.out),
        ])
        .args(["--include=*macos*", "--exclude=*"]);
    for (cmd, what) in [(rsync, "rsync"), (ssh, "ssh"), (fetch, "rsync back")] {
        match ctx.exec(&cmd).await {
            Ok(ExecEnd::Done(0)) => {}
            Ok(ExecEnd::Done(code)) => return Outcome::Fail(format!("{what} exited with {code}")),
            Ok(ExecEnd::Cancelled) => return Outcome::Cancelled,
            Err(error) => return Outcome::Fail(format!("{error:#}")),
        }
    }
    Outcome::Ok
}
