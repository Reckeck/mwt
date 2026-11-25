#![warn(unreachable_pub)]
//! IPC command boundary types shared with the frontend.
//! These are exported to TypeScript by the `gen-types` binary of `mwt-usecases`
//! (`crates/usecases/src/bin/gen_types.rs`).
//! Commands in the `app` crate only translate between these DTOs and the
//! domain services — never expose internal model directly.

use std::fmt;

use serde::{Deserialize, Serialize};

use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_base::secret::Hidden;
use mwt_household::contacts::ContactDraft;
use mwt_model::CategoryGroup;
use mwt_model::Chain;
use mwt_model::DailyBudgetMode;
use mwt_model::EntityRef;
use mwt_model::StoreScope;
use mwt_model::{
    AccountRef, AdjustmentLine, AdjustmentTx, AssetSellTx, ConvertSide, ConvertTx, DebtEffect,
    DefiAction, DefiTx, Fee, FuturesAction, FuturesCategory, FuturesDirection, KindDetails,
    Liquidity, OpPeriod, PaymentTx, PriceStamp, ReceivedPart, Spread, TopupTx, TradeFuturesTx,
    TradeSide, TradeSpotTx, Transaction, TransferTx, TxKind, TxMeta, allowed_kinds, default_kind,
};
use mwt_model::{AlertCondition, BudgetPeriod, BudgetTarget, Schedule, TransactionDraft};
use mwt_model::{
    AssetRef, Binding, Custody, ExchangeProvider, LiabilityTerms, PropertyInfo, Purpose, StoreKind,
    SubType,
};
use mwt_model::{DistKind, DistributionPart, DistributionRow, EnvelopeMove, OnOtherAmount};
use mwt_planning::agenda::PriceAlertDraft;
use mwt_planning::automations::AutomationDraft;
use mwt_planning::budget::BudgetDraft;
use mwt_planning::distribution::ResolvedDistribution;
use mwt_planning::goals::GoalDraft;
use mwt_planning::templates::TemplateDraft;

pub use mwt_household::shared_events::inputs::{
    AttachPaymentInput, SetShareOverrideInput, SetSharedDueInput, SetSharedItemSplitInput,
    SettleSharedInput, SharedEventDraft, SharedItemDraft, SharedParticipantDraft,
    SharedPurchaseDraft, SharedRemainderInput,
};

pub mod cryptotrack;
pub mod events;
pub mod security;

// -- auth --------------------------------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateVaultInput {
    pub path: String,
    /// `None` makes a vault without a password (its file is not encrypted);
    /// an empty string is refused.
    #[serde(default)]
    pub password: Option<String>,
    /// Name of the cash wallet every vault starts with, in the user's language.
    #[serde(default)]
    pub cash_name: Option<String>,
}

impl fmt::Debug for CreateVaultInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("CreateVaultInput")
            .field("path", &self.path)
            .field("password", &Hidden)
            .field("cash_name", &self.cash_name)
            .finish()
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UnlockInput {
    pub path: String,
    pub password: String,
}

impl fmt::Debug for UnlockInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("UnlockInput")
            .field("path", &self.path)
            .field("password", &Hidden)
            .finish()
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChangePasswordInput {
    pub current_password: String,
    pub new_password: String,
}

impl fmt::Debug for ChangePasswordInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ChangePasswordInput")
            .field("current_password", &Hidden)
            .field("new_password", &Hidden)
            .finish()
    }
}

/// `serde(default)` for `encrypted`: a payload without it is a vault with a password.
fn yes() -> bool {
    true
}

/// Set a password on the open vault that has none.
#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetVaultPasswordInput {
    pub new_password: String,
    /// Also delete this vault's `.bak` copies that are not encrypted.
    #[serde(default)]
    pub delete_plain_copies: bool,
}

impl fmt::Debug for SetVaultPasswordInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("SetVaultPasswordInput")
            .field("new_password", &Hidden)
            .field("delete_plain_copies", &self.delete_plain_copies)
            .finish()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetVaultPasswordResult {
    /// How many unencrypted copies were deleted.
    pub removed_copies: u32,
}

/// Take the password off the open vault.
#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RemoveVaultPasswordInput {
    pub current_password: String,
    /// The user agreed that the external copy loses its password too. Without
    /// it a vault with an external copy keeps its password (`INVALID_INPUT`).
    #[serde(default)]
    pub external_copy_too: bool,
}

impl fmt::Debug for RemoveVaultPasswordInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("RemoveVaultPasswordInput")
            .field("current_password", &Hidden)
            .field("external_copy_too", &self.external_copy_too)
            .finish()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VaultInfo {
    pub path: String,
    pub doc_version: u32,
    pub id: String,
    pub created_at: String,
    pub format: String,
    pub base_currency: String,
    /// The vault has a password; `false` = its file is not encrypted.
    #[serde(default = "yes")]
    pub encrypted: bool,
}

/// How vault files are kept on this platform.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VaultStatus {
    pub path: String,
    pub exists: bool,
    pub unlocked: bool,
    pub info: Option<VaultInfo>,
    #[serde(default)]
    pub storage: VaultStorage,
    /// The vault at `path` has a password (so the unlock screen asks for it
    /// and the lock button is shown). `true` when the file cannot be read.
    #[serde(default = "yes")]
    pub encrypted: bool,
    /// The open vault is the sandbox (demo data, erased on exit); the unlock
    /// screen is for the real vault, the sandbox bar shows while this is set.
    #[serde(default)]
    pub sandbox: bool,
}

// -- stores ------------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// An exchange's futures sub-account: the coin its margin is kept in.
    #[serde(default)]
    pub margin: Option<AssetRef>,
    /// Money the wallet already holds; booked as one opening top-up. With
    /// `card` it goes on the card.
    #[serde(default)]
    pub opening: Option<mwt_money::stores::OpeningBalance>,
    /// A new bank: its first card, created with it (a card is a bank account).
    #[serde(default)]
    pub card: Option<mwt_money::stores::FirstCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateEnvelopeInput {
    pub id: String,
    pub draft: mwt_planning::envelopes::EnvelopeDraft,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<mwt_planning::envelopes::EnvelopeDraft>,
}

/// `amount > 0` earmarks from the free balance, `< 0` releases.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FundEnvelopeInput {
    pub id: String,
    pub amount: Amount,
}

/// Put aside `amount` (goal currency) for a goal in one of its wallets.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FundGoalInput {
    pub goal_id: String,
    pub store_id: String,
    pub amount: Amount,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MoveEnvelopeInput {
    pub from: String,
    pub to: String,
    pub amount: Amount,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetImportIntervalInput {
    pub store_id: String,
    /// Seconds; `0` turns the background import off.
    pub interval_sec: u32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateTerminalStateInput {
    pub store_id: String,
    pub patch: mwt_cryptotrack::exchange_sync::TerminalPatch,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SaveTerminalDrawingsInput {
    pub store_id: String,
    pub symbol: String,
    /// The chart drawing manager's JSON export; empty or `[]` removes it.
    pub json: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<mwt_model::Store>,
}

/// Re-hang a sub-account; `parent_id: None` makes it a top-level wallet.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MoveStoreInput {
    pub id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

// -- transactions ------------------------------------------------------------
//
// One input enum, internally tagged with `type` — an IPC shape only, never
// persisted. Each variant mirrors its
// domain transaction minus what the backend fills in: id, origin, prices.

/// Shared asset identification, so each variant does not repeat five
/// optional fields it does not always need.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    #[cfg_attr(feature = "ts", ts(skip))]
    pub price_base: Option<Amount>,
    /// Set together with `price_base`: the base currency it is in.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub price_base_currency: Option<String>,
    /// The property the operation belongs to (plan G4); empty — none.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub property_id: Option<String>,
    /// What the operation is «for» (spec P1); none lets a rule decide (D3).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period: Option<OpPeriod>,
    /// How an operation over several months is cut into months (spec P5).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub spread: Option<Spread>,
    /// «Really no period»: a new operation without a period takes no rule (spec D3).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub no_period_rule: bool,
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
        meta.period = self.period.clone();
        meta.spread = self.spread.clone();
        meta
    }

    fn stamp(&self) -> PriceStamp {
        PriceStamp {
            price_usd: self.price_usd,
            price_base: self.price_base,
            price_base_currency: self.price_base_currency.clone(),
            fx_override: self.fx_override,
        }
    }
}

/// One receiving part of an income that arrived in parts (spec S1).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReceivedPartInput {
    pub store_id: String,
    pub amount: Amount,
    /// The part's own date; `None` = the income's `timestamp`.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub timestamp: Option<String>,
}

impl ReceivedPartInput {
    fn to_part(&self) -> ReceivedPart {
        ReceivedPart {
            store_id: self.store_id.clone(),
            amount: self.amount,
            timestamp: self.timestamp.clone().filter(|stamp| !stamp.is_empty()),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConvertSideInput {
    pub account: AccountRef,
    pub asset: AssetIdent,
    pub amount: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TradeSideInput {
    pub asset: AssetIdent,
    pub qty: Amount,
}

/// What the form sends for any kind of transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
        /// «Arrived in parts» (spec S1): empty = a single-store income.
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ReceivedPartInput>>", optional))]
        parts: Vec<ReceivedPartInput>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AdjustmentLineInput {
    pub asset: AssetIdent,
    pub actual: Amount,
}

/// What the correction screen opens with: every asset of the account and
/// its ledger balance at `at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    pub fn common(&self) -> &TxCommonInput {
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

    /// The property the operation belongs to, trimmed; `None` — none.
    pub fn property_id(&self) -> Option<String> {
        text(&self.common().property_id)
    }

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
                parts,
            } => Transaction::Topup(TopupTx {
                parts: parts.iter().map(ReceivedPartInput::to_part).collect(),
                distribution: None,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddLinkedTransactionsInput {
    pub transactions: Vec<TransactionInput>,
    /// Join this chain; `None` starts a new one.
    #[serde(default)]
    pub flow_id: Option<String>,
}

/// Replace a stored transaction with a new version of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateTransactionInput {
    pub id: String,
    pub transaction: TransactionInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<mwt_model::Transaction>,
}

/// A draft to preview; `replaces` is the id of the transaction being edited.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewTransactionInput {
    pub transaction: TransactionInput,
    pub replaces: Option<String>,
}

/// One month of a period change: what the operation counts for in it before and after, in the
/// budget currency (spec I7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MonthEffect {
    /// Month key `YYYY-MM`.
    pub month: String,
    pub before: Amount,
    pub after: Amount,
}

/// What a new period or spread would do to the months an operation counts in (spec I7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PeriodEffect {
    /// Oldest first; months the change leaves at zero on both sides are left out.
    pub months: Vec<MonthEffect>,
}

// -- income distribution (spec op-period I8-I12) ----------------------------

/// The income a distribution call is about: a draft not saved yet, or a stored one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum IncomeSource {
    /// A draft of an income (a `topup`); saved together with its distribution.
    Draft { transaction: Box<TransactionInput> },
    /// A stored income.
    Stored { id: String },
}

/// `preview_distribution`: the rows against the income (spec I8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewDistributionInput {
    pub income: IncomeSource,
    pub rows: Vec<DistributionRow>,
    /// Least balance left on the card after the rows (spec D10).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub keep_on_card_min: Option<Amount>,
}

/// `distribute_income`: save the income (a draft) or take a stored one, and distribute it (spec I9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DistributeIncomeInput {
    pub income: IncomeSource,
    pub rows: Vec<DistributionRow>,
}

/// `update_distribution`: «Изменить раскладку» of a stored income (spec I10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateDistributionInput {
    pub income_id: String,
    pub rows: Vec<DistributionRow>,
}

/// `confirm_distribution`: the answer to an automation run that asks (spec I12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConfirmDistributionInput {
    pub run_id: String,
    pub amount: Amount,
    /// How to distribute the confirmed amount; the rule's own way when omitted.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mode: Option<OnOtherAmount>,
}

/// One row with the amount it resolved to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResolvedDistributionRow {
    pub row: DistributionRow,
    pub amount: Amount,
}

/// A row cut by the floor on the card (spec D10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DistributionCut {
    pub row_index: u32,
    pub by: Amount,
}

/// What a distribution would book (spec I8, I11): the rows resolved, nothing written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DistributionPreview {
    /// The payout the rows were resolved against.
    pub amount: Amount,
    /// The next planned moment of a rule's payout (RFC3339); a rule preview only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub planned_at: Option<String>,
    /// The period the template's rule gives the payout; a rule preview only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period: Option<OpPeriod>,
    /// Every row in order, the remainder included; empty on `over` and `ask`.
    pub rows: Vec<ResolvedDistributionRow>,
    /// What the remainder row took.
    pub remainder: Amount,
    /// By how much the fixed and percent rows exceed the payout: nothing would be booked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub over: Option<Amount>,
    /// The rule asks for a confirmation instead of distributing (spec D9).
    pub ask: bool,
    pub cut: Vec<DistributionCut>,
}

impl DistributionPreview {
    /// The rows of `resolved` for a payout of `amount`.
    pub fn of(
        amount: Amount,
        planned_at: Option<String>,
        period: Option<OpPeriod>,
        resolved: ResolvedDistribution,
    ) -> Self {
        Self {
            amount,
            planned_at,
            period,
            rows: resolved
                .rows
                .into_iter()
                .map(|resolved| ResolvedDistributionRow {
                    row: resolved.row,
                    amount: resolved.amount,
                })
                .collect(),
            remainder: resolved.remainder,
            over: resolved.over,
            ask: resolved.ask,
            cut: resolved
                .cut
                .into_iter()
                .map(|(index, by)| DistributionCut {
                    row_index: u32::try_from(index).unwrap_or(u32::MAX),
                    by,
                })
                .collect(),
        }
    }
}

// -- operation templates (spec op-period I13) -------------------------------------------

/// An operation template as the form edits it; the id, the archive mark and the creation time
/// are the vault's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OperationTemplateInput {
    pub name: String,
    pub kind: mwt_model::TemplateKind,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub store_id: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub asset: Option<AssetRef>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub amount: Option<Amount>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub contact_id: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period_rule: Option<mwt_model::PeriodRule>,
    /// Income only.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub distribution: Option<mwt_model::DistributionRule>,
}

impl OperationTemplateInput {
    pub fn to_draft(&self) -> TemplateDraft {
        TemplateDraft {
            name: self.name.clone(),
            kind: self.kind,
            store_id: self.store_id.clone(),
            asset: self.asset.clone(),
            amount: self.amount,
            categories: self.categories.clone(),
            comment: self.comment.clone(),
            contact_id: self.contact_id.clone(),
            period_rule: self.period_rule,
            distribution: self.distribution.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateOperationTemplateInput {
    pub id: String,
    pub template: OperationTemplateInput,
}

/// Quick-add from a template; every field left out comes from the template.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateFromTemplateInput {
    pub template_id: String,
    /// RFC3339; now by default.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub timestamp: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub amount: Option<Amount>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub store_id: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub comment: Option<String>,
    /// «Really none»: the operation takes no period rule.
    #[serde(default)]
    pub no_period_rule: bool,
}

/// The operation a template made. `ask` means the distribution rule asks for a confirmation:
/// the income is saved whole and nothing is distributed; the tree of an income that distributed
/// is `get_income_distribution`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TemplateOperation {
    pub transaction: Transaction,
    pub ask: bool,
}

/// An envelope move a distribution made (spec I9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EnvelopeMoveRef {
    pub envelope_id: String,
    pub move_id: String,
    pub amount: Amount,
}

/// An income distributed with the records it made (spec I9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DistributedIncome {
    pub income: Transaction,
    pub transfers: Vec<Transaction>,
    pub envelope_moves: Vec<EnvelopeMoveRef>,
}

/// One stored part with the record it produced (spec I10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IncomePart {
    pub part: DistributionPart,
    /// The transfer of a store part; `None` when the part has none or the record is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub transfer: Option<Transaction>,
    /// The entry of an envelope or goal part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub envelope_move: Option<EnvelopeMove>,
}

/// The stored distribution of an income, top to bottom (spec I10, S10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IncomeTree {
    pub income: Transaction,
    /// Empty when the income does not distribute.
    pub parts: Vec<IncomePart>,
}

impl IncomeTree {
    /// The marker «разложено на N»: the parts without the remainder (spec S10).
    pub fn distributed_count(&self) -> usize {
        self.parts
            .iter()
            .filter(|entry| !matches!(entry.part.kind, DistKind::Remainder))
            .count()
    }
}

/// Which transaction kinds a wallet of one kind accepts, split the way the
/// form presents them, with the kind to pre-select.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
                .filter(|tx_kind| {
                    !tx_kind.is_verb() && tx_kind.in_form() && tx_kind.typed_by_hand()
                })
                .collect(),
            default: Some(default_kind(kind))
                .filter(|tx_kind| tx_kind.typed_by_hand())
                .unwrap_or(TxKind::Transfer),
        }
    }
}

#[cfg(test)]
mod transaction_kinds_tests {
    use super::*;

    #[test]
    fn no_form_offers_a_trade_or_defi_for_a_new_record() {
        for kind in [
            StoreKind::Cash,
            StoreKind::Bank,
            StoreKind::Exchange,
            StoreKind::Crypto,
            StoreKind::Property,
            StoreKind::Loyalty,
            StoreKind::Liability,
        ] {
            let kinds = TransactionKinds::for_store_kind(kind);
            for offered in kinds
                .verbs
                .iter()
                .chain(&kinds.special)
                .chain([&kinds.default])
            {
                assert!(offered.typed_by_hand(), "{kind:?} offers {offered:?}");
            }
        }
        assert_eq!(
            TransactionKinds::for_store_kind(StoreKind::Exchange).default,
            TxKind::Transfer
        );
    }
}

// -- wallet catalog -------------------------------------------------------------

/// Binding shape of a sub-account type, as the creation form needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BindingShapeName {
    Asset,
    Pair,
    Market,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SubTypeInfo {
    pub sub_type: SubType,
    pub shape: BindingShapeName,
    pub default_purpose: Option<Purpose>,
    /// Carries loan terms (a credit card).
    pub credit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PresetInfo {
    pub preset: mwt_model::AutomationPreset,
    pub tx_kind: TxKind,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WalletKindInfo {
    pub kind: StoreKind,
    pub presets: Vec<PresetInfo>,
    pub max_roots: Option<u32>,
    pub user_creatable: bool,
    /// Hidden while the vault keeps no crypto (`Settings::crypto`).
    pub crypto: bool,
    pub default_purpose: Purpose,
    pub sub_types: Vec<SubTypeInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ExchangeProviderInfo {
    pub provider: ExchangeProvider,
    /// The app can connect to it; others are shown as «soon».
    pub supported: bool,
}

/// What every wallet kind allows, in sidebar order. Static domain data, so the
/// UI never duplicates these rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WalletCatalog {
    pub kinds: Vec<WalletKindInfo>,
    pub providers: Vec<ExchangeProviderInfo>,
    /// A property's presets depend on its type: they replace the kind's list.
    pub property_presets: Vec<PropertyPresetsInfo>,
}

/// Ready-made automations of one property type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PropertyPresetsInfo {
    pub category: mwt_model::PropertyCategory,
    pub presets: Vec<PresetInfo>,
}

fn preset_infos(presets: &[mwt_model::AutomationPreset]) -> Vec<PresetInfo> {
    presets
        .iter()
        .map(|&preset| PresetInfo {
            preset,
            tx_kind: preset.tx_kind(),
            category: preset.category().map(str::to_string),
        })
        .collect()
}

impl WalletCatalog {
    pub fn build() -> Self {
        use mwt_model::BindingShape;
        let kinds = StoreKind::ALL
            .iter()
            .map(|&kind| WalletKindInfo {
                kind,
                presets: preset_infos(kind.automation_presets()),
                max_roots: kind.max_roots().map(|max| max as u32),
                user_creatable: kind.user_creatable(),
                crypto: kind.is_crypto(),
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
        use mwt_model::PropertyCategory;
        let property_presets = [
            PropertyCategory::Flat,
            PropertyCategory::House,
            PropertyCategory::Car,
            PropertyCategory::Land,
            PropertyCategory::Other,
        ]
        .into_iter()
        .map(|category| PropertyPresetsInfo {
            category,
            presets: preset_infos(category.automation_presets()),
        })
        .collect();
        Self {
            kinds,
            providers,
            property_presets,
        }
    }
}

// -- contacts ------------------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateContactInput {
    pub id: String,
    pub contact: ContactDraft,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<ContactDraft>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ContactTurnoverInput {
    pub contact_id: String,
    pub account_id: Option<String>,
}

// -- attachments (Full-New-Attach) ------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ListAttachmentsInput {
    /// One operation or goal; `None` lists every attachment.
    pub owner: Option<EntityRef>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddAttachmentLinkInput {
    pub owner: EntityRef,
    pub url: String,
    /// The caption; empty shows the address.
    #[serde(default)]
    pub title: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddAttachmentFileInput {
    pub owner: EntityRef,
    pub name: String,
    pub mime: Option<String>,
    /// The file's bytes, base64.
    pub data_base64: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttachmentIdInput {
    pub id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RenameAttachmentInput {
    pub id: String,
    pub title: String,
}

// -- categories and rules --------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateCategoryInput {
    /// Wanted key; derived from the name when empty.
    pub key: Option<String>,
    pub name: String,
    pub group: CategoryGroup,
    /// The period an operation of the category gets when it has none (spec D3).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period_rule: Option<mwt_model::PeriodRule>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateCategoryInput {
    pub key: String,
    /// Empty = the built-in label.
    pub name: Option<String>,
    pub group: CategoryGroup,
    /// Mandatory spending flag for `Daily`; `None` keeps it.
    #[serde(default)]
    pub essential: Option<bool>,
    /// Passive income flag for `Income`; `None` keeps it.
    #[serde(default)]
    pub passive: Option<bool>,
    /// The period rule of the category; `None` clears it.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period_rule: Option<mwt_model::PeriodRule>,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<mwt_model::Category>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateRuleInput {
    pub pattern: String,
    pub category: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateRuleInput {
    pub id: String,
    pub pattern: String,
    pub category: String,
    pub active: bool,
}

// -- property value --------------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetManualPriceInput {
    pub asset_key: String,
    pub unit_price: Amount,
    pub currency: String,
}

// -- pricing -----------------------------------------------------------------

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RefreshResult {
    pub prices: usize,
    pub fx: usize,
    /// Transaction sides that were saved without a price and got one now.
    pub restamped: usize,
    pub updated_at: String,
}

/// Loose asset identification: enough to rebuild an [`AssetRef`] cache key
/// without making the frontend construct a full `quote` field.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CachedPrice {
    pub price_usd: Amount,
    pub ts: String,
}

/// Whether the "+ Transaction" key can be registered by the app, and the command
/// a compositor binding runs instead when it cannot (Wayland).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ShortcutSupport {
    pub global_keys: bool,
    /// The running executable with `--quick-add`.
    pub launch_command: String,
}

/// Set up a bonus programme.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetLoyaltyInfoInput {
    pub store_id: String,
    pub info: mwt_model::LoyaltyInfo,
}

/// Create a bonus programme with its points in one action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateLoyaltyProgramInput {
    pub name: String,
    pub color: String,
    /// The shop or bank behind the programme, from the contacts.
    pub contact_id: Option<String>,
    pub info: mwt_model::LoyaltyInfo,
}

/// The odometer of a car, read today.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VehicleMileageInput {
    pub store_id: String,
    pub km: u32,
}

/// A car's service done on `date` (`YYYY-MM-DD`), at `km` if read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VehicleServiceDoneInput {
    pub store_id: String,
    #[cfg_attr(feature = "ts", ts(type = "LocalDate"))]
    pub date: String,
    pub km: Option<u32>,
}

/// Credit (positive) or spend (negative) points of a programme.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AdjustPointsInput {
    pub store_id: String,
    pub delta: Amount,
    pub comment: String,
    /// The card whose purchases earned the points.
    pub source_store_id: Option<String>,
    /// RFC3339; empty means now.
    pub timestamp: Option<String>,
}

/// Set or clear the interest terms of a savings account, deposit or PDS (a Russian long-term savings programme).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetDepositTermsInput {
    pub store_id: String,
    pub terms: Option<mwt_model::DepositTerms>,
}

/// Create the missing Funding / Unified / Earn sub-accounts of a Bybit wallet.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ExchangeCoinRow {
    pub coin: String,
    pub exchange: Amount,
    pub ledger: Amount,
    /// `exchange − ledger`: what imports have not explained yet.
    pub diff: Amount,
}

/// A Bybit account (`FUND`, `UNIFIED`, `EARN`) and the sub-account standing for it.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ExchangeAccountBalance {
    pub account: String,
    /// `None` when the wallet has no sub-account of that kind yet.
    pub store_id: Option<String>,
    pub rows: Vec<ExchangeCoinRow>,
}

/// Make sure the FX cache has a rate of each currency for a day, fetching from
/// the public sources (and earlier days) what it lacks.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EnsureFxInput {
    /// Lowercase codes; USD and unknown codes are ignored.
    pub currencies: Vec<String>,
    /// `YYYY-MM-DD` (UTC); empty or absent means today.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "LocalDate | null"))]
    pub date: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EnsureFxOutput {
    /// Rates fetched just now; a point's `ts` is the day its source had, which
    /// may be before the day asked for.
    pub points: Vec<mwt_model::FxPoint>,
    /// Currencies no source had within the look-back window.
    pub missing: Vec<String>,
}

/// Convert an amount between two currency codes at a moment (plan D20).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConvertAmountInput {
    pub from: String,
    pub to: String,
    pub amount: Amount,
    /// RFC3339; empty means now.
    #[serde(default)]
    pub at: Option<String>,
}

/// `amount` is `None` when a rate of either currency is unknown.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConvertedAmount {
    pub amount: Option<Amount>,
}

// -- planning (plan §7) ---------------------------------------------------------

/// An automation as the form edits it. The template is an ordinary
/// transaction input of one of the four verbs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AutomationInput {
    pub name: String,
    pub active: bool,
    #[serde(default)]
    pub preset: Option<mwt_model::AutomationPreset>,
    pub schedule: Schedule,
    /// RFC3339, the first planned date.
    pub starts_at: String,
    pub template: TransactionInput,
    /// The same amount every time, or one that differs (a salary).
    #[serde(default)]
    pub amount_mode: mwt_model::AmountMode,
    /// Where a planned date on a weekend or a holiday goes.
    #[serde(default)]
    pub workday_shift: mwt_model::WorkdayShift,
    /// The local time it fires, «HH:MM»; `None` — 09:00, as before.
    #[serde(default)]
    pub time: Option<String>,
    /// It asks "yes" before it books the operation.
    #[serde(default)]
    pub asks: bool,
    /// An income only: the month it is for, in months from the one it is
    /// planned in (-1 — the previous, 0 — this one); `None` — not set.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub for_month_offset: Option<i8>,
    /// The operation template whose period and distribution rules a run uses.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub operation_template_id: Option<String>,
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
            property_id: self.template.property_id(),
            time: self.time.clone(),
            asks: self.asks,
            for_month_offset: self.for_month_offset,
            operation_template_id: self.operation_template_id.clone(),
        })
    }
}

/// When a new automation would fire: the form asks for its next dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AutomationDatesInput {
    pub schedule: Schedule,
    /// RFC3339, the first planned date.
    pub starts_at: String,
    #[serde(default)]
    pub workday_shift: mwt_model::WorkdayShift,
    /// «HH:MM»; `None` — 09:00.
    #[serde(default)]
    pub time: Option<String>,
}

impl AutomationDatesInput {
    /// The next three dates, RFC3339.
    pub fn upcoming(&self, now: i64, zone: &str) -> Vec<String> {
        let when = mwt_workflow::automation::When {
            schedule: self.schedule,
            starts_at: &self.starts_at,
            workday_shift: self.workday_shift,
            time: self.time.as_deref(),
        };
        let cal = mwt_base::calendar::Calendar::new(zone);
        mwt_workflow::automation::upcoming(&when, &cal, now, 3)
            .into_iter()
            .map(mwt_base::calendar::iso)
            .collect()
    }
}

/// The real amount of an operation booked approximately.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConfirmEstimatedInput {
    pub tx_id: String,
    /// A decimal string, like every amount the app takes.
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateAutomationInput {
    pub id: String,
    pub automation: AutomationInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<AutomationInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BudgetInput {
    pub active: bool,
    pub target: BudgetTarget,
    pub period: BudgetPeriod,
    pub limit: Amount,
    pub currency: String,
    /// Notify near the limit; on when the caller does not say.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warn: Option<bool>,
}

impl From<BudgetInput> for BudgetDraft {
    fn from(input: BudgetInput) -> Self {
        Self {
            active: input.active,
            target: input.target,
            period: input.period,
            limit: input.limit,
            currency: input.currency,
            warn: input.warn.unwrap_or(true),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateBudgetInput {
    pub id: String,
    pub budget: BudgetInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<BudgetInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// Expected return, percent a year.
    #[serde(default)]
    pub expected_return: Option<Amount>,
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
            expected_return: input.expected_return.filter(|value| !value.is_zero()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateGoalInput {
    pub id: String,
    pub goal: GoalInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<GoalInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReminderInput {
    pub text: String,
    /// RFC3339: the date and the time.
    pub due_at: String,
    /// `None` = once.
    #[serde(default)]
    pub repeat: Option<mwt_model::ReminderRepeat>,
    /// "Pay" in the reminder; `None` — no button.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pay: Option<mwt_model::ReminderPay>,
}

/// Emulate the automations ahead: one of them, or all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SimulateAutomationsInput {
    #[serde(default)]
    pub automation_id: Option<String>,
    /// How many days ahead, 1–366.
    pub days: u32,
}

/// Which kind of record carries the «keep» mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MergeTarget {
    Transaction,
    Reminder,
}

/// «Allow merging»: clear the «keep» mark of one record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AllowMergingInput {
    pub target: MergeTarget,
    pub id: String,
}

/// Put a reminder off until a moment (RFC3339).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SnoozeReminderInput {
    pub id: String,
    pub until: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateReminderInput {
    pub id: String,
    pub reminder: ReminderInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<ReminderInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PriceAlertInput {
    pub asset: AssetIdent,
    pub condition: AlertCondition,
    pub price: Amount,
    pub currency: String,
    pub active: bool,
    /// The exchange whose terminal sets the alert.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub store_id: Option<String>,
    /// Fires on every crossing instead of once.
    #[serde(default)]
    pub repeats: bool,
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
            repeats: input.repeats,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdatePriceAlertInput {
    pub id: String,
    pub alert: PriceAlertInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<PriceAlertInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UtilityInput {
    pub name: String,
    pub active: bool,
    pub kind: mwt_model::UtilityKind,
    pub currency: String,
    /// The property the bill belongs to.
    pub property_id: String,
    pub store_id: Option<String>,
    /// Category key of the payments.
    pub category: String,
}

impl From<UtilityInput> for mwt_household::utilities::UtilityDraft {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateUtilityInput {
    pub id: String,
    pub item: UtilityInput,
    /// The version the form loaded. When set, only the fields the user changed against it are
    /// applied onto the current document (an edit never overwrites a remote change to another
    /// field); `None` keeps the whole-input behaviour.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub base: Option<UtilityInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AutomationRejectedEvent {
    pub run_id: String,
    pub automation_id: String,
    pub name: String,
    pub planned_at: String,
    pub reason: mwt_model::RejectReason,
}

// -- chains --------------------------------------------------------------------

/// Put transactions into one chain: `flow_id`, else the chain one of them is
/// already in, else a new one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LinkTransactionsInput {
    pub ids: Vec<String>,
    #[serde(default)]
    pub flow_id: Option<String>,
}

// -- dashboard -----------------------------------------------------------------

pub use mwt_model::DashPeriod;

pub use mwt_reports::comparison::{BaseKind, Comparison};

#[cfg(test)]
mod wire_tests {
    use super::*;

    /// Money and calendar dates cross the wire as JSON strings, never numbers.
    #[test]
    fn wire_dates_and_amounts_are_strings() {
        let points = AdjustPointsInput {
            store_id: "s".into(),
            delta: Amount::parse("-12.50").unwrap(),
            comment: String::new(),
            source_store_id: None,
            timestamp: None,
        };
        let json = serde_json::to_value(&points).unwrap();
        assert_eq!(json["delta"], serde_json::json!("-12.50"));

        let run = mwt_model::WorkflowRun {
            id: "run".into(),
            workflow_id: "flow".into(),
            trigger: String::new(),
            started_at: String::new(),
            outcome: mwt_model::RunOutcome::Done,
            created: Vec::new(),
            queued: 0,
            trace: Vec::new(),
            dry_run: false,
            planned_date: mwt_base::time::LocalDate::from_ymd_opt(2026, 3, 15),
        };
        let serialized = serde_json::to_value(&run).unwrap();
        assert_eq!(serialized["planned_date"], serde_json::json!("2026-03-15"));
    }
}

// -- calc --------------------------------------------------------------------

pub use mwt_base::calc::{CalcInput, CalcResult};

// -- settings ----------------------------------------------------------------

/// Partial update of the **vault-scoped** settings.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateSettingsInput {
    pub display_decimals: Option<u32>,
    pub dashboard_widgets: Option<Vec<mwt_model::DashboardWidgetCfg>>,
    pub fx_refresh_interval_sec: Option<u32>,
    pub minimize_to_tray: Option<bool>,
    /// Share of income planned for savings, 0..1.
    pub savings_rate: Option<Amount>,
    pub daily_budget_mode: Option<DailyBudgetMode>,
    /// Empty string = follow the base currency.
    pub budget_currency: Option<String>,
    /// What to be told about and how far ahead.
    #[serde(default)]
    pub notifications: Option<mwt_model::NotificationSettings>,
    /// Replaces the list of checklist steps marked "Not needed".
    #[serde(default)]
    pub onboarding_skipped: Option<Vec<String>>,
    /// Usual monthly income while there is no history; zero clears it.
    #[serde(default)]
    pub manual_income: Option<Amount>,
    /// Usual monthly spending while there is no history; zero clears it.
    #[serde(default)]
    pub manual_expenses: Option<Amount>,
    #[serde(default)]
    pub dashboard_period: Option<mwt_model::DashPeriod>,
    #[serde(default)]
    pub onboarding_checklist_hidden: Option<bool>,
    /// Replaces the flat inflation table of the group, percent by currency
    /// code. Currencies whose value did not change are not stamped.
    #[serde(default)]
    pub inflation_manual: Option<std::collections::BTreeMap<String, Amount>>,
}

pub use mwt_model::{FxPoint, PricePoint};

// -- tx inspection -----------------------------------------------------------

pub use mwt_money::txinfo::TxInfo;

/// Chain the user picked when the hash alone was ambiguous. Without it the
/// backend never guesses an EVM network.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct TxFieldHint {
    pub field: String,
    pub value: String,
}

/// Result of a best-effort lookup. `fetched` tells the UI whether to say
/// anything at all; a `false` with no `notice` means "stay silent".
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

use mwt_net::{Category, TradePrefill};

/// A fill as offered in the "recent trades" dropdown.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct BybitExecutionsResult {
    pub executions: Vec<BybitExecution>,
    pub notice: Option<String>,
}

impl From<mwt_net::Execution> for BybitExecution {
    fn from(source: mwt_net::Execution) -> Self {
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

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct BybitExecutionsInput {
    pub category: Category,
    pub symbol: Option<String>,
    pub limit: Option<u32>,
    pub store_id: Option<String>,
}

/// Whether a key is stored. The secret is never part of this response.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ExchangeKeyStatusInput {
    pub store_id: String,
}

/// Stores a read-only key for a specific store. Both fields are written to the
/// encrypted vault and are never echoed back to the frontend.
#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct SetExchangeKeyInput {
    pub store_id: String,
    pub api_key: String,
    pub api_secret: String,
}

impl fmt::Debug for SetExchangeKeyInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("SetExchangeKeyInput")
            .field("store_id", &self.store_id)
            .field("api_key", &Hidden)
            .field("api_secret", &Hidden)
            .finish()
    }
}

/// Which sub-accounts imported spot and futures fills land on.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct SetExchangeTargetsInput {
    pub store_id: String,
    pub spot_store_id: Option<String>,
    pub futures_store_id: Option<String>,
}

/// Input to clear a key for a specific store.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ClearExchangeKeyInput {
    pub store_id: String,
}

// -- bybit import (D2.4) -----------------------------------------------------

/// Fetch every Bybit fill for a store and stage it for user confirmation.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ImportBybitHistoryInput {
    pub store_id: String,
    /// Safety cap on fetched fills (default 1000, max 10000). The import always
    /// sweeps every category the key has access to.
    pub limit: Option<u32>,
}

/// How many fills this run newly staged (already-known ids are skipped).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ImportBybitHistoryOutput {
    pub staged: u32,
    /// Machine code: `no_key` | `unreachable` | `rate_limited` |
    /// `forbidden_region`.
    pub notice: Option<String>,
    /// The sync as kept in the history; `None` without a key.
    #[serde(default)]
    pub run: Option<mwt_model::ImportRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetImportSettingsInput {
    pub store_id: String,
    pub settings: mwt_model::ImportSettings,
}

/// What a sync would bring; `notice` (`no_key`, `unreachable`, …) when the
/// exchange could not be asked.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImportPreviewOutput {
    pub preview: Option<mwt_cryptotrack::exchange_sync::ImportPreview>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ListImportedTransactionsInput {
    pub store_id: String,
    /// Default 50, at most 500.
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ListPendingImportsOutput {
    pub imports: Vec<mwt_model::PendingImport>,
}

/// Turn staged fills into real trades (two legs each, like `add_trade`).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ConfirmPendingImportsInput {
    /// Empty = confirm everything currently staged.
    pub exec_ids: Vec<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ConfirmPendingImportsOutput {
    pub confirmed: u32,
}

/// Drop staged fills without creating transactions.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct DiscardPendingImportsInput {
    /// Empty = discard everything currently staged.
    pub exec_ids: Vec<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct DiscardPendingImportsOutput {
    pub discarded: u32,
}

/// A recently opened vault file.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// The vault has a password (read from the file's header on each listing).
    #[serde(default = "yes")]
    pub encrypted: bool,
    /// The file is in the app's own folder (`vaults_dir`): the history offers
    /// "delete the vault" there instead of just forgetting it.
    #[serde(default)]
    pub in_app_folder: bool,
}

/// Input to select or pick a vault file.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct SelectVaultInput {
    pub path: String,
}

/// Input to put a forgotten vault back into recents.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct RestoreRecentVaultInput {
    pub path: String,
    /// Position the row had before it was forgotten.
    pub index: u32,
}

/// Input to point a recents row at the file found in a new place.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct RelocateRecentVaultInput {
    /// The path the row has now (the file is not there any more).
    pub path: String,
    /// Where the file was found; the row keeps its place in the list.
    pub new_path: String,
}

/// Input to delete a vault from the app's folder.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct DeleteVaultFileInput {
    pub path: String,
}

/// What `delete_vault_file` removed.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct DeleteVaultFileOutput {
    /// Name of the deleted vault file.
    pub file_name: String,
    /// Names of the backup copies deleted with it.
    pub backups: Vec<String>,
}

/// Input to save a copy of a vault file elsewhere.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ExportVaultCopyInput {
    /// The vault to copy.
    pub path: String,
}

/// A vault file chosen in the system dialog.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct PickedVault {
    /// Local path to open or create (a copy in app storage on mobile).
    pub path: String,
    pub display_name: String,
    /// The picked document was copied into app storage.
    pub imported: bool,
}

/// Keep a copy of the unlocked vault in an external document.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct LinkVaultInput {
    /// Pick an existing file (it gets replaced by the vault) instead of
    /// creating a new one.
    pub existing: bool,
}

/// The external copy of the unlocked vault, if any.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct VaultLinkStatus {
    pub linked: bool,
    pub display_name: Option<String>,
    /// RFC 3339 time of the last successful copy.
    pub last_synced_at: Option<String>,
    /// Why the last copy failed; cleared by the next successful one.
    pub last_error: Option<String>,
}

/// A backup copy of the vault file, next to it.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct VaultBackup {
    /// File name inside the vault's folder (`vault.wallet.v2.bak`).
    pub name: String,
    /// RFC 3339 time the copy was written.
    pub modified_at: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size_bytes: u64,
}

impl From<mwt_vault::vault_io::backups::BackupFile> for VaultBackup {
    fn from(file: mwt_vault::vault_io::backups::BackupFile) -> Self {
        Self {
            name: file.name,
            modified_at: file.modified_at,
            size_bytes: file.size_bytes,
        }
    }
}

/// Which backup `restore_vault_backup` puts back.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct RestoreVaultBackupInput {
    /// A name from `list_vault_backups`; anything else is refused.
    pub name: String,
}

/// Which copy wins after `SYNC_CONFLICT`.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ConflictSide {
    #[default]
    Local,
    Remote,
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct ResolveVaultConflictInput {
    pub path: String,
    pub keep: ConflictSide,
    /// The vault's password: keeping the remote copy trial-opens it first.
    #[serde(default)]
    pub password: String,
}

impl fmt::Debug for ResolveVaultConflictInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ResolveVaultConflictInput")
            .field("path", &self.path)
            .field("keep", &self.keep)
            .field("password", &Hidden)
            .finish()
    }
}

/// Output of the dev dump vault command (DEV only).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct DevDumpVaultOutput {
    pub json: String,
}

// -- market data (candles) --------------------------------------------------

/// Single OHLCV candle point for charting.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct CandlePoint {
    pub ts: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct GetCandlesInput {
    pub symbol: String,
    pub interval_min: u32,
    pub limit: u32,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct GetCandlesOutput {
    pub candles: Vec<CandlePoint>,
}

// -- node workflows (plan §16) -------------------------------------------------

/// Create (`id` empty) or update a workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SaveWorkflowInput {
    #[serde(default)]
    pub id: Option<String>,
    pub workflow: mwt_workflow::service::records::WorkflowDraft,
}

/// A workflow from a gallery template: the form's fields by key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WorkflowFromTemplateInput {
    pub template_id: String,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
}

/// A workflow file's text and the wallet swaps (file id to this vault's id).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImportWorkflowInput {
    pub json: String,
    #[serde(default)]
    pub stores: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetWorkflowActiveInput {
    pub id: String,
    pub active: bool,
}

/// The "On" switch of a "Mine" row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetMyAutomationActiveInput {
    pub kind: mwt_planning::my_automations::MineKind,
    pub id: String,
    pub active: bool,
}

/// Emulate a graph — saved or still in the editor — on a copy of the vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DryRunWorkflowInput {
    /// The graph as JSON.
    pub graph: String,
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// The trigger to start from; a manual one, else the first, by default.
    #[serde(default)]
    pub trigger_node: Option<String>,
    /// Answers to "Wait for confirmation" given so far, by node id; a question
    /// without one pauses the emulation (outcome `asking`).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub answers: Option<std::collections::BTreeMap<String, bool>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SaveWorkflowDraftInput {
    pub id: String,
    /// The editor's graph as JSON.
    pub graph: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AnswerWorkflowWaitInput {
    pub id: String,
    /// "Yes" (true) or "No" (false).
    pub yes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RunWorkflowInput {
    pub id: String,
    #[serde(default)]
    pub trigger_node: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ListWorkflowRunsInput {
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// Newest first, 100 by default.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RestoreWorkflowVersionInput {
    pub id: String,
    pub saved_at: String,
}

/// Set a variable by hand; `value` is a `WfValue` in JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetWorkflowVarInput {
    pub scope: mwt_workflow::VarScope,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeleteWorkflowVarInput {
    pub scope: mwt_workflow::VarScope,
    pub name: String,
}

/// A graph and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ValidateWorkflowInput {
    pub graph: String,
}

/// Add a test to a workflow (empty `test.id`) or replace one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SaveWorkflowTestInput {
    pub workflow_id: String,
    pub test: mwt_workflow::WorkflowTest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeleteWorkflowTestInput {
    pub workflow_id: String,
    pub test_id: String,
}

// -- device sync (docs/sync.md) ---------------------------------------------------

/// This device's place in the sync group.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncProgress {
    pub received: u32,
    pub total: u32,
}

/// One connection to another device.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncSessionView {
    pub session_id: String,
    pub device_id: String,
    pub name: String,
    pub phase: SyncPhase,
    /// Its clock jumped this session: every batch from it waits for confirmation.
    pub unreliable: bool,
    /// Its clock minus ours, ms (shown when beyond 5 minutes).
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub clock_offset_ms: i64,
    pub progress: Option<SyncProgress>,
    /// The other device's window has focus.
    pub focused: bool,
    /// While pairing: the code both screens show.
    pub pairing_code: Option<String>,
    /// While pairing: when the code stops being valid (Unix ms), for the countdown.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub pairing_expires_ms: Option<u64>,
    /// The list of changes waiting for the user, if any.
    pub plan: Option<SyncPendingPlan>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncPendingPlan {
    pub reason: SyncConfirmReason,
    pub summary: mwt_sync::SyncSummary,
    /// Capital and balances once applied; `None` when it could not be worked out.
    #[serde(default)]
    pub after: Option<AfterApply>,
}

/// What the vault would show after a batch is applied, worked out on an
/// in-memory copy (nothing is written).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AfterApply {
    /// The base currency of both net worths and every store balance.
    pub currency: String,
    pub net_worth_before: Amount,
    pub net_worth_after: Amount,
    /// Stores whose balance the batch moves (or that it adds or removes).
    pub stores: Vec<StoreBalanceChange>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreBalanceChange {
    pub id: String,
    pub name: String,
    pub before: Amount,
    pub after: Amount,
    pub currency: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// A backup from before a sync exists ("Restore from before the sync").
    pub has_backup: bool,
    /// Sync goes on in the background here (Android: a foreground service).
    pub background_supported: bool,
    /// Minutes in the background before an Android device locks (0 = never).
    pub background_lock_min: u32,
    /// What the network looks like from here (the diagnostics line).
    pub network: SyncNetworkView,
    /// Android 13+: the notification background sync needs is not allowed, so
    /// sync pauses in the background. Always false elsewhere.
    pub notifications_blocked: bool,
}

/// How a firewall that drops incoming sync connections can be opened here.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncNetworkView {
    /// The TCP port this device listens on while sync runs.
    pub port: Option<u16>,
    /// This device's address on the local network.
    pub address: Option<String>,
    /// Devices with this vault seen nearby right now.
    pub seen: u32,
    /// When this device started looking (ms since the epoch).
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub since_ms: Option<i64>,
    /// Another device could not connect here: a firewall drops incoming connections.
    pub inbound_blocked: bool,
    /// Windows: the rule was added but the network is Public, so the
    /// Private-only rule does not apply (set by `sync_open_firewall`).
    pub network_public: bool,
    pub firewall_fix: SyncFirewallFix,
    /// The commands that open the port by hand.
    pub firewall_commands: Vec<String>,
    /// The code of the last sync error (`SYNC_*`), cleared by a session that goes live.
    pub last_error: Option<String>,
}

/// Where handing a vault to a new device stands (either side).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    #[default]
    Idle,
    /// Receiving: visible nearby, no request yet. Sending: waiting for the
    /// other device to accept.
    Waiting,
    /// Receiving: a device asks to hand its vault over ("Accept / Decline").
    Request,
    /// Receiving: showing the code. Sending: asking for it.
    Code,
    Transferring,
    Done,
    Failed,
}

/// The receiving side (`transfer-receive` event, `transfer_receive_*`).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TransferReceiveView {
    pub phase: TransferPhase,
    /// The name other devices see this one by.
    pub device_name: String,
    /// When this device stops being visible (ms since the epoch).
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub expires_ms: Option<i64>,
    pub sender_name: Option<String>,
    /// The six digits to type on the sending device.
    pub code: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub done_bytes: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_bytes: u64,
    /// Where the received vault was saved (to open it).
    pub saved_path: Option<String>,
    /// `SYNC_*` code of what went wrong.
    pub error: Option<String>,
}

/// The sending side (`transfer-send` event, `transfer_send*`).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TransferSendView {
    pub phase: TransferPhase,
    /// The receiving device.
    pub device_name: String,
    /// Tries left for the code.
    pub attempts_left: u32,
    /// The last code typed was wrong.
    pub wrong_code: bool,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub done_bytes: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_bytes: u64,
    pub error: Option<String>,
}

/// A device nearby waiting to receive a vault (`transfer-nearby` event).
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TransferNearby {
    pub id: String,
    pub name: String,
    /// `mobile` or `desktop`.
    pub kind: String,
}

/// "Send": to whom, and the vault password typed again.
#[derive(Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TransferSendInput {
    pub id: String,
    pub password: String,
}

impl fmt::Debug for TransferSendInput {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("TransferSendInput")
            .field("id", &self.id)
            .field("password", &Hidden)
            .finish()
    }
}

pub use mwt_model::SyncCutReason;

/// A trusted device as the settings list shows it.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncPeerView {
    pub device_id: String,
    pub name: String,
    pub paired_at: String,
    pub last_synced_at: Option<String>,
    pub online: bool,
    /// The user declined its list of changes: no exchange until "Resume".
    #[serde(default)]
    pub paused: bool,
    /// The device refused this one on purpose and told so.
    #[serde(default)]
    pub cut_by: Option<SyncCutReason>,
    /// The user forgot this device here: no exchange until "Pair again".
    #[serde(default)]
    pub forgotten: bool,
}

/// Partial update of this device's sync settings.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncOfferEvent {
    pub session_id: String,
    pub peer_name: String,
}

/// `sync-pairing`: show the code; both screens must show the same one.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncPairingEvent {
    pub session_id: String,
    pub peer_name: String,
    pub code: String,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SyncConfirmReason {
    /// "Ask before syncing" is on: the first batch of a session.
    #[default]
    First,
    /// Just paired: the first sync with this device.
    Paired,
    /// The device's clock jumped this session.
    Unreliable,
}

/// `sync-plan-ready`: what a batch would change, waiting for the user.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncPlanEvent {
    pub session_id: String,
    pub peer_name: String,
    pub reason: SyncConfirmReason,
    pub summary: mwt_sync::SyncSummary,
    #[serde(default)]
    pub after: Option<AfterApply>,
}

/// `vault-synced`: a batch was applied here.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VaultSyncedEvent {
    pub session_id: String,
    pub batch: mwt_sync::history::AppliedBatch,
    /// Sorted distinct collections the batch touched, the dedupe and repair
    /// deletions included (`batch.summary` does not count those and keeps its
    /// `total`). The screens of all of them re-read.
    #[serde(default)]
    pub collections: Vec<String>,
}

/// `sync-error`: a session ended or refused to start; `code` is a `SYNC_*` code.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncErrorEvent {
    pub code: String,
    pub message: String,
    pub peer_name: Option<String>,
    /// `SYNC_VERSION_MISMATCH` only: the peer's app version (empty: an old
    /// build that does not send it).
    #[serde(default)]
    pub peer_version: Option<String>,
    /// `SYNC_VERSION_MISMATCH` only: this app's version.
    #[serde(default)]
    pub own_version: Option<String>,
}

/// `sync-clock`: the other device's clock differs or jumped.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncClockEvent {
    pub session_id: String,
    pub peer_name: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub offset_ms: i64,
    pub unreliable: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SyncDecisionInput {
    pub session_id: String,
    pub accept: bool,
}

// -- shared events (shared sit-down events) ----------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateSharedEventInput {
    pub id: String,
    pub event: SharedEventDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedEventFilter {
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedParticipantInput {
    pub event_id: String,
    pub participant: SharedParticipantDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateSharedParticipantInput {
    pub event_id: String,
    pub participant_id: String,
    pub participant: SharedParticipantDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedParticipantRef {
    pub event_id: String,
    pub participant_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedItemInput {
    pub event_id: String,
    pub item: SharedItemDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateSharedItemInput {
    pub event_id: String,
    pub item_id: String,
    pub item: SharedItemDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedItemRef {
    pub event_id: String,
    pub item_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddSharedPurchaseInput {
    pub event_id: String,
    pub purchase: SharedPurchaseDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateSharedPurchaseInput {
    pub event_id: String,
    pub purchase_id: String,
    pub purchase: SharedPurchaseDraft,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedPurchaseRef {
    pub event_id: String,
    pub purchase_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedSettlementRef {
    pub event_id: String,
    pub settlement_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArchiveSharedEventInput {
    pub id: String,
    pub archived: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedShareTextInput {
    pub event_id: String,
    /// `ru` or `en`; empty: `ru`.
    #[serde(default)]
    pub locale: String,
}

// -- updates -----------------------------------------------------------------

/// Where the update check stands (event `update-state` carries the whole
/// [`UpdateStatusDto`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    Checking,
    UpToDate,
    /// A self-updating install is fetching the package in the background.
    Downloading {
        percent: u8,
    },
    /// Downloaded and verified; applied on exit or by `update_restart_now`.
    Ready {
        version: String,
    },
    /// A newer version exists but this install updates by hand.
    Available {
        offer: mwt_net::UpdateOffer,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateStatusDto {
    pub phase: UpdatePhase,
    pub current_version: String,
    pub install_kind: mwt_net::InstallKind,
    pub channel: mwt_net::Channel,
    pub auto_update: bool,
    /// RFC 3339 time of the last finished check.
    pub last_check: Option<String>,
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    const LEAK: &str = "fake-leak-7Hq2";

    fn leaks(debug: String) -> bool {
        debug.contains(LEAK)
    }

    #[test]
    fn create_vault_input_debug_hides_the_password() {
        let input = CreateVaultInput {
            path: "/v".into(),
            password: Some(LEAK.into()),
            ..Default::default()
        };
        assert!(!leaks(format!("{input:?}")));
        assert!(format!("{input:?}").contains("/v"));
    }

    #[test]
    fn unlock_input_debug_hides_the_password() {
        let input = UnlockInput {
            path: "/v".into(),
            password: LEAK.into(),
        };
        assert!(!leaks(format!("{input:?}")));
    }

    #[test]
    fn change_password_input_debug_hides_both_passwords() {
        let input = ChangePasswordInput {
            current_password: LEAK.into(),
            new_password: LEAK.into(),
        };
        assert!(!leaks(format!("{input:?}")));
    }

    #[test]
    fn set_vault_password_input_debug_hides_the_password() {
        let input = SetVaultPasswordInput {
            new_password: LEAK.into(),
            delete_plain_copies: true,
        };
        assert!(!leaks(format!("{input:?}")));
    }

    #[test]
    fn remove_vault_password_input_debug_hides_the_password() {
        let input = RemoveVaultPasswordInput {
            current_password: LEAK.into(),
            external_copy_too: false,
        };
        assert!(!leaks(format!("{input:?}")));
    }

    #[test]
    fn set_exchange_key_input_debug_hides_key_and_secret() {
        let input = SetExchangeKeyInput {
            store_id: "s1".into(),
            api_key: LEAK.into(),
            api_secret: LEAK.into(),
        };
        let shown = format!("{input:?}");
        assert!(!leaks(shown.clone()));
        assert!(shown.contains("s1"));
    }

    #[test]
    fn resolve_vault_conflict_input_debug_hides_the_password() {
        let input = ResolveVaultConflictInput {
            path: "/v".into(),
            keep: ConflictSide::Local,
            password: LEAK.into(),
        };
        assert!(!leaks(format!("{input:?}")));
    }

    #[test]
    fn transfer_send_input_debug_hides_the_password() {
        let input = TransferSendInput {
            id: "t1".into(),
            password: LEAK.into(),
        };
        assert!(!leaks(format!("{input:?}")));
    }
}

#[cfg(test)]
mod period_input_tests {
    use super::*;
    use mwt_model::{MonthShare, PeriodKind, SpreadMode};

    fn topup(parts: Vec<ReceivedPartInput>) -> TransactionInput {
        TransactionInput::Topup {
            common: TxCommonInput::default(),
            store_id: "card".into(),
            asset: AssetIdent {
                symbol: "USD".into(),
                ..AssetIdent::default()
            },
            amount: Amount::parse("100").unwrap(),
            fee: None,
            from: None,
            details: KindDetails::default(),
            parts,
        }
    }

    #[test]
    fn an_input_without_the_new_fields_reads_as_before() {
        let mut json = serde_json::to_value(topup(Vec::new())).unwrap();
        let object = json.as_object_mut().unwrap();
        object.remove("parts");
        let common = object.get_mut("common").unwrap().as_object_mut().unwrap();
        for key in ["period", "spread", "no_period_rule"] {
            common.remove(key);
        }
        let input: TransactionInput = serde_json::from_value(json).unwrap();
        let tx = input.to_transaction();
        assert_eq!(tx.meta().period, None);
        assert_eq!(tx.meta().spread, None);
        assert!(!input.common().no_period_rule);
        let Transaction::Topup(topup) = tx else {
            panic!("a top-up")
        };
        assert!(topup.parts.is_empty());
    }

    #[test]
    fn period_spread_and_parts_reach_the_transaction() {
        let part = ReceivedPartInput {
            store_id: "card".into(),
            amount: Amount::parse("60").unwrap(),
            timestamp: Some(String::new()),
        };
        let mut input = topup(vec![part]);
        input.common_mut().period = Some(OpPeriod {
            kind: PeriodKind::Quarter,
            start: "2026-10-01".into(),
            end: "2026-12-31".into(),
        });
        input.common_mut().spread = Some(Spread {
            mode: SpreadMode::Even,
            shares: vec![MonthShare {
                month: "2026-10".into(),
                amount: Amount::parse("33.33").unwrap(),
            }],
        });
        let tx = input.to_transaction();
        assert_eq!(tx.meta().period, input.common().period);
        assert_eq!(tx.meta().spread, input.common().spread);
        let Transaction::Topup(topup) = tx else {
            panic!("a top-up")
        };
        assert_eq!(topup.parts.len(), 1);
        assert_eq!(topup.parts[0].timestamp, None);
    }
}

#[cfg(test)]
mod distribution_dto_tests {
    use super::*;
    use mwt_model::PartStatus;
    use mwt_planning::distribution::ResolvedRow;

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    #[test]
    fn a_resolved_rule_becomes_the_preview_rows() {
        let row = DistributionRow {
            kind: DistKind::Remainder,
            target: mwt_model::DistTarget::None,
        };
        let resolved = ResolvedDistribution {
            rows: vec![ResolvedRow {
                row: row.clone(),
                amount: amount("70"),
            }],
            remainder: amount("70"),
            cut: vec![(0, amount("5"))],
            ..ResolvedDistribution::default()
        };
        let preview = DistributionPreview::of(amount("100"), None, None, resolved);
        assert_eq!(preview.rows[0].row, row);
        assert_eq!(preview.rows[0].amount, amount("70"));
        assert_eq!(preview.cut[0].row_index, 0);
        assert_eq!(preview.cut[0].by, amount("5"));
        assert!(!preview.ask);
    }

    #[test]
    fn the_marker_counts_the_parts_without_the_remainder() {
        let part = |kind| IncomePart {
            part: DistributionPart {
                kind,
                target: mwt_model::DistTarget::None,
                amount: amount("1"),
                status: PartStatus::Done,
                effect: None,
            },
            transfer: None,
            envelope_move: None,
        };
        let income = TransactionInput::Topup {
            common: TxCommonInput::default(),
            store_id: "card".into(),
            asset: AssetIdent::default(),
            amount: amount("3"),
            fee: None,
            from: None,
            details: KindDetails::default(),
            parts: Vec::new(),
        }
        .to_transaction();
        let tree = IncomeTree {
            income,
            parts: vec![
                part(DistKind::Sum {
                    amount: amount("1"),
                }),
                part(DistKind::Percent { pct: amount("5") }),
                part(DistKind::Remainder),
            ],
        };
        assert_eq!(tree.distributed_count(), 2);
    }
}
