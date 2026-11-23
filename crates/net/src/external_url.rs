//! The one rule for a link the app hands to the operating system's browser.
//!
//! A link can come from a long way off: an explorer address stored in the vault
//! and synchronised from another device, a release address from the update
//! manifest, a template a user typed. `open_url` would hand any of them to the
//! desktop, where `file:` reads a file and a scheme registered by another
//! program starts that program. So the string is checked here, in one place,
//! instead of at each call site, and the check is a whitelist: https, a plain
//! host, nothing that could carry a second meaning.
use mwt_base::{Error, Result};

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

/// Refuse a link of an attachment unless it is a web or mail address, the
/// two kinds `Session::add_link_attachment` admits.
///
/// It is the same rule with a wider whitelist, not a second one: what the user
/// typed into an attachment may be `http://`, carry a port or an address
/// literal (a home server) or a non-ASCII host, which [`check_external_url`]
/// refuses for the app's own links, and `mailto:`. What it keeps is the part
/// that matters at the click: the scheme is `http`, `https` or `mailto` and
/// nothing else, the link is one printable token, and something follows the
/// scheme. The attachments collection syncs and is edited in the JSON editor
/// without being checked again, so this runs at the click, not only at the
/// creation. The error never contains the link.
pub fn check_attachment_url(url: &str) -> Result<()> {
    const REFUSED: &str = "only a web or mail link can be opened";

    if url.len() > 2048 || url.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return Err(Error::InvalidInput(REFUSED.into()));
    }
    let after = |scheme: &str| {
        url.get(..scheme.len())
            .filter(|head| head.eq_ignore_ascii_case(scheme))
            .map(|_| &url[scheme.len()..])
    };
    if let Some(rest) = after("https://").or_else(|| after("http://")) {
        // A host must be there: `https:///etc/passwd` has none.
        let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if !host.is_empty() {
            return Ok(());
        }
    } else if let Some(address) = after("mailto:")
        && !address.is_empty()
    {
        return Ok(());
    }
    Err(Error::InvalidInput(REFUSED.into()))
}

#[cfg(test)]
mod tests {
    use super::{check_attachment_url, check_external_url};

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

    // -- attachment links ---------------------------------------------------

    /// Every form `Session::add_link_attachment` admits (web and mail
    /// addresses, normalized or kept as typed) still opens.
    #[test]
    fn what_an_attachment_link_may_hold_still_opens() {
        for url in [
            "https://example.com/x",
            "HTTPS://Example.com/x?a=1#b",
            "http://example.com/x",
            "http://192.168.1.5:8080/share",
            "https://nas.local:5001/file",
            "https://пример.рф/страница",
            "mailto:me@example.com",
            "MAILTO:me@example.com?subject=Receipt%20123",
        ] {
            assert!(check_attachment_url(url).is_ok(), "refused {url}");
        }
    }

    /// A record from a paired device or the JSON editor is not checked when
    /// it arrives, so the check at the click is the one that counts.
    #[test]
    fn an_attachment_link_with_another_scheme_is_refused() {
        for url in [
            "",
            "file:///etc/passwd",
            "FILE:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "ftp://example.com/x",
            "steam://run/12345",
            "smb://host/share",
            "tel:+10000000000",
            "//example.com/x",
            "example.com/x",
            "https:/example.com",
            "https://",
            "http://",
            "mailto:",
            "https://example.com/a b",
            "https://example.com/\n",
            "https://example.com/\u{0}x",
            "https:///etc/passwd",
        ] {
            assert!(check_attachment_url(url).is_err(), "admitted {url:?}");
        }
        let long = format!("https://example.com/{}", "a".repeat(2100));
        assert!(check_attachment_url(&long).is_err());
    }

    #[test]
    fn the_attachment_refusal_never_carries_the_link() {
        let secret = "file:///home/me/proapi_do_not_log";
        let message = check_attachment_url(secret).unwrap_err().to_string();
        assert!(
            !message.contains("proapi_do_not_log"),
            "the error echoed the link: {message}"
        );
    }
}
