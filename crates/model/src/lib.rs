#![warn(unreachable_pub)]
//! `mwt-model`: the data model stored inside the encrypted vault (`VaultDoc` and
//! every type it holds), the record kinds, the sync registry table, the coin tables
//! and the typed ids. It depends on `mwt-base` only. Modules are private; this root
//! re-lists every public item, and `trade-core` keeps the old module paths as
//! re-exports of the same items.

mod attachment;
mod automation_facts;
mod bybit;
mod bybit_staging;
mod category;
mod chain;
mod coins;
mod contact;
mod crypto_history;
mod cryptotrack;
mod datasource;
mod deposit;
mod device_secrets;
mod enrichment;
mod entity_ref;
mod ids;
mod inflation;
mod iso_date;
mod json_value;
mod llama_coin;
mod notify;
mod planning;
mod rate_index;
mod record;
mod registry;
mod run_key;
mod schedule_moments;
mod shared_event;
mod store;
mod sync_state;
mod transaction;
mod vault;
mod workflow;

pub use attachment::{Attachment, AttachmentKey, AttachmentKind, MAX_ATTACHMENT_BYTES};
pub use automation_facts::{
    ESTIMATE_OF, TRIGGER_NODE, When, fire_time, made_by, main_leg, parse_time, recent_average,
    schedule_key, schedule_params, start_mark, when_params, workflow_mark,
};
pub use bybit::{BybitCategory, EarnYield, Execution, InterTransfer, TradePrefill};
pub use bybit_staging::PendingImportExt;
pub use category::{
    CATEGORIES_REV, CATEGORY_ADDITIONS, Category, CategoryGroup, DEFAULT_ESSENTIAL,
    DEFAULT_PASSIVE, EXCHANGE_IMPORT_CATEGORY, default_categories, is_passive_key,
};
pub use chain::{ALL_CHAINS, Chain, ChainInfo};
pub use coins::{is_usd_stablecoin, well_known_coin, well_known_coins};
pub use contact::{Contact, ContactAccount, ContactKind, DebtTerms, DetailKind};
pub use crypto_history::CryptoHistory;
pub use cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoCursor, CryptoDirection, CryptoFee, CryptoLeg,
    CryptoOverride, CryptoPosition, CryptoPricePoint, CryptoSnapshot, CryptoToken, CryptoTx,
    CryptoTxKind, CryptoWipe, TokenId, TokenVerdict, TrackingRef, chain_slug, is_evm,
    normalize_address,
};
pub use datasource::{
    AssetPriceSource, BROKEN_AFTER, DataSource, DepositRateSource, Extracted, FileFormat,
    JUMP_FACTOR, MAX_SOURCE_HISTORY, NumberFormat, SeriesPoint, SourceInterval, SourceMethod,
    SourceOutput, SourceStatus, SourceValue, TableRow,
};
pub use deposit::{
    Capitalization, DepositTerms, IncomeBracket, KeyRatePoint, PdsTerms, RatePeriod,
};
pub use device_secrets::{BlockscoutKey, DeviceKey, DeviceSecrets, NetworkEndpoints};
pub use enrichment::{EnrichmentData, EnrichmentRecord, EnrichmentSource, EnrichmentStatus};
pub use entity_ref::{EntityKind, EntityRef, Pinned};
pub use ids::StoreId;
pub use inflation::{InflationMode, InflationReading, InflationSource};
pub use json_value::{json_field, json_number, json_text};
pub use llama_coin::llama_coin;
pub use notify::{
    MAX_NOTICE_LOG, NoticeCategory, NoticeCategoryCfg, NoticeRecord, NotificationSettings, notice,
    notice_at, notice_title,
};
pub use planning::{
    AlertCondition, AmountMode, AssetClass, AssetClassLink, Automation, AutomationOutcome,
    AutomationPreset, AutomationRun, AutomationSpec, Budget, BudgetPeriod, BudgetTarget,
    CalcScenario, CategorizationRule, EmergencyFund, Envelope, EnvelopeMove, Goal, HealthColor,
    HealthReasonCode, HealthSnapshot, MeterReading, PriceAlert, RejectReason, Reminder,
    ReminderPay, ReminderRepeat, ReminderSource, ReminderSourceKind, RepeatUnit, ScenarioSource,
    Schedule, TransactionDraft, UtilityItem, UtilityKind, WorkdayShift, describe_reject,
    draft_to_transaction,
};
pub use rate_index::RateIndex;
pub use record::{Record, RecordKind, level3_collections, record_kinds};
pub use registry::{
    COLLECTIONS, Collection, DEVICE_SETTINGS, InnerEntry, LOCAL, Rule, SETTINGS_SCOPE, Scope,
    UNION_DAYS, find, is_secret, is_secret_field,
};
pub use run_key::RunKey;
pub use schedule_moments::schedule_moments_limited;
pub use shared_event::{
    EventItem, GuestDetails, ItemPlan, ItemSplit, ME, Participant, PriceMode, Purchase,
    ROUNDING_STEPS, RemovedWithPayment, SettleDirection, Settlement, SettlementKind, ShareOverride,
    SharedEvent, SharedEventStatus, SharedLink,
};
pub use store::{
    AssetRef, Binding, BindingShape, CarInfo, Custody, ExchangeProfile, ExchangeProvider, Holding,
    ImportAccountRoute, ImportCategories, ImportSettings, IndicatorCfg, LiabilityKind,
    LiabilityTerms, LoyaltyInfo, LoyaltyUnit, MAX_DRAWINGS_JSON, MAX_STORE_DEPTH, MarketKind,
    PropertyCategory, PropertyInfo, Purpose, RevalueFloor, RevalueRule, Store, StoreKind,
    StoreScope, SubType, TerminalLayout, TerminalState,
};
pub use sync_state::{
    BATCH_DAYS, BatchEntry, ChangeItem, ChangeKind, CollectionChanges, CutBy, DEVICE_TAG_LEN,
    FieldChange, MAX_BATCHES, MAX_FUTURE_MS, MAX_TRANSFERS, OverwrittenField, REVOKED_GATE_DAYS,
    RecordStamps, RecordState, Stamp, SyncBatch, SyncCutReason, SyncJournalCount, SyncJournalEntry,
    SyncJournalOutcome, SyncState, SyncSummary, TOMBSTONE_DAYS, TransferRecord, TrustedPeer,
    acked_through, device_tag, is_own_device,
};
pub use transaction::{
    AccountRef, AdjustmentLine, AdjustmentTx, AssetSellTx, CashNote, ConvertSide, ConvertTx,
    DebtEffect, DefiAction, DefiTx, Fee, FuturesAction, FuturesCategory, FuturesDirection,
    KindDetails, Leg, LegRole, Liquidity, PaymentTx, PriceStamp, TopupTx, TradeFuturesTx,
    TradeSide, TradeSpotTx, Transaction, TransferTx, TxKind, TxMeta, TxOrigin, allowed_kinds,
    default_kind,
};
#[cfg(feature = "schema")]
pub use vault::json_schema;
pub use vault::{
    ACCOUNT_EARN, ACCOUNT_FUND, ACCOUNT_UNIFIED, DailyBudgetMode, DashPeriod, DashboardWidgetCfg,
    ExchangeKeys, Exchanges, FxPoint, ImportKind, ImportRun, ImportSource, MAX_DISCARDED_IMPORTS,
    MAX_IMPORT_RUNS, ManualPrice, PendingImport, PricePoint, Settings, TxProperty,
    VAULT_FORMAT_VERSION, Valuation, VaultDoc, default_fx_interval, default_minimize_to_tray,
    iso_to_millis, now_iso, unix_to_iso,
};
pub use workflow::{
    GraphVersion, MAX_GRAPH_VERSIONS, MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, StepStatus,
    TestExpect, TraceStep, VarScope, WaitKind, Workflow, WorkflowRun, WorkflowState, WorkflowTest,
    WorkflowVar, WorkflowWait,
};
