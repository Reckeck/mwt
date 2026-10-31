//! `cargo xtask publish-aur`: push multi-wallet-tracker-bin to the AUR for a release. Meant for an
//! archlinux container (pacman, useradd, makepkg). The key (AUR_SSH_PRIVATE_KEY, a file path or the key
//! text) only goes into `~/.ssh/aur`: never into a command, an argument or the output.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::AurArgs;
use crate::exec::Cmd;

const PACKAGE: &str = "multi-wallet-tracker-bin";
const KEY_VAR: &str = "AUR_SSH_PRIVATE_KEY";

/// Where everything is; the key is not here on purpose.
pub struct Plan {
    pub version: String,
    pub tarball: PathBuf,
    pub url: String,
    pub home: PathBuf,
    pub work: PathBuf,
    pub root: PathBuf,
    pub git_name: String,
    pub git_email: String,
}

pub enum Step {
    /// `$AUR_SSH_PRIVATE_KEY` into `~/.ssh/aur`, mode 0600.
    WriteKey(PathBuf),
    Run(Cmd),
    /// The stdout of a command into a file.
    Capture {
        cmd: Cmd,
        out: PathBuf,
        append: bool,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
    },
    /// Exit code 0 means there is nothing to commit: the run ends.
    StopIfClean(Cmd),
}

pub fn tarball_name(version: &str) -> String {
    format!("multi-wallet-tracker-{version}-linux-x86_64.tar.gz")
}

pub fn tarball_url(repo: &str, version: &str) -> String {
    format!(
        "https://github.com/{repo}/releases/download/v{version}/{}",
        tarball_name(version)
    )
}

fn display(cmd: &Cmd) -> String {
    let env: String = cmd
        .env
        .iter()
        .map(|(key, value)| format!("{key}={} ", super::quote(value)))
        .collect();
    format!("{env}{}", super::shell_line(&cmd.program, &cmd.args))
}

impl Step {
    pub fn show(&self) -> String {
        match self {
            Self::WriteKey(path) => format!("write ${KEY_VAR} to {} (mode 0600)", path.display()),
            Self::Run(cmd) => display(cmd),
            Self::Capture { cmd, out, append } => {
                format!(
                    "{} {} {}",
                    display(cmd),
                    if *append { ">>" } else { ">" },
                    out.display()
                )
            }
            Self::Copy { from, to } => format!("cp {} {}", from.display(), to.display()),
            Self::StopIfClean(cmd) => {
                format!("{}  # exit 0 = AUR is up to date, stop", display(cmd))
            }
        }
    }
}

pub fn steps(plan: &Plan) -> Vec<Step> {
    let key = plan.home.join(".ssh/aur");
    let clone = plan.work.join("aur");
    let clone_text = clone.display().to_string();
    let ssh = format!("ssh -i {} -o IdentitiesOnly=yes", key.display());
    let git = |rest: &[&str]| {
        Cmd::new("git")
            .args([
                "-c",
                &format!("safe.directory={clone_text}"),
                "-C",
                &clone_text,
            ])
            .args(rest)
    };
    vec![
        Step::Run(Cmd::new("pacman").args([
            "-Sy",
            "--noconfirm",
            "--needed",
            "base-devel",
            "git",
            "openssh",
        ])),
        Step::WriteKey(key),
        Step::Capture {
            cmd: Cmd::new("ssh-keyscan").args(["-t", "ed25519,rsa", "aur.archlinux.org"]),
            out: plan.home.join(".ssh/known_hosts"),
            append: true,
        },
        Step::Run(Cmd::new("sh").args(["-c", "id builder >/dev/null 2>&1 || useradd -m builder"])),
        Step::Run(
            Cmd::new("git")
                .args([
                    "clone",
                    &format!("ssh://aur@aur.archlinux.org/{PACKAGE}.git"),
                    &clone_text,
                ])
                .env("GIT_SSH_COMMAND", &ssh),
        ),
        Step::Capture {
            cmd: Cmd::new(
                plan.root
                    .join("packaging/aur/render.sh")
                    .display()
                    .to_string()
                    .as_str(),
            )
            .args([
                plan.version.as_str(),
                &plan.tarball.display().to_string(),
                &plan.url,
            ]),
            out: clone.join("PKGBUILD"),
            append: false,
        },
        Step::Copy {
            from: plan.root.join(format!("packaging/aur/{PACKAGE}.install")),
            to: clone.join(format!("{PACKAGE}.install")),
        },
        Step::Run(Cmd::new("chown").args(["-R", "builder", &plan.work.display().to_string()])),
        Step::Run(Cmd::new("su").args([
            "builder",
            "-c",
            &format!("cd '{clone_text}' && makepkg --printsrcinfo > .SRCINFO"),
        ])),
        Step::Run(git(&["config", "user.name", &plan.git_name])),
        Step::Run(git(&["config", "user.email", &plan.git_email])),
        Step::Run(git(&[
            "add",
            "PKGBUILD",
            ".SRCINFO",
            &format!("{PACKAGE}.install"),
        ])),
        Step::StopIfClean(git(&["diff", "--cached", "--quiet"])),
        Step::Run(git(&[
            "commit",
            "-m",
            &format!("Update to {}", plan.version),
        ])),
        Step::Run(git(&["push", "origin", "HEAD:master"]).env("GIT_SSH_COMMAND", &ssh)),
    ]
}

/// The key text: the value itself, or the content of the file it names.
fn key_text(value: &str) -> Result<String> {
    let text = if Path::new(value).is_file() {
        std::fs::read_to_string(value).context("read the AUR key file")?
    } else {
        value.to_string()
    };
    Ok(format!("{}\n", text.trim_end()))
}

fn write_key(path: &Path) -> Result<()> {
    let value = std::env::var(KEY_VAR).unwrap_or_default();
    if value.is_empty() {
        bail!("set {KEY_VAR}");
    }
    let text = key_text(&value)?;
    let dir = path.parent().context("key path")?;
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        // Created with 0600: the key is never readable by others, not even for a moment.
        let _ = std::fs::remove_file(path);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(text.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, text)?;
    Ok(())
}

fn command(cmd: &Cmd) -> Command {
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .envs(cmd.env.iter().map(|(key, value)| (key, value)));
    command
}

fn exec(cmd: &Cmd) -> Result<bool> {
    println!("$ {}", display(cmd));
    let status = command(cmd)
        .status()
        .with_context(|| format!("cannot start `{}`", cmd.program))?;
    Ok(status.success())
}

fn execute(step: &Step) -> Result<bool> {
    match step {
        Step::WriteKey(path) => write_key(path)?,
        Step::Run(cmd) => {
            if !exec(cmd)? {
                bail!("`{}` failed", cmd.program);
            }
        }
        Step::Capture { cmd, out, append } => {
            println!("$ {}", step.show());
            let output = command(cmd)
                .output()
                .with_context(|| format!("cannot start `{}`", cmd.program))?;
            if !output.status.success() {
                bail!("`{}` failed", cmd.program);
            }
            let mut file = std::fs::OpenOptions::new();
            file.create(true)
                .write(true)
                .append(*append)
                .truncate(!*append);
            std::io::Write::write_all(&mut file.open(out)?, &output.stdout)?;
        }
        Step::Copy { from, to } => {
            println!("$ {}", step.show());
            std::fs::copy(from, to)?;
        }
        Step::StopIfClean(cmd) => return exec(cmd).map(|clean| !clean),
    }
    Ok(true)
}

pub fn run(root: &Path, args: &AurArgs) -> Result<()> {
    let name = tarball_name(&args.version);
    let actor = std::env::var("GITHUB_ACTOR").unwrap_or_else(|_| "github-actions[bot]".into());
    let plan = Plan {
        version: args.version.clone(),
        tarball: std::path::absolute(&args.dir)?.join(&name),
        url: tarball_url(&super::repo(), &args.version),
        home: PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into())),
        work: std::env::temp_dir().join(format!("mwt-aur-{}", std::process::id())),
        root: root.to_path_buf(),
        git_name: std::env::var("AUR_GIT_NAME").unwrap_or_else(|_| actor.clone()),
        git_email: std::env::var("AUR_GIT_EMAIL")
            .unwrap_or_else(|_| format!("{actor}@users.noreply.github.com")),
    };
    let all = steps(&plan);
    if args.dry_run {
        for step in &all {
            println!("$ {}", step.show());
        }
        return Ok(());
    }
    if !plan.tarball.is_file() {
        bail!("{} not found", plan.tarball.display());
    }
    for step in &all {
        if !execute(step)? {
            println!("AUR already at {}", args.version);
            return Ok(());
        }
    }
    Ok(())
}
