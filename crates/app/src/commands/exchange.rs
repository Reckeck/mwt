//! Read-only transaction inspection and exchange prefill.
//!
//! Every command here is deliberately *soft-failing*: the trade form must stay
//! usable with no network, no API key and no provider support. So a lookup that
//! cannot be completed returns `fetched: false` (or an empty list) instead of an
//! error the user has to dismiss.

use tauri::State;

use trade_core::exchange::{BybitClient, Category};
use trade_core::ipc_contract::{
    BybitExecution, BybitExecutionsInput, BybitExecutionsResult, ExchangeKeyStatus, InspectTxInput,
    InspectTxResult, SetExchangeKeyInput, TxFieldHint,
};
use trade_core::model::vault::ExchangeKeys;
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
        match g.as_ref().and_then(|v| v.doc.bybit.as_ref()) {
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

/// Whether a read-only Bybit key is stored. The secret is never returned.
#[tauri::command]
pub async fn exchange_key_status(state: State<'_, AppState>) -> Result<ExchangeKeyStatus, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    match g.as_ref().and_then(|v| v.doc.bybit.as_ref()) {
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

/// Store a read-only Bybit key in the encrypted vault.
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
    if key.is_empty() || secret.is_empty() {
        return Err("INVALID_INPUT: api key and secret are required".into());
    }
    v.doc.bybit = Some(ExchangeKeys {
        api_key: key,
        api_secret: secret,
    });
    v.persist().map_err(err_to_string)?;
    let status = v
        .doc
        .bybit
        .as_ref()
        .map(|k| ExchangeKeyStatus {
            configured: true,
            hint: Some(k.hint()),
        })
        .unwrap_or_default();
    Ok(status)
}

/// Remove the stored Bybit key from the vault.
#[tauri::command]
pub async fn clear_exchange_key(state: State<'_, AppState>) -> Result<ExchangeKeyStatus, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.doc.bybit = None;
    v.persist().map_err(err_to_string)?;
    Ok(ExchangeKeyStatus::default())
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
