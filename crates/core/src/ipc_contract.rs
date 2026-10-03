//! IPC command boundary types shared with the frontend.
//! These are exported to TypeScript by the `gen-types` binary (see `src/bin`).
//! Commands in the `app` crate only translate between these DTOs and the
//! domain services — never expose internal model directly.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::StoreKind;
use crate::model::transaction::TxKind;
use crate::model::vault::{Locale, Theme};

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
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateStoreInput {
    pub id: String,
    pub name: String,
    pub color: String,
    pub note: String,
}

// -- transactions ------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct TxnInput {
    pub kind: TxKind,
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    pub contract: Option<String>,
    /// Always a positive magnitude; sign is applied by the backend per kind.
    pub amount: Amount,
    pub store_id: String,
    pub comment: String,
    pub category: Option<String>,
    pub timestamp: Option<String>,
    pub tx_hash: Option<String>,
    pub network: Option<Chain>,
    pub price_usd: Option<Amount>,
    pub fee: Option<Amount>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct TransferInput {
    pub from_store_id: String,
    pub to_store_id: String,
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    pub contract: Option<String>,
    pub amount: Amount,
    pub comment: String,
    pub category: Option<String>,
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
    pub fee: Option<Amount>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct TradeInput {
    pub store_id: String,
    pub sell_symbol: String,
    pub sell_name: String,
    pub sell_coingecko_id: Option<String>,
    pub sell_chain: Option<Chain>,
    pub sell_contract: Option<String>,
    pub sell_qty: Amount,
    pub buy_symbol: String,
    pub buy_name: String,
    pub buy_coingecko_id: Option<String>,
    pub buy_chain: Option<Chain>,
    pub buy_contract: Option<String>,
    pub buy_qty: Amount,
    pub comment: String,
    pub category: Option<String>,
    pub fee: Option<Amount>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateCommentInput {
    pub transaction_id: String,
    pub comment: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SetTxPricesInput {
    pub transaction_id: String,
    pub price_usd: Option<Amount>,
    pub fx_override: Option<Amount>,
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

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateSettingsInput {
    pub base_currency: Option<String>,
    pub theme: Option<Theme>,
    pub locale: Option<Locale>,
    pub display_decimals: Option<u32>,
}

// -- sim ---------------------------------------------------------------------

use crate::sim::Strategy;

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SaveStrategyInput {
    pub strategy: Strategy,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct RunStrategyInput {
    pub strategy_id: String,
    /// Number of candles to replay (e.g. 500).
    pub candles: u32,
    /// candle granularity in minutes (e.g. 60 = 1h candles).
    pub granularity_min: u32,
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
    fn from(e: crate::exchange::Execution) -> Self {
        BybitExecution {
            symbol: e.symbol.clone(),
            side: e.side.clone(),
            order_type: e.order_type.clone(),
            exec_price: e.exec_price.clone(),
            exec_qty: e.exec_qty.clone(),
            exec_value: e.exec_value.clone(),
            exec_fee: e.exec_fee.clone(),
            exec_time: e.exec_time.clone(),
            is_maker: e.is_maker,
            prefill: e.to_prefill(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct BybitExecutionsInput {
    pub category: Category,
    pub symbol: Option<String>,
    pub limit: Option<u32>,
}

/// Whether a key is stored. The secret is never part of this response.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ExchangeKeyStatus {
    pub configured: bool,
    /// Masked identifier such as `abcd…wxyz`, for the settings screen.
    pub hint: Option<String>,
}

/// Stores a read-only key. Both fields are written to the encrypted vault and
/// are never echoed back to the frontend.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct SetExchangeKeyInput {
    pub api_key: String,
    pub api_secret: String,
}
