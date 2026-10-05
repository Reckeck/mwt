//! IPC command boundary types shared with the frontend.
//! These are exported to TypeScript by the `gen-types` binary (see `src/bin`).
//! Commands in the `app` crate only translate between these DTOs and the
//! domain services — never expose internal model directly.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::StoreKind;
use crate::model::transaction::TxKind;

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

// -- run replay (D2.2) -------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GetRunCandlesInput {
    pub run_id: String,
}

/// Candles of a finished run: synthetic runs are regenerated from
/// `candle_params` (deterministic seed), real runs are re-fetched from
/// Binance for the run's time range.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GetRunCandlesOutput {
    pub candles: Vec<CandlePoint>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GbmPathsInput {
    pub run_id: String,
    /// number of fan lines (clamped server-side).
    pub paths: u32,
    /// steps into the future (clamped server-side).
    pub horizon: u32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GbmPathPoint {
    pub ts: String,
    pub price: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct GbmPathsOutput {
    pub paths: Vec<Vec<GbmPathPoint>>,
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
