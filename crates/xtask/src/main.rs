//! `cargo xtask`: the build and check engine (task graph, fail-fast, logs).

mod audit;
mod build;
mod changelog;
mod channel;
mod checks;
mod exec;
mod graph;
mod guard;
mod images;
mod lock;
mod platforms;
mod publish;
mod release;
mod sccache;
mod sink;
mod tauri_config;
mod version;

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
    /// Bundles for every platform (linux pacman flatpak android windows macos) into
    /// `.cache/build-all/builds/<channel>/`.
    Build(BuildArgs),
    /// The version a build ships under (X.Y.Z, or with `--channel` the one CI builds).
    Version(VersionArgs),
    /// Refresh the commit list of `## [Unreleased]` in CHANGELOG.md (git-cliff).
    Changelog,
    /// The next version into every file and `[Unreleased]` becomes its section of CHANGELOG.md.
    Bump {
        /// auto, patch, minor, major or X.Y.Z.
        #[arg(default_value = "auto")]
        level: String,
    },
    /// Dependency advisories (cargo-audit, pnpm audit) minus the reasoned exceptions of audit-ignore.toml.
    Audit,
    /// A GitHub release or pre-release: SHA256SUMS, then `gh release create`.
    Publish(publish::PublishArgs),
    /// Delete the old pre-releases (and their tags) beyond the newest N.
    PrunePrereleases(publish::PruneArgs),
    /// Open the pull request main -> development after a release.
    Backmerge(publish::BackmergeArgs),
    /// Push multi-wallet-tracker-bin to the AUR (for an archlinux container).
    PublishAur(publish::AurArgs),
    /// Write the Tauri config overlay of a build and print its absolute path.
    TauriConfig(TauriConfigArgs),
    /// The CI image tag: a hash of the Dockerfile, the apt lists and the [tools] of mise.toml.
    ImageTag,
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
    /// Only the checks of the web part (`checks::FRONTEND`; layout and links are sharded, see `--shard`).
    #[arg(long, conflicts_with_all = ["tasks", "no_frontend"])]
    frontend: bool,
    /// Everything else but the sharded sweeps (layout, links): a new task lands here, so a CI split into jobs cannot lose it.
    #[arg(long, conflicts_with = "tasks")]
    no_frontend: bool,
    /// Without the compile checks for other platforms (`platform:*`).
    #[arg(long, conflicts_with_all = ["tasks", "frontend"])]
    no_platforms: bool,
    /// layout and links over every width and both themes instead of `--quick` (CI runs this way).
    #[arg(long)]
    full_browser: bool,
    /// Only the K-th of N shares of the units of layout and links (the CI matrix); needs `--full-browser`.
    #[arg(long, value_name = "K/N", value_parser = checks::parse_shard, requires = "full_browser")]
    shard: Option<String>,
    /// Processes per share for layout and links (the script picks by the cores when absent).
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    jobs: Option<u32>,
    /// Print the task graph and exit.
    #[arg(long)]
    list: bool,
}

#[derive(Args)]
struct VersionArgs {
    /// Without it: the plain app version. `prod` is X.Y.Z, `dev` X.Y.Z-<N> (one past the last published pre-release), `none` X.Y.Z-<run>.
    #[arg(long, value_parser = ["dev", "prod", "none"])]
    channel: Option<String>,
    /// The run number, and the pre-release number too when given (default: $GITHUB_RUN_NUMBER, $CI_PIPELINE_IID, 1).
    #[arg(long)]
    run: Option<u64>,
    /// Append BUILD_VERSION, ANDROID_VERSION_CODE and RELEASE_CHANNEL as KEY=VALUE lines
    /// (the format of $GITHUB_OUTPUT).
    #[arg(long, requires = "channel")]
    output: Option<String>,
}

#[derive(Args)]
struct TauriConfigArgs {
    #[arg(long, default_value = "dev", value_parser = ["dev", "prod", "none"])]
    channel: String,
    #[arg(long)]
    version: String,
    /// The Android identifier instead of the desktop one.
    #[arg(long)]
    android: bool,
    #[arg(long, default_value_t = 1)]
    version_code: u32,
    /// dist/ is already built: tauri does not run `pnpm build` again.
    #[arg(long)]
    prebuilt_frontend: bool,
    /// Relative to the repository root.
    #[arg(long, default_value = tauri_config::DEFAULT_OUT)]
    out: String,
}

#[derive(Args)]
struct BuildArgs {
    /// Platforms (with what they need); all when empty.
    platforms: Vec<String>,
    /// `dev` is the app of development, `prod` the release of main (channel.rs).
    #[arg(long, default_value = "dev", value_parser = ["dev", "prod"])]
    channel: String,
    /// Default: the version of tauri.conf.json (`cargo xtask version`).
    #[arg(long)]
    version: Option<String>,
    /// Build linux and flatpak on this host instead of in containers.
    #[arg(long)]
    native: bool,
    /// `dist/` is already built (a CI job downloaded it): no frontend task, tauri does not run
    /// `pnpm build` either.
    #[arg(long)]
    prebuilt_frontend: bool,
    /// The Android versionCode (`cargo xtask version --channel ... --output` prints it).
    #[arg(long, default_value_t = 1)]
    version_code: u32,
    /// Cargo builds at once (android, windows, linux).
    #[arg(long, default_value_t = 2)]
    heavy: usize,
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
    let mut built = None;
    let (tasks, heavy) = match &cli.command {
        Command::Check(args) => {
            let browser = checks::BrowserOpts {
                full: args.full_browser,
                shard: args.shard.clone(),
                jobs: args.jobs.map(|jobs| jobs as usize),
            };
            let mut all = checks::check_tasks(&root, args.overlap_browser, &browser);
            if args.no_platforms {
                all.retain(|task| !task.name.starts_with("platform:"));
            }
            if args.no_frontend {
                all.retain(|task| {
                    !checks::FRONTEND.contains(&task.name.as_str())
                        && !checks::SHARDED.contains(&task.name.as_str())
                });
            }
            let names = if args.frontend {
                checks::FRONTEND.map(String::from).to_vec()
            } else {
                args.tasks.clone()
            };
            let tasks = graph::select(all, &names)?;
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
                graph::select(
                    checks::check_tasks(&root, false, &checks::BrowserOpts::default()),
                    &names,
                )?,
                *heavy,
            )
        }
        Command::Version(args) => {
            version::run(
                &root,
                &version::Args {
                    channel: args.channel.clone(),
                    run: args.run,
                    output: args.output.clone(),
                },
            )?;
            return Ok(0);
        }
        Command::Changelog => {
            release::run_changelog(&root)?;
            return Ok(0);
        }
        Command::Bump { level } => {
            release::run_bump(&root, level)?;
            return Ok(0);
        }
        Command::Audit => {
            return audit::run(&root);
        }
        Command::Publish(args) => {
            publish::run_publish(&root, args)?;
            return Ok(0);
        }
        Command::PrunePrereleases(args) => {
            publish::run_prune(args)?;
            return Ok(0);
        }
        Command::Backmerge(args) => {
            publish::run_backmerge(args)?;
            return Ok(0);
        }
        Command::PublishAur(args) => {
            publish::run_aur(&root, args)?;
            return Ok(0);
        }
        Command::TauriConfig(args) => {
            let path = tauri_config::write(
                &root,
                &args.out,
                &tauri_config::Params {
                    channel: channel::Channel::parse(&args.channel)?,
                    android: args.android,
                    version: &args.version,
                    version_code: args.version_code,
                    prebuilt_frontend: args.prebuilt_frontend,
                },
            )?;
            println!("{}", path.display());
            return Ok(0);
        }
        Command::ImageTag => {
            println!("{}", images::tag(&root)?);
            return Ok(0);
        }
        Command::Build(args) => {
            let version = match &args.version {
                Some(version) => version.clone(),
                None => version::tauri_version(&root)?,
            };
            build::check_version(&version)?;
            let env = Arc::new(build::Env::detect(
                root.clone(),
                &args.channel,
                &version,
                args.native,
                args.version_code,
            )?);
            if args.prebuilt_frontend && !root.join("dist/index.html").is_file() {
                anyhow::bail!("--prebuilt-frontend: dist/index.html is missing");
            }
            let facts = build::Facts {
                mac: build::MacFacts::detect(),
                tarball: env.tarball().is_file(),
                prebuilt_frontend: args.prebuilt_frontend,
            };
            let tasks = build::graph(&env, &args.platforms, &facts)?;
            if args.list {
                list(&tasks);
                return Ok(0);
            }
            std::fs::create_dir_all(root.join(&env.out))?;
            built = Some(env);
            (tasks, args.heavy)
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
    if let (Some(env), 0) = (&built, code) {
        build::print_output(env, &sink);
    }
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
    sccache::prepare(sink);
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
    if let Some(line) = sccache::stats_line() {
        sink.line(&line);
    }
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
