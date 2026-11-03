//! The command line: every command and its flags.

use clap::{Args, Parser, Subcommand};

use crate::bundle::BundleArgs;
use crate::output::OutputArgs;
use crate::version::Kind;
use crate::{checks, publish, tauri_config};

#[derive(Parser)]
#[command(name = "xtask", about = "Build and check engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// The gate: lint, typecheck, UI checks, browser checks, clippy, tests, platforms (or the part you name).
    Check(CheckArgs),
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
    #[command(hide = true)]
    Publish(publish::PublishArgs),
    /// Delete the old pre-releases (and their tags) beyond the newest N.
    #[command(hide = true)]
    PrunePrereleases(publish::PruneArgs),
    /// Open the pull request main -> development after a release.
    #[command(hide = true)]
    Backmerge(publish::BackmergeArgs),
    /// Push multi-wallet-tracker-bin to the AUR (for an archlinux container).
    #[command(hide = true)]
    PublishAur(publish::AurArgs),
    /// Write the Tauri config overlay of a build and print its absolute path.
    #[command(hide = true)]
    TauriConfig(TauriConfigArgs),
    /// The packaging step of a platform build (runs in the build containers with the static xtask).
    #[command(hide = true)]
    Bundle(BundleArgs),
    /// The CI image tag: a hash of the Dockerfile, the apt lists and the [tools] of mise.toml.
    #[command(hide = true)]
    ImageTag,
    /// The cache key of a CI lane: `<lane>-<16 hex>` (check-linux, build-linux-dev, node ...).
    #[command(hide = true)]
    LaneKey { lane: String },
    /// Compile only the dependencies of a lane, so sccache holds them (the `deps` job).
    #[command(hide = true)]
    DepsCook { lane: String },
    /// Delete the old caches of a lane on this ref, keeping the key just saved.
    #[command(hide = true)]
    DepsPrune { prefix: String, keep: String },
    /// Append the shared job environment (sccache, RUST_LOG) to $GITHUB_ENV.
    #[command(hide = true)]
    CiEnv,
    /// Install the pinned cargo-chef.
    #[command(hide = true)]
    InstallChef,
    /// For tests of the run lock: sleeps, then exits with `--code`.
    #[command(hide = true)]
    SelftestSleep {
        seconds: u64,
        #[arg(long, default_value_t = 0)]
        code: i32,
    },
}

/// The flags of every command that runs a task graph.
#[derive(Args, Clone, Default)]
pub struct GraphArgs {
    /// Print the task graph and exit.
    #[arg(long, conflicts_with = "dry_run")]
    pub list: bool,
    /// Print each task in run order with the command it would run, and exit without running.
    #[arg(long)]
    pub dry_run: bool,
    /// A failure does not cancel the other tasks; only those that depend on it are skipped
    /// (CANCELLED, `dependency <name> failed`). The exit code is still 1.
    #[arg(long)]
    pub keep_going: bool,
    #[command(flatten)]
    pub out: OutputArgs,
}

#[derive(Args)]
#[command(after_help = checks::groups::help())]
pub struct CheckArgs {
    /// Tasks and @groups to run, united, with the tasks they depend on; everything when empty.
    /// `linux`, `windows`, `android` and `macos` name the compile check of a platform.
    pub items: Vec<String>,
    /// Leave out a task or @group (repeatable). It is taken from the selection first; a task a kept
    /// one depends on still runs (`gen-types` needs `test`).
    #[arg(long, value_name = "NAME|@GROUP")]
    pub exclude: Vec<String>,
    /// Cargo builds at once.
    #[arg(long, default_value_t = 2)]
    pub heavy: usize,
    /// Run layout/links next to the cargo builds (faster, but timing checks can flake under load).
    #[arg(long)]
    pub overlap_browser: bool,
    /// layout and links on 1280 and 390 only. Without it they run over every width and both themes.
    #[arg(long)]
    pub quick: bool,
    /// Only the K-th of N shares of the units of layout and links (the CI matrix); full runs only.
    #[arg(long, value_name = "K/N", value_parser = checks::parse_shard, conflicts_with = "quick")]
    pub shard: Option<String>,
    /// Processes per share for layout and links (the script picks by the cores when absent).
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub jobs: Option<u32>,
    #[command(flatten)]
    pub graph: GraphArgs,
}

#[derive(Args)]
pub struct VersionArgs {
    /// Without it: the plain app version. `prod` is X.Y.Z, `dev` X.Y.Z-<N> (one past the last published pre-release), `none` X.Y.Z-<run>.
    #[arg(long, value_enum)]
    pub channel: Option<Kind>,
    /// The run number, and the pre-release number too when given (default: $GITHUB_RUN_NUMBER, 1).
    #[arg(long)]
    pub run: Option<u64>,
    /// Append BUILD_VERSION, ANDROID_VERSION_CODE and RELEASE_CHANNEL as KEY=VALUE lines
    /// (the format of $GITHUB_OUTPUT).
    #[arg(long, requires = "channel")]
    pub output: Option<String>,
}

#[derive(Args)]
pub struct TauriConfigArgs {
    #[arg(long, value_enum, default_value_t = Kind::Dev)]
    pub channel: Kind,
    #[arg(long)]
    pub version: String,
    /// The Android identifier instead of the desktop one.
    #[arg(long)]
    pub android: bool,
    #[arg(long, default_value_t = 1)]
    pub version_code: u32,
    /// dist/ is already built: tauri does not run `pnpm build` again.
    #[arg(long)]
    pub prebuilt_frontend: bool,
    /// Relative to the repository root.
    #[arg(long, default_value = tauri_config::DEFAULT_OUT)]
    pub out: String,
}

#[derive(Args)]
pub struct BuildArgs {
    /// Platforms (with what they need); all when empty.
    pub platforms: Vec<String>,
    /// `dev` is the app of development, `prod` the release of main (channel.rs); `none` is refused.
    #[arg(long, value_enum, default_value_t = Kind::Dev)]
    pub channel: Kind,
    /// Default: the version of tauri.conf.json (`cargo xtask version`).
    #[arg(long)]
    pub version: Option<String>,
    /// Build linux and flatpak on this host instead of in containers.
    #[arg(long)]
    pub native: bool,
    /// `dist/` is already built (a CI job downloaded it): no frontend task, tauri does not run
    /// `pnpm build` either.
    #[arg(long)]
    pub prebuilt_frontend: bool,
    /// The Android versionCode (`cargo xtask version --channel ... --output` prints it).
    #[arg(long, default_value_t = 1)]
    pub version_code: u32,
    /// Cargo builds at once (android, windows, linux).
    #[arg(long, default_value_t = 2)]
    pub heavy: usize,
    #[command(flatten)]
    pub graph: GraphArgs,
}
