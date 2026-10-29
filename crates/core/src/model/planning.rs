//! Planning data: automations, budgets, goals, reminders, alerts, rules and
//! utilities. Defined now so the vault schema breaks once (plan D11); the
//! features that use most of it arrive in later phases.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::store::{AssetRef, StoreScope};
use crate::model::transaction::{ConvertTx, PaymentTx, TopupTx, TransferTx};

pub use crate::calendar::workdays::WorkdayShift;

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
    /// A car's regular service.
    VehicleService,
    Fuel,
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
            | Self::Utilities
            | Self::VehicleService
            | Self::Fuel => TxKind::Payment,
        }
    }

    /// Category key the operation gets, from the default set.
    pub fn category(self) -> Option<&'static str> {
        match self {
            Self::Salary => Some("salary"),
            Self::DepositInterest => Some("deposit_interest"),
            Self::StakingReward => Some("staking"),
            Self::RentIncome => Some("rent_income"),
            Self::ToSavings => Some("savings"),
            Self::DcaBuy => Some("trading"),
            Self::MarketTransfer | Self::TopUp => Some("transfer"),
            Self::LoanPayment => Some("loan_payment"),
            Self::PropertyTax => Some("taxes"),
            Self::Insurance => Some("insurance"),
            Self::Utilities => Some("utilities"),
            Self::VehicleService => Some("repair"),
            Self::Fuel => Some("fuel"),
            Self::Expense | Self::CardPayment => None,
        }
    }
}

/// Whether an automation books the same amount each time.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AmountMode {
    /// The template's amount, every time.
    #[default]
    Fixed,
    /// Differs every time (a salary): booked at the average of the latest
    /// corrected amounts, marked approximate until the user puts in the real one.
    Varies,
}

/// What the user set in the automation form. Stored on the workflow the
/// automation runs as (`Workflow::automation`); the graph is built from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AutomationSpec {
    /// The ready-made kind it was created from, for the wallet's list.
    #[serde(default)]
    pub preset: Option<AutomationPreset>,
    pub schedule: Schedule,
    /// First planned date, RFC3339.
    pub starts_at: String,
    pub template: TransactionDraft,
    #[serde(default)]
    pub amount_mode: AmountMode,
    /// Where a planned date on a weekend or a holiday goes.
    #[serde(default)]
    pub workday_shift: WorkdayShift,
    /// The property its operations belong to (plan G4).
    #[serde(default)]
    pub property_id: Option<String>,
    /// The local time it fires, «HH:MM»; `None` — 09:00, as before.
    #[serde(default)]
    pub time: Option<String>,
    /// It asks "yes" before it books the operation.
    #[serde(default)]
    pub asks: bool,
    /// An income only: the month it is for, in months from the one it is
    /// planned in (-1 — the previous, 0 — this one); `None` — not set.
    #[serde(default)]
    #[ts(optional)]
    pub for_month_offset: Option<i8>,
}

/// An automation as the app shows it: a workflow built from an
/// [`AutomationSpec`]. Not stored as such (the vault keeps the workflow).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Automation {
    /// The workflow's id.
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
    /// Last planned moment already processed (created, rejected or skipped).
    pub last_planned_at: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub amount_mode: AmountMode,
    #[serde(default)]
    pub workday_shift: WorkdayShift,
    /// With [`AmountMode::Varies`]: the amount the next run books (the
    /// average of the latest corrected ones); `None` — the template's.
    #[serde(default)]
    pub estimate: Option<Amount>,
    /// The property its operations belong to (plan G4).
    #[serde(default)]
    pub property_id: Option<String>,
    /// The local time it fires, «HH:MM»; `None` — 09:00, as before.
    #[serde(default)]
    pub time: Option<String>,
    /// It asks "yes" before it books the operation.
    #[serde(default)]
    pub asks: bool,
    /// An income only: the month it is for, in months from the one it is
    /// planned in (-1 — the previous, 0 — this one); `None` — not set.
    #[serde(default)]
    #[ts(optional)]
    pub for_month_offset: Option<i8>,
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
    /// The workflow run's id.
    pub id: String,
    pub automation_id: String,
    pub planned_at: String,
    pub processed_at: String,
    pub outcome: AutomationOutcome,
    /// The operation was booked with an approximate amount not corrected yet.
    #[serde(default)]
    pub estimated: bool,
    /// What the operation it made moves, and in which asset.
    #[serde(default)]
    pub amount: Option<Amount>,
    #[serde(default)]
    pub symbol: String,
}

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
    /// Notify when the period's spending nears the limit. Older vaults: on.
    #[serde(default = "warn_on")]
    pub warn: bool,
}

fn warn_on() -> bool {
    true
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
    /// The calculator scenario the goal follows: "Planned by today".
    #[serde(default)]
    pub scenario_id: Option<String>,
    /// What the savings are expected to earn, percent a year, compounded
    /// monthly: the monthly need and the forecast count on it (plan B).
    #[serde(default)]
    pub expected_return: Option<Amount>,
}

/// Where a calculator scenario takes its numbers from (plan A, "From your data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ScenarioSource {
    /// The wallets whose value is the start amount and whose deposit terms
    /// give the rate.
    pub scope: StoreScope,
    /// Finished months the average income is taken over: 3, 6 or 12.
    pub income_months: u8,
    /// Share of that income contributed each month, percent.
    pub income_pct: Amount,
}

/// A saved investment calculation, comparable with others and followed by
/// goals made from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CalcScenario {
    pub id: String,
    pub name: String,
    pub input: crate::calc::invest::InvestInput,
    #[serde(default)]
    pub source: Option<ScenarioSource>,
    /// RFC3339 of the last "Refresh from data": the numbers are a snapshot.
    #[serde(default)]
    pub prefilled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// The capital checkup's light (plan C4), worst first in `Ord` terms last.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    ts_rs::TS,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HealthColor {
    /// Too little history to judge: no finished month and no hand-entered month.
    NoData,
    Green,
    Yellow,
    Red,
}

/// Why the checkup shows its colour; each comes with a first step.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    ts_rs::TS,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HealthReasonCode {
    /// No finished month of history and no hand-entered month.
    NoHistory,
    /// A loan above the key rate, or a credit card with a debt.
    CostlyLoan,
    /// A loan with no rate: it cannot be classed.
    LoanWithoutRate,
    /// Payments take more than half of income.
    DtiHigh,
    /// Payments take 30–50 % of income.
    DtiElevated,
    /// No emergency fund chosen yet.
    FundNotSet,
    /// The fund covers less than a month.
    FundBelowOne,
    /// The fund covers less than three months.
    FundBelowThree,
    /// The last three months kept less than the savings plan.
    SavingsBelowPlan,
    /// Spending grew as fast as income or faster.
    LifestyleCreep,
}

/// What the last monthly checkup notice said (plan C4, decision 28).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct HealthSnapshot {
    /// `YYYY-MM` of the check.
    pub month: String,
    pub color: HealthColor,
    pub reasons: Vec<HealthReasonCode>,
}

/// The emergency fund (plan C1): savings accounts set aside for bad times,
/// measured in months of mandatory spending. No stores = no fund.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(default)]
pub struct EmergencyFund {
    /// An explicit list of `Purpose::Savings` stores, `include_children`
    /// off: an empty `StoreScope` would mean every store.
    pub scope: StoreScope,
    /// How many months of mandatory spending the fund should cover.
    pub target_months: u8,
    /// Finished months mandatory spending is averaged over.
    pub avg_months: u8,
}

impl Default for EmergencyFund {
    fn default() -> Self {
        Self {
            scope: StoreScope {
                store_ids: Vec::new(),
                include_children: false,
            },
            target_months: 6,
            avg_months: 6,
        }
    }
}

impl EmergencyFund {
    pub fn is_set(&self) -> bool {
        !self.scope.store_ids.is_empty()
    }
}

/// A basket of the portfolio with its target share (plan F). The shares of
/// all baskets sum to 100.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AssetClass {
    pub id: String,
    /// Empty on a preset basket nobody renamed yet: the UI shows its own name.
    pub name: String,
    /// Target share, percent.
    pub target_pct: Amount,
    /// Above its target the basket gets no top-ups (the risky part).
    pub cap: bool,
}

/// An asset placed in a basket by hand; beats the store's basket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AssetClassLink {
    /// `AssetRef::key()`.
    pub asset_key: String,
    pub class_id: String,
}

/// Unit of a reminder's repeat interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepeatUnit {
    Day,
    Week,
    Month,
    Year,
}

/// «Every `every` `unit`s»: a monthly bill is `{ every: 1, unit: month }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ReminderRepeat {
    pub every: u16,
    pub unit: RepeatUnit,
}

impl ReminderRepeat {
    /// The occurrence after `from`. A month too short for the day keeps its
    /// last day, the way a bank moves a payment date.
    pub fn advance(
        self,
        from: chrono::DateTime<chrono::Utc>,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        self.nth(from, 1)
    }

    /// The `count`-th occurrence after `anchor`, counted from the anchor
    /// itself so the 31st stays the 31st after a short month.
    pub fn nth(
        self,
        anchor: chrono::DateTime<chrono::Utc>,
        count: u32,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        let steps = u32::from(self.every.max(1)).checked_mul(count)?;
        match self.unit {
            RepeatUnit::Day => anchor.checked_add_days(chrono::Days::new(u64::from(steps))),
            RepeatUnit::Week => anchor.checked_add_days(chrono::Days::new(u64::from(steps) * 7)),
            RepeatUnit::Month => anchor.checked_add_months(chrono::Months::new(steps)),
            RepeatUnit::Year => {
                anchor.checked_add_months(chrono::Months::new(steps.checked_mul(12)?))
            }
        }
    }
}

/// What an automatic reminder is made from (plan §15). Its `id` is the id of
/// the store, utility item, contact debt or goal it follows.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReminderSourceKind {
    LoanPayment,
    CreditCardPayment,
    Rent,
    Utilities,
    MeterReadings,
    DepositEnd,
    PdsContribution,
    LoyaltyExpiry,
    Debt,
    GoalSaving,
    GoalDeadline,
    PropertyTax,
    InterestTax,
    /// A car's insurance (ОСАГО or КАСКО) ends.
    VehicleInsurance,
    /// A car's service is due by date or by mileage.
    VehicleService,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ReminderSource {
    pub kind: ReminderSourceKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Reminder {
    pub id: String,
    pub text: String,
    /// RFC3339: the date and the time it is due.
    pub due_at: String,
    pub done: bool,
    /// A repeating reminder moves to its next date when done instead of
    /// staying done.
    #[serde(default)]
    pub repeat: Option<ReminderRepeat>,
    /// Set on a reminder the app keeps in step with its source.
    #[serde(default)]
    pub source: Option<ReminderSource>,
    /// An automatic reminder the user turned off; it is kept so it is not
    /// created again.
    #[serde(default)]
    pub muted: bool,
    /// The user changed the time of an automatic reminder: the sync keeps it.
    #[serde(default)]
    pub user_edited: bool,
    /// Put off until this moment (RFC3339); it is not due before.
    #[serde(default)]
    pub snoozed_until: Option<String>,
    /// An automatic reminder: the schedule it was last placed by (the day,
    /// how far ahead, the time). Only a changed schedule moves it again, so a
    /// sync never undoes «done».
    #[serde(default)]
    pub source_key: String,
    /// "Button in the reminder" (Auto-2 · 10): "Pay" opens the payment
    /// form filled with this wallet and category.
    #[serde(default)]
    pub pay: Option<ReminderPay>,
}

/// What "Pay" in a reminder fills the payment form with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ReminderPay {
    #[serde(default)]
    pub store_id: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

impl Reminder {
    pub fn new(text: &str, due_at: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.trim().to_string(),
            due_at: due_at.to_string(),
            done: false,
            repeat: None,
            source: None,
            muted: false,
            user_edited: false,
            snoozed_until: None,
            source_key: String::new(),
            pay: None,
        }
    }
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
    /// The exchange whose terminal set the alert; `None` for a vault-wide one.
    #[serde(default)]
    #[ts(optional)]
    pub store_id: Option<String>,
    /// Fires again each time the price crosses back: re-armed once the
    /// condition stops holding. Off — fires once and stays triggered.
    #[serde(default)]
    pub repeats: bool,
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
    /// Billed by meter: consumption × tariff, or the user's own formula.
    Metered {
        unit: String,
        tariff: Amount,
        /// The bill by a formula (`utilities_formula`); `None` = consumption × tariff.
        #[serde(default)]
        formula: Option<String>,
        /// Tariff zones of a multi-rate meter ("day", "night"): a reading per
        /// zone; empty = one reading.
        #[serde(default)]
        zones: Vec<String>,
    },
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
    /// Day of the month the meter was read (1–31); `None` = the month's end.
    /// A month may hold several readings: moving out on the 15th and the
    /// next flat's first reading the same day.
    #[serde(default)]
    pub day: Option<u8>,
    /// The tariff zone of a multi-rate meter; `None` for a one-rate one.
    #[serde(default)]
    pub zone: Option<String>,
}

impl MeterReading {
    /// Order in time: by month, then by day. A reading without a day (an
    /// older vault, or a bill paid without one) stands on the day it was
    /// entered when that falls in its month, else at the month's end — so a
    /// reading taken later in the same month is not mistaken for an earlier one.
    pub fn sort_key(&self) -> (String, u8) {
        (self.period.clone(), self.effective_day())
    }

    fn effective_day(&self) -> u8 {
        if let Some(day) = self.day {
            return day;
        }
        self.recorded_at
            .strip_prefix(self.period.as_str())
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|rest| rest.get(..2))
            .and_then(|day| day.parse().ok())
            .unwrap_or(u8::MAX)
    }
}

/// An envelope: a named, earmarked part of one asset's balance in one store
/// ("vacation", "Taxes"). Nothing physically moves — the store still holds the
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
