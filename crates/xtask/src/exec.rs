//! Running one external command for a task: output to the task log, the whole
//! process tree killed on cancel.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

const GRACE: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Cmd {
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_string()));
        self
    }

    pub fn env(mut self, key: &str, value: impl AsRef<str>) -> Self {
        self.env.push((key.to_string(), value.as_ref().to_string()));
        self
    }

    pub fn display(&self) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args.iter().cloned());
        parts.join(" ")
    }
}

pub enum ExecEnd {
    /// The process ended on its own with this exit code.
    Done(i32),
    /// The run was cancelled and the process tree was killed.
    Cancelled,
}

/// What a task action gets: where to run, where to log, how to stop.
#[derive(Clone)]
pub struct Ctx {
    pub root: PathBuf,
    pub log: PathBuf,
    pub cancel: CancellationToken,
}

impl Ctx {
    /// A line for the task log (reasons for SKIP and FAIL).
    pub fn note(&self, text: &str) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.log) {
            let _ = writeln!(file, "{text}");
        }
    }

    pub async fn exec(&self, cmd: &Cmd) -> anyhow::Result<ExecEnd> {
        self.note(&format!("$ {}", cmd.display()));
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)?;
        let err = out.try_clone()?;
        let mut command = Command::new(&cmd.program);
        command
            .args(&cmd.args)
            .envs(cmd.env.iter().map(|(key, value)| (key, value)))
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|source| anyhow::anyhow!("cannot start `{}`: {source}", cmd.program))?;
        let pid = child.id();
        tokio::select! {
            status = child.wait() => Ok(ExecEnd::Done(status?.code().unwrap_or(-1))),
            () = self.cancel.cancelled() => {
                terminate(&mut child, pid).await;
                Ok(ExecEnd::Cancelled)
            }
        }
    }

    /// Runs a command and returns its stdout when it succeeds (for probes; not logged).
    pub async fn capture(&self, program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program)
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Runs a command, output to the log; `Ok(true)` when it exited with 0.
    pub async fn succeeds(&self, cmd: &Cmd) -> anyhow::Result<Option<bool>> {
        Ok(match self.exec(cmd).await? {
            ExecEnd::Done(code) => Some(code == 0),
            ExecEnd::Cancelled => None,
        })
    }
}

pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if is_file(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        for ext in ["exe", "cmd", "bat"] {
            let with_ext = dir.join(format!("{program}.{ext}"));
            if is_file(&with_ext) {
                return Some(with_ext);
            }
        }
    }
    None
}

fn is_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(unix)]
async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    let Some(pid) = pid else {
        let _ = child.kill().await;
        return;
    };
    let group = -(pid as i32);
    // SAFETY: plain signal to the process group this task created.
    unsafe { libc::kill(group, libc::SIGTERM) };
    let deadline = tokio::time::Instant::now() + GRACE;
    let _ = tokio::time::timeout(GRACE, child.wait()).await;
    // The leader may be gone while its children still clean up: wait for the group.
    while tokio::time::Instant::now() < deadline && group_alive(group) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // SAFETY: as above; ESRCH when the group is already gone.
    unsafe { libc::kill(group, libc::SIGKILL) };
    let _ = child.wait().await;
}

#[cfg(unix)]
fn group_alive(group: i32) -> bool {
    // SAFETY: signal 0 only probes.
    unsafe { libc::kill(group, 0) == 0 }
}

#[cfg(not(unix))]
async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    if let Some(pid) = pid {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()
            .await;
    }
    let _ = tokio::time::timeout(GRACE, child.wait()).await;
    let _ = child.kill().await;
}
