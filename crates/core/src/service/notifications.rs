//! Notifications (plan §14): what is worth telling the user now, and the log
//! of what was already told. Every notice has a key — its source and the
//! occurrence — so a restart, a second window or the locked-vault scheduler
//! never shows the same one twice.

use std::collections::HashSet;

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::model::notify::{MAX_NOTICE_LOG, NoticeCategory, NoticeRecord};
use crate::model::vault::now_iso;

use super::UnlockedVault;
use super::agenda::reminder_secs;
use super::auto_reminders::category_of;

/// A reminder older than this is not announced any more: after a long time
/// away the user gets the list, not a flood.
const REMINDER_STALE_SECS: i64 = 7 * 86_400;

/// Share of a budget's limit that warns.
const BUDGET_WARN_PERCENT: i64 = 90;

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

/// A notice about to be shown, stamped now.
pub fn notice(
    category: NoticeCategory,
    key: String,
    title: &str,
    body: String,
    link: Option<String>,
    at: String,
) -> NoticeRecord {
    NoticeRecord {
        key,
        category,
        title: title.to_string(),
        body,
        link,
        at,
        delivered_at: now_iso(),
        read: false,
    }
}

impl UnlockedVault {
    /// Whether notices of this kind are wanted at all.
    pub fn notices_enabled(&self, category: NoticeCategory) -> bool {
        self.doc.settings.notifications.category(category).enabled
    }

    /// Every notice shown so far, newest first.
    pub fn notifications(&self) -> Vec<NoticeRecord> {
        let mut out = self.doc.notification_log.clone();
        out.reverse();
        out
    }

    pub fn unread_notifications(&self) -> u32 {
        self.doc
            .notification_log
            .iter()
            .filter(|record| !record.read)
            .count() as u32
    }

    /// Mark notices read: the ones named, or all of them.
    pub fn mark_notifications_read(&mut self, keys: Option<&[String]>) -> Result<()> {
        for record in &mut self.doc.notification_log {
            if keys.is_none_or(|keys| keys.contains(&record.key)) {
                record.read = true;
            }
        }
        self.persist()
    }

    /// Add notices to the log, skipping known keys and switched-off kinds.
    /// Returns the ones that are new; persists once.
    pub fn log_notices(&mut self, records: Vec<NoticeRecord>) -> Result<Vec<NoticeRecord>> {
        let mut known: HashSet<String> = self
            .doc
            .notification_log
            .iter()
            .map(|record| record.key.clone())
            .collect();
        let fresh: Vec<NoticeRecord> = records
            .into_iter()
            .filter(|record| self.notices_enabled(record.category))
            .filter(|record| known.insert(record.key.clone()))
            .collect();
        if fresh.is_empty() {
            return Ok(fresh);
        }
        self.doc.notification_log.extend(fresh.iter().cloned());
        let overflow = self
            .doc
            .notification_log
            .len()
            .saturating_sub(MAX_NOTICE_LOG);
        self.doc.notification_log.drain(..overflow);
        self.persist()?;
        Ok(fresh)
    }

    /// What is due to be told now that was not told before: reminders whose
    /// time came, planned money going out the balance will not cover, budgets
    /// near or over their limit. They are logged before being returned.
    pub fn due_notices(&mut self, now: i64, zone: &str, ru: bool) -> Result<Vec<NoticeRecord>> {
        let mut out = Vec::new();

        for reminder in self.due_reminders(now) {
            let secs = reminder_secs(&reminder);
            if now - secs > REMINDER_STALE_SECS {
                continue;
            }
            let category = reminder
                .source
                .as_ref()
                .map_or(NoticeCategory::Reminder, |source| category_of(source.kind));
            let at = calendar::iso(secs);
            out.push(notice(
                category,
                format!("reminder:{}:{at}", reminder.id),
                notice_title(category, ru),
                reminder.text.clone(),
                Some(format!("open:reminder:{}", reminder.id)),
                at,
            ));
        }

        let lead = i64::from(
            self.doc
                .settings
                .notifications
                .category(NoticeCategory::Shortfall)
                .lead_days,
        );
        for item in self.upcoming(lead.clamp(0, 60) as u32, now, zone) {
            let Some(short) = item.shortfall.filter(|value| value.is_positive()) else {
                continue;
            };
            let code = item
                .asset
                .as_ref()
                .map(|asset| asset.symbol.clone())
                .unwrap_or_default();
            let body = if ru {
                format!("{} ({}): не хватит {short} {code}", item.title, item.date)
            } else {
                format!("{} ({}): {short} {code} short", item.title, item.date)
            };
            out.push(notice(
                NoticeCategory::Shortfall,
                format!("shortfall:{:?}:{}:{}", item.kind, item.source_id, item.date),
                notice_title(NoticeCategory::Shortfall, ru),
                body,
                item.store_id.map(|id| format!("wallet:{id}")),
                item.at.clone(),
            ));
        }

        let hundred = Amount::new(rust_decimal::Decimal::from(100));
        for status in self.budget_status(now, zone) {
            let percent = (status.used * hundred).round(0);
            let level = if percent >= hundred {
                100
            } else if status.budget.warn
                && percent >= Amount::new(rust_decimal::Decimal::from(BUDGET_WARN_PERCENT))
            {
                BUDGET_WARN_PERCENT
            } else {
                continue;
            };
            let code = status.budget.currency.to_uppercase();
            let body = match (level, ru) {
                (100, true) => format!(
                    "Лимит превышен: потрачено {} из {} {code}",
                    status.spent, status.budget.limit
                ),
                (100, false) => format!(
                    "Over the limit: {} of {} {code} spent",
                    status.spent, status.budget.limit
                ),
                (_, true) => format!(
                    "Почти исчерпан: {percent}% — осталось {} {code}",
                    status.remaining
                ),
                (_, false) => format!(
                    "Nearly used up: {percent}%, {} {code} left",
                    status.remaining
                ),
            };
            out.push(notice(
                NoticeCategory::Budget,
                format!(
                    "budget:{}:{}:{level}",
                    status.budget.id, status.period_start
                ),
                notice_title(NoticeCategory::Budget, ru),
                body,
                Some(format!("open:budget:{}", status.budget.id)),
                calendar::iso(now),
            ));
        }

        out.extend(self.health_notice(now, zone, ru)?);
        self.log_notices(out)
    }

    /// A notice shown elsewhere (an alert fired while locked, an automation
    /// run, a Bybit import): logged so it is not shown again.
    pub fn record_notice(&mut self, record: NoticeRecord) -> Result<bool> {
        if record.key.trim().is_empty() {
            return Err(Error::InvalidInput("a notice needs a key".into()));
        }
        Ok(!self.log_notices(vec![record])?.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::notify::NoticeCategoryCfg;
    use crate::pricing::history::secs_or_now;

    #[test]
    fn a_due_reminder_is_told_once_and_survives_in_the_log() {
        let (_dir, mut vault) = empty_vault();
        vault
            .create_reminder("Позвонить в банк", "2026-03-05T09:00:00Z", None)
            .unwrap();
        let now = secs_or_now("2026-03-05T09:00:30Z");
        let first = vault.due_notices(now, "UTC", true).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].category, NoticeCategory::Reminder);
        assert_eq!(first[0].body, "Позвонить в банк");
        assert!(vault.due_notices(now + 60, "UTC", true).unwrap().is_empty());
        assert_eq!(vault.unread_notifications(), 1);
        vault.mark_notifications_read(None).unwrap();
        assert_eq!(vault.unread_notifications(), 0);
        assert_eq!(vault.notifications().len(), 1);
    }

    #[test]
    fn a_switched_off_kind_is_silent_and_an_old_reminder_is_not_announced() {
        let (_dir, mut vault) = empty_vault();
        vault
            .create_reminder("Old", "2026-01-01T09:00:00Z", None)
            .unwrap();
        let now = secs_or_now("2026-03-05T09:00:00Z");
        assert!(vault.due_notices(now, "UTC", false).unwrap().is_empty());

        vault
            .create_reminder("Fresh", "2026-03-05T08:00:00Z", None)
            .unwrap();
        vault
            .doc
            .settings
            .notifications
            .categories
            .push(NoticeCategoryCfg {
                category: NoticeCategory::Reminder,
                enabled: false,
                lead_days: 0,
                auto_reminders: true,
            });
        assert!(vault.due_notices(now, "UTC", false).unwrap().is_empty());
    }

    #[test]
    fn the_log_keeps_the_newest_and_never_repeats_a_key() {
        let (_dir, mut vault) = empty_vault();
        let record = |index: usize| {
            notice(
                NoticeCategory::PriceAlert,
                format!("alert:{index}"),
                "Alert",
                String::new(),
                None,
                now_iso(),
            )
        };
        assert!(vault.record_notice(record(1)).unwrap());
        assert!(!vault.record_notice(record(1)).unwrap());
        let many: Vec<NoticeRecord> = (0..MAX_NOTICE_LOG + 10).map(record).collect();
        vault.log_notices(many).unwrap();
        assert_eq!(vault.doc.notification_log.len(), MAX_NOTICE_LOG);
        assert_eq!(
            vault.notifications()[0].key,
            format!("alert:{}", MAX_NOTICE_LOG + 9)
        );
    }
}
