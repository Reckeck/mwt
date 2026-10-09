use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::vault::now_iso;

/// Direction of an asset movement relative to the store it is booked on.
///
/// This is data, not a convention. The flat `Transaction` this replaced inferred
/// the sign from `TxKind` inside `signed_delta`, so a withdrawal stored a
/// negative `amount` that nothing checked. Here the direction is an explicit
/// field and [`Flow::signed`] is the only place it becomes a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    /// Asset flows **into** the store.
    In,
    /// Asset flows **out of** the store.
    Out,
}

impl Flow {
    /// Magnitude with the direction applied: positive for `In`, non-positive
    /// for `Out`.
    pub fn signed(self, amount: Amount) -> Amount {
        match self {
            Self::In => amount.abs(),
            Self::Out => -amount.abs(),
        }
    }

    pub fn is_in(self) -> bool {
        matches!(self, Self::In)
    }
}

/// Price stamps a money movement can carry.
///
/// Shared by every variant because pricing is a property of an asset at a
/// moment, not of the kind of movement: a bank transfer and a DeFi claim are
/// both worth something at a rate. Keeping it in one struct stops eleven
/// copies from drifting apart, and [`PriceStamp::on`] is the single place that
/// decides which legs a price reaches.
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
    ///
    /// A price belongs to an asset, and a trade has two, so a manual override
    /// must not erase the price the other leg already carries.
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

/// Fields every transaction carries, whatever it is.
///
/// Not repeated per variant: these five are never kind-specific, and twelve
/// copies would guarantee they drift apart.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TxMeta {
    pub id: String,
    /// RFC3339 UTC, lexicographically sortable — a real sort key, not a label.
    pub timestamp: String,
    /// User note. Free-form; the UI requires it.
    pub comment: String,
    /// User tags. Any number, all members of `VaultDoc::categories`.
    /// Normalized (trimmed, lowercased, deduplicated) before it is stored.
    pub categories: Vec<String>,
}

impl TxMeta {
    pub fn new(comment: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_iso(),
            comment: comment.trim().to_string(),
            categories: Vec::new(),
        }
    }
}

/// One bookkeeping effect on one `(store, asset)` pair.
///
/// A transaction with two sides expands into two of these. Everything that
/// sums money — `recompute_holdings`, the history series, realized PnL —
/// consumes legs and never inspects the variant, so those replays behave
/// identically no matter what kind of transaction produced the movement.
///
/// Derived, never persisted: the vault stores the variant, and `legs()`
/// recomputes these on read.
#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    pub store_id: String,
    pub asset: AssetRef,
    /// Signed: positive adds to `store_id`, negative subtracts. Build it with
    /// [`Flow::signed`] rather than negating by hand.
    pub amount: Amount,
    /// Counterparty store for cross-store movements. Set on both legs so a
    /// store can find every transaction touching it from either side.
    pub to_store_id: Option<String>,
    pub price_usd: Option<Amount>,
    pub price_base: Option<Amount>,
    pub fx_override: Option<Amount>,
}

impl Leg {
    fn new(store_id: &str, asset: AssetRef, amount: Amount) -> Self {
        Self {
            store_id: store_id.to_string(),
            asset,
            amount,
            to_store_id: None,
            price_usd: None,
            price_base: None,
            fx_override: None,
        }
    }

    pub fn out_going(store_id: &str, asset: AssetRef, amount: Amount) -> Self {
        Self {
            amount: -amount.abs(),
            ..Self::new(store_id, asset, amount)
        }
    }

    pub fn in_coming(store_id: &str, asset: AssetRef, amount: Amount) -> Self {
        Self {
            amount: amount.abs(),
            ..Self::new(store_id, asset, amount)
        }
    }
}

/// Where a bank payment moved. Meaningless for a crypto wallet, which is why it
/// exists only on [`BankTransferTx`].
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BankRail {
    /// Same-bank move between the user's own accounts.
    #[default]
    Internal,
    Sepa,
    Swift,
    Wire,
    Card,
    Cashier,
}

/// A payment made from a bank account.
///
/// The pilot for per-kind fields: everything on it is a fact about banking that
/// the other variants cannot express, which is exactly what the flat
/// `Transaction` had nowhere to put.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct BankTransferTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    /// Magnitude; [`Self::flow`] carries the direction.
    pub amount: Amount,
    pub flow: Flow,
    pub rail: BankRail,
    /// Who received the money — usually a name, not an identifier.
    #[serde(default)]
    pub counterparty_name: String,
    /// IBAN / BIC / account number, as the rail requires.
    #[serde(default)]
    pub counterparty_account: Option<String>,
    /// The counterparty's own reference, for reconciliation.
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub fee: Option<Amount>,
    #[serde(default)]
    pub fee_asset: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl BankTransferTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::new(
            &self.store_id,
            self.asset.clone(),
            self.flow.signed(self.amount),
        );
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// A purchase paid by card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CardPaymentTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    /// Last four digits only; the full number is never stored.
    #[serde(default)]
    pub card_last4: Option<String>,
    #[serde(default)]
    pub merchant: Option<String>,
    /// Merchant category code as it appears on the statement.
    #[serde(default)]
    pub mcc: Option<String>,
    #[serde(default)]
    pub fee: Option<Amount>,
    #[serde(default)]
    pub fee_asset: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl CardPaymentTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::out_going(&self.store_id, self.asset.clone(), self.amount);
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// A movement into or out of a crypto wallet, on-chain or across an
/// exchange's internal ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct WalletTransferTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub flow: Flow,
    #[serde(default)]
    pub network: Option<Chain>,
    /// On-chain transaction hash; links to the explorer.
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub from_address: Option<String>,
    #[serde(default)]
    pub to_address: Option<String>,
    /// Destination tag / memo; several exchanges require one to credit funds.
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub fee: Option<Amount>,
    #[serde(default)]
    pub fee_asset: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl WalletTransferTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::new(
            &self.store_id,
            self.asset.clone(),
            self.flow.signed(self.amount),
        );
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// What was done to a DeFi position.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DefiAction {
    #[default]
    Stake,
    Unstake,
    Claim,
    AddLiquidity,
    RemoveLiquidity,
}

/// An on-chain DeFi interaction on a hot or cold wallet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DefiActionTx {
    pub meta: TxMeta,
    pub store_id: String,
    /// The asset the action moved: the staked asset, or the reward claimed.
    pub asset: AssetRef,
    pub amount: Amount,
    pub flow: Flow,
    pub action: DefiAction,
    #[serde(default)]
    pub network: Option<Chain>,
    #[serde(default)]
    pub tx_hash: Option<String>,
    /// e.g. `lido`, `uniswap-v3`.
    #[serde(default)]
    pub protocol: Option<String>,
    /// Pool, vault or staking contract identifier.
    #[serde(default)]
    pub pool_id: Option<String>,
    #[serde(default)]
    pub fee: Option<Amount>,
    #[serde(default)]
    pub fee_asset: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl DefiActionTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::new(
            &self.store_id,
            self.asset.clone(),
            self.flow.signed(self.amount),
        );
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// One side of a trade, as booked on the store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TradeSide {
    pub asset: AssetRef,
    /// Magnitude, always positive; the direction comes from the leg it lands on.
    pub qty: Amount,
}

/// A trade between two assets inside one store.
///
/// Both sides live in one record rather than in two transactions joined by a
/// `group_id`. A trade is one event with two effects; splitting it meant every
/// reader had to reassemble the halves by hand, and deleting one half silently
/// orphaned the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ExchangeTradeTx {
    pub meta: TxMeta,
    pub store_id: String,
    /// The side that leaves the store.
    pub sold: TradeSide,
    /// The side that arrives in the store.
    pub bought: TradeSide,
    #[serde(default)]
    pub fee: Option<Amount>,
    #[serde(default)]
    pub fee_asset: Option<String>,
    /// The exchange's own order identifier, for reconciling with the account.
    #[serde(default)]
    pub order_id: Option<String>,
    /// Which market the fill happened on: `spot`, `linear`, `inverse`.
    #[serde(default)]
    pub market: Option<String>,
    /// `maker` or `taker`.
    #[serde(default)]
    pub liquidity: Option<String>,
    /// Prices the bought side: that is the leg a holding's average cost is
    /// derived from.
    #[serde(default)]
    pub price: PriceStamp,
}

impl ExchangeTradeTx {
    pub fn legs(&self) -> Vec<Leg> {
        // Both legs carry the fill price. The sold leg needs it to measure
        // realized PnL (it is the exit price), and the bought leg needs it to
        // open the average cost. Double-counting is not a risk:
        // `recompute_holdings` only folds a price into avg_cost when the leg is
        // an increase, so the sold leg contributes its quantity and nothing
        // more.
        let mut sold = Leg::out_going(&self.store_id, self.sold.asset.clone(), self.sold.qty);
        self.price.on(&mut sold);
        let mut bought = Leg::in_coming(&self.store_id, self.bought.asset.clone(), self.bought.qty);
        self.price.on(&mut bought);
        vec![sold, bought]
    }
}

/// A movement between two stores of any kind.
///
/// One variant rather than one per ordered pair of store kinds: six kinds
/// would mean twelve near-identical variants to keep in sync, and the
/// kind-specific detail of each side already belongs to the store it lands on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TransferTx {
    pub meta: TxMeta,
    /// Store the asset leaves.
    pub from_store_id: String,
    /// Store the asset arrives in.
    pub to_store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    #[serde(default)]
    pub fee: Option<Amount>,
    #[serde(default)]
    pub fee_asset: Option<String>,
    #[serde(default)]
    pub network: Option<Chain>,
    #[serde(default)]
    pub tx_hash: Option<String>,
    /// Both legs move the same asset, so both share one price.
    #[serde(default)]
    pub price: PriceStamp,
}

impl TransferTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut out = Leg::out_going(&self.from_store_id, self.asset.clone(), self.amount);
        out.to_store_id = Some(self.to_store_id.clone());
        self.price.on(&mut out);
        let mut inn = Leg::in_coming(&self.to_store_id, self.asset.clone(), self.amount);
        inn.to_store_id = Some(self.from_store_id.clone());
        self.price.on(&mut inn);
        vec![out, inn]
    }
}

/// Where money came from.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IncomeSource {
    #[default]
    Other,
    Salary,
    Reward,
    Airdrop,
    Staking,
    Interest,
    Mining,
}

/// An increase of a store's balance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct IncomeTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub source: IncomeSource,
    #[serde(default)]
    pub price: PriceStamp,
}

impl IncomeTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::in_coming(&self.store_id, self.asset.clone(), self.amount);
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// A withdrawal or a spend that is not a purchase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ExpenseTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    #[serde(default)]
    pub price: PriceStamp,
}

impl ExpenseTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::out_going(&self.store_id, self.asset.clone(), self.amount);
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// Buying something: money out, but the reporting built on the tags is about
/// spending rather than about withdrawals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PaymentTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    /// Who was paid: a shop, a person, a service.
    #[serde(default)]
    pub payee: String,
    /// Receipt, invoice or subscription id.
    #[serde(default)]
    pub receipt_ref: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl PaymentTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::out_going(&self.store_id, self.asset.clone(), self.amount);
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// A fee booked on its own rather than attached to the operation that caused it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct FeeTx {
    pub meta: TxMeta,
    pub store_id: String,
    /// Asset the fee was charged in; may differ from the traded asset.
    pub asset: AssetRef,
    pub amount: Amount,
    /// Transaction this fee was split out of, when it was.
    #[serde(default)]
    pub related_id: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl FeeTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::out_going(&self.store_id, self.asset.clone(), self.amount);
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// Physical money.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CashFlowTx {
    pub meta: TxMeta,
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub flow: Flow,
    /// Where the cash moved: an ATM, a shop, a person.
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub counterparty: Option<String>,
    #[serde(default)]
    pub price: PriceStamp,
}

impl CashFlowTx {
    pub fn legs(&self) -> Vec<Leg> {
        let mut leg = Leg::new(
            &self.store_id,
            self.asset.clone(),
            self.flow.signed(self.amount),
        );
        self.price.on(&mut leg);
        vec![leg]
    }
}

/// A recorded transaction.
///
/// Every variant carries its own fields instead of sharing one flat struct
/// with everything optional: a `BankTransferTx` cannot hold a DEX `pool_id` and
/// a `CashFlowTx` cannot hold an IBAN, so an impossible record is not
/// representable.
///
/// Serde's default externally tagged shape is used deliberately. It round
/// trips through bincode, which cannot handle string tags, and in TypeScript it
/// becomes a union of single-key objects that `'bank_transfer' in tx` narrows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Transaction {
    Income(IncomeTx),
    Expense(ExpenseTx),
    Payment(PaymentTx),
    Fee(FeeTx),
    BankTransfer(BankTransferTx),
    CardPayment(CardPaymentTx),
    WalletTransfer(WalletTransferTx),
    DefiAction(DefiActionTx),
    ExchangeTrade(ExchangeTradeTx),
    Transfer(TransferTx),
    CashFlow(CashFlowTx),
}

impl Transaction {
    /// Stable tag for this transaction's kind: the externally tagged key.
    /// Lets a UI filter or label without matching every variant.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Income(_) => "income",
            Self::Expense(_) => "expense",
            Self::Payment(_) => "payment",
            Self::Fee(_) => "fee",
            Self::BankTransfer(_) => "bank_transfer",
            Self::CardPayment(_) => "card_payment",
            Self::WalletTransfer(_) => "wallet_transfer",
            Self::DefiAction(_) => "defi_action",
            Self::ExchangeTrade(_) => "exchange_trade",
            Self::Transfer(_) => "transfer",
            Self::CashFlow(_) => "cash_flow",
        }
    }

    /// Every kind, in declaration order. Drives filter lists so the UI does not
    /// hard-code a list that can fall out of step with the enum.
    pub const ALL_TAGS: &'static [&'static str] = &[
        "income",
        "expense",
        "payment",
        "fee",
        "bank_transfer",
        "card_payment",
        "wallet_transfer",
        "defi_action",
        "exchange_trade",
        "transfer",
        "cash_flow",
    ];

    /// Kinds that make sense on *any* wallet, whatever it is.
    ///
    /// `transfer` belongs here despite spanning two stores: it is available
    /// from every wallet precisely because the other end can be anything, so
    /// binding it to one store kind would be wrong.
    pub const COMMON_TAGS: &'static [&'static str] =
        &["income", "expense", "payment", "fee", "transfer"];

    /// Kinds that only make sense for a particular kind of wallet.
    ///
    /// This is the whole point of giving each variant its own fields: a
    /// [`BankTransferTx`] can only exist on a bank, because nowhere else is
    /// there an IBAN to record.
    ///
    /// # This is the single source of truth
    ///
    /// The frontend asks [`Self::tags_for_store_kind`] rather than keeping its
    /// own copy, so a new [`StoreKind`] cannot leave the UI offering kinds that
    /// make no sense for it.
    pub fn specific_tags_for_store_kind(kind: StoreKind) -> &'static [&'static str] {
        match kind {
            StoreKind::Exchange => &["exchange_trade", "wallet_transfer"],
            StoreKind::HotWallet | StoreKind::ColdWallet => &["wallet_transfer", "defi_action"],
            StoreKind::Bank => &["bank_transfer", "card_payment"],
            StoreKind::Cash => &["cash_flow"],
        }
    }

    /// Every kind a wallet of this kind accepts, common ones first.
    ///
    /// Ordering is the order the UI should present them in: the kinds that work
    /// anywhere lead, and the store-specific ones follow.
    pub fn tags_for_store_kind(kind: StoreKind) -> Vec<&'static str> {
        Self::COMMON_TAGS
            .iter()
            .copied()
            .chain(Self::specific_tags_for_store_kind(kind).iter().copied())
            .collect()
    }

    /// The kind to pre-select when a wallet of this kind is opened.
    ///
    /// The first store-specific kind, because that is what the user most often
    /// came to record on this wallet — an exchange is usually opened to log a
    /// trade, a bank to log a transfer. Falls back to `income` only if a kind
    /// ever has nothing specific to it.
    pub fn default_tag_for_store_kind(kind: StoreKind) -> &'static str {
        Self::specific_tags_for_store_kind(kind)
            .first()
            .copied()
            .unwrap_or("income")
    }

    pub fn meta(&self) -> &TxMeta {
        match self {
            Self::Income(tx) => &tx.meta,
            Self::Expense(tx) => &tx.meta,
            Self::Payment(tx) => &tx.meta,
            Self::Fee(tx) => &tx.meta,
            Self::BankTransfer(tx) => &tx.meta,
            Self::CardPayment(tx) => &tx.meta,
            Self::WalletTransfer(tx) => &tx.meta,
            Self::DefiAction(tx) => &tx.meta,
            Self::ExchangeTrade(tx) => &tx.meta,
            Self::Transfer(tx) => &tx.meta,
            Self::CashFlow(tx) => &tx.meta,
        }
    }

    pub fn meta_mut(&mut self) -> &mut TxMeta {
        match self {
            Self::Income(tx) => &mut tx.meta,
            Self::Expense(tx) => &mut tx.meta,
            Self::Payment(tx) => &mut tx.meta,
            Self::Fee(tx) => &mut tx.meta,
            Self::BankTransfer(tx) => &mut tx.meta,
            Self::CardPayment(tx) => &mut tx.meta,
            Self::WalletTransfer(tx) => &mut tx.meta,
            Self::DefiAction(tx) => &mut tx.meta,
            Self::ExchangeTrade(tx) => &mut tx.meta,
            Self::Transfer(tx) => &mut tx.meta,
            Self::CashFlow(tx) => &mut tx.meta,
        }
    }

    /// The legs this transaction produced. The only thing the analytics layer
    /// needs to know about kinds.
    pub fn legs(&self) -> Vec<Leg> {
        match self {
            Self::Income(tx) => tx.legs(),
            Self::Expense(tx) => tx.legs(),
            Self::Payment(tx) => tx.legs(),
            Self::Fee(tx) => tx.legs(),
            Self::BankTransfer(tx) => tx.legs(),
            Self::CardPayment(tx) => tx.legs(),
            Self::WalletTransfer(tx) => tx.legs(),
            Self::DefiAction(tx) => tx.legs(),
            Self::ExchangeTrade(tx) => tx.legs(),
            Self::Transfer(tx) => tx.legs(),
            Self::CashFlow(tx) => tx.legs(),
        }
    }

    /// Stamp a manual price / FX override, filling only what is missing.
    ///
    /// Legs are derived rather than stored, so there is no mutable view of
    /// them: editing means rebuilding the variant. Amounts and stores are what
    /// the history says happened and are not rewritable.
    pub fn stamp_prices(&mut self, price_usd: Option<Amount>, fx_override: Option<Amount>) {
        match self {
            Self::Income(tx) => tx.price.fill(price_usd, fx_override),
            Self::Expense(tx) => tx.price.fill(price_usd, fx_override),
            Self::Payment(tx) => tx.price.fill(price_usd, fx_override),
            Self::Fee(tx) => tx.price.fill(price_usd, fx_override),
            Self::BankTransfer(tx) => tx.price.fill(price_usd, fx_override),
            Self::CardPayment(tx) => tx.price.fill(price_usd, fx_override),
            Self::WalletTransfer(tx) => tx.price.fill(price_usd, fx_override),
            Self::DefiAction(tx) => tx.price.fill(price_usd, fx_override),
            Self::CashFlow(tx) => tx.price.fill(price_usd, fx_override),
            Self::ExchangeTrade(tx) => tx.price.fill(price_usd, fx_override),
            Self::Transfer(tx) => tx.price.fill(price_usd, fx_override),
        }
    }

    /// Does this transaction touch `store_id`, as either side of a movement?
    pub fn touches(&self, store_id: &str) -> bool {
        self.legs()
            .iter()
            .any(|leg| leg.store_id == store_id || leg.to_store_id.as_deref() == Some(store_id))
    }

    /// Store the transaction is booked on. For a cross-store transfer this is
    /// the source.
    pub fn primary_store_id(&self) -> &str {
        match self {
            Self::Income(tx) => &tx.store_id,
            Self::Expense(tx) => &tx.store_id,
            Self::Payment(tx) => &tx.store_id,
            Self::Fee(tx) => &tx.store_id,
            Self::BankTransfer(tx) => &tx.store_id,
            Self::CardPayment(tx) => &tx.store_id,
            Self::WalletTransfer(tx) => &tx.store_id,
            Self::DefiAction(tx) => &tx.store_id,
            Self::ExchangeTrade(tx) => &tx.store_id,
            Self::CashFlow(tx) => &tx.store_id,
            Self::Transfer(tx) => &tx.from_store_id,
        }
    }

    /// Every `(network, tx_hash)` the transaction claims, for shape-checking.
    ///
    /// Separate from [`Self::legs`] because a hash belongs to the *transaction*:
    /// a trade has two assets and one hash.
    pub fn on_chain_refs(&self) -> Vec<(Option<Chain>, Option<&str>)> {
        match self {
            Self::WalletTransfer(tx) => vec![(tx.network, tx.tx_hash.as_deref())],
            Self::DefiAction(tx) => vec![(tx.network, tx.tx_hash.as_deref())],
            Self::Transfer(tx) => vec![(tx.network, tx.tx_hash.as_deref())],
            _ => vec![],
        }
    }

    /// Chains of the assets this transaction moved, so a claimed hash can be
    /// checked against the network it says it is on.
    pub fn asset_chains(&self) -> Vec<Option<Chain>> {
        self.legs().into_iter().map(|leg| leg.asset.chain).collect()
    }

    /// Explorer link, for the variants that carry a hash and a chain.
    pub fn explorer_url(&self) -> Option<String> {
        let (chain, hash) = match self {
            Self::WalletTransfer(tx) => (tx.network, tx.tx_hash.as_deref()),
            Self::DefiAction(tx) => (tx.network, tx.tx_hash.as_deref()),
            Self::Transfer(tx) => (tx.network, tx.tx_hash.as_deref()),
            _ => return None,
        };
        match (chain, hash) {
            (Some(chain), Some(hash)) if !hash.is_empty() => chain.explorer_url(hash),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `StoreKind`, spelled out: a new variant that is not added here
    /// makes the coverage tests fail rather than pass vacuously.
    const ALL_STORE_KINDS: &[StoreKind] = &[
        StoreKind::Exchange,
        StoreKind::HotWallet,
        StoreKind::ColdWallet,
        StoreKind::Bank,
        StoreKind::Cash,
    ];

    fn usd() -> AssetRef {
        AssetRef::fiat("USD", "US Dollar")
    }

    fn bank_transfer(store: &str, flow: Flow, amount: &str) -> BankTransferTx {
        BankTransferTx {
            meta: TxMeta::new("rent"),
            store_id: store.to_string(),
            asset: usd(),
            amount: Amount::parse(amount).unwrap(),
            flow,
            rail: BankRail::Sepa,
            counterparty_name: "Landlord GmbH".into(),
            counterparty_account: Some("DE89…".into()),
            reference: Some("INV-1".into()),
            fee: None,
            fee_asset: None,
            price: PriceStamp::default(),
        }
    }

    /// The direction field must be the only source of the sign: a stored
    /// magnitude is always positive and never relies on convention.
    #[test]
    fn flow_decides_the_sign() {
        assert!(Flow::In.signed(Amount::parse("30").unwrap()).is_positive());
        assert!(Flow::Out.signed(Amount::parse("30").unwrap()).is_negative());
        // a negative magnitude is normalized, not trusted
        assert!(Flow::In.signed(Amount::parse("-30").unwrap()).is_positive());
        assert!(
            Flow::Out
                .signed(Amount::parse("-30").unwrap())
                .is_negative()
        );
    }

    #[test]
    fn bank_transfer_leg_follows_the_direction() {
        let out = Transaction::BankTransfer(bank_transfer("s1", Flow::Out, "100"));
        assert_eq!(out.legs()[0].amount.to_string(), "-100");

        let inn = Transaction::BankTransfer(bank_transfer("s1", Flow::In, "250"));
        assert_eq!(inn.legs()[0].amount.to_string(), "250");
    }

    #[test]
    fn bank_transfer_keeps_its_own_fields_only() {
        let tx = bank_transfer("s1", Flow::Out, "100");
        // Round-trip the whole record: the banking fields are what survives.
        let json = serde_json::to_value(&tx).unwrap();
        assert_eq!(json["rail"], "sepa");
        assert_eq!(json["counterparty_account"], "DE89…");
        assert_eq!(json["flow"], "out");
        let back: BankTransferTx = serde_json::from_value(json).unwrap();
        assert_eq!(back, tx);
    }

    /// A trade is one record with two legs, not two records joined by a
    /// `group_id`.
    #[test]
    fn exchange_trade_is_one_record_with_two_legs() {
        let trade = ExchangeTradeTx {
            meta: TxMeta::new("btc dip"),
            store_id: "binance".into(),
            sold: TradeSide {
                asset: AssetRef::fiat("USDT", "Tether"),
                qty: Amount::parse("1000").unwrap(),
            },
            bought: TradeSide {
                asset: AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin),
                qty: Amount::parse("0.02").unwrap(),
            },
            fee: Some(Amount::parse("1").unwrap()),
            fee_asset: None,
            order_id: Some("ord-1".into()),
            market: Some("spot".into()),
            liquidity: Some("maker".into()),
            price: PriceStamp {
                price_usd: Some(Amount::parse("50000").unwrap()),
                ..Default::default()
            },
        };
        let legs = trade.legs();
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].amount.to_string(), "-1000");
        assert_eq!(legs[1].amount.to_string(), "0.02");
        // Both legs are on one store but different assets, so this is valid.
        assert_eq!(legs[0].store_id, "binance");
        assert_eq!(legs[1].store_id, "binance");
        assert_ne!(legs[0].asset.key(), legs[1].asset.key());
        // Both legs carry the fill price: the sold one is the exit price PnL
        // is measured against, the bought one opens the average cost.
        assert_eq!(legs[0].price_usd.unwrap().to_string(), "50000");
        assert_eq!(legs[1].price_usd.unwrap().to_string(), "50000");
    }

    /// Both legs of a cross-store transfer carry the counterparty, so a store
    /// finds the movement from either side.
    #[test]
    fn transfer_legs_name_both_stores() {
        let transfer = TransferTx {
            meta: TxMeta::new("to cold"),
            from_store_id: "binance".into(),
            to_store_id: "ledger".into(),
            asset: AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin),
            amount: Amount::parse("1").unwrap(),
            fee: None,
            fee_asset: None,
            network: None,
            tx_hash: None,
            price: PriceStamp::default(),
        };
        let legs = transfer.legs();
        assert_eq!(legs[0].amount.to_string(), "-1");
        assert_eq!(legs[0].store_id, "binance");
        assert_eq!(legs[0].to_store_id.as_deref(), Some("ledger"));
        assert_eq!(legs[1].amount.to_string(), "1");
        assert_eq!(legs[1].store_id, "ledger");
        assert_eq!(legs[1].to_store_id.as_deref(), Some("binance"));

        let tx = Transaction::Transfer(transfer);
        assert!(tx.touches("binance"));
        assert!(tx.touches("ledger"));
        assert!(!tx.touches("cash"));
    }

    /// Tag and filter list must not drift apart.
    #[test]
    fn tags_stay_in_sync_with_the_variants() {
        let samples = [
            Transaction::Income(IncomeTx {
                meta: TxMeta::new(""),
                store_id: "s".into(),
                asset: usd(),
                amount: Amount::parse("1").unwrap(),
                source: IncomeSource::Salary,
                price: PriceStamp::default(),
            }),
            Transaction::BankTransfer(bank_transfer("s", Flow::In, "1")),
        ];
        for tx in &samples {
            assert!(Transaction::ALL_TAGS.contains(&tx.tag()));
        }
        assert_eq!(Transaction::ALL_TAGS.len(), 11);
    }

    /// A price must fill in what is missing without clobbering what is there.
    #[test]
    fn stamping_never_overwrites_an_existing_price() {
        let mut tx = Transaction::BankTransfer(bank_transfer("s", Flow::In, "10"));
        tx.stamp_prices(Some(Amount::parse("1.10").unwrap()), None);
        assert_eq!(tx.legs()[0].price_usd.unwrap().to_string(), "1.10");

        tx.stamp_prices(
            Some(Amount::parse("9.99").unwrap()),
            Some(Amount::parse("0.9").unwrap()),
        );
        assert_eq!(tx.legs()[0].price_usd.unwrap().to_string(), "1.10");
        assert_eq!(tx.legs()[0].fx_override.unwrap().to_string(), "0.9");
    }

    /// Every wallet kind must accept the kinds that work anywhere, including
    /// the cross-store transfer.
    #[test]
    fn every_store_kind_accepts_the_common_kinds() {
        for kind in ALL_STORE_KINDS {
            let tags = Transaction::tags_for_store_kind(*kind);
            for common in Transaction::COMMON_TAGS {
                assert!(tags.contains(common), "{kind:?} must accept `{common}`");
            }
            assert!(
                tags.contains(&"transfer"),
                "{kind:?} must accept a cross-store transfer"
            );
        }
    }

    /// Nothing may be offered that the enum does not have, or the UI would show
    /// a kind the backend cannot build.
    #[test]
    fn no_store_kind_offers_an_unknown_kind() {
        for kind in ALL_STORE_KINDS {
            for tag in Transaction::tags_for_store_kind(*kind) {
                assert!(
                    Transaction::ALL_TAGS.contains(&tag),
                    "{kind:?} offers `{tag}`, which is not a transaction kind"
                );
            }
        }
    }

    /// A kind specific to one wallet must not leak onto another: the whole
    /// reason a bank transfer has its own variant is that it only exists there.
    #[test]
    fn banking_kinds_are_confined_to_a_bank() {
        let bank = Transaction::tags_for_store_kind(StoreKind::Bank);
        assert!(bank.contains(&"bank_transfer"));
        assert!(bank.contains(&"card_payment"));
        for kind in [
            StoreKind::Exchange,
            StoreKind::HotWallet,
            StoreKind::ColdWallet,
            StoreKind::Cash,
        ] {
            let tags = Transaction::tags_for_store_kind(kind);
            assert!(
                !tags.contains(&"bank_transfer") && !tags.contains(&"card_payment"),
                "{kind:?} must not offer banking kinds"
            );
        }
    }

    #[test]
    fn defi_is_confined_to_on_chain_wallets() {
        for kind in [StoreKind::HotWallet, StoreKind::ColdWallet] {
            assert!(Transaction::tags_for_store_kind(kind).contains(&"defi_action"));
        }
        for kind in [StoreKind::Exchange, StoreKind::Bank, StoreKind::Cash] {
            assert!(
                !Transaction::tags_for_store_kind(kind).contains(&"defi_action"),
                "{kind:?} must not offer DeFi"
            );
        }
    }

    #[test]
    fn cash_only_offers_cash() {
        let tags = Transaction::tags_for_store_kind(StoreKind::Cash);
        assert!(tags.contains(&"cash_flow"));
        for specific in ["exchange_trade", "bank_transfer", "defi_action"] {
            assert!(
                !tags.contains(&specific),
                "cash must not offer `{specific}`"
            );
        }
    }

    /// No duplicates, and common kinds lead the list.
    #[test]
    fn the_offered_list_is_ordered_and_deduplicated() {
        for kind in ALL_STORE_KINDS {
            let tags = Transaction::tags_for_store_kind(*kind);
            let mut unique = tags.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), tags.len(), "{kind:?} offers a kind twice");

            let common_len = Transaction::COMMON_TAGS.len();
            assert_eq!(
                &tags[..common_len],
                Transaction::COMMON_TAGS,
                "{kind:?} must lead with the common kinds"
            );
        }
    }

    /// The pre-selected kind has to be one the wallet actually accepts, or the
    /// form would open on something it cannot submit.
    #[test]
    fn the_default_kind_is_always_offered() {
        for kind in ALL_STORE_KINDS {
            let default = Transaction::default_tag_for_store_kind(*kind);
            assert!(
                Transaction::tags_for_store_kind(*kind).contains(&default),
                "{kind:?} defaults to `{default}`, which it does not offer"
            );
        }
        assert_eq!(
            Transaction::default_tag_for_store_kind(StoreKind::Exchange),
            "exchange_trade"
        );
        assert_eq!(
            Transaction::default_tag_for_store_kind(StoreKind::Bank),
            "bank_transfer"
        );
        assert_eq!(
            Transaction::default_tag_for_store_kind(StoreKind::Cash),
            "cash_flow"
        );
        assert_eq!(
            Transaction::default_tag_for_store_kind(StoreKind::HotWallet),
            "wallet_transfer"
        );
    }

    /// A variant nobody can create is dead weight, and a wallet with no
    /// specific kinds has nothing to pre-select.
    ///
    /// (Adding a `StoreKind` is caught at compile time instead: the `match` in
    /// `specific_tags_for_store_kind` has no wildcard arm.)
    #[test]
    fn every_variant_is_reachable_from_some_wallet() {
        let offered: Vec<&str> = ALL_STORE_KINDS
            .iter()
            .flat_map(|kind| {
                Transaction::specific_tags_for_store_kind(*kind)
                    .iter()
                    .copied()
            })
            .collect();
        for tag in Transaction::ALL_TAGS {
            if Transaction::COMMON_TAGS.contains(tag) {
                continue;
            }
            assert!(
                offered.contains(tag),
                "no wallet kind offers `{tag}`, so it cannot be created"
            );
        }
        for kind in ALL_STORE_KINDS {
            assert!(
                !Transaction::specific_tags_for_store_kind(*kind).is_empty(),
                "{kind:?} has no store-specific kinds to pre-select"
            );
        }
    }

    /// The enum's wire shape: externally tagged, so bincode can round-trip it
    /// and TypeScript gets a discriminated union.
    #[test]
    fn the_enum_is_externally_tagged() {
        let tx = Transaction::BankTransfer(bank_transfer("s", Flow::Out, "5"));
        let json = serde_json::to_value(&tx).unwrap();
        assert!(json.get("bank_transfer").is_some(), "{json}");
        let back: Transaction = serde_json::from_value(json).unwrap();
        assert_eq!(back, tx);
    }
}
