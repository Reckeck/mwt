//! Where a vault comes from: a path on a file system or a document URI.
//!
//! Desktop file dialogs hand back paths (`/home/…`, `C:\…`, `\\server\…`).
//! Android hands back `content://` URIs of the Storage Access Framework, which
//! `std::fs` cannot open; iOS hands back `file://` URLs. Everything that keeps
//! or shows a vault location goes through [`VaultLocation`], so a URI is never
//! mistaken for a path again.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VaultLocation {
    /// A file the app can open with `std::fs`.
    Path(PathBuf),
    /// A document only the platform can open (`content://…` on Android).
    Uri(String),
}

impl VaultLocation {
    /// Read what a dialog, the recents list or the UI handed over.
    ///
    /// `file://` URLs become paths; any other `scheme://` is a URI. A scheme
    /// needs two letters at least, so a Windows drive (`C:\`, `C://`) stays a path.
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        match scheme_of(raw) {
            Some(scheme) if scheme.eq_ignore_ascii_case("file") => {
                Self::Path(file_url_to_path(&raw[scheme.len() + 3..]))
            }
            Some(_) => Self::Uri(raw.to_string()),
            None => Self::Path(PathBuf::from(raw)),
        }
    }

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path),
            Self::Uri(_) => None,
        }
    }

    pub fn is_uri(&self) -> bool {
        matches!(self, Self::Uri(_))
    }

    /// File name to show the user: `vault.wallet` for `/a/b/vault.wallet`,
    /// `C:\a\vault.wallet` and `content://…/primary%3ADownload%2Fvault.wallet`.
    pub fn display_name(&self) -> String {
        match self {
            Self::Path(path) => last_segment(&path.to_string_lossy(), &['/', '\\']),
            Self::Uri(uri) => {
                let decoded = percent_decode(uri.split(['?', '#']).next().unwrap_or(uri));
                last_segment(&decoded, &['/', ':'])
            }
        }
    }
}

impl fmt::Display for VaultLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(path) => write!(formatter, "{}", path.display()),
            Self::Uri(uri) => formatter.write_str(uri),
        }
    }
}

impl From<PathBuf> for VaultLocation {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

/// File name of a path written with either separator (the recents list may
/// hold a path from another OS after a vault was synced across machines).
pub fn display_name_of(raw: &str) -> String {
    VaultLocation::parse(raw).display_name()
}

fn scheme_of(raw: &str) -> Option<&str> {
    let end = raw.find("://")?;
    let scheme = &raw[..end];
    let mut chars = scheme.chars();
    let valid = scheme.len() >= 2
        && chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|rest| rest.is_ascii_alphanumeric() || matches!(rest, '+' | '-' | '.'));
    valid.then_some(scheme)
}

/// `localhost/x`, `/home/a%20b/x`, `/C:/x` → a native path.
fn file_url_to_path(rest: &str) -> PathBuf {
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    let bytes = decoded.as_bytes();
    // `/C:/…` is a Windows drive; the leading slash belongs to the URL only.
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        return PathBuf::from(&decoded[1..]);
    }
    PathBuf::from(decoded)
}

fn last_segment(text: &str, separators: &[char]) -> String {
    let trimmed = text.trim_end_matches(separators);
    trimmed
        .rsplit(separators)
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

/// `%XX` → byte; malformed escapes are kept as they are.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = bytes
                .get(index + 1..index + 3)
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths_of_every_os_stay_paths() {
        for raw in [
            "/home/user/vault.wallet",
            r"C:\Users\me\vault.wallet",
            "C://odd/vault.wallet",
            r"\\server\share\vault.wallet",
            "relative/vault.wallet",
        ] {
            assert_eq!(
                VaultLocation::parse(raw),
                VaultLocation::Path(PathBuf::from(raw)),
                "{raw}"
            );
        }
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(
            VaultLocation::parse("file:///home/a%20b/v.wallet"),
            VaultLocation::Path(PathBuf::from("/home/a b/v.wallet"))
        );
        assert_eq!(
            VaultLocation::parse("file://localhost/tmp/v.wallet"),
            VaultLocation::Path(PathBuf::from("/tmp/v.wallet"))
        );
        assert_eq!(
            VaultLocation::parse("file:///C:/Users/v.wallet"),
            VaultLocation::Path(PathBuf::from("C:/Users/v.wallet"))
        );
    }

    #[test]
    fn content_uris_are_never_paths() {
        let raw = "content://com.android.externalstorage.documents/document/primary%3ADownload%2Fmy%20money.wallet";
        let location = VaultLocation::parse(raw);
        assert!(location.is_uri());
        assert_eq!(location.as_path(), None);
        assert_eq!(location.to_string(), raw);
        assert_eq!(location.display_name(), "my money.wallet");
    }

    #[test]
    fn display_names_handle_both_separators() {
        assert_eq!(display_name_of("/a/b/vault.wallet"), "vault.wallet");
        assert_eq!(display_name_of(r"C:\a\b\work.wallet"), "work.wallet");
        assert_eq!(display_name_of("/a/b/"), "b");
        assert_eq!(
            display_name_of("content://media/external/file/42?x=1"),
            "42"
        );
    }

    #[test]
    fn broken_escapes_do_not_panic() {
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
        assert_eq!(percent_decode("%E2%82%AC"), "€");
    }
}
