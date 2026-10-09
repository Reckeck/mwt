use tauri::State;

use trade_core::ipc_contract::{
    BankTransferInput, CardPaymentInput, CashFlowInput, DefiActionInput, ExpenseInput, FeeInput,
    IncomeInput, PaymentInput, TradeInput, TransactionKinds, TransferInput, UpdateCommentInput,
    WalletTransferInput, to_bank_transfer, to_card_payment, to_cash_flow, to_defi_action,
    to_exchange_trade, to_expense, to_fee, to_income, to_payment, to_transfer, to_wallet_transfer,
};
use trade_core::model::store::{StoreKind, StoreScope};
use trade_core::model::transaction::Transaction;

use crate::commands::{AppState, err_to_string, require_vault};

/// Every write goes through here: validate, append, re-derive holdings, persist.
///
/// The per-variant commands differ only in which input DTO they forward, so
/// the transaction is built and stored in exactly one place.
fn store_transaction(tx: Transaction, state: &State<'_, AppState>) -> Result<Transaction, String> {
    let mut guard = require_vault(state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .add_transaction(tx)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn list_transactions(
    store_id: Option<String>,
    include_children: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let scope = match store_id {
        Some(id) => StoreScope::store(&id, include_children.unwrap_or(true)),
        None => StoreScope::default(),
    };
    Ok(guard.as_ref().unwrap().transactions(&scope))
}

#[tauri::command]
pub async fn add_income(
    input: IncomeInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_income(&input), &state)
}

#[tauri::command]
pub async fn add_expense(
    input: ExpenseInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_expense(&input), &state)
}

#[tauri::command]
pub async fn add_payment(
    input: PaymentInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_payment(&input), &state)
}

#[tauri::command]
pub async fn add_fee(input: FeeInput, state: State<'_, AppState>) -> Result<Transaction, String> {
    store_transaction(to_fee(&input), &state)
}

#[tauri::command]
pub async fn add_bank_transfer(
    input: BankTransferInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_bank_transfer(&input), &state)
}

#[tauri::command]
pub async fn add_card_payment(
    input: CardPaymentInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_card_payment(&input), &state)
}

#[tauri::command]
pub async fn add_wallet_transfer(
    input: WalletTransferInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_wallet_transfer(&input), &state)
}

#[tauri::command]
pub async fn add_defi_action(
    input: DefiActionInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_defi_action(&input), &state)
}

/// Both sides of a trade are one record now, so this returns a single
/// transaction rather than the pair of legs the old command returned.
#[tauri::command]
pub async fn add_trade(
    input: TradeInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_exchange_trade(&input), &state)
}

/// Likewise a cross-store transfer is one record with two legs.
#[tauri::command]
pub async fn add_transfer(
    input: TransferInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    if input.from_store_id == input.to_store_id {
        return Err("INVALID_INPUT: transfer origin == destination".into());
    }
    store_transaction(to_transfer(&input), &state)
}

#[tauri::command]
pub async fn add_cash_flow(
    input: CashFlowInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    store_transaction(to_cash_flow(&input), &state)
}

#[tauri::command]
pub async fn delete_transaction(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_transaction(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_comment(
    input: UpdateCommentInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_comment(&input.transaction_id, &input.comment)
        .map_err(err_to_string)
}

/// Which transaction kinds a wallet of this kind accepts.
///
/// Pure: the binding is static domain data, so this needs no vault and takes no
/// lock. The frontend calls it with the kind of the wallet it is showing, which
/// keeps "what may a bank record" in exactly one place.
#[tauri::command]
pub async fn transaction_kinds_for_store_kind(kind: StoreKind) -> Result<TransactionKinds, String> {
    Ok(TransactionKinds::for_store_kind(kind))
}
