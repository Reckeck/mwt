//! Entering the module: what the move retires, and the archiving itself
//! (spec E11).
//!
//! # What the move does
//!
//! Before the module there was only one way to keep a coin: a wallet of kind
//! `Crypto` or `Exchange` with hand-made operations in it. From level 3 on the
//! module reads the chain itself, so those records are history: the move
//! **archives** them. Nothing is deleted — not an operation, not a holding,
//! not a wallet — and the archive stays readable, which is the whole reason
//! archiving was chosen over anything else.
//!
//! A wallet the module reads itself stays, because it is not a record of the
//! past but a source of the present: a wallet that already shows a followed
//! address ([`crate::model::store::Store::crypto_account_id`]), an exchange
//! the module reads through its key
//! ([`UnlockedVault::exchange_is_engine_driven`], the mark E11 reads) and an
//! exchange of a provider the module talks to at all — the user's Bybit
//! wallet in particular, with or without a key today.
//!
//! That last one is a change to my own default of 2026-11-07 ("a keyless
//! exchange wallet counts as hand-kept"), and the reason is the order of
//! events: a key can only be stored at level 3, so the wallet the user is
//! connecting is made *before* the move and keyed *after* it. At the moment
//! of the raise every exchange is keyless and the rule cannot tell the one
//! being connected from an abandoned ledger — so it does not try. What a
//! Bybit wallet holds is not retired but converted into the module's own
//! operations (spec E7, decision 1 of the plan), which is the answer the plan
//! already gives for it. An exchange of a provider the module cannot read
//! (`Binance`, `Okx`) can hold no key on any device, so it is hand-kept
//! beyond doubt and is retired with the rest.
//!
//! An empty wallet is retired by nothing: there is no record in it to retire,
//! and the move is about what the user wrote down.
//!
//! # Why a wallet is archived and not emptied
//!
//! The capital is read off `doc.holdings`, and `service::worth` never looks at
//! `Store::archived`, so archiving alone would move nothing. What takes the
//! retired coins out of the totals is the fold of the module
//! (`super::totals::apply_crypto_holdings`): an archived, non-engine-driven
//! wallet of kind `Crypto` or `Exchange` yields its holdings to the addresses
//! the module follows. That is why this step ends in `recompute_holdings` —
//! the flag alone is not the answer the user sees.
//!
//! Which also means the capital drops at the moment the module starts
//! accounting for something, not a moment earlier: below level 3 there is no
//! followed address, so the same archived wallets still count. The plan says
//! what *will* leave, which is exactly what the wizard has to warn about.
//!
//! # Why it is idempotent
//!
//! The candidates are a function of the document — kind, `archived`,
//! `crypto_account_id`, the exchange keys — and an archived wallet is no
//! longer a candidate. So a second run finds nothing, changes nothing and
//! writes nothing, and the step can be run by the wizard, by the raise
//! itself ([`UnlockedVault::raise_to_crypto`]) and by every open of a level-3
//! vault. That last one is what makes a failure between the re-encryption and
//! the archiving repair itself: the open chain in `service::mod` runs this
//! step like its other repairs and persists once if any of them changed
//! something.

use std::collections::BTreeSet;

use crate::Amount;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::transition::{
    CryptoArchiveReport, CryptoRetiredWallet, CryptoTransitionPlan,
};
use crate::ipc_contract::security::SecurityLevel;
use crate::model::store::StoreKind;

use super::super::UnlockedVault;

impl UnlockedVault {
    /// What entering the module would change: the hand-kept crypto wallets the
    /// move archives, and how much capital leaves the totals with them.
    ///
    /// Computed every time and stored nowhere, so the wizard can ask for it at
    /// any level. A wallet the module reads itself is not in the list — an
    /// exchange with a working key stays and becomes a source of the engine,
    /// and nothing of it leaves.
    pub fn crypto_transition_plan(&self) -> CryptoTransitionPlan {
        let (base_currency, fx) = self.base_and_fx();
        let wallets = self.manual_crypto_wallets(fx);
        let mut leaving = Amount::ZERO;
        for wallet in &wallets {
            leaving += wallet.leaving_base;
        }
        CryptoTransitionPlan {
            base_currency,
            wallets,
            leaving_base: leaving,
        }
    }

    /// Archive the crypto wallets kept by hand and save. What the wizard's
    /// last step calls; idempotent, so calling it again writes nothing.
    pub fn crypto_archive_manual_wallets(&mut self) -> Result<CryptoArchiveReport> {
        let report = self.retire_manual_crypto_wallets();
        if report.changed {
            self.persist()?;
        }
        Ok(report)
    }

    /// The archiving as one step of a chain: it sets `Store::archived` on the
    /// hand-kept crypto wallets, deletes no record, saves nothing and reports
    /// whether it changed anything. The caller decides when to write — the
    /// open chain persists once for all its steps.
    ///
    /// Below level 3 it does nothing: the module cannot follow an address
    /// there, so there is nothing to hand the wallets over to.
    pub(in crate::service) fn retire_manual_crypto_wallets(&mut self) -> CryptoArchiveReport {
        let mut out = CryptoArchiveReport::default();
        if self.security_level() != SecurityLevel::Crypto {
            return out;
        }
        let (_, fx) = self.base_and_fx();
        out.wallets = self.manual_crypto_wallets(fx);
        if out.wallets.is_empty() {
            // Already archived, or nothing was ever kept by hand.
            return out;
        }
        let retired: BTreeSet<&str> = out
            .wallets
            .iter()
            .map(|wallet| wallet.store_id.as_str())
            .collect();
        for store in &mut self.doc.stores {
            if retired.contains(store.id.as_str()) {
                store.archived = true;
            }
        }
        // An archived wallet is no part of the emergency fund, exactly as
        // `UnlockedVault::archive_store` leaves it.
        self.prune_emergency_fund();
        // The flag is only half of it: the totals are read off `doc.holdings`,
        // and the yielding happens in the module's fold.
        self.recompute_holdings();
        out.changed = true;
        out
    }

    /// The crypto wallets still kept by hand, with what each one holds today.
    ///
    /// Three things decide it, and the order matters.
    ///
    /// **A wallet the module reads is not a record of the past.** A followed
    /// address or an exchange with a key stays, with every sub-account under
    /// it ([`Self::engine_drives`]).
    ///
    /// **An exchange the module can read is not retired either**, with or
    /// without a key today ([`Self::exchange_the_module_reads`]). A key can
    /// only be stored at level 3, so the wallet the user is connecting is
    /// made *before* the move and gets its key *after* it: at the moment of
    /// the raise every exchange is keyless, and the rule cannot tell the one
    /// being connected from an abandoned ledger. It does not have to — what
    /// such a wallet holds is not retired but *converted* into the module's
    /// own operations (spec E7, `legacy_bybit.rs`), which is the plan's own
    /// answer for Bybit. An exchange of a provider the module cannot read
    /// (`Binance`, `Okx`) can hold no key on any device, so it is hand-kept
    /// beyond doubt and goes with the rest. A key never syncs, so this is
    /// also what keeps a second device from archiving a working exchange it
    /// simply cannot see the key of.
    ///
    /// **A wallet with nothing written in it has nothing to retire.** An
    /// empty wallet is not a record of the past, whatever its kind, and the
    /// move retires what the user wrote down.
    ///
    /// **A family goes together.** Records anywhere under an exchange retire
    /// the exchange and every account in it, so a spot sub-account with the
    /// fills does not leave an empty futures one behind in the totals.
    fn manual_crypto_wallets(&self, fx: Amount) -> Vec<CryptoRetiredWallet> {
        let mut families: BTreeSet<String> = BTreeSet::new();
        for store in &self.doc.stores {
            if store.archived || !matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange) {
                continue;
            }
            if !self.wallet_has_records(&store.id) {
                continue;
            }
            let root = self.root_of(&store.id);
            if self.engine_drives(&root) {
                continue;
            }
            if self.exchange_the_module_reads(&root) {
                continue;
            }
            families.insert(root);
        }
        self.doc
            .stores
            .iter()
            .filter(|store| !store.archived)
            .filter(|store| matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange))
            .filter(|store| families.contains(&self.root_of(&store.id)))
            .map(|store| CryptoRetiredWallet {
                store_id: store.id.clone(),
                name: store.name.clone(),
                kind: store.kind,
                leaving_base: self.wallet_value_base(&store.id, fx),
            })
            .collect()
    }

    /// Whether anything is written down in this wallet: a holding or an
    /// operation that names it. An empty wallet is not a record of the past,
    /// whatever its kind.
    fn wallet_has_records(&self, store_id: &str) -> bool {
        self.doc
            .holdings
            .iter()
            .any(|holding| holding.store_id == store_id)
            || self
                .doc
                .transactions
                .iter()
                .any(|transaction| transaction.touches(store_id))
    }

    /// Whether this wallet is an exchange the module can read at all: a
    /// provider it talks to (`ExchangeProvider::supported`). Whether a key is
    /// set today says nothing — a key lives on one device and arrives only
    /// after the move — so the provider is the whole answer here.
    fn exchange_the_module_reads(&self, store_id: &str) -> bool {
        let root = self.root_of(store_id);
        self.find_store(&root).is_some_and(|store| {
            store.kind == StoreKind::Exchange
                && store
                    .exchange
                    .as_ref()
                    .is_some_and(|profile| profile.provider.supported())
        })
    }

    /// Whether the module reads this wallet instead of the user keeping it by
    /// hand: the wallet (or one it sits in) shows a followed address, or it is
    /// an exchange the module reads through its key.
    fn engine_drives(&self, store_id: &str) -> bool {
        if self.exchange_is_engine_driven(store_id) {
            return true;
        }
        let mut current = self.find_store(store_id);
        while let Some(store) = current {
            if store.crypto_account_id.is_some() {
                return true;
            }
            current = store
                .parent_id
                .as_deref()
                .and_then(|parent| self.find_store(parent));
        }
        false
    }

    /// What a wallet's own holdings are worth in the base currency, counted the
    /// way `service::worth` counts them: a derivative is notional and not the
    /// user's money, so it is left out here as it is left out there.
    fn wallet_value_base(&self, store_id: &str, fx: Amount) -> Amount {
        let mut total = Amount::ZERO;
        for holding in &self.doc.holdings {
            if holding.store_id != store_id || holding.asset.is_derivative() {
                continue;
            }
            total += holding.quantity * self.price_usd_for(&holding.asset.key()) * fx;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::store::AssetRef;
    use crate::model::store::{ExchangeProfile, ExchangeProvider};
    use crate::model::vault::{ExchangeKeys, PricePoint};
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

    /// The acceptance line of spec E11: a crash between the re-encryption and
    /// the archiving is repaired at the next open.
    #[test]
    fn an_open_finishes_an_archiving_the_raise_did_not_reach() {
        let (dir, mut vault) = empty_vault();
        let path = dir.path().join("test.wallet");
        quote(&mut vault, &eth_ref(), "2000");
        vault.raise_to_level3_for_tests();
        // The vault as the crash left it: level 3 on the disk, the hand-kept
        // wallet still counting.
        let old = hand_kept(&mut vault, "Old cold", StoreKind::Crypto);
        vault
            .persist()
            .expect("the state before the repair is saved");
        assert_eq!(vault.net_worth().financial, amount("4000"));
        drop(vault);

        let reopened = UnlockedVault::open(path, "pw").expect("the level-3 file opens");

        assert!(
            archived(&reopened, &old),
            "the open chain runs the same step and finishes the move"
        );
        assert_eq!(
            reopened.net_worth().financial,
            Amount::ZERO,
            "and the retired coins are out of the totals"
        );
    }
}
