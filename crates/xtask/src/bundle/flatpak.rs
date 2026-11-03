//! The Flatpak bundle from the Linux tarball (`xtask bundle linux`), in an archlinux image.
//! flatpak-builder needs bubblewrap: the container must be privileged (GitHub-hosted runners
//! are; `podman run --privileged` locally).
//!
//! `MWT_APP_NAME` and `MWT_APP_FLATPAK_ID` come from the caller (`Channel::env` in crates/xtask).

use std::path::Path;

use anyhow::{Result, bail};

use super::LinuxArgs;
use crate::exec::{Cmd, run_inherited, which};

/// The committed manifest and metainfo are the dev app's.
const TEMPLATE: &str = "dev.mwt.MultiWalletTracker";
const DEV_NAME: &str = "<name>Multi Wallet Tracker Dev</name>";
const REMOTE: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// Another channel gets a copy of a template file under its own id and name.
fn rewrite(text: &str, flatpak_id: &str, app_name: &str) -> String {
    text.replace(TEMPLATE, flatpak_id)
        .replace(DEV_NAME, &format!("<name>{app_name}</name>"))
}

fn required(name: &str, what: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => bail!("{name}: {what}"),
    }
}

pub fn run(root: &Path, args: &LinuxArgs) -> Result<()> {
    let app_name = required("MWT_APP_NAME", "the channel product name")?;
    let flatpak_id = required("MWT_APP_FLATPAK_ID", "the channel Flatpak id")?;
    let release = root.join(&args.out);
    let version = &args.version;
    if which("flatpak-builder").is_none() {
        run_inherited(
            root,
            &Cmd::new("pacman").args([
                "-Sy",
                "--noconfirm",
                "--needed",
                "flatpak",
                "flatpak-builder",
                "git",
            ]),
        )?;
    }
    let flatpak = root.join("packaging/flatpak");
    let modules = flatpak.join("shared-modules");
    if modules.exists() {
        std::fs::remove_dir_all(&modules)?;
    }
    run_inherited(
        root,
        &Cmd::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "https://github.com/flathub/shared-modules.git",
            ])
            .arg(modules.display().to_string()),
    )?;
    std::fs::copy(
        release.join(format!(
            "multi-wallet-tracker-{version}-linux-x86_64.tar.gz"
        )),
        flatpak.join("multi-wallet-tracker-linux-x86_64.tar.gz"),
    )?;
    if flatpak_id != TEMPLATE {
        for suffix in ["yml", "metainfo.xml"] {
            let text = std::fs::read_to_string(flatpak.join(format!("{TEMPLATE}.{suffix}")))?;
            std::fs::write(
                flatpak.join(format!("{flatpak_id}.{suffix}")),
                rewrite(&text, &flatpak_id, &app_name),
            )?;
        }
    }
    run_inherited(
        root,
        &Cmd::new("flatpak").args(["remote-add", "--user", "--if-not-exists", "flathub", REMOTE]),
    )?;
    let manifest = format!("packaging/flatpak/{flatpak_id}.yml");
    run_inherited(
        root,
        &Cmd::new("flatpak-builder")
            .args([
                "--user",
                "--disable-rofiles-fuse",
                "--install-deps-from=flathub",
                "--force-clean",
            ])
            .args(["--repo=.flatpak-repo", ".flatpak-build", &manifest]),
    )?;
    let bundle = release.join(format!("multi-wallet-tracker-{version}-x86_64.flatpak"));
    run_inherited(
        root,
        &Cmd::new("flatpak")
            .args([
                "build-bundle",
                &format!("--runtime-repo={REMOTE}"),
                ".flatpak-repo",
            ])
            .arg(bundle.display().to_string())
            .arg(&flatpak_id),
    )?;
    run_inherited(
        root,
        &Cmd::new("ls").arg("-l").arg(bundle.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_manifest_gets_its_own_id_and_name() {
        let text = "id: dev.mwt.MultiWalletTracker\n<name>Multi Wallet Tracker Dev</name>\nx dev.mwt.MultiWalletTracker";
        assert_eq!(
            rewrite(text, "mwt.app.MultiWalletTracker", "Multi Wallet Tracker"),
            "id: mwt.app.MultiWalletTracker\n<name>Multi Wallet Tracker</name>\nx mwt.app.MultiWalletTracker"
        );
    }

    #[test]
    fn the_dev_template_is_left_as_it_is() {
        let text = "dev.mwt.MultiWalletTracker";
        assert_eq!(rewrite(text, TEMPLATE, "x"), text);
    }
}
