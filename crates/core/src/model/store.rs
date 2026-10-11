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
    /// Money owed: a mortgage, a loan. Its balance is negative.
    Liability,
}

impl StoreKind {
    /// Sidebar order.
    pub const ALL: [StoreKind; 6] = [
        Self::Cash,
        Self::Bank,
        Self::Crypto,
        Self::Exchange,
        Self::Property,
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

    /// Sub-account types a wallet of this kind offers.
    pub fn sub_types(self) -> &'static [SubType] {
        use SubType::*;
        match self {
            Self::Cash => &[Pocket],
            Self::Bank => &[Card, Account, Deposit, Savings, CreditCard],
            Self::Crypto => &[Token],
            Self::Exchange => &[Spot, Futures, Earn, Pair],
            Self::Property | Self::Liability => &[],
        }
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
            Self::Liability => &[LoanPayment],
        }
    }

    /// What the money in a top-level wallet of this kind is for by default.
    pub fn default_purpose(self) -> Purpose {
        match self {
            Self::Cash | Self::Bank => Purpose::Everyday,
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
    Spot,
    Futures,
    Earn,
    /// A trading pair such as BTCUSDT: holds both sides, converts both ways.
    Pair,
}

impl SubType {
    /// Which binding shape this template requires.
    pub fn binding_shape(self) -> BindingShape {
        match self {
            Self::Spot => BindingShape::Market(MarketKind::Spot),
            Self::Futures => BindingShape::Market(MarketKind::Futures),
            Self::Earn => BindingShape::Market(MarketKind::Earn),
            Self::Pair => BindingShape::Pair,
            _ => BindingShape::Asset,
        }
    }

    pub fn default_purpose(self) -> Option<Purpose> {
        match self {
            Self::Deposit | Self::Savings => Some(Purpose::Savings),
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
            Self::Market(MarketKind::Spot | MarketKind::Earn) => !asset.is_derivative(),
            // Margin can be any coin (inverse contracts), positions are perps.
            Self::Market(MarketKind::Futures) => true,
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
