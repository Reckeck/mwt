//! `cargo xtask build`: bundles for every platform, with the scripts and images GitLab CI uses
//! (`scripts/ci/build-*.sh`). The frontend is built once on the host; the platforms that do not
//! depend on each other run side by side.

mod env;
mod macos;
mod targets;

use std::sync::Arc;

use anyhow::{Result, bail};

pub use env::Env;
pub use macos::MacFacts;

use crate::exec::Cmd;
use crate::graph::{Class, Outcome, Task, run_cmd};
use crate::sink::Sink;

/// Declaration order is the start order: the long builds first.
pub const PLATFORMS: [&str; 6] = ["android", "windows", "linux", "macos", "pacman", "flatpak"];

pub fn task_name(platform: &str) -> String {
    format!("build:{platform}")
}

/// What the platform choice depends on, apart from the choice itself.
#[derive(Clone, Debug)]
pub struct Facts {
    pub mac: MacFacts,
    /// The Linux tarball of this channel and version is already in the output folder.
    pub tarball: bool,
    /// `dist/` was built before: no `build:frontend` task, nothing waits for it.
    pub prebuilt_frontend: bool,
}

/// The version ends up inside shell quotes of the container scripts.
pub fn check_version(version: &str) -> Result<()> {
    let ok = !version.is_empty()
        && version
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '+'));
    if !ok {
        bail!("--version `{version}`: letters, digits, `.`, `-` and `+` only");
    }
    Ok(())
}

/// The tasks for the chosen platforms (all when empty) plus what they need:
/// - `frontend` once, for the builds that tauri would otherwise each start `pnpm build` for;
/// - pacman and flatpak start from the Linux tarball: after `linux` when it is chosen too, from the
///   tarball already in the output folder otherwise, and without either it is an error before
///   anything is built .
pub fn graph(env: &Arc<Env>, chosen: &[String], facts: &Facts) -> Result<Vec<Task>> {
    for name in chosen {
        if !PLATFORMS.contains(&name.as_str()) {
            bail!(
                "unknown platform `{name}` (one of: {})",
                PLATFORMS.join(", ")
            );
        }
    }
    let wanted = |name: &str| chosen.is_empty() || chosen.iter().any(|item| item == name);
    let frontend = task_name("frontend");
    let linux = task_name("linux");
    let after_frontend = if facts.prebuilt_frontend {
        Vec::new()
    } else {
        vec![frontend.as_str()]
    };
    let mut tasks = Vec::new();
    for platform in PLATFORMS.into_iter().filter(|name| wanted(name)) {
        let name = task_name(platform);
        let shared = env.clone();
        let task = match platform {
            "android" => Task::func(&name, Class::Heavy, &after_frontend, move |ctx| {
                targets::android(shared.clone(), ctx)
            }),
            "windows" => Task::func(&name, Class::Heavy, &after_frontend, move |ctx| {
                targets::windows(shared.clone(), ctx)
            }),
            "linux" => Task::func(&name, Class::Heavy, &after_frontend, move |ctx| {
                targets::linux(shared.clone(), ctx)
            }),
            "macos" => {
                let mac = facts.mac.clone();
                let deps = if mac.local() {
                    after_frontend.clone()
                } else {
                    Vec::new()
                };
                Task::func(&name, Class::Light, &deps, move |ctx| {
                    macos::build(shared.clone(), mac.clone(), ctx)
                })
            }
            _ => {
                let deps = if wanted("linux") {
                    vec![linux.as_str()]
                } else if facts.tarball {
                    Vec::new()
                } else {
                    bail!(
                        "no {}: `{platform}` starts from the Linux tarball, build it first \
                         (cargo xtask build linux {platform})",
                        env.tarball().display()
                    );
                };
                if platform == "pacman" {
                    Task::func(&name, Class::Light, &deps, move |ctx| {
                        targets::pacman(shared.clone(), ctx)
                    })
                } else {
                    Task::func(&name, Class::Light, &deps, move |ctx| {
                        targets::flatpak(shared.clone(), ctx)
                    })
                }
            }
        };
        tasks.push(task);
    }
    if !facts.prebuilt_frontend && tasks.iter().any(|task| task.deps.contains(&frontend)) {
        // The first to start: everything else waits for it.
        tasks.insert(0, frontend_task(&frontend));
    }
    Ok(tasks)
}

/// `pnpm build` (vue-tsc + vite) into `dist/`; the dependencies are installed first when the
/// working copy has none (the containers install their own).
fn frontend_task(name: &str) -> Task {
    Task::func(name, Class::Light, &[], |ctx| async move {
        if !ctx.root.join("node_modules/.bin/vue-tsc").exists() {
            let install =
                Cmd::new("pnpm").args(["install", "--frozen-lockfile", "--prefer-offline"]);
            let outcome = run_cmd(&ctx, &install).await;
            if !matches!(outcome, Outcome::Ok) {
                return outcome;
            }
        }
        run_cmd(&ctx, &Cmd::new("pnpm").args(["build"])).await
    })
}

/// The bundles that landed in the output folder, for the end of the run.
pub fn print_output(env: &Env, sink: &Sink) {
    sink.line(&format!(
        "Multi Wallet Tracker {} {} -> {}/",
        env.channel.label(),
        env.version,
        env.out
    ));
    let mut files: Vec<(String, u64)> = std::fs::read_dir(env.root.join(&env.out))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let size = entry.metadata().ok()?.len();
            let name = entry.file_name().to_string_lossy().into_owned();
            // Files of other versions are left from earlier builds.
            name.contains(&env.version).then_some((name, size))
        })
        .collect();
    files.sort();
    for (name, size) in files {
        sink.raw(&format!("    {name}  ({} MB)", size / 1_000_000));
    }
}

#[cfg(test)]
mod tests;
