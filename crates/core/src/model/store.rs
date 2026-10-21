use serde::{Deserialize, Serialize};

use crate::model::chain::Chain;
use schemars::JsonSchema;

/// What kind of container a store represents. The sidebar groups wallets by
/// it, and it decides which sub-accounts, settings and automations apply.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StoreKind {
    /// The single cash wallet, created with the vault.
    #[default]
    Cash,
    Bank,
    /// A self-custody crypto wallet (hot or cold, see [`Custody`]).
    Crypto,
    /// CEX (e.g. Bybit). Carries the exchange profile and its terminal.
    Exchange,
    /// Non-financial property valued by hand: a flat, a car, a collection.
    Property,
    /// A bonus or loyalty programme: points with a value per point.
    Loyalty,
    /// Money owed: a mortgage, a loan. Its balance is negative.
    Liability,
}

impl StoreKind {
    /// Sidebar order.
    pub const ALL: [StoreKind; 7] = [
        Self::Cash,
        Self::Bank,
        Self::Crypto,
        Self::Exchange,
        Self::Property,
        Self::Loyalty,
        Self::Liability,
    ];

    /// How many top-level wallets of this kind may exist.
    pub fn max_roots(self) -> Option<usize> {
        match self {
            Self::Cash => Some(1),
            _ => None,
        }
    }

    /// Whether the user creates wallets of this kind (cash comes with the vault).
    pub fn user_creatable(self) -> bool {
        !matches!(self, Self::Cash)
    }

    /// Sub-account types a new sub-account of this kind is offered as.
    pub fn sub_types(self) -> &'static [SubType] {
        use SubType::*;
        match self {
            Self::Cash => &[Pocket],
            // A card is a bank account: one type for both.
            Self::Bank => &[Card, Deposit, Savings, Pds, CreditCard],
            Self::Crypto => &[Token],
            // Bybit's own split: Funding, Unified Trading (spot and
            // derivatives together), Earn.
            Self::Exchange => &[Funding, Unified, Earn],
            Self::Property | Self::Loyalty | Self::Liability => &[],
        }
    }

    /// Every sub-account type a wallet of this kind may hold: what it offers
    /// plus the older types it used to offer (a separate account next to the
    /// card, spot and futures next to the unified account, a pair).
    pub fn admits_sub_type(self, sub_type: SubType) -> bool {
        use SubType::*;
        self.sub_types().contains(&sub_type)
            || matches!(
                (self, sub_type),
                (Self::Bank, Account) | (Self::Exchange, Spot | Futures | Pair)
            )
    }

    /// Ready-made automations a wallet of this kind offers.
    pub fn automation_presets(self) -> &'static [crate::model::planning::AutomationPreset] {
        use crate::model::planning::AutomationPreset::*;
        match self {
            Self::Cash => &[Expense, TopUp],
            Self::Bank => &[Salary, CardPayment, ToSavings, DepositInterest],
            Self::Crypto => &[DcaBuy, StakingReward],
            Self::Exchange => &[DcaBuy, MarketTransfer],
            Self::Property => &[RentIncome, PropertyTax, Insurance, Utilities],
            Self::Loyalty => &[],
            Self::Liability => &[LoanPayment],
        }
    }

    /// What the money in a top-level wallet of this kind is for by default.
    pub fn default_purpose(self) -> Purpose {
        match self {
            Self::Cash | Self::Bank => Purpose::Everyday,
            // Points are kept, not spent from the daily budget.
            Self::Loyalty => Purpose::Savings,
            Self::Crypto | Self::Exchange | Self::Property | Self::Liability => Purpose::Investment,
        }
    }
}

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Custody {
    #[default]
    Hot,
    Cold,
}

/// A sub-account template. Which ones a wallet offers depends on its kind,
/// see [`StoreKind::sub_types`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubType {
    /// Cash in one currency.
    Pocket,
    Card,
    Account,
    Deposit,
    Savings,
    /// A credit card: carries [`LiabilityTerms`], may go negative.
    CreditCard,
    /// One token on one network.
    Token,
    /// Программа долгосрочных сбережений: savings with state co-financing.
    Pds,
    Spot,
    Futures,
    Earn,
    /// A trading pair such as BTCUSDT: holds both sides, converts both ways.
    Pair,
    /// An exchange's funding account (Bybit «Финансовый счёт»): deposits,
    /// withdrawals, P2P, transfers between accounts — coins only.
    Funding,
    /// A unified trading account (Bybit UTA): spot and derivatives together.
    Unified,
}

impl SubType {
    /// Which binding shape this template requires.
    pub fn binding_shape(self) -> BindingShape {
        match self {
            Self::Spot => BindingShape::Market(MarketKind::Spot),
            Self::Futures => BindingShape::Market(MarketKind::Futures),
            Self::Earn => BindingShape::Market(MarketKind::Earn),
            Self::Funding => BindingShape::Market(MarketKind::Funding),
            Self::Unified => BindingShape::Market(MarketKind::Unified),
            Self::Pair => BindingShape::Pair,
            _ => BindingShape::Asset,
        }
    }

    pub fn default_purpose(self) -> Option<Purpose> {
        match self {
            Self::Deposit | Self::Savings | Self::Pds => Some(Purpose::Savings),
            Self::Card | Self::CreditCard | Self::Account | Self::Pocket => Some(Purpose::Everyday),
            _ => None,
        }
    }

    pub fn is_credit(self) -> bool {
        matches!(self, Self::CreditCard)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketKind {
    Spot,
    Futures,
    Earn,
    Funding,
    Unified,
}

/// The binding shape a sub-account type expects (see [`Binding`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingShape {
    Asset,
    Pair,
    Market(MarketKind),
}

/// What a sub-account is locked to. A top-level wallet is always `Any`.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    #[default]
    Any,
    /// Exactly one currency or token.
    Asset(AssetRef),
    /// Both sides of a pair, in either direction.
    Pair {
        base: AssetRef,
        quote: AssetRef,
    },
    Market(MarketKind),
}

impl Binding {
    pub fn shape(&self) -> Option<BindingShape> {
        match self {
            Self::Any => None,
            Self::Asset(_) => Some(BindingShape::Asset),
            Self::Pair { .. } => Some(BindingShape::Pair),
            Self::Market(market) => Some(BindingShape::Market(*market)),
        }
    }

    /// Whether `asset` may sit in a store with this binding.
    pub fn admits(&self, asset: &AssetRef) -> bool {
        match self {
            Self::Any => true,
            Self::Asset(bound) => same_asset(bound, asset),
            Self::Pair { base, quote } => same_asset(base, asset) || same_asset(quote, asset),
            Self::Market(MarketKind::Spot | MarketKind::Earn | MarketKind::Funding) => {
                !asset.is_derivative()
            }
            // Margin can be any coin (inverse contracts), positions are perps;
            // a unified account holds coins and positions side by side.
            Self::Market(MarketKind::Futures | MarketKind::Unified) => true,
        }
    }
}

/// Same asset for binding purposes: the cache key when both sides have one
/// that identifies them, else the ticker on the same network.
fn same_asset(left: &AssetRef, right: &AssetRef) -> bool {
    if left.key() == right.key() {
        return true;
    }
    left.symbol.eq_ignore_ascii_case(&right.symbol)
        && (left.chain.is_none() || right.chain.is_none() || left.chain == right.chain)
        && !left.is_derivative()
        && !right.is_derivative()
}

/// What the money in a wallet is for. Replaces the old «count in budget»
/// switch: only everyday money is in the daily budget.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Money to live on: its spending and income are in the daily budget.
    #[default]
    Everyday,
    /// Put aside: moving money here is saving, not spending.
    Savings,
    /// Trading and price swings never touch the budget; counted in net worth.
    Investment,
    /// Not the user's money (a tax reserve, a friend's cash): neither in the
    /// budget nor in net worth.
    Transit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LiabilityKind {
    Mortgage,
    Loan,
    CreditCard,
    Installment,
    Other,
}

/// Terms of a `liability` store or a credit card. Informational, except
/// `limit`, which caps what an automation may charge to a credit card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct LiabilityTerms {
    pub kind: LiabilityKind,
    /// Annual interest rate in percent.
    pub rate: Option<crate::Amount>,
    /// Credit limit, in the store's currency.
    pub limit: Option<crate::Amount>,
    /// Day of the month the mandatory payment is due (1–31).
    pub due_day: Option<u8>,
    pub monthly_payment: Option<crate::Amount>,
}

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PropertyCategory {
    #[default]
    Flat,
    House,
    Car,
    Land,
    Other,
}

/// A bonus programme: its points and what one point is worth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct LoyaltyInfo {
    /// Ticker of the points, unique among the user's programmes (`СПАСИБО`, `МИЛИ`).
    pub points_symbol: String,
    /// What one point is worth in `currency`.
    pub point_value: crate::Amount,
    pub currency: String,
    /// `YYYY-MM-DD` the next batch of points burns, if the programme has one.
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub expiring_points: Option<crate::Amount>,
    /// What the programme knows the user by: a phone, a card number, an account.
    #[serde(default)]
    pub login: Option<String>,
    /// Cards whose purchases earn the points; the first is the default source.
    #[serde(default)]
    pub source_store_ids: Vec<String>,
}

impl LoyaltyInfo {
    /// The asset the programme's points are booked as.
    pub fn asset(&self, programme: &str) -> AssetRef {
        let symbol = self.points_symbol.trim().to_uppercase();
        AssetRef {
            symbol: symbol.clone(),
            name: programme.to_string(),
            coingecko_id: None,
            chain: None,
            contract: None,
            quote: symbol,
        }
    }
}

/// What a property is. Its value comes from manual prices; expenses
/// (utilities) point at it through `UtilityItem.property_id`.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PropertyInfo {
    pub category: PropertyCategory,
    pub address: String,
    /// Square metres, informational.
    pub area: Option<crate::Amount>,
    /// YYYY-MM-DD.
    pub purchased_at: Option<String>,
    pub purchase_price: Option<crate::Amount>,
    pub currency: String,
    /// Day of the month the utilities are usually paid (1–31), all at once.
    #[serde(default)]
    #[ts(optional)]
    pub utilities_pay_day: Option<u8>,
}

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeProvider {
    #[default]
    Bybit,
    Binance,
    Okx,
}

impl ExchangeProvider {
    /// Whether the app can talk to this exchange yet (others show as «soon»).
    pub fn supported(self) -> bool {
        matches!(self, Self::Bybit)
    }
}

/// One indicator on the terminal chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct IndicatorCfg {
    /// e.g. `ema`, `sma`, `rsi`.
    pub kind: String,
    pub period: u32,
}

/// Terminal panels' layout, as the UI left it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TerminalLayout {
    pub show_watchlist: bool,
    /// Watchlist width in pixels.
    pub watchlist_width: u32,
}

impl Default for TerminalLayout {
    fn default() -> Self {
        Self {
            show_watchlist: true,
            watchlist_width: 240,
        }
    }
}

/// Everything the trading terminal of one exchange remembers. It lives in the
/// exchange wallet, so two exchanges keep two independent terminals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TerminalState {
    pub watchlist: Vec<String>,
    pub active_symbol: Option<String>,
    pub timeframe: String,
    pub indicators: Vec<IndicatorCfg>,
    /// Symbol → the drawing manager's JSON export.
    pub drawings: std::collections::BTreeMap<String, String>,
    pub layout: TerminalLayout,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            watchlist: vec!["BTCUSDT".into(), "ETHUSDT".into()],
            active_symbol: None,
            timeframe: "1h".into(),
            indicators: Vec::new(),
            drawings: std::collections::BTreeMap::new(),
            layout: TerminalLayout::default(),
        }
    }
}

/// Upper bound on one symbol's drawings JSON; the vault is re-encrypted on
/// every save, so a runaway chart must not bloat it.
pub const MAX_DRAWINGS_JSON: usize = 256 * 1024;

/// The exchange a wallet stands for, plus its terminal. Keys are kept apart in
/// `VaultDoc.exchange_keys` so secrets never travel with the store.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ExchangeProfile {
    pub provider: ExchangeProvider,
    /// Background import interval; 0 = off.
    pub import_interval_sec: u32,
    #[serde(default)]
    pub terminal: TerminalState,
    /// What an import brings in and where it books it.
    #[serde(default)]
    pub import: ImportSettings,
}

fn yes() -> bool {
    true
}

/// What an exchange import brings in and where it books it: set up after the
/// key is saved. The defaults are what the import did before these settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ImportSettings {
    /// Spot and futures fills.
    #[serde(default = "yes")]
    pub trades: bool,
    /// Moves between the exchange's own accounts.
    #[serde(default = "yes")]
    pub transfers: bool,
    /// Earn payouts.
    #[serde(default = "yes")]
    pub earn: bool,
    /// Book new operations at once instead of staging them for confirmation.
    #[serde(default)]
    pub auto_confirm: bool,
    /// When the ledger and the exchange disagree after everything is booked,
    /// a correction sets the exchange's live balance.
    #[serde(default = "yes")]
    pub auto_reconcile: bool,
    /// Only these coins (upper case); empty = every coin.
    #[serde(default)]
    pub coins: Vec<String>,
    /// Nothing older than this moment (RFC3339) is imported.
    #[serde(default)]
    pub since: Option<String>,
    /// Category of each kind of operation; `None` = the built-in one.
    #[serde(default)]
    pub categories: ImportCategories,
    /// Where each exchange account books; an account not listed goes to the
    /// sub-account bound to it (as before).
    #[serde(default)]
    pub accounts: Vec<ImportAccountRoute>,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            trades: true,
            transfers: true,
            earn: true,
            auto_confirm: false,
            auto_reconcile: true,
            coins: Vec::new(),
            since: None,
            categories: ImportCategories::default(),
            accounts: Vec::new(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ImportCategories {
    #[serde(default)]
    pub trades: Option<String>,
    #[serde(default)]
    pub transfers: Option<String>,
    #[serde(default)]
    pub earn: Option<String>,
}

/// One exchange account (Bybit `FUND`, `UNIFIED`, `EARN`) and the wallet it
/// books in; `store_id: None` = this account is not imported at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ImportAccountRoute {
    pub account: String,
    #[serde(default)]
    pub store_id: Option<String>,
}

/// Deepest allowed node depth; a root is depth 0, so the tree has two levels:
/// wallet → sub-account.
pub const MAX_STORE_DEPTH: usize = 1;

/// A "store"/"platform" (хранилище) — a portfolio container.
/// Important: a store is NOT an asset — it holds holdings (`Holding`).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Store {
    pub id: String,
    pub name: String,
    pub kind: StoreKind,
    /// CSS-friendly accent color for the UI (hex, e.g. `#0ea5e9`).
    pub color: String,
    pub archived: bool,
    /// Free-form description shown in the UI.
    pub note: String,
    /// Parent store ID for sub-accounts; `None` for a top-level wallet. A
    /// sub-account has its parent's kind, see `MAX_STORE_DEPTH`.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Sub-account template; `None` on a top-level wallet.
    #[serde(default)]
    pub sub_type: Option<SubType>,
    /// What a sub-account may hold; `Any` on a top-level wallet.
    #[serde(default)]
    pub binding: Binding,
    #[serde(default)]
    pub purpose: Purpose,
    /// Saved contact this wallet belongs to (an exchanger, a bank). Top-level only.
    pub contact_id: Option<String>,
    /// Loan terms; on `liability` stores and credit cards.
    pub liability: Option<LiabilityTerms>,
    /// Crypto wallets only.
    #[serde(default)]
    pub custody: Option<Custody>,
    /// Property wallets only.
    #[serde(default)]
    pub property: Option<PropertyInfo>,
    /// A loan secured by a property (mortgage → flat).
    #[serde(default)]
    pub secured_by: Option<String>,
    /// Exchange wallets only (top level).
    #[serde(default)]
    pub exchange: Option<ExchangeProfile>,
    /// Loyalty wallets: the programme's points and what a point is worth.
    #[serde(default)]
    #[ts(optional)]
    pub loyalty: Option<LoyaltyInfo>,
    /// Savings accounts, term deposits and ПДС: rate history and terms.
    #[serde(default)]
    #[ts(optional)]
    pub deposit: Option<crate::model::deposit::DepositTerms>,
    /// Portfolio basket of everything held here unless an asset has its own (plan F).
    #[serde(default)]
    #[ts(optional)]
    pub asset_class: Option<String>,
}

impl Store {
    pub fn new(name: &str, kind: StoreKind, color: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            kind,
            color: color.to_string(),
            purpose: kind.default_purpose(),
            custody: (kind == StoreKind::Crypto).then_some(Custody::Hot),
            ..Self::default()
        }
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Whether this store's income and spending count in the daily budget.
    /// A loan never does: paying it off is a cost, its balance is not money.
    pub fn in_budget(&self) -> bool {
        self.purpose == Purpose::Everyday && self.kind != StoreKind::Liability
    }

    /// Whether the balance may go below zero by design.
    pub fn is_debt(&self) -> bool {
        self.kind == StoreKind::Liability || self.liability.is_some()
    }
}

/// Which stores a query covers. The one way every report filters by wallet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct StoreScope {
    /// Empty = every store.
    pub store_ids: Vec<String>,
    /// Expand each id to its whole subtree.
    pub include_children: bool,
}

impl Default for StoreScope {
    fn default() -> Self {
        Self {
            store_ids: Vec::new(),
            include_children: true,
        }
    }
}

impl StoreScope {
    pub fn store(id: &str, include_children: bool) -> Self {
        Self {
            store_ids: vec![id.to_string()],
            include_children,
        }
    }
}

/// A position of one asset inside one store (хранилище !== актив).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Holding {
    pub id: String,
    pub store_id: String,
    pub asset: AssetRef,
    pub quantity: crate::Amount,
    /// Optional manual average cost basis per unit (in base currency).
    pub avg_cost: Option<crate::Amount>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AssetRef {
    pub symbol: String,
    pub name: String,
    /// e.g. `bitcoin` — used by CoinGecko.
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    /// Token contract address (for tokens on EVM/Solana-like networks).
    pub contract: Option<String>,
    /// `base` quote currency symbol for FX peers (e.g. `usd`, `eur`).
    /// For crypto this is the network quote (e.g. `btc`, `eth`).
    /// Kept authoritative for display only.
    pub quote: String,
}

impl AssetRef {
    pub fn crypto(symbol: &str, name: &str, coingecko_id: Option<&str>, chain: Chain) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            name: name.to_string(),
            coingecko_id: coingecko_id.map(|id| id.to_string()),
            chain: Some(chain),
            contract: None,
            quote: symbol.to_uppercase(),
        }
    }

    pub fn fiat(symbol: &str, name: &str) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            name: name.to_string(),
            coingecko_id: None,
            chain: Some(Chain::Fiat),
            contract: None,
            quote: symbol.to_uppercase(),
        }
    }

    pub fn is_fiat(&self) -> bool {
        self.chain == Some(Chain::Fiat)
    }

    pub fn is_crypto(&self) -> bool {
        !self.is_fiat()
    }

    /// A perpetual futures contract. It is held like an asset (the open
    /// position), but its notional is not the user's money, so it never counts
    /// in portfolio value.
    pub fn perp(exchange: &str, symbol: &str) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            name: symbol.to_uppercase(),
            coingecko_id: None,
            chain: None,
            contract: Some(format!("{PERP_PREFIX}{}", exchange.to_lowercase())),
            quote: symbol.to_uppercase(),
        }
    }

    /// The same asset as `other`: one cache key, or one ticker on the same (or
    /// an unnamed) network. A derivative is never the same as its coin.
    pub fn same_as(&self, other: &AssetRef) -> bool {
        same_asset(self, other)
    }

    pub fn is_derivative(&self) -> bool {
        self.contract
            .as_deref()
            .is_some_and(|contract| contract.starts_with(PERP_PREFIX))
    }

    /// Stable key used for price cache lookups.
    pub fn key(&self) -> String {
        if self.is_derivative() {
            // perp:{exchange}:{symbol}
            return format!(
                "{}:{}",
                self.contract.as_deref().unwrap_or_default(),
                self.symbol
            );
        }
        match (&self.coingecko_id, &self.chain, &self.contract) {
            (Some(_), Some(chain), Some(contract)) => {
                format!("token:{}:{}:{}", chain_key(chain), self.symbol, contract)
            }
            (Some(id), _, _) => format!("coin:{id}"),
            (None, Some(chain), Some(contract)) => {
                format!("token:{}:{}:{}", chain_key(chain), self.symbol, contract)
            }
            _ => format!(
                "{}:{}",
                if self.is_fiat() { "fiat" } else { "asset" },
                self.symbol
            ),
        }
    }
}

const PERP_PREFIX: &str = "perp:";

fn chain_key(chain: &Chain) -> &'static str {
    match chain {
        Chain::Fiat => "fiat",
        _ => chain.label(),
    }
}
