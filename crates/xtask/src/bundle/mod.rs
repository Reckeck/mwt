//! `xtask bundle <platform>`: the packaging step of a build, run inside the build container (a
//! static musl xtask is mounted there) or natively by `cargo xtask build`.

mod android;
mod flatpak;
mod linux;
mod pacman;

use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args)]
pub struct BundleArgs {
    #[command(subcommand)]
    platform: Platform,
}

#[derive(Subcommand)]
enum Platform {
    /// deb, rpm, AppImage and the portable tarball, under release names.
    Linux(LinuxArgs),
    /// The universal APK and AAB, signed.
    Android(LinuxArgs),
    /// The pacman package from the Linux tarball (in an archlinux container).
    Pacman(LinuxArgs),
    /// The Flatpak bundle from the Linux tarball (in an archlinux container).
    Flatpak(LinuxArgs),
}

#[derive(Args)]
pub struct LinuxArgs {
    /// The build version: X.Y.Z, or a pre-release X.Y.Z-N.
    #[arg(long)]
    pub version: String,
    /// The folder the bundles land in, relative to the repository root.
    #[arg(long)]
    pub out: String,
}

pub fn run(root: &Path, args: &BundleArgs) -> Result<()> {
    match &args.platform {
        Platform::Linux(args) => linux::run(root, args),
        Platform::Android(args) => android::run(root, args),
        Platform::Pacman(args) => pacman::run(root, args),
        Platform::Flatpak(args) => flatpak::run(root, args),
    }
}
