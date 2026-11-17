//! The only way a command hands a link or a file to the desktop.
//!
//! The URL check is part of [`Opener::open_url`], so a command cannot
//! reach the system without it. Neither a refusal nor a failure repeats the
//! link, which may carry a key.

use std::path::Path;

use trade_core::error::Error;
use trade_core::external_url::{check_attachment_url, check_external_url};

use super::IpcError;

/// Which rule a link has to pass before it leaves the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlKind {
    /// A block explorer address: `https` only.
    Explorer,
    /// An attachment link: a web or mail address.
    Attachment,
}

/// What actually reaches the desktop. Private: a command can only go through
/// [`Opener`], which runs the URL check first.
trait Launcher: Send + Sync {
    /// Hand an already checked link to the system.
    fn launch_url(&self, url: &str) -> Result<(), IpcError>;

    /// Hand a file to the system viewer.
    fn open_path(&self, path: &Path) -> Result<(), IpcError>;

    /// Show a file in the system file manager.
    fn reveal_path(&self, path: &Path) -> Result<(), IpcError>;
}

/// The port to the desktop: links are checked by their kind, then opened.
pub struct Opener(Box<dyn Launcher>);

impl Opener {
    /// Opens through the desktop's default programs.
    pub fn system() -> Self {
        Self(Box::new(SystemLauncher))
    }

    /// The Tauri plugin that backs [`Opener::system`]; the one place it is named.
    pub fn plugin<R: tauri::Runtime>() -> impl tauri::plugin::Plugin<R> {
        tauri_plugin_opener::init()
    }

    /// Check the link by its kind, then open it.
    pub fn open_url(&self, url: &str, kind: UrlKind) -> Result<(), IpcError> {
        match kind {
            UrlKind::Explorer => check_external_url(url)?,
            UrlKind::Attachment => check_attachment_url(url)?,
        }
        self.0.launch_url(url)
    }

    /// Hand a file to the system viewer.
    pub fn open_path(&self, path: &Path) -> Result<(), IpcError> {
        self.0.open_path(path)
    }

    /// Show a file in the system file manager (desktop).
    pub fn reveal_path(&self, path: &Path) -> Result<(), IpcError> {
        self.0.reveal_path(path)
    }
}

struct SystemLauncher;

impl Launcher for SystemLauncher {
    fn launch_url(&self, url: &str) -> Result<(), IpcError> {
        tauri_plugin_opener::open_url(url, None::<&str>)
            .map_err(|_| IpcError::from("failed to open the link"))
    }

    fn open_path(&self, path: &Path) -> Result<(), IpcError> {
        tauri_plugin_opener::open_path(path, None::<&str>)
            .map_err(|_| IpcError::from("failed to open the file"))
    }

    #[cfg(desktop)]
    fn reveal_path(&self, path: &Path) -> Result<(), IpcError> {
        tauri_plugin_opener::reveal_item_in_dir(path)
            .map_err(|err| IpcError::from(Error::Io(err.to_string())))
    }

    #[cfg(mobile)]
    fn reveal_path(&self, _path: &Path) -> Result<(), IpcError> {
        Err(IpcError::from(Error::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct RecordingLauncher {
        urls: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }

    impl Launcher for RecordingLauncher {
        fn launch_url(&self, url: &str) -> Result<(), IpcError> {
            self.urls.lock().unwrap().push(url.to_owned());
            if self.fail {
                return Err("failed to open the link".into());
            }
            Ok(())
        }

        fn open_path(&self, _path: &Path) -> Result<(), IpcError> {
            Ok(())
        }

        fn reveal_path(&self, _path: &Path) -> Result<(), IpcError> {
            Ok(())
        }
    }

    /// An opener on a recording launcher, and the links that reached it.
    fn recording(fail: bool) -> (Opener, Arc<Mutex<Vec<String>>>) {
        let urls = Arc::new(Mutex::new(Vec::new()));
        let launcher = RecordingLauncher {
            urls: Arc::clone(&urls),
            fail,
        };
        (Opener(Box::new(launcher)), urls)
    }

    #[test]
    fn the_explorer_check_runs_before_the_launcher() {
        let (opener, seen) = recording(false);
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "steam://run/1",
            "mailto:a@b.c",
        ] {
            assert!(opener.open_url(url, UrlKind::Explorer).is_err(), "{url}");
        }
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn a_valid_link_reaches_the_launcher_once() {
        let (opener, seen) = recording(false);
        opener
            .open_url("https://etherscan.io/tx/0x1", UrlKind::Explorer)
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), ["https://etherscan.io/tx/0x1"]);
    }

    #[test]
    fn an_attachment_link_uses_the_attachment_rule() {
        let (opener, seen) = recording(false);
        opener
            .open_url("mailto:a@example.com", UrlKind::Attachment)
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), ["mailto:a@example.com"]);
        assert!(
            opener
                .open_url("file:///etc/passwd", UrlKind::Attachment)
                .is_err()
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_failed_open_does_not_repeat_the_url() {
        let (opener, seen) = recording(true);
        let url = "https://example.com/?key=secret";
        let error = opener.open_url(url, UrlKind::Explorer).unwrap_err();
        assert_eq!(error.to_string(), "failed to open the link");
        assert!(!error.to_string().contains("secret"));
        assert_eq!(seen.lock().unwrap().len(), 1);
    }
}
