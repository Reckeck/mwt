//! The followed addresses: connect, list, edit and forget (spec E9).
//!
//! A connected address is an ordinary wallet of kind `Crypto` whose
//! `Store::crypto_account_id` points at its [`CryptoAccount`] (decision P5 of
//! 2026-11-11). Every change here ends in
//! [`UnlockedVault::crypto_sync_holdings`] and not in a bare `persist()`: the
//! capital is read from `doc.holdings`, which `persist()` does not re-derive,
//! so without it the wallet would list the address and count nothing until
//! the next poll (spec E8).
//!
//! # No secrets, ever
//!
//! Only a public address comes in. A string that looks like a seed phrase or
//! a private key ([`looks_like_secret`]) is refused before anything else is
//! looked at: it is not stored, not echoed in the error and not written to a
//! log line, and the input's `Debug` hides the field.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::super::UnlockedVault;
use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::cryptotrack::{chains, tokens};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::accounts::{
    ConnectAddressInput, ConnectAddressResult, CryptoAccountView, MonthChange, QuarantinedHolding,
    SECRET_REFUSED, UpdateAccountInput,
};
use crate::model::cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoDirection, TokenId, normalize_address,
};
use crate::model::store::{Store, StoreKind};
use crate::pricing::history::{secs_or_now, unix_secs};
use crate::service::StoreExtras;

/// Where "the whole history" starts: before any chain existed.
const WHOLE_HISTORY: &str = "1970-01-01T00:00:00Z";
/// The colour of a connected address until the user picks another.
const ADDRESS_COLOR: &str = "#627eea";

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

impl UnlockedVault {
    /// The followed accounts, each with the wallet that shows it and the
    /// tokens of it that wait in quarantine.
    pub fn crypto_list_accounts(&self) -> Vec<CryptoAccountView> {
        let waiting = self.crypto_quarantine_by_account();
        self.doc
            .crypto_accounts
            .iter()
            .map(|account| CryptoAccountView {
                id: account.id.clone(),
                name: account.name.clone(),
                chain: account.chain,
                address: account.address.clone(),
                source: account.source.clone(),
                archived: account.archived,
                note: account.note.clone(),
                created_at: account.created_at.clone(),
                store_id: self
                    .doc
                    .stores
                    .iter()
                    .find(|store| store.crypto_account_id.as_deref() == Some(account.id.as_str()))
                    .map(|store| store.id.clone()),
                quarantine: waiting.get(&account.id).cloned().unwrap_or_default(),
                month_change: self.crypto_month_change(&account.id, secs_or_now("")),
            })
            .collect()
    }

    /// How the account's value moved over about a month, from its own daily
    /// snapshots (spec E8's `crypto_snapshots`, one per read).
    ///
    /// The older value is the latest snapshot at or before `now - 30 days`,
    /// and only if it is at most 7 days older than that: a freshly connected
    /// address, or a stretch with no reads (tracking runs only while the vault
    /// is unlocked), has no such snapshot, and then there is *no* number
    /// rather than one computed from a short window. The newer value is the
    /// account's latest snapshot, so both ends are values the module recorded.
    pub(crate) fn crypto_month_change(
        &self,
        account_id: &str,
        now_secs: i64,
    ) -> Option<MonthChange> {
        const DAY: i64 = 86_400;
        let cutoff = now_secs - 30 * DAY;
        let mut points: Vec<(i64, &crate::model::cryptotrack::CryptoSnapshot)> = self
            .doc
            .crypto_snapshots
            .iter()
            .filter(|record| record.account_id.as_deref() == Some(account_id))
            .filter_map(|record| unix_secs(&record.at).map(|secs| (secs, record)))
            .collect();
        points.sort_by_key(|(secs, _)| *secs);
        let (_, then) = points
            .iter()
            .rev()
            .find(|(secs, _)| *secs <= cutoff)
            .filter(|(secs, _)| cutoff - *secs <= 7 * DAY)?;
        let (_, now) = points.last()?;
        // Both ends are recorded values, but a document may carry any number:
        // a change a decimal cannot hold is no number, not a panic.
        let change = math::sub(now.value_usd, then.value_usd)?;
        let pct = math::mul(change, Amount::from(100))
            .and_then(|scaled| math::div(scaled, then.value_usd))
            .map(|value| value.round(2));
        Some(MonthChange {
            change_usd: change.round(2),
            change_pct: pct.filter(|_| then.value_usd.is_positive()),
            since: then.at.clone(),
        })
    }

    /// What each account holds of the tokens left out of the totals until the
    /// user decides: quarantined, not rejected, and met in an event.
    fn crypto_quarantine_by_account(&self) -> BTreeMap<String, Vec<QuarantinedHolding>> {
        let left_out: Vec<String> = self
            .crypto_outside_totals()
            .into_iter()
            .filter(|row| row.reason == super::totals::CryptoLeftOut::Quarantined)
            .filter(|row| {
                // Hidden tokens are the settings' business, not this block's.
                self.doc
                    .crypto_tokens
                    .iter()
                    .find(|record| record.id().key() == row.asset_key)
                    .is_some_and(|record| {
                        tokens::status(&self.doc, &record.id()) == tokens::TokenStatus::Quarantined
                    })
            })
            .map(|row| row.asset_key)
            .collect();
        let mut held: BTreeMap<(String, String), (Amount, String, TokenId)> = BTreeMap::new();
        for event in &self.doc.crypto_events {
            for leg in &event.legs {
                let CryptoAsset::Token(token) = &leg.asset else {
                    continue;
                };
                let key = token.key();
                if !left_out.contains(&key) {
                    continue;
                }
                let entry = held
                    .entry((event.account_id.clone(), key))
                    .or_insert_with(|| (Amount::ZERO, event.time.clone(), token.clone()));
                // A junk token may report any number; one a decimal cannot hold
                // saturates instead of taking the list (and the app) down.
                entry.0 = match leg.direction {
                    CryptoDirection::In => {
                        math::add(entry.0, leg.amount).unwrap_or(Amount::new(Decimal::MAX))
                    }
                    CryptoDirection::Out => {
                        math::sub(entry.0, leg.amount).unwrap_or(Amount::new(Decimal::MIN))
                    }
                };
                if event.time < entry.1 {
                    entry.1 = event.time.clone();
                }
            }
        }
        let mut out: BTreeMap<String, Vec<QuarantinedHolding>> = BTreeMap::new();
        for ((account_id, _), (quantity, first_seen, token)) in held {
            out.entry(account_id).or_default().push(QuarantinedHolding {
                chain: token.chain,
                contract: token.contract,
                quantity,
                first_seen,
            });
        }
        out
    }

    /// Follow a public address: a wallet of kind `Crypto` and its account,
    /// made together. Nothing is stored when anything is refused.
    pub fn crypto_connect_address(
        &mut self,
        input: &ConnectAddressInput,
    ) -> Result<ConnectAddressResult> {
        // First of all, before anything is looked at for anything else. The
        // name is checked too: it is stored in the vault and synced to the
        // other devices exactly like the address, so a seed pasted into the
        // field beside the right one must not get in either.
        if looks_like_secret(&input.address) || looks_like_secret(&input.name) {
            return Err(Error::InvalidInput(SECRET_REFUSED.into()));
        }
        if chains::evm_chain(input.chain).is_none() {
            return Err(Error::InvalidInput(
                "crypto: this network cannot be followed yet".into(),
            ));
        }
        let address = normalize_address(input.chain, &input.address);
        if !is_evm_address(&address) {
            return Err(Error::InvalidInput(
                "crypto: a public address starts with 0x and has 40 hex digits".into(),
            ));
        }
        if self
            .doc
            .crypto_accounts
            .iter()
            .any(|account| account.chain == input.chain && account.address == address)
        {
            return Err(Error::AlreadyExists(
                "this address is already followed".into(),
            ));
        }
        let created_at = match input.start_date.as_deref().map(str::trim) {
            Some(date) if !date.is_empty() => start_moment(date)?,
            _ => WHOLE_HISTORY.to_string(),
        };
        let name = match input.name.trim() {
            "" => short_address(&address),
            name => name.to_string(),
        };

        let account_id = format!("cacc-{}", uuid::Uuid::new_v4());
        let account = CryptoAccount::new(&account_id, &name, input.chain, &address, &created_at);
        account.validate()?;
        // The wallet first: its name can be refused (taken, empty), and then
        // there must be no account left behind.
        let store = self.create_store_with(
            &name,
            StoreKind::Crypto,
            ADDRESS_COLOR,
            "",
            None,
            StoreExtras::default(),
        )?;
        self.doc.crypto_accounts.push(account);
        self.bind_store_to_account(&store, &account_id);
        self.crypto_sync_holdings()?;
        Ok(ConnectAddressResult {
            account_id,
            store_id: store.id,
        })
    }

    fn bind_store_to_account(&mut self, store: &Store, account_id: &str) {
        if let Some(own) = self.doc.stores.iter_mut().find(|own| own.id == store.id) {
            own.crypto_account_id = Some(account_id.to_string());
        }
    }

    /// Rename an account, change its note, or stop counting it (archive) and
    /// start again.
    pub fn crypto_update_account(&mut self, input: &UpdateAccountInput) -> Result<()> {
        let index = self
            .doc
            .crypto_accounts
            .iter()
            .position(|account| account.id == input.account_id)
            .ok_or_else(|| Error::NotFound(format!("crypto account {}", input.account_id)))?;
        // The name and the note are stored and synced: a secret in either is
        // refused, as it is when the address is first followed.
        for text in [input.name.as_deref(), input.note.as_deref()] {
            if text.is_some_and(looks_like_secret) {
                return Err(Error::InvalidInput(SECRET_REFUSED.into()));
            }
        }
        let name = input.name.as_deref().map(str::trim);
        if name == Some("") {
            return Err(Error::InvalidInput(
                "crypto: the name must not be empty".into(),
            ));
        }
        if let Some(name) = name {
            let store_id = self
                .doc
                .stores
                .iter()
                .find(|store| store.crypto_account_id.as_deref() == Some(input.account_id.as_str()))
                .map(|store| store.id.clone());
            if let Some(store_id) =
                store_id.filter(|_| self.doc.crypto_accounts[index].name != name)
            {
                self.validate_placement(Some(&store_id), None, name)?;
                if let Some(store) = self
                    .doc
                    .stores
                    .iter_mut()
                    .find(|store| store.id == store_id)
                {
                    store.name = name.to_string();
                }
            }
            self.doc.crypto_accounts[index].name = name.to_string();
        }
        let account = &mut self.doc.crypto_accounts[index];
        if let Some(note) = &input.note {
            account.note = note.trim().to_string();
        }
        if let Some(archived) = input.archived {
            account.archived = archived;
        }
        self.crypto_sync_holdings()
    }

    /// Stop following an address: the account, its read positions and its
    /// cursors go, the
    /// operations already read stay until the module is wiped (spec E12). The
    /// wallet that showed it is archived, because nothing drives it any more
    /// and it must not turn into a hand-kept wallet.
    pub fn crypto_forget_account(&mut self, account_id: &str) -> Result<()> {
        if !self
            .doc
            .crypto_accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            return Err(Error::NotFound(format!("crypto account {account_id}")));
        }
        self.doc
            .crypto_accounts
            .retain(|account| account.id != account_id);
        self.doc
            .crypto_cursors
            .retain(|cursor| cursor.account_id != account_id);
        // A position is a derived reading of the account: left behind, it
        // would stay listed as open and valued, and its receipts would keep
        // being dropped from the capital of other accounts on the network.
        self.doc
            .crypto_positions
            .retain(|position| position.account_id != account_id);
        for store in &mut self.doc.stores {
            if store.crypto_account_id.as_deref() == Some(account_id) {
                store.crypto_account_id = None;
                store.archived = true;
            }
        }
        self.crypto_sync_holdings()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::empty_vault;
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoLeg, CryptoTx, CryptoTxKind, TrackingRef};

    /// A junk token reports quantities past the input bound, so these tests
    /// build them without `Amount::parse`.
    fn unbounded(text: &str) -> Amount {
        Amount::new(text.parse().expect("a test amount"))
    }

    const ADDRESS: &str = "0x7a3F19c4D2b85E6F0a9C41b7cE3a0D8e2f4b9bC1";
    const SCAM: &str = "0x00000000000000000000000000000000deadbeef";

    fn connect(address: &str, name: &str) -> ConnectAddressInput {
        ConnectAddressInput {
            chain: Chain::Ethereum,
            address: address.to_string(),
            name: name.to_string(),
            start_date: Some("2026-01-01".to_string()),
        }
    }

    // The published BIP-39 and BIP-32 test vectors, assembled at run time: no
    // line of this file is a whole seed phrase, private key, WIF or xprv. The
    // rule is that no key-shaped material sits in the source at all — a
    // scanner reads a fixture exactly as it reads the real thing, and the next
    // person copying a test should not find one to copy.
    fn seed_text() -> String {
        [
            "legal winner thank year wave sausage",
            "worth useful legal winner thank yellow",
        ]
        .join(" ")
    }

    fn key_text() -> String {
        [
            "0x4c0883a69102937d6231471b5dbb6204",
            "fe5129617082792ae468d01a3f362318",
        ]
        .concat()
    }

    fn xprv_text() -> String {
        [
            "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvv",
            "NKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi",
        ]
        .concat()
    }

    fn wif_texts() -> [String; 2] {
        [
            ["5HueCGU8rMjxEXxiPuD5BDku4Mk", "FqeZyd4dZ1jvhTVqvbTLvyTJ"].concat(),
            ["L1aW4aubDFB7yfras2S1mN3bqg9", "nwySY8nkoLmJebSLD5BWv3ENZ"].concat(),
        ]
    }

    #[test]
    fn what_looks_like_a_secret_is_told_from_an_address() {
        let seed = seed_text();
        let key = key_text();
        assert!(looks_like_secret(&seed));
        assert!(looks_like_secret(&seed.to_uppercase()));
        assert!(looks_like_secret(&key));
        assert!(looks_like_secret(key.trim_start_matches("0x")));
        assert!(looks_like_secret(&xprv_text()));
        for wif in wif_texts() {
            assert!(looks_like_secret(&wif), "{wif}");
        }
        // Addresses are not secrets.
        assert!(!looks_like_secret(ADDRESS));
        assert!(!looks_like_secret(
            "0xde0B295669a9FD93d5F28D9Ec85E40f4cb697BAe"
        ));
        assert!(!looks_like_secret(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        ));
        assert!(!looks_like_secret(
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
        ));
        assert!(!looks_like_secret(""));
        assert!(!looks_like_secret("my main wallet"));
    }

    #[test]
    fn a_secret_in_the_name_or_the_note_is_refused_like_one_in_the_address() {
        // The field beside the right one: a name and a note are stored in the
        // vault and synced to every device, so a seed pasted there must not
        // get in through the back door.
        let (_dir, mut vault) = empty_vault();
        let stores = vault.doc.stores.len();
        let error = vault
            .crypto_connect_address(&connect(ADDRESS, &seed_text()))
            .expect_err("a secret in the name must be refused");
        assert!(format!("{error}").contains("seed phrase or a private key"));
        assert_eq!(vault.doc.stores.len(), stores, "nothing is stored");
        assert!(vault.doc.crypto_accounts.is_empty());

        // The address alone goes through, and then the name and the note are
        // checked on every change of it.
        let made = vault
            .crypto_connect_address(&connect(ADDRESS, "Main"))
            .expect("a public address with an ordinary name");
        for secret in [seed_text(), key_text()] {
            for field in 0..2 {
                let input = UpdateAccountInput {
                    account_id: made.account_id.clone(),
                    name: (field == 0).then(|| secret.clone()),
                    note: (field == 1).then(|| secret.clone()),
                    archived: None,
                };
                let refused = vault
                    .crypto_update_account(&input)
                    .expect_err("a secret must be refused");
                assert!(format!("{refused}").contains("seed phrase or a private key"));
            }
        }
        let doc = serde_json::to_string(&vault.doc).unwrap();
        assert!(!doc.contains(&seed_text()));
        assert!(!doc.contains(&key_text()));
    }

    #[test]
    fn a_seed_phrase_or_a_key_is_refused_and_leaves_no_trace_in_the_vault() {
        for secret in [seed_text(), key_text()] {
            let secret = secret.as_str();
            let (_dir, mut vault) = empty_vault();
            let stores = vault.doc.stores.len();
            let error = vault
                .crypto_connect_address(&connect(secret, "Main"))
                .expect_err("a secret must be refused");
            let shown = format!("{error} / {error:?}");
            assert!(shown.contains("seed phrase or a private key"), "{shown}");
            // Neither the error nor the input's Debug repeats the string.
            for word in secret.split_whitespace() {
                assert!(
                    !shown.contains(word) || word.len() < 4,
                    "the error echoes {word}"
                );
            }
            assert!(!shown.contains(&key_text()));
            assert!(vault.doc.crypto_accounts.is_empty());
            assert_eq!(vault.doc.stores.len(), stores);
            // Not a single byte of the document carries it.
            let document = serde_json::to_string(&vault.doc).unwrap();
            assert!(!document.contains(secret));
            let debug = format!("{:?}", connect(secret, "Main"));
            assert!(!debug.contains(secret) && debug.contains("<hidden>"));
        }
    }

    #[test]
    fn connecting_makes_the_wallet_and_its_account_together() {
        let (_dir, mut vault) = empty_vault();
        let done = vault
            .crypto_connect_address(&connect(ADDRESS, "Main"))
            .unwrap();
        let account = vault.crypto_list_accounts().remove(0);
        assert_eq!(account.id, done.account_id);
        assert_eq!(account.store_id.as_deref(), Some(done.store_id.as_str()));
        assert_eq!(account.address, ADDRESS.to_lowercase());
        assert_eq!(account.created_at, "2026-01-01T00:00:00Z");
        let store = vault.find_store(&done.store_id).unwrap();
        assert_eq!(store.kind, StoreKind::Crypto);
        assert_eq!(
            store.crypto_account_id.as_deref(),
            Some(done.account_id.as_str())
        );
    }

    #[test]
    fn a_bad_address_a_second_connect_and_a_taken_name_leave_nothing_behind() {
        let (_dir, mut vault) = empty_vault();
        assert!(
            vault
                .crypto_connect_address(&connect("0x12", "Main"))
                .is_err()
        );
        assert!(
            vault
                .crypto_connect_address(&connect(ADDRESS, "Main"))
                .is_ok()
        );
        // The same address in another case is the same address.
        let again = vault.crypto_connect_address(&connect(
            &ADDRESS.to_uppercase().replace("0X", "0x"),
            "Other",
        ));
        assert!(matches!(again, Err(Error::AlreadyExists(_))), "{again:?}");
        // A taken wallet name is refused without an account being left over.
        let accounts = vault.doc.crypto_accounts.len();
        assert!(
            vault
                .crypto_connect_address(&connect(SCAM, "Main"))
                .is_err()
        );
        assert_eq!(vault.doc.crypto_accounts.len(), accounts);
        let mut bad_date = connect(SCAM, "Second");
        bad_date.start_date = Some("yesterday".into());
        assert!(vault.crypto_connect_address(&bad_date).is_err());
        let mut whole = connect(SCAM, "Second");
        whole.start_date = None;
        vault.crypto_connect_address(&whole).unwrap();
        assert!(vault.doc.crypto_accounts[1].created_at.starts_with("1970"));
    }

    fn event(
        account: &str,
        token: &TokenId,
        amount: &str,
        direction: CryptoDirection,
        time: &str,
    ) -> CryptoTx {
        CryptoTx {
            id: format!("ev-{time}-{amount}"),
            chain: Chain::Ethereum,
            account_id: account.to_string(),
            time: time.to_string(),
            block: None,
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: vec![CryptoLeg {
                direction,
                asset: CryptoAsset::Token(token.clone()),
                amount: unbounded(amount),
                counterparty: None,
            }],
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{time}"),
            },
        }
    }

    #[test]
    fn the_quarantine_lists_what_arrived_and_when_and_leaves_the_decided_out() {
        let (_dir, mut vault) = empty_vault();
        let done = vault
            .crypto_connect_address(&connect(ADDRESS, "Main"))
            .unwrap();
        let token = TokenId::new(Chain::Ethereum, SCAM);
        vault
            .crypto_register_token(&token, "PEPE2", "Pepe 2", 18)
            .unwrap();
        vault.doc.crypto_events.push(event(
            &done.account_id,
            &token,
            "1200000",
            CryptoDirection::In,
            "2026-10-14T10:00:00Z",
        ));
        let listed = vault.crypto_list_accounts().remove(0);
        assert_eq!(listed.quarantine.len(), 1);
        assert_eq!(
            listed.quarantine[0].quantity,
            Amount::parse("1200000").unwrap()
        );
        assert_eq!(listed.quarantine[0].first_seen, "2026-10-14T10:00:00Z");
        vault.crypto_confirm_token(&token).unwrap();
        assert!(vault.crypto_list_accounts()[0].quarantine.is_empty());
        vault.crypto_reject_token(&token).unwrap();
        assert!(vault.crypto_list_accounts()[0].quarantine.is_empty());
    }

    #[test]
    fn two_huge_transfers_of_a_junk_token_do_not_take_the_list_down() {
        // A token with no decimals may report any whole number: two transfers
        // of 5e28 add up to 1e29, which a decimal cannot hold. The sum must
        // saturate, not panic (the events are already stored, so a panic here
        // would repeat on every opening of the list).
        let (_dir, mut vault) = empty_vault();
        let done = vault
            .crypto_connect_address(&connect(ADDRESS, "Main"))
            .unwrap();
        let token = TokenId::new(Chain::Ethereum, SCAM);
        vault
            .crypto_register_token(&token, "JUNK", "Junk", 0)
            .unwrap();
        let huge = "50000000000000000000000000000";
        for time in ["2026-10-14T10:00:00Z", "2026-10-15T10:00:00Z"] {
            vault.doc.crypto_events.push(event(
                &done.account_id,
                &token,
                huge,
                CryptoDirection::In,
                time,
            ));
        }
        let listed = vault.crypto_list_accounts().remove(0);
        assert_eq!(listed.quarantine.len(), 1);
        assert!(listed.quarantine[0].quantity >= unbounded(huge));
    }

    #[test]
    fn a_month_change_that_does_not_fit_says_nothing_instead_of_panicking() {
        use crate::model::cryptotrack::CryptoSnapshot;
        let (_dir, mut vault) = empty_vault();
        let snapshot = |day: &str, value: &str| CryptoSnapshot {
            id: format!("snap-{day}"),
            at: format!("{day}T12:00:00Z"),
            account_id: Some("acc".into()),
            value_usd: unbounded(value),
        };
        vault.doc.crypto_snapshots = vec![
            snapshot("2026-10-13", "-70000000000000000000000000000"),
            snapshot("2026-11-12", "70000000000000000000000000000"),
        ];
        let now = unix_secs("2026-11-12T12:00:00Z").unwrap();
        assert_eq!(vault.crypto_month_change("acc", now), None);
        // A change that fits but whose percentage does not: no percentage.
        vault.doc.crypto_snapshots = vec![
            snapshot("2026-10-13", "1"),
            snapshot("2026-11-12", "70000000000000000000000000000"),
        ];
        let change = vault.crypto_month_change("acc", now).unwrap();
        assert_eq!(change.change_pct, None);
    }

    #[test]
    fn forgetting_an_account_takes_its_positions_and_their_receipts_with_it() {
        use crate::model::cryptotrack::CryptoPosition;
        let (_dir, mut vault) = empty_vault();
        let gone = vault
            .crypto_connect_address(&connect(ADDRESS, "Main"))
            .unwrap();
        let kept = vault
            .crypto_connect_address(&connect(SCAM, "Other"))
            .unwrap();
        let receipt = CryptoAsset::Token(TokenId::new(Chain::Ethereum, SCAM));
        let position = |account: &str| CryptoPosition {
            id: format!("pos-{account}"),
            account_id: account.to_string(),
            chain: Chain::Ethereum,
            protocol: "lido".into(),
            name: "Staked".into(),
            legs: Vec::new(),
            receipts: vec![receipt.clone()],
            updated_at: "2026-11-01T00:00:00Z".into(),
        };
        vault.doc.crypto_positions = vec![position(&gone.account_id), position(&kept.account_id)];

        vault.crypto_forget_account(&gone.account_id).unwrap();

        assert_eq!(vault.doc.crypto_positions.len(), 1);
        assert_eq!(vault.doc.crypto_positions[0].account_id, kept.account_id);
        assert!(
            vault
                .crypto_list_positions(&[], "2026-11-02T00:00:00Z")
                .iter()
                .all(|view| view.account_id != gone.account_id),
            "a forgotten account's position is no longer listed"
        );
    }

    #[test]
    fn the_month_line_needs_a_snapshot_a_month_old_and_otherwise_says_nothing() {
        use crate::model::cryptotrack::CryptoSnapshot;
        let (_dir, mut vault) = empty_vault();
        let snapshot = |day: &str, value: &str| CryptoSnapshot {
            id: format!("snap-{day}"),
            at: format!("{day}T12:00:00Z"),
            account_id: Some("acc".into()),
            value_usd: Amount::parse(value).unwrap(),
        };
        let now = unix_secs("2026-11-12T12:00:00Z").unwrap();
        // Only a week of reads: no number, not one from a short window.
        vault.doc.crypto_snapshots = vec![
            snapshot("2026-11-05", "900"),
            snapshot("2026-11-12", "1000"),
        ];
        assert_eq!(vault.crypto_month_change("acc", now), None);
        // A snapshot 30 days old: both ends are recorded values.
        vault
            .doc
            .crypto_snapshots
            .push(snapshot("2026-10-13", "800"));
        let change = vault
            .crypto_month_change("acc", now)
            .expect("a month of data");
        assert_eq!(change.change_usd, Amount::parse("200").unwrap());
        assert_eq!(change.change_pct, Some(Amount::parse("25").unwrap()));
        assert!(change.since.starts_with("2026-10-13"));
        // Another account's snapshots never count.
        assert_eq!(vault.crypto_month_change("other", now), None);
        // A reference far older than the month is a gap in tracking, not a month.
        vault.doc.crypto_snapshots = vec![
            snapshot("2026-08-01", "800"),
            snapshot("2026-11-12", "1000"),
        ];
        assert_eq!(vault.crypto_month_change("acc", now), None);
        // A zero start has an amount but no percent.
        vault.doc.crypto_snapshots =
            vec![snapshot("2026-10-13", "0"), snapshot("2026-11-12", "50")];
        let from_zero = vault.crypto_month_change("acc", now).unwrap();
        assert_eq!(from_zero.change_pct, None);
    }

    #[test]
    fn a_new_account_with_events_shows_its_capital_at_once() {
        // `persist()` alone would leave the wallet at zero until the next poll.
        let (_dir, mut vault) = empty_vault();
        let done = vault
            .crypto_connect_address(&connect(ADDRESS, "Main"))
            .unwrap();
        let native = CryptoAsset::Native {
            chain: Chain::Ethereum,
        };
        let mut incoming = event(
            &done.account_id,
            &TokenId::new(Chain::Ethereum, SCAM),
            "2",
            CryptoDirection::In,
            "2026-02-01T10:00:00Z",
        );
        incoming.legs[0].asset = native;
        vault.doc.crypto_events.push(incoming);
        // Forgetting and archiving re-derive the holdings too.
        vault
            .crypto_update_account(&UpdateAccountInput {
                account_id: done.account_id.clone(),
                name: None,
                note: Some("hot".into()),
                archived: Some(false),
            })
            .unwrap();
        let held = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == done.store_id)
            .expect("the capital is there without a poll");
        assert_eq!(held.quantity, Amount::parse("2").unwrap());
        vault
            .crypto_update_account(&UpdateAccountInput {
                account_id: done.account_id.clone(),
                name: Some("Renamed".into()),
                note: None,
                archived: Some(true),
            })
            .unwrap();
        assert!(
            vault
                .doc
                .holdings
                .iter()
                .all(|holding| holding.store_id != done.store_id)
        );
        assert_eq!(vault.find_store(&done.store_id).unwrap().name, "Renamed");
        vault.crypto_forget_account(&done.account_id).unwrap();
        assert!(vault.doc.crypto_accounts.is_empty());
        assert!(vault.find_store(&done.store_id).unwrap().archived);
        assert!(
            vault
                .find_store(&done.store_id)
                .unwrap()
                .crypto_account_id
                .is_none()
        );
        assert_eq!(vault.doc.crypto_events.len(), 1, "the operations stay");
    }
}
