//! The only way a command hands a link or a file to the desktop.
//!
//! The URL check is part of [`ExternalOpener::open_url`], so a command cannot
//! reach the system without it. Neither a refusal nor a failure repeats the
//! link, which may carry a key.

use std::path::Path;

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

pub trait ExternalOpener: Send + Sync {
    /// Hand an already checked link to the system.
    fn launch_url(&self, url: &str) -> Result<(), IpcError>;

    /// Hand a file to the system viewer.
    fn open_path(&self, path: &Path) -> Result<(), IpcError>;

    /// Check the link by its kind, then open it.
    fn open_url(&self, url: &str, kind: UrlKind) -> Result<(), IpcError> {
        match kind {
            UrlKind::Explorer => check_external_url(url)?,
            UrlKind::Attachment => check_attachment_url(url)?,
        }
        self.launch_url(url)
    }
}

/// Opens through the desktop's default programs.
pub struct SystemOpener;

impl ExternalOpener for SystemOpener {
    fn launch_url(&self, url: &str) -> Result<(), IpcError> {
        tauri_plugin_opener::open_url(url, None::<&str>)
            .map_err(|_| IpcError::from("failed to open the link"))
    }

    fn open_path(&self, path: &Path) -> Result<(), IpcError> {
        tauri_plugin_opener::open_path(path, None::<&str>)
            .map_err(|_| IpcError::from("failed to open the file"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct RecordingOpener {
        urls: Mutex<Vec<String>>,
        fail: bool,
    }

    impl ExternalOpener for RecordingOpener {
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
    }

    impl RecordingOpener {
        fn seen(&self) -> Vec<String> {
            self.urls.lock().unwrap().clone()
        }
    }

    #[test]
    fn the_explorer_check_runs_before_the_opener() {
        let opener = RecordingOpener::default();
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "steam://run/1",
            "mailto:a@b.c",
        ] {
            assert!(opener.open_url(url, UrlKind::Explorer).is_err(), "{url}");
        }
        assert!(opener.seen().is_empty());
    }

    #[test]
    fn a_valid_link_reaches_the_opener_once() {
        let opener = RecordingOpener::default();
        opener
            .open_url("https://etherscan.io/tx/0x1", UrlKind::Explorer)
            .unwrap();
        assert_eq!(opener.seen(), ["https://etherscan.io/tx/0x1"]);
    }

    #[test]
    fn an_attachment_link_uses_the_attachment_rule() {
        let opener = RecordingOpener::default();
        opener
            .open_url("mailto:a@example.com", UrlKind::Attachment)
            .unwrap();
        assert_eq!(opener.seen(), ["mailto:a@example.com"]);
        assert!(
            opener
                .open_url("file:///etc/passwd", UrlKind::Attachment)
                .is_err()
        );
        assert_eq!(opener.seen().len(), 1);
    }

    #[test]
    fn a_failed_open_does_not_repeat_the_url() {
        let opener = RecordingOpener {
            fail: true,
            ..Default::default()
        };
        let url = "https://example.com/?key=secret";
        let error = opener.open_url(url, UrlKind::Explorer).unwrap_err();
        assert_eq!(error.to_string(), "failed to open the link");
        assert!(!error.to_string().contains("secret"));
        assert_eq!(opener.seen().len(), 1);
    }
}
