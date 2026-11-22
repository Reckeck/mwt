//! The rules of the followed addresses (spec E9): what an address may look
//! like, how a new account is made and how an edit changes one.
//!
//! A connected address is an ordinary wallet of kind `Crypto` whose
//! `Store::crypto_account_id` points at its [`CryptoAccount`] (decision P5 of
//! 2026-11-11). The scenarios that write sit in
//! `usecases::cryptotrack::accounts`; every change there ends in
//! `UnlockedVault::crypto_sync_holdings` and not in a bare save: the capital is
//! read from `doc.holdings`, which a save does not re-derive, so without it the
//! wallet would list the address and count nothing until the next poll
//! (spec E8).
//!
//! # No secrets, ever
//!
//! Only a public address comes in. A string that looks like a seed phrase or
//! a private key ([`looks_like_secret`]) is refused before anything else is
//! looked at: it is not stored, not echoed in the error and not written to a
//! log line, and the input's `Debug` hides the field.

use crate::chains;
use crate::validate::Validate;
use mwt_base::error::{Error, Result};
use mwt_model::Chain;
use mwt_model::Store;
use mwt_model::VaultDoc;
use mwt_model::{CryptoAccount, normalize_address};

/// Where "the whole history" starts: before any chain existed.
const WHOLE_HISTORY: &str = "1970-01-01T00:00:00Z";
/// The colour of a connected address until the user picks another.
pub const ADDRESS_COLOR: &str = "#627eea";

const BASE58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Whether a pasted string looks like a seed phrase or a private key rather
/// than a public address.
///
/// It errs on the side of refusing: a real address is never several words,
/// never 64 hex digits and never a WIF or an extended private key, so a false
/// alarm costs the user one paste and a miss costs them their funds.
pub fn looks_like_secret(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    // A mnemonic: several plain words. An address has no spaces.
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() >= 6
        && words.iter().all(|word| {
            (2..=12).contains(&word.len()) && word.chars().all(|ch| ch.is_ascii_alphabetic())
        })
    {
        return true;
    }
    // A raw key: 32 bytes or more of hex, with or without the prefix.
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    if hex.len() >= 64 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return true;
    }
    // Extended private keys (BIP32): xprv, yprv, zprv and their test twins.
    if ["xprv", "yprv", "zprv", "tprv", "uprv", "vprv"]
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        return true;
    }
    let is_base58 = text.chars().all(|ch| BASE58.contains(ch));
    // Bitcoin WIF: 51 characters starting with 5, or 52 starting with K or L.
    if is_base58
        && ((text.len() == 51 && text.starts_with('5'))
            || (text.len() == 52 && (text.starts_with('K') || text.starts_with('L'))))
    {
        return true;
    }
    // A Solana keypair, base58 (87-88 characters) or as a byte array.
    if is_base58 && (87..=88).contains(&text.len()) {
        return true;
    }
    text.starts_with('[') && text.matches(',').count() >= 31
}

/// `0x` and 40 hex digits: the shape of an EVM address.
fn is_evm_address(address: &str) -> bool {
    address
        .strip_prefix("0x")
        .is_some_and(|rest| rest.len() == 40 && rest.chars().all(|ch| ch.is_ascii_hexdigit()))
}

/// `0x7a3F…9bC1`: how an address is shown where there is no room for all of it.
fn short_address(address: &str) -> String {
    let chars: Vec<char> = address.chars().collect();
    if chars.len() <= 12 {
        return address.to_string();
    }
    let head: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// `YYYY-MM-DD` as a start moment, or an error that names the field.
fn start_moment(date: &str) -> Result<String> {
    let bad = || Error::InvalidInput("crypto: the start date must be YYYY-MM-DD".into());
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad());
    };
    let number = |text: &str, len: usize| -> Option<u32> {
        (text.len() == len && text.chars().all(|ch| ch.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    let (Some(year), Some(month), Some(day)) = (number(year, 4), number(month, 2), number(day, 2))
    else {
        return Err(bad());
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 2009 {
        return Err(bad());
    }
    Ok(format!("{year:04}-{month:02}-{day:02}T00:00:00Z"))
}

/// The wallet that shows an account, when one does.
pub fn wallet_of<'doc>(doc: &'doc VaultDoc, account_id: &str) -> Option<&'doc Store> {
    doc.stores
        .iter()
        .find(|store| store.crypto_account_id.as_deref() == Some(account_id))
}

/// The account a new address makes, or the reason it is refused: a network
/// that cannot be followed, a string that is not a public EVM address, an
/// address that is already followed, a start date that is not `YYYY-MM-DD`,
/// an account the model refuses. The name falls back to the short address.
/// Nothing is changed; the caller stores the account under `id`.
pub fn new_account(
    doc: &VaultDoc,
    id: &str,
    chain: Chain,
    address: &str,
    name: &str,
    start_date: Option<&str>,
) -> Result<CryptoAccount> {
    if chains::evm_chain(chain).is_none() {
        return Err(Error::InvalidInput(
            "crypto: this network cannot be followed yet".into(),
        ));
    }
    let address = normalize_address(chain, address);
    if !is_evm_address(&address) {
        return Err(Error::InvalidInput(
            "crypto: a public address starts with 0x and has 40 hex digits".into(),
        ));
    }
    if doc
        .crypto_accounts
        .iter()
        .any(|account| account.chain == chain && account.address == address)
    {
        return Err(Error::AlreadyExists(
            "this address is already followed".into(),
        ));
    }
    let created_at = match start_date.map(str::trim) {
        Some(date) if !date.is_empty() => start_moment(date)?,
        _ => WHOLE_HISTORY.to_string(),
    };
    let name = match name.trim() {
        "" => short_address(&address),
        name => name.to_string(),
    };
    let account = CryptoAccount::new(id, &name, chain, &address, &created_at);
    account.validate()?;
    Ok(account)
}

/// Rename an account, change its note, or stop counting it (archive) and
/// start again. `name` is already trimmed and not empty.
pub fn edit_account(
    account: &mut CryptoAccount,
    name: Option<&str>,
    note: Option<&str>,
    archived: Option<bool>,
) {
    if let Some(name) = name {
        account.name = name.to_string();
    }
    if let Some(note) = note {
        account.note = note.trim().to_string();
    }
    if let Some(archived) = archived {
        account.archived = archived;
    }
}
