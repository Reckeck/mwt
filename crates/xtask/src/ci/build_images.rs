//! `xtask ci-images`: builds and pushes the CI images (node, linux, android) that are not in ghcr
//! yet under `images::tag()`. Writes `tag` and `base` (lower-case ghcr.io/<owner>/<repo>) to
//! $GITHUB_OUTPUT. `--force` (or FORCE set) rebuilds. Needs a docker login to ghcr.io and
//! `packages: write`.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::exec::{Cmd, run_inherited};
use crate::images;

/// In the order of the Dockerfile stages: linux builds on node, android on linux.
pub const TARGETS: [&str; 3] = ["node", "linux", "android"];

/// `ghcr.io/<owner>/<repo>`, lower-cased (registry paths must be).
pub fn base(repository: &str) -> String {
    format!("ghcr.io/{}", repository.to_lowercase())
}

pub fn image(base: &str, target: &str, tag: &str) -> String {
    format!("{base}/ci-{target}:{tag}")
}

/// The earlier stage is in the local layer cache: linux reuses node, android reuses linux.
pub fn build_cmd(target: &str, image: &str, repository: &str) -> Cmd {
    Cmd::new("docker")
        .args([
            "build",
            "--target",
            target,
            "-f",
            "scripts/ci/ci.Dockerfile",
            "--label",
        ])
        .arg(format!(
            "org.opencontainers.image.source=https://github.com/{repository}"
        ))
        .args(["-t", image, "."])
}

fn pushed(image: &str) -> bool {
    Command::new("docker")
        .args(["manifest", "inspect", image])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn run(root: &Path, force: bool) -> Result<i32> {
    let repository = std::env::var("GITHUB_REPOSITORY")
        .ok()
        .filter(|value| !value.is_empty())
        .context("GITHUB_REPOSITORY is not set")?;
    let force = force || std::env::var_os("FORCE").is_some_and(|value| !value.is_empty());
    let tag = images::tag(root)?;
    let base = base(&repository);
    super::write_outputs(&[format!("tag={tag}"), format!("base={base}")])?;

    let mut missing = Vec::new();
    for target in TARGETS {
        let name = image(&base, target, &tag);
        if !force && pushed(&name) {
            println!("{name} is already pushed");
        } else {
            missing.push(target);
        }
    }
    if missing.is_empty() {
        return Ok(0);
    }

    // The android image is ~8 GB on top of what the runner image holds.
    let cleanup = Cmd::new("sudo").args([
        "rm",
        "-rf",
        "/usr/share/dotnet",
        "/opt/ghc",
        "/usr/local/lib/android",
        "/opt/hostedtoolcache/CodeQL",
    ]);
    let _ = run_inherited(root, &cleanup);
    let _ = run_inherited(root, &Cmd::new("df").args(["-h", "/"]));

    for target in missing {
        let name = image(&base, target, &tag);
        run_inherited(root, &build_cmd(target, &name, &repository))?;
        run_inherited(root, &Cmd::new("docker").args(["push", &name]))?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_is_lower_case() {
        assert_eq!(base("Reckeck/MWT"), "ghcr.io/reckeck/mwt");
    }

    #[test]
    fn images_are_built_in_stage_order() {
        assert_eq!(TARGETS, ["node", "linux", "android"]);
        assert_eq!(
            image("ghcr.io/o/r", "linux", "abc"),
            "ghcr.io/o/r/ci-linux:abc"
        );
    }

    #[test]
    fn the_build_command_names_the_stage_the_label_and_the_tag() {
        let cmd = build_cmd("android", "ghcr.io/o/r/ci-android:abc", "O/R");
        assert_eq!(
            cmd.display(),
            "docker build --target android -f scripts/ci/ci.Dockerfile --label \
             org.opencontainers.image.source=https://github.com/O/R -t ghcr.io/o/r/ci-android:abc ."
        );
    }
}
