//! Read-only transaction inspection and exchange prefill.
//!
//! Every command here is deliberately *soft-failing*: the trade form must stay
//! usable with no network, no API key and no provider support. So a lookup that
//! cannot be completed returns `fetched: false` (or an empty list) instead of an
//! error the user has to dismiss.

use tauri::State;

use trade_core::calendar::now_secs;
use trade_core::cryptotrack::exchange_sync::ImportedTransactions;
use trade_core::cryptotrack::{exchange_import as import_reads, exchange_sync as sync_reads};
use trade_core::error::Error;
use trade_core::exchange::bybit::KeyCheck;
use trade_core::exchange::staging::PendingImportExt;
use trade_core::exchange::{BybitClient, Category};
use trade_core::ipc_contract::{
    BybitExecution, BybitExecutionsInput, BybitExecutionsResult, ClearExchangeKeyInput,
    ConfirmPendingImportsInput, ConfirmPendingImportsOutput, CreateBybitAccountsInput,
    DiscardPendingImportsInput, DiscardPendingImportsOutput, ExchangeAccountBalance,
    ExchangeKeyStatus, ExchangeKeyStatusInput, ImportBybitHistoryInput, ImportBybitHistoryOutput,
    ImportPreviewOutput, InspectTxInput, InspectTxResult, ListImportedTransactionsInput,
    ListPendingImportsOutput, SaveTerminalDrawingsInput, SetExchangeKeyInput,
    SetExchangeTargetsInput, SetImportIntervalInput, SetImportSettingsInput, TxFieldHint,
    UpdateTerminalStateInput,
};
use trade_core::model::store::{ImportSettings, TerminalState};
use trade_core::model::vault::{ImportRun, ImportSource, PendingImport};
use trade_core::txinfo::{TxInfo, TxInfoService};
use trade_core::usecases::cryptotrack::{exchange_import, exchange_sync};
use trade_core::vault_io::device_secrets::DeviceKey;

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Look up a transaction hash and suggest form fields.
///
/// Never errors for a missing/unreachable provider — `fetched: false` with an
/// optional `notice` is the failure channel.
#[tauri::command]
pub async fn inspect_tx(
    input: InspectTxInput,
    state: State<'_, AppState>,
) -> Result<InspectTxResult, IpcError> {
    crate::commands::sandbox::refuse(&state, "looking up a transaction online")?;
    let hash = input.tx_hash.trim();
    if hash.is_empty() {
        return Ok(InspectTxResult {
            fetched: false,
            info: None,
            hints: Vec::new(),
            notice: Some("empty".into()),
        });
    }

    // `inspect` classifies offline and enriches best-effort; the fallback keeps
    // this command non-fatal even if that ever stops holding.
    let info = TxInfoService::default()
        .inspect(hash, input.network)
        .await
        .unwrap_or_else(|_| TxInfo::from_hash(hash));

    let hints = hints_from(&info);
    // Nothing actionable means the form should stay exactly as typed.
    let fetched = info.occurred_at.is_some()
        || info.amount_out.is_some()
        || info.amount_in.is_some()
        || info.fee.is_some();

    let notice = if info.needs_manual_network {
        Some("choose_network".into())
    } else if !fetched {
        Some("unavailable".into())
    } else {
        None
    };

    Ok(InspectTxResult {
        fetched,
        info: Some(info),
        hints,
        notice,
    })
}

/// Turn inspected data into concrete field suggestions for the form.
fn hints_from(info: &TxInfo) -> Vec<TxFieldHint> {
    let mut hints = Vec::new();
    let mut push = |field: &str, value: Option<String>| {
        if let Some(filled) = value.filter(|text| !text.trim().is_empty()) {
            hints.push(TxFieldHint {
                field: field.to_string(),
                value: filled,
            });
        }
    };

    push("occurredAt", info.occurred_at.clone());
    push("qtyBought", info.amount_out.clone());
    push("qtySold", info.amount_in.clone());
    push("assetBought", info.asset_out.clone());
    push("assetSold", info.asset_in.clone());
    push("fee", info.fee.clone());
    push("feeAsset", info.fee_asset.clone());
    push("status", info.status.clone());
    // `explorer_url` is deliberately not a hint: it is metadata for an
    // "open in explorer" link, not a value the trade form can consume.
    hints
}

/// The Bybit key serving a store (its own or its wallet's; `None` = any),
/// decrypted from the device file for this request and copied out so the
/// guard can be dropped before any network call. It zeroizes on drop.
/// Below level 3 a key still in the vault gives `SECURITY_LEVEL_REQUIRED`.
fn request_key(state: &AppState, store_id: Option<&str>) -> Result<Option<DeviceKey>, Error> {
    let guard = require_vault(state)?;
    guard.as_ref().unwrap().request_key(store_id)
}

/// A key saved while Bybit could not be asked is checked before its first
/// use: one that can trade or withdraw is refused, a failed check leaves it
/// unverified for the next time (the request then fails on its own).
async fn ensure_read_only(state: &AppState, key: &mut DeviceKey) -> Result<(), Error> {
    if key.verified {
        return Ok(());
    }
    match BybitClient::default()
        .check_key(&key.keys.api_key, &key.keys.api_secret)
        .await
    {
        Ok(KeyCheck::ReadOnly) => {
            let mut guard = require_vault(state)?;
            guard
                .as_mut()
                .unwrap()
                .mark_device_key_verified(&key.keys.store_id)?;
            key.verified = true;
            Ok(())
        }
        Ok(KeyCheck::NotReadOnly) => Err(Error::ExchangeKeyNotReadOnly),
        Err(err @ Error::ExchangeKeyRejected(_)) => Err(err),
        Err(_) => Ok(()),
    }
}

/// [`request_key`] then [`ensure_read_only`]: the key a request may use.
async fn usable_key(state: &AppState, store_id: Option<&str>) -> Result<Option<DeviceKey>, Error> {
    let Some(mut key) = request_key(state, store_id)? else {
        return Ok(None);
    };
    ensure_read_only(state, &mut key).await?;
    Ok(Some(key))
}

/// Recent Bybit fills for the prefill dropdown.
///
/// Empty list + `notice` when unconfigured or offline; never an error.
#[tauri::command]
pub async fn bybit_executions(
    input: BybitExecutionsInput,
    state: State<'_, AppState>,
) -> Result<BybitExecutionsResult, IpcError> {
    // No wallet in context (the global form): any key will do. The form
    // stays usable: a key it may not use (below level 3, refused, unreadable)
    // reads as no key here; the wallet's exchange tab says why.
    let store_id = input.store_id.as_deref().filter(|id| !id.is_empty());
    let key = match usable_key(&state, store_id).await {
        Ok(Some(key)) => key,
        Ok(None) | Err(_) => {
            return Ok(BybitExecutionsResult {
                executions: Vec::new(),
                notice: Some("no_key".into()),
            });
        }
    };

    let category: Category = input.category;
    let client = BybitClient::default();
    let list = client
        .executions(
            &key.keys.api_key,
            &key.keys.api_secret,
            category,
            input.symbol.as_deref(),
            input.limit.unwrap_or(20),
        )
        .await;

    match list {
        Ok(execs) => Ok(BybitExecutionsResult {
            executions: execs.into_iter().map(BybitExecution::from).collect(),
            notice: None,
        }),
        Err(err) => {
            // Surface a hint, but never block the form.
            let notice = if err.to_string().contains("403") {
                Some("forbidden_region".into())
            } else if err.to_string().contains("429") {
                Some("rate_limited".into())
            } else {
                Some("unreachable".into())
            };
            Ok(BybitExecutionsResult {
                executions: Vec::new(),
                notice,
            })
        }
    }
}

/// Whether a read-only Bybit key is stored for the given store.
/// The secret is never returned.
#[tauri::command]
pub async fn exchange_key_status(
    input: ExchangeKeyStatusInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyStatus, IpcError> {
    let guard = require_vault(&state)?;
    let vault = guard.as_ref().unwrap();
    match import_reads::bybit_key_for(&vault.books(), &input.store_id) {
        Some(entry) => Ok(exchange_import::exchange_key_status(vault, &entry.store_id)),
        None => Ok(ExchangeKeyStatus::default()),
    }
}

/// Store a read-only Bybit key for a store in this device's secrets file
/// (level 3 only: `SECURITY_LEVEL_REQUIRED` below it).
///
/// Bybit is asked about the key's rights first, with no lock held: a key
/// that can trade or withdraw is refused (`EXCHANGE_KEY_NOT_READ_ONLY`), a
/// key Bybit rejects (bad key or secret, no permission, IP not whitelisted)
/// is refused too (`EXCHANGE_KEY_REJECTED`); when
/// Bybit cannot be reached the key is kept as not verified and checked again
/// at the next import.
#[tauri::command]
pub async fn set_exchange_key(
    input: SetExchangeKeyInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyStatus, IpcError> {
    crate::commands::sandbox::refuse(&state, "an exchange key")?;
    let api_key = zeroize::Zeroizing::new(input.api_key.trim().to_string());
    let api_secret = zeroize::Zeroizing::new(input.api_secret.trim().to_string());
    {
        let guard = require_vault(&state)?;
        guard
            .as_ref()
            .unwrap()
            .check_exchange_key_input(&input.store_id, &api_key, &api_secret)?;
    }
    let verified = match BybitClient::default()
        .check_key(&api_key, &api_secret)
        .await
    {
        Ok(KeyCheck::ReadOnly) => true,
        Ok(KeyCheck::NotReadOnly) => return Err(Error::ExchangeKeyNotReadOnly.into()),
        Err(err @ Error::ExchangeKeyRejected(_)) => return Err(err.into()),
        Err(_) => false,
    };
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().unwrap();
    vault.store_device_key(&input.store_id, &api_key, &api_secret, verified)?;
    vault.persist()?;
    Ok(exchange_import::exchange_key_status(vault, &input.store_id))
}

/// Choose the sub-accounts imported spot and futures fills land on.
#[tauri::command]
pub async fn set_exchange_targets(
    input: SetExchangeTargetsInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyStatus, IpcError> {
    let mut guard = require_vault(&state)?;
    exchange_import::set_exchange_targets(
        guard.as_mut().unwrap(),
        &input.store_id,
        input.spot_store_id,
        input.futures_store_id,
    )
    .map_err(IpcError::from)
}

/// The exchange's own background import interval; `0` turns it off.
#[tauri::command]
pub async fn set_import_interval(
    input: SetImportIntervalInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    exchange_import::set_import_interval(
        guard.as_mut().unwrap(),
        &input.store_id,
        input.interval_sec,
    )
    .map_err(IpcError::from)
}

/// Everything the exchange's trading terminal remembers.
#[tauri::command]
pub async fn get_terminal_state(
    store_id: String,
    state: State<'_, AppState>,
) -> Result<TerminalState, IpcError> {
    let guard = require_vault(&state)?;
    import_reads::terminal_state(&guard.as_ref().unwrap().doc, &store_id).map_err(IpcError::from)
}

/// Patch the terminal: only the fields present change.
#[tauri::command]
pub async fn update_terminal_state(
    input: UpdateTerminalStateInput,
    state: State<'_, AppState>,
) -> Result<TerminalState, IpcError> {
    let mut guard = require_vault(&state)?;
    exchange_import::update_terminal_state(guard.as_mut().unwrap(), &input.store_id, input.patch)
        .map_err(IpcError::from)
}

/// Replace one symbol's chart drawings in the exchange's terminal.
#[tauri::command]
pub async fn save_terminal_drawings(
    input: SaveTerminalDrawingsInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    exchange_import::save_terminal_drawings(
        guard.as_mut().unwrap(),
        &input.store_id,
        &input.symbol,
        &input.json,
    )
    .map_err(IpcError::from)
}

/// Remove the stored Bybit key for a specific store.
#[tauri::command]
pub async fn clear_exchange_key(
    input: ClearExchangeKeyInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyStatus, IpcError> {
    let mut guard = require_vault(&state)?;
    guard.as_mut().unwrap().clear_device_key(&input.store_id)?;
    Ok(ExchangeKeyStatus::default())
}

// -- Bybit import staging (D2.4) -----------------------------------------
//
// All staging/dedup/confirm logic lives in `trade_core::service` so it is unit
// tested; these handlers only move data across the IPC boundary.

/// Machine-readable reason for a soft failure (never surfaces as an error).
fn import_notice(message: &str) -> &'static str {
    if message.contains("403") {
        "forbidden_region"
    } else if message.contains("429") {
        "rate_limited"
    } else {
        "unreachable"
    }
}

/// Give a Bybit wallet its Funding, Unified Trading and Earn sub-accounts;
/// the ones already there are kept. With `detect`, Bybit is asked which of
/// them hold anything first (the unified one is always created). Returns the
/// wallet's sub-account ids.
#[tauri::command]
pub async fn create_bybit_accounts(
    input: CreateBybitAccountsInput,
    state: State<'_, AppState>,
) -> Result<Vec<String>, IpcError> {
    let mut wanted = [true; 3];
    if input.detect {
        // Detection is a convenience: without a usable key all are created.
        if let Ok(Some(key)) = usable_key(&state, Some(&input.store_id)).await
            && let Ok(balances) = BybitClient::default()
                .balances(&key.keys.api_key, &key.keys.api_secret)
                .await
        {
            wanted = trade_core::exchange::bybit::accounts_in_use(&balances);
        }
    }
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().unwrap();
    let names = [
        input.funding_name.trim(),
        input.unified_name.trim(),
        input.earn_name.trim(),
    ];
    exchange_import::ensure_bybit_accounts_for(vault, &input.store_id, names, wanted)
        .map(|ids| ids.into_iter().flatten().collect())
        .map_err(IpcError::from)
}

/// What each Bybit account holds per the API, next to the ledger's figures.
/// When everything is booked and they still disagree, the live balance is
/// set by a correction first (unless the import settings turn that off).
/// An empty list when the wallet has no key; a network failure is an error.
#[tauri::command]
pub async fn bybit_balances(
    store_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ExchangeAccountBalance>, IpcError> {
    let Some(key) = usable_key(&state, Some(&store_id)).await? else {
        return Ok(Vec::new());
    };
    let root_id = key.keys.store_id.clone();
    let balances = BybitClient::default()
        .balances(&key.keys.api_key, &key.keys.api_secret)
        .await?;
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().unwrap();
    exchange_sync::reconcile_exchange(vault, &root_id, &balances, now_secs())?;
    Ok(exchange_import::bybit_reconcile(
        &vault.books(),
        &root_id,
        &balances,
    ))
}

/// One sync of a Bybit wallet: fetch fills, moves between accounts and Earn
/// payouts, let the import settings and the history of ids sort them, stage
/// (or book) the new ones, then set the live balance where the ledger still
/// disagrees. A failed fetch is kept in the history with its reason. Shared
/// by the background poll and "Sync now".
pub(crate) async fn sync_bybit(
    state: &AppState,
    mut key: DeviceKey,
    limit: u32,
    source: ImportSource,
) -> Result<ImportRun, IpcError> {
    ensure_read_only(state, &mut key).await?;
    let root_id = key.keys.store_id.as_str();
    let (api_key, api_secret) = (key.keys.api_key.as_str(), key.keys.api_secret.as_str());
    let client = BybitClient::default();
    let imports = match fetch_bybit(&client, root_id, api_key, api_secret, limit).await {
        Ok(imports) => imports,
        Err(message) => {
            let mut guard = state.vault.lock().map_err(|err| err.to_string())?;
            let vault = guard
                .as_mut()
                .ok_or("REQUIRES_UNLOCK: vault was locked mid-sync")?;
            return exchange_sync::record_import_error(
                vault,
                root_id,
                source,
                import_notice(&message),
                now_secs(),
            )
            .map_err(IpcError::from);
        }
    };
    let mut run = {
        let mut guard = state.vault.lock().map_err(|err| err.to_string())?;
        let vault = guard
            .as_mut()
            .ok_or("REQUIRES_UNLOCK: vault was locked mid-sync")?;
        exchange_sync::sync_exchange(vault, root_id, imports, source, now_secs())?
    };
    // The live balance is optional: a key without wallet rights still syncs.
    if let Ok(balances) = client.balances(api_key, api_secret).await {
        let mut guard = state.vault.lock().map_err(|err| err.to_string())?;
        let vault = guard
            .as_mut()
            .ok_or("REQUIRES_UNLOCK: vault was locked mid-sync")?;
        let made = exchange_sync::reconcile_exchange(vault, root_id, &balances, now_secs())?;
        run.reconcile_tx_id = made.into_iter().next();
    }
    Ok(run)
}

/// Fills, moves between accounts and Earn payouts of a Bybit key, shaped for
/// staging. Only the fills are required: a key without the rights for the
/// other two still imports its trades. The error is Bybit's message.
async fn fetch_bybit(
    client: &BybitClient,
    root_id: &str,
    api_key: &str,
    api_secret: &str,
    limit: u32,
) -> Result<Vec<PendingImport>, String> {
    let execs = client
        .all_executions(api_key, api_secret, limit)
        .await
        .map_err(|err| err.to_string())?;
    let transfers = client
        .inter_transfers(api_key, api_secret, limit as usize)
        .await
        .unwrap_or_default();
    let yields = client
        .earn_yields(api_key, api_secret, limit as usize)
        .await
        .unwrap_or_default();
    let fills: Vec<_> = execs.into_iter().take(limit as usize).collect();
    Ok(PendingImport::bybit_batch(
        root_id, &fills, &transfers, &yields,
    ))
}

/// What a sync would bring under the given import settings (the form's,
/// saved or not): fetched from Bybit, sorted, nothing staged.
#[tauri::command]
pub async fn preview_bybit_import(
    input: SetImportSettingsInput,
    state: State<'_, AppState>,
) -> Result<ImportPreviewOutput, IpcError> {
    let Some(key) = usable_key(&state, Some(&input.store_id)).await? else {
        return Ok(ImportPreviewOutput {
            preview: None,
            notice: Some("no_key".into()),
        });
    };
    match fetch_bybit(
        &BybitClient::default(),
        &key.keys.store_id,
        &key.keys.api_key,
        &key.keys.api_secret,
        500,
    )
    .await
    {
        Ok(imports) => {
            let guard = require_vault(&state)?;
            let preview = sync_reads::preview_exchange_sync(
                &guard.as_ref().unwrap().books(),
                imports,
                &input.settings,
            );
            Ok(ImportPreviewOutput {
                preview: Some(preview),
                notice: None,
            })
        }
        Err(message) => Ok(ImportPreviewOutput {
            preview: None,
            notice: Some(import_notice(&message).into()),
        }),
    }
}

/// Fetch Bybit fills and stage them for confirmation (or book them at once
/// when the import settings say so).
#[tauri::command]
pub async fn import_bybit_history(
    input: ImportBybitHistoryInput,
    state: State<'_, AppState>,
) -> Result<ImportBybitHistoryOutput, IpcError> {
    let Some(key) = request_key(&state, Some(&input.store_id))? else {
        return Ok(ImportBybitHistoryOutput {
            staged: 0,
            notice: Some("no_key".into()),
            run: None,
        });
    };
    let limit = input.limit.unwrap_or(1000).clamp(1, 10_000);
    let run = sync_bybit(&state, key, limit, ImportSource::Manual).await?;
    Ok(ImportBybitHistoryOutput {
        staged: run.staged,
        notice: run.error.clone(),
        run: Some(run),
    })
}

/// What an exchange import brings in and where it books it.
#[tauri::command]
pub async fn get_import_settings(
    store_id: String,
    state: State<'_, AppState>,
) -> Result<ImportSettings, IpcError> {
    let guard = require_vault(&state)?;
    sync_reads::import_settings(&guard.as_ref().unwrap().books(), &store_id).map_err(IpcError::from)
}

#[tauri::command]
pub async fn set_import_settings(
    input: SetImportSettingsInput,
    state: State<'_, AppState>,
) -> Result<ImportSettings, IpcError> {
    let mut guard = require_vault(&state)?;
    exchange_sync::set_import_settings(guard.as_mut().unwrap(), &input.store_id, input.settings)
        .map_err(IpcError::from)
}

/// Syncs of one exchange wallet, newest first.
#[tauri::command]
pub async fn list_import_runs(
    store_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ImportRun>, IpcError> {
    let guard = require_vault(&state)?;
    Ok(sync_reads::import_runs(
        &guard.as_ref().unwrap().books(),
        &store_id,
    ))
}

/// Operations brought in from one exchange wallet, newest first.
#[tauri::command]
pub async fn list_imported_transactions(
    input: ListImportedTransactionsInput,
    state: State<'_, AppState>,
) -> Result<ImportedTransactions, IpcError> {
    let guard = require_vault(&state)?;
    Ok(sync_reads::imported_transactions(
        &guard.as_ref().unwrap().books(),
        &input.store_id,
        input.limit.unwrap_or(50).clamp(1, 500) as usize,
        input.offset.unwrap_or(0) as usize,
    ))
}

/// Every staged fill, newest first.
#[tauri::command]
pub async fn list_pending_imports(
    state: State<'_, AppState>,
) -> Result<ListPendingImportsOutput, IpcError> {
    let guard = require_vault(&state)?;
    let vault = guard.as_ref().unwrap();
    Ok(ListPendingImportsOutput {
        imports: import_reads::pending_imports(&vault.doc),
    })
}

/// Turn staged fills into real trades. Empty `exec_ids` confirms everything.
#[tauri::command]
pub async fn confirm_pending_imports(
    input: ConfirmPendingImportsInput,
    state: State<'_, AppState>,
) -> Result<ConfirmPendingImportsOutput, IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().unwrap();
    let confirmed = exchange_import::confirm_pending_imports(vault, &input.exec_ids)?;
    Ok(ConfirmPendingImportsOutput { confirmed })
}

/// Drop staged fills without creating transactions.
#[tauri::command]
pub async fn discard_pending_imports(
    input: DiscardPendingImportsInput,
    state: State<'_, AppState>,
) -> Result<DiscardPendingImportsOutput, IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().unwrap();
    let discarded = exchange_import::discard_pending_imports(vault, &input.exec_ids)?;
    Ok(DiscardPendingImportsOutput { discarded })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info_with_amounts() -> TxInfo {
        TxInfo {
            tx_hash: "abc".into(),
            network: Some(trade_core::model::chain::Chain::Bitcoin),
            amount_in: Some("1.0".into()),
            asset_in: Some("BTC".into()),
            amount_out: Some("65000".into()),
            asset_out: Some("USDT".into()),
            fee: Some("0.0001".into()),
            fee_asset: Some("BTC".into()),
            occurred_at: Some("2023-11-14T22:13:20Z".into()),
            status: Some("confirmed".into()),
            explorer_url: Some("https://mempool.space/tx/abc".into()),
            needs_manual_network: false,
        }
    }

    #[test]
    fn hints_only_include_fields_that_have_values() {
        let hints = hints_from(&info_with_amounts());
        let fields: Vec<&str> = hints.iter().map(|hint| hint.field.as_str()).collect();
        assert!(fields.contains(&"occurredAt"));
        assert!(fields.contains(&"qtyBought"));
        assert!(fields.contains(&"qtySold"));
        assert!(fields.contains(&"fee"));
        // Neither `network` nor `explorerUrl` are form fields; they stay on
        // the info payload for the UI to render separately.
        assert!(!fields.contains(&"network"));
        assert!(!fields.contains(&"explorerUrl"));
    }

    #[test]
    fn empty_values_are_never_suggested() {
        let mut info = info_with_amounts();
        info.occurred_at = Some("   ".into());
        let hints = hints_from(&info);
        assert!(!hints.iter().any(|hint| hint.field == "occurredAt"));
    }

    /// An offline-only classification yields no field hints at all, so the
    /// form is left untouched instead of being half-filled.
    #[test]
    fn offline_classification_produces_no_field_hints() {
        let info =
            TxInfo::from_hash("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b");
        assert_eq!(info.network, Some(trade_core::model::chain::Chain::Bitcoin));
        assert!(hints_from(&info).is_empty());
    }
}

ipc_commands!(exchange_commands, EXCHANGE_NAMES, commands::exchange: [inspect_tx, bybit_executions, exchange_key_status, set_exchange_key, set_exchange_targets, set_import_interval, get_terminal_state, update_terminal_state, save_terminal_drawings, clear_exchange_key, import_bybit_history, get_import_settings, preview_bybit_import, set_import_settings, list_import_runs, list_imported_transactions, create_bybit_accounts, bybit_balances, list_pending_imports, confirm_pending_imports, discard_pending_imports]);
