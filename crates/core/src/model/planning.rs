//! Planning data: automations, budgets, goals, reminders, alerts, rules and
//! utilities. Defined now so the vault schema breaks once (plan D11); the
//! features that use most of it arrive in later phases.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::store::{AssetRef, StoreScope};
use crate::model::transaction::{ConvertTx, PaymentTx, TopupTx, TransferTx};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Schedule {
    /// Day of the month; a shorter month uses its last day.
    Monthly {
        day: u8,
    },
    /// 0 = Monday … 6 = Sunday.
    Weekly {
        weekday: u8,
    },
    EveryNDays {
        days: u16,
    },
}

/// The transaction an automation creates. `meta.id`, `meta.timestamp`,
/// `meta.origin` and the price stamps are ignored and filled in when it runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransactionDraft {
    Topup(TopupTx),
    Payment(PaymentTx),
    Transfer(TransferTx),
    Convert(ConvertTx),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub schedule: Schedule,
    /// First planned date, RFC3339.
    pub starts_at: String,
    pub template: TransactionDraft,
    /// Last planned date already processed (created or rejected).
    pub last_planned_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    InsufficientFunds {
        store_id: String,
        asset: AssetRef,
        required: Amount,
        available: Amount,
    },
    CreditLimitExceeded {
        store_id: String,
        limit: Amount,
        required: Amount,
    },
    /// The draft failed validation (an archived store and the like).
    Invalid { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutomationOutcome {
    Created {
        tx_id: String,
    },
    Rejected {
        reason: RejectReason,
    },
    /// The user dismissed a rejected run.
    Skipped,
}

/// One processed planned date of an automation. Never retried automatically;
/// doubles as the automation's history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    pub planned_at: String,
    pub processed_at: String,
    pub outcome: AutomationOutcome,
}

/// Upper bound on stored automation runs; the oldest are dropped.
pub const MAX_AUTOMATION_RUNS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetTarget {
    Total,
    Category(String),
    Store(StoreScope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Budget {
    pub id: String,
    pub active: bool,
    pub target: BudgetTarget,
    pub period: BudgetPeriod,
    pub limit: Amount,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Goal {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub target_amount: Amount,
    pub currency: String,
    pub starts_at: String,
    pub deadline: Option<String>,
    /// Importance, 1–5.
    pub priority: u8,
    /// What the user is ready to put aside per month.
    pub monthly_commitment: Option<Amount>,
    pub scope: StoreScope,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Reminder {
    pub id: String,
    pub text: String,
    pub due_at: String,
    pub done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlertCondition {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct PriceAlert {
    pub id: String,
    pub asset: AssetRef,
    pub condition: AlertCondition,
    pub price: Amount,
    pub currency: String,
    pub active: bool,
    pub triggered_at: Option<String>,
}

/// «If the comment or the contact name contains `pattern`, use `category`.»
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CategorizationRule {
    pub id: String,
    /// Case-insensitive substring.
    pub pattern: String,
    /// Category key.
    pub category: String,
    pub active: bool,
    /// Lower runs first.
    pub order: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UtilityKind {
    /// Billed by meter: consumption × tariff.
    Metered { unit: String, tariff: Amount },
    /// A fixed amount, re-used from last time.
    Static { amount: Amount },
    /// Rent: a monthly amount, a deposit with the first payment, a due day.
    Housing {
        amount: Amount,
        deposit: Option<Amount>,
        due_day: u8,
        automation_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct UtilityItem {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub kind: UtilityKind,
    pub currency: String,
    pub store_id: Option<String>,
    /// Category key.
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct MeterReading {
    pub id: String,
    pub item_id: String,
    pub value: Amount,
    /// YYYY-MM.
    pub period: String,
    pub recorded_at: String,
}
