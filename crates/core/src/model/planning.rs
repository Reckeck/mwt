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

/// A ready-made automation a wallet offers: it decides the operation kind and
/// the category, the user fills in only the amount and the date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutomationPreset {
    /// A regular spending (rent of a parking spot, a subscription).
    Expense,
    /// Topping the wallet up from another one of yours.
    TopUp,
    Salary,
    CardPayment,
    ToSavings,
    DepositInterest,
    /// Dollar-cost averaging: buy the same amount on a schedule.
    DcaBuy,
    StakingReward,
    /// Moving money between exchange accounts (spot ↔ futures).
    MarketTransfer,
    RentIncome,
    PropertyTax,
    Insurance,
    Utilities,
    LoanPayment,
}

impl AutomationPreset {
    pub fn tx_kind(self) -> crate::model::transaction::TxKind {
        use crate::model::transaction::TxKind;
        match self {
            Self::Salary | Self::DepositInterest | Self::StakingReward | Self::RentIncome => {
                TxKind::Topup
            }
            Self::TopUp | Self::ToSavings | Self::MarketTransfer | Self::LoanPayment => {
                TxKind::Transfer
            }
            Self::DcaBuy => TxKind::Convert,
            Self::Expense
            | Self::CardPayment
            | Self::PropertyTax
            | Self::Insurance
            | Self::Utilities => TxKind::Payment,
        }
    }

    /// Category key the operation gets, from the default set.
    pub fn category(self) -> Option<&'static str> {
        match self {
            Self::Salary => Some("salary"),
            Self::DepositInterest => Some("deposit_interest"),
            Self::StakingReward => Some("income_other"),
            Self::RentIncome => Some("income_other"),
            Self::ToSavings => Some("savings"),
            Self::DcaBuy => Some("trading"),
            Self::MarketTransfer | Self::TopUp => Some("transfer"),
            Self::LoanPayment => Some("loan_payment"),
            Self::PropertyTax => Some("taxes"),
            Self::Insurance => Some("insurance"),
            Self::Utilities => Some("utilities"),
            Self::Expense | Self::CardPayment => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub active: bool,
    /// The ready-made kind it was created from, for the wallet's list.
    #[serde(default)]
    pub preset: Option<AutomationPreset>,
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
    /// The property (a top-level `property` wallet) the bill belongs to.
    #[serde(default)]
    pub property_id: String,
    /// The wallet the bill is paid from.
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

/// An envelope: a named, earmarked part of one asset's balance in one store
/// («отпуск», «налоги»). Nothing physically moves — the store still holds the
/// money; envelopes only mark how much of it is spoken for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Envelope {
    pub id: String,
    pub store_id: String,
    pub name: String,
    pub asset: AssetRef,
    /// How much is earmarked now, in `asset`.
    pub allocated: Amount,
    /// What the envelope is filling up to, in `asset`.
    pub target: Option<Amount>,
    /// A goal whose progress is this envelope.
    pub goal_id: Option<String>,
    pub created_at: String,
}
