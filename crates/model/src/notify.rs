//! Notifications (plan §14): what the user wants to be told about, and the
//! log of what was already told, so a restart never repeats a notice.

use serde::{Deserialize, Serialize};

use crate::vault::now_iso;

/// A kind of notice. Each one can be turned off and told ahead of time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NoticeCategory {
    Reminder,
    PriceAlert,
    Automation,
    BybitImport,
    Loan,
    Rent,
    Utilities,
    Debt,
    Deposit,
    Loyalty,
    Goal,
    Budget,
    /// Planned money going out that the forecast balance will not cover.
    Shortfall,
    Tax,
    /// A new inflation figure or a key-rate decision (fetched daily).
    Rates,
    /// The monthly capital checkup changed its colour or reasons.
    Health,
    /// A car's insurance ends or its service is due.
    Vehicle,
    /// Something about the vault itself: it was handed to another device.
    /// Always shown; there is no switch for it.
    Security,
}

impl NoticeCategory {
    pub const ALL: [NoticeCategory; 17] = [
        Self::Reminder,
        Self::PriceAlert,
        Self::Automation,
        Self::BybitImport,
        Self::Loan,
        Self::Rent,
        Self::Utilities,
        Self::Debt,
        Self::Deposit,
        Self::Loyalty,
        Self::Goal,
        Self::Budget,
        Self::Shortfall,
        Self::Tax,
        Self::Rates,
        Self::Health,
        Self::Vehicle,
    ];

    /// Days ahead a notice of this kind comes by default.
    pub fn default_lead_days(self) -> u16 {
        match self {
            Self::Loan | Self::Rent | Self::Utilities | Self::Debt | Self::Shortfall => 1,
            Self::Deposit | Self::Loyalty => 7,
            Self::Tax | Self::Goal | Self::Vehicle => 14,
            _ => 0,
        }
    }
}

/// One category's switches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NoticeCategoryCfg {
    pub category: NoticeCategory,
    pub enabled: bool,
    /// How many days before the date the notice comes; 0 = on the day.
    pub lead_days: u16,
    /// Whether reminders of this kind are created from the data by themselves.
    pub auto_reminders: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NotificationSettings {
    /// Show notices from the system as well as inside the app.
    pub system: bool,
    /// While the vault is locked: "Loan payment" without the name and the sum.
    pub hide_details_when_locked: bool,
    /// `HH:MM`, local: when automatic reminders are due.
    pub default_time: String,
    /// One entry per category; a missing one has its defaults.
    pub categories: Vec<NoticeCategoryCfg>,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            system: true,
            hide_details_when_locked: false,
            default_time: "10:00".into(),
            categories: Vec::new(),
        }
    }
}

impl NotificationSettings {
    /// The switches of a category, its defaults when not set.
    pub fn category(&self, category: NoticeCategory) -> NoticeCategoryCfg {
        self.categories
            .iter()
            .find(|cfg| cfg.category == category)
            .cloned()
            .unwrap_or(NoticeCategoryCfg {
                category,
                enabled: true,
                lead_days: category.default_lead_days(),
                auto_reminders: true,
            })
    }
}

/// Upper bound on the notification log; the oldest entries are dropped.
pub const MAX_NOTICE_LOG: usize = 500;

/// A notice that was shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NoticeRecord {
    /// Source and occurrence, e.g. `reminder:<id>:2026-11-01T07:00:00Z`.
    pub key: String,
    pub category: NoticeCategory,
    pub title: String,
    pub body: String,
    /// In-app route the notice opens, e.g. `/automations/reminders`.
    pub link: Option<String>,
    /// RFC3339: when it was due.
    pub at: String,
    /// RFC3339: when it was shown.
    pub delivered_at: String,
    pub read: bool,
}

/// Short titles in the user's language.
pub fn notice_title(category: NoticeCategory, ru: bool) -> &'static str {
    use NoticeCategory::*;
    match (category, ru) {
        (Reminder, true) => "Напоминание",
        (Reminder, false) => "Reminder",
        (PriceAlert, true) => "Ценовой алерт",
        (PriceAlert, false) => "Price alert",
        (Automation, true) => "Автоматизация",
        (Automation, false) => "Automation",
        (BybitImport, true) => "Импорт Bybit",
        (BybitImport, false) => "Bybit import",
        (Loan, true) => "Платёж по кредиту",
        (Loan, false) => "Loan payment",
        (Rent, true) => "Аренда",
        (Rent, false) => "Rent",
        (Utilities, true) => "ЖКХ",
        (Utilities, false) => "Utilities",
        (Debt, true) => "Долг",
        (Debt, false) => "Debt",
        (Deposit, true) => "Вклад",
        (Deposit, false) => "Deposit",
        (Loyalty, true) => "Бонусные баллы",
        (Loyalty, false) => "Loyalty points",
        (Goal, true) => "Цель",
        (Goal, false) => "Goal",
        (Budget, true) => "Бюджет",
        (Budget, false) => "Budget",
        (Shortfall, true) => "Не хватит денег",
        (Shortfall, false) => "Money will run short",
        (Tax, true) => "Налоги",
        (Tax, false) => "Taxes",
        (Rates, true) => "Инфляция и ставка",
        (Rates, false) => "Inflation and rates",
        (Health, true) => "Чекап капитала",
        (Health, false) => "Capital checkup",
        (Security, true) => "Безопасность",
        (Security, false) => "Security",
        (Vehicle, true) => "Машина",
        (Vehicle, false) => "Car",
    }
}

/// A notice about to be shown, stamped `delivered_at`.
pub fn notice_at(
    category: NoticeCategory,
    key: String,
    title: &str,
    body: String,
    link: Option<String>,
    at: String,
    delivered_at: String,
) -> NoticeRecord {
    debug_assert!(
        link.as_deref().is_none_or(mwt_base::routes::is_known_link),
        "unknown notice link {link:?}"
    );
    NoticeRecord {
        key,
        category,
        title: title.to_string(),
        body,
        link,
        at,
        delivered_at,
        read: false,
    }
}

/// A notice about to be shown, stamped now.
pub fn notice(
    category: NoticeCategory,
    key: String,
    title: &str,
    body: String,
    link: Option<String>,
    at: String,
) -> NoticeRecord {
    notice_at(category, key, title, body, link, at, now_iso())
}
