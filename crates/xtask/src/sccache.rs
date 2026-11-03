//! A machine-wide sccache for the host cargo tasks and the build containers, so a
//! second working copy takes compiled crates from the cache instead of building them.
//!
//! Host and containers get separate directories: their keys never match (another
//! rustc, other paths), and two servers on one directory would evict each other.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::exec::which;
use crate::sink::Sink;

const HOST_SIZE: &str = "16G";
const CONTAINER_SIZE: &str = "4G";
/// Where the containers directory is mounted inside.
const CONTAINER_DIR: &str = "/sccache";
/// The containers share the host network: on the default port 4226 they would talk to the host's
/// server (which cannot see their rustc) and hang.
pub const CONTAINER_PORT: u16 = 4227;

/// Env for the cargo tasks of this run; set once by `prepare`.
static HOST_ENV: OnceLock<Vec<(String, String)>> = OnceLock::new();

/// What the environment looks like, as plain values (testable without touching the process env).
pub struct Inputs {
    /// `XTASK_SCCACHE=0`.
    pub disabled: bool,
    pub installed: bool,
    pub home: Option<PathBuf>,
    /// Names of the variables the user already set.
    pub set: HashSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Setup {
    Off,
    /// Not installed: work as before and say how to get it.
    Missing,
    On(Vec<(String, String)>),
}

pub fn host_dir(home: &Path) -> PathBuf {
    home.join(".cache/sccache/host")
}

pub fn containers_dir(home: &Path) -> PathBuf {
    home.join(".cache/sccache/containers")
}

/// Variables to add for host cargo; the ones the user already set are left alone.
pub fn plan(inputs: &Inputs) -> Setup {
    if inputs.disabled {
        return Setup::Off;
    }
    if !inputs.installed {
        return Setup::Missing;
    }
    let mut env = Vec::new();
    let mut add = |name: &str, value: String| {
        if !inputs.set.contains(name) {
            env.push((name.to_string(), value));
        }
    };
    add("RUSTC_WRAPPER", "sccache".to_string());
    if let Some(home) = &inputs.home {
        add("SCCACHE_DIR", host_dir(home).display().to_string());
    }
    add("SCCACHE_CACHE_SIZE", HOST_SIZE.to_string());
    Setup::On(env)
}

/// Docker/podman arguments that share the containers cache; empty when disabled.
pub fn container_args(disabled: bool, home: Option<&Path>, port: u16) -> Vec<String> {
    let Some(home) = home.filter(|_| !disabled) else {
        return Vec::new();
    };
    vec![
        "-v".into(),
        format!("{}:{CONTAINER_DIR}", containers_dir(home).display()),
        "-e".into(),
        format!("SCCACHE_DIR={CONTAINER_DIR}"),
        "-e".into(),
        format!("SCCACHE_CACHE_SIZE={CONTAINER_SIZE}"),
        "-e".into(),
        format!("SCCACHE_SERVER_PORT={port}"),
    ]
}

pub fn disabled() -> bool {
    std::env::var("XTASK_SCCACHE").is_ok_and(|value| value == "0")
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Mount arguments for a container run; creates the directory first (owned by the user, not root).
pub fn container_mount() -> Vec<String> {
    container_mount_on(CONTAINER_PORT)
}

/// As `container_mount`, on the server port of one build lane: containers building side by side
/// share the host network, and one server for all of them dies with the container that started it.
pub fn container_mount_on(port: u16) -> Vec<String> {
    let home = home();
    let args = container_args(disabled(), home.as_deref(), port);
    if let Some(home) = home.filter(|_| !args.is_empty()) {
        let _ = std::fs::create_dir_all(containers_dir(&home));
    }
    args
}

/// Decides, starts the server and remembers the env for `host_env`. Call once before the graph.
pub fn prepare(sink: &Sink) {
    let inputs = Inputs {
        disabled: disabled(),
        installed: which("sccache").is_some(),
        home: home(),
        set: ["RUSTC_WRAPPER", "SCCACHE_DIR", "SCCACHE_CACHE_SIZE"]
            .into_iter()
            .filter(|name| std::env::var_os(name).is_some())
            .map(str::to_string)
            .collect(),
    };
    match plan(&inputs) {
        Setup::Off => {}
        Setup::Missing => sink.line("sccache not found: building without it (mise use -g sccache)"),
        Setup::On(env) => {
            let dir = env
                .iter()
                .find(|(name, _)| name == "SCCACHE_DIR")
                .map(|(_, value)| value.clone())
                .or_else(|| std::env::var("SCCACHE_DIR").ok());
            if let Some(dir) = dir {
                let _ = std::fs::create_dir_all(dir);
            }
            // One server before the parallel tasks, or each of them races to start it.
            let _ = Command::new("sccache")
                .arg("--start-server")
                .envs(env.iter().map(|(key, value)| (key, value)))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = HOST_ENV.set(env);
        }
    }
}

/// Env to add to a command: only for `cargo`, and only what the command does not set itself.
pub fn host_env(program: &str, own: &[(String, String)]) -> Vec<(String, String)> {
    if program != "cargo" {
        return Vec::new();
    }
    HOST_ENV
        .get()
        .map(|env| {
            env.iter()
                .filter(|(name, _)| !own.iter().any(|(key, _)| key == name))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// What `sccache --show-stats` says (only when this run used sccache).
pub fn stats_text() -> Option<String> {
    let env = HOST_ENV.get()?;
    let mut command = Command::new("sccache");
    command
        .arg("--show-stats")
        .envs(env.iter().map(|(key, value)| (key, value)));
    let output = command.stderr(Stdio::null()).output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The counters of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub requests: u64,
    pub hits: u64,
    pub misses: u64,
}

impl Stats {
    /// Hits among the lookups that were answered, in percent.
    pub fn hit_rate(&self) -> Option<u64> {
        let answered = self.hits + self.misses;
        (answered > 0).then(|| self.hits * 100 / answered)
    }
}

pub fn parse_stats(text: &str) -> Option<Stats> {
    let number = |label| count(text, label)?.parse().ok();
    Some(Stats {
        requests: number("Compile requests")?,
        hits: number("Cache hits")?,
        misses: number("Cache misses")?,
    })
}

fn count(text: &str, label: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(label)?;
        Some(rest.trim().to_string())
    })
}

pub fn format_stats(text: &str) -> String {
    let get = |label| count(text, label).unwrap_or_else(|| "?".into());
    format!(
        "sccache: {} requests, {} hits, {} misses",
        get("Compile requests"),
        get("Cache hits"),
        get("Cache misses")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(set: &[&str]) -> Inputs {
        Inputs {
            disabled: false,
            installed: true,
            home: Some(PathBuf::from("/home/wd")),
            set: set.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    fn names(setup: Setup) -> Vec<(String, String)> {
        match setup {
            Setup::On(env) => env,
            other => panic!("expected On, got {other:?}"),
        }
    }

    #[test]
    fn defaults_with_sccache() {
        let env = names(plan(&inputs(&[])));
        assert_eq!(
            env,
            vec![
                ("RUSTC_WRAPPER".into(), "sccache".into()),
                ("SCCACHE_DIR".into(), "/home/wd/.cache/sccache/host".into()),
                ("SCCACHE_CACHE_SIZE".into(), "16G".into()),
            ]
        );
    }

    #[test]
    fn user_values_win() {
        let env = names(plan(&inputs(&["RUSTC_WRAPPER", "SCCACHE_DIR"])));
        assert_eq!(env, vec![("SCCACHE_CACHE_SIZE".into(), "16G".into())]);
    }

    #[test]
    fn missing_and_disabled() {
        let mut missing = inputs(&[]);
        missing.installed = false;
        assert_eq!(plan(&missing), Setup::Missing);
        let mut off = inputs(&[]);
        off.disabled = true;
        assert_eq!(plan(&off), Setup::Off);
    }

    #[test]
    fn container_mount_args() {
        let home = PathBuf::from("/home/wd");
        let args = container_args(false, Some(&home), CONTAINER_PORT);
        assert_eq!(args[1], "/home/wd/.cache/sccache/containers:/sccache");
        assert!(args.contains(&"SCCACHE_DIR=/sccache".to_string()));
        assert!(args.contains(&"SCCACHE_CACHE_SIZE=4G".to_string()));
        assert!(args.contains(&"SCCACHE_SERVER_PORT=4227".to_string()));
        assert!(container_args(true, Some(&home), CONTAINER_PORT).is_empty());
        assert!(container_args(false, None, CONTAINER_PORT).is_empty());
    }

    #[test]
    fn stats_line_reads_counters() {
        let text = "Compile requests                    12\nCache hits                           7\nCache misses                         5\n";
        assert_eq!(format_stats(text), "sccache: 12 requests, 7 hits, 5 misses");
    }

    #[test]
    fn stats_numbers() {
        let text = "Compile requests  12\nCache hits  9\nCache misses  3\n";
        let stats = parse_stats(text).unwrap();
        assert_eq!((stats.requests, stats.hits, stats.misses), (12, 9, 3));
        assert_eq!(stats.hit_rate(), Some(75));
        assert!(parse_stats("nothing").is_none());
    }

    #[test]
    fn host_env_only_for_cargo() {
        assert!(host_env("pnpm", &[]).is_empty());
    }
}
