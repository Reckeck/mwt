//! The committed icon folder, put back after `tauri icon` has rewritten it.

use std::path::{Path, PathBuf};

use anyhow::Result;

pub struct Backup {
    icons: PathBuf,
    saved: PathBuf,
}

impl Backup {
    pub fn take(root: &Path) -> Result<Self> {
        let icons = root.join("crates/app/icons");
        // Next to the folder, so the way back is a rename on the same file system.
        let saved = root.join(format!(".cache/xtask/icons-backup-{}", std::process::id()));
        if let Some(parent) = saved.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::guard::copy_dir(&icons, &saved)?;
        Ok(Self { icons, saved })
    }
}

impl Drop for Backup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.icons);
        if std::fs::rename(&self.saved, &self.icons).is_err() {
            let _ = crate::guard::copy_dir(&self.saved, &self.icons);
            let _ = std::fs::remove_dir_all(&self.saved);
        }
    }
}
