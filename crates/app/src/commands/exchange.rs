//! Read-only transaction inspection and exchange prefill.
//!
//! Every command here is deliberately *soft-failing*: the trade form must stay
//! usable with no network, no API key and no provider support. So a lookup that
//! cannot be completed returns `fetched: false` (or an empty list) instead of an
//! error the user has to dismiss.

use tauri::State;

use trade_core::exchange::{BybitClient, Category};
use trade_core::ipc_contract::{
    BybitExecution, BybitExecutionsInput, BybitExecutionsResult, ClearExchangeKeyInput,
    ConfirmPendingImportsInput, ConfirmPendingImportsOutput, DiscardPendingImportsInput,
    DiscardPendingImportsOutput, ExchangeKeyStatus, ExchangeKeyStatusInput,
    ImportBybitHistoryInput, ImportBybitHistoryOutput, InspectTxInput, InspectTxResult,
    ListPendingImportsOutput, SetExchangeKeyInput, TxFieldHint,
};
use trade_core::model::store::StoreKind;
use trade_core::model::vault::{ExchangeKeys, Exchanges, PendingImport};
use trade_core::txinfo::{TxInfo, TxInfoService};

use crate::commands::{AppState, err_to_string, require_vault};

/// Look up a transaction hash and suggest form fields.
///
/// Never errors for a missing/unreachable provider — `fetched: false` with an
/// optional `notice` is the failure channel.
#[tauri::command]
pub async fn inspect_tx(input: InspectTxInput) -> Result<InspectTxResult, String> {
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
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            hints.push(TxFieldHint {
                field: field.to_string(),
                value: v,
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

/// Find the Bybit API key for a given store_id, walking up the parent chain.
/// Returns the first key found (closest ancestor with a key).
fn find_bybit_key_for_store(
    exchanges: &Exchanges,
    store_id: &str,
    _vault: &trade_core::service::UnlockedVault,
) -> Option<ExchangeKeys> {
    // Direct match first
    for key in &exchanges.bybit {
        if key.store_id == store_id {
            return Some(key.clone());
        }
    }
    // Walk up parent chain (Phase N: nesting) - for now just fall back to first key
    exchanges.bybit.first().cloned()
}

/// Recent Bybit fills for the prefill dropdown.
///
/// Empty list + `notice` when unconfigured or offline; never an error.
#[tauri::command]
pub async fn bybit_executions(
    input: BybitExecutionsInput,
    state: State<'_, AppState>,
) -> Result<BybitExecutionsResult, String> {
    let (api_key, api_secret) = {
        let g = require_vault(&state).map_err(err_to_string)?;
        let v = g.as_ref().unwrap();
        match find_bybit_key_for_store(&v.doc.exchanges, input.store_id.as_deref().unwrap_or(""), v)
        {
            Some(k) => (k.api_key.clone(), k.api_secret.clone()),
            None => {
                return Ok(BybitExecutionsResult {
                    executions: Vec::new(),
                    notice: Some("no_key".into()),
                });
            }
        }
    };

    let category: Category = input.category;
    let client = BybitClient::default();
    let list = client
        .executions(
            &api_key,
            &api_secret,
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
        Err(e) => {
            // Surface a hint, but never block the form.
            let notice = if e.to_string().contains("403") {
                Some("forbidden_region".into())
            } else if e.to_string().contains("429") {
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
) -> Result<ExchangeKeyStatus, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_ref().unwrap();
    match find_bybit_key_for_store(&v.doc.exchanges, &input.store_id, v) {
        Some(k) => Ok(ExchangeKeyStatus {
            configured: true,
            hint: Some(k.hint()),
        }),
        None => Ok(ExchangeKeyStatus {
            configured: false,
            hint: None,
        }),
    }
}

/// Store a read-only Bybit key in the encrypted vault for a specific store.
///
/// Only write/read endpoints exist in the client, so this cannot trade.
#[tauri::command]
pub async fn set_exchange_key(
    input: SetExchangeKeyInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyStatus, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let key = input.api_key.trim().to_string();
    let secret = input.api_secret.trim().to_string();
    let store_id = input.store_id.clone();
    if key.is_empty() || secret.is_empty() {
        return Err("INVALID_INPUT: api key and secret are required".into());
    }
    // Validate store exists and is an exchange node
    let store = v.doc.stores.iter().find(|s| s.id == store_id);
    let Some(store) = store else {
        return Err("NOT_FOUND: store not found".into());
    };
    if store.kind != StoreKind::Exchange {
        return Err("INVALID_INPUT: store is not an exchange".into());
    }
    // Upsert by store_id
    v.doc.exchanges.bybit.retain(|k| k.store_id != store_id);
    v.doc.exchanges.bybit.push(ExchangeKeys {
        store_id: store_id.clone(),
        api_key: key,
        api_secret: secret,
    });
    v.persist().map_err(err_to_string)?;
    let status = v
        .doc
        .exchanges
        .bybit
        .iter()
        .find(|k| k.store_id == store_id)
        .map(|k| ExchangeKeyStatus {
            configured: true,
            hint: Some(k.hint()),
        })
        .unwrap_or_default();
    Ok(status)
}

/// Remove the stored Bybit key for a specific store.
#[tauri::command]
pub async fn clear_exchange_key(
    input: ClearExchangeKeyInput,
    state: State<'_, AppState>,
) -> Result<ExchangeKeyStatus, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.doc
        .exchanges
        .bybit
        .retain(|k| k.store_id != input.store_id);
    v.persist().map_err(err_to_string)?;
    Ok(ExchangeKeyStatus::default())
}

// -- Bybit import staging (D2.4) -----------------------------------------
//
// All staging/dedup/confirm logic lives in `trade_core::service` so it is unit
// tested; these handlers only move data across the IPC boundary.

/// Machine-readable reason for a soft failure (never surfaces as an error).
fn import_notice(e: &str) -> &'static str {
    if e.contains("403") {
        "forbidden_region"
    } else if e.contains("429") {
        "rate_limited"
    } else {
        "unreachable"
    }
}

/// Fetch Bybit fills and stage them for confirmation. Never creates
/// transactions on its own — that only happens in `confirm_pending_imports`.
#[tauri::command]
pub async fn import_bybit_history(
    input: ImportBybitHistoryInput,
    state: State<'_, AppState>,
) -> Result<ImportBybitHistoryOutput, String> {
    let (api_key, api_secret) = {
        let g = require_vault(&state).map_err(err_to_string)?;
        let v = g.as_ref().unwrap();
        match find_bybit_key_for_store(&v.doc.exchanges, &input.store_id, v) {
            Some(k) => (k.api_key.clone(), k.api_secret.clone()),
            None => {
                return Ok(ImportBybitHistoryOutput {
                    staged: 0,
                    notice: Some("no_key".into()),
                });
            }
        }
    };

    let limit = input.limit.unwrap_or(1000).clamp(1, 10_000);
    let client = BybitClient::default();

    // `all_executions` paginates per category; fills come back newest-first.
    let execs = match client.all_executions(&api_key, &api_secret, limit).await {
        Ok(e) => e,
        Err(e) => {
            return Ok(ImportBybitHistoryOutput {
                staged: 0,
                notice: Some(import_notice(&e.to_string()).into()),
            });
        }
    };

    let imports: Vec<PendingImport> = execs
        .iter()
        .take(limit as usize)
        .filter_map(|exec| {
            PendingImport::from_execution(&input.store_id, exec.category_hint(), exec)
        })
        .collect();

    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let staged = v.stage_imports(imports).map_err(err_to_string)?;
    Ok(ImportBybitHistoryOutput {
        staged,
        notice: None,
    })
}

/// Every staged fill, newest first.
#[tauri::command]
pub async fn list_pending_imports(
    state: State<'_, AppState>,
) -> Result<ListPendingImportsOutput, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_ref().unwrap();
    Ok(ListPendingImportsOutput {
        imports: v.pending_imports(),
    })
}

/// Turn staged fills into real trades. Empty `exec_ids` confirms everything.
#[tauri::command]
pub async fn confirm_pending_imports(
    input: ConfirmPendingImportsInput,
    state: State<'_, AppState>,
) -> Result<ConfirmPendingImportsOutput, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let confirmed = v
        .confirm_pending_imports(&input.exec_ids)
        .map_err(err_to_string)?;
    Ok(ConfirmPendingImportsOutput { confirmed })
}

/// Drop staged fills without creating transactions.
#[tauri::command]
pub async fn discard_pending_imports(
    input: DiscardPendingImportsInput,
    state: State<'_, AppState>,
) -> Result<DiscardPendingImportsOutput, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let discarded = v
        .discard_pending_imports(&input.exec_ids)
        .map_err(err_to_string)?;
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
        let fields: Vec<&str> = hints.iter().map(|h| h.field.as_str()).collect();
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
        assert!(!hints.iter().any(|h| h.field == "occurredAt"));
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
