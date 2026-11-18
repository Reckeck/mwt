//! The `check` graph: the same set of checks the old `mise run check` ran,
//! plus `cargo fmt --check` and the platform builds.

pub mod groups;

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

/// The module path of the slow peer-sync convergence tests: `test-slow` runs only them, `test` skips them.
pub const SLOW_FILTER: &str = "service::peer_sync::convergence";

const QUIET_AFTER: [&str; 11] = [
    "biome",
    "vue-tsc",
    "unit",
    "checks",
    "test",
    "test-slow",
    "gen-types",
    "clippy",
    "platform:android",
    "platform:windows",
    "platform:macos",
];

/// How layout and links run: in full unless `quick`, one share of the units (`--shard K/N`, the CI matrix),
/// processes per share (`--jobs`; the script picks by cores when empty).
#[derive(Clone, Default, Debug)]
pub struct BrowserOpts {
    pub quick: bool,
    pub shard: Option<String>,
    pub jobs: Option<usize>,
}

/// `scripts/run-browser-check.mjs <name> [--quick] [--shard K/N] [--jobs N]`: the vitest project `layout` on one file.
fn browser_check(name: &str, opts: &BrowserOpts) -> Task {
    Task::cmd(name, Class::Browser, &[], browser_cmd(name, opts))
}

fn browser_cmd(name: &str, opts: &BrowserOpts) -> Cmd {
    let mut cmd = Cmd::new("node").args(["scripts/run-browser-check.mjs", name]);
    if opts.quick {
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

/// Declaration order is the start order: the long tasks first. Groups of the tasks: `groups.rs`.
pub fn check_tasks(root: &Path, overlap_browser: bool, browser_opts: &BrowserOpts) -> Vec<Task> {
    let mut tasks = vec![
        Task::cmd(
            "test-slow",
            Class::Heavy,
            &[],
            Cmd::new("cargo").args(["test", "-p", "trade-core", "--lib", "--", SLOW_FILTER]),
        ),
        Task::cmd(
            "test",
            Class::Heavy,
            &[],
            Cmd::new("cargo").args(["test", "--workspace", "--", "--skip", SLOW_FILTER]),
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
        ci_scripts(),
        arch(),
        strangle(),
        Task::cmd(
            "fmt",
            Class::Light,
            &[],
            Cmd::new("cargo").args(["fmt", "--all", "--check"]),
        ),
    ]);
    tasks
}

/// Static checks of the CI scripts (`ci::lint`).
fn ci_scripts() -> Task {
    Task::func("ci-scripts", Class::Light, &[], |ctx: Ctx| async move {
        let problems = crate::ci::lint::check(&ctx.root);
        for problem in &problems {
            ctx.note(problem);
        }
        if problems.is_empty() {
            Outcome::Ok
        } else {
            Outcome::Fail(format!(
                "{} problem(s) in the CI scripts, see the log",
                problems.len()
            ))
        }
    })
}

/// The module graph of `crates/core/src` against the layer table (`crate::arch`).
fn arch() -> Task {
    Task::func("arch", Class::Light, &[], |ctx: Ctx| async move {
        let report = match crate::arch::check(&ctx.root) {
            Ok(report) => report,
            Err(error) => return Outcome::Fail(format!("{error:#}")),
        };
        for line in report.lines() {
            ctx.note(&line);
        }
        match report.failures() {
            0 => Outcome::Ok,
            count => Outcome::Fail(format!(
                "{count} back edge(s) or stale allowlist entries, see the log"
            )),
        }
    })
}

const DEFAULT_BASE_REF: &str = "origin/development";

/// The revision the CI gate diffs against: `origin/<GITHUB_BASE_REF>` on a PR, else `origin/development`.
fn diff_base(github_base_ref: Option<&str>) -> String {
    match github_base_ref.filter(|name| !name.is_empty()) {
        Some(name) => format!("origin/{name}"),
        None => DEFAULT_BASE_REF.to_string(),
    }
}

/// The one line shown when the diff base is absent locally.
fn missing_base_notice(base: &str) -> String {
    format!("base ref {base} not found locally: running --all without --diff")
}

/// Puts the notice in the task log, the run's visible reason, instead of raw stderr.
fn note_missing_base(ctx: &Ctx, base: &str) {
    ctx.note(&missing_base_notice(base));
}

/// Arguments of `strangle check`: `--all`, plus `--diff <base>` when the base exists.
fn strangle_args(base: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = ["strangle", "check", "--all"].map(String::from).into();
    if let Some(base) = base {
        args.extend(["--diff".to_string(), base.to_string()]);
    }
    args
}

/// `strangle check --all --diff <base>`: the moved areas against `baseline.json` and the changed
/// files against the task scope. Without the base ref locally it runs `--all` alone and says so.
/// Runs this very binary, so no second cargo build is started.
fn strangle() -> Task {
    Task::func("strangle", Class::Light, &[], |ctx: Ctx| async move {
        let program = match std::env::current_exe() {
            Ok(path) => path.display().to_string(),
            Err(error) => return Outcome::Fail(format!("{error:#}")),
        };
        let base = diff_base(std::env::var("GITHUB_BASE_REF").ok().as_deref());
        let known = ctx
            .capture("git", &["rev-parse", "--verify", "--quiet", &base])
            .await
            .is_some();
        if !known {
            note_missing_base(&ctx, &base);
        }
        let cmd = Cmd::new(&program).args(strangle_args(known.then_some(base.as_str())));
        match ctx.exec(&cmd).await {
            Ok(ExecEnd::Done(0)) => Outcome::Ok,
            Ok(ExecEnd::Done(code)) => Outcome::Fail(format!("strangle check exited with {code}")),
            Ok(ExecEnd::Cancelled) => Outcome::Cancelled,
            Err(error) => Outcome::Fail(format!("{error:#}")),
        }
    })
}

/// Regenerates ipc.gen.d.ts and fails when it differs from the committed file; the file is put back.
fn gen_types(_root: &Path) -> Task {
    Task::func("gen-types", Class::Heavy, &[], |ctx: Ctx| async move {
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
    })
    .after(&["test"])
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserOpts, browser_cmd, diff_base, missing_base_notice, note_missing_base, parse_shard,
        strangle_args,
    };

    fn opts(quick: bool, shard: Option<&str>, jobs: Option<usize>) -> BrowserOpts {
        BrowserOpts {
            quick,
            shard: shard.map(String::from),
            jobs,
        }
    }

    #[test]
    fn browser_checks_are_full_unless_quick() {
        assert_eq!(
            browser_cmd("layout", &opts(true, None, None)).display(),
            "node scripts/run-browser-check.mjs layout --quick"
        );
        assert_eq!(
            browser_cmd("links", &opts(false, None, None)).display(),
            "node scripts/run-browser-check.mjs links"
        );
    }

    #[test]
    fn browser_checks_pass_the_shard_and_jobs_on() {
        assert_eq!(
            browser_cmd("layout", &opts(false, Some("2/6"), Some(1))).display(),
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

    #[test]
    fn strangle_diffs_against_origin_development_by_default() {
        assert_eq!(diff_base(None), "origin/development");
        assert_eq!(diff_base(Some("")), "origin/development");
        assert_eq!(
            strangle_args(Some(&diff_base(None))),
            ["strangle", "check", "--all", "--diff", "origin/development"]
        );
    }

    #[test]
    fn strangle_base_follows_github_base_ref() {
        assert_eq!(diff_base(Some("main")), "origin/main");
    }

    #[test]
    fn strangle_without_a_base_checks_all_only() {
        assert_eq!(strangle_args(None), ["strangle", "check", "--all"]);
    }

    #[test]
    fn missing_base_notice_is_one_line_naming_the_ref() {
        let notice = missing_base_notice("origin/development");
        assert_eq!(
            notice,
            "base ref origin/development not found locally: running --all without --diff"
        );
        assert!(!notice.contains('\n'));
    }

    #[test]
    fn strangle_missing_base_notice_lands_in_the_task_log() {
        let dir = std::env::temp_dir().join(format!("xtask-note-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = crate::exec::Ctx {
            root: dir.clone(),
            log: dir.join("strangle.log"),
            cancel: tokio_util::sync::CancellationToken::new(),
        };
        note_missing_base(&ctx, "origin/development");
        let logged = std::fs::read_to_string(&ctx.log).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            logged,
            format!("{}\n", missing_base_notice("origin/development"))
        );
    }
}
