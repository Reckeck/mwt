//! What every build task shares: channel, version, container engine, image names, volumes.

use std::path::PathBuf;

use tokio::sync::Mutex;

use crate::channel::Channel;
use crate::exec::{Cmd, Ctx, which};
use crate::graph::Outcome;

/// One sccache server port per lane, see `sccache::container_mount_on`.
fn sccache_port(lane: &str) -> u16 {
    crate::sccache::CONTAINER_PORT
        + match lane {
            "android" => 1,
            "windows" => 2,
            _ => 0,
        }
}

pub const ARCH_IMAGE: &str = "docker.io/library/archlinux:base-devel";

pub struct Env {
    pub root: PathBuf,
    pub channel: Channel,
    pub version: String,
    /// Where the bundles land, relative to the root.
    pub out: String,
    /// Build linux and flatpak on this host instead of in containers.
    pub native: bool,
    /// The Android versionCode of the overlay.
    pub version_code: u32,
    /// podman or docker; `None` when neither is installed.
    pub engine: Option<String>,
    pub image_tag: String,
    pub network: String,
    /// Hash of the root path: the volumes of each working copy are its own.
    pub checkout: String,
    /// Images are pulled or built one at a time (linux and windows share the ci-linux image).
    images: Mutex<()>,
}

impl Env {
    pub fn detect(
        root: PathBuf,
        channel: &str,
        version: &str,
        native: bool,
        version_code: u32,
    ) -> anyhow::Result<Self> {
        let engine = std::env::var("CONTAINER_ENGINE")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                which("podman")
                    .or_else(|| which("docker"))
                    .map(|path| path.display().to_string())
            });
        let channel = Channel::parse(channel)?;
        Ok(Self {
            channel,
            version: version.to_string(),
            out: format!(".cache/build-all/builds/{}", channel.label()),
            native,
            version_code,
            engine,
            image_tag: crate::images::tag(&root)?,
            network: std::env::var("CONTAINER_NETWORK")
                .ok()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "host".into()),
            checkout: crate::platforms::checkout_hash(&root),
            images: Mutex::new(()),
            root,
        })
    }

    /// The Linux tarball the pacman and flatpak builds start from.
    pub fn tarball(&self) -> PathBuf {
        self.root.join(&self.out).join(format!(
            "multi-wallet-tracker-{}-linux-x86_64.tar.gz",
            self.version
        ))
    }

    fn podman(&self) -> bool {
        self.engine
            .as_deref()
            .is_some_and(|engine| engine.ends_with("podman"))
    }

    pub fn engine(&self) -> Result<&str, Outcome> {
        self.engine.as_deref().ok_or_else(|| {
            Outcome::Fail("no podman or docker (CONTAINER_ENGINE names another engine)".into())
        })
    }

    /// `image build` arguments. podman builds OCI images by default, which do not keep SHELL: the
    /// stages of ci.Dockerfile built FROM node would run `source` in /bin/sh. Docker keeps it.
    fn build_args(&self) -> Vec<String> {
        let mut args = vec![format!("--network={}", self.network)];
        if self.podman() {
            args.extend(["--format".into(), "docker".into()]);
        }
        args
    }

    /// Runs a probe or a preparation step: `Ok(true)` when it exited with 0.
    async fn step(ctx: &Ctx, cmd: &Cmd) -> Result<bool, Outcome> {
        match ctx.succeeds(cmd).await {
            Ok(Some(done)) => Ok(done),
            Ok(None) => Err(Outcome::Cancelled),
            Err(error) => Err(Outcome::Fail(format!("{error:#}"))),
        }
    }

    /// Whether the engine has the image (its JSON description stays out of the log).
    async fn present(ctx: &Ctx, engine: &str, image: &str) -> bool {
        ctx.capture(engine, &["image", "inspect", image])
            .await
            .is_some()
    }

    /// The CI image of a kind (`linux` or `android`): present, pulled, or built from ci.Dockerfile.
    pub async fn ci_image(&self, ctx: &Ctx, kind: &str) -> Result<String, Outcome> {
        let engine = self.engine()?;
        let image = crate::images::image(kind, &self.image_tag);
        let _one_at_a_time = self.images.lock().await;
        if Self::present(ctx, engine, &image).await {
            return Ok(image);
        }
        if Self::step(ctx, &Cmd::new(engine).args(["pull", "-q", &image])).await? {
            return Ok(image);
        }
        let build = Cmd::new(engine)
            .args(["build"])
            .args(self.build_args())
            .args([
                "--target",
                kind,
                "-f",
                "scripts/ci/ci.Dockerfile",
                "-t",
                &image,
                ".",
            ]);
        if Self::step(ctx, &build).await? {
            Ok(image)
        } else {
            Err(Outcome::Fail(format!("cannot build the image {image}")))
        }
    }

    /// `localhost/mwt-windows-cross:<tag>`: ci-linux plus cargo-xwin, built here.
    pub async fn windows_image(&self, ctx: &Ctx) -> Result<String, Outcome> {
        let base = self.ci_image(ctx, "linux").await?;
        let engine = self.engine()?;
        let image = format!("localhost/mwt-windows-cross:{}", self.image_tag);
        let _one_at_a_time = self.images.lock().await;
        if Self::present(ctx, engine, &image).await {
            return Ok(image);
        }
        let build = Cmd::new(engine)
            .args(["build"])
            .args(self.build_args())
            .args(["--build-arg", &format!("BASE={base}")])
            .args([
                "-f",
                "scripts/local/windows-cross.Dockerfile",
                "-t",
                &image,
                "scripts/local",
            ]);
        if Self::step(ctx, &build).await? {
            Ok(image)
        } else {
            Err(Outcome::Fail(format!("cannot build the image {image}")))
        }
    }

    /// The command in a CI image with the project at /src, as a GitHub Actions job would see it.
    /// `lane` names the volumes (`mwt-build-<checkout>-<lane>-{target,node,home}`).
    pub fn run_ci(
        &self,
        lane: &str,
        image: &str,
        script: &str,
        more: Vec<String>,
    ) -> Result<Cmd, Outcome> {
        let engine = self.engine()?;
        let prefix = format!("mwt-build-{}-{lane}", self.checkout);
        let mut cmd = Cmd::new(engine)
            .args([
                "run",
                "--rm",
                "--init",
                &format!("--network={}", self.network),
            ])
            .args(["--name", &self.container_name(lane)])
            .args(["-v", &format!("{}:/src", self.root.display()), "-w", "/src"])
            .args(["-v", &format!("{prefix}-target:/src/.rust-target")])
            .args(["-v", &format!("{prefix}-node:/src/node_modules")])
            .args(["-v", &format!("{prefix}-home:/root")])
            .args([
                "-e",
                "CI_PROJECT_DIR=/src",
                "-e",
                "CI=true",
                "-e",
                "GRADLE_USER_HOME=/src/.cache/gradle",
                "-e",
                &format!("RELEASE_CHANNEL={}", self.channel.label()),
            ]);
        for (name, value) in self.channel.env() {
            cmd = cmd.args(["-e", &format!("{name}={value}")]);
        }
        // The channel's release profile (dev: no LTO). A value set in the environment of the run
        // wins and is copied into the container by name.
        for (name, value) in self.channel.build_env_here() {
            cmd = cmd.args(["-e", &format!("{name}={value}")]);
        }
        for name in [
            "CARGO_PROFILE_RELEASE_LTO",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
            "ANDROID_TARGETS",
            "ANDROID_KEYSTORE_BASE64",
            "ANDROID_KEYSTORE_PASSWORD",
            "ANDROID_KEY_ALIAS",
            "ANDROID_KEY_PASSWORD",
            // CI: AppImage without FUSE, Gradle without a daemon.
            "APPIMAGE_EXTRACT_AND_RUN",
            "NO_STRIP",
            "GRADLE_OPTS",
        ] {
            cmd = cmd.args(["-e", name]);
        }
        Ok(cmd
            .args(crate::sccache::container_mount_on(sccache_port(lane)))
            .args(more)
            .args([
                image,
                "bash",
                "-c",
                &format!(
                    "set -eo pipefail; source scripts/ci/setup-linux.sh; mkdir -p dist; {script}"
                ),
            ]))
    }

    /// The command in archlinux:base-devel (pacman, flatpak).
    pub fn run_arch(&self, lane: &str, script: &str, more: &[&str]) -> Result<Cmd, Outcome> {
        let engine = self.engine()?;
        Ok(Cmd::new(engine)
            .args([
                "run",
                "--rm",
                "--init",
                &format!("--network={}", self.network),
            ])
            .args(["--name", &self.container_name(lane)])
            .args(["-e", &format!("RELEASE_CHANNEL={}", self.channel.label())])
            .args(
                self.channel
                    .env()
                    .iter()
                    .flat_map(|(name, value)| ["-e".to_string(), format!("{name}={value}")]),
            )
            .args(more)
            .args(["-v", &format!("{}:/src", self.root.display()), "-w", "/src"])
            .args([
                ARCH_IMAGE,
                "bash",
                "-c",
                &format!("set -eo pipefail; {script}"),
            ]))
    }

    fn container_name(&self, lane: &str) -> String {
        format!("mwt-xtask-{}-{lane}-{}", self.checkout, std::process::id())
    }

    /// Runs a container command. A cancelled or failed run leaves no container behind: killing the
    /// `podman run` client does not always stop the container itself.
    pub async fn finish(&self, ctx: &Ctx, lane: &str, cmd: Cmd) -> Outcome {
        let outcome = crate::graph::run_cmd(ctx, &cmd).await;
        if !matches!(outcome, Outcome::Ok)
            && let Some(engine) = &self.engine
        {
            let name = self.container_name(lane);
            for args in [vec!["kill", name.as_str()], vec!["rm", "-f", name.as_str()]] {
                let _ = std::process::Command::new(engine)
                    .args(args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
        outcome
    }
}
