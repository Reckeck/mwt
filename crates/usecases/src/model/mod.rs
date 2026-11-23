//! Domain model stored inside the encrypted vault (`VaultDoc`): the facade of `mwt-model`.
//! Every old path `model::<module>::<item>` still resolves to the item of the crate.

#[cfg(any(test, feature = "testkit"))]
pub mod fixtures;
/// Tests of model items that name a higher layer; they stay in the facade.
#[cfg(test)]
#[path = "../model_tests/mod.rs"]
mod model_tests;

pub use mwt_model::{Chain, ChainInfo, StoreKind, Transaction, VaultDoc};

pub mod attachment {
    pub use mwt_model::{Attachment, AttachmentKey, AttachmentKind, MAX_ATTACHMENT_BYTES};
}

pub mod bybit {
    pub use mwt_model::{
        BybitCategory, EarnYield, Execution, InterTransfer, PendingImportExt, TradePrefill,
    };
}

pub mod category {
    pub use mwt_model::{
        CATEGORIES_REV, CATEGORY_ADDITIONS, Category, CategoryGroup, DEFAULT_ESSENTIAL,
        DEFAULT_PASSIVE, EXCHANGE_IMPORT_CATEGORY, default_categories, is_passive_key,
    };
}

pub mod chain {
    pub use mwt_model::{ALL_CHAINS, Chain, ChainInfo};
}

pub mod coins {
    pub use mwt_model::{is_usd_stablecoin, well_known_coin, well_known_coins};
}

pub mod contact {
    pub use mwt_model::{Contact, ContactAccount, ContactKind, DebtTerms, DetailKind};
}

pub mod cryptotrack {
    pub use mwt_model::{
        CryptoAccount, CryptoAsset, CryptoCursor, CryptoDirection, CryptoFee, CryptoLeg,
        CryptoOverride, CryptoPosition, CryptoPricePoint, CryptoSnapshot, CryptoToken, CryptoTx,
        CryptoTxKind, CryptoWipe, TokenId, TokenVerdict, TrackingRef, chain_slug, is_evm,
        normalize_address,
    };
}

pub mod datasource {
    pub use mwt_model::{
        AssetPriceSource, BROKEN_AFTER, DataSource, DepositRateSource, Extracted, FileFormat,
        JUMP_FACTOR, MAX_SOURCE_HISTORY, NumberFormat, SeriesPoint, SourceInterval, SourceMethod,
        SourceOutput, SourceStatus, SourceValue, TableRow,
    };
}

pub mod device_secrets {
    pub use mwt_model::{BlockscoutKey, DeviceKey, DeviceSecrets, NetworkEndpoints};
}

pub mod deposit {
    pub use mwt_model::{
        Capitalization, DepositTerms, IncomeBracket, KeyRatePoint, PdsTerms, RatePeriod,
    };
}

pub mod enrichment {
    pub use mwt_model::{EnrichmentData, EnrichmentRecord, EnrichmentSource, EnrichmentStatus};
}

pub mod entity_ref {
    pub use mwt_model::{EntityKind, EntityRef, Pinned};
}

pub mod ids {
    pub use mwt_model::StoreId;
}

pub mod inflation {
    pub use mwt_model::{InflationMode, InflationReading, InflationSource};
}

pub mod notify {
    pub use mwt_model::{
        MAX_NOTICE_LOG, NoticeCategory, NoticeCategoryCfg, NoticeRecord, NotificationSettings,
        notice, notice_at, notice_title,
    };
}

pub mod planning {
    pub use mwt_model::{
        AlertCondition, AmountMode, AssetClass, AssetClassLink, Automation, AutomationOutcome,
        AutomationPreset, AutomationRun, AutomationSpec, Budget, BudgetPeriod, BudgetTarget,
        CalcScenario, CategorizationRule, EmergencyFund, Envelope, EnvelopeMove, Goal, HealthColor,
        HealthReasonCode, HealthSnapshot, MeterReading, PriceAlert, RejectReason, Reminder,
        ReminderPay, ReminderRepeat, ReminderSource, ReminderSourceKind, RepeatUnit,
        ScenarioSource, Schedule, TransactionDraft, UtilityItem, UtilityKind, WorkdayShift,
        describe_reject, draft_to_transaction,
    };
}

pub mod record {
    pub use mwt_model::{Record, RecordKind, level3_collections, record_kinds};
}

pub mod registry {
    pub use mwt_model::{
        COLLECTIONS, Collection, DEVICE_SETTINGS, InnerEntry, LOCAL, Rule, SETTINGS_SCOPE, Scope,
        UNION_DAYS, find, is_secret, is_secret_field,
    };
}

pub mod run_key {
    pub use mwt_model::RunKey;
}

pub mod shared_event {
    pub use mwt_model::{
        EventItem, GuestDetails, ItemPlan, ItemSplit, ME, Participant, PriceMode, Purchase,
        ROUNDING_STEPS, RemovedWithPayment, SettleDirection, Settlement, SettlementKind,
        ShareOverride, SharedEvent, SharedEventStatus, SharedLink,
    };
}

pub mod store {
    pub use mwt_model::{
        AssetRef, Binding, BindingShape, CarInfo, Custody, ExchangeProfile, ExchangeProvider,
        Holding, ImportAccountRoute, ImportCategories, ImportSettings, IndicatorCfg, LiabilityKind,
        LiabilityTerms, LoyaltyInfo, LoyaltyUnit, MAX_DRAWINGS_JSON, MAX_STORE_DEPTH, MarketKind,
        PropertyCategory, PropertyInfo, Purpose, RevalueFloor, RevalueRule, Store, StoreKind,
        StoreScope, SubType, TerminalLayout, TerminalState,
    };
}

pub mod sync_state {
    pub use mwt_model::{
        BATCH_DAYS, BatchEntry, ChangeItem, ChangeKind, CollectionChanges, CutBy, DEVICE_TAG_LEN,
        FieldChange, MAX_BATCHES, MAX_FUTURE_MS, MAX_TRANSFERS, OverwrittenField,
        REVOKED_GATE_DAYS, RecordStamps, RecordState, Stamp, SyncBatch, SyncCutReason,
        SyncJournalCount, SyncJournalEntry, SyncJournalOutcome, SyncState, SyncSummary,
        TOMBSTONE_DAYS, TransferRecord, TrustedPeer, acked_through, device_tag, is_own_device,
    };
}

pub mod transaction {
    pub use mwt_model::{
        AccountRef, AdjustmentLine, AdjustmentTx, AssetSellTx, CashNote, ConvertSide, ConvertTx,
        DebtEffect, DefiAction, DefiTx, Fee, FuturesAction, FuturesCategory, FuturesDirection,
        KindDetails, Leg, LegRole, Liquidity, PaymentTx, PriceStamp, TopupTx, TradeFuturesTx,
        TradeSide, TradeSpotTx, Transaction, TransferTx, TxKind, TxMeta, TxOrigin, allowed_kinds,
        default_kind,
    };
}

pub mod vault {
    #[cfg(feature = "schema")]
    pub use mwt_model::json_schema;
    pub use mwt_model::{
        ACCOUNT_EARN, ACCOUNT_FUND, ACCOUNT_UNIFIED, DailyBudgetMode, DashPeriod,
        DashboardWidgetCfg, ExchangeKeys, Exchanges, FxPoint, ImportKind, ImportRun, ImportSource,
        MAX_DISCARDED_IMPORTS, MAX_IMPORT_RUNS, ManualPrice, PendingImport, PricePoint, Settings,
        TxProperty, VAULT_FORMAT_VERSION, Valuation, VaultDoc, default_fx_interval,
        default_minimize_to_tray, iso_to_millis, now_iso, unix_to_iso,
    };
}

pub mod workflow {
    pub use mwt_model::{
        GraphVersion, MAX_GRAPH_VERSIONS, MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, StepStatus,
        TestExpect, TraceStep, VarScope, WaitKind, Workflow, WorkflowRun, WorkflowState,
        WorkflowTest, WorkflowVar, WorkflowWait,
    };
}

pub mod llama_coin {
    pub use mwt_model::llama_coin;
}
