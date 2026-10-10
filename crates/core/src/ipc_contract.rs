//! IPC command boundary types shared with the frontend.
//! These are exported to TypeScript by the `gen-types` binary (see `src/bin`).
//! Commands in the `app` crate only translate between these DTOs and the
//! domain services — never expose internal model directly.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::category::CategoryGroup;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, LiabilityTerms, StoreKind};
use crate::model::transaction::{
    AccountRef, AssetSellTx, ConvertSide, ConvertTx, DebtEffect, DefiAction, DefiTx, Fee,
    FuturesAction, FuturesCategory, FuturesDirection, KindDetails, Liquidity, PaymentTx,
    PriceStamp, TopupTx, TradeFuturesTx, TradeSide, TradeSpotTx, Transaction, TransferTx, TxKind,
    TxMeta, allowed_kinds, default_kind,
};
use crate::model::vault::DailyBudgetMode;
use crate::service::ContactDraft;

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
    /// `None` = the default: by kind for a wallet, the parent's for a sub-account.
    #[serde(default)]
    pub in_budget: Option<bool>,
    /// Loan terms; only for `liability`.
    #[serde(default)]
    pub liability: Option<LiabilityTerms>,
    /// Contact the wallet belongs to; top-level only.
    #[serde(default)]
    pub contact_id: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateStoreInput {
    pub id: String,
    pub name: String,
    pub color: String,
    pub note: String,
    /// `None` keeps the current value.
    #[serde(default)]
    pub in_budget: Option<bool>,
    #[serde(default)]
    pub liability: Option<LiabilityTerms>,
    #[serde(default)]
    pub contact_id: Option<String>,
}

/// Re-hang a sub-account; `parent_id: None` makes it a top-level wallet.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct MoveStoreInput {
    pub id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

// -- transactions ------------------------------------------------------------
//
// One input enum, internally tagged with `type` — an IPC shape only, never
// persisted (bincode cannot read internal tags). Each variant mirrors its
// domain transaction minus what the backend fills in: id, origin, prices.

/// Shared asset identification, so each variant does not repeat five
/// optional fields it does not always need.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AssetIdent {
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    pub contract: Option<String>,
}

impl AssetIdent {
    /// Rebuild a full [`AssetRef`]. `quote` is display-only and follows the symbol.
    pub fn to_asset(&self) -> AssetRef {
        let symbol = self.symbol.trim().to_uppercase();
        AssetRef {
            quote: symbol.clone(),
            symbol,
            name: self.name.trim().to_string(),
            coingecko_id: self.coingecko_id.clone().filter(|id| !id.is_empty()),
            chain: self.chain,
            contract: self
                .contract
                .clone()
                .filter(|contract| !contract.is_empty()),
        }
    }
}

/// Fields every transaction input carries.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TxCommonInput {
    /// RFC3339 UTC. Empty means "now".
    pub timestamp: Option<String>,
    pub comment: String,
    /// Category keys.
    pub categories: Vec<String>,
    pub flow_id: Option<String>,
    pub contact_id: Option<String>,
    /// Manual USD price of the main asset (the one received, for two-asset kinds).
    pub price_usd: Option<Amount>,
    pub fx_override: Option<Amount>,
}

impl TxCommonInput {
    fn meta(&self) -> TxMeta {
        let mut meta = TxMeta::new(&self.comment);
        if let Some(raw) = self.timestamp.as_deref().filter(|raw| !raw.is_empty()) {
            meta.timestamp = raw.to_string();
        }
        meta.categories = self.categories.clone();
        meta.flow_id = self.flow_id.clone().filter(|flow| !flow.trim().is_empty());
        meta.contact_id = self
            .contact_id
            .clone()
            .filter(|contact| !contact.is_empty());
        meta
    }

    fn stamp(&self) -> PriceStamp {
        PriceStamp {
            price_usd: self.price_usd,
            price_base: None,
            fx_override: self.fx_override,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct FeeInput {
    pub asset: AssetIdent,
    pub amount: Amount,
}

impl FeeInput {
    fn to_fee(&self) -> Fee {
        Fee {
            asset: self.asset.to_asset(),
            amount: self.amount,
            price: PriceStamp::default(),
        }
    }
}

fn fee_of(input: &Option<FeeInput>) -> Option<Fee> {
    input
        .as_ref()
        .filter(|fee| !fee.amount.is_zero())
        .map(FeeInput::to_fee)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ConvertSideInput {
    pub account: AccountRef,
    pub asset: AssetIdent,
    pub amount: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TradeSideInput {
    pub asset: AssetIdent,
    pub qty: Amount,
}

/// What the form sends for any kind of transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransactionInput {
    Topup {
        common: TxCommonInput,
        store_id: String,
        asset: AssetIdent,
        amount: Amount,
        fee: Option<FeeInput>,
        from: Option<AccountRef>,
        details: KindDetails,
    },
    Payment {
        common: TxCommonInput,
        store_id: String,
        asset: AssetIdent,
        amount: Amount,
        fee: Option<FeeInput>,
        to: Option<AccountRef>,
        details: KindDetails,
    },
    Transfer {
        common: TxCommonInput,
        from: AccountRef,
        to: AccountRef,
        asset: AssetIdent,
        amount: Amount,
        fee: Option<FeeInput>,
        network: Option<Chain>,
        tx_hash: Option<String>,
        debt: Option<DebtEffect>,
    },
    Convert {
        common: TxCommonInput,
        from: ConvertSideInput,
        to: ConvertSideInput,
        fee: Option<FeeInput>,
        rate_override: Option<Amount>,
    },
    TradeSpot {
        common: TxCommonInput,
        store_id: String,
        sold: TradeSideInput,
        bought: TradeSideInput,
        fee: Option<FeeInput>,
        liquidity: Option<Liquidity>,
        order_id: Option<String>,
    },
    TradeFutures {
        common: TxCommonInput,
        store_id: String,
        /// Exchange the contract trades on, e.g. `bybit`.
        venue: String,
        /// Contract symbol, e.g. `BTCUSDT`.
        symbol: String,
        settle: AssetIdent,
        direction: FuturesDirection,
        action: FuturesAction,
        quantity: Amount,
        fill_price: Amount,
        leverage: Option<Amount>,
        fee: Option<FeeInput>,
        closed_pnl: Option<Amount>,
        order_id: Option<String>,
        category: FuturesCategory,
    },
    Defi {
        common: TxCommonInput,
        store_id: String,
        action: DefiAction,
        asset: AssetIdent,
        amount: Amount,
        protocol: Option<String>,
        pool_id: Option<String>,
        network: Option<Chain>,
        tx_hash: Option<String>,
        fee: Option<FeeInput>,
    },
    AssetSell {
        common: TxCommonInput,
        store_id: String,
        asset: AssetIdent,
        unit_price: Amount,
        quantity: Amount,
        currency: AssetIdent,
        proceeds_to: Option<String>,
        fee: Option<FeeInput>,
    },
}

fn text(raw: &Option<String>) -> Option<String> {
    raw.as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

impl TransactionInput {
    /// Build the domain transaction. Rules are checked by the service, not here.
    pub fn to_transaction(&self) -> Transaction {
        match self {
            Self::Topup {
                common,
                store_id,
                asset,
                amount,
                fee,
                from,
                details,
            } => Transaction::Topup(TopupTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                asset: asset.to_asset(),
                amount: *amount,
                fee: fee_of(fee),
                from: from.clone(),
                details: details.clone(),
                price: common.stamp(),
            }),
            Self::Payment {
                common,
                store_id,
                asset,
                amount,
                fee,
                to,
                details,
            } => Transaction::Payment(PaymentTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                asset: asset.to_asset(),
                amount: *amount,
                fee: fee_of(fee),
                to: to.clone(),
                details: details.clone(),
                price: common.stamp(),
            }),
            Self::Transfer {
                common,
                from,
                to,
                asset,
                amount,
                fee,
                network,
                tx_hash,
                debt,
            } => Transaction::Transfer(TransferTx {
                meta: common.meta(),
                from: from.clone(),
                to: to.clone(),
                asset: asset.to_asset(),
                amount: *amount,
                fee: fee_of(fee),
                network: *network,
                tx_hash: text(tx_hash),
                debt: *debt,
                price: common.stamp(),
            }),
            Self::Convert {
                common,
                from,
                to,
                fee,
                rate_override,
            } => Transaction::Convert(ConvertTx {
                meta: common.meta(),
                from: ConvertSide {
                    account: from.account.clone(),
                    asset: from.asset.to_asset(),
                    amount: from.amount,
                    price: PriceStamp::default(),
                },
                to: ConvertSide {
                    account: to.account.clone(),
                    asset: to.asset.to_asset(),
                    amount: to.amount,
                    price: common.stamp(),
                },
                fee: fee_of(fee),
                rate_override: *rate_override,
            }),
            Self::TradeSpot {
                common,
                store_id,
                sold,
                bought,
                fee,
                liquidity,
                order_id,
            } => Transaction::TradeSpot(TradeSpotTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                sold: TradeSide {
                    asset: sold.asset.to_asset(),
                    qty: sold.qty,
                    price: PriceStamp::default(),
                },
                bought: TradeSide {
                    asset: bought.asset.to_asset(),
                    qty: bought.qty,
                    price: common.stamp(),
                },
                fee: fee_of(fee),
                liquidity: *liquidity,
                order_id: text(order_id),
            }),
            Self::TradeFutures {
                common,
                store_id,
                venue,
                symbol,
                settle,
                direction,
                action,
                quantity,
                fill_price,
                leverage,
                fee,
                closed_pnl,
                order_id,
                category,
            } => Transaction::TradeFutures(TradeFuturesTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                contract: AssetRef::perp(venue.trim(), symbol.trim()),
                settle: settle.to_asset(),
                direction: *direction,
                action: *action,
                quantity: *quantity,
                fill_price: *fill_price,
                leverage: *leverage,
                fee: fee_of(fee),
                closed_pnl: *closed_pnl,
                order_id: text(order_id),
                category: *category,
                price: common.stamp(),
            }),
            Self::Defi {
                common,
                store_id,
                action,
                asset,
                amount,
                protocol,
                pool_id,
                network,
                tx_hash,
                fee,
            } => Transaction::Defi(DefiTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                action: *action,
                asset: asset.to_asset(),
                amount: *amount,
                protocol: text(protocol),
                pool_id: text(pool_id),
                network: *network,
                tx_hash: text(tx_hash),
                fee: fee_of(fee),
                price: common.stamp(),
            }),
            Self::AssetSell {
                common,
                store_id,
                asset,
                unit_price,
                quantity,
                currency,
                proceeds_to,
                fee,
            } => Transaction::AssetSell(AssetSellTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                asset: asset.to_asset(),
                unit_price: *unit_price,
                quantity: *quantity,
                currency: currency.to_asset(),
                proceeds_to: text(proceeds_to),
                fee: fee_of(fee),
                price: common.stamp(),
            }),
        }
    }
}

/// Replace a stored transaction with a new version of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateTransactionInput {
    pub id: String,
    pub transaction: TransactionInput,
}

/// Which transaction kinds a wallet of one kind accepts, split the way the
/// form presents them, with the kind to pre-select.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TransactionKinds {
    /// Of the four everyday verbs, the ones this wallet accepts.
    pub verbs: Vec<TxKind>,
    /// Specialised kinds (trades, DeFi, selling property).
    pub special: Vec<TxKind>,
    pub default: TxKind,
}

impl TransactionKinds {
    pub fn for_store_kind(kind: StoreKind) -> Self {
        let allowed = allowed_kinds(kind);
        Self {
            verbs: allowed
                .iter()
                .copied()
                .filter(|tx_kind| tx_kind.is_verb())
                .collect(),
            special: allowed
                .iter()
                .copied()
                .filter(|tx_kind| !tx_kind.is_verb())
                .collect(),
            default: default_kind(kind),
        }
    }
}

// -- contacts ------------------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateContactInput {
    pub id: String,
    pub contact: ContactDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ContactTurnoverInput {
    pub contact_id: String,
    pub account_id: Option<String>,
}

// -- categories and rules --------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateCategoryInput {
    /// Wanted key; derived from the name when empty.
    pub key: Option<String>,
    pub name: String,
    pub group: CategoryGroup,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateCategoryInput {
    pub key: String,
    /// Empty = the built-in label.
    pub name: Option<String>,
    pub group: CategoryGroup,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateRuleInput {
    pub pattern: String,
    pub category: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateRuleInput {
    pub id: String,
    pub pattern: String,
    pub category: String,
    pub active: bool,
}

// -- property value --------------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SetManualPriceInput {
    pub asset_key: String,
    pub unit_price: Amount,
    pub currency: String,
}

// -- pricing -----------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct RefreshResult {
    pub prices: usize,
    pub fx: usize,
    /// Transaction sides that were saved without a price and got one now.
    pub restamped: usize,
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
    /// RFC3339 moment to price at; empty means now.
    #[serde(default)]
    pub at: Option<String>,
}

impl AssetPriceInput {
    pub fn to_asset(&self) -> AssetRef {
        let symbol = self.symbol.trim().to_uppercase();
        AssetRef {
            quote: symbol.clone(),
            name: symbol.clone(),
            symbol,
            coingecko_id: self.coingecko_id.clone().filter(|id| !id.is_empty()),
            chain: self.chain,
            contract: self
                .contract
                .clone()
                .filter(|contract| !contract.is_empty()),
        }
    }
}

/// Cached USD quote nearest the asked moment. `ts` is when that rate was
/// taken, empty when the asset was never priced; a hand-set price and USD
/// itself hold for all time and carry the asked moment.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CachedPrice {
    pub price_usd: Amount,
    pub ts: String,
}

/// Convert an amount between two currency codes at a moment (plan D20).
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ConvertAmountInput {
    pub from: String,
    pub to: String,
    pub amount: Amount,
    /// RFC3339; empty means now.
    #[serde(default)]
    pub at: Option<String>,
}

/// `amount` is `None` when a rate of either currency is unknown.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ConvertedAmount {
    pub amount: Option<Amount>,
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
    /// Share of income planned for savings, 0..1.
    pub savings_rate: Option<Amount>,
    pub daily_budget_mode: Option<DailyBudgetMode>,
    /// Empty string = follow the base currency.
    pub budget_currency: Option<String>,
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
    /// Sub-account spot fills are booked on; `None` = the key's own store.
    pub spot_store_id: Option<String>,
    /// Sub-account futures fills are booked on; `None` = the key's own store.
    pub futures_store_id: Option<String>,
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

/// Which sub-accounts imported spot and futures fills land on.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct SetExchangeTargetsInput {
    pub store_id: String,
    pub spot_store_id: Option<String>,
    pub futures_store_id: Option<String>,
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
