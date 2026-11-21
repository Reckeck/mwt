//! The v4 vault document, frozen.
//!
//! FROZEN: the types below never change once committed; the only additions are the conversion to
//! the live document with what the shims did on open ([`transition`]: the Bybit queue in
//! [`adopt_bybit`], the graph steps of versions 1-3 in `graph`) and the stream-owned copies named at
//! the end (T17). It is deleted together with the one-time transition from payload v4 (container 1) to payload v1 (container 2); until then it is the only
//! place that still reads a v4 file.
//!
//! The graph steps have a second user outside the transition: `import_workflow`
//! (`service/records.rs`) runs the graph of an export file of version 1 through
//! [`migrate_export_graph`]. `graph::migrate_graph_json` is only its test seam
//! (`cfg(any(test, feature = "testing"))`).
//!
//! [`VaultDocV4`] is a copy of the live `VaultDoc` as it was written by the last build that
//! produced v4. Types the format reset changes are copied here (`SettingsV4` with `crypto`,
//! `WorkflowWaitV4` with `queue_mode`, `ExchangesV4`/`ExchangeKeysV4`, `PendingImportV4`); every
//! other nested type is reused from the live model, because the reset leaves it alone.
//! Parsing only: nothing here is written back.
//!
//! The D4 defaulted fields that sit inside the vault document are frozen here too, as they were
//! (`CategoryV4`: `essential`/`passive` default to `None`; `BudgetV4`: `warn` defaults to on), so later tasks may drop those defaults
//! from the live types without touching this file. Three D4 sites have no copy because no vault
//! payload holds them: the notification snapshot is a separate device file, `sync_ids` is an empty
//! placeholder, and the planning and shared-event defaults (`SharedEvent` and the planning types
//! are reused from the live model) belong to streams C and D, whose files are edited only by T17;
//! if T17 changes the shape of those live types, that is the one case where it adds a copy.

mod adopt;
pub mod graph;
mod graph_kinds;

use serde::Deserialize;

/// The graph JSON of an old workflow export brought to the live version, or `None` when it needs
/// nothing.
pub fn migrate_export_graph(json: &str) -> Option<String> {
    graph::migrate_graph(json)
}

pub use adopt::{LegacyAdoption, adopt_bybit};
#[cfg(any(test, feature = "testing"))]
pub use adopt::{Stored, event_ids, store_batch};

use mwt_base::Amount;
use mwt_model::Contact;
use mwt_model::EnrichmentRecord;
use mwt_model::Transaction;
use mwt_model::{
    Budget, BudgetPeriod, BudgetTarget, CategorizationRule, Goal, MeterReading, PriceAlert,
    Reminder, UtilityItem,
};
use mwt_model::{Category, CategoryGroup};
use mwt_model::{
    DailyBudgetMode, DashPeriod, DashboardWidgetCfg, ExchangeKeys, Exchanges, FxPoint, ImportKind,
    ImportRun, ManualPrice, PendingImport, PricePoint, Settings, TxProperty, Valuation, VaultDoc,
    default_fx_interval, default_minimize_to_tray,
};
use mwt_model::{Holding, Store};
use mwt_model::{WaitKind, Workflow, WorkflowWait};

/// The full plaintext document of a v4 vault.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VaultDocV4 {
    pub version: u32,
    pub id: String,
    pub created_at: String,
    pub settings: SettingsV4,
    pub stores: Vec<Store>,
    pub holdings: Vec<Holding>,
    pub transactions: Vec<Transaction>,
    pub categories: Vec<CategoryV4>,
    pub price_cache: Vec<PricePoint>,
    pub fx_cache: Vec<FxPoint>,
    /// Exchange credentials, added in payload v1. Since H2 the keys live in
    /// the device file `<vault>.secrets` (`vault_io::device_secrets`): at level
    /// 3 this is an in-memory index of which stores have a key and where
    /// their fills land, with empty `api_key`/`api_secret`, and it is left
    /// out of every save, so the stored field stays empty. Keys found here
    /// in an older file move to the device file at the first level-3 open;
    /// below level 3 they are kept as they were and not used.
    #[serde(default)]
    pub exchanges: ExchangesV4,
    /// Staged Bybit fills awaiting user confirmation (D2.4).
    #[serde(default)]
    pub pending_imports: Vec<PendingImportV4>,
    pub contacts: Vec<Contact>,
    pub budgets: Vec<BudgetV4>,
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
    pub envelopes: Vec<mwt_model::Envelope>,
    /// Due dates and notes of debts with contacts.
    #[serde(default)]
    pub debt_terms: Vec<mwt_model::DebtTerms>,
    /// Bank of Russia key rate history, for the deposit-interest tax limit.
    #[serde(default)]
    pub key_rates: Vec<mwt_model::KeyRatePoint>,
    /// Notices already shown, newest last (payload v2).
    #[serde(default)]
    pub notification_log: Vec<mwt_model::NoticeRecord>,
    /// Node workflows: their graphs, runs, variables, the operations waiting
    /// for confirmation and what their triggers last saw.
    #[serde(default)]
    pub workflows: Vec<mwt_model::Workflow>,
    #[serde(default)]
    pub workflow_runs: Vec<mwt_model::WorkflowRun>,
    #[serde(default)]
    pub workflow_vars: Vec<mwt_model::WorkflowVar>,
    #[serde(default)]
    pub workflow_queue: Vec<mwt_model::QueuedActions>,
    #[serde(default)]
    pub workflow_state: Vec<mwt_model::WorkflowState>,
    /// Runs asleep at a "Wait" node.
    #[serde(default)]
    pub workflow_waits: Vec<WorkflowWaitV4>,
    /// Syncs with exchanges, newest last.
    #[serde(default)]
    pub import_runs: Vec<ImportRun>,
    /// Exchange ids the user discarded: a later sync does not stage them again.
    #[serde(default)]
    pub discarded_imports: Vec<String>,
    /// Saved investment calculator scenarios (plan A).
    #[serde(default)]
    pub calc_scenarios: Vec<mwt_model::CalcScenario>,
    /// Inflation by currency and month, fetched in the background (plan I1).
    #[serde(default)]
    pub inflation_points: Vec<mwt_model::InflationReading>,
    /// The emergency fund; an empty store list means none (plan C1).
    #[serde(default)]
    pub emergency_fund: mwt_model::EmergencyFund,
    /// The last monthly checkup notice, so the next one comes only on a change.
    #[serde(default)]
    pub health_last: Option<mwt_model::HealthSnapshot>,
    /// Portfolio baskets with target shares (plan F); empty = the preset.
    #[serde(default)]
    pub asset_classes: Vec<mwt_model::AssetClass>,
    /// Assets placed in a basket by hand, by `AssetRef::key()`.
    #[serde(default)]
    pub asset_class_of: Vec<mwt_model::AssetClassLink>,
    /// Dated valuations of property (plan G3).
    #[serde(default)]
    pub valuations: Vec<Valuation>,
    /// Operations linked to a property (plan G4).
    #[serde(default)]
    pub tx_properties: Vec<TxProperty>,
    /// The user's own data sources (plan I2).
    #[serde(default)]
    pub data_sources: Vec<mwt_model::DataSource>,
    /// Assets priced by a data source, by `AssetRef::key()`.
    #[serde(default)]
    pub asset_price_sources: Vec<mwt_model::AssetPriceSource>,
    /// Deposits whose rate a data source follows.
    #[serde(default)]
    pub deposit_rate_sources: Vec<mwt_model::DepositRateSource>,
    /// Device-to-device sync bookkeeping: change stamps, deletions, trusted
    /// devices, batches kept for undo (docs/sync.md). Managed by the vault
    /// itself; the JSON editor's copy is ignored on save.
    #[serde(default)]
    pub sync: mwt_model::SyncState,
    /// Files and links of operations and goals (Full-New-Attach).
    #[serde(default)]
    pub attachments: Vec<mwt_model::Attachment>,
    /// The keys the attachments' files are sealed with.
    #[serde(default)]
    pub attachment_keys: Vec<mwt_model::AttachmentKey>,
    /// Shared events: things bought together and split between people.
    #[serde(default)]
    pub shared_events: Vec<mwt_model::SharedEvent>,
    // -- the crypto module (stage 2, decision P1 of 2026-11-10) ------------
    //
    // Sibling top-level fields instead of one nested object: a sync stamp, a
    // tombstone and an undo copy are per record, so editing one operation
    // sends one record and the 16 MiB message limit is never reached.
    // Nothing derived is stored (balances, average prices): it is computed
    // from the events. Each field serializes even when empty, so
    // `every_vault_field_has_a_sync_rule` sees it.
    /// Addresses the crypto module follows.
    #[serde(default)]
    pub crypto_accounts: Vec<mwt_model::CryptoAccount>,
    /// Operations the module read from its sources.
    #[serde(default)]
    pub crypto_events: Vec<mwt_model::CryptoTx>,
    /// The user's corrections over what a source reported.
    #[serde(default)]
    pub crypto_overrides: Vec<mwt_model::CryptoOverride>,
    /// Tokens the module has met, with what it takes to show and price them.
    #[serde(default)]
    pub crypto_tokens: Vec<mwt_model::CryptoToken>,
    /// Holdings inside protocols (the DeFi stage fills them).
    #[serde(default)]
    pub crypto_positions: Vec<mwt_model::CryptoPosition>,
    /// The module's value over time, so its history survives a refetch.
    #[serde(default)]
    pub crypto_snapshots: Vec<mwt_model::CryptoSnapshot>,
    /// How far each source has read each account: this device's own progress,
    /// never synced (`sync::registry::LOCAL`).
    #[serde(default)]
    pub crypto_cursors: Vec<mwt_model::CryptoCursor>,
    /// Cached token prices; a cache, so it unions silently between devices.
    #[serde(default)]
    pub crypto_prices: Vec<mwt_model::CryptoPricePoint>,
    /// Wipes of the module, so the other devices learn about one through an
    /// ordinary sync instead of sending the data back.
    #[serde(default)]
    pub crypto_wiped_at: Vec<mwt_model::CryptoWipe>,
}

/// Vault-scoped settings of v4; carries the derived `crypto` flag that the reset drops.
/// Vault-scoped settings.
///
/// The preferences that describe the *user* rather than this particular vault
/// (theme, language, base currency, favourite currencies) live outside the
/// encrypted doc — see [`crate::appconfig::AppSettings`].
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SettingsV4 {
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
    pub notifications: mwt_model::NotificationSettings,
    /// The budget settings were saved at least once: the checklist step
    /// "Savings rate" is done even when 10 % was the right answer.
    #[serde(default)]
    pub budget_configured: bool,
    /// Checklist steps the user said they do not need.
    #[serde(default)]
    pub onboarding_skipped: Vec<String>,
    /// Where each currency's inflation comes from; a currency missing here
    /// reads as [`crate::service::inflation`] decides.
    #[serde(default)]
    pub inflation_mode: std::collections::BTreeMap<String, mwt_model::InflationMode>,
    /// The data source of each currency in `InflationMode::Source`.
    #[serde(default)]
    pub inflation_source: std::collections::BTreeMap<String, String>,
    /// Flat annual inflation, percent, by lowercase currency code, where the
    /// group differs from the shipped table (`default_inflation`); read through
    /// `UnlockedVault::flat_inflation`. Group data: every device sees one table.
    #[serde(default)]
    pub inflation_manual: std::collections::BTreeMap<String, Amount>,
    /// The device-local rates of the app config were copied into
    /// `inflation_manual` once (`UnlockedVault::adopt_flat_inflation`).
    #[serde(default)]
    pub inflation_adopted: bool,
    /// The user's usual monthly income in the budget currency, used until
    /// three months of history exist (plan B).
    #[serde(default)]
    pub manual_income: Option<Amount>,
    /// The user's usual monthly spending, same rule.
    #[serde(default)]
    pub manual_expenses: Option<Amount>,
    /// The last batch of built-in categories this vault has received
    /// ([`mwt_model::CATEGORY_ADDITIONS`]): each batch is added
    /// once, so a category the user deleted does not come back.
    #[serde(default)]
    pub categories_rev: u32,
    /// Yearly return the freedom index potential assumes, percent; `None`
    /// takes the key rate (plan C3).
    #[serde(default)]
    pub freedom_rate: Option<Amount>,
    /// `YYYY-MM` the unspent money of each month is carried from into the
    /// next; `None` = every month stands alone (redesign «month's balance»).
    #[serde(default)]
    pub carry_since: Option<String>,
    /// The sidebar's «Pinned» block, in its order (plan J2).
    #[serde(default)]
    pub pinned: Vec<mwt_model::Pinned>,
    /// The period every dashboard widget measures unless it has its own.
    #[serde(default)]
    pub dashboard_period: DashPeriod,
    /// The user hid the "Where to start" checklist (plan H3; it lived in localStorage).
    #[serde(default)]
    pub onboarding_checklist_hidden: bool,
    /// Whether the vault holds crypto. Derived since spec E14 (the crypto module has data, or a
    /// hand-made crypto wallet remains): [`crate::UnlockedVault::open`] refreshes this copy, nobody
    /// sets it. The field stays in the file for compatibility with old and new builds.
    #[serde(default)]
    pub crypto: Option<bool>,
    /// The IANA zone schedule moments are computed in, the same on every
    /// device so they agree on which moment it is. `None` until the first
    /// device that runs a schedule writes its own zone.
    #[serde(default)]
    pub schedule_timezone: Option<String>,
    /// The raise to level 3 re-encrypted this vault and has not yet archived
    /// the hand-kept crypto wallets (spec E11). The raise sets it before the
    /// re-encryption, so it is on disk in the very first level-3 file, and
    /// clears it when the archiving is done. An open that finds it set
    /// finishes the archiving; a level-3 vault without it is never archived
    /// by an open. The device's own: a peer's mark says nothing about a raise
    /// that was cut short here (`sync::registry::SETTINGS_SCOPE`).
    #[serde(default)]
    pub crypto_raise_pending: bool,
}

/// A category of v4: `essential` and `passive` carry the `#[serde(default)]` that D4 may drop live.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct CategoryV4 {
    pub key: String,
    pub name: Option<String>,
    pub group: CategoryGroup,
    pub archived: bool,
    #[serde(default)]
    pub essential: Option<bool>,
    #[serde(default)]
    pub passive: Option<bool>,
}

/// A budget of v4: files of old builds may lack `warn`, which read as on. The live type has no
/// default, so the transition writes it explicitly.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BudgetV4 {
    pub id: String,
    pub active: bool,
    pub target: BudgetTarget,
    pub period: BudgetPeriod,
    pub limit: Amount,
    pub currency: String,
    #[serde(default = "warn_on")]
    pub warn: bool,
}

fn warn_on() -> bool {
    true
}

/// Exchange credentials of v4.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct ExchangeKeysV4 {
    pub store_id: String,
    pub api_key: String,
    pub api_secret: String,
    /// Sub-account spot fills are booked on; `None` = `store_id`.
    pub spot_store_id: Option<String>,
    /// Sub-account futures fills are booked on; `None` = `store_id`.
    pub futures_store_id: Option<String>,
}

impl std::fmt::Debug for ExchangeKeysV4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeKeysV4")
            .field("store_id", &self.store_id)
            .field("api_key", &"<redacted>")
            .field("api_secret", &"<redacted>")
            .finish()
    }
}

impl Drop for ExchangeKeysV4 {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.store_id.zeroize();
        self.api_key.zeroize();
        self.api_secret.zeroize();
    }
}

/// Exchange credentials of v4 grouped by exchange name.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ExchangesV4 {
    #[serde(default)]
    pub bybit: Vec<ExchangeKeysV4>,
}

/// A staged Bybit fill of v4: the legacy queue the transition turns into crypto events.
/// A staged Bybit fill awaiting user confirmation before becoming a trade.
///
/// Fills land here first and never touch `transactions` until the user
/// confirms them, so a background import can never silently rewrite history.
/// Deduplication key is Bybit's `execId`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PendingImportV4 {
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

/// A run asleep at a "Wait" node, v4; `queue_mode` is unused and dropped by the reset.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkflowWaitV4 {
    pub id: String,
    pub workflow_id: String,
    pub run_id: String,
    pub node_id: String,
    /// RFC3339; empty for a question without a deadline.
    pub resume_at: String,
    /// Node outputs computed before the wait, as JSON.
    pub memo: String,
    pub queue_mode: bool,
    pub trigger: String,
    #[serde(default)]
    pub kind: WaitKind,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub graph: String,
}

// -- conversion to the live document ----------------------------------------------------------

/// What the transition hands on besides the document.
#[derive(Debug)]
pub struct Carry {
    /// What the conversion of the old Bybit queue did.
    pub adoption: LegacyAdoption,
}

/// The live document of a v4 file, with the work the shims used to do on open done once: the old
/// Bybit queue is converted into operations of the crypto module and every workflow graph is
/// brought to the live version. The holdings are not refolded here.
pub fn transition(mut doc: VaultDocV4) -> (VaultDoc, Carry) {
    let adoption = adopt_bybit(&mut doc);
    (VaultDoc::from(doc), Carry { adoption })
}

impl From<VaultDocV4> for VaultDoc {
    /// Field by field, so a field added to either side fails to compile here. The exchange keys
    /// and the queue are carried as they are; the graphs are brought to the live version.
    fn from(old: VaultDocV4) -> Self {
        // Exhaustive on purpose: no `..`.
        let VaultDocV4 {
            version,
            id,
            created_at,
            settings,
            stores,
            holdings,
            transactions,
            categories,
            price_cache,
            fx_cache,
            exchanges,
            pending_imports,
            contacts,
            budgets,
            goals,
            reminders,
            price_alerts,
            categorization_rules,
            utility_items,
            meter_readings,
            enrichments,
            manual_prices,
            envelopes,
            debt_terms,
            key_rates,
            notification_log,
            workflows,
            workflow_runs,
            workflow_vars,
            workflow_queue,
            workflow_state,
            workflow_waits,
            import_runs,
            discarded_imports,
            calc_scenarios,
            inflation_points,
            emergency_fund,
            health_last,
            asset_classes,
            asset_class_of,
            valuations,
            tx_properties,
            data_sources,
            asset_price_sources,
            deposit_rate_sources,
            sync,
            attachments,
            attachment_keys,
            shared_events,
            crypto_accounts,
            crypto_events,
            crypto_overrides,
            crypto_tokens,
            crypto_positions,
            crypto_snapshots,
            crypto_cursors,
            crypto_prices,
            crypto_wiped_at,
        } = old;
        Self {
            version,
            id,
            created_at,
            settings: settings.into(),
            stores,
            holdings,
            transactions,
            categories: categories.into_iter().map(Category::from).collect(),
            price_cache,
            fx_cache,
            exchanges: Exchanges::from(exchanges),
            pending_imports: pending_imports
                .into_iter()
                .map(PendingImport::from)
                .collect(),
            contacts,
            budgets: budgets.into_iter().map(Budget::from).collect(),
            goals,
            reminders,
            price_alerts,
            categorization_rules,
            utility_items,
            meter_readings,
            enrichments,
            manual_prices,
            envelopes,
            debt_terms,
            key_rates,
            notification_log,
            workflows: workflows.into_iter().map(migrated_workflow).collect(),
            workflow_runs,
            workflow_vars,
            workflow_queue,
            workflow_state,
            workflow_waits: workflow_waits.into_iter().map(WorkflowWait::from).collect(),
            import_runs,
            discarded_imports,
            calc_scenarios,
            inflation_points,
            emergency_fund,
            health_last,
            asset_classes,
            asset_class_of,
            valuations,
            tx_properties,
            data_sources,
            asset_price_sources,
            deposit_rate_sources,
            attachments,
            attachment_keys,
            shared_events,
            sync,
            crypto_accounts,
            crypto_events,
            crypto_overrides,
            crypto_tokens,
            crypto_positions,
            crypto_snapshots,
            crypto_cursors,
            crypto_prices,
            crypto_wiped_at,
        }
    }
}

/// A workflow with its graph, its earlier versions and its unsaved draft brought to the live version.
fn migrated_workflow(mut workflow: Workflow) -> Workflow {
    migrate_graph(&mut workflow.graph);
    for version in workflow.versions.iter_mut().chain(workflow.draft.as_mut()) {
        migrate_graph(&mut version.graph);
    }
    workflow
}

fn migrate_graph(json: &mut String) {
    if let Some(migrated) = graph::migrate_graph(json) {
        *json = migrated;
    }
}

impl From<SettingsV4> for Settings {
    fn from(old: SettingsV4) -> Self {
        // Exhaustive on purpose: no `..`.
        let SettingsV4 {
            display_decimals,
            dashboard_widgets,
            fx_refresh_interval_sec,
            minimize_to_tray,
            savings_rate,
            daily_budget_mode,
            budget_currency,
            notifications,
            budget_configured,
            onboarding_skipped,
            inflation_mode,
            inflation_source,
            inflation_manual,
            inflation_adopted,
            manual_income,
            manual_expenses,
            categories_rev,
            freedom_rate,
            carry_since,
            pinned,
            dashboard_period,
            onboarding_checklist_hidden,
            crypto: _,
            schedule_timezone,
            crypto_raise_pending,
        } = old;
        Self {
            display_decimals,
            dashboard_widgets,
            fx_refresh_interval_sec,
            minimize_to_tray,
            savings_rate,
            daily_budget_mode,
            budget_currency,
            notifications,
            budget_configured,
            onboarding_skipped,
            inflation_mode,
            inflation_source,
            inflation_manual,
            inflation_adopted,
            manual_income,
            manual_expenses,
            categories_rev,
            freedom_rate,
            carry_since,
            pinned,
            dashboard_period,
            onboarding_checklist_hidden,
            schedule_timezone,
            crypto_raise_pending,
        }
    }
}

impl From<CategoryV4> for Category {
    fn from(old: CategoryV4) -> Self {
        Self {
            key: old.key,
            name: old.name,
            group: old.group,
            archived: old.archived,
            essential: old.essential,
            passive: old.passive,
        }
    }
}

impl From<BudgetV4> for Budget {
    fn from(old: BudgetV4) -> Self {
        Self {
            id: old.id,
            active: old.active,
            target: old.target,
            period: old.period,
            limit: old.limit,
            currency: old.currency,
            warn: old.warn,
        }
    }
}

impl From<&ExchangeKeysV4> for ExchangeKeys {
    fn from(old: &ExchangeKeysV4) -> Self {
        Self {
            store_id: old.store_id.clone(),
            api_key: old.api_key.clone(),
            api_secret: old.api_secret.clone(),
            spot_store_id: old.spot_store_id.clone(),
            futures_store_id: old.futures_store_id.clone(),
        }
    }
}

impl From<ExchangesV4> for Exchanges {
    fn from(old: ExchangesV4) -> Self {
        Self {
            bybit: old.bybit.iter().map(ExchangeKeys::from).collect(),
        }
    }
}

impl From<PendingImportV4> for PendingImport {
    fn from(old: PendingImportV4) -> Self {
        Self {
            exec_id: old.exec_id,
            store_id: old.store_id,
            category: old.category,
            symbol: old.symbol,
            side: old.side,
            asset_bought: old.asset_bought,
            qty_bought: old.qty_bought,
            asset_sold: old.asset_sold,
            qty_sold: old.qty_sold,
            price: old.price,
            fee: old.fee,
            fee_asset: old.fee_asset,
            occurred_at: old.occurred_at,
            comment: old.comment,
            is_maker: old.is_maker,
            closed_size: old.closed_size,
            closed_pnl: old.closed_pnl,
            kind: old.kind,
            from_account: old.from_account,
            to_account: old.to_account,
        }
    }
}

impl From<WorkflowWaitV4> for WorkflowWait {
    fn from(old: WorkflowWaitV4) -> Self {
        let mut graph = old.graph;
        migrate_graph(&mut graph);
        Self {
            id: old.id,
            workflow_id: old.workflow_id,
            run_id: old.run_id,
            node_id: old.node_id,
            resume_at: old.resume_at,
            memo: old.memo,
            trigger: old.trigger,
            kind: old.kind,
            question: old.question,
            created_at: old.created_at,
            graph,
        }
    }
}

#[cfg(test)]
pub(crate) mod dropped {
    /// Fields the reset drops from the live document, as JSON pointers; the live and the
    /// frozen document are compared without them.
    pub(crate) const DROPPED: [&str; 1] = ["/settings/crypto"];

    /// The document with every [`DROPPED`] field removed.
    pub(crate) fn without_dropped(mut doc: serde_json::Value) -> serde_json::Value {
        for pointer in DROPPED {
            if let Some((parent, field)) = pointer.rsplit_once('/')
                && let Some(object) = doc
                    .pointer_mut(parent)
                    .and_then(|node| node.as_object_mut())
            {
                object.remove(field);
            }
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::dropped::without_dropped;
    use super::*;
    use crate::storage::fixtures_v4::V4_PASSWORD;
    use mwt_crypto::format::SnapshotFormat;

    #[test]
    fn a_budget_without_warn_reads_as_on() {
        let budget: BudgetV4 = serde_json::from_value(serde_json::json!({
            "id": "b1", "active": true,
            "target": "total",
            "period": "month",
            "limit": "100",
            "currency": "USD"
        }))
        .unwrap();
        assert!(Budget::from(budget).warn);
    }

    const FIXTURES: [(&str, SnapshotFormat, &[u8]); 2] = [
        (
            "v4.json.wallet",
            SnapshotFormat::Json,
            include_bytes!("fixtures/v4.json.wallet"),
        ),
        (
            "v4.pack.wallet",
            SnapshotFormat::Pack,
            include_bytes!("fixtures/v4.pack.wallet"),
        ),
    ];

    /// The fixture's payload parsed as the frozen document, in its own format.
    fn parse_fixture(name: &str, fmt: SnapshotFormat, bytes: &[u8]) -> VaultDocV4 {
        let (payload, read, _key) =
            mwt_crypto::open_bytes_keyed(bytes, V4_PASSWORD.as_bytes()).unwrap();
        assert_eq!(read, fmt, "{name}");
        match fmt {
            SnapshotFormat::Json => serde_json::from_slice(&payload).unwrap(),
            _ => rmp_serde::from_slice(&payload).unwrap(),
        }
    }

    #[test]
    fn both_fixtures_parse_into_the_frozen_document() {
        for (name, fmt, bytes) in FIXTURES {
            let doc = parse_fixture(name, fmt, bytes);
            assert_eq!(doc.version, 4, "{name}");
            assert_eq!(doc.settings.crypto, Some(true), "{name}");
            assert!(
                doc.exchanges
                    .bybit
                    .iter()
                    .any(|keys| !keys.api_secret.is_empty()),
                "{name}"
            );
            assert!(!doc.pending_imports.is_empty(), "{name}");
        }
    }

    /// The expected document, with the graph strings brought to the live version as the
    /// conversion does (the graph steps themselves are tested in `graph`).
    fn expected_converted() -> serde_json::Value {
        let mut expected: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/v4.expected.json")).unwrap();
        let migrate = |holder: &mut serde_json::Value| {
            let graph = holder["graph"].as_str().unwrap().to_owned();
            if let Some(migrated) = graph::migrate_graph(&graph) {
                holder["graph"] = migrated.into();
            }
        };
        for workflow in expected["workflows"].as_array_mut().unwrap() {
            migrate(workflow);
            for version in workflow["versions"].as_array_mut().unwrap() {
                migrate(version);
            }
            if workflow["draft"].is_object() {
                migrate(&mut workflow["draft"]);
            }
        }
        for wait in expected["workflow_waits"].as_array_mut().unwrap() {
            if wait["graph"].as_str().is_some_and(|text| !text.is_empty()) {
                migrate(wait);
            }
        }
        expected
    }

    #[test]
    fn converting_the_fixtures_gives_the_expected_document() {
        for (name, fmt, bytes) in FIXTURES {
            let live = VaultDoc::from(parse_fixture(name, fmt, bytes));
            assert_eq!(
                without_dropped(serde_json::to_value(&live).unwrap()),
                without_dropped(expected_converted()),
                "{name}"
            );
        }
    }

    #[test]
    fn exchanges_are_carried_unchanged() {
        let (name, fmt, bytes) = FIXTURES[0];
        let old = parse_fixture(name, fmt, bytes);
        let secrets: Vec<(String, String, String)> = old
            .exchanges
            .bybit
            .iter()
            .map(|keys| {
                (
                    keys.store_id.clone(),
                    keys.api_key.clone(),
                    keys.api_secret.clone(),
                )
            })
            .collect();
        assert!(!secrets.is_empty());
        let (live, _carry) = transition(old);
        let carried = |keys: &[ExchangeKeys]| -> Vec<(String, String, String)> {
            keys.iter()
                .map(|key| {
                    (
                        key.store_id.clone(),
                        key.api_key.clone(),
                        key.api_secret.clone(),
                    )
                })
                .collect()
        };
        assert_eq!(carried(&live.exchanges.bybit), secrets);
    }

    #[test]
    fn the_transition_accounts_for_every_queue_entry() {
        for (name, fmt, bytes) in FIXTURES {
            let old = parse_fixture(name, fmt, bytes);
            let queued = u32::try_from(old.pending_imports.len()).unwrap();
            let (live, carry) = transition(old);
            let adoption = carry.adoption;
            assert_eq!(
                adoption.converted + adoption.known + adoption.kept,
                queued,
                "{name}"
            );
            assert_eq!(live.pending_imports.len() as u32, adoption.kept, "{name}");
        }
    }
}
