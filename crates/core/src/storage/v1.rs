//! Payload v1, frozen: the document as every vault before payload v2 holds it.
//!
//! v1 release builds wrote bincode, which is positional: such a file reads
//! only with the exact v1 field list. Every type whose shape changed in v2 is
//! copied here as it was; the unchanged ones are reused from the model. When
//! a later version changes one of the reused types, freeze its v1 shape here
//! first.
//!
//! Never edit these definitions: they describe files already on users' disks.

use serde::Deserialize;

use super::v2::{AutomationRunV2, AutomationV2, VaultDocV2};
use crate::Amount;
use crate::model::category::Category;
use crate::model::contact::{Contact, ContactAccount, ContactKind, DebtTerms};
use crate::model::deposit::KeyRatePoint;
use crate::model::enrichment::EnrichmentRecord;
use crate::model::notify::NotificationSettings;
use crate::model::planning::{
    Budget, CategorizationRule, Envelope, Goal, MeterReading, PriceAlert, Reminder, UtilityItem,
    UtilityKind,
};
use crate::model::store::{Holding, Store};
use crate::model::transaction::Transaction;
use crate::model::vault::{
    DailyBudgetMode, DashboardWidgetCfg, Exchanges, FxPoint, ManualPrice, PendingImport,
    PricePoint, Settings, VaultDoc, default_fx_interval, default_minimize_to_tray,
};

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct SettingsV1 {
    pub display_decimals: u32,
    #[serde(default)]
    pub dashboard_widgets: Vec<DashboardWidgetCfg>,
    #[serde(default = "default_fx_interval")]
    pub fx_refresh_interval_sec: u32,
    #[serde(default = "default_minimize_to_tray")]
    pub minimize_to_tray: bool,
    pub savings_rate: Amount,
    pub daily_budget_mode: DailyBudgetMode,
    pub budget_currency: String,
}

/// A goal before it could follow a calculator scenario.
#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct GoalV1 {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub target_amount: Amount,
    pub currency: String,
    pub starts_at: String,
    pub deadline: Option<String>,
    pub priority: u8,
    pub monthly_commitment: Option<Amount>,
    pub scope: crate::model::store::StoreScope,
    pub created_at: String,
}

impl From<GoalV1> for Goal {
    fn from(old: GoalV1) -> Self {
        Self {
            id: old.id,
            name: old.name,
            active: old.active,
            target_amount: old.target_amount,
            currency: old.currency,
            starts_at: old.starts_at,
            deadline: old.deadline,
            priority: old.priority,
            monthly_commitment: old.monthly_commitment,
            scope: old.scope,
            created_at: old.created_at,
            scenario_id: None,
            expected_return: None,
        }
    }
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct ReminderV1 {
    pub id: String,
    pub text: String,
    pub due_at: String,
    pub done: bool,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct MeterReadingV1 {
    pub id: String,
    pub item_id: String,
    pub value: Amount,
    pub period: String,
    pub recorded_at: String,
}

/// `UtilityKind` before the meter formula and zones (frozen when they came).
#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(rename_all = "snake_case")]
pub enum UtilityKindV1 {
    Metered {
        unit: String,
        tariff: Amount,
    },
    Static {
        amount: Amount,
    },
    Housing {
        amount: Amount,
        deposit: Option<Amount>,
        due_day: u8,
        automation_id: Option<String>,
    },
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct UtilityItemV1 {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub kind: UtilityKindV1,
    pub currency: String,
    #[serde(default)]
    pub property_id: String,
    pub store_id: Option<String>,
    pub category: String,
}

impl From<UtilityItemV1> for UtilityItem {
    fn from(old: UtilityItemV1) -> Self {
        Self {
            id: old.id,
            name: old.name,
            active: old.active,
            kind: match old.kind {
                UtilityKindV1::Metered { unit, tariff } => UtilityKind::Metered {
                    unit,
                    tariff,
                    formula: None,
                    zones: Vec::new(),
                },
                UtilityKindV1::Static { amount } => UtilityKind::Static { amount },
                UtilityKindV1::Housing {
                    amount,
                    deposit,
                    due_day,
                    automation_id,
                } => UtilityKind::Housing {
                    amount,
                    deposit,
                    due_day,
                    automation_id,
                },
            },
            currency: old.currency,
            property_id: old.property_id,
            store_id: old.store_id,
            category: old.category,
        }
    }
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct ContactV1 {
    pub id: String,
    pub name: String,
    pub kind: ContactKind,
    pub accounts: Vec<ContactAccount>,
    pub note: String,
    pub archived: bool,
    pub created_at: String,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct VaultDocV1 {
    pub version: u32,
    pub id: String,
    pub created_at: String,
    pub settings: SettingsV1,
    pub stores: Vec<Store>,
    pub holdings: Vec<Holding>,
    pub transactions: Vec<Transaction>,
    pub categories: Vec<Category>,
    pub price_cache: Vec<PricePoint>,
    pub fx_cache: Vec<FxPoint>,
    #[serde(default)]
    pub exchanges: Exchanges,
    #[serde(default)]
    pub pending_imports: Vec<PendingImport>,
    pub contacts: Vec<ContactV1>,
    pub automations: Vec<AutomationV2>,
    pub automation_runs: Vec<AutomationRunV2>,
    pub budgets: Vec<Budget>,
    pub goals: Vec<GoalV1>,
    pub reminders: Vec<ReminderV1>,
    pub price_alerts: Vec<PriceAlert>,
    pub categorization_rules: Vec<CategorizationRule>,
    pub utility_items: Vec<UtilityItemV1>,
    pub meter_readings: Vec<MeterReadingV1>,
    pub enrichments: Vec<EnrichmentRecord>,
    pub manual_prices: Vec<ManualPrice>,
    #[serde(default)]
    pub envelopes: Vec<Envelope>,
    #[serde(default)]
    pub debt_terms: Vec<DebtTerms>,
    #[serde(default)]
    pub key_rates: Vec<KeyRatePoint>,
}

/// v1 → v2: reminders gain a repeat and a source, meter readings a day,
/// contacts a group, settings the notification switches, and the document a
/// notification log. Every v1 value keeps its place.
impl From<VaultDocV1> for VaultDocV2 {
    fn from(old: VaultDocV1) -> Self {
        let settings = old.settings;
        let doc = VaultDoc {
            version: 2,
            id: old.id,
            created_at: old.created_at,
            settings: Settings {
                display_decimals: settings.display_decimals,
                dashboard_widgets: settings.dashboard_widgets,
                fx_refresh_interval_sec: settings.fx_refresh_interval_sec,
                minimize_to_tray: settings.minimize_to_tray,
                savings_rate: settings.savings_rate,
                daily_budget_mode: settings.daily_budget_mode,
                budget_currency: settings.budget_currency,
                notifications: NotificationSettings::default(),
                budget_configured: false,
                onboarding_skipped: Vec::new(),
                inflation_mode: std::collections::BTreeMap::new(),
                inflation_source: std::collections::BTreeMap::new(),
                manual_income: None,
                manual_expenses: None,
                categories_rev: 0,
                freedom_rate: None,
                pinned: Vec::new(),
                dashboard_period: Default::default(),
                onboarding_checklist_hidden: false,
            },
            stores: old.stores,
            holdings: old.holdings,
            transactions: old.transactions,
            categories: old.categories,
            price_cache: old.price_cache,
            fx_cache: old.fx_cache,
            exchanges: old.exchanges,
            pending_imports: old.pending_imports,
            contacts: old
                .contacts
                .into_iter()
                .map(|contact| Contact {
                    id: contact.id,
                    name: contact.name,
                    kind: contact.kind,
                    accounts: contact.accounts,
                    note: contact.note,
                    archived: contact.archived,
                    created_at: contact.created_at,
                    group: String::new(),
                })
                .collect(),
            budgets: old.budgets,
            goals: old.goals.into_iter().map(Into::into).collect(),
            reminders: old
                .reminders
                .into_iter()
                .map(|reminder| Reminder {
                    id: reminder.id,
                    done: reminder.done,
                    ..Reminder::new(&reminder.text, &reminder.due_at)
                })
                .collect(),
            price_alerts: old.price_alerts,
            categorization_rules: old.categorization_rules,
            utility_items: old.utility_items.into_iter().map(Into::into).collect(),
            meter_readings: old
                .meter_readings
                .into_iter()
                .map(|reading| MeterReading {
                    id: reading.id,
                    item_id: reading.item_id,
                    value: reading.value,
                    period: reading.period,
                    recorded_at: reading.recorded_at,
                    day: None,
                    zone: None,
                })
                .collect(),
            enrichments: old.enrichments,
            manual_prices: old.manual_prices,
            envelopes: old.envelopes,
            debt_terms: old.debt_terms,
            key_rates: old.key_rates,
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
        };
        VaultDocV2 {
            doc,
            automations: old.automations,
            automation_runs: old.automation_runs,
        }
    }
}
