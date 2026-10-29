//! `cargo xtask`: the build and check engine (task graph, fail-fast, logs).

mod checks;
mod exec;
mod graph;
mod guard;
mod lock;
mod platforms;
mod sink;

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use graph::{Opts, Status, Task};
use sink::Sink;

#[derive(Parser)]
#[command(name = "xtask", about = "Build and check engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// The full gate: lint, typecheck, UI checks, browser checks, clippy, tests, platforms.
    Check(CheckArgs),
    /// Compile check (clippy) per platform: linux, windows, android, macos.
    CheckPlatforms {
        platforms: Vec<String>,
        /// Cargo builds at once.
        #[arg(long, default_value_t = 2)]
        heavy: usize,
    },
    /// For tests of the run lock: sleeps, then exits with `--code`.
    #[command(hide = true)]
    SelftestSleep {
        seconds: u64,
        #[arg(long, default_value_t = 0)]
        code: i32,
    },
}

#[derive(Args)]
struct CheckArgs {
    /// Task names (with their dependencies); all when empty.
    tasks: Vec<String>,
    /// Cargo builds at once.
    #[arg(long, default_value_t = 2)]
    heavy: usize,
    /// Run layout/links next to the cargo builds (faster, but timing checks can flake under load).
    #[arg(long)]
    overlap_browser: bool,
    /// Print the task graph and exit.
    #[arg(long)]
    list: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(error) => {
            eprintln!("xtask: {error:#}");
            ExitCode::from(2)
        }
    }
}

async fn run() -> Result<i32> {
    let cli = Cli::parse();
    let root = lock::find_root()?;
    let (tasks, heavy) = match &cli.command {
        Command::Check(args) => {
            let tasks = graph::select(
                checks::check_tasks(&root, args.overlap_browser),
                &args.tasks,
            )?;
            if args.list {
                list(&tasks);
                return Ok(0);
            }
            (tasks, args.heavy)
        }
        Command::CheckPlatforms { platforms, heavy } => {
            let mut names = Vec::new();
            for platform in platforms {
                let Some(name) = platforms::task_name(platform) else {
                    anyhow::bail!(
                        "unknown platform `{platform}` (one of: {})",
                        platforms::PLATFORMS.join(", ")
                    );
                };
                names.push(name);
            }
            if names.is_empty() {
                names = platforms::PLATFORMS
                    .iter()
                    .filter_map(|name| platforms::task_name(name))
                    .collect();
            }
            (
                graph::select(checks::check_tasks(&root, false), &names)?,
                *heavy,
            )
        }
        Command::SelftestSleep { .. } => (Vec::new(), 1),
    };
    let cmdline = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let guard = match lock::enter(&root, &cmdline).await? {
        lock::Role::Attached(code) => return Ok(code),
        lock::Role::Owner(guard) => guard,
    };
    let sink = guard.sink.clone();
    let cancel = CancellationToken::new();
    let interrupted = watch_signals(cancel.clone());
    let code = match cli.command {
        Command::SelftestSleep { seconds, code } => selftest(&sink, &cancel, seconds, code).await,
        _ => run_tasks(tasks, heavy, &root, &sink, &cancel).await,
    };
    let code = if interrupted.is_cancelled() {
        130
    } else {
        code
    };
    sink.line(&format!("exit code {code}"));
    sink.exit_marker(code);
    Ok(code)
}

async fn run_tasks(
    tasks: Vec<Task>,
    heavy: usize,
    root: &std::path::Path,
    sink: &Arc<Sink>,
    cancel: &CancellationToken,
) -> i32 {
    let schemas = guard::SchemasGuard::take(root);
    let reports = graph::run_graph(
        tasks,
        Opts { heavy, browser: 1 },
        cancel.clone(),
        sink.clone(),
        root,
        &root.join(".cache/xtask"),
    )
    .await;
    schemas.restore();
    graph::summary(&reports, sink.elapsed(), sink);
    i32::from(reports.iter().any(|report| report.status == Status::Fail))
}

async fn selftest(sink: &Sink, cancel: &CancellationToken, seconds: u64, code: i32) -> i32 {
    sink.line("start  sleep");
    tokio::select! {
        () = tokio::time::sleep(Duration::from_secs(seconds)) => sink.line("OK     sleep"),
        () = cancel.cancelled() => sink.line("CANCELLED sleep"),
    }
    code
}

fn list(tasks: &[Task]) {
    for task in tasks {
        let deps = if task.deps.is_empty() {
            String::new()
        } else {
            format!("  after: {}", task.deps.join(", "))
        };
        println!("{:<18} {:?}{deps}", task.name, task.class);
    }
}

/// Ctrl+C and SIGTERM cancel the run like a failure does. The returned token is set when that happened.
fn watch_signals(cancel: CancellationToken) -> CancellationToken {
    let interrupted = CancellationToken::new();
    let flag = interrupted.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        flag.cancel();
        cancel.cancel();
    });
    interrupted
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut term), Ok(mut hup)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::hangup()),
    ) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
        _ = hup.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
