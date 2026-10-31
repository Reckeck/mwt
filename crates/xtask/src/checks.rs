//! The `check` graph: the same set of checks the old `mise run check` ran,
//! plus `cargo fmt --check` and the platform builds.

use std::path::{Path, PathBuf};

use crate::exec::{Cmd, Ctx, ExecEnd};
use crate::graph::{Class, Outcome, Task};
use crate::platforms;

const LINT: [&str; 3] = ["--", "-D", "warnings"];
const GEN_TYPES_FILE: &str = "src/shared/api/ipc.gen.d.ts";

/// `$CARGO_TARGET_DIR` (relative to the root) or `.rust-target`.
pub fn target_base(root: &Path) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| PathBuf::from(".rust-target"), PathBuf::from);
    if base.is_absolute() {
        base
    } else {
        root.join(base)
    }
}

/// A separate target directory for one cargo task: parallel cargo runs in one directory block each other.
/// Goes to cargo as `--target-dir`: in `CARGO_TARGET_DIR` it would become part of every sccache key.
pub fn lane(root: &Path, name: &str) -> String {
    target_base(root)
        .join("lanes")
        .join(name)
        .display()
        .to_string()
}

fn vitest(name: &str, class: Class, project: &str) -> Task {
    let cmd = Cmd::new("pnpm").args(["exec", "vitest", "run", "--project", project]);
    Task::cmd(name, class, &[], cmd)
}

/// Timing-sensitive checks (long tasks, layout shift, overlays opening) fail on a loaded machine,
/// so by default they run alone: after every other task, one browser at a time
/// (`--overlap-browser` starts them at once; they are mostly waiting, so it saves minutes but can flake).
fn browser(task: Task, overlap: bool) -> Task {
    if overlap {
        task
    } else {
        task.after(&QUIET_AFTER)
    }
}

const QUIET_AFTER: [&str; 10] = [
    "biome",
    "vue-tsc",
    "unit",
    "checks",
    "test",
    "gen-types",
    "clippy",
    "platform:android",
    "platform:windows",
    "platform:macos",
];

/// How layout and links run: `--quick` unless `full`, one share of the units (`--shard K/N`, the CI matrix),
/// processes per share (`--jobs`; the script picks by cores when empty).
#[derive(Clone, Default, Debug)]
pub struct BrowserOpts {
    pub full: bool,
    pub shard: Option<String>,
    pub jobs: Option<usize>,
}

/// `scripts/run-browser-check.mjs <name> [--quick] [--shard K/N] [--jobs N]`: the vitest project `layout` on one file.
fn browser_check(name: &str, opts: &BrowserOpts) -> Task {
    Task::cmd(name, Class::Browser, &[], browser_cmd(name, opts))
}

fn browser_cmd(name: &str, opts: &BrowserOpts) -> Cmd {
    let mut cmd = Cmd::new("node").args(["scripts/run-browser-check.mjs", name]);
    if !opts.full {
        cmd = cmd.arg("--quick");
    }
    if let Some(shard) = &opts.shard {
        cmd = cmd.args(["--shard", shard]);
    }
    if let Some(jobs) = opts.jobs {
        cmd = cmd.args(["--jobs", &jobs.to_string()]);
    }
    cmd
}

/// `K/N` with 1 <= K <= N (clap's value parser of `--shard`).
pub fn parse_shard(text: &str) -> Result<String, String> {
    let bad = || format!("`{text}` is not K/N with 1 <= K <= N");
    let (index, total) = text.split_once('/').ok_or_else(bad)?;
    let index: u32 = index.parse().map_err(|_| bad())?;
    let total: u32 = total.parse().map_err(|_| bad())?;
    if (1..=total).contains(&index) {
        Ok(text.to_string())
    } else {
        Err(bad())
    }
}

fn clippy_workspace(root: &Path) -> Task {
    let cmd = Cmd::new("cargo")
        .args(["clippy", "--workspace", "--all-targets", "--target-dir"])
        .arg(lane(root, "clippy"))
        .args(LINT);
    Task::cmd("clippy", Class::Heavy, &[], cmd)
}

/// Declaration order is the start order: the long tasks first.
/// The checks of the web part: need `node_modules` (CI runs them in a job of their own).
pub const FRONTEND: [&str; 5] = ["biome", "vue-tsc", "unit", "checks", "e2e"];

/// The browser sweeps: CI cuts them into shards (`--shard K/N`, job `frontend-layout`), so neither
/// `--frontend` nor `--no-frontend` runs them there.
pub const SHARDED: [&str; 2] = ["layout", "links"];

pub fn check_tasks(root: &Path, overlap_browser: bool, browser_opts: &BrowserOpts) -> Vec<Task> {
    let mut tasks = vec![
        Task::cmd(
            "test",
            Class::Heavy,
            &[],
            Cmd::new("cargo").args(["test", "--workspace"]),
        ),
        gen_types(root),
    ];
    tasks.extend(platforms::tasks(root));
    tasks.push(clippy_workspace(root));
    tasks.extend([
        browser(vitest("e2e", Class::Browser, "e2e"), overlap_browser),
        browser(browser_check("layout", browser_opts), overlap_browser),
        browser(browser_check("links", browser_opts), overlap_browser),
        Task::cmd(
            "biome",
            Class::Light,
            &[],
            Cmd::new("pnpm").args(["exec", "biome", "check", "."]),
        ),
        Task::cmd(
            "vue-tsc",
            Class::Light,
            &[],
            Cmd::new("pnpm").args(["exec", "vue-tsc", "--noEmit"]),
        ),
        vitest("unit", Class::Light, "unit"),
        // onboarding, routes, forms, ui, fsd (tests/checks)
        vitest("checks", Class::Light, "checks"),
        Task::cmd(
            "ci-scripts",
            Class::Light,
            &[],
            Cmd::new("scripts/ci/lint-ci.sh"),
        ),
        Task::cmd(
            "fmt",
            Class::Light,
            &[],
            Cmd::new("cargo").args(["fmt", "--all", "--check"]),
        ),
    ]);
    tasks
}

/// Regenerates ipc.gen.d.ts and fails when it differs from the committed file; the file is put back.
fn gen_types(_root: &Path) -> Task {
    Task::func(
        "gen-types",
        Class::Heavy,
        &["test"],
        |ctx: Ctx| async move {
            let path = ctx.root.join(GEN_TYPES_FILE);
            let before = std::fs::read(&path).ok();
            let cmd = Cmd::new("cargo").args(["run", "-p", "trade-core", "--bin", "gen-types"]);
            let end = ctx.exec(&cmd).await;
            // A killed generator leaves its scratch directory behind.
            let _ = std::fs::remove_dir_all(ctx.root.join("src/shared/api/.ts-tmp"));
            let after = std::fs::read(&path).ok();
            if after != before {
                match &before {
                    Some(bytes) => {
                        let _ = std::fs::write(&path, bytes);
                    }
                    None => {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
            match end {
                Ok(ExecEnd::Done(0)) if after == before => Outcome::Ok,
                Ok(ExecEnd::Done(0)) => {
                    Outcome::Fail("ipc.gen.d.ts is out of date: run `mise run gen-types`".into())
                }
                Ok(ExecEnd::Done(code)) => Outcome::Fail(format!("gen-types exited with {code}")),
                Ok(ExecEnd::Cancelled) => Outcome::Cancelled,
                Err(error) => Outcome::Fail(format!("{error:#}")),
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{BrowserOpts, browser_cmd, parse_shard};

    fn opts(full: bool, shard: Option<&str>, jobs: Option<usize>) -> BrowserOpts {
        BrowserOpts {
            full,
            shard: shard.map(String::from),
            jobs,
        }
    }

    #[test]
    fn browser_checks_are_quick_unless_full() {
        assert_eq!(
            browser_cmd("layout", &opts(false, None, None)).display(),
            "node scripts/run-browser-check.mjs layout --quick"
        );
        assert_eq!(
            browser_cmd("links", &opts(true, None, None)).display(),
            "node scripts/run-browser-check.mjs links"
        );
    }

    #[test]
    fn browser_checks_pass_the_shard_and_jobs_on() {
        assert_eq!(
            browser_cmd("layout", &opts(true, Some("2/6"), Some(1))).display(),
            "node scripts/run-browser-check.mjs layout --shard 2/6 --jobs 1"
        );
    }

    #[test]
    fn shard_is_k_of_n() {
        assert_eq!(parse_shard("3/6"), Ok("3/6".to_string()));
        for bad in ["0/6", "7/6", "3", "a/b", "1/0", "/"] {
            assert!(parse_shard(bad).is_err(), "{bad}");
        }
    }
}
