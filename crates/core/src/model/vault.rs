use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::category::{Category, default_categories};
use crate::model::chain::ChainInfo;
use crate::model::contact::Contact;
use crate::model::enrichment::EnrichmentRecord;
use crate::model::planning::{
    Budget, CategorizationRule, Goal, MeterReading, PriceAlert, Reminder, UtilityItem,
};
use crate::model::store::{AssetRef, Holding, Store};
use crate::model::transaction::Transaction;
use schemars::JsonSchema;

/// One widget on the dashboard grid (12 columns). A widget not in the list is
/// not on the dashboard; the grid compacts upwards, so `y` is only an order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DashboardWidgetCfg {
    pub id: String,
    /// Left column, 0-based.
    pub col: u32,
    /// Top row; the grid compacts upwards, so it only orders.
    pub row: u32,
    /// Width in columns.
    pub width: u32,
    /// Height in rows.
    pub height: u32,
    /// Header hidden: the widget is locked in place and not resizable.
    #[serde(default)]
    pub pinned: bool,
    /// A widget about one wallet (an exchange's chart) remembers which.
    #[serde(default)]
    pub store_id: Option<String>,
}

impl Default for DashboardWidgetCfg {
    fn default() -> Self {
        Self {
            id: "total".into(),
            col: 0,
            row: 0,
            width: 4,
            height: 4,
            pinned: false,
            store_id: None,
        }
    }
}

pub(crate) fn default_fx_interval() -> u32 {
    300
}

pub(crate) fn default_minimize_to_tray() -> bool {
    true
}

/// Vault-scoped settings.
///
/// The preferences that describe the *user* rather than this particular vault
/// (theme, language, base currency, favourite currencies) live outside the
/// encrypted doc — see [`crate::appconfig::AppSettings`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Settings {
    /// Decimal places used to render balances.
    pub display_decimals: u32,
    #[serde(default)]
    pub dashboard_widgets: Vec<DashboardWidgetCfg>,
    #[serde(default = "default_fx_interval")]
    pub fx_refresh_interval_sec: u32,
    #[serde(default = "default_minimize_to_tray")]
    pub minimize_to_tray: bool,
    /// Share of income planned for savings, 0..1 (plan D16).
    pub savings_rate: Amount,
    pub daily_budget_mode: DailyBudgetMode,
    /// Currency of budgets and month totals; empty = the base currency.
    pub budget_currency: String,
    /// What to be told about and how far ahead.
    #[serde(default)]
    pub notifications: crate::model::notify::NotificationSettings,
    /// The budget settings were saved at least once: the checklist step
    /// «доля сбережений» is done even when 10 % was the right answer.
    #[serde(default)]
    pub budget_configured: bool,
    /// Checklist steps the user said they do not need.
    #[serde(default)]
    pub onboarding_skipped: Vec<String>,
    /// Where each currency's inflation comes from; a currency missing here
    /// reads as [`crate::service::inflation`] decides.
    #[serde(default)]
    pub inflation_mode: std::collections::BTreeMap<String, crate::model::inflation::InflationMode>,
    /// The data source of each currency in `InflationMode::Source`.
    #[serde(default)]
    pub inflation_source: std::collections::BTreeMap<String, String>,
    /// The user's usual monthly income in the budget currency, used until
    /// three months of history exist (plan B).
    #[serde(default)]
    pub manual_income: Option<Amount>,
    /// The user's usual monthly spending, same rule.
    #[serde(default)]
    pub manual_expenses: Option<Amount>,
    /// The last batch of built-in categories this vault has received
    /// ([`crate::model::category::CATEGORY_ADDITIONS`]): each batch is added
    /// once, so a category the user deleted does not come back.
    #[serde(default)]
    pub categories_rev: u32,
    /// Yearly return the freedom index potential assumes, percent; `None`
    /// takes the key rate (plan C3).
    #[serde(default)]
    pub freedom_rate: Option<Amount>,
    /// The sidebar's «Pinned» block, in its order (plan J2).
    #[serde(default)]
    pub pinned: Vec<crate::model::entity_ref::Pinned>,
}

/// How the daily budget is computed (plan D18).
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DailyBudgetMode {
    /// Month's free money spread over its days; what is left carries over.
    #[default]
    Month,
    /// Free balance spread over the days until the next income.
    UntilIncome,
}

/// A hand-set value of a non-financial asset; applies to its whole history
/// (plan D24), so revaluing a flat does not make the net-worth chart jump.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ManualPrice {
    /// `AssetRef::key()` of the asset.
    pub asset_key: String,
    pub unit_price: Amount,
    /// Lowercase currency code of `unit_price`.
    pub currency: String,
    pub updated_at: String,
}

/// A dated value of one unit of a property asset (plan G3). Between two
/// valuations the price runs in a straight line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Valuation {
    pub id: String,
    pub store_id: String,
    /// `AssetRef::key()` of the asset.
    pub asset_key: String,
    /// YYYY-MM-DD.
    pub date: String,
    pub unit_price: Amount,
    /// Lowercase currency code of `unit_price`.
    pub currency: String,
}

/// An operation that belongs to a property (plan G4): rent received for a
/// flat, fuel or insurance for a car. Kept beside the operations, not in
/// them, so the operation format the older files store by position stays put.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TxProperty {
    pub tx_id: String,
    /// Id of a `StoreKind::Property` store.
    pub property_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display_decimals: 8,
            dashboard_widgets: vec![],
            fx_refresh_interval_sec: 300,
            minimize_to_tray: true,
            savings_rate: Amount::parse("0.10").unwrap_or(Amount::ZERO),
            daily_budget_mode: DailyBudgetMode::Month,
            budget_currency: String::new(),
            notifications: crate::model::notify::NotificationSettings::default(),
            budget_configured: false,
            onboarding_skipped: Vec::new(),
            inflation_mode: std::collections::BTreeMap::new(),
            inflation_source: std::collections::BTreeMap::new(),
            manual_income: None,
            manual_expenses: None,
            categories_rev: crate::model::category::CATEGORIES_REV,
            freedom_rate: None,
            pinned: Vec::new(),
        }
    }
}

/// A cached price: one unit of an asset in USD.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PricePoint {
    /// `AssetRef::key()`.
    pub asset_key: String,
    pub value_usd: Amount,
    /// ISO-8601 UTC.
    pub ts: String,
    /// The provider that quoted it (`coingecko`, `coincap`, `binance`…).
    pub source: String,
    /// Change over the last 24 hours, percent, when the provider said it.
    #[serde(default)]
    pub change_24h_pct: Option<Amount>,
}

/// A cached FX rate: units of `currency` per 1 USD.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct FxPoint {
    pub currency: String,
    pub per_usd: Amount,
    pub ts: String,
    pub source: String,
}

/// Read-only exchange API credentials.
///
/// This is the only secret stored in the vault, so it gets extra care:
/// - `Debug` is hand-written to redact the secret and mask the key, so it can
///   never leak through a `{:?}` in a log or an error message.
/// - both fields are zeroized when the value is dropped.
/// - `ts(skip)` on the `VaultDoc` field keeps it out of the generated
///   TypeScript bindings entirely.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeKeys {
    pub store_id: String,
    pub api_key: String,
    pub api_secret: String,
    /// Sub-account spot fills are booked on; `None` = `store_id`.
    pub spot_store_id: Option<String>,
    /// Sub-account futures fills are booked on; `None` = `store_id`.
    pub futures_store_id: Option<String>,
}

impl ExchangeKeys {
    /// Short, non-secret identifier safe to show in the UI (e.g. `abcd…wxyz`).
    pub fn hint(&self) -> String {
        let api_key = self.api_key.as_str();
        if api_key.len() <= 8 {
            return "*".repeat(api_key.len());
        }
        let head: String = api_key.chars().take(4).collect();
        let tail: String = api_key.chars().skip(api_key.chars().count() - 4).collect();
        format!("{head}…{tail}")
    }
}

impl std::fmt::Debug for ExchangeKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeKeys")
            .field("store_id", &self.store_id)
            .field("api_key", &self.hint())
            .field("api_secret", &"<redacted>")
            .finish()
    }
}

impl Drop for ExchangeKeys {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.store_id.zeroize();
        self.api_key.zeroize();
        self.api_secret.zeroize();
    }
}

/// What an import stages.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// A trade fill; becomes a spot or futures trade.
    #[default]
    Fill,
    /// A move between the exchange's own accounts; becomes a transfer
    /// between the matching sub-accounts.
    Transfer,
    /// A payout of an Earn product; becomes income on the Earn sub-account.
    EarnIncome,
}

/// A staged Bybit fill awaiting user confirmation before becoming a trade.
///
/// Fills land here first and never touch `transactions` until the user
/// confirms them, so a background import can never silently rewrite history.
/// Deduplication key is Bybit's `execId`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PendingImport {
    /// Bybit's execution ID (`execId`) — the deduplication key.
    pub exec_id: String,
    /// Store the fill belongs to.
    pub store_id: String,
    /// Product category: `spot` | `linear` | `inverse`.
    pub category: String,
    /// Pair symbol as Bybit reports it (e.g. `BTCUSDT`).
    pub symbol: String,
    /// `"buy"` or `"sell"`.
    pub side: String,
    /// Asset received.
    pub asset_bought: String,
    pub qty_bought: Amount,
    /// Asset given up.
    pub asset_sold: String,
    pub qty_sold: Amount,
    /// Fill price of one unit of the base asset.
    pub price: Amount,
    pub fee: Amount,
    /// Fee denom; empty when Bybit reported none.
    pub fee_asset: String,
    /// ISO-8601 UTC.
    pub occurred_at: String,
    /// Provenance note shown in the staging UI.
    pub comment: String,
    pub is_maker: Option<bool>,
    /// Size this fill closed, for futures; `Some(>0)` means it reduced a position.
    pub closed_size: Option<Amount>,
    /// Realized PnL of a closing futures fill. Bybit's execution list does not
    /// carry it, so the import leaves it empty; the user can fill it in.
    pub closed_pnl: Option<Amount>,
    #[serde(default)]
    pub kind: ImportKind,
    /// Transfer: Bybit account types, e.g. `FUND` → `UNIFIED`.
    #[serde(default)]
    pub from_account: Option<String>,
    #[serde(default)]
    pub to_account: Option<String>,
}

impl PendingImport {
    /// Shape a Bybit fill for staging. Returns `None` for fills we cannot make
    /// sense of (no execution id, or an unparseable quantity/price) — a garbled
    /// fill must never reach the user's transactions.
    pub fn from_execution(
        store_id: &str,
        category: &str,
        exec: &crate::exchange::Execution,
    ) -> Option<Self> {
        let exec_id = exec.exec_id.trim();
        if exec_id.is_empty() {
            return None;
        }
        let prefill = exec.to_prefill();
        // Both legs must be fully known: a trade with an unknown quote leg
        // cannot be turned into balanced holdings.
        let asset_bought = nonempty(prefill.asset_bought?);
        let asset_sold = nonempty(prefill.asset_sold?);
        let qty_bought = Amount::parse(&nonempty(prefill.qty_bought?)).ok()?;
        let qty_sold = Amount::parse(&nonempty(prefill.qty_sold?)).ok()?;
        let price = Amount::parse(&nonempty(prefill.price?)).ok()?;
        if !qty_bought.is_positive() || !qty_sold.is_positive() || !price.is_positive() {
            return None;
        }
        Some(Self {
            exec_id: exec_id.to_string(),
            store_id: store_id.to_string(),
            category: category.to_string(),
            symbol: nonempty(prefill.symbol),
            side: nonempty(prefill.side),
            asset_bought,
            qty_bought,
            asset_sold,
            qty_sold,
            price,
            fee: prefill
                .fee
                .as_deref()
                .and_then(|amount| Amount::parse(amount).ok())
                .unwrap_or(Amount::ZERO),
            fee_asset: prefill.fee_asset.map(nonempty).unwrap_or_default(),
            occurred_at: nonempty(prefill.occurred_at.unwrap_or_default()),
            comment: prefill.comment.unwrap_or_default(),
            is_maker: Some(exec.is_maker),
            closed_size: exec
                .closed_size
                .as_deref()
                .and_then(|raw| Amount::parse(raw).ok())
                .filter(|size| size.is_positive()),
            closed_pnl: None,
            kind: ImportKind::Fill,
            from_account: None,
            to_account: None,
        })
    }

    /// Stage a move between the user's own Bybit accounts. The amount sits on
    /// both sides (`*_sold` leaves `from_account`, `*_bought` reaches `to_account`).
    pub fn from_transfer(store_id: &str, row: &crate::exchange::InterTransfer) -> Option<Self> {
        let id = row.transfer_id.trim();
        let amount = Amount::parse(row.amount.trim())
            .ok()
            .filter(|value| value.is_positive())?;
        let coin = row.coin.trim().to_ascii_uppercase();
        if id.is_empty() || coin.is_empty() {
            return None;
        }
        Some(Self {
            exec_id: format!("transfer:{id}"),
            store_id: store_id.to_string(),
            category: "transfer".into(),
            symbol: coin.clone(),
            side: String::new(),
            asset_bought: coin.clone(),
            qty_bought: amount,
            asset_sold: coin,
            qty_sold: amount,
            price: Amount::ZERO,
            occurred_at: millis_to_iso(&row.timestamp),
            comment: format!("Bybit: {} → {}", row.from_account_type, row.to_account_type),
            kind: ImportKind::Transfer,
            from_account: Some(row.from_account_type.trim().to_ascii_uppercase()),
            to_account: Some(row.to_account_type.trim().to_ascii_uppercase()),
            ..Self::default()
        })
    }

    /// Stage an Earn payout as income of the Earn account.
    pub fn from_earn_yield(store_id: &str, row: &crate::exchange::EarnYield) -> Option<Self> {
        let amount = Amount::parse(row.amount.trim())
            .ok()
            .filter(|value| value.is_positive())?;
        let coin = row.coin.trim().to_ascii_uppercase();
        let id = if row.id.trim().is_empty() {
            format!("{}:{}:{}", row.product_id, row.created_at, row.amount)
        } else {
            row.id.trim().to_string()
        };
        if coin.is_empty() {
            return None;
        }
        Some(Self {
            exec_id: format!("earn:{id}"),
            store_id: store_id.to_string(),
            category: "earn".into(),
            symbol: coin.clone(),
            side: String::new(),
            asset_bought: coin,
            qty_bought: amount,
            occurred_at: millis_to_iso(&row.created_at),
            comment: "Bybit Earn".into(),
            kind: ImportKind::EarnIncome,
            to_account: Some(crate::exchange::bybit::ACCOUNT_EARN.into()),
            ..Self::default()
        })
    }

    /// Everything one Bybit sweep stages for an exchange wallet: fills,
    /// moves between its accounts and Earn payouts. Rows that cannot be read
    /// are dropped, never guessed at.
    pub fn bybit_batch(
        root_id: &str,
        fills: &[crate::exchange::Execution],
        transfers: &[crate::exchange::InterTransfer],
        yields: &[crate::exchange::EarnYield],
    ) -> Vec<Self> {
        fills
            .iter()
            .filter_map(|exec| Self::from_execution(root_id, exec.category_hint(), exec))
            .chain(
                transfers
                    .iter()
                    .filter_map(|row| Self::from_transfer(root_id, row)),
            )
            .chain(
                yields
                    .iter()
                    .filter_map(|row| Self::from_earn_yield(root_id, row)),
            )
            .collect()
    }

    /// Assets referenced by this fill, for the staging UI summary.
    pub fn symbols(&self) -> [String; 2] {
        [self.asset_sold.clone(), self.asset_bought.clone()]
    }
}

/// Bybit millisecond timestamps as RFC3339; empty when unreadable.
fn millis_to_iso(raw: &str) -> String {
    raw.trim()
        .parse::<i64>()
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|moment| moment.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

fn nonempty(raw: String) -> String {
    raw.trim().to_string()
}

/// Exchange credentials grouped by exchange name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Exchanges {
    #[serde(default)]
    pub bybit: Vec<ExchangeKeys>,
}

/// What started a sync with an exchange.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    /// The background poll.
    #[default]
    Auto,
    /// «Синхронизировать сейчас» / «Загрузить историю».
    Manual,
}

/// One sync with an exchange, for its history in the wallet settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ImportRun {
    pub id: String,
    /// The exchange wallet (root store).
    pub store_id: String,
    /// RFC3339.
    pub at: String,
    pub source: ImportSource,
    /// Records the exchange sent.
    pub fetched: u32,
    /// New ones staged for confirmation (or booked at once).
    pub staged: u32,
    /// Seen before (staged, booked or discarded).
    pub known: u32,
    /// Left out by the import settings (kind, coin, account, date).
    pub filtered: u32,
    /// Booked at once by «подтверждать сразу».
    pub confirmed: u32,
    /// The correction that set the live balance, if one was needed.
    #[serde(default)]
    pub reconcile_tx_id: Option<String>,
    /// Why the sync failed; `None` when it went through.
    #[serde(default)]
    pub error: Option<String>,
}

/// Most syncs kept in the history, over all exchanges.
pub const MAX_IMPORT_RUNS: usize = 500;
/// Most discarded exchange ids remembered so they are not staged again.
pub const MAX_DISCARDED_IMPORTS: usize = 20_000;

/// Full plaintext vault document. Serialized as JSON (dev) or bincode (prod).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct VaultDoc {
    pub version: u32,
    pub id: String,
    pub created_at: String,
    pub settings: Settings,
    pub stores: Vec<Store>,
    pub holdings: Vec<Holding>,
    pub transactions: Vec<Transaction>,
    pub categories: Vec<Category>,
    pub price_cache: Vec<PricePoint>,
    pub fx_cache: Vec<FxPoint>,
    /// Exchange credentials, added in payload v1.
    #[serde(default)]
    #[ts(skip)]
    pub exchanges: Exchanges,
    /// Staged Bybit fills awaiting user confirmation (D2.4).
    #[serde(default)]
    pub pending_imports: Vec<PendingImport>,
    pub contacts: Vec<Contact>,
    pub budgets: Vec<Budget>,
    pub goals: Vec<Goal>,
    pub reminders: Vec<Reminder>,
    pub price_alerts: Vec<PriceAlert>,
    pub categorization_rules: Vec<CategorizationRule>,
    pub utility_items: Vec<UtilityItem>,
    pub meter_readings: Vec<MeterReading>,
    pub enrichments: Vec<EnrichmentRecord>,
    pub manual_prices: Vec<ManualPrice>,
    /// Virtual envelopes: earmarked parts of a store's balance.
    #[serde(default)]
    pub envelopes: Vec<crate::model::planning::Envelope>,
    /// Due dates and notes of debts with contacts.
    #[serde(default)]
    pub debt_terms: Vec<crate::model::contact::DebtTerms>,
    /// Bank of Russia key rate history, for the deposit-interest tax limit.
    #[serde(default)]
    pub key_rates: Vec<crate::model::deposit::KeyRatePoint>,
    /// Notices already shown, newest last (payload v2).
    #[serde(default)]
    pub notification_log: Vec<crate::model::notify::NoticeRecord>,
    /// Node workflows: their graphs, runs, variables, the operations waiting
    /// for confirmation and what their triggers last saw.
    #[serde(default)]
    pub workflows: Vec<crate::workflow::Workflow>,
    #[serde(default)]
    pub workflow_runs: Vec<crate::workflow::WorkflowRun>,
    #[serde(default)]
    pub workflow_vars: Vec<crate::workflow::WorkflowVar>,
    #[serde(default)]
    pub workflow_queue: Vec<crate::workflow::QueuedActions>,
    #[serde(default)]
    pub workflow_state: Vec<crate::workflow::WorkflowState>,
    /// Runs asleep at a «Подождать» node.
    #[serde(default)]
    pub workflow_waits: Vec<crate::workflow::model::WorkflowWait>,
    /// Syncs with exchanges, newest last.
    #[serde(default)]
    pub import_runs: Vec<ImportRun>,
    /// Exchange ids the user discarded: a later sync does not stage them again.
    #[serde(default)]
    pub discarded_imports: Vec<String>,
    /// Saved investment calculator scenarios (plan A).
    #[serde(default)]
    pub calc_scenarios: Vec<crate::model::planning::CalcScenario>,
    /// Inflation by currency and month, fetched in the background (plan I1).
    #[serde(default)]
    pub inflation_points: Vec<crate::model::inflation::InflationReading>,
    /// The emergency fund; an empty store list means none (plan C1).
    #[serde(default)]
    pub emergency_fund: crate::model::planning::EmergencyFund,
    /// The last monthly checkup notice, so the next one comes only on a change.
    #[serde(default)]
    pub health_last: Option<crate::model::planning::HealthSnapshot>,
    /// Portfolio baskets with target shares (plan F); empty = the preset.
    #[serde(default)]
    pub asset_classes: Vec<crate::model::planning::AssetClass>,
    /// Assets placed in a basket by hand, by `AssetRef::key()`.
    #[serde(default)]
    pub asset_class_of: Vec<crate::model::planning::AssetClassLink>,
    /// Dated valuations of property (plan G3).
    #[serde(default)]
    pub valuations: Vec<Valuation>,
    /// Operations linked to a property (plan G4).
    #[serde(default)]
    pub tx_properties: Vec<TxProperty>,
    /// The user's own data sources (plan I2).
    #[serde(default)]
    pub data_sources: Vec<crate::model::datasource::DataSource>,
    /// Assets priced by a data source, by `AssetRef::key()`.
    #[serde(default)]
    pub asset_price_sources: Vec<crate::model::datasource::AssetPriceSource>,
    /// Deposits whose rate a data source follows.
    #[serde(default)]
    pub deposit_rate_sources: Vec<crate::model::datasource::DepositRateSource>,
    /// Device-to-device sync bookkeeping: change stamps, deletions, trusted
    /// devices, batches kept for undo (docs/sync.md). Managed by the vault
    /// itself; the JSON editor's copy is ignored on save.
    #[serde(default)]
    #[ts(skip)]
    #[schemars(with = "serde_json::Value")]
    pub sync: crate::sync::SyncState,
}

/// JSON Schema of the decrypted document.
///
/// Derived from [`VaultDoc`] itself, so the raw JSON editor's field
/// suggestions cannot drift away from the format the vault is actually
/// serialized in. This is also why `#[serde]` attributes — not `#[ts]` ones —
/// decide the field names: the editor shows the bytes on disk, and those come
/// from serde.
pub fn json_schema() -> serde_json::Value {
    schemars::schema_for!(VaultDoc).into()
}

impl Default for VaultDoc {
    fn default() -> Self {
        Self {
            version: crate::VAULT_FORMAT_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            created_at: now_iso(),
            settings: Settings::default(),
            stores: Vec::new(),
            holdings: Vec::new(),
            transactions: Vec::new(),
            categories: default_categories(),
            price_cache: Vec::new(),
            fx_cache: Vec::new(),
            exchanges: Exchanges::default(),
            pending_imports: Vec::new(),
            contacts: Vec::new(),
            budgets: Vec::new(),
            goals: Vec::new(),
            reminders: Vec::new(),
            price_alerts: Vec::new(),
            categorization_rules: Vec::new(),
            utility_items: Vec::new(),
            meter_readings: Vec::new(),
            enrichments: Vec::new(),
            manual_prices: Vec::new(),
            envelopes: Vec::new(),
            debt_terms: Vec::new(),
            key_rates: Vec::new(),
            notification_log: Vec::new(),
            workflows: Vec::new(),
            workflow_runs: Vec::new(),
            workflow_vars: Vec::new(),
            workflow_queue: Vec::new(),
            workflow_state: Vec::new(),
            workflow_waits: Vec::new(),
            import_runs: Vec::new(),
            discarded_imports: Vec::new(),
            calc_scenarios: Vec::new(),
            inflation_points: Vec::new(),
            emergency_fund: Default::default(),
            health_last: None,
            asset_classes: Vec::new(),
            asset_class_of: Vec::new(),
            valuations: Vec::new(),
            tx_properties: Vec::new(),
            data_sources: Vec::new(),
            asset_price_sources: Vec::new(),
            deposit_rate_sources: Vec::new(),
            sync: crate::sync::SyncState::default(),
        }
    }
}

impl VaultDoc {
    /// ISO-8601 UTC now (RFC3339, lexicographically sortable).
    pub fn timestamp() -> String {
        now_iso()
    }

    pub fn fiat_asset(symbol: &str, name: &str) -> AssetRef {
        AssetRef::fiat(symbol, name)
    }

    pub fn chain_list() -> Vec<ChainInfo> {
        crate::model::chain::ALL_CHAINS
            .iter()
            .map(|chain| chain.info())
            .collect()
    }
}

/// ISO-8601 (RFC3339) UTC timestamp, e.g. `2026-09-28T21:47:00.123Z`.
pub fn now_iso() -> String {
    use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_utc();
    now.format(&Rfc3339).unwrap_or_default()
}

pub fn iso_to_millis(iso: &str) -> i64 {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::parse(iso, &Rfc3339)
        .map(|parsed| (parsed.unix_timestamp_nanos() / 1_000_000) as i64)
        .unwrap_or(0)
}

/// Unix timestamp (seconds) to ISO-8601 (RFC3339) UTC.
/// Returns an empty string for negative/absurd input rather than panicking.
pub fn unix_to_iso(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|stamp| stamp.format(&Rfc3339).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod json_schema_tests {
    use super::*;

    #[test]
    fn schema_describes_the_serialized_document() {
        let schema = json_schema();
        let defs = schema["$defs"].as_object().expect("$defs");
        let root = schema["properties"].as_object().expect("root properties");

        // The Strategy Lab is gone, so the document must no longer carry a
        // `sim` section — not at the root, and not as a nested `SimState` def.
        // An old vault that still has it is silently accepted: serde ignores
        // unknown fields, which is what lets the format version stay pinned.
        assert!(
            !root.contains_key("sim"),
            "`sim` was removed with the Strategy Lab and must not come back"
        );
        assert!(
            !root.contains_key("strategies"),
            "`strategies` lived under `sim` and must be gone from the root too"
        );
        assert!(
            !defs.contains_key("SimState"),
            "SimState must no longer appear in the schema"
        );

        // Every field serde actually writes must be describable.
        let on_disk = serde_json::to_value(VaultDoc::default()).unwrap();
        let written = on_disk.as_object().unwrap();
        for key in written.keys() {
            assert!(root.contains_key(key), "schema is missing `{key}`");
        }

        // `Amount` reaches JSON as a decimal string.
        let quantity = &defs["Holding"]["properties"]["quantity"];
        assert_eq!(quantity["type"], "string");

        // Enums are constrained, so the editor can offer the values. Variants
        // carrying doc comments come back as `oneOf` with `const` instead of a
        // flat `enum`, so only require that the names are present somewhere.
        let store_kinds = defs["StoreKind"].to_string();
        for kind in ["exchange", "crypto", "property", "bank", "cash"] {
            assert!(store_kinds.contains(kind), "StoreKind is missing `{kind}`");
        }
        // `paper` only ever held the Strategy Lab's virtual money.
        assert!(
            !store_kinds.contains("paper"),
            "the paper account went away with the Strategy Lab"
        );

        // The user-scoped preferences live in the app config, not in the
        // document: the editor must not offer to write them into the vault.
        let settings = defs["Settings"]["properties"]
            .as_object()
            .expect("Settings properties");
        for moved in ["base_currency", "theme", "locale", "favorite_currencies"] {
            assert!(
                !settings.contains_key(moved),
                "`{moved}` moved to appconfig and must leave the vault schema"
            );
        }

        // `Transaction` is an externally tagged enum, so the document stores
        // one of these keys. The editor has to offer them, and must not offer a
        // flat record with a `kind` discriminator.
        let tx_schema = &defs["Transaction"];
        assert!(
            tx_schema.get("oneOf").is_some(),
            "Transaction must be a union of tagged variants"
        );
        let tags = tx_schema.to_string();
        for kind in crate::model::transaction::TxKind::ALL {
            assert!(
                tags.contains(kind.tag()),
                "schema is missing `{}`",
                kind.tag()
            );
        }

        // Optional fields admit null, so the editor can leave them empty.
        let fee = &defs["TopupTx"]["properties"]["fee"];
        assert!(fee.to_string().contains("null"), "{fee}");

        // Every collection the later phases need is already in the schema
        // (plan D11), so the editor can show and edit them.
        for collection in [
            "contacts",
            "workflows",
            "workflow_runs",
            "budgets",
            "goals",
            "reminders",
            "price_alerts",
            "categorization_rules",
            "utility_items",
            "meter_readings",
            "enrichments",
            "manual_prices",
        ] {
            assert!(
                root.contains_key(collection),
                "schema is missing `{collection}`"
            );
        }
        for def in [
            "Contact",
            "Category",
            "AutomationSpec",
            "Goal",
            "LiabilityTerms",
            "EnrichmentRecord",
        ] {
            assert!(defs.contains_key(def), "schema is missing `{def}`");
        }
    }
}
