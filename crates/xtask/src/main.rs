//! `cargo xtask`: the build and check engine (task graph, fail-fast, logs).

mod annotate;
mod audit;
mod build;
mod bundle;
mod cache;
mod changelog;
mod channel;
mod checks;
mod ci;
mod cli;
mod exec;
mod graph;
mod guard;
mod images;
mod jobsummary;
mod lock;
mod logtext;
mod output;
mod platforms;
mod publish;
mod release;
mod runner;
mod sccache;
mod sink;
mod tauri_config;
mod version;

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio_util::sync::CancellationToken;

use cli::{Cli, Command, GraphArgs};
use graph::{Opts, Task};
use output::Output;

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

/// A selected graph and how to run it.
struct Plan {
    tasks: Vec<Task>,
    heavy: usize,
    graph: GraphArgs,
}

/// `--list` and `--dry-run` print and finish; `None` means "run it".
fn print_only(plan: &Plan) -> Option<i32> {
    if plan.graph.list {
        print!("{}", graph::list_text(&plan.tasks));
    } else if plan.graph.dry_run {
        print!("{}", graph::plan_text(&plan.tasks));
    } else {
        return None;
    }
    Some(0)
}

async fn run() -> Result<i32> {
    let cli = Cli::parse();
    // No checkout in that job: no workspace root either.
    if matches!(cli.command, Command::CiOk) {
        ci::ci_ok::run()?;
        return Ok(0);
    }
    let root = lock::find_root()?;
    let mut built = None;
    let plan = match &cli.command {
        Command::Check(args) => {
            let browser = checks::BrowserOpts {
                quick: args.quick,
                shard: args.shard.clone(),
                jobs: args.jobs.map(|jobs| jobs as usize),
            };
            let all = checks::check_tasks(&root, args.overlap_browser, &browser);
            let known: Vec<String> = all.iter().map(|task| task.name.clone()).collect();
            let names = checks::groups::choose(&known, &args.items, &args.exclude)?;
            Plan {
                tasks: graph::select(all, &names)?,
                heavy: args.heavy,
                graph: args.graph.clone(),
            }
        }
        Command::Version(args) => {
            version::run(
                &root,
                &version::Args {
                    channel: args.channel,
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
                    channel: args.channel.names(),
                    android: args.android,
                    version: &args.version,
                    version_code: args.version_code,
                    prebuilt_frontend: args.prebuilt_frontend,
                },
            )?;
            println!("{}", path.display());
            return Ok(0);
        }
        Command::Bundle(args) => {
            bundle::run(&root, args)?;
            return Ok(0);
        }
        Command::ImageTag => {
            println!("{}", images::tag(&root)?);
            return Ok(0);
        }
        Command::ReleasePlan => {
            return ci::release_plan::run();
        }
        Command::CheckReleaseFiles {
            dir,
            version,
            macos,
        } => {
            return ci::release_files::run(dir, version, *macos);
        }
        Command::CiImages { force } => {
            return ci::build_images::run(&root, *force);
        }
        Command::LaneKey { lane } => {
            println!("{}", cache::lane_key::run(&root, lane)?);
            return Ok(0);
        }
        Command::DepsCook { lane } => {
            cache::deps_cook::run(&root, lane)?;
            return Ok(0);
        }
        Command::DepsPrune { prefix, keep } => {
            cache::deps_prune::run(prefix, keep)?;
            return Ok(0);
        }
        Command::CiEnv => {
            cache::ci_env::run()?;
            return Ok(0);
        }
        Command::CiContainerEnv { cargo } => {
            ci::container_env::run(*cargo)?;
            return Ok(0);
        }
        Command::CiOk => {
            ci::ci_ok::run()?;
            return Ok(0);
        }
        Command::CiPnpmInstall => {
            ci::pnpm_install::run()?;
            return Ok(0);
        }
        Command::CacheCleanup => {
            ci::cache_cleanup::run()?;
            return Ok(0);
        }
        Command::InstallChef => {
            cache::install_chef::run()?;
            return Ok(0);
        }
        Command::Build(args) => {
            let channel = args.channel.publishing("build")?;
            let version = match &args.version {
                Some(version) => version.clone(),
                None => version::tauri_version(&root)?,
            };
            build::check_version(&version)?;
            if args.free_disk {
                build::free_disk();
            }
            let env = Arc::new(build::Env::detect(
                root.clone(),
                channel,
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
            let plan = Plan {
                tasks,
                heavy: args.heavy,
                graph: args.graph.clone(),
            };
            if let Some(code) = print_only(&plan) {
                return Ok(code);
            }
            std::fs::create_dir_all(root.join(&env.out))?;
            built = Some(env);
            plan
        }
        Command::SelftestSleep { .. } => Plan {
            tasks: Vec::new(),
            heavy: 1,
            graph: GraphArgs::default(),
        },
    };
    if let Some(code) = print_only(&plan) {
        return Ok(code);
    }
    let cmdline = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let output = Output::from_args(&plan.graph.out);
    let guard = match lock::enter(&root, &cmdline, output).await? {
        lock::Role::Attached(code) => return Ok(code),
        lock::Role::Owner(guard) => guard,
    };
    let sink = guard.sink.clone();
    let cancel = CancellationToken::new();
    let interrupted = runner::watch_signals(cancel.clone());
    let (code, finished) = match cli.command {
        Command::SelftestSleep { seconds, code } => {
            (runner::selftest(&sink, &cancel, seconds, code).await, None)
        }
        _ => {
            let opts = Opts {
                heavy: plan.heavy,
                browser: 1,
                keep_going: plan.graph.keep_going,
            };
            let finished = runner::run_tasks(plan.tasks, opts, &root, &sink, &cancel).await;
            (i32::from(finished.failed()), Some(finished))
        }
    };
    let bundles = match (&built, code) {
        (Some(env), 0) => {
            build::print_output(env, &sink);
            build::bundles(env)
        }
        _ => Vec::new(),
    };
    if let Some(finished) = &finished {
        let stats = finished
            .stats_text
            .as_deref()
            .and_then(sccache::parse_stats);
        jobsummary::append(&jobsummary::markdown(
            &cmdline,
            &finished.reports,
            finished.total,
            stats.as_ref(),
            &bundles,
        ));
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
