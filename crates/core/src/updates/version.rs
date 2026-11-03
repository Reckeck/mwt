//! Application version "X.Y.Z" or "X.Y.Z-N" (N = pre-release build number).

use std::cmp::Ordering;

use crate::error::{Error, Result};

/// A parsed app version. A pre-release (`X.Y.Z-N`) precedes the release `X.Y.Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<u64>,
}

impl AppVersion {
    pub fn parse(text: &str) -> Result<Self> {
        let bad = || Error::InvalidInput(format!("bad version: {text}"));
        let body = text.trim();
        let body = body.strip_prefix('v').unwrap_or(body);
        let (core, pre) = match body.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (body, None),
        };
        let mut parts = core.split('.');
        let mut next =
            || -> Result<u64> { parse_number(parts.next().ok_or_else(bad)?).ok_or_else(bad) };
        let (major, minor, patch) = (next()?, next()?, next()?);
        if parts.next().is_some() {
            return Err(bad());
        }
        let pre = match pre {
            Some(raw) => Some(parse_number(raw).ok_or_else(bad)?),
            None => None,
        };
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }
}

fn parse_number(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok()
}

impl Ord for AppVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre, other.pre) {
                (None, None) => Ordering::Equal,
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(left), Some(right)) => left.cmp(&right),
            })
    }
}

impl PartialOrd for AppVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for AppVersion {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = self.pre {
            write!(out, "-{pre}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ver(text: &str) -> AppVersion {
        AppVersion::parse(text).expect("version")
    }

    #[test]
    fn parses_release_prerelease_and_v_prefix() {
        let release = ver("1.2.3");
        assert_eq!(
            (release.major, release.minor, release.patch, release.pre),
            (1, 2, 3, None)
        );
        assert_eq!(ver("v1.2.3-7").pre, Some(7));
        assert_eq!(ver("1.2.3-7").to_string(), "1.2.3-7");
    }

    #[test]
    fn rejects_malformed() {
        for text in [
            "",
            "1.2",
            "1.2.3.4",
            "a.b.c",
            "1.2.3-",
            "1.2.3-rc1",
            "1.2.-3",
            "1..3",
            "1.2.3-1-2",
        ] {
            assert!(
                matches!(AppVersion::parse(text), Err(Error::InvalidInput(_))),
                "{text}"
            );
        }
    }

    #[test]
    fn orders_prerelease_before_release() {
        assert!(ver("1.2.3-5") < ver("1.2.3"));
        assert!(ver("1.2.3-2") < ver("1.2.3-10"));
        assert!(ver("1.2.0") < ver("1.2.1-1"));
        assert!(ver("1.10.0") > ver("1.9.9"));
        assert_eq!(ver("1.2.3"), ver("v1.2.3"));
    }
}
