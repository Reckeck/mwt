use tauri::State;

use trade_core::ipc_contract::{TradeInput, TransferInput, TxnInput, UpdateCommentInput};
use trade_core::model::store::AssetRef;
use trade_core::model::transaction::{Transaction, TxKind};

use crate::commands::{AppState, err_to_string, require_vault};

fn asset_from(prefix: &str, i: &TxnInput) -> AssetRef {
    AssetRef {
        symbol: i.symbol.to_uppercase(),
        name: i.name.clone(),
        coingecko_id: i.coingecko_id.clone(),
        chain: i.chain,
        contract: i.contract.clone(),
        quote: format!("{prefix}{}", i.symbol.to_uppercase()),
    }
}

fn to_transaction(i: &TxnInput) -> Transaction {
    let signed = match i.kind {
        TxKind::Expense | TxKind::Fee => -i.amount.abs(),
        _ => i.amount.abs(),
    };
    let mut tx = Transaction::new(i.kind, asset_from("", i), signed, &i.store_id, &i.comment);
    tx.category = i.category.clone();
    tx.tx_hash = i.tx_hash.clone();
    tx.network = i.network;
    tx.fee = i.fee.map(|f| f.abs());
    tx.price_usd = i.price_usd;
    if let Some(ts) = &i.timestamp
        && !ts.is_empty()
    {
        tx.timestamp = ts.clone();
    }
    tx
}

#[tauri::command]
pub async fn list_transactions(
    store_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().transactions(store_id.as_deref()))
}

#[tauri::command]
pub async fn add_transaction(
    input: TxnInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let tx = to_transaction(&input);
    v.add_transaction(tx).map_err(err_to_string)
}

#[tauri::command]
pub async fn add_transfer(
    input: TransferInput,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let asset = AssetRef {
        symbol: input.symbol.to_uppercase(),
        name: input.name.clone(),
        coingecko_id: input.coingecko_id.clone(),
        chain: input.chain,
        contract: input.contract.clone(),
        quote: input.symbol.to_uppercase(),
    };
    let (out, inn) = v
        .transfer(
            &input.from_store_id,
            &input.to_store_id,
            asset,
            input.amount,
            input.fee,
            input.network,
            input.tx_hash.clone(),
            &input.comment,
            input.category.clone(),
        )
        .map_err(err_to_string)?;
    Ok(vec![out, inn])
}

#[tauri::command]
pub async fn add_trade(
    input: TradeInput,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let sell = AssetRef {
        symbol: input.sell_symbol.to_uppercase(),
        name: input.sell_name.clone(),
        coingecko_id: input.sell_coingecko_id.clone(),
        chain: input.sell_chain,
        contract: input.sell_contract.clone(),
        quote: input.sell_symbol.to_uppercase(),
    };
    let buy = AssetRef {
        symbol: input.buy_symbol.to_uppercase(),
        name: input.buy_name.clone(),
        coingecko_id: input.buy_coingecko_id.clone(),
        chain: input.buy_chain,
        contract: input.buy_contract.clone(),
        quote: input.buy_symbol.to_uppercase(),
    };
    let (out, inn) = v
        .add_trade(
            &input.store_id,
            sell,
            input.sell_qty,
            buy,
            input.buy_qty,
            input.fee,
            &input.comment,
            input.category.clone(),
        )
        .map_err(err_to_string)?;
    Ok(vec![out, inn])
}

#[tauri::command]
pub async fn delete_transaction(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.delete_transaction(&id).map_err(err_to_string)
}

#[tauri::command]
pub async fn update_comment(
    input: UpdateCommentInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.update_comment(&input.transaction_id, &input.comment)
        .map_err(err_to_string)
}
