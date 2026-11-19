use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::AssetRef;
use crate::model::store::StoreKind;
use crate::model::store::{ExchangeProfile, ExchangeProvider};
use crate::model::vault::{ExchangeKeys, PricePoint};
use crate::service::UnlockedVault;
use crate::service::test_support::*;

fn eth_ref() -> AssetRef {
    AssetRef::crypto("ETH", "Ether", Some("ethereum"), Chain::Ethereum)
}

/// What every holding is valued from.
fn quote(vault: &mut UnlockedVault, asset: &AssetRef, usd: &str) {
    vault.doc.price_cache.push(PricePoint {
        asset_key: asset.key(),
        value_usd: amount(usd),
        ts: "2026-03-09T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    });
}

/// The provider of an exchange wallet: `Bybit` is one the module reads,
/// `Binance` is one it cannot, which is what makes a wallet hand-kept
/// beyond doubt (no device can hold a key for it).
fn provider(vault: &mut UnlockedVault, store_id: &str, provider: ExchangeProvider) {
    store_mut(vault, store_id).exchange = Some(ExchangeProfile {
        provider,
        ..ExchangeProfile::default()
    });
}

/// The mark the module leaves on an exchange it reads through its key.
/// At level 3 the secret itself lives in the device file and the document
/// keeps this pointer only; the open chain rebuilds it
/// (`adopt_device_secrets`) before the archiving below looks at it.
fn keyed(vault: &mut UnlockedVault, store_id: &str) {
    vault.doc.exchanges.bybit.push(ExchangeKeys {
        store_id: store_id.to_string(),
        api_key: "abcd1234wxyz".into(),
        api_secret: "test-secret-no-real-key".into(),
        spot_store_id: None,
        futures_store_id: None,
    });
}

/// A wallet with 2 ETH at 2000 in it, kept the way it was before the
/// module existed.
fn hand_kept(vault: &mut UnlockedVault, name: &str, kind: StoreKind) -> String {
    let wallet = store(vault, name, kind, None);
    vault
        .add_transaction(income_of(&wallet, eth_ref(), "2", "2000", "bought"))
        .expect("the purchase is booked");
    wallet
}

fn store_mut<'vault>(
    vault: &'vault mut UnlockedVault,
    store_id: &str,
) -> &'vault mut crate::model::store::Store {
    vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == store_id)
        .expect("the wallet exists")
}

fn archived(vault: &UnlockedVault, store_id: &str) -> bool {
    vault
        .find_store(store_id)
        .expect("the wallet exists")
        .archived
}

#[test]
fn the_plan_counts_only_the_crypto_kept_by_hand() {
    let (_dir, mut vault) = empty_vault();
    quote(&mut vault, &eth_ref(), "2000");
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    vault
        .add_transaction(income(&bank, "1000", "salary"))
        .expect("the top-up is booked");
    let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
    let binance = hand_kept(&mut vault, "Old exchange", StoreKind::Exchange);
    provider(&mut vault, &binance, ExchangeProvider::Binance);
    let bybit = hand_kept(&mut vault, "Bybit", StoreKind::Exchange);
    provider(&mut vault, &bybit, ExchangeProvider::Bybit);
    keyed(&mut vault, &bybit);
    let fresh = store(&mut vault, "New cold", StoreKind::Crypto, None);

    let plan = vault.crypto_transition_plan();

    let named: Vec<&str> = plan
        .wallets
        .iter()
        .map(|wallet| wallet.name.as_str())
        .collect();
    assert_eq!(
        named,
        vec!["Old cold", "Old exchange"],
        "an exchange the module reads is not retired, an empty wallet has nothing to retire, and fiat is not crypto"
    );
    assert!(
        !plan.wallets.iter().any(|wallet| wallet.store_id == fresh),
        "a wallet with nothing written in it is not a record of the past"
    );
    assert_eq!(plan.base_currency, "usd");
    assert_eq!(
        plan.leaving_base,
        amount("8000"),
        "the number the wizard warns with: both hand-kept wallets"
    );
    assert_eq!(plan.wallets[0].store_id, old);
    assert_eq!(plan.wallets[1].store_id, binance);
}

#[test]
fn the_move_archives_what_it_named_and_leaves_the_engines_wallet() {
    let (_dir, mut vault) = empty_vault();
    quote(&mut vault, &eth_ref(), "2000");
    let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
    let bybit = hand_kept(&mut vault, "Bybit", StoreKind::Exchange);
    provider(&mut vault, &bybit, ExchangeProvider::Bybit);
    keyed(&mut vault, &bybit);
    vault.raise_to_level3_for_tests();

    assert!(archived(&vault, &old), "the raise retires the old record");
    assert!(
        !archived(&vault, &bybit),
        "the wallet the module reads stays: it is a source, not history"
    );
    assert_eq!(
        vault
            .doc
            .transactions
            .iter()
            .filter(|tx| tx.touches(&old))
            .count(),
        1,
        "nothing is deleted: the archive stays readable"
    );

    // Idempotent: the wizard, the raise and the next open all run it.
    let again = vault
        .crypto_archive_manual_wallets()
        .expect("a second run saves nothing");
    assert!(!again.changed);
    assert!(again.wallets.is_empty());
    assert!(
        vault.crypto_transition_plan().wallets.is_empty(),
        "and the plan has nothing left to warn about"
    );
}

#[test]
fn below_level_three_nothing_is_retired() {
    let (_dir, mut vault) = empty_vault();
    quote(&mut vault, &eth_ref(), "2000");
    let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);

    let report = vault
        .crypto_archive_manual_wallets()
        .expect("the call is allowed, it just has nothing to do");

    assert!(!report.changed);
    assert!(!archived(&vault, &old));
    assert_eq!(
        vault.net_worth().financial,
        amount("4000"),
        "the capital of a level-1 vault is untouched"
    );
    assert_eq!(
        vault.crypto_transition_plan().leaving_base,
        amount("4000"),
        "the plan answers at any level: that is what the warning is for"
    );
}

/// A sub-account carries its parent's kind, so the whole tree of a keyless
/// exchange is retired and no part of a keyed one is.
#[test]
fn the_sub_accounts_follow_their_exchange() {
    let (_dir, mut vault) = empty_vault();
    quote(&mut vault, &eth_ref(), "2000");
    let binance = store(&mut vault, "Old exchange", StoreKind::Exchange, None);
    provider(&mut vault, &binance, ExchangeProvider::Binance);
    // Only the sub-account has the records: the family goes with it.
    let binance_spot = store(&mut vault, "Spot", StoreKind::Exchange, Some(&binance));
    vault
        .add_transaction(income_of(&binance_spot, eth_ref(), "2", "2000", "bought"))
        .expect("the purchase is booked");
    let bybit = store(&mut vault, "Bybit", StoreKind::Exchange, None);
    provider(&mut vault, &bybit, ExchangeProvider::Bybit);
    let bybit_spot = store(&mut vault, "Spot", StoreKind::Exchange, Some(&bybit));
    vault
        .add_transaction(income_of(&bybit_spot, eth_ref(), "2", "2000", "bought"))
        .expect("the purchase is booked");
    vault.raise_to_level3_for_tests();

    assert!(
        archived(&vault, &binance),
        "the exchange goes with the account that held the records"
    );
    assert!(archived(&vault, &binance_spot));
    assert!(!archived(&vault, &bybit));
    assert!(
        !archived(&vault, &bybit_spot),
        "the module reads this exchange, and that covers every account under it"
    );
}

/// The wallet the user is connecting: an exchange made just before the
/// move, because its key cannot be stored until level 3. The move must
/// not retire it, and a second device — which never receives the key —
/// must not either.
#[test]
fn the_exchange_being_connected_is_not_retired() {
    let (_dir, mut vault) = empty_vault();
    quote(&mut vault, &eth_ref(), "2000");
    let bybit = hand_kept(&mut vault, "Bybit", StoreKind::Exchange);
    provider(&mut vault, &bybit, ExchangeProvider::Bybit);
    let cold = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
    vault.raise_to_level3_for_tests();

    assert!(
        !archived(&vault, &bybit),
        "no key yet, and it is the wallet the move was started for"
    );
    assert!(archived(&vault, &cold));
    // What the key would change: nothing here — it is already a source.
    keyed(&mut vault, &bybit);
    assert!(!vault.retire_manual_crypto_wallets().changed);
}

/// A raise cut short right after the level-3 file was written: the vault
/// as the crash left it, with the raise's mark on disk and the hand-kept
/// wallet not yet archived. Returns the wallet's id and the path.
fn cut_short_raise() -> (tempfile::TempDir, std::path::PathBuf, String) {
    use crate::crypto::kdf_profile::{KDF_MEM_KIB_MIN, KdfParams};
    use crate::service::security_level::RaiseStep;
    let (dir, mut vault) = empty_vault();
    let path = dir.path().join("test.wallet");
    quote(&mut vault, &eth_ref(), "2000");
    let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
    let password = vault.password.clone();
    let kdf = KdfParams::hardened(KDF_MEM_KIB_MIN, 1, 1);
    let cut = vault.raise_inner(
        &password,
        &password,
        kdf,
        |_| {},
        |step| {
            if step == RaiseStep::Persisted {
                Err(crate::error::Error::Unsupported)
            } else {
                Ok(())
            }
        },
    );
    assert!(cut.is_err(), "the raise stops after the file is written");
    assert!(!archived(&vault, &old), "the archiving was not reached");
    assert!(vault.doc.settings.crypto_raise_pending);
    drop(vault);
    (dir, path, old)
}

/// The acceptance line of spec E11: a crash between the re-encryption and
/// the archiving is repaired at the next open — and only that crash: the
/// raise leaves a mark, and the open acts on the mark, not on the level.
#[test]
fn an_open_finishes_an_archiving_the_raise_did_not_reach() {
    let (_dir, path, old) = cut_short_raise();

    let reopened = UnlockedVault::open(path, "pw").expect("the level-3 file opens");

    assert!(
        archived(&reopened, &old),
        "the open chain finishes the move the raise began"
    );
    assert_eq!(
        reopened.net_worth().financial,
        Amount::ZERO,
        "and the retired coins are out of the totals"
    );
    assert!(
        !reopened.doc.settings.crypto_raise_pending,
        "the mark goes with the archiving"
    );
}

/// The mark is cleared on disk, so the next open has nothing to finish:
/// a wallet the user restored after that stays restored.
#[test]
fn a_finished_archiving_is_not_repeated_by_the_next_open() {
    let (_dir, path, old) = cut_short_raise();
    let mut reopened = UnlockedVault::open(path.clone(), "pw").expect("opens");
    assert!(archived(&reopened, &old));
    store_mut(&mut reopened, &old).archived = false;
    reopened.recompute_holdings();
    reopened.persist().expect("the restored wallet is saved");
    drop(reopened);

    let again = UnlockedVault::open(path, "pw").expect("opens again");

    assert!(!again.doc.settings.crypto_raise_pending);
    assert!(!archived(&again, &old), "nothing archives it a second time");
    assert_eq!(again.net_worth().financial, amount("4000"));
}

/// A vault that was at level 3 before the module (stage 1 raised it for
/// the exchange keys) and holds a wallet kept by hand: no raise is cut
/// short, so opening it archives nothing and the capital is as it was.
#[test]
fn a_level_3_vault_without_the_mark_keeps_its_hand_kept_wallets() {
    let (dir, mut vault) = empty_vault();
    let path = dir.path().join("test.wallet");
    quote(&mut vault, &eth_ref(), "2000");
    vault.raise_to_level3_for_tests();
    assert!(!vault.doc.settings.crypto_raise_pending);
    let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
    let binance = store(&mut vault, "Old exchange", StoreKind::Exchange, None);
    provider(&mut vault, &binance, ExchangeProvider::Binance);
    vault
        .add_transaction(income_of(&binance, eth_ref(), "1", "2000", "bought"))
        .expect("the purchase is booked");
    vault.persist().expect("saved");
    let capital = vault.net_worth().financial;
    assert_eq!(capital, amount("6000"));
    drop(vault);

    let reopened = UnlockedVault::open(path.clone(), "pw").expect("opens");

    assert!(!archived(&reopened, &old));
    assert!(!archived(&reopened, &binance));
    assert_eq!(
        reopened.net_worth().financial,
        capital,
        "the capital is the same, and not counted twice"
    );
    // A second open changes nothing either.
    drop(reopened);
    let again = UnlockedVault::open(path, "pw").expect("opens");
    assert!(!archived(&again, &old));
    assert_eq!(again.net_worth().financial, capital);
}

/// A raise that runs to the end leaves no mark, in memory or on disk.
#[test]
fn a_finished_raise_leaves_no_mark() {
    let (dir, mut vault) = empty_vault();
    let path = dir.path().join("test.wallet");
    quote(&mut vault, &eth_ref(), "2000");
    let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
    vault.raise_to_level3_for_tests();

    assert!(archived(&vault, &old), "the raise archives at its end");
    assert!(!vault.doc.settings.crypto_raise_pending);
    drop(vault);
    let (doc, _) = crate::storage::VaultFile::open(&path, "pw").expect("reads back");
    assert!(!doc.settings.crypto_raise_pending, "and not on disk either");
}

/// A raise that fails before the level-3 file exists changes nothing: the
/// vault stays where it was, and no mark is left in memory to be saved.
#[test]
fn a_raise_that_never_reached_level_3_leaves_no_mark() {
    use crate::crypto::kdf_profile::{KDF_MEM_KIB_MIN, KdfParams};
    let (_dir, mut vault) = empty_vault();
    let password = vault.password.clone();
    // A wrong current password: refused before anything is written.
    let refused = vault.raise_inner(
        "not the password",
        &password,
        KdfParams::hardened(KDF_MEM_KIB_MIN, 1, 1),
        |_| {},
        |_| Ok(()),
    );
    assert!(refused.is_err());
    assert!(!vault.doc.settings.crypto_raise_pending);
}
