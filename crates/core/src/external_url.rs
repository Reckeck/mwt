//! The one rule for a link the app hands to the operating system's browser.
//!
//! A link can come from a long way off: an explorer address stored in the vault
//! and synchronised from another device, a release address from the update
//! manifest, a template a user typed. `open_url` would hand any of them to the
//! desktop, where `file:` reads a file and a scheme registered by another
//! program starts that program. So the string is checked here, in one place,
//! instead of at each call site, and the check is a whitelist: https, a plain
//! host, nothing that could carry a second meaning.
use crate::{Error, Result};

/// Refuse anything but a plain `https://` link to a named host.
///
/// The error never contains the link: it travels to a toast and to a log, and a
/// link can hold a token in its query.
pub fn check_external_url(url: &str) -> Result<()> {
    const REFUSED: &str = "only an https link can be opened";

    if url.len() > 2048 {
        return Err(Error::InvalidInput(REFUSED.into()));
    }
    // A control character or a space can split the string for whoever reads it
    // next, so the whole link must be printable ASCII without blanks.
    if url
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control() || !ch.is_ascii())
    {
        return Err(Error::InvalidInput(REFUSED.into()));
    }
    // The scheme is compared case-insensitively, because `HtTpS:` opens too.
    let rest = url
        .get(..8)
        .filter(|head| head.eq_ignore_ascii_case("https://"))
        .map(|_| &url[8..])
        .ok_or_else(|| Error::InvalidInput(REFUSED.into()))?;
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    // `user:password@host` and `host:port` are both refused: credentials in a
    // link are a leak, and a port is not needed to open a public page.
    if host.is_empty() || host.contains('@') || host.contains(':') {
        return Err(Error::InvalidInput(REFUSED.into()));
    }
    // A host with no dot is a local name, and a local name is not a public page.
    if !host.contains('.')
        || host.starts_with('.')
        || host.ends_with('.')
        || host.contains("..")
        || !host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-')
    {
        return Err(Error::InvalidInput(REFUSED.into()));
    }
    // The last label must be a real suffix, which rules out an address literal:
    // `127.0.0.1` is all digits and dots and would otherwise pass as a host.
    let suffix = host.rsplit('.').next().unwrap_or_default();
    if suffix.len() < 2 || !suffix.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(Error::InvalidInput(REFUSED.into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_external_url;

    #[test]
    fn a_plain_https_page_is_opened() {
        for url in [
            "https://etherscan.io/tx/0xabc",
            "https://eth.blockscout.com/address/0xAbC?tab=tokens",
            "https://github.com/owner/repo/releases/download/v1.2.3/app.apk",
            "https://www.bybit.com/",
            "HTTPS://etherscan.io/tx/0xabc",
        ] {
            assert!(check_external_url(url).is_ok(), "refused {url}");
        }
    }

    #[test]
    fn everything_that_is_not_a_public_https_page_is_refused() {
        for url in [
            "",
            "http://etherscan.io/tx/0xabc",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "ftp://example.com/x",
            "steam://run/12345",
            "https://",
            "https://localhost/x",
            "https://127.0.0.1/x",
            "https://user:pass@etherscan.io/tx/0xabc",
            "https://etherscan.io:8080/tx/0xabc",
            "https://etherscan.io\n/tx",
            "https://etherscan.io /tx",
            "https://еtherscan.io/tx",
            "https://.io/x",
            "https://etherscan..io/x",
        ] {
            assert!(check_external_url(url).is_err(), "admitted {url}");
        }
    }

    #[test]
    fn the_error_never_carries_the_link() {
        let secret = "https://explorer.example.com/x?apikey=proapi_do_not_log";
        let message = check_external_url(&format!("http{secret}"))
            .unwrap_err()
            .to_string();
        assert!(
            !message.contains("proapi_do_not_log"),
            "the error echoed the link: {message}"
        );
    }
}
