//! Scenarios of the followed addresses (spec E9): list, connect, edit and
//! forget. The rules (what an address looks like, how an account is made and
//! edited) are in `cryptotrack::service::accounts`.
//!
//! A change is one `vault.transact` over the account records
//! (`tx.collection::<CryptoAccount>()`, so a forgotten account leaves its
//! tombstone) and is followed by `crypto_sync_holdings`: the capital is read
//! from `doc.holdings`, which a save does not re-derive, so without it the
//! wallet would list the address and count nothing until the next poll
//! (spec E8). That fold is a write of its own until it leaves the vault.
//!
//! A string that looks like a seed phrase or a private key is refused before
//! anything else is looked at ([`refuse_secrets`]).

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::cryptotrack::service::accounts::{self as rules, looks_like_secret};
use crate::cryptotrack::service::totals::crypto_quarantined;
use crate::cryptotrack::tokens;
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::accounts::{
    ConnectAddressInput, ConnectAddressResult, CryptoAccountView, MonthChange, QuarantinedHolding,
    SECRET_REFUSED, UpdateAccountInput,
};
use crate::model::cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoDirection, CryptoPosition, TokenId,
};
use crate::model::store::{Store, StoreKind};
use crate::model::vault::VaultDoc;
use crate::money::books::Books;
use crate::service::{StoreExtras, UnlockedVault};
use crate::time::{secs_or_now, unix_secs};

/// A secret in any of these texts is refused: they are stored in the vault and
/// synced to the other devices exactly like the address, so a seed pasted into
/// the field beside the right one must not get in either. Not stored, not
/// echoed in the error.
fn refuse_secrets(texts: &[Option<&str>]) -> Result<()> {
    if texts.iter().flatten().any(|text| looks_like_secret(text)) {
        return Err(Error::InvalidInput(SECRET_REFUSED.into()));
    }
    Ok(())
}

/// The followed accounts, each with the wallet that shows it and the
/// tokens of it that wait in quarantine.
pub fn crypto_list_accounts(books: &Books<'_>) -> Vec<CryptoAccountView> {
    let doc = books.doc();
    let waiting = crypto_quarantine_by_account(books);
    doc.crypto_accounts
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
            store_id: rules::wallet_of(doc, &account.id).map(|store| store.id.clone()),
            quarantine: waiting.get(&account.id).cloned().unwrap_or_default(),
            month_change: crypto_month_change(doc, &account.id, secs_or_now("")),
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
fn crypto_month_change(doc: &VaultDoc, account_id: &str, now_secs: i64) -> Option<MonthChange> {
    const DAY: i64 = 86_400;
    let cutoff = now_secs - 30 * DAY;
    let mut points: Vec<(i64, &crate::model::cryptotrack::CryptoSnapshot)> = doc
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
fn crypto_quarantine_by_account(books: &Books<'_>) -> BTreeMap<String, Vec<QuarantinedHolding>> {
    let doc = books.doc();
    let left_out: Vec<String> = crypto_quarantined(doc)
        .into_iter()
        .filter(|row| {
            // Hidden tokens are the settings' business, not this block's.
            doc.crypto_tokens
                .iter()
                .find(|record| record.id().key() == row.asset_key)
                .is_some_and(|record| {
                    tokens::status(doc, &record.id()) == tokens::TokenStatus::Quarantined
                })
        })
        .map(|row| row.asset_key)
        .collect();
    let mut held: BTreeMap<(String, String), (Amount, String, TokenId)> = BTreeMap::new();
    for event in &doc.crypto_events {
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
    vault: &mut UnlockedVault,
    input: &ConnectAddressInput,
) -> Result<ConnectAddressResult> {
    // First of all, before anything is looked at for anything else.
    refuse_secrets(&[Some(&input.address), Some(&input.name)])?;
    let account_id = format!("cacc-{}", uuid::Uuid::new_v4());
    let account = rules::new_account(
        &vault.doc,
        &account_id,
        input.chain,
        &input.address,
        &input.name,
        input.start_date.as_deref(),
    )?;
    // The wallet first: its name can be refused (taken, empty), and then
    // there must be no account left behind. Another area's writer, so it is a
    // step of its own, outside the transaction.
    let store = crate::usecases::money::stores::create_store_with(
        vault,
        &account.name,
        StoreKind::Crypto,
        rules::ADDRESS_COLOR,
        "",
        None,
        StoreExtras::default(),
    )?;
    vault.transact(|tx| {
        tx.collection::<CryptoAccount>().insert(account)?;
        tx.collection::<Store>().update(&store.id, |own| {
            own.crypto_account_id = Some(account_id.clone());
        })
    })?;
    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(vault)?;
    Ok(ConnectAddressResult {
        account_id,
        store_id: store.id,
    })
}

/// Rename an account, change its note, or stop counting it (archive) and
/// start again.
pub fn crypto_update_account(vault: &mut UnlockedVault, input: &UpdateAccountInput) -> Result<()> {
    let account = vault
        .doc
        .crypto_accounts
        .iter()
        .find(|account| account.id == input.account_id)
        .ok_or_else(|| Error::NotFound(format!("crypto account {}", input.account_id)))?;
    // The name and the note are stored and synced: a secret in either is
    // refused, as it is when the address is first followed.
    refuse_secrets(&[input.name.as_deref(), input.note.as_deref()])?;
    let name = input.name.as_deref().map(str::trim);
    if name == Some("") {
        return Err(Error::InvalidInput(
            "crypto: the name must not be empty".into(),
        ));
    }
    // The wallet that shows the account follows its name.
    let renamed_wallet = name.filter(|name| account.name != *name).and_then(|name| {
        rules::wallet_of(&vault.doc, &input.account_id).map(|store| (store.id.clone(), name))
    });
    if let Some((store_id, name)) = &renamed_wallet {
        crate::money::stores::validate_placement(&vault.doc, Some(store_id), None, name)?;
    }
    vault.transact(|tx| {
        if let Some((store_id, name)) = &renamed_wallet {
            tx.collection::<Store>()
                .update(store_id, |store| store.name = name.to_string())?;
        }
        tx.collection::<CryptoAccount>()
            .update(&input.account_id, |account| {
                rules::edit_account(account, name, input.note.as_deref(), input.archived);
            })
    })?;
    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(vault)
}

/// Stop following an address: the account, its read positions and its
/// cursors go, the operations already read stay until the module is wiped
/// (spec E12). The wallet that showed it is archived, because nothing drives
/// it any more and it must not turn into a hand-kept wallet.
pub fn crypto_forget_account(vault: &mut UnlockedVault, account_id: &str) -> Result<()> {
    if !vault
        .doc
        .crypto_accounts
        .iter()
        .any(|account| account.id == account_id)
    {
        return Err(Error::NotFound(format!("crypto account {account_id}")));
    }
    vault.transact(|tx| {
        tx.collection::<CryptoAccount>().remove(account_id)?;
        // The cursors stay on the device and are no record.
        tx.doc_mut()
            .crypto_cursors
            .retain(|cursor| cursor.account_id != account_id);
        // A position is a derived reading of the account: left behind, it
        // would stay listed as open and valued, and its receipts would keep
        // being dropped from the capital of other accounts on the network.
        let positions: Vec<String> = tx
            .collection::<CryptoPosition>()
            .iter()
            .filter(|position| position.account_id == account_id)
            .map(|position| position.id.clone())
            .collect();
        for id in &positions {
            tx.collection::<CryptoPosition>().remove(id)?;
        }
        let wallets: Vec<String> = tx
            .collection::<Store>()
            .iter()
            .filter(|store| store.crypto_account_id.as_deref() == Some(account_id))
            .map(|store| store.id.clone())
            .collect();
        for id in &wallets {
            tx.collection::<Store>().update(id, |store| {
                store.crypto_account_id = None;
                store.archived = true;
            })?;
        }
        Ok(())
    })?;
    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(vault)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoLeg, CryptoTx, CryptoTxKind, TrackingRef};
    use crate::service::test_support::empty_vault;

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
        let error = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, &seed_text()),
        )
        .expect_err("a secret in the name must be refused");
        assert!(format!("{error}").contains("seed phrase or a private key"));
        assert_eq!(vault.doc.stores.len(), stores, "nothing is stored");
        assert!(vault.doc.crypto_accounts.is_empty());

        // The address alone goes through, and then the name and the note are
        // checked on every change of it.
        let made = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
        .expect("a public address with an ordinary name");
        for secret in [seed_text(), key_text()] {
            for field in 0..2 {
                let input = UpdateAccountInput {
                    account_id: made.account_id.clone(),
                    name: (field == 0).then(|| secret.clone()),
                    note: (field == 1).then(|| secret.clone()),
                    archived: None,
                };
                let refused = crate::usecases::cryptotrack::accounts::crypto_update_account(
                    &mut vault, &input,
                )
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
            let error = crate::usecases::cryptotrack::accounts::crypto_connect_address(
                &mut vault,
                &connect(secret, "Main"),
            )
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
        let done = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
        .unwrap();
        let account =
            crate::usecases::cryptotrack::accounts::crypto_list_accounts(&vault.books()).remove(0);
        assert_eq!(account.id, done.account_id);
        assert_eq!(account.store_id.as_deref(), Some(done.store_id.as_str()));
        assert_eq!(account.address, ADDRESS.to_lowercase());
        assert_eq!(account.created_at, "2026-01-01T00:00:00Z");
        let store = crate::money::tree::find_store(&vault.doc, &done.store_id).unwrap();
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
            crate::usecases::cryptotrack::accounts::crypto_connect_address(
                &mut vault,
                &connect("0x12", "Main")
            )
            .is_err()
        );
        assert!(
            crate::usecases::cryptotrack::accounts::crypto_connect_address(
                &mut vault,
                &connect(ADDRESS, "Main")
            )
            .is_ok()
        );
        // The same address in another case is the same address.
        let again = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(&ADDRESS.to_uppercase().replace("0X", "0x"), "Other"),
        );
        assert!(matches!(again, Err(Error::AlreadyExists(_))), "{again:?}");
        // A taken wallet name is refused without an account being left over.
        let accounts = vault.doc.crypto_accounts.len();
        assert!(
            crate::usecases::cryptotrack::accounts::crypto_connect_address(
                &mut vault,
                &connect(SCAM, "Main")
            )
            .is_err()
        );
        assert_eq!(vault.doc.crypto_accounts.len(), accounts);
        let mut bad_date = connect(SCAM, "Second");
        bad_date.start_date = Some("yesterday".into());
        assert!(
            crate::usecases::cryptotrack::accounts::crypto_connect_address(&mut vault, &bad_date)
                .is_err()
        );
        let mut whole = connect(SCAM, "Second");
        whole.start_date = None;
        crate::usecases::cryptotrack::accounts::crypto_connect_address(&mut vault, &whole).unwrap();
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
        let done = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
        .unwrap();
        let token = TokenId::new(Chain::Ethereum, SCAM);
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &token, "PEPE2", "Pepe 2", 18,
        )
        .unwrap();
        vault.doc.crypto_events.push(event(
            &done.account_id,
            &token,
            "1200000",
            CryptoDirection::In,
            "2026-10-14T10:00:00Z",
        ));
        let listed =
            crate::usecases::cryptotrack::accounts::crypto_list_accounts(&vault.books()).remove(0);
        assert_eq!(listed.quarantine.len(), 1);
        assert_eq!(
            listed.quarantine[0].quantity,
            Amount::parse("1200000").unwrap()
        );
        assert_eq!(listed.quarantine[0].first_seen, "2026-10-14T10:00:00Z");
        crate::usecases::cryptotrack::tokens::crypto_confirm_token(&mut vault, &token).unwrap();
        assert!(
            crate::usecases::cryptotrack::accounts::crypto_list_accounts(&vault.books())[0]
                .quarantine
                .is_empty()
        );
        crate::usecases::cryptotrack::tokens::crypto_reject_token(&mut vault, &token).unwrap();
        assert!(
            crate::usecases::cryptotrack::accounts::crypto_list_accounts(&vault.books())[0]
                .quarantine
                .is_empty()
        );
    }

    #[test]
    fn two_huge_transfers_of_a_junk_token_do_not_take_the_list_down() {
        // A token with no decimals may report any whole number: two transfers
        // of 5e28 add up to 1e29, which a decimal cannot hold. The sum must
        // saturate, not panic (the events are already stored, so a panic here
        // would repeat on every opening of the list).
        let (_dir, mut vault) = empty_vault();
        let done = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
        .unwrap();
        let token = TokenId::new(Chain::Ethereum, SCAM);
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &token, "JUNK", "Junk", 0,
        )
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
        let listed =
            crate::usecases::cryptotrack::accounts::crypto_list_accounts(&vault.books()).remove(0);
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
        assert_eq!(crypto_month_change(&vault.doc, "acc", now), None);
        // A change that fits but whose percentage does not: no percentage.
        vault.doc.crypto_snapshots = vec![
            snapshot("2026-10-13", "1"),
            snapshot("2026-11-12", "70000000000000000000000000000"),
        ];
        let change = crypto_month_change(&vault.doc, "acc", now).unwrap();
        assert_eq!(change.change_pct, None);
    }

    #[test]
    fn forgetting_an_account_takes_its_positions_and_their_receipts_with_it() {
        use crate::model::cryptotrack::CryptoPosition;
        let (_dir, mut vault) = empty_vault();
        let gone = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
        .unwrap();
        let kept = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(SCAM, "Other"),
        )
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

        crate::usecases::cryptotrack::accounts::crypto_forget_account(&mut vault, &gone.account_id)
            .unwrap();

        assert_eq!(vault.doc.crypto_positions.len(), 1);
        assert_eq!(vault.doc.crypto_positions[0].account_id, kept.account_id);
        assert!(
            crate::usecases::cryptotrack::positions::crypto_list_positions(
                &vault.books(),
                &[],
                "2026-11-02T00:00:00Z"
            )
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
        assert_eq!(crypto_month_change(&vault.doc, "acc", now), None);
        // A snapshot 30 days old: both ends are recorded values.
        vault
            .doc
            .crypto_snapshots
            .push(snapshot("2026-10-13", "800"));
        let change = crypto_month_change(&vault.doc, "acc", now).expect("a month of data");
        assert_eq!(change.change_usd, Amount::parse("200").unwrap());
        assert_eq!(change.change_pct, Some(Amount::parse("25").unwrap()));
        assert!(change.since.starts_with("2026-10-13"));
        // Another account's snapshots never count.
        assert_eq!(crypto_month_change(&vault.doc, "other", now), None);
        // A reference far older than the month is a gap in tracking, not a month.
        vault.doc.crypto_snapshots = vec![
            snapshot("2026-08-01", "800"),
            snapshot("2026-11-12", "1000"),
        ];
        assert_eq!(crypto_month_change(&vault.doc, "acc", now), None);
        // A zero start has an amount but no percent.
        vault.doc.crypto_snapshots =
            vec![snapshot("2026-10-13", "0"), snapshot("2026-11-12", "50")];
        let from_zero = crypto_month_change(&vault.doc, "acc", now).unwrap();
        assert_eq!(from_zero.change_pct, None);
    }

    #[test]
    fn a_new_account_with_events_shows_its_capital_at_once() {
        // `persist()` alone would leave the wallet at zero until the next poll.
        let (_dir, mut vault) = empty_vault();
        let done = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
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
        crate::usecases::cryptotrack::accounts::crypto_update_account(
            &mut vault,
            &UpdateAccountInput {
                account_id: done.account_id.clone(),
                name: None,
                note: Some("hot".into()),
                archived: Some(false),
            },
        )
        .unwrap();
        let held = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == done.store_id)
            .expect("the capital is there without a poll");
        assert_eq!(held.quantity, Amount::parse("2").unwrap());
        crate::usecases::cryptotrack::accounts::crypto_update_account(
            &mut vault,
            &UpdateAccountInput {
                account_id: done.account_id.clone(),
                name: Some("Renamed".into()),
                note: None,
                archived: Some(true),
            },
        )
        .unwrap();
        assert!(
            vault
                .doc
                .holdings
                .iter()
                .all(|holding| holding.store_id != done.store_id)
        );
        assert_eq!(
            crate::money::tree::find_store(&vault.doc, &done.store_id)
                .unwrap()
                .name,
            "Renamed"
        );
        crate::usecases::cryptotrack::accounts::crypto_forget_account(&mut vault, &done.account_id)
            .unwrap();
        assert!(vault.doc.crypto_accounts.is_empty());
        assert!(
            crate::money::tree::find_store(&vault.doc, &done.store_id)
                .unwrap()
                .archived
        );
        assert!(
            crate::money::tree::find_store(&vault.doc, &done.store_id)
                .unwrap()
                .crypto_account_id
                .is_none()
        );
        assert_eq!(vault.doc.crypto_events.len(), 1, "the operations stay");
    }

    #[test]
    fn an_account_note_survives_a_reopen_when_holdings_do_not_change() {
        let (_dir, mut vault) = empty_vault();
        let made = crate::usecases::cryptotrack::accounts::crypto_connect_address(
            &mut vault,
            &connect(ADDRESS, "Main"),
        )
        .unwrap();
        crate::usecases::cryptotrack::accounts::crypto_update_account(
            &mut vault,
            &UpdateAccountInput {
                account_id: made.account_id.clone(),
                name: None,
                note: Some("kept".into()),
                archived: None,
            },
        )
        .unwrap();
        let path = vault.path.clone();
        drop(vault);
        let reopened = UnlockedVault::open(path, "pw").unwrap();
        let account = reopened
            .doc
            .crypto_accounts
            .iter()
            .find(|account| account.id == made.account_id)
            .expect("the account is stored");
        assert_eq!(account.note, "kept");
    }
}
