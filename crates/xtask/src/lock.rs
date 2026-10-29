//! One xtask run per working copy. The OS lock `.cache/xtask/run.lock` goes
//! away with the process, so a killed run never blocks the next one. A second
//! run of the same command attaches to the first (follows `run.events`); a
//! different command waits for it.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::sink::Sink;

#[derive(Serialize, Deserialize, Clone)]
struct RunInfo {
    pid: u32,
    cmdline: String,
    started: String,
}

pub enum Role {
    /// This process runs; keep `RunGuard` until the end.
    Owner(RunGuard),
    /// Another run of the same command was followed to its end: exit with its code.
    Attached(i32),
}

pub struct RunGuard {
    _lock: File,
    pub sink: Arc<Sink>,
}

/// The working copy root: up from the current directory to a `Cargo.toml` with `[workspace]`.
pub fn find_root() -> Result<PathBuf> {
    let start = std::env::current_dir()?;
    for dir in start.ancestors() {
        if let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml"))
            && text.lines().any(|line| line.trim() == "[workspace]")
        {
            return Ok(dir.to_path_buf());
        }
    }
    anyhow::bail!("no Cargo.toml with [workspace] above {}", start.display())
}

pub async fn enter(root: &Path, cmdline: &str) -> Result<Role> {
    let dir = root.join(".cache/xtask");
    std::fs::create_dir_all(&dir)?;
    let lock_path = dir.join("run.lock");
    let mut announced = false;
    loop {
        let lock = open_lock(&lock_path)?;
        match lock.try_lock() {
            Ok(()) => return own(&dir, lock, cmdline),
            Err(TryLockError::Error(error)) => return Err(error).context("run.lock"),
            Err(TryLockError::WouldBlock) => {}
        }
        // The owner has just locked and may still be rewriting run.json.
        tokio::time::sleep(Duration::from_millis(250)).await;
        let info = read_info(&dir).await;
        match info {
            Some(info) if info.cmdline == cmdline => {
                return Ok(Role::Attached(attach(&dir, &info).await?));
            }
            other if !announced => {
                announced = true;
                match other {
                    Some(info) => println!(
                        "waiting for `xtask {}` (pid {}) to finish",
                        info.cmdline, info.pid
                    ),
                    None => println!("waiting for another xtask run to finish"),
                }
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn open_lock(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(path)?)
}

fn own(dir: &Path, lock: File, cmdline: &str) -> Result<Role> {
    let _ = std::fs::remove_file(dir.join("run.json"));
    let events = File::create(dir.join("run.events"))?;
    let info = RunInfo {
        pid: std::process::id(),
        cmdline: cmdline.to_string(),
        started: chrono::Local::now().format("%H:%M:%S").to_string(),
    };
    let tmp = dir.join("run.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(&info)?)?;
    std::fs::rename(&tmp, dir.join("run.json"))?;
    Ok(Role::Owner(RunGuard {
        _lock: lock,
        sink: Arc::new(Sink::new(Some(events))),
    }))
}

async fn read_info(dir: &Path) -> Option<RunInfo> {
    for _ in 0..20 {
        if let Ok(bytes) = std::fs::read(dir.join("run.json"))
            && let Ok(info) = serde_json::from_slice(&bytes)
        {
            return Some(info);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

/// Prints what the first run has written so far and follows it. Ctrl+C only detaches.
async fn attach(dir: &Path, info: &RunInfo) -> Result<i32> {
    println!(
        "attaching to the run of pid {}, started at {}: xtask {}",
        info.pid, info.started, info.cmdline
    );
    let events = dir.join("run.events");
    let lock_path = dir.join("run.lock");
    let mut offset = 0_u64;
    loop {
        if let Some(code) = drain(&events, &mut offset) {
            return Ok(code);
        }
        let free = open_lock(&lock_path)?.try_lock().is_ok();
        if free {
            // The owner is gone: take what it wrote just before it went.
            if let Some(code) = drain(&events, &mut offset) {
                return Ok(code);
            }
            println!(
                "the run of pid {} ended without a result (killed?)",
                info.pid
            );
            return Ok(1);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Prints the complete new lines; returns the exit code once the `EXIT` record is seen.
fn drain(events: &Path, offset: &mut u64) -> Option<i32> {
    let mut file = File::open(events).ok()?;
    file.seek(SeekFrom::Start(*offset)).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    let complete = text.rfind('\n').map_or(0, |index| index + 1);
    *offset += complete as u64;
    for line in text[..complete].lines() {
        if let Some(code) = line.strip_prefix("EXIT ") {
            return code.trim().parse().ok();
        }
        println!("{line}");
    }
    None
}
