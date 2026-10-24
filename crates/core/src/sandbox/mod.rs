//! The sandbox: a place to try the app without touching real data.
//!
//! It is a separate, temporary vault — not a mode on top of the real one —
//! kept in `<app cache dir>/sandbox/`, filled with demo data and erased when
//! the sandbox is closed, when the app quits and when it next starts.
//!
//! **Without a password.** The file is sealed with the open variant of the
//! container (`SealKey::plain`, see `crypto`): the demo data is public by
//! nature, an in-memory one-time key would only cost an Argon2 run on every
//! open and reset, and a lost key would leave an unreadable file behind for
//! the start-up erase to cope with. The erase is what keeps the folder clean.
//!
//! **Fenced in.** Every path the sandbox deletes or opens is checked against
//! its folder (`SandboxDir::contains`): canonical, and never a symbolic link.
//!
//! Not to be confused with `UnlockedVault::sandbox`, an in-memory dry-run
//! copy that never writes: that one is for emulating automations.

mod seed;
#[cfg(test)]
mod tests;

use std::path::{Component, Path, PathBuf};

use chrono::NaiveDate;

use crate::service::UnlockedVault;
use crate::{Error, Result};

/// The folder's name inside the app cache directory.
pub const DIR_NAME: &str = "sandbox";
/// The vault file inside it.
pub const FILE_NAME: &str = "sandbox.wallet";

/// The sandbox folder and what may be done with it.
#[derive(Debug, Clone)]
pub struct SandboxDir {
    root: PathBuf,
}

fn refuse(reason: &str) -> Error {
    Error::Sandbox(reason.to_string())
}

impl SandboxDir {
    /// `<cache_dir>/sandbox`. The folder is the only thing this type touches.
    pub fn in_cache(cache_dir: &Path) -> Self {
        Self {
            root: cache_dir.join(DIR_NAME),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The vault file's path.
    pub fn file(&self) -> PathBuf {
        self.root.join(FILE_NAME)
    }

    /// Remove the folder and everything in it. A symbolic link in its place
    /// is removed as a link: what it points to is never touched. Nothing
    /// there is fine.
    pub fn erase(&self) -> Result<()> {
        if self.root.file_name().and_then(|name| name.to_str()) != Some(DIR_NAME) {
            return Err(refuse("not the sandbox folder"));
        }
        let kind = match std::fs::symlink_metadata(&self.root) {
            Ok(meta) => meta.file_type(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        if kind.is_dir() {
            // Links inside are removed as links, never followed.
            std::fs::remove_dir_all(&self.root)?;
        } else {
            std::fs::remove_file(&self.root)?;
        }
        Ok(())
    }

    /// A fresh sandbox vault with the demo data up to `today`: what was left
    /// of an earlier one is erased first. The vault is open and saved.
    pub fn create(&self, today: NaiveDate) -> Result<UnlockedVault> {
        self.erase()?;
        std::fs::create_dir_all(&self.root)?;
        let file = self.contains(&self.file())?;
        let mut vault = UnlockedVault::create_with(file, None)?;
        seed::fill(&mut vault, today)?;
        Ok(vault)
    }

    /// `path` as a canonical path inside the folder, or a refusal: outside it,
    /// the folder itself, reaching out through `..` or through a symbolic link.
    pub fn contains(&self, path: &Path) -> Result<PathBuf> {
        if path.components().any(|part| part == Component::ParentDir) {
            return Err(refuse("a path with .. is not allowed"));
        }
        let root = std::fs::canonicalize(&self.root).map_err(|_| refuse("no sandbox folder"))?;
        let canonical = match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(refuse("a symbolic link is not allowed"));
            }
            Ok(_) => std::fs::canonicalize(path)?,
            // A file to be created: its folder must be inside.
            Err(_) => {
                let parent = path.parent().ok_or_else(|| refuse("no folder"))?;
                let name = path.file_name().ok_or_else(|| refuse("no file name"))?;
                std::fs::canonicalize(parent)?.join(name)
            }
        };
        if canonical.starts_with(&root) && canonical != root {
            Ok(canonical)
        } else {
            Err(refuse("the path is outside the sandbox folder"))
        }
    }
}
