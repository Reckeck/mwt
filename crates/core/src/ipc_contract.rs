//! IPC command boundary types shared with the frontend.
//! These are exported to TypeScript by the `gen-types` binary (see `src/bin`).
//! Commands in the `app` crate only translate between these DTOs and the
//! domain services — never expose internal model directly.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::category::CategoryGroup;
use crate::model::chain::Chain;
use crate::model::planning::{
    AlertCondition, BudgetPeriod, BudgetTarget, Schedule, TransactionDraft,
};
use crate::model::store::StoreScope;
use crate::model::store::{
    AssetRef, Binding, Custody, ExchangeProvider, LiabilityTerms, PropertyInfo, Purpose, StoreKind,
    SubType,
};
use crate::model::transaction::{
    AccountRef, AdjustmentLine, AdjustmentTx, AssetSellTx, ConvertSide, ConvertTx, DebtEffect,
    DefiAction, DefiTx, Fee, FuturesAction, FuturesCategory, FuturesDirection, KindDetails,
    Liquidity, PaymentTx, PriceStamp, TopupTx, TradeFuturesTx, TradeSide, TradeSpotTx, Transaction,
    TransferTx, TxKind, TxMeta, allowed_kinds, default_kind,
};
use crate::model::vault::DailyBudgetMode;
use crate::service::{AutomationDraft, BudgetDraft, ContactDraft, GoalDraft, PriceAlertDraft};

// -- auth --------------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateVaultInput {
    pub path: String,
    pub password: String,
    /// Name of the cash wallet every vault starts with, in the user's language.
    #[serde(default)]
    pub cash_name: Option<String>,
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

/// How vault files are kept on this platform.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum VaultStorage {
    /// Desktop: a vault is any file the user picks, opened where it lies.
    #[default]
    Files,
    /// Android/iOS: vaults live in the app's own storage; a picked file is
    /// copied in, and an external copy is kept by linking it.
    App,
}

/// What the unlock screen needs to know about the app.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct VaultStatus {
    pub path: String,
    pub exists: bool,
    pub unlocked: bool,
    pub info: Option<VaultInfo>,
    #[serde(default)]
    pub storage: VaultStorage,
}

// -- stores ------------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateStoreInput {
    pub name: String,
    /// Ignored for a sub-account: it always has its wallet's kind.
    pub kind: StoreKind,
    pub color: String,
    pub note: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Sub-accounts: the template, from `StoreKind::sub_types`.
    #[serde(default)]
    pub sub_type: Option<SubType>,
    /// Sub-accounts: what it is locked to; market types fill it themselves.
    #[serde(default)]
    pub binding: Option<Binding>,
    /// `None` = the default of the sub-account type, else of the kind.
    #[serde(default)]
    pub purpose: Option<Purpose>,
    /// Loan terms; for `liability` wallets and credit cards.
    #[serde(default)]
    pub liability: Option<LiabilityTerms>,
    /// Contact the wallet belongs to; top-level only.
    #[serde(default)]
    pub contact_id: Option<String>,
    #[serde(default)]
    pub custody: Option<Custody>,
    #[serde(default)]
    pub property: Option<PropertyInfo>,
    #[serde(default)]
    pub secured_by: Option<String>,
    /// Exchange wallets: which exchange it is.
    #[serde(default)]
    pub provider: Option<ExchangeProvider>,
    /// Money the wallet already holds; booked as one opening top-up. With
    /// `card` it goes on the card.
    #[serde(default)]
    pub opening: Option<crate::service::OpeningBalance>,
    /// A new bank: its first card, created with it (a card is a bank account).
    #[serde(default)]
    pub card: Option<crate::service::FirstCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateEnvelopeInput {
    pub id: String,
    pub draft: crate::service::EnvelopeDraft,
}

/// `amount > 0` earmarks from the free balance, `< 0` releases.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct FundEnvelopeInput {
    pub id: String,
    pub amount: Amount,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct MoveEnvelopeInput {
    pub from: String,
    pub to: String,
    pub amount: Amount,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SetImportIntervalInput {
    pub store_id: String,
    /// Seconds; `0` turns the background import off.
    pub interval_sec: u32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateTerminalStateInput {
    pub store_id: String,
    pub patch: crate::service::TerminalPatch,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SaveTerminalDrawingsInput {
    pub store_id: String,
    pub symbol: String,
    /// The chart drawing manager's JSON export; empty or `[]` removes it.
    pub json: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateStoreInput {
    pub id: String,
    pub name: String,
    pub color: String,
    pub note: String,
    /// `None` keeps the current value.
    #[serde(default)]
    pub purpose: Option<Purpose>,
    /// Sub-accounts; `None` keeps the current binding. Refused when booked
    /// operations would no longer fit.
    #[serde(default)]
    pub binding: Option<Binding>,
    #[serde(default)]
    pub liability: Option<LiabilityTerms>,
    #[serde(default)]
    pub contact_id: Option<String>,
    /// `None` keeps the current value.
    #[serde(default)]
    pub custody: Option<Custody>,
    /// `None` keeps the current value.
    #[serde(default)]
    pub property: Option<PropertyInfo>,
    #[serde(default)]
    pub secured_by: Option<String>,
    /// `None` keeps the current exchange.
    #[serde(default)]
    pub provider: Option<ExchangeProvider>,
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
    /// Manual price of one unit in `price_currency`; turned into `price_usd`
    /// at the rate of the transaction's moment before saving.
    #[serde(default)]
    pub price: Option<Amount>,
    /// Lowercase code `price` is in; empty means the base currency.
    #[serde(default)]
    pub price_currency: Option<String>,
    /// Set by the service when `price` was given in the base currency.
    #[serde(default)]
    #[ts(skip)]
    pub price_base: Option<Amount>,
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
            price_base: self.price_base,
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
    /// Set the balances of one account to what is really there.
    Adjustment {
        common: TxCommonInput,
        store_id: String,
        lines: Vec<AdjustmentLineInput>,
    },
}

/// One asset of a correction: the balance really there. What the ledger
/// held is worked out when it is saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AdjustmentLineInput {
    pub asset: AssetIdent,
    pub actual: Amount,
}

/// What the correction screen opens with: every asset of the account and
/// its ledger balance at `at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AdjustmentDraftInput {
    pub store_id: String,
    /// RFC3339; empty = now.
    pub at: Option<String>,
    /// The correction being edited: left out of the balances.
    pub replaces: Option<String>,
}

fn text(raw: &Option<String>) -> Option<String> {
    raw.as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

impl TransactionInput {
    /// Build the domain transaction. Rules are checked by the service, not here.
    /// The fields every kind shares.
    pub fn common_mut(&mut self) -> &mut TxCommonInput {
        match self {
            Self::Topup { common, .. }
            | Self::Payment { common, .. }
            | Self::Transfer { common, .. }
            | Self::Convert { common, .. }
            | Self::TradeSpot { common, .. }
            | Self::TradeFutures { common, .. }
            | Self::Defi { common, .. }
            | Self::AssetSell { common, .. }
            | Self::Adjustment { common, .. } => common,
        }
    }

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
            Self::Adjustment {
                common,
                store_id,
                lines,
            } => Transaction::Adjustment(AdjustmentTx {
                meta: common.meta(),
                store_id: store_id.clone(),
                lines: lines
                    .iter()
                    .map(|line| AdjustmentLine {
                        asset: line.asset.to_asset(),
                        // Filled in from the ledger when it is saved.
                        before: Amount::ZERO,
                        after: line.actual,
                        price: PriceStamp::default(),
                    })
                    .collect(),
                fee: None,
            }),
        }
    }
}

/// Several operations saved as one chain: all or none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AddLinkedTransactionsInput {
    pub transactions: Vec<TransactionInput>,
    /// Join this chain; `None` starts a new one.
    #[serde(default)]
    pub flow_id: Option<String>,
}

/// Replace a stored transaction with a new version of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateTransactionInput {
    pub id: String,
    pub transaction: TransactionInput,
}

/// A draft to preview; `replaces` is the id of the transaction being edited.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct PreviewTransactionInput {
    pub transaction: TransactionInput,
    pub replaces: Option<String>,
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
                .filter(|tx_kind| !tx_kind.is_verb() && tx_kind.in_form())
                .collect(),
            default: default_kind(kind),
        }
    }
}

// -- wallet catalog -------------------------------------------------------------

/// Binding shape of a sub-account type, as the creation form needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum BindingShapeName {
    Asset,
    Pair,
    Market,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SubTypeInfo {
    pub sub_type: SubType,
    pub shape: BindingShapeName,
    pub default_purpose: Option<Purpose>,
    /// Carries loan terms (a credit card).
    pub credit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PresetInfo {
    pub preset: crate::model::planning::AutomationPreset,
    pub tx_kind: TxKind,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct WalletKindInfo {
    pub kind: StoreKind,
    pub presets: Vec<PresetInfo>,
    pub max_roots: Option<u32>,
    pub user_creatable: bool,
    pub default_purpose: Purpose,
    pub sub_types: Vec<SubTypeInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ExchangeProviderInfo {
    pub provider: ExchangeProvider,
    /// The app can connect to it; others are shown as «soon».
    pub supported: bool,
}

/// What every wallet kind allows, in sidebar order. Static domain data, so the
/// UI never duplicates these rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct WalletCatalog {
    pub kinds: Vec<WalletKindInfo>,
    pub providers: Vec<ExchangeProviderInfo>,
}

impl WalletCatalog {
    pub fn build() -> Self {
        use crate::model::store::BindingShape;
        let kinds = StoreKind::ALL
            .iter()
            .map(|&kind| WalletKindInfo {
                kind,
                presets: kind
                    .automation_presets()
                    .iter()
                    .map(|&preset| PresetInfo {
                        preset,
                        tx_kind: preset.tx_kind(),
                        category: preset.category().map(str::to_string),
                    })
                    .collect(),
                max_roots: kind.max_roots().map(|max| max as u32),
                user_creatable: kind.user_creatable(),
                default_purpose: kind.default_purpose(),
                sub_types: kind
                    .sub_types()
                    .iter()
                    .map(|&sub_type| SubTypeInfo {
                        sub_type,
                        shape: match sub_type.binding_shape() {
                            BindingShape::Asset => BindingShapeName::Asset,
                            BindingShape::Pair => BindingShapeName::Pair,
                            BindingShape::Market(_) => BindingShapeName::Market,
                        },
                        default_purpose: sub_type.default_purpose(),
                        credit: sub_type.is_credit(),
                    })
                    .collect(),
            })
            .collect();
        let providers = [
            ExchangeProvider::Bybit,
            ExchangeProvider::Binance,
            ExchangeProvider::Okx,
        ]
        .into_iter()
        .map(|provider| ExchangeProviderInfo {
            provider,
            supported: provider.supported(),
        })
        .collect();
        Self { kinds, providers }
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

/// Whether the «+ Операция» key can be registered by the app, and the command
/// a compositor binding runs instead when it cannot (Wayland).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ShortcutSupport {
    pub global_keys: bool,
    /// The running executable with `--quick-add`.
    pub launch_command: String,
}

/// Set up a bonus programme.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SetLoyaltyInfoInput {
    pub store_id: String,
    pub info: crate::model::store::LoyaltyInfo,
}

/// Credit (positive) or spend (negative) points of a programme.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct AdjustPointsInput {
    pub store_id: String,
    pub delta: Amount,
    pub comment: String,
    /// The card whose purchases earned the points.
    pub source_store_id: Option<String>,
    /// RFC3339; empty means now.
    pub timestamp: Option<String>,
}

/// Set or clear the interest terms of a savings account, deposit or ПДС.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SetDepositTermsInput {
    pub store_id: String,
    pub terms: Option<crate::model::deposit::DepositTerms>,
}

/// Create the missing Funding / Unified / Earn sub-accounts of a Bybit wallet.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct CreateBybitAccountsInput {
    pub store_id: String,
    /// Names in the UI language, for the ones that get created.
    pub funding_name: String,
    pub unified_name: String,
    pub earn_name: String,
    /// Ask Bybit which accounts hold anything and create only those (the
    /// unified one always); without the network all three are created.
    #[serde(default)]
    pub detect: bool,
}

/// One coin of one exchange account: the exchange's figure next to the ledger's.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ExchangeCoinRow {
    pub coin: String,
    pub exchange: Amount,
    pub ledger: Amount,
    /// `exchange − ledger`: what imports have not explained yet.
    pub diff: Amount,
}

/// A Bybit account (`FUND`, `UNIFIED`, `EARN`) and the sub-account standing for it.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ExchangeAccountBalance {
    pub account: String,
    /// `None` when the wallet has no sub-account of that kind yet.
    pub store_id: Option<String>,
    pub rows: Vec<ExchangeCoinRow>,
}

/// Make sure the FX cache has a rate of each currency for a day, fetching from
/// the public sources (and earlier days) what it lacks.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct EnsureFxInput {
    /// Lowercase codes; USD and unknown codes are ignored.
    pub currencies: Vec<String>,
    /// `YYYY-MM-DD` (UTC); empty or absent means today.
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct EnsureFxOutput {
    /// Rates fetched just now; a point's `ts` is the day its source had, which
    /// may be before the day asked for.
    pub points: Vec<crate::model::vault::FxPoint>,
    /// Currencies no source had within the look-back window.
    pub missing: Vec<String>,
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

// -- planning (plan §7) ---------------------------------------------------------

/// An automation as the form edits it. The template is an ordinary
/// transaction input of one of the four verbs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AutomationInput {
    pub name: String,
    pub active: bool,
    #[serde(default)]
    pub preset: Option<crate::model::planning::AutomationPreset>,
    pub schedule: Schedule,
    /// RFC3339, the first planned date.
    pub starts_at: String,
    pub template: TransactionInput,
    /// The same amount every time, or one that differs (a salary).
    #[serde(default)]
    pub amount_mode: crate::model::planning::AmountMode,
    /// Where a planned date on a weekend or a holiday goes.
    #[serde(default)]
    pub workday_shift: crate::model::planning::WorkdayShift,
}

impl AutomationInput {
    pub fn to_draft(&self) -> Result<AutomationDraft> {
        let template = match self.template.to_transaction() {
            Transaction::Topup(inner) => TransactionDraft::Topup(inner),
            Transaction::Payment(inner) => TransactionDraft::Payment(inner),
            Transaction::Transfer(inner) => TransactionDraft::Transfer(inner),
            Transaction::Convert(inner) => TransactionDraft::Convert(inner),
            _ => {
                return Err(Error::InvalidInput(
                    "an automation repeats a top-up, payment, transfer or exchange".into(),
                ));
            }
        };
        Ok(AutomationDraft {
            name: self.name.clone(),
            active: self.active,
            preset: self.preset,
            schedule: self.schedule,
            starts_at: self.starts_at.clone(),
            template,
            amount_mode: self.amount_mode,
            workday_shift: self.workday_shift,
        })
    }
}

/// The real amount of an operation booked approximately.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ConfirmEstimatedInput {
    pub tx_id: String,
    /// A decimal string, like every amount the app takes.
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateAutomationInput {
    pub id: String,
    pub automation: AutomationInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct BudgetInput {
    pub active: bool,
    pub target: BudgetTarget,
    pub period: BudgetPeriod,
    pub limit: Amount,
    pub currency: String,
}

impl From<BudgetInput> for BudgetDraft {
    fn from(input: BudgetInput) -> Self {
        Self {
            active: input.active,
            target: input.target,
            period: input.period,
            limit: input.limit,
            currency: input.currency,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateBudgetInput {
    pub id: String,
    pub budget: BudgetInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct GoalInput {
    pub name: String,
    pub active: bool,
    pub target_amount: Amount,
    pub currency: String,
    pub starts_at: String,
    pub deadline: Option<String>,
    /// Importance, 1–5.
    pub priority: u8,
    pub monthly_commitment: Option<Amount>,
    pub scope: StoreScope,
    /// The calculator scenario the goal follows.
    #[serde(default)]
    pub scenario_id: Option<String>,
}

impl From<GoalInput> for GoalDraft {
    fn from(input: GoalInput) -> Self {
        Self {
            name: input.name,
            active: input.active,
            target_amount: input.target_amount,
            currency: input.currency,
            starts_at: input.starts_at,
            deadline: input.deadline.filter(|value| !value.trim().is_empty()),
            priority: input.priority,
            monthly_commitment: input.monthly_commitment,
            scope: input.scope,
            scenario_id: input.scenario_id.filter(|value| !value.is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateGoalInput {
    pub id: String,
    pub goal: GoalInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ReminderInput {
    pub text: String,
    /// RFC3339: the date and the time.
    pub due_at: String,
    /// `None` = once.
    #[serde(default)]
    pub repeat: Option<crate::model::planning::ReminderRepeat>,
}

/// Emulate the automations ahead: one of them, or all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SimulateAutomationsInput {
    #[serde(default)]
    pub automation_id: Option<String>,
    /// How many days ahead, 1–366.
    pub days: u32,
}

/// Put a reminder off until a moment (RFC3339).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SnoozeReminderInput {
    pub id: String,
    pub until: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateReminderInput {
    pub id: String,
    pub reminder: ReminderInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PriceAlertInput {
    pub asset: AssetIdent,
    pub condition: AlertCondition,
    pub price: Amount,
    pub currency: String,
    pub active: bool,
    /// The exchange whose terminal sets the alert.
    #[serde(default)]
    #[ts(optional)]
    pub store_id: Option<String>,
}

impl From<PriceAlertInput> for PriceAlertDraft {
    fn from(input: PriceAlertInput) -> Self {
        Self {
            asset: input.asset.to_asset(),
            condition: input.condition,
            price: input.price,
            currency: input.currency,
            active: input.active,
            store_id: input.store_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdatePriceAlertInput {
    pub id: String,
    pub alert: PriceAlertInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UtilityInput {
    pub name: String,
    pub active: bool,
    pub kind: crate::model::planning::UtilityKind,
    pub currency: String,
    /// The property the bill belongs to.
    pub property_id: String,
    pub store_id: Option<String>,
    /// Category key of the payments.
    pub category: String,
}

impl From<UtilityInput> for crate::service::UtilityDraft {
    fn from(input: UtilityInput) -> Self {
        Self {
            name: input.name,
            active: input.active,
            kind: input.kind,
            currency: input.currency,
            property_id: input.property_id,
            store_id: input.store_id.filter(|id| !id.trim().is_empty()),
            category: input.category,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateUtilityInput {
    pub id: String,
    pub item: UtilityInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct MeterReadingInput {
    pub item_id: String,
    pub value: Amount,
    /// `YYYY-MM`.
    pub period: String,
    /// Day of the month (1–31); `None` = the month's end.
    #[serde(default)]
    pub day: Option<u8>,
    /// The zone of a multi-rate meter; `None` for a one-rate one.
    #[serde(default)]
    pub zone: Option<String>,
}

/// Correct a recorded meter reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateMeterReadingInput {
    pub id: String,
    pub value: Amount,
    /// `YYYY-MM`.
    pub period: String,
    /// Day of the month (1–31); `None` = the month's end.
    #[serde(default)]
    pub day: Option<u8>,
}

/// Payload of `automation-rejected`: one run the scheduler could not create.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AutomationRejectedEvent {
    pub run_id: String,
    pub automation_id: String,
    pub name: String,
    pub planned_at: String,
    pub reason: crate::model::planning::RejectReason,
}

// -- chains --------------------------------------------------------------------

/// Put transactions into one chain: `flow_id`, else the chain one of them is
/// already in, else a new one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct LinkTransactionsInput {
    pub ids: Vec<String>,
    #[serde(default)]
    pub flow_id: Option<String>,
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
    /// Share of income planned for savings, 0..1.
    pub savings_rate: Option<Amount>,
    pub daily_budget_mode: Option<DailyBudgetMode>,
    /// Empty string = follow the base currency.
    pub budget_currency: Option<String>,
    /// What to be told about and how far ahead.
    #[serde(default)]
    pub notifications: Option<crate::model::notify::NotificationSettings>,
    /// Replaces the list of checklist steps marked «Не нужно».
    #[serde(default)]
    pub onboarding_skipped: Option<Vec<String>>,
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
    /// The sync as kept in the history; `None` without a key.
    #[serde(default)]
    pub run: Option<crate::model::vault::ImportRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SetImportSettingsInput {
    pub store_id: String,
    pub settings: crate::model::store::ImportSettings,
}

/// What a sync would bring; `notice` (`no_key`, `unreachable`, …) when the
/// exchange could not be asked.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ImportPreviewOutput {
    pub preview: Option<crate::service::ImportPreview>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct ListImportedTransactionsInput {
    pub store_id: String,
    /// Default 50, at most 500.
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
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
    /// File name to show, whatever the OS's separator.
    #[serde(default)]
    pub display_name: String,
    /// Saves are copied to an external document.
    #[serde(default)]
    pub linked: bool,
}

/// Input to select or pick a vault file.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct SelectVaultInput {
    pub path: String,
}

/// A vault file chosen in the system dialog.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct PickedVault {
    /// Local path to open or create (a copy in app storage on mobile).
    pub path: String,
    pub display_name: String,
    /// The picked document was copied into app storage.
    pub imported: bool,
}

/// Keep a copy of the unlocked vault in an external document.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct LinkVaultInput {
    /// Pick an existing file (it gets replaced by the vault) instead of
    /// creating a new one.
    pub existing: bool,
}

/// The external copy of the unlocked vault, if any.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct VaultLinkStatus {
    pub linked: bool,
    pub display_name: Option<String>,
    /// RFC 3339 time of the last successful copy.
    pub last_synced_at: Option<String>,
    /// Why the last copy failed; cleared by the next successful one.
    pub last_error: Option<String>,
}

/// Which copy wins after `SYNC_CONFLICT`.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSide {
    #[default]
    Local,
    Remote,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct ResolveVaultConflictInput {
    pub path: String,
    pub keep: ConflictSide,
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

// -- node workflows (plan §16) -------------------------------------------------

/// Create (`id` empty) or update a workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SaveWorkflowInput {
    #[serde(default)]
    pub id: Option<String>,
    pub workflow: crate::service::WorkflowDraft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SetWorkflowActiveInput {
    pub id: String,
    pub active: bool,
}

/// Emulate a graph — saved or still in the editor — on a copy of the vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DryRunWorkflowInput {
    /// The graph as JSON.
    pub graph: String,
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// The trigger to start from; a manual one, else the first, by default.
    #[serde(default)]
    pub trigger_node: Option<String>,
    /// Answers to «Ждать подтверждения» given so far, by node id; a question
    /// without one pauses the emulation (outcome `asking`).
    #[serde(default)]
    #[ts(optional)]
    pub answers: Option<std::collections::BTreeMap<String, bool>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SaveWorkflowDraftInput {
    pub id: String,
    /// The editor's graph as JSON.
    pub graph: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AnswerWorkflowWaitInput {
    pub id: String,
    /// «Да» (true) or «Нет» (false).
    pub yes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct RunWorkflowInput {
    pub id: String,
    #[serde(default)]
    pub trigger_node: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ListWorkflowRunsInput {
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// Newest first, 100 by default.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct RestoreWorkflowVersionInput {
    pub id: String,
    pub saved_at: String,
}

/// Set a variable by hand; `value` is a `WfValue` in JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SetWorkflowVarInput {
    pub scope: crate::workflow::VarScope,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DeleteWorkflowVarInput {
    pub scope: crate::workflow::VarScope,
    pub name: String,
}

/// A graph and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ValidateWorkflowInput {
    pub graph: String,
}

/// Add a test to a workflow (empty `test.id`) or replace one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SaveWorkflowTestInput {
    pub workflow_id: String,
    pub test: crate::workflow::WorkflowTest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DeleteWorkflowTestInput {
    pub workflow_id: String,
    pub test_id: String,
}

// -- device sync (docs/sync.md) ---------------------------------------------------

/// This device's place in the sync group.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SyncRole {
    /// Sync is off, or the vault is locked.
    #[default]
    Off,
    /// Looking for the group's main device.
    Searching,
    /// The other devices connect here; the background jobs run here.
    Main,
    /// Connected to the main device.
    Client,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    #[default]
    Connecting,
    /// A device with this vault that is not trusted yet: the banner offers pairing.
    Offer,
    /// Both screens show the code.
    Pairing,
    /// Manifests and records are crossing.
    Exchanging,
    /// The list of changes waits for the user.
    AwaitingConfirm,
    /// In sync; changes go across as they are saved.
    Live,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncProgress {
    pub received: u32,
    pub total: u32,
}

/// One connection to another device.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncSessionView {
    pub session_id: String,
    pub device_id: String,
    pub name: String,
    pub phase: SyncPhase,
    /// Its clock jumped this session: every batch from it waits for confirmation.
    pub unreliable: bool,
    /// Its clock minus ours, ms (shown when beyond 5 minutes).
    #[ts(type = "number")]
    pub clock_offset_ms: i64,
    pub progress: Option<SyncProgress>,
    /// The other device's window has focus.
    pub focused: bool,
    /// While pairing: the code both screens show.
    pub pairing_code: Option<String>,
    /// The list of changes waiting for the user, if any.
    pub plan: Option<SyncPendingPlan>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncPendingPlan {
    pub reason: SyncConfirmReason,
    pub summary: crate::sync::SyncSummary,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncStatus {
    pub enabled: bool,
    pub role: SyncRole,
    pub device_id: String,
    pub device_name: String,
    pub confirm_first: bool,
    pub prefer_main: bool,
    /// Where this device's key lives: `hardware`, `system` (OS keyring) or
    /// `memory` (gone after a restart: pairing again).
    pub key_storage: String,
    pub sessions: Vec<SyncSessionView>,
    /// A backup from before a sync exists («Восстановить до синхронизации»).
    pub has_backup: bool,
    /// Sync goes on in the background here (Android: a foreground service).
    pub background_supported: bool,
    /// Minutes in the background before an Android device locks (0 = never).
    pub background_lock_min: u32,
    /// What the network looks like from here (the diagnostics line).
    pub network: SyncNetworkView,
}

/// How a firewall that drops incoming sync connections can be opened here.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SyncFirewallFix {
    /// Nothing to open (Android, iOS, macOS asks by itself).
    #[default]
    None,
    /// `sync_open_firewall` opens it (asks for the system password).
    Auto,
    /// Only by hand: the user runs [`SyncNetworkView::firewall_commands`]
    /// (Flatpak cannot reach the host firewall).
    Manual,
}

/// The diagnostics of sync on the local network.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncNetworkView {
    /// The TCP port this device listens on while sync runs.
    pub port: Option<u16>,
    /// This device's address on the local network.
    pub address: Option<String>,
    /// Devices with this vault seen nearby right now.
    pub seen: u32,
    /// When this device started looking (ms since the epoch).
    #[ts(type = "number | null")]
    pub since_ms: Option<i64>,
    /// Another device could not connect here: a firewall drops incoming connections.
    pub inbound_blocked: bool,
    pub firewall_fix: SyncFirewallFix,
    /// The commands that open the port by hand.
    pub firewall_commands: Vec<String>,
    /// The code of the last sync error (`SYNC_*`), cleared by a session that goes live.
    pub last_error: Option<String>,
}

/// Where handing a vault to a new device stands (either side).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    #[default]
    Idle,
    /// Receiving: visible nearby, no request yet. Sending: waiting for the
    /// other device to accept.
    Waiting,
    /// Receiving: a device asks to hand its vault over («Принять / Отклонить»).
    Request,
    /// Receiving: showing the code. Sending: asking for it.
    Code,
    Transferring,
    Done,
    Failed,
}

/// The receiving side (`transfer-receive` event, `transfer_receive_*`).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TransferReceiveView {
    pub phase: TransferPhase,
    /// The name other devices see this one by.
    pub device_name: String,
    /// When this device stops being visible (ms since the epoch).
    #[ts(type = "number | null")]
    pub expires_ms: Option<i64>,
    pub sender_name: Option<String>,
    /// The six digits to type on the sending device.
    pub code: Option<String>,
    #[ts(type = "number")]
    pub done_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// Where the received vault was saved (to open it).
    pub saved_path: Option<String>,
    /// `SYNC_*` code of what went wrong.
    pub error: Option<String>,
}

/// The sending side (`transfer-send` event, `transfer_send*`).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TransferSendView {
    pub phase: TransferPhase,
    /// The receiving device.
    pub device_name: String,
    /// Tries left for the code.
    pub attempts_left: u32,
    /// The last code typed was wrong.
    pub wrong_code: bool,
    #[ts(type = "number")]
    pub done_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub error: Option<String>,
}

/// A device nearby waiting to receive a vault (`transfer-nearby` event).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TransferNearby {
    pub id: String,
    pub name: String,
    /// `mobile` or `desktop`.
    pub kind: String,
}

/// «Передать»: to whom, and the vault password typed again.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct TransferSendInput {
    pub id: String,
    pub password: String,
}

/// A trusted device as the settings list shows it.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncPeerView {
    pub device_id: String,
    pub name: String,
    pub paired_at: String,
    pub last_synced_at: Option<String>,
    pub online: bool,
}

/// Partial update of this device's sync settings.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncSettingsInput {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub confirm_first: Option<bool>,
    #[serde(default)]
    pub prefer_main: Option<bool>,
    /// An empty name goes back to the host name.
    #[serde(default)]
    pub device_name: Option<String>,
    /// Android: minutes in the background before the vault locks (0 = never).
    #[serde(default)]
    pub background_lock_min: Option<u32>,
}

/// `sync-offer`: a device with this vault the user has not paired with.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncOfferEvent {
    pub session_id: String,
    pub peer_name: String,
}

/// `sync-pairing`: show the code; both screens must show the same one.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncPairingEvent {
    pub session_id: String,
    pub peer_name: String,
    pub code: String,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SyncConfirmReason {
    /// «Спрашивать перед синхронизацией» is on: the first batch of a session.
    #[default]
    First,
    /// Just paired: the first sync with this device.
    Paired,
    /// The device's clock jumped this session.
    Unreliable,
}

/// `sync-plan-ready`: what a batch would change, waiting for the user.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncPlanEvent {
    pub session_id: String,
    pub peer_name: String,
    pub reason: SyncConfirmReason,
    pub summary: crate::sync::SyncSummary,
}

/// `vault-synced`: a batch was applied here.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct VaultSyncedEvent {
    pub session_id: String,
    pub batch: crate::service::AppliedBatch,
}

/// `sync-error`: a session ended or refused to start; `code` is a `SYNC_*` code.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncErrorEvent {
    pub code: String,
    pub message: String,
    pub peer_name: Option<String>,
}

/// `sync-clock`: the other device's clock differs or jumped.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncClockEvent {
    pub session_id: String,
    pub peer_name: String,
    #[ts(type = "number")]
    pub offset_ms: i64,
    pub unreliable: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncDecisionInput {
    pub session_id: String,
    pub accept: bool,
}
