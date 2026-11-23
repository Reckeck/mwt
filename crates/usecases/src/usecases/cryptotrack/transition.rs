//! Entering the module: what the move retires, and the archiving itself
//! (spec E11). The decision lives here (which wallets are kept by hand and what
//! they hold, over the store tree and the prices); the one rule that works on
//! the document alone, `archive_wallets`, is in `cryptotrack::service::transition`.
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
//! (`Session::exchange_is_engine_driven`, the mark E11 reads) and an
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
//! itself ([`Session::raise_to_crypto`]) and by an open that finds the
//! raise's mark (`Settings::crypto_raise_pending`). That last one is what
//! makes a failure between the re-encryption and the archiving repair itself:
//! the raise sets the mark before it re-encrypts, so it is in the first
//! level-3 file, and clears it when the archiving is done. The open chain in
//! `service::mod` runs the step only when the mark is set and persists once
//! if any of its repairs changed something. A level-3 vault without the mark
//! (raised before the module existed) is never archived by an open: the
//! wallets it keeps by hand are the user's, and the wizard is the one way
//! they leave.

use std::collections::BTreeSet;

use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::cryptotrack::service::transition::archive_wallets;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::transition::{
    CryptoArchiveReport, CryptoRetiredWallet, CryptoTransitionPlan,
};
use crate::ipc_contract::security::SecurityLevel;
use crate::model::store::StoreKind;
use crate::model::vault::VaultDoc;
use crate::money::books::Books;
use crate::money::tree::{find_store, root_of};
use crate::service::Session;
use crate::usecases::planning::emergency::prune_emergency_fund;

use super::legacy_bybit::exchange_is_engine_driven;

/// What entering the module would change: the hand-kept crypto wallets the
/// move archives, and how much capital leaves the totals with them.
///
/// Computed every time and stored nowhere, so the wizard can ask for it at
/// any level. A wallet the module reads itself is not in the list — an
/// exchange with a working key stays and becomes a source of the engine,
/// and nothing of it leaves.
pub fn crypto_transition_plan(books: &Books<'_>) -> CryptoTransitionPlan {
    let (base_currency, fx) = books.base_and_fx();
    let wallets = manual_crypto_wallets(books, fx);
    let mut leaving = Amount::ZERO;
    for wallet in &wallets {
        leaving = math::add(leaving, wallet.leaving_base)
            .unwrap_or(Amount::new(rust_decimal::Decimal::MAX));
    }
    CryptoTransitionPlan {
        base_currency: base_currency.to_string(),
        wallets,
        leaving_base: leaving,
    }
}

/// Archive the crypto wallets kept by hand and save. What the wizard's
/// last step calls; idempotent, so calling it again writes nothing.
pub fn crypto_archive_manual_wallets(vault: &mut Session) -> Result<CryptoArchiveReport> {
    let report = retire_manual_crypto_wallets(vault);
    vault.transact(|tx| {
        tx.touch_if(report.changed);
        Ok(report)
    })
}

/// The archiving as one step of a chain: it sets `Store::archived` on the
/// hand-kept crypto wallets, deletes no record, saves nothing and reports
/// whether it changed anything. The caller decides when to write — the
/// open chain persists once for all its steps.
///
/// Below level 3 it does nothing: the module cannot follow an address
/// there, so there is nothing to hand the wallets over to.
pub(crate) fn retire_manual_crypto_wallets(vault: &mut Session) -> CryptoArchiveReport {
    let mut out = CryptoArchiveReport::default();
    if vault.security_level() != SecurityLevel::Crypto {
        return out;
    }
    {
        let books = vault.books();
        let (_, fx) = books.base_and_fx();
        out.wallets = manual_crypto_wallets(&books, fx);
    }
    if out.wallets.is_empty() {
        // Already archived, or nothing was ever kept by hand.
        return out;
    }
    let retired: BTreeSet<&str> = out
        .wallets
        .iter()
        .map(|wallet| wallet.store_id.as_str())
        .collect();
    archive_wallets(&mut vault.doc, &retired);
    // An archived wallet is no part of the emergency fund, exactly as
    // `Session::archive_store` leaves it.
    prune_emergency_fund(vault);
    // The flag is only half of it: the totals are read off `doc.holdings`,
    // and the yielding happens in the module's fold.
    crate::usecases::money::portfolio::recompute_holdings(vault);
    out.changed = true;
    out
}

/// The crypto wallets still kept by hand, with what each one holds today.
///
/// Three things decide it, and the order matters.
///
/// **A wallet the module reads is not a record of the past.** A followed
/// address or an exchange with a key stays, with every sub-account under
/// it ([`engine_drives`]).
///
/// **An exchange the module can read is not retired either**, with or
/// without a key today ([`exchange_the_module_reads`]). A key can
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
fn manual_crypto_wallets(books: &Books<'_>, fx: Amount) -> Vec<CryptoRetiredWallet> {
    let doc = books.doc();
    let mut families: BTreeSet<String> = BTreeSet::new();
    for store in &doc.stores {
        if store.archived || !matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange) {
            continue;
        }
        if !wallet_has_records(doc, &store.id) {
            continue;
        }
        let root = root_of(doc, &store.id);
        if engine_drives(doc, &root) {
            continue;
        }
        if exchange_the_module_reads(doc, &root) {
            continue;
        }
        families.insert(root);
    }
    doc.stores
        .iter()
        .filter(|store| !store.archived)
        .filter(|store| matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange))
        .filter(|store| families.contains(&root_of(doc, &store.id)))
        .map(|store| CryptoRetiredWallet {
            store_id: store.id.clone(),
            name: store.name.clone(),
            kind: store.kind,
            leaving_base: wallet_value_base(books, &store.id, fx),
        })
        .collect()
}

/// Whether anything is written down in this wallet: a holding or an
/// operation that names it. An empty wallet is not a record of the past,
/// whatever its kind.
fn wallet_has_records(doc: &VaultDoc, store_id: &str) -> bool {
    doc.holdings
        .iter()
        .any(|holding| holding.store_id == store_id)
        || doc
            .transactions
            .iter()
            .any(|transaction| transaction.touches(store_id))
}

/// Whether this wallet is an exchange the module can read at all: a
/// provider it talks to (`ExchangeProvider::supported`). Whether a key is
/// set today says nothing — a key lives on one device and arrives only
/// after the move — so the provider is the whole answer here.
fn exchange_the_module_reads(doc: &VaultDoc, store_id: &str) -> bool {
    let root = root_of(doc, store_id);
    find_store(doc, &root).is_some_and(|store| {
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
fn engine_drives(doc: &VaultDoc, store_id: &str) -> bool {
    if exchange_is_engine_driven(doc, store_id) {
        return true;
    }
    let mut current = find_store(doc, store_id);
    while let Some(store) = current {
        if store.crypto_account_id.is_some() {
            return true;
        }
        current = store
            .parent_id
            .as_deref()
            .and_then(|parent| find_store(doc, parent));
    }
    false
}

/// What a wallet's own holdings are worth in the base currency, counted the
/// way `service::worth` counts them: a derivative is notional and not the
/// user's money, so it is left out here as it is left out there.
fn wallet_value_base(books: &Books<'_>, store_id: &str, fx: Amount) -> Amount {
    let mut total = Amount::ZERO;
    for holding in &books.doc().holdings {
        if holding.store_id != store_id || holding.asset.is_derivative() {
            continue;
        }
        // A quantity from the network times a price may not fit a decimal:
        // the wallet then counts as the largest value, not as a panic.
        let value = math::mul(holding.quantity, books.price_usd(&holding.asset.key()))
            .and_then(|usd| math::mul(usd, fx))
            .unwrap_or(Amount::new(rust_decimal::Decimal::MAX));
        total = math::add(total, value).unwrap_or(Amount::new(rust_decimal::Decimal::MAX));
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{ExchangeProfile, ExchangeProvider, Store};

    #[test]
    fn an_exchange_of_a_provider_the_module_reads_is_read_with_or_without_a_key() {
        let mut doc = VaultDoc::default();
        let mut bybit = Store::new("Bybit", StoreKind::Exchange, "#000");
        bybit.exchange = Some(ExchangeProfile {
            provider: ExchangeProvider::Bybit,
            ..ExchangeProfile::default()
        });
        let mut binance = Store::new("Binance", StoreKind::Exchange, "#000");
        binance.exchange = Some(ExchangeProfile {
            provider: ExchangeProvider::Binance,
            ..ExchangeProfile::default()
        });
        let (bybit_id, binance_id) = (bybit.id.clone(), binance.id.clone());
        doc.stores = vec![bybit, binance];

        assert!(exchange_the_module_reads(&doc, &bybit_id));
        assert!(!exchange_the_module_reads(&doc, &binance_id));
        assert!(
            !engine_drives(&doc, &bybit_id),
            "no key and no followed account yet"
        );
    }
}
