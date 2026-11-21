//! Scenarios of syncing an exchange wallet: the import settings, one sync,
//! the failed sync kept for the history and the live-balance correction. The
//! rules (which records are new, the history, the settings) are in
//! `cryptotrack::exchange_sync`; where a fill is booked is in
//! `cryptotrack::exchange_import`.
//!
//! Since spec E7 nothing is staged: a record becomes an operation of the
//! module at once. `ImportRun::staged` therefore stays `0` (the field keeps
//! its meaning and stops being written, `AGENTS.md` rule 5) and
//! `ImportRun::confirmed` counts what reached the module.

use crate::cryptotrack::exchange_import::{exchange_profile_mut, held_asset};
use crate::cryptotrack::exchange_sync::{
    self as rules, account_skipped, import_settings, last_run_index, root_of,
};
use crate::error::{Error, Result};
use crate::exchange::CoinBalance;
use crate::model::store::{ImportSettings, StoreKind};
use crate::model::transaction::{
    AdjustmentLine, AdjustmentTx, PriceStamp, Transaction, TxMeta, TxOrigin,
};
use crate::model::vault::{ImportRun, ImportSource, PendingImport};
use crate::money::validate::live_store;
use crate::service::UnlockedVault;
use crate::usecases::cryptotrack::exchange_import::bybit_reconcile;
use crate::usecases::cryptotrack::legacy_bybit::exchange_is_engine_driven;
use crate::usecases::cryptotrack::recompute::crypto_sync_holdings;

/// Change what an import brings in and where it books it: one `transact`.
pub fn set_import_settings(
    vault: &mut UnlockedVault,
    store_id: &str,
    settings: ImportSettings,
) -> Result<ImportSettings> {
    let root = root_of(&vault.books(), store_id);
    let settings = rules::normalize_settings(&vault.doc, settings, |target| {
        let store = live_store(&vault.doc, target)?;
        if store.kind != StoreKind::Exchange || root_of(&vault.books(), target) != root {
            return Err(Error::InvalidInput(
                "an exchange account books only into this wallet or its sub-accounts".into(),
            ));
        }
        Ok(())
    })?;
    vault.transact(|tx| {
        exchange_profile_mut(tx.doc_mut(), &root)?.import = settings.clone();
        Ok(())
    })?;
    Ok(settings)
}

/// One sync of an exchange wallet: leave out what the settings do not
/// want and what was seen before, record the rest as operations of the
/// crypto module, and keep the sync in the history.
pub fn sync_exchange(
    vault: &mut UnlockedVault,
    store_id: &str,
    imports: Vec<PendingImport>,
    source: ImportSource,
    now: i64,
) -> Result<ImportRun> {
    let (root, mut run, fresh, counts) = {
        let books = vault.books();
        let root = root_of(&books, store_id);
        let settings = import_settings(&books, &root)?;
        let mut run = rules::new_run(&root, source, now);
        run.fetched = imports.len() as u32;
        let taken = rules::intake(
            &books,
            imports,
            &settings,
            rules::known_import_ids(books.doc()),
        );
        (root, run, taken.fresh, (taken.known, taken.filtered))
    };
    (run.known, run.filtered) = counts;
    rules::count_kinds(&mut run, &fresh);
    let stored = vault.bybit_store(&root, &fresh)?;
    run.known += stored.known;
    run.confirmed = stored.recorded();
    // A poll that brought nothing new refreshes the last empty run in memory
    // and writes nothing: the background import polls every few minutes.
    if run.confirmed == 0 {
        rules::note_empty_run(&mut vault.doc, run.clone());
        return Ok(run);
    }
    rules::keep_import_run(&mut vault.doc, run.clone());
    // The fills became events of the module, and the capital is read from
    // `doc.holdings`, which a save does not re-derive (spec E8).
    crypto_sync_holdings(vault)?;
    Ok(run)
}

/// A sync that did not go through, for the history.
pub fn record_import_error(
    vault: &mut UnlockedVault,
    store_id: &str,
    source: ImportSource,
    error: &str,
    now: i64,
) -> Result<ImportRun> {
    let root = root_of(&vault.books(), store_id);
    let mut run = rules::new_run(&root, source, now);
    run.error = Some(error.to_string());
    vault.transact(|tx| {
        rules::keep_import_run(tx.doc_mut(), run.clone());
        Ok(())
    })?;
    Ok(run)
}

/// Set the live balance of each exchange account where the ledger
/// disagrees with it, by one correction per sub-account. Nothing happens
/// while operations of this wallet wait for confirmation (they explain
/// the difference) or when the settings turn it off. Returns the ids of
/// the corrections made.
///
/// A wallet the crypto module drives is never corrected (spec E7): its
/// balance is recomputed from the operations the module holds, so writing
/// the exchange's live balance into the ledger as well would count the
/// same coins twice. The old queue is what this was for, and it is still
/// served while one exists.
pub fn reconcile_exchange(
    vault: &mut UnlockedVault,
    store_id: &str,
    balances: &[(String, Vec<CoinBalance>)],
    now: i64,
) -> Result<Vec<String>> {
    let (root, corrections) = {
        let books = vault.books();
        let root = root_of(&books, store_id);
        let settings = import_settings(&books, &root)?;
        let waiting = books
            .doc()
            .pending_imports
            .iter()
            .any(|pending| root_of(&books, &pending.store_id) == root);
        if !settings.auto_reconcile || waiting || exchange_is_engine_driven(books.doc(), &root) {
            return Ok(Vec::new());
        }
        (
            root.clone(),
            corrections(&books, &root, &settings, balances, now),
        )
    };
    let mut made = Vec::new();
    for correction in corrections {
        let saved = vault.add_transaction(correction)?;
        made.push(saved.meta().id.clone());
    }
    if let Some(first) = made.first().cloned() {
        vault.transact(|tx| {
            if let Some(index) = last_run_index(tx.doc(), &root) {
                rules::mark_reconciled(tx.doc_mut(), index, first);
            }
            Ok(())
        })?;
    }
    Ok(made)
}

/// The corrections the live balances ask for, one per sub-account.
fn corrections(
    books: &crate::money::books::Books<'_>,
    root: &str,
    settings: &ImportSettings,
    balances: &[(String, Vec<CoinBalance>)],
    now: i64,
) -> Vec<Transaction> {
    let at = crate::calendar::iso(now);
    let mut found = Vec::new();
    for account in bybit_reconcile(books, root, balances) {
        let Some(target) = account.store_id.clone() else {
            continue;
        };
        if account_skipped(books, settings, Some(&account.account)) {
            continue;
        }
        let lines: Vec<AdjustmentLine> = account
            .rows
            .iter()
            .filter(|row| !row.diff.is_zero() && !row.exchange.is_negative())
            .filter(|row| {
                settings.coins.is_empty() || settings.coins.contains(&row.coin.to_ascii_uppercase())
            })
            .map(|row| AdjustmentLine {
                asset: held_asset(books.doc(), &target, &row.coin),
                before: row.ledger,
                after: row.exchange,
                price: PriceStamp::default(),
            })
            .collect();
        if lines.is_empty() {
            continue;
        }
        // A correction has no categories: the import origin marks it.
        let mut meta = TxMeta::new("Сверка с биржей: живой баланс");
        meta.timestamp = at.clone();
        meta.origin = TxOrigin::Import {
            exec_id: format!("reconcile:{}:{at}", account.account),
        };
        found.push(Transaction::Adjustment(AdjustmentTx {
            meta,
            store_id: target,
            lines,
            fee: None,
        }));
    }
    found
}
