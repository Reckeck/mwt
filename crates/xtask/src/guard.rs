//! Builds regenerate tauri's committed schemas (`crates/app/gen/schemas`): the
//! snapshot is put back after the run, so a check leaves no diff behind.

use std::io;
use std::path::{Path, PathBuf};

pub struct SchemasGuard {
    dir: PathBuf,
    snapshot: Option<PathBuf>,
}

impl SchemasGuard {
    pub fn take(root: &Path) -> Self {
        let dir = root.join("crates/app/gen/schemas");
        let snapshot = root.join(format!(".cache/xtask/schemas-{}", std::process::id()));
        let kept = dir.is_dir() && copy_dir(&dir, &snapshot).is_ok();
        Self {
            dir,
            snapshot: kept.then_some(snapshot),
        }
    }

    pub fn restore(self) {
        let Some(snapshot) = self.snapshot else {
            return;
        };
        let _ = std::fs::remove_dir_all(&self.dir);
        if copy_dir(&snapshot, &self.dir).is_err() {
            eprintln!(
                "xtask: could not restore {}; snapshot kept in {}",
                self.dir.display(),
                snapshot.display()
            );
            return;
        }
        let _ = std::fs::remove_dir_all(&snapshot);
    }
}

pub fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
