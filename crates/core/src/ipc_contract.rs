//! IPC command boundary types shared with the frontend.
//! These are exported to TypeScript by the `gen-types` binary (see `src/bin`).
//! Commands in the `app` crate only translate between these DTOs and the
//! domain services — never expose internal model directly.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::{
    BankRail, BankTransferTx, CardPaymentTx, CashFlowTx, DefiAction, DefiActionTx, ExchangeTradeTx,
    ExpenseTx, FeeTx, Flow, IncomeSource, IncomeTx, PaymentTx, PriceStamp, TradeSide, Transaction,
    TransferTx, TxMeta, WalletTransferTx,
};

// -- auth --------------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateVaultInput {
    pub path: String,
    pub password: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UnlockInput {
    pub path: String,
    pub password: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ChangePasswordInput {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct VaultInfo {
    pub path: String,
    pub doc_version: u32,
    pub id: String,
    pub created_at: String,
    pub format: String,
    pub base_currency: String,
}

/// What the unlock screen needs to know about the app.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct VaultStatus {
    pub path: String,
    pub exists: bool,
    pub unlocked: bool,
    pub info: Option<VaultInfo>,
}

// -- stores ------------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateStoreInput {
    pub name: String,
    pub kind: StoreKind,
    pub color: String,
    pub note: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateStoreInput {
    pub id: String,
    pub name: String,
    pub color: String,
    pub note: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

// -- transactions ------------------------------------------------------------
//
// One input per transaction variant, mirroring the variant's own fields. The
// flat `TxnInput { kind, ... }` this replaced let the frontend send a
// `tx_hash` on a cash payment or an `iban` on a crypto trade; the backend
// ignored whatever did not fit the kind, so a typo failed silently. Here an
// impossible request does not compile in TypeScript.
//
// Shared asset identification, so each variant does not repeat five optional
// fields it does not always need.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AssetIdent {
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    pub contract: Option<String>,
}

impl AssetIdent {
    /// Rebuild a full [`AssetRef`]. `quote` is display-only and follows the
    /// symbol, the same convention the form uses.
    pub fn to_asset(&self) -> AssetRef {
        let symbol = self.symbol.to_uppercase();
        AssetRef {
            quote: symbol.clone(),
            symbol,
            name: self.name.clone(),
            coingecko_id: self.coingecko_id.clone(),
            chain: self.chain,
            contract: self.contract.clone(),
        }
    }
}

/// Fields every transaction input carries.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TxInputCommon {
    /// Positive magnitude. The direction is a separate field (`flow`) or is
    /// implied by the variant, so a negative amount cannot be sent.
    pub amount: Amount,
    pub comment: String,
    /// User tags; any number, normalized by the backend.
    pub categories: Vec<String>,
    /// RFC3339 UTC. Empty means "now".
    pub timestamp: Option<String>,
    pub price_usd: Option<Amount>,
    pub fx_override: Option<Amount>,
}

impl TxInputCommon {
    fn meta(&self) -> TxMeta {
        TxMeta {
            timestamp: match self.timestamp.as_deref() {
                Some(raw) if !raw.is_empty() => raw.to_string(),
                _ => crate::model::vault::now_iso(),
            },
            comment: self.comment.clone(),
            categories: self.categories.clone(),
            ..TxMeta::new(&self.comment)
        }
    }

    fn stamp(&self) -> PriceStamp {
        PriceStamp {
            price_usd: self.price_usd,
            fx_override: self.fx_override,
            price_base: None,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct IncomeInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub source: IncomeSource,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ExpenseInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PaymentInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub payee: String,
    pub receipt_ref: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct FeeInput {
    pub store_id: String,
    /// Asset the fee was charged in; may differ from the traded asset.
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub related_id: Option<String>,
}

// No `Default`: a defaulted direction would book the movement the wrong
// way, so every call site must state `flow` explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct BankTransferInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub flow: Flow,
    pub rail: BankRail,
    pub counterparty_name: String,
    pub counterparty_account: Option<String>,
    pub reference: Option<String>,
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CardPaymentInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub card_last4: Option<String>,
    pub merchant: Option<String>,
    pub mcc: Option<String>,
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
}

// No `Default`: a defaulted direction would book the movement the wrong
// way, so every call site must state `flow` explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct WalletTransferInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub flow: Flow,
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub memo: Option<String>,
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
}

// No `Default`: a defaulted direction would book the movement the wrong
// way, so every call site must state `flow` explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DefiActionInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub flow: Flow,
    pub action: DefiAction,
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
    pub protocol: Option<String>,
    pub pool_id: Option<String>,
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TradeInput {
    pub store_id: String,
    pub sold: AssetIdent,
    pub sold_qty: Amount,
    pub bought: AssetIdent,
    pub bought_qty: Amount,
    pub common: TxInputCommon,
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
    pub order_id: Option<String>,
    pub market: Option<String>,
    pub liquidity: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TransferInput {
    pub from_store_id: String,
    pub to_store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub fee: Option<Amount>,
    pub fee_asset: Option<String>,
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
}

// No `Default`: a defaulted direction would book the movement the wrong
// way, so every call site must state `flow` explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CashFlowInput {
    pub store_id: String,
    pub asset: AssetIdent,
    pub common: TxInputCommon,
    pub flow: Flow,
    pub location: Option<String>,
    pub counterparty: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateCommentInput {
    pub transaction_id: String,
    pub comment: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SetTxPricesInput {
    pub transaction_id: String,
    pub price_usd: Option<Amount>,
    pub fx_override: Option<Amount>,
}

/// Which transaction kinds a wallet of one kind accepts.
///
/// Returned already split into the two groups the UI presents, and with the
/// kind to pre-select, so the frontend holds no copy of the binding and cannot
/// disagree with the backend about what a bank is allowed to record.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct TransactionKinds {
    /// Kinds that make sense on any wallet.
    pub common: Vec<String>,
    /// Kinds specific to this kind of wallet — a bank transfer, a trade, cash.
    pub specific: Vec<String>,
    /// The kind to pre-select when the form opens.
    pub default: String,
}

impl TransactionKinds {
    pub fn for_store_kind(kind: StoreKind) -> Self {
        Self {
            common: Transaction::COMMON_TAGS
                .iter()
                .map(|tag| (*tag).to_string())
                .collect(),
            specific: Transaction::specific_tags_for_store_kind(kind)
                .iter()
                .map(|tag| (*tag).to_string())
                .collect(),
            default: Transaction::default_tag_for_store_kind(kind).to_string(),
        }
    }
}

// -- transaction construction -------------------------------------------------
//
// Input → domain, one function per variant. Kept beside the DTOs so the
// mapping is a single readable table rather than a switch buried in a command.

macro_rules! build_single_leg {
    ($variant:ident, $body:ident, $input:ident, $($extra:tt)*) => {{
        let $input = $input;
        Transaction::$variant($body {
            meta: $input.common.meta(),
            store_id: $input.store_id.clone(),
            asset: $input.asset.to_asset(),
            amount: $input.common.amount,
            $($extra)*
            price: $input.common.stamp(),
        })
    }};
}

pub fn to_income(input: &IncomeInput) -> crate::model::transaction::Transaction {
    build_single_leg!(Income, IncomeTx, input, source: input.source,)
}

pub fn to_expense(input: &ExpenseInput) -> crate::model::transaction::Transaction {
    build_single_leg!(Expense, ExpenseTx, input,)
}

pub fn to_payment(input: &PaymentInput) -> crate::model::transaction::Transaction {
    build_single_leg!(
        Payment, PaymentTx,
        input,
        payee: input.payee.clone(),
        receipt_ref: input.receipt_ref.clone(),
    )
}

pub fn to_fee(input: &FeeInput) -> crate::model::transaction::Transaction {
    build_single_leg!(Fee, FeeTx, input, related_id: input.related_id.clone(),)
}

pub fn to_bank_transfer(input: &BankTransferInput) -> crate::model::transaction::Transaction {
    build_single_leg!(
        BankTransfer, BankTransferTx,
        input,
        flow: input.flow,
        rail: input.rail,
        counterparty_name: input.counterparty_name.clone(),
        counterparty_account: input.counterparty_account.clone(),
        reference: input.reference.clone(),
        fee: input.fee,
        fee_asset: input.fee_asset.clone(),
    )
}

pub fn to_card_payment(input: &CardPaymentInput) -> crate::model::transaction::Transaction {
    build_single_leg!(
        CardPayment, CardPaymentTx,
        input,
        card_last4: input.card_last4.clone(),
        merchant: input.merchant.clone(),
        mcc: input.mcc.clone(),
        fee: input.fee,
        fee_asset: input.fee_asset.clone(),
    )
}

pub fn to_wallet_transfer(input: &WalletTransferInput) -> crate::model::transaction::Transaction {
    build_single_leg!(
        WalletTransfer, WalletTransferTx,
        input,
        flow: input.flow,
        network: input.network,
        tx_hash: input.tx_hash.clone(),
        from_address: input.from_address.clone(),
        to_address: input.to_address.clone(),
        memo: input.memo.clone(),
        fee: input.fee,
        fee_asset: input.fee_asset.clone(),
    )
}

pub fn to_defi_action(input: &DefiActionInput) -> crate::model::transaction::Transaction {
    build_single_leg!(
        DefiAction, DefiActionTx,
        input,
        flow: input.flow,
        action: input.action,
        network: input.network,
        tx_hash: input.tx_hash.clone(),
        protocol: input.protocol.clone(),
        pool_id: input.pool_id.clone(),
        fee: input.fee,
        fee_asset: input.fee_asset.clone(),
    )
}

pub fn to_cash_flow(input: &CashFlowInput) -> crate::model::transaction::Transaction {
    build_single_leg!(
        CashFlow, CashFlowTx,
        input,
        flow: input.flow,
        location: input.location.clone(),
        counterparty: input.counterparty.clone(),
    )
}

/// A trade is one record with two legs, so it does not fit the single-leg
/// macro.
pub fn to_exchange_trade(input: &TradeInput) -> crate::model::transaction::Transaction {
    crate::model::transaction::Transaction::ExchangeTrade(ExchangeTradeTx {
        meta: input.common.meta(),
        store_id: input.store_id.clone(),
        sold: TradeSide {
            asset: input.sold.to_asset(),
            qty: input.sold_qty,
        },
        bought: TradeSide {
            asset: input.bought.to_asset(),
            qty: input.bought_qty,
        },
        fee: input.fee,
        fee_asset: input.fee_asset.clone(),
        order_id: input.order_id.clone(),
        market: input.market.clone(),
        liquidity: input.liquidity.clone(),
        price: input.common.stamp(),
    })
}

pub fn to_transfer(input: &TransferInput) -> crate::model::transaction::Transaction {
    crate::model::transaction::Transaction::Transfer(TransferTx {
        meta: input.common.meta(),
        from_store_id: input.from_store_id.clone(),
        to_store_id: input.to_store_id.clone(),
        asset: input.asset.to_asset(),
        amount: input.common.amount,
        fee: input.fee,
        fee_asset: input.fee_asset.clone(),
        network: input.network,
        tx_hash: input.tx_hash.clone(),
        price: input.common.stamp(),
    })
}

// -- pricing -----------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct RefreshResult {
    pub prices: usize,
    pub fx: usize,
    pub updated_at: String,
}

/// Loose asset identification: enough to rebuild an [`AssetRef`] cache key
/// without making the frontend construct a full `quote` field.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct AssetPriceInput {
    pub symbol: String,
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    pub contract: Option<String>,
}

/// Last cached USD quote. `ts` is empty when the asset was never priced.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CachedPrice {
    pub price_usd: Amount,
    pub ts: String,
}

// -- calc --------------------------------------------------------------------

pub use crate::calc::{CalcInput, CalcResult};

// -- settings ----------------------------------------------------------------

/// Partial update of the **vault-scoped** settings.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateSettingsInput {
    pub display_decimals: Option<u32>,
    pub dashboard_widgets: Option<Vec<crate::model::vault::DashboardWidgetCfg>>,
    pub fx_refresh_interval_sec: Option<u32>,
    pub minimize_to_tray: Option<bool>,
    /// Background Bybit poll interval; `0` disables it (D2.4).
    pub bybit_import_interval_sec: Option<u32>,
}

// -- app settings (outside the vault) ------------------------------------------

pub use crate::appconfig::{AppSettings, SupportedCurrencies, UpdateAppSettingsInput};
pub use crate::appconfig::{Locale, Theme};

/// Catalog of currencies the base/favourite pickers may offer.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CurrencyCatalogOutput {
    pub catalog: SupportedCurrencies,
    /// Machine code describing where the list came from: `builtin` when it is
    /// the built-in fallback (never fetched), `cached` when the network was
    /// unreachable and the last successful fetch was reused, `null` when the
    /// providers answered just now.
    pub notice: Option<String>,
}

pub use crate::model::vault::{FxPoint, PricePoint};

// -- tx inspection -----------------------------------------------------------

pub use crate::txinfo::TxInfo;

/// Chain the user picked when the hash alone was ambiguous. Without it the
/// backend never guesses an EVM network.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct InspectTxInput {
    pub tx_hash: String,
    /// `None` for EVM hashes, where a wrong guess would be worse than no data.
    pub network: Option<Chain>,
}

/// A single field suggestion for the trade form.
///
/// Everything is optional and unvalidated on purpose: the form only fills in
/// what the provider actually returned.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct TxFieldHint {
    pub field: String,
    pub value: String,
}

/// Result of a best-effort lookup. `fetched` tells the UI whether to say
/// anything at all; a `false` with no `notice` means "stay silent".
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct InspectTxResult {
    /// `false` when the provider could not be reached or the hash is unusable.
    pub fetched: bool,
    pub info: Option<TxInfo>,
    pub hints: Vec<TxFieldHint>,
    /// Why nothing came back, for a quiet inline hint. Never a blocking error.
    pub notice: Option<String>,
}

// -- exchange ----------------------------------------------------------------

use crate::exchange::{Category, TradePrefill};

/// A fill as offered in the "recent trades" dropdown.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct BybitExecution {
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub exec_price: String,
    pub exec_qty: String,
    pub exec_value: String,
    pub exec_fee: String,
    pub exec_time: String,
    pub is_maker: bool,
    /// Pre-shaped form values, so the frontend never re-derives the swap.
    pub prefill: TradePrefill,
}

/// Never fails: an unconfigured or unreachable key yields an empty list.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct BybitExecutionsResult {
    pub executions: Vec<BybitExecution>,
    pub notice: Option<String>,
}

impl From<crate::exchange::Execution> for BybitExecution {
    fn from(source: crate::exchange::Execution) -> Self {
        BybitExecution {
            symbol: source.symbol.clone(),
            side: source.side.clone(),
            order_type: source.order_type.clone(),
            exec_price: source.exec_price.clone(),
            exec_qty: source.exec_qty.clone(),
            exec_value: source.exec_value.clone(),
            exec_fee: source.exec_fee.clone(),
            exec_time: source.exec_time.clone(),
            is_maker: source.is_maker,
            prefill: source.to_prefill(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct BybitExecutionsInput {
    pub category: Category,
    pub symbol: Option<String>,
    pub limit: Option<u32>,
    pub store_id: Option<String>,
}

/// Whether a key is stored. The secret is never part of this response.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ExchangeKeyStatus {
    pub configured: bool,
    /// Masked identifier such as `abcd…wxyz`, for the settings screen.
    pub hint: Option<String>,
}

/// Input to query key status for a specific store.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ExchangeKeyStatusInput {
    pub store_id: String,
}

/// Stores a read-only key for a specific store. Both fields are written to the
/// encrypted vault and are never echoed back to the frontend.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct SetExchangeKeyInput {
    pub store_id: String,
    pub api_key: String,
    pub api_secret: String,
}

/// Input to clear a key for a specific store.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ClearExchangeKeyInput {
    pub store_id: String,
}

// -- bybit import (D2.4) -----------------------------------------------------

/// Fetch every Bybit fill for a store and stage it for user confirmation.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ImportBybitHistoryInput {
    pub store_id: String,
    /// Safety cap on fetched fills (default 1000, max 10000). The import always
    /// sweeps every category the key has access to.
    pub limit: Option<u32>,
}

/// How many fills this run newly staged (already-known ids are skipped).
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ImportBybitHistoryOutput {
    pub staged: u32,
    /// Machine code: `no_key` | `unreachable` | `rate_limited` |
    /// `forbidden_region`.
    pub notice: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ListPendingImportsOutput {
    pub imports: Vec<crate::model::vault::PendingImport>,
}

/// Turn staged fills into real trades (two legs each, like `add_trade`).
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ConfirmPendingImportsInput {
    /// Empty = confirm everything currently staged.
    pub exec_ids: Vec<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ConfirmPendingImportsOutput {
    pub confirmed: u32,
}

/// Drop staged fills without creating transactions.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct DiscardPendingImportsInput {
    /// Empty = discard everything currently staged.
    pub exec_ids: Vec<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct DiscardPendingImportsOutput {
    pub discarded: u32,
}

/// A recently opened vault file.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct RecentVault {
    pub path: String,
    pub last_opened: String,
    pub exists: bool,
}

/// Input to select or pick a vault file.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct SelectVaultInput {
    pub path: String,
}

/// Output of the dev dump vault command (DEV only).
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct DevDumpVaultOutput {
    pub json: String,
}

// -- market data (candles) --------------------------------------------------

/// Single OHLCV candle point for charting.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct CandlePoint {
    pub ts: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GetCandlesInput {
    pub symbol: String,
    pub interval_min: u32,
    pub limit: u32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GetCandlesOutput {
    pub candles: Vec<CandlePoint>,
}
