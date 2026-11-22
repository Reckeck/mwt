//! Links from a stored [`TrackingRef`] to a page the user can open.
//!
//! The stored value stays the source reference (chain, hash, exec id); the
//! URL is built when it is shown, so a changed explorer setting applies to
//! every old record. Only `https` links are ever produced: a custom explorer
//! that is not `https` is ignored (the preset is used), never opened.

use std::collections::HashMap;

use crate::chains::evm_chain;
use mwt_base::error::{Error, Result};
use mwt_model::Chain;
use mwt_model::TrackingRef;

/// The user's own block explorers, one base URL per network (EIP-3091 paths:
/// `<base>/tx/<hash>`, `<base>/address/<address>`). The stored form is the
/// plain string the user typed; [`normalize_explorer`] checks it on save and
/// again on use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerSettings {
    pub custom: HashMap<Chain, String>,
}

impl ExplorerSettings {
    /// Set the explorer of a network; a refused URL changes nothing.
    pub fn set(&mut self, chain: Chain, url: &str) -> Result<()> {
        let base = normalize_explorer(url)?;
        self.custom.insert(chain, base);
        Ok(())
    }

    fn base(&self, chain: Chain) -> Option<String> {
        // Re-checked here: a value synced from another device or edited in
        // JSON must not become a link just because it is stored.
        self.custom
            .get(&chain)
            .and_then(|url| normalize_explorer(url).ok())
    }
}

/// A link to show, with an optional note shown next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingLink {
    pub url: String,
    /// For an exchange fill: the `exec_id` to look for on the page.
    pub note: Option<String>,
}

/// Accept a custom explorer base URL: `https` only, a real host, no
/// credentials, query or fragment, no whitespace or control characters.
/// Returns it without a trailing slash.
pub fn normalize_explorer(raw: &str) -> Result<String> {
    let text = raw.trim();
    let reject = |why: &str| Err(Error::InvalidInput(format!("explorer URL: {why}")));
    if text
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\')
    {
        return reject("contains whitespace or control characters");
    }
    // Compare the scheme case-insensitively, but only `https://` passes:
    // `http:`, `javascript:`, `data:` and scheme-less text all fall through.
    let Some(rest) = text
        .get(..8)
        .filter(|head| head.eq_ignore_ascii_case("https://"))
        .map(|_| &text[8..])
    else {
        return reject("only https:// links are allowed");
    };
    let (authority, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, ""),
    };
    if authority.is_empty() || authority.contains('@') {
        return reject("missing host, or credentials in the URL");
    }
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(name, port)| {
            if port.chars().all(|ch| ch.is_ascii_digit()) && !port.is_empty() {
                name
            } else {
                authority
            }
        });
    let host_ok = !host.is_empty()
        && host.contains('.')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-');
    if !host_ok {
        return reject("not a valid host name");
    }
    if path.contains(['?', '#']) {
        return reject("query and fragment are not allowed");
    }
    Ok(format!(
        "https://{}{}",
        authority.to_ascii_lowercase(),
        path.trim_end_matches('/')
    ))
}

/// The page of an exchange's own history, by the exchange id a record stores.
/// Exchanges do not publish a deep link to one fill, so the id of the fill is
/// shown next to the page instead. Only the site root is known for Bybit:
/// the exact path of its history page could not be confirmed from the
/// documentation, so none is invented.
fn exchange_history_page(exchange: &str) -> Option<&'static str> {
    match exchange.to_ascii_lowercase().as_str() {
        "bybit" => Some("https://www.bybit.com/"),
        _ => None,
    }
}

/// A hash or address that is safe to put into a URL path: `0x`, then hex.
fn is_hex_word(text: &str, digits: Option<usize>) -> bool {
    text.strip_prefix("0x").is_some_and(|body| {
        !body.is_empty()
            && digits.is_none_or(|len| body.len() == len)
            && body.chars().all(|ch| ch.is_ascii_hexdigit())
    })
}

pub trait TrackingRefExt: Sized {
    /// The page this reference points at, `https` only. A user's own explorer
    /// for the network overrides the preset. `None` when no safe link exists.
    fn url(&self, settings: &ExplorerSettings) -> Option<String>;
    /// Like [`TrackingRef::url`], with the note to show next to the link.
    fn link(&self, settings: &ExplorerSettings) -> Option<TrackingLink>;
}

impl TrackingRefExt for TrackingRef {
    fn url(&self, settings: &ExplorerSettings) -> Option<String> {
        self.link(settings).map(|link| link.url)
    }

    fn link(&self, settings: &ExplorerSettings) -> Option<TrackingLink> {
        let plain = |url: String| Some(TrackingLink { url, note: None });
        match self {
            Self::ChainTx { chain, hash } => {
                if let Some(base) = settings.base(*chain) {
                    return word(hash, Some(64), &base, "tx").and_then(plain);
                }
                match evm_chain(*chain) {
                    Some(row) => word(hash, Some(64), row.explorer, "tx").and_then(plain),
                    // Networks outside the EVM table keep the older template.
                    None => chain
                        .explorer_url(hash)
                        .filter(|url| url.starts_with("https://"))
                        .and_then(plain),
                }
            }
            Self::Address { chain, address } => explorer_base(settings, *chain)
                .and_then(|base| word(address, Some(40), &base, "address"))
                .and_then(plain),
            // A position id is a token id or an address; only an address
            // has a page of its own on an explorer.
            Self::Position { chain, id, .. } => explorer_base(settings, *chain)
                .and_then(|base| word(id, Some(40), &base, "address"))
                .and_then(plain),
            Self::ExchangeHistory { exchange, exec_id } => {
                let url = exchange_history_page(exchange)?.to_owned();
                let note = (!exec_id.is_empty()).then(|| exec_id.clone());
                Some(TrackingLink { url, note })
            }
        }
    }
}

fn explorer_base(settings: &ExplorerSettings, chain: Chain) -> Option<String> {
    settings
        .base(chain)
        .or_else(|| evm_chain(chain).map(|row| row.explorer.to_owned()))
}

fn word(text: &str, digits: Option<usize>, base: &str, segment: &str) -> Option<String> {
    is_hex_word(text, digits).then(|| format!("{base}/{segment}/{text}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::all;

    const HASH: &str = "0xdd5e3b2bd2b34b8a1f2c7c1e0a6e1e1f5b0a51f7d1a7e9bf4a0ac5dc7d6e1a2b";
    const ADDRESS: &str = "0x00000000000000000000000000000000000000aa";

    fn none() -> ExplorerSettings {
        ExplorerSettings::default()
    }

    #[test]
    fn every_chain_builds_a_tx_and_an_address_link_on_its_preset() {
        for row in all() {
            let tx = TrackingRef::ChainTx {
                chain: row.chain,
                hash: HASH.into(),
            };
            assert_eq!(tx.url(&none()), Some(format!("{}/tx/{HASH}", row.explorer)));
            let addr = TrackingRef::Address {
                chain: row.chain,
                address: ADDRESS.into(),
            };
            assert_eq!(
                addr.url(&none()),
                Some(format!("{}/address/{ADDRESS}", row.explorer))
            );
            let pos = TrackingRef::Position {
                protocol: "aave-v3".into(),
                chain: row.chain,
                id: ADDRESS.into(),
            };
            assert_eq!(
                pos.url(&none()),
                Some(format!("{}/address/{ADDRESS}", row.explorer))
            );
        }
    }

    #[test]
    fn a_position_that_is_not_an_address_has_no_link() {
        let pos = TrackingRef::Position {
            protocol: "uniswap-v3".into(),
            chain: Chain::Base,
            id: "123456".into(),
        };
        assert_eq!(pos.url(&none()), None);
    }

    #[test]
    fn a_users_explorer_overrides_the_preset() {
        let mut settings = none();
        settings
            .set(Chain::Ethereum, "https://Explorer.Example.org/")
            .unwrap();
        let tx = TrackingRef::ChainTx {
            chain: Chain::Ethereum,
            hash: HASH.into(),
        };
        assert_eq!(
            tx.url(&settings),
            Some(format!("https://explorer.example.org/tx/{HASH}"))
        );
        let addr = TrackingRef::Address {
            chain: Chain::Ethereum,
            address: ADDRESS.into(),
        };
        assert_eq!(
            addr.url(&settings),
            Some(format!("https://explorer.example.org/address/{ADDRESS}"))
        );
        // Another network keeps its preset.
        let other = TrackingRef::ChainTx {
            chain: Chain::Base,
            hash: HASH.into(),
        };
        assert_eq!(
            other.url(&settings),
            Some(format!("https://basescan.org/tx/{HASH}"))
        );
    }

    #[test]
    fn insecure_or_script_explorers_are_rejected() {
        for bad in [
            "http://explorer.example.org",
            "javascript:alert(1)",
            "JAVASCRIPT:alert(1)",
            "data:text/html,x",
            "ftp://explorer.example.org",
            "explorer.example.org",
            "//explorer.example.org",
            "https://",
            "https://user:pw@explorer.example.org",
            "https://explorer.example.org/?q=1",
            "https://explorer.example.org/#x",
            "https://explorer example.org",
            "https://localhost",
        ] {
            assert!(normalize_explorer(bad).is_err(), "{bad} must be rejected");
            assert!(none().set(Chain::Ethereum, bad).is_err());
        }
    }

    #[test]
    fn a_bad_stored_explorer_falls_back_to_the_preset() {
        for bad in ["http://evil.example", "javascript:alert(1)"] {
            let mut settings = none();
            settings.custom.insert(Chain::Ethereum, bad.into());
            let tx = TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: HASH.into(),
            };
            assert_eq!(
                tx.url(&settings),
                Some(format!("https://etherscan.io/tx/{HASH}"))
            );
        }
    }

    #[test]
    fn a_hash_that_is_not_hex_never_reaches_a_url() {
        let tx = TrackingRef::ChainTx {
            chain: Chain::Ethereum,
            hash: "0x12/../../x?y".into(),
        };
        assert_eq!(tx.url(&none()), None);
        let addr = TrackingRef::Address {
            chain: Chain::Ethereum,
            address: "javascript:1".into(),
        };
        assert_eq!(addr.url(&none()), None);
    }

    #[test]
    fn an_exchange_fill_links_the_history_page_and_keeps_the_exec_id_aside() {
        let fill = TrackingRef::ExchangeHistory {
            exchange: "Bybit".into(),
            exec_id: "exec-42".into(),
        };
        let link = fill.link(&none()).expect("bybit has a page");
        assert!(link.url.starts_with("https://www.bybit.com"));
        assert!(!link.url.contains("exec-42"), "no fake deep link");
        assert_eq!(link.note.as_deref(), Some("exec-42"));
        let unknown = TrackingRef::ExchangeHistory {
            exchange: "nowhere".into(),
            exec_id: "x1".into(),
        };
        assert_eq!(unknown.url(&none()), None);
    }

    #[test]
    fn a_chain_outside_the_evm_table_keeps_the_older_template() {
        let tx = TrackingRef::ChainTx {
            chain: Chain::Solana,
            hash: "5abcDEF".into(),
        };
        // `is_hex_word` is EVM-only; the legacy template has its own rule.
        assert!(
            tx.url(&none())
                .is_none_or(|url| url.starts_with("https://"))
        );
        let addr = TrackingRef::Address {
            chain: Chain::Bitcoin,
            address: "bc1q".into(),
        };
        assert_eq!(addr.url(&none()), None);
    }
}
