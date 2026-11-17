//! Payload v3, frozen: the document as every vault written before the crypto
//! module holds it.
//!
//! v4 adds the crypto module's own top-level collections (`crypto_accounts`,
//! `crypto_events`, …). They are additive and defaulted, so a v3 file reads
//! without them and nothing has to be converted but the version: the step
//! exists so that the version on disk says whether a file may hold crypto
//! data at all. An older build then refuses a newer file at the handshake
//! ("the versions differ, update") instead of meeting an unknown sync
//! collection halfway through an exchange (decision P2 of 2026-11-10).
//!
//! `VaultDocV3` lists the v3 top-level fields by name, so a field added to the
//! live document later does not leak into what a v3 file is read as.
//!
//! Rule for nested types: the field types below are the live ones, as `v2`
//! does for everything outside the automations it converts. When a later
//! version changes a nested type in a way a v3 file cannot be read with,
//! freeze that type here first and point the field at the frozen copy.
//!
//! Never edit this definition: it describes files already on users' disks.

use serde::Deserialize;

use super::v2::VaultDocV2;
use crate::model::attachment::{Attachment, AttachmentKey};
use crate::model::category::Category;
use crate::model::contact::{Contact, DebtTerms};
use crate::model::datasource::{AssetPriceSource, DataSource, DepositRateSource};
use crate::model::deposit::KeyRatePoint;
use crate::model::enrichment::EnrichmentRecord;
use crate::model::inflation::InflationReading;
use crate::model::notify::NoticeRecord;
use crate::model::planning::{
    AssetClass, AssetClassLink, Budget, CalcScenario, CategorizationRule, EmergencyFund, Envelope,
    Goal, HealthSnapshot, MeterReading, PriceAlert, Reminder, UtilityItem,
};
use crate::model::shared_event::SharedEvent;
use crate::model::store::{Holding, Store};
use crate::model::transaction::Transaction;
use crate::model::vault::{
    Exchanges, FxPoint, ImportRun, ManualPrice, PendingImport, PricePoint, Settings, TxProperty,
    Valuation, VaultDoc,
};
use crate::sync::SyncState;
use crate::workflow::model::WorkflowWait;
use crate::workflow::{QueuedActions, Workflow, WorkflowRun, WorkflowState, WorkflowVar};

/// The v3 document: the top-level fields of a v3 file, without the crypto
/// module's collections.
#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct VaultDocV3 {
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
    #[serde(default)]
    pub exchanges: Exchanges,
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
    #[serde(default)]
    pub envelopes: Vec<Envelope>,
    #[serde(default)]
    pub debt_terms: Vec<DebtTerms>,
    #[serde(default)]
    pub key_rates: Vec<KeyRatePoint>,
    #[serde(default)]
    pub notification_log: Vec<NoticeRecord>,
    #[serde(default)]
    pub workflows: Vec<Workflow>,
    #[serde(default)]
    pub workflow_runs: Vec<WorkflowRun>,
    #[serde(default)]
    pub workflow_vars: Vec<WorkflowVar>,
    #[serde(default)]
    pub workflow_queue: Vec<QueuedActions>,
    #[serde(default)]
    pub workflow_state: Vec<WorkflowState>,
    #[serde(default)]
    pub workflow_waits: Vec<WorkflowWait>,
    #[serde(default)]
    pub import_runs: Vec<ImportRun>,
    #[serde(default)]
    pub discarded_imports: Vec<String>,
    #[serde(default)]
    pub calc_scenarios: Vec<CalcScenario>,
    #[serde(default)]
    pub inflation_points: Vec<InflationReading>,
    #[serde(default)]
    pub emergency_fund: EmergencyFund,
    #[serde(default)]
    pub health_last: Option<HealthSnapshot>,
    #[serde(default)]
    pub asset_classes: Vec<AssetClass>,
    #[serde(default)]
    pub asset_class_of: Vec<AssetClassLink>,
    #[serde(default)]
    pub valuations: Vec<Valuation>,
    #[serde(default)]
    pub tx_properties: Vec<TxProperty>,
    #[serde(default)]
    pub data_sources: Vec<DataSource>,
    #[serde(default)]
    pub asset_price_sources: Vec<AssetPriceSource>,
    #[serde(default)]
    pub deposit_rate_sources: Vec<DepositRateSource>,
    #[serde(default)]
    pub sync: SyncState,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub attachment_keys: Vec<AttachmentKey>,
    #[serde(default)]
    pub shared_events: Vec<SharedEvent>,
}

/// v3 → v4: the crypto collections of a v3 file are absent, so they start
/// empty, and the document is rewritten at v4.
impl From<VaultDocV3> for VaultDoc {
    fn from(old: VaultDocV3) -> Self {
        Self {
            id: old.id,
            created_at: old.created_at,
            settings: old.settings,
            stores: old.stores,
            holdings: old.holdings,
            transactions: old.transactions,
            categories: old.categories,
            price_cache: old.price_cache,
            fx_cache: old.fx_cache,
            exchanges: old.exchanges,
            pending_imports: old.pending_imports,
            contacts: old.contacts,
            budgets: old.budgets,
            goals: old.goals,
            reminders: old.reminders,
            price_alerts: old.price_alerts,
            categorization_rules: old.categorization_rules,
            utility_items: old.utility_items,
            meter_readings: old.meter_readings,
            enrichments: old.enrichments,
            manual_prices: old.manual_prices,
            envelopes: old.envelopes,
            debt_terms: old.debt_terms,
            key_rates: old.key_rates,
            notification_log: old.notification_log,
            workflows: old.workflows,
            workflow_runs: old.workflow_runs,
            workflow_vars: old.workflow_vars,
            workflow_queue: old.workflow_queue,
            workflow_state: old.workflow_state,
            workflow_waits: old.workflow_waits,
            import_runs: old.import_runs,
            discarded_imports: old.discarded_imports,
            calc_scenarios: old.calc_scenarios,
            inflation_points: old.inflation_points,
            emergency_fund: old.emergency_fund,
            health_last: old.health_last,
            asset_classes: old.asset_classes,
            asset_class_of: old.asset_class_of,
            valuations: old.valuations,
            tx_properties: old.tx_properties,
            data_sources: old.data_sources,
            asset_price_sources: old.asset_price_sources,
            deposit_rate_sources: old.deposit_rate_sources,
            sync: old.sync,
            attachments: old.attachments,
            attachment_keys: old.attachment_keys,
            shared_events: old.shared_events,
            version: 4,
            crypto_accounts: Vec::new(),
            crypto_events: Vec::new(),
            crypto_overrides: Vec::new(),
            crypto_tokens: Vec::new(),
            crypto_positions: Vec::new(),
            crypto_snapshots: Vec::new(),
            crypto_cursors: Vec::new(),
            crypto_prices: Vec::new(),
            crypto_wiped_at: Vec::new(),
        }
    }
}

/// The last step of a v2 migration now lands on v3 instead of the live
/// document: `v2` hands over a document at version 3, and the step above
/// takes it from there.
impl From<VaultDocV2> for VaultDocV3 {
    fn from(old: VaultDocV2) -> Self {
        let doc = VaultDoc::from(old);
        Self {
            version: doc.version,
            id: doc.id,
            created_at: doc.created_at,
            settings: doc.settings,
            stores: doc.stores,
            holdings: doc.holdings,
            transactions: doc.transactions,
            categories: doc.categories,
            price_cache: doc.price_cache,
            fx_cache: doc.fx_cache,
            exchanges: doc.exchanges,
            pending_imports: doc.pending_imports,
            contacts: doc.contacts,
            budgets: doc.budgets,
            goals: doc.goals,
            reminders: doc.reminders,
            price_alerts: doc.price_alerts,
            categorization_rules: doc.categorization_rules,
            utility_items: doc.utility_items,
            meter_readings: doc.meter_readings,
            enrichments: doc.enrichments,
            manual_prices: doc.manual_prices,
            envelopes: doc.envelopes,
            debt_terms: doc.debt_terms,
            key_rates: doc.key_rates,
            notification_log: doc.notification_log,
            workflows: doc.workflows,
            workflow_runs: doc.workflow_runs,
            workflow_vars: doc.workflow_vars,
            workflow_queue: doc.workflow_queue,
            workflow_state: doc.workflow_state,
            workflow_waits: doc.workflow_waits,
            import_runs: doc.import_runs,
            discarded_imports: doc.discarded_imports,
            calc_scenarios: doc.calc_scenarios,
            inflation_points: doc.inflation_points,
            emergency_fund: doc.emergency_fund,
            health_last: doc.health_last,
            asset_classes: doc.asset_classes,
            asset_class_of: doc.asset_class_of,
            valuations: doc.valuations,
            tx_properties: doc.tx_properties,
            data_sources: doc.data_sources,
            asset_price_sources: doc.asset_price_sources,
            deposit_rate_sources: doc.deposit_rate_sources,
            sync: doc.sync,
            attachments: doc.attachments,
            attachment_keys: doc.attachment_keys,
            shared_events: doc.shared_events,
        }
    }
}

/// Where v3 sits in the chain (`storage::chain`): one step up is the live
/// document, so this is the last step of a migration.
impl super::chain::Legacy for VaultDocV3 {
    type Next = VaultDoc;
    const VERSION: u32 = 3;
}
