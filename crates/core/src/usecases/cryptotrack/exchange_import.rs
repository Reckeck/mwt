//! Scenarios of the exchange import and the exchange terminal: the key and its
//! targets, staging, confirming and discarding what was staged, the account
//! set-up and the terminal's state. Where a fill is booked and what it becomes
//! is in `cryptotrack::exchange_import` (pure: it reads a `Books` and writes
//! nothing); the HTTP stays behind `ExchangeClient`.
//!
//! Every write is one `transact`. Confirming also re-derives the holdings,
//! which needs the vault, so it edits through `run` and commits once with
//! them (`rebase_and_commit`).

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::cryptotrack::exchange_import::{
    self as rules, bound_child, bybit_account_store, exchange_profile_mut,
};
use crate::cryptotrack::exchange_sync::find_store;
use crate::error::{Error, Result};
use crate::exchange::CoinBalance;
use crate::ipc_contract::{ExchangeAccountBalance, ExchangeCoinRow, ExchangeKeyStatus};
use crate::model::store::{MarketKind, StoreKind, SubType, TerminalState};
use crate::model::vault::PendingImport;
use crate::money::books::Books;
use crate::money::tree;
use crate::service::{MAX_PENDING_IMPORTS, StoreExtras, UnlockedVault};
use crate::usecases::money::stores::rebase_and_commit;
use mwt_cryptotrack::exchange_sync::TerminalPatch;

/// Status of the Bybit key stored for exactly this store.
pub fn exchange_key_status(vault: &UnlockedVault, store_id: &str) -> ExchangeKeyStatus {
    vault
        .doc
        .exchanges
        .bybit
        .iter()
        .find(|entry| entry.store_id == store_id)
        .map(|entry| ExchangeKeyStatus {
            configured: true,
            hint: Some(vault.exchange_key_hint(entry)),
            spot_store_id: entry.spot_store_id.clone(),
            futures_store_id: entry.futures_store_id.clone(),
        })
        .unwrap_or_default()
}

/// Store (or replace) a Bybit key for an exchange store in the device
/// secrets file, unverified (the app runs the read-only check first and
/// stores it verified). Level 3 only. Sub-account targets of a replaced key
/// are kept.
pub fn set_exchange_key(
    vault: &mut UnlockedVault,
    store_id: &str,
    api_key: &str,
    api_secret: &str,
) -> Result<ExchangeKeyStatus> {
    vault.store_device_key(store_id, api_key, api_secret, false)?;
    Ok(exchange_key_status(vault, store_id))
}

/// Point imported spot / futures fills at sub-accounts of the key's
/// store. Each target must be an exchange store inside that wallet.
pub fn set_exchange_targets(
    vault: &mut UnlockedVault,
    store_id: &str,
    spot_store_id: Option<String>,
    futures_store_id: Option<String>,
) -> Result<ExchangeKeyStatus> {
    let mut allowed: HashSet<String> = tree::subtree_ids(&vault.doc, store_id)
        .into_iter()
        .collect();
    allowed.insert(store_id.to_string());
    for target in [&spot_store_id, &futures_store_id].into_iter().flatten() {
        let ok = allowed.contains(target)
            && tree::find_store(&vault.doc, target)
                .is_some_and(|store| store.kind == StoreKind::Exchange && !store.archived);
        if !ok {
            return Err(Error::InvalidInput(
                "fills can only go to an exchange sub-account of this wallet".into(),
            ));
        }
    }
    vault.transact(|tx| {
        let entry = tx
            .doc_mut()
            .exchanges
            .bybit
            .iter_mut()
            .find(|entry| entry.store_id == store_id)
            .ok_or_else(|| Error::Missing("exchange key".into()))?;
        entry.spot_store_id = spot_store_id;
        entry.futures_store_id = futures_store_id;
        Ok(())
    })?;
    Ok(exchange_key_status(vault, store_id))
}

/// Stage fills for user confirmation, once per `exec_id`: re-importing the
/// same history is idempotent. Nothing here touches `transactions` — the
/// user has to confirm first. Returns how many fills were newly staged.
pub fn stage_imports(vault: &mut UnlockedVault, imports: Vec<PendingImport>) -> Result<u32> {
    let staged = rules::plan_staging(
        &vault.books(),
        imports,
        vault.doc.pending_imports.len(),
        MAX_PENDING_IMPORTS,
        |store_id| vault.ensure_store(store_id).is_ok(),
    );
    let count = staged.len() as u32;
    if count > 0 {
        vault.transact(|tx| {
            tx.doc_mut().pending_imports.extend(staged);
            Ok(())
        })?;
    }
    Ok(count)
}

/// Turn staged fills into real trades.
///
/// An empty `exec_ids` confirms everything staged. Confirmed entries leave
/// the staging area so they cannot be imported twice; a fill that fails
/// validation (its store was archived, say) stays staged. The queue and the
/// transactions change in one edit and the holdings are re-derived before the
/// one write.
pub fn confirm_pending_imports(vault: &mut UnlockedVault, exec_ids: &[String]) -> Result<u32> {
    let plan = rules::plan_confirmation(&vault.books(), exec_ids, |tx| {
        crate::usecases::household::validate::validate_transaction(vault, tx).is_ok()
    });
    let confirmed = plan.booked();
    vault.run(|run| {
        rules::apply_confirmation(run.doc_mut(), plan);
        Ok(())
    })?;
    if confirmed > 0 {
        rebase_and_commit(vault)?;
    } else {
        vault.transact(|tx| {
            tx.touch();
            Ok(())
        })?;
    }
    Ok(confirmed)
}

/// Drop staged fills without creating transactions; their ids are
/// remembered. An empty `exec_ids` discards everything.
pub fn discard_pending_imports(vault: &mut UnlockedVault, exec_ids: &[String]) -> Result<u32> {
    if rules::pending_named(&vault.doc, exec_ids) == 0 {
        return Ok(0);
    }
    vault.transact(|tx| Ok(rules::discard_pending(tx.doc_mut(), exec_ids)))
}

/// Give a Bybit wallet its three accounts — Funding, Unified Trading and
/// Earn — creating the ones missing. Returns the ids `(funding, unified, earn)`.
pub fn ensure_bybit_accounts(
    vault: &mut UnlockedVault,
    store_id: &str,
    names: [&str; 3],
) -> Result<[String; 3]> {
    let ids = ensure_bybit_accounts_for(vault, store_id, names, [true; 3])?;
    Ok(ids.map(Option::unwrap_or_default))
}

/// [`ensure_bybit_accounts`] for the accounts `wanted` (funding, unified,
/// earn); the others are left alone.
pub fn ensure_bybit_accounts_for(
    vault: &mut UnlockedVault,
    store_id: &str,
    names: [&str; 3],
    wanted: [bool; 3],
) -> Result<[Option<String>; 3]> {
    let root = find_store(&vault.books(), store_id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
    if root.kind != StoreKind::Exchange || root.parent_id.is_some() {
        return Err(Error::InvalidInput("not an exchange wallet".into()));
    }
    let accounts = [
        (SubType::Funding, MarketKind::Funding),
        (SubType::Unified, MarketKind::Unified),
        (SubType::Earn, MarketKind::Earn),
    ];
    let mut ids: [Option<String>; 3] = Default::default();
    for (index, (sub_type, market)) in accounts.into_iter().enumerate() {
        let existing = bound_child(&vault.books(), store_id, market);
        ids[index] = match existing {
            Some(id) => Some(id),
            None if wanted[index] => Some(
                crate::usecases::money::stores::create_store_with(
                    vault,
                    names[index],
                    StoreKind::Exchange,
                    &root.color,
                    "",
                    Some(store_id.to_string()),
                    StoreExtras {
                        sub_type: Some(sub_type),
                        binding: Some(crate::model::store::Binding::Market(market)),
                        ..StoreExtras::default()
                    },
                )?
                .id,
            ),
            None => None,
        };
    }
    Ok(ids)
}

/// What each Bybit account holds per the API next to what the ledger
/// says its sub-account holds, coin by coin.
pub fn bybit_reconcile(
    books: &Books<'_>,
    store_id: &str,
    balances: &[(String, Vec<CoinBalance>)],
) -> Vec<ExchangeAccountBalance> {
    balances
        .iter()
        .map(|(account, coins)| {
            let target = bybit_account_store(books, store_id, account);
            let ledger: HashMap<String, Amount> = target
                .as_deref()
                .map(|id| {
                    books
                        .doc()
                        .holdings
                        .iter()
                        .filter(|holding| holding.store_id == id && !holding.asset.is_derivative())
                        .map(|holding| {
                            (holding.asset.symbol.to_ascii_uppercase(), holding.quantity)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut symbols: Vec<String> = coins.iter().map(|coin| coin.coin.clone()).collect();
            symbols.extend(ledger.keys().cloned());
            symbols.sort();
            symbols.dedup();
            let rows = symbols
                .into_iter()
                .map(|coin| {
                    let exchange = coins
                        .iter()
                        .find(|row| row.coin == coin)
                        .and_then(|row| Amount::parse(&row.amount).ok())
                        .unwrap_or(Amount::ZERO);
                    let booked = ledger.get(&coin).copied().unwrap_or(Amount::ZERO);
                    ExchangeCoinRow {
                        coin,
                        exchange,
                        ledger: booked,
                        diff: exchange - booked,
                    }
                })
                .collect();
            ExchangeAccountBalance {
                account: account.clone(),
                store_id: target,
                rows,
            }
        })
        .collect()
}

/// Background import interval of one exchange; `0` disables it, otherwise it
/// is kept between a minute and a day.
pub fn set_import_interval(
    vault: &mut UnlockedVault,
    store_id: &str,
    interval_sec: u32,
) -> Result<()> {
    vault.transact(|tx| {
        exchange_profile_mut(tx.doc_mut(), store_id)?.import_interval_sec =
            rules::clamp_interval(interval_sec);
        Ok(())
    })
}

/// Exchanges with a key on this device whose background import is on,
/// with their interval. Empty below level 3 (a stored key is not used
/// there) and when the device file does not open. The keys zeroize on drop.
pub fn import_schedule(
    vault: &UnlockedVault,
) -> Vec<(u32, crate::vault_io::device_secrets::DeviceKey)> {
    if !vault.keys_on_device() || vault.doc.exchanges.bybit.is_empty() {
        return Vec::new();
    }
    let Ok(secrets) = vault.device_secrets_for_request() else {
        return Vec::new();
    };
    secrets
        .bybit
        .into_iter()
        .filter(|entry| !entry.keys.api_key.is_empty() && !entry.keys.api_secret.is_empty())
        .filter_map(|entry| {
            let store = tree::find_store(&vault.doc, &entry.keys.store_id)?;
            let interval = store.exchange.as_ref()?.import_interval_sec;
            (!store.archived && interval > 0).then_some((interval, entry))
        })
        .collect()
}

/// `terminal` with the fields `patch` names replaced; only those change, so a
/// timeframe switch never overwrites the watchlist or the drawings.
fn patched(mut terminal: TerminalState, patch: TerminalPatch) -> TerminalState {
    if let Some(watchlist) = patch.watchlist {
        terminal.watchlist = rules::normalize_watchlist(watchlist);
    }
    if let Some(symbol) = patch.active_symbol {
        terminal.active_symbol = rules::terminal_symbol(&symbol);
    }
    if let Some(timeframe) = patch.timeframe {
        terminal.timeframe = timeframe;
    }
    if let Some(indicators) = patch.indicators {
        terminal.indicators = indicators;
    }
    if let Some(layout) = patch.layout {
        terminal.layout = layout;
    }
    terminal
}

/// Patch the terminal of one exchange. The terminal re-sends its state on
/// every symbol or timeframe click, so an unchanged state writes nothing.
pub fn update_terminal_state(
    vault: &mut UnlockedVault,
    store_id: &str,
    patch: TerminalPatch,
) -> Result<TerminalState> {
    let before = rules::terminal_state(&vault.doc, store_id)?;
    let after = patched(before.clone(), patch);
    if after != before {
        write_terminal(vault, store_id, after.clone())?;
    }
    Ok(after)
}

/// Replace one symbol's drawings; an empty export removes the entry.
pub fn save_terminal_drawings(
    vault: &mut UnlockedVault,
    store_id: &str,
    symbol: &str,
    json: &str,
) -> Result<()> {
    if json.len() > crate::model::store::MAX_DRAWINGS_JSON {
        return Err(Error::TooLarge("chart drawings".into()));
    }
    let symbol = rules::terminal_symbol(symbol)
        .ok_or_else(|| Error::InvalidInput("symbol is required".into()))?;
    let mut terminal = rules::terminal_state(&vault.doc, store_id)?;
    if rules::set_drawings(&mut terminal, &symbol, json) {
        write_terminal(vault, store_id, terminal)?;
    }
    Ok(())
}

/// Put `terminal` into the wallet's profile: one `transact`.
fn write_terminal(
    vault: &mut UnlockedVault,
    store_id: &str,
    terminal: TerminalState,
) -> Result<()> {
    vault.transact(|tx| {
        exchange_profile_mut(tx.doc_mut(), store_id)?.terminal = terminal;
        Ok(())
    })
}
