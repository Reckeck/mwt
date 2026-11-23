//! Scenarios of the engine's writes into the vault (spec E5, E8): storing a
//! source's batch, snapshotting the value and re-deriving the holdings the
//! capital is read from. The rules (how an operation is merged, what the
//! node's balance asks for, what a snapshot is) and the queries (the fold,
//! the totals, the capital) are in `cryptotrack::service::recompute` and
//! `cryptotrack::service::totals`.
//!
//! A pass edits the document through the rules and ends in the one write:
//! `transact(touch_if(changed))`, however many tokens, operations and
//! snapshots it met (a batch can meet hundreds of junk tokens, and each save
//! is a full encryption under the vault's lock). The balances the capital is
//! read from live in `doc.holdings`, which a save does not re-derive
//! (spec E8), so a pass that changed what the module holds folds them again
//! ([`refold_holdings`]) before the write.

use crate::cryptotrack::engine;
use crate::cryptotrack::service::recompute::{self as rules, CryptoApplied, CryptoBatch};
use crate::cryptotrack::service::totals;
use crate::cryptotrack::sources::evm::SourceBatch;
use crate::error::Result;
use crate::ipc_contract::security::SecurityLevel;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoSnapshot, TokenId};
use crate::service::Session;

/// Store a source's batch, fold everything again, reconcile with the
/// network and snapshot the value. Saves once, and only if the batch
/// changed something (a pass that met nothing new writes nothing).
pub fn crypto_apply_batch(
    vault: &mut Session,
    request: &CryptoBatch<'_>,
    batch: &SourceBatch,
) -> Result<CryptoApplied> {
    let (out, changed) = apply_batch_unsaved(vault, request, batch)?;
    vault.transact(|tx| {
        tx.touch_if(changed);
        Ok(out)
    })
}

/// [`crypto_apply_batch`] without the save: the caller owns it. Returns what
/// was applied and whether the document changed.
pub(crate) fn apply_batch_unsaved(
    vault: &mut Session,
    request: &CryptoBatch<'_>,
    batch: &SourceBatch,
) -> Result<(CryptoApplied, bool)> {
    let target = rules::batch_target(&vault.doc, request)?;
    let made = engine::normalize(
        &batch.raw_events,
        engine::Followed {
            account_id: request.account_id,
            chain: target.chain,
            address: &target.address,
        },
    );
    // Every token the batch met, registered before the fold so a
    // quarantined one is already out of the totals.
    let mut changed = register_met_tokens(vault, target.chain, batch)?;
    let (mut out, stored) = rules::store_events(&mut vault.doc, made, batch.rejected.len());
    changed |= stored;

    if request.history_done {
        let (events, corrected) = rules::reconcile(&vault.books(), request, &target, batch);
        out.corrected = corrected;
        changed |= rules::store_adjustments(&mut vault.doc, events);
    }

    let (snapshots, snapshots_changed) = take_snapshot(vault, request.read_at);
    out.snapshots = snapshots;
    changed |= snapshots_changed;
    changed |= refold_holdings(vault);
    Ok((out, changed))
}

/// Note each token of the batch once; saved with the rest of the batch, not
/// once per token.
fn register_met_tokens(vault: &mut Session, chain: Chain, batch: &SourceBatch) -> Result<bool> {
    let mut changed = false;
    for met in rules::met_tokens(batch).into_values() {
        let (_, registered) = super::tokens::register_token_unsaved(
            vault,
            &TokenId::new(chain, met.contract),
            met.symbol,
            met.name,
            met.decimals,
        )?;
        changed |= registered;
    }
    Ok(changed)
}

/// Write the value of each account and of the module for the day of `at`,
/// replacing that day's snapshots. Does not save on its own. Returns how
/// many it wrote and whether any of them differs from what was kept.
fn take_snapshot(vault: &mut Session, at: &str) -> (usize, bool) {
    let records = rules::snapshot_records(&vault.books(), at);
    rules::store_snapshots(&mut vault.doc, records)
}

/// Write the snapshots of `at` and save, if any of them is new.
pub fn crypto_snapshot(vault: &mut Session, at: &str) -> Result<Vec<CryptoSnapshot>> {
    let (_, changed) = take_snapshot(vault, at);
    vault.transact(|tx| {
        tx.touch_if(changed);
        Ok(())
    })?;
    Ok(rules::snapshots_of_day(&vault.doc, at))
}

/// Put the engine's balances into `doc.holdings`, replacing what they
/// supersede. Called at the end of `recompute_holdings`; saves nothing.
pub(crate) fn apply_crypto_holdings(vault: &mut Session) {
    // A vault without the module keeps `doc.holdings` exactly as the
    // transactions left it: nothing here speaks for it, so a wallet
    // archived long ago counts as it always did. The module is there once
    // the vault is at level 3 — hand-kept crypto has no place from that
    // point on, and the move retires it before the first address is
    // connected — or once an address is followed, which is the only state
    // a vault below level 3 can be in (a level-2 vault cannot hold one).
    if vault.doc.crypto_accounts.is_empty() && vault.security_level() != SecurityLevel::Crypto {
        return;
    }
    let made = totals::capital(&vault.books()).holdings;
    let retired = totals::crypto_retired_wallets(&vault.doc);
    if made.is_empty() && retired.is_empty() {
        return;
    }
    totals::supersede_holdings(&mut vault.doc, made, &retired);
}

/// Fold the engine again into `doc.holdings` and save: what the module's
/// own writers call after they changed an event, a verdict or an account,
/// since nothing else re-derives the holdings for them.
pub fn crypto_sync_holdings(vault: &mut Session) -> Result<()> {
    // Unconditional: callers rely on this save for their own edits, which
    // the refold comparison knows nothing about.
    refold_holdings(vault);
    vault.transact(|tx| {
        tx.touch();
        Ok(())
    })
}

/// Fold the engine again into `doc.holdings` without saving. Reports
/// whether the positions differ from before.
pub(crate) fn refold_holdings(vault: &mut Session) -> bool {
    let before = std::mem::take(&mut vault.doc.holdings);
    crate::usecases::money::portfolio::recompute_holdings(vault);
    totals::holdings_differ(&before, &vault.doc.holdings)
}
