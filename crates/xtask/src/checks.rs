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
/// so by default they run alone: after every cargo build, one browser at a time
/// (`--overlap-browser` starts them at once; they are mostly waiting, so it saves minutes but can flake).
fn browser(task: Task, overlap: bool) -> Task {
    if overlap {
        task
    } else {
        task.after(&QUIET_AFTER)
    }
}

const QUIET_AFTER: [&str; 6] = [
    "test",
    "gen-types",
    "clippy",
    "platform:android",
    "platform:windows",
    "platform:macos",
];

/// `scripts/run-browser-check.mjs <name> --quick`: the vitest project `layout` on one file.
fn browser_check(name: &str) -> Task {
    let cmd = Cmd::new("node").args(["scripts/run-browser-check.mjs", name, "--quick"]);
    Task::cmd(name, Class::Browser, &[], cmd)
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
pub const FRONTEND: [&str; 7] = [
    "biome", "vue-tsc", "unit", "checks", "e2e", "layout", "links",
];

pub fn check_tasks(root: &Path, overlap_browser: bool) -> Vec<Task> {
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
        browser(browser_check("layout"), overlap_browser),
        browser(browser_check("links"), overlap_browser),
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
