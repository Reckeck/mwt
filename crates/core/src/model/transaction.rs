//! Transactions: what the user did with money, and the bookkeeping legs it
//! implies.
//!
//! Persisted in the vault, which release builds write with bincode. bincode is
//! not self-describing, so every enum here is externally tagged and nothing
//! uses `#[serde(tag)]`, `untagged`, `flatten` or `serde_json::Value`. Adding
//! or removing a field breaks existing release files — the schema is frozen
//! deliberately (plan D11).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::vault::now_iso;

/// Price stamps a money movement can carry.
///
/// Pricing is a property of an asset at a moment, not of the kind of
/// movement, so every priced side of a transaction carries one of these.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PriceStamp {
    /// Price of one unit of the asset in USD.
    #[serde(default)]
    pub price_usd: Option<Amount>,
    /// Price of one unit of the asset in the base currency, when already known.
    #[serde(default)]
    pub price_base: Option<Amount>,
    /// Manual USD→base rate the user entered at that moment.
    #[serde(default)]
    pub fx_override: Option<Amount>,
}

impl PriceStamp {
    /// Copy these stamps onto a leg.
    pub fn on(&self, leg: &mut Leg) {
        leg.price_usd = self.price_usd;
        leg.price_base = self.price_base;
        leg.fx_override = self.fx_override;
    }

    /// Fill in what is missing, never overwriting a price already recorded.
    pub fn fill(&mut self, price_usd: Option<Amount>, fx_override: Option<Amount>) {
        self.price_usd = self.price_usd.or(price_usd);
        if fx_override.is_some() {
            self.fx_override = fx_override;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.price_usd.is_none() && self.price_base.is_none() && self.fx_override.is_none()
    }
}

/// How a transaction came to exist.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TxOrigin {
    /// Entered by the user.
    #[default]
    Manual,
    /// Confirmed from an exchange import; `exec_id` is the exchange's own fill id.
    Import { exec_id: String },
    /// Created by an automation for one planned date.
    Automation {
        automation_id: String,
        planned_at: String,
    },
}

/// Fields every transaction carries, whatever it is.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TxMeta {
    pub id: String,
    /// RFC3339 UTC, lexicographically sortable — a real sort key, not a label.
    pub timestamp: String,
    /// Free-form user note.
    pub comment: String,
    /// Category keys, all members of `VaultDoc::categories`.
    pub categories: Vec<String>,
    /// Groups the operations of one movement of money (RUB → USDT → THB).
    pub flow_id: Option<String>,
    /// Who the operation was with or through (an exchanger, a shop, a person).
    pub contact_id: Option<String>,
    pub origin: TxOrigin,
}

impl TxMeta {
    pub fn new(comment: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_iso(),
            comment: comment.trim().to_string(),
            categories: Vec::new(),
            flow_id: None,
            contact_id: None,
            origin: TxOrigin::Manual,
        }
    }
}

/// A fee paid in some asset. It is booked as its own outgoing leg, so holdings
/// shrink by it — before this model fees were recorded and then ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Fee {
    pub asset: AssetRef,
    pub amount: Amount,
    #[serde(default)]
    pub price: PriceStamp,
}

/// One side of a movement: one of the user's own stores, a saved contact (and
/// optionally one of its accounts), or free text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountRef {
    Store(String),
    Contact {
        contact_id: String,
        account_id: Option<String>,
    },
    Raw(String),
}

impl AccountRef {
    pub fn store_id(&self) -> Option<&str> {
        match self {
            Self::Store(id) => Some(id),
            _ => None,
        }
    }

    pub fn contact_id(&self) -> Option<&str> {
        match self {
            Self::Contact { contact_id, .. } => Some(contact_id),
            _ => None,
        }
    }
}

/// What a leg represents inside its transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LegRole {
    /// The movement itself.
    Principal,
    /// A fee; never a sale, never part of realized PnL as a trade.
    Fee,
    /// Realized profit or loss settled by the exchange (futures close).
    Pnl,
}

/// One bookkeeping effect on one `(store, asset)` pair.
///
/// Everything that sums money — holdings, history, PnL — consumes legs and
/// never inspects the variant. Derived, never persisted.
#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    pub store_id: String,
    pub asset: AssetRef,
    /// Signed: positive adds to `store_id`, negative subtracts.
    pub amount: Amount,
    pub role: LegRole,
    /// Counterparty store of a store-to-store movement, set on both legs so a
    /// store finds every transaction touching it from either side.
    pub to_store_id: Option<String>,
    pub price_usd: Option<Amount>,
    pub price_base: Option<Amount>,
    pub fx_override: Option<Amount>,
}

impl Leg {
    fn new(
        store_id: &str,
        asset: &AssetRef,
        amount: Amount,
        role: LegRole,
        price: &PriceStamp,
    ) -> Self {
        let mut leg = Self {
            store_id: store_id.to_string(),
            asset: asset.clone(),
            amount,
            role,
            to_store_id: None,
            price_usd: None,
            price_base: None,
            fx_override: None,
        };
        price.on(&mut leg);
        leg
    }

    pub fn out_going(store_id: &str, asset: &AssetRef, amount: Amount, price: &PriceStamp) -> Self {
        Self::new(store_id, asset, -amount.abs(), LegRole::Principal, price)
    }

    pub fn in_coming(store_id: &str, asset: &AssetRef, amount: Amount, price: &PriceStamp) -> Self {
        Self::new(store_id, asset, amount.abs(), LegRole::Principal, price)
    }

    fn fee(store_id: &str, fee: &Fee) -> Self {
        Self::new(
            store_id,
            &fee.asset,
            -fee.amount.abs(),
            LegRole::Fee,
            &fee.price,
        )
    }
}

/// Pushes the fee leg, if there is a fee and a store to book it on.
fn push_fee(legs: &mut Vec<Leg>, store_id: Option<&str>, fee: &Option<Fee>) {
    if let (Some(store_id), Some(fee)) = (store_id, fee)
        && !fee.amount.is_zero()
    {
        legs.push(Leg::fee(store_id, fee));
    }
}

/// One banknote line of a cash count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CashNote {
    pub denomination: Amount,
    pub count: u32,
}

/// Facts that only some store kinds have. Validated against `store.kind`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct KindDetails {
    /// `cash` only; must add up to the amount.
    pub cash_breakdown: Option<Vec<CashNote>>,
    /// `asset` only: price of one unit when acquiring (`amount` is the unit count).
    pub unit_price: Option<Amount>,
    /// Crypto stores only.
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
}

impl KindDetails {
    pub fn is_empty(&self) -> bool {
        self.cash_breakdown.is_none()
            && self.unit_price.is_none()
            && self.network.is_none()
            && self.tx_hash.is_none()
    }
}

// -- the eight variants ------------------------------------------------------

/// «Пополнить»: money comes into a store from outside (salary, cashback, buying an asset).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TopupTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub fee: Option<Fee>,
    /// Who paid; never one of the user's stores (that is a transfer).
    pub from: Option<AccountRef>,
    pub details: KindDetails,
    pub price: PriceStamp,
}

/// «Оплатить»: money leaves a store for good (a purchase, a bill, interest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PaymentTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub fee: Option<Fee>,
    /// Who was paid; never one of the user's stores.
    pub to: Option<AccountRef>,
    pub details: KindDetails,
    pub price: PriceStamp,
}

/// Effect of a transfer on a personal debt with a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DebtEffect {
    /// Lent money (store → contact): they owe me more.
    Lend,
    /// Borrowed money (contact → store): I owe more.
    Borrow,
    /// Paid my debt back (store → contact): I owe less.
    Repay,
    /// Got my money back (contact → store): they owe me less.
    Collect,
}

impl DebtEffect {
    /// Whether the money leaves the user's store.
    pub fn is_outgoing(self) -> bool {
        matches!(self, Self::Lend | Self::Repay)
    }
}

/// «Перевести»: one asset moves between two places, at least one of them mine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TransferTx {
    pub meta: TxMeta,
    pub from: AccountRef,
    pub to: AccountRef,
    pub asset: AssetRef,
    /// What the receiver gets; the fee is paid on top by the sender.
    pub amount: Amount,
    pub fee: Option<Fee>,
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
    /// Set when the transfer opens or settles a personal debt with a contact.
    pub debt: Option<DebtEffect>,
    pub price: PriceStamp,
}

/// One side of a conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ConvertSide {
    pub account: AccountRef,
    pub asset: AssetRef,
    pub amount: Amount,
    #[serde(default)]
    pub price: PriceStamp,
}

/// «Обменять»: give one asset, receive another (bank FX, an exchanger, P2P).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ConvertTx {
    pub meta: TxMeta,
    pub from: ConvertSide,
    pub to: ConvertSide,
    pub fee: Option<Fee>,
    /// Rate the user was actually given, `to` per `from`, when it should be kept.
    pub rate_override: Option<Amount>,
}

/// One side of a spot trade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TradeSide {
    pub asset: AssetRef,
    pub qty: Amount,
    #[serde(default)]
    pub price: PriceStamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Liquidity {
    Maker,
    Taker,
}

/// A spot trade on an exchange.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TradeSpotTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub sold: TradeSide,
    pub bought: TradeSide,
    pub fee: Option<Fee>,
    pub liquidity: Option<Liquidity>,
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FuturesDirection {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FuturesAction {
    Open,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FuturesCategory {
    Linear,
    Inverse,
}

/// Opening or closing (part of) a futures position.
///
/// The position is a holding of `contract` (a derivative, see
/// [`AssetRef::perp`]); it is excluded from portfolio value. Realized PnL is
/// the `closed_pnl` the exchange settled, booked in `settle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TradeFuturesTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub contract: AssetRef,
    /// Asset margin and PnL settle in (USDT for linear, the coin for inverse).
    pub settle: AssetRef,
    pub direction: FuturesDirection,
    pub action: FuturesAction,
    pub quantity: Amount,
    /// Fill price, in `settle` per contract unit.
    pub fill_price: Amount,
    pub leverage: Option<Amount>,
    pub fee: Option<Fee>,
    /// Only on `Close`; signed (a loss is negative).
    pub closed_pnl: Option<Amount>,
    pub order_id: Option<String>,
    pub category: FuturesCategory,
    /// Price of the settle asset, for valuing the PnL leg.
    pub price: PriceStamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DefiAction {
    Stake,
    Unstake,
    Claim,
    AddLiquidity,
    RemoveLiquidity,
}

impl DefiAction {
    /// Whether the asset comes back into the wallet.
    pub fn is_incoming(self) -> bool {
        matches!(self, Self::Unstake | Self::Claim | Self::RemoveLiquidity)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DefiTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub action: DefiAction,
    pub asset: AssetRef,
    pub amount: Amount,
    pub protocol: Option<String>,
    pub pool_id: Option<String>,
    pub network: Option<Chain>,
    pub tx_hash: Option<String>,
    pub fee: Option<Fee>,
    pub price: PriceStamp,
}

/// Selling a non-financial asset (a flat, a car) out of an `asset` store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AssetSellTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub unit_price: Amount,
    pub quantity: Amount,
    /// Currency of `unit_price` and of the proceeds.
    pub currency: AssetRef,
    /// Store that received the money, if it is tracked.
    pub proceeds_to: Option<String>,
    pub fee: Option<Fee>,
    /// Price of the currency, for valuing the proceeds.
    pub price: PriceStamp,
}

/// Kind of a transaction, without its data — what a store accepts, what a form shows.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TxKind {
    Topup,
    Payment,
    Transfer,
    Convert,
    TradeSpot,
    TradeFutures,
    Defi,
    AssetSell,
}

impl TxKind {
    pub const ALL: &'static [TxKind] = &[
        TxKind::Topup,
        TxKind::Payment,
        TxKind::Transfer,
        TxKind::Convert,
        TxKind::TradeSpot,
        TxKind::TradeFutures,
        TxKind::Defi,
        TxKind::AssetSell,
    ];

    /// The four everyday verbs; the rest are specialised.
    pub fn is_verb(self) -> bool {
        matches!(
            self,
            Self::Topup | Self::Payment | Self::Transfer | Self::Convert
        )
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Topup => "topup",
            Self::Payment => "payment",
            Self::Transfer => "transfer",
            Self::Convert => "convert",
            Self::TradeSpot => "trade_spot",
            Self::TradeFutures => "trade_futures",
            Self::Defi => "defi",
            Self::AssetSell => "asset_sell",
        }
    }
}

/// Which kinds a store of this kind accepts. The single source of the binding.
pub fn allowed_kinds(kind: StoreKind) -> &'static [TxKind] {
    use TxKind::*;
    match kind {
        StoreKind::Cash | StoreKind::Bank => &[Topup, Payment, Transfer, Convert],
        StoreKind::Exchange => &[Topup, Payment, Transfer, Convert, TradeSpot, TradeFutures],
        StoreKind::Crypto => &[Topup, Payment, Transfer, Convert, Defi],
        StoreKind::Property => &[Topup, AssetSell],
        StoreKind::Liability => &[Payment, Transfer],
    }
}

/// The kind a new-transaction form opens with on a store of this kind.
pub fn default_kind(kind: StoreKind) -> TxKind {
    match kind {
        StoreKind::Cash | StoreKind::Bank => TxKind::Payment,
        StoreKind::Exchange => TxKind::TradeSpot,
        StoreKind::Crypto | StoreKind::Liability => TxKind::Transfer,
        StoreKind::Property => TxKind::Topup,
    }
}

/// One thing the user did with money.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Transaction {
    Topup(TopupTx),
    Payment(PaymentTx),
    Transfer(TransferTx),
    Convert(ConvertTx),
    TradeSpot(TradeSpotTx),
    TradeFutures(TradeFuturesTx),
    Defi(DefiTx),
    AssetSell(AssetSellTx),
}

macro_rules! each_variant {
    ($self:expr, $tx:ident => $body:expr) => {
        match $self {
            Transaction::Topup($tx) => $body,
            Transaction::Payment($tx) => $body,
            Transaction::Transfer($tx) => $body,
            Transaction::Convert($tx) => $body,
            Transaction::TradeSpot($tx) => $body,
            Transaction::TradeFutures($tx) => $body,
            Transaction::Defi($tx) => $body,
            Transaction::AssetSell($tx) => $body,
        }
    };
}

impl Transaction {
    pub fn kind(&self) -> TxKind {
        match self {
            Self::Topup(_) => TxKind::Topup,
            Self::Payment(_) => TxKind::Payment,
            Self::Transfer(_) => TxKind::Transfer,
            Self::Convert(_) => TxKind::Convert,
            Self::TradeSpot(_) => TxKind::TradeSpot,
            Self::TradeFutures(_) => TxKind::TradeFutures,
            Self::Defi(_) => TxKind::Defi,
            Self::AssetSell(_) => TxKind::AssetSell,
        }
    }

    pub fn tag(&self) -> &'static str {
        self.kind().tag()
    }

    pub fn meta(&self) -> &TxMeta {
        each_variant!(self, tx => &tx.meta)
    }

    pub fn meta_mut(&mut self) -> &mut TxMeta {
        each_variant!(self, tx => &mut tx.meta)
    }

    pub fn fee(&self) -> Option<&Fee> {
        each_variant!(self, tx => tx.fee.as_ref())
    }

    /// Every bookkeeping effect, per §4.3 of the plan: principal legs on the
    /// user's stores, a fee leg on the paying side, a PnL leg on a futures close.
    pub fn legs(&self) -> Vec<Leg> {
        let mut legs = Vec::new();
        match self {
            Self::Topup(tx) => {
                legs.push(Leg::in_coming(
                    &tx.store_id,
                    &tx.asset,
                    tx.amount,
                    &tx.price,
                ));
                push_fee(&mut legs, Some(&tx.store_id), &tx.fee);
            }
            Self::Payment(tx) => {
                legs.push(Leg::out_going(
                    &tx.store_id,
                    &tx.asset,
                    tx.amount,
                    &tx.price,
                ));
                push_fee(&mut legs, Some(&tx.store_id), &tx.fee);
            }
            Self::Transfer(tx) => {
                let from = tx.from.store_id();
                let to = tx.to.store_id();
                if let Some(from) = from {
                    let mut leg = Leg::out_going(from, &tx.asset, tx.amount, &tx.price);
                    leg.to_store_id = to.map(str::to_string);
                    legs.push(leg);
                }
                if let Some(to) = to {
                    let mut leg = Leg::in_coming(to, &tx.asset, tx.amount, &tx.price);
                    leg.to_store_id = from.map(str::to_string);
                    legs.push(leg);
                }
                push_fee(&mut legs, from.or(to), &tx.fee);
            }
            Self::Convert(tx) => {
                let from = tx.from.account.store_id();
                let to = tx.to.account.store_id();
                if let Some(from) = from {
                    let mut leg =
                        Leg::out_going(from, &tx.from.asset, tx.from.amount, &tx.from.price);
                    leg.to_store_id = to.map(str::to_string);
                    legs.push(leg);
                }
                if let Some(to) = to {
                    let mut leg = Leg::in_coming(to, &tx.to.asset, tx.to.amount, &tx.to.price);
                    leg.to_store_id = from.map(str::to_string);
                    legs.push(leg);
                }
                push_fee(&mut legs, from.or(to), &tx.fee);
            }
            Self::TradeSpot(tx) => {
                legs.push(Leg::out_going(
                    &tx.store_id,
                    &tx.sold.asset,
                    tx.sold.qty,
                    &tx.sold.price,
                ));
                legs.push(Leg::in_coming(
                    &tx.store_id,
                    &tx.bought.asset,
                    tx.bought.qty,
                    &tx.bought.price,
                ));
                push_fee(&mut legs, Some(&tx.store_id), &tx.fee);
            }
            Self::TradeFutures(tx) => {
                let grows = matches!(
                    (tx.direction, tx.action),
                    (FuturesDirection::Long, FuturesAction::Open)
                        | (FuturesDirection::Short, FuturesAction::Close)
                );
                let none = PriceStamp::default();
                legs.push(if grows {
                    Leg::in_coming(&tx.store_id, &tx.contract, tx.quantity, &none)
                } else {
                    Leg::out_going(&tx.store_id, &tx.contract, tx.quantity, &none)
                });
                if let (FuturesAction::Close, Some(pnl)) = (tx.action, tx.closed_pnl)
                    && !pnl.is_zero()
                {
                    legs.push(Leg::new(
                        &tx.store_id,
                        &tx.settle,
                        pnl,
                        LegRole::Pnl,
                        &tx.price,
                    ));
                }
                push_fee(&mut legs, Some(&tx.store_id), &tx.fee);
            }
            Self::Defi(tx) => {
                legs.push(if tx.action.is_incoming() {
                    Leg::in_coming(&tx.store_id, &tx.asset, tx.amount, &tx.price)
                } else {
                    Leg::out_going(&tx.store_id, &tx.asset, tx.amount, &tx.price)
                });
                push_fee(&mut legs, Some(&tx.store_id), &tx.fee);
            }
            Self::AssetSell(tx) => {
                let mut sold =
                    Leg::out_going(&tx.store_id, &tx.asset, tx.quantity, &PriceStamp::default());
                sold.to_store_id = tx.proceeds_to.clone();
                legs.push(sold);
                if let Some(target) = &tx.proceeds_to {
                    let mut proceeds = Leg::in_coming(
                        target,
                        &tx.currency,
                        tx.unit_price * tx.quantity,
                        &tx.price,
                    );
                    proceeds.to_store_id = Some(tx.store_id.clone());
                    legs.push(proceeds);
                }
                push_fee(&mut legs, Some(&tx.store_id), &tx.fee);
            }
        }
        legs
    }

    /// Every account reference the transaction names (both sides of a
    /// transfer or conversion, the counterparty of a topup or payment).
    pub fn accounts(&self) -> Vec<&AccountRef> {
        match self {
            Self::Topup(tx) => tx.from.iter().collect(),
            Self::Payment(tx) => tx.to.iter().collect(),
            Self::Transfer(tx) => vec![&tx.from, &tx.to],
            Self::Convert(tx) => vec![&tx.from.account, &tx.to.account],
            _ => Vec::new(),
        }
    }

    /// Every store the transaction books on or names.
    pub fn store_ids(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for leg in self.legs() {
            for id in std::iter::once(leg.store_id).chain(leg.to_store_id) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        for account in self.accounts() {
            if let Some(id) = account.store_id()
                && !out.iter().any(|known| known == id)
            {
                out.push(id.to_string());
            }
        }
        out
    }

    pub fn touches(&self, store_id: &str) -> bool {
        self.store_ids().iter().any(|id| id == store_id)
    }

    /// Store the transaction is primarily booked on: the source of a transfer
    /// (or its target when the source is external).
    pub fn primary_store_id(&self) -> Option<&str> {
        match self {
            Self::Topup(tx) => Some(&tx.store_id),
            Self::Payment(tx) => Some(&tx.store_id),
            Self::Transfer(tx) => tx.from.store_id().or(tx.to.store_id()),
            Self::Convert(tx) => tx.from.account.store_id().or(tx.to.account.store_id()),
            Self::TradeSpot(tx) => Some(&tx.store_id),
            Self::TradeFutures(tx) => Some(&tx.store_id),
            Self::Defi(tx) => Some(&tx.store_id),
            Self::AssetSell(tx) => Some(&tx.store_id),
        }
    }

    /// Every contact the transaction names: `meta.contact_id` and contact sides.
    pub fn contact_ids(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.meta().contact_id.as_deref().into_iter().collect();
        for account in self.accounts() {
            if let Some(id) = account.contact_id()
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
        out
    }

    /// A transfer or conversion with both sides inside one top-level wallet:
    /// money moved, but it neither came in nor went out.
    pub fn is_internal(&self, root_of: impl Fn(&str) -> String) -> bool {
        let (from, to) = match self {
            Self::Transfer(tx) => (&tx.from, &tx.to),
            Self::Convert(tx) => (&tx.from.account, &tx.to.account),
            _ => return false,
        };
        match (from.store_id(), to.store_id()) {
            (Some(left), Some(right)) => root_of(left) == root_of(right),
            _ => false,
        }
    }

    /// `(network, hash)` pairs the transaction claims, for shape checks and explorer links.
    pub fn on_chain_refs(&self) -> Vec<(Option<Chain>, Option<&str>)> {
        match self {
            Self::Topup(tx) => vec![(tx.details.network, tx.details.tx_hash.as_deref())],
            Self::Payment(tx) => vec![(tx.details.network, tx.details.tx_hash.as_deref())],
            Self::Transfer(tx) => vec![(tx.network, tx.tx_hash.as_deref())],
            Self::Defi(tx) => vec![(tx.network, tx.tx_hash.as_deref())],
            _ => vec![],
        }
    }

    /// Chains of the principal assets moved.
    pub fn asset_chains(&self) -> Vec<Option<Chain>> {
        self.legs()
            .into_iter()
            .filter(|leg| leg.role == LegRole::Principal)
            .map(|leg| leg.asset.chain)
            .collect()
    }

    pub fn explorer_url(&self) -> Option<String> {
        self.on_chain_refs()
            .into_iter()
            .find_map(|(chain, hash)| match (chain, hash) {
                (Some(chain), Some(hash)) if !hash.is_empty() => chain.explorer_url(hash),
                _ => None,
            })
    }

    /// Every price stamp with the asset it prices — the one place that knows
    /// which sides of which variant carry a price.
    pub fn stamps_mut(&mut self) -> Vec<(AssetRef, &mut PriceStamp)> {
        let mut out: Vec<(AssetRef, &mut PriceStamp)> = Vec::new();
        let fee = match self {
            Self::Topup(tx) => {
                out.push((tx.asset.clone(), &mut tx.price));
                &mut tx.fee
            }
            Self::Payment(tx) => {
                out.push((tx.asset.clone(), &mut tx.price));
                &mut tx.fee
            }
            Self::Transfer(tx) => {
                out.push((tx.asset.clone(), &mut tx.price));
                &mut tx.fee
            }
            Self::Convert(tx) => {
                out.push((tx.from.asset.clone(), &mut tx.from.price));
                out.push((tx.to.asset.clone(), &mut tx.to.price));
                &mut tx.fee
            }
            Self::TradeSpot(tx) => {
                out.push((tx.sold.asset.clone(), &mut tx.sold.price));
                out.push((tx.bought.asset.clone(), &mut tx.bought.price));
                &mut tx.fee
            }
            Self::TradeFutures(tx) => {
                out.push((tx.settle.clone(), &mut tx.price));
                &mut tx.fee
            }
            Self::Defi(tx) => {
                out.push((tx.asset.clone(), &mut tx.price));
                &mut tx.fee
            }
            Self::AssetSell(tx) => {
                out.push((tx.currency.clone(), &mut tx.price));
                &mut tx.fee
            }
        };
        if let Some(fee) = fee {
            out.push((fee.asset.clone(), &mut fee.price));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn usd() -> AssetRef {
        AssetRef::fiat("USD", "US Dollar")
    }

    fn btc() -> AssetRef {
        AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin)
    }

    fn fee(raw: &str) -> Option<Fee> {
        Some(Fee {
            asset: usd(),
            amount: amount(raw),
            price: PriceStamp::default(),
        })
    }

    fn transfer(from: AccountRef, to: AccountRef, fee_raw: Option<&str>) -> Transaction {
        Transaction::Transfer(TransferTx {
            meta: TxMeta::new("move"),
            from,
            to,
            asset: usd(),
            amount: amount("100"),
            fee: fee_raw.and_then(fee),
            network: None,
            tx_hash: None,
            debt: None,
            price: PriceStamp::default(),
        })
    }

    fn signed(legs: &[Leg]) -> Vec<(String, String, LegRole)> {
        legs.iter()
            .map(|leg| (leg.store_id.clone(), leg.amount.to_string(), leg.role))
            .collect()
    }

    #[test]
    fn topup_and_payment_book_the_fee_on_their_store() {
        let topup = Transaction::Topup(TopupTx {
            meta: TxMeta::new("salary"),
            store_id: "bank".into(),
            asset: usd(),
            amount: amount("1000"),
            fee: fee("2"),
            from: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        });
        assert_eq!(
            signed(&topup.legs()),
            vec![
                ("bank".into(), "1000".into(), LegRole::Principal),
                ("bank".into(), "-2".into(), LegRole::Fee),
            ]
        );
        let payment = Transaction::Payment(PaymentTx {
            meta: TxMeta::new("rent"),
            store_id: "bank".into(),
            asset: usd(),
            amount: amount("500"),
            fee: fee("1"),
            to: Some(AccountRef::Raw("landlord".into())),
            details: KindDetails::default(),
            price: PriceStamp::default(),
        });
        assert_eq!(
            signed(&payment.legs()),
            vec![
                ("bank".into(), "-500".into(), LegRole::Principal),
                ("bank".into(), "-1".into(), LegRole::Fee),
            ]
        );
    }

    #[test]
    fn a_transfer_between_stores_charges_the_sender() {
        let tx = transfer(
            AccountRef::Store("a".into()),
            AccountRef::Store("b".into()),
            Some("3"),
        );
        let legs = tx.legs();
        assert_eq!(
            signed(&legs),
            vec![
                ("a".into(), "-100".into(), LegRole::Principal),
                ("b".into(), "100".into(), LegRole::Principal),
                ("a".into(), "-3".into(), LegRole::Fee),
            ]
        );
        assert_eq!(legs[0].to_store_id.as_deref(), Some("b"));
        assert_eq!(legs[1].to_store_id.as_deref(), Some("a"));
        assert!(tx.touches("a") && tx.touches("b") && !tx.touches("c"));
    }

    #[test]
    fn a_transfer_with_an_outside_side_books_only_the_store() {
        let contact = AccountRef::Contact {
            contact_id: "mom".into(),
            account_id: None,
        };
        let out = transfer(AccountRef::Store("a".into()), contact.clone(), Some("1"));
        assert_eq!(
            signed(&out.legs()),
            vec![
                ("a".into(), "-100".into(), LegRole::Principal),
                ("a".into(), "-1".into(), LegRole::Fee),
            ]
        );
        let inn = transfer(contact, AccountRef::Store("b".into()), Some("1"));
        assert_eq!(
            signed(&inn.legs()),
            vec![
                ("b".into(), "100".into(), LegRole::Principal),
                ("b".into(), "-1".into(), LegRole::Fee),
            ]
        );
        assert_eq!(inn.contact_ids(), vec!["mom"]);
    }

    #[test]
    fn convert_books_both_sides_and_the_fee_from_the_source() {
        let tx = Transaction::Convert(ConvertTx {
            meta: TxMeta::new("fx"),
            from: ConvertSide {
                account: AccountRef::Store("rub".into()),
                asset: AssetRef::fiat("RUB", "Ruble"),
                amount: amount("9000"),
                price: PriceStamp::default(),
            },
            to: ConvertSide {
                account: AccountRef::Store("usd".into()),
                asset: usd(),
                amount: amount("100"),
                price: PriceStamp::default(),
            },
            fee: fee("1"),
            rate_override: None,
        });
        assert_eq!(
            signed(&tx.legs()),
            vec![
                ("rub".into(), "-9000".into(), LegRole::Principal),
                ("usd".into(), "100".into(), LegRole::Principal),
                ("rub".into(), "-1".into(), LegRole::Fee),
            ]
        );
    }

    fn futures(
        action: FuturesAction,
        direction: FuturesDirection,
        pnl: Option<&str>,
    ) -> Transaction {
        Transaction::TradeFutures(TradeFuturesTx {
            meta: TxMeta::new("perp"),
            store_id: "bybit".into(),
            contract: AssetRef::perp("bybit", "BTCUSDT"),
            settle: AssetRef::fiat("USDT", "Tether"),
            direction,
            action,
            quantity: amount("0.5"),
            fill_price: amount("60000"),
            leverage: Some(amount("10")),
            fee: None,
            closed_pnl: pnl.map(amount),
            order_id: None,
            category: FuturesCategory::Linear,
            price: PriceStamp::default(),
        })
    }

    #[test]
    fn futures_move_the_contract_and_settle_pnl() {
        let open = futures(FuturesAction::Open, FuturesDirection::Short, None);
        assert_eq!(open.legs()[0].amount.to_string(), "-0.5");
        assert!(open.legs()[0].asset.is_derivative());

        let close = futures(FuturesAction::Close, FuturesDirection::Short, Some("-120"));
        let legs = close.legs();
        assert_eq!(legs[0].amount.to_string(), "0.5");
        assert_eq!(legs[1].role, LegRole::Pnl);
        assert_eq!(legs[1].amount.to_string(), "-120");
        assert_eq!(legs[1].asset.symbol, "USDT");
    }

    #[test]
    fn asset_sell_moves_the_proceeds() {
        let tx = Transaction::AssetSell(AssetSellTx {
            meta: TxMeta::new("car"),
            store_id: "things".into(),
            asset: AssetRef::fiat("CAR", "Car"),
            unit_price: amount("15000"),
            quantity: amount("1"),
            currency: usd(),
            proceeds_to: Some("bank".into()),
            fee: None,
            price: PriceStamp::default(),
        });
        assert_eq!(
            signed(&tx.legs()),
            vec![
                ("things".into(), "-1".into(), LegRole::Principal),
                ("bank".into(), "15000".into(), LegRole::Principal),
            ]
        );
    }

    #[test]
    fn internal_means_one_top_level_wallet() {
        let root = |id: &str| {
            if id.starts_with("bybit") {
                "bybit".to_string()
            } else {
                id.to_string()
            }
        };
        let inside = transfer(
            AccountRef::Store("bybit-spot".into()),
            AccountRef::Store("bybit-futures".into()),
            None,
        );
        assert!(inside.is_internal(root));
        let across = transfer(
            AccountRef::Store("bybit-spot".into()),
            AccountRef::Store("ledger".into()),
            None,
        );
        assert!(!across.is_internal(root));
        let out = transfer(
            AccountRef::Store("bybit-spot".into()),
            AccountRef::Raw("x".into()),
            None,
        );
        assert!(!out.is_internal(root));
    }

    #[test]
    fn every_store_kind_has_a_default_it_accepts() {
        for kind in [
            StoreKind::Cash,
            StoreKind::Bank,
            StoreKind::Exchange,
            StoreKind::Crypto,
            StoreKind::Crypto,
            StoreKind::Property,
            StoreKind::Liability,
        ] {
            assert!(
                allowed_kinds(kind).contains(&default_kind(kind)),
                "{kind:?}"
            );
        }
        assert!(!allowed_kinds(StoreKind::Property).contains(&TxKind::Payment));
        assert!(!allowed_kinds(StoreKind::Liability).contains(&TxKind::Topup));
        assert!(allowed_kinds(StoreKind::Exchange).contains(&TxKind::TradeFutures));
    }

    #[test]
    fn stamps_cover_every_priced_side() {
        let mut tx = Transaction::TradeSpot(TradeSpotTx {
            meta: TxMeta::new("dip"),
            store_id: "ex".into(),
            sold: TradeSide {
                asset: usd(),
                qty: amount("1000"),
                price: PriceStamp::default(),
            },
            bought: TradeSide {
                asset: btc(),
                qty: amount("0.02"),
                price: PriceStamp::default(),
            },
            fee: fee("1"),
            liquidity: Some(Liquidity::Maker),
            order_id: None,
        });
        let assets: Vec<String> = tx
            .stamps_mut()
            .into_iter()
            .map(|(asset, _)| asset.symbol)
            .collect();
        assert_eq!(assets, vec!["USD", "BTC", "USD"]);
        for (asset, stamp) in tx.stamps_mut() {
            stamp.fill(
                Some(if asset.symbol == "BTC" {
                    amount("50000")
                } else {
                    amount("1")
                }),
                None,
            );
        }
        let legs = tx.legs();
        assert_eq!(legs[1].price_usd, Some(amount("50000")));
        assert_eq!(legs[0].price_usd, Some(amount("1")));
    }

    #[test]
    fn every_variant_round_trips_through_json() {
        let samples = vec![
            transfer(
                AccountRef::Store("a".into()),
                AccountRef::Raw("b".into()),
                Some("1"),
            ),
            futures(FuturesAction::Close, FuturesDirection::Long, Some("5")),
        ];
        for tx in samples {
            let json = serde_json::to_string(&tx).unwrap();
            let back: Transaction = serde_json::from_str(&json).unwrap();
            assert_eq!(back, tx);
        }
    }
}
