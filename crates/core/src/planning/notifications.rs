//! Notifications (plan §14): what is worth telling the user now, and the log
//! of what was already told. Every notice has a key — its source and the
//! occurrence — so a restart, a second window or the locked-vault scheduler
//! never shows the same one twice. The rules here read a document and take
//! the moment as an argument; the scenarios that log and sweep are in
//! `usecases::planning::notifications`.

use std::collections::HashSet;

use crate::Amount;
use crate::calendar;
use crate::model::notify::{MAX_NOTICE_LOG, NoticeCategory, NoticeRecord};
pub use crate::model::notify::{notice_at, notice_title};
use crate::model::planning::Reminder;
use crate::model::vault::VaultDoc;
use crate::planning::agenda::{UpcomingItem, reminder_is_due, reminder_secs};
use crate::planning::auto_reminders::category_of;
use crate::planning::budget::BudgetStatus;

/// A reminder older than this is not announced any more: after a long time
/// away the user gets the list, not a flood.
const REMINDER_STALE_SECS: i64 = 7 * 86_400;

/// Share of a budget's limit that warns.
const BUDGET_WARN_PERCENT: i64 = 90;

/// An amount for a notice text: at most two decimals, thousands apart
/// (`22767.9636…` is `22 767,96`, `15000` is `15 000`).
pub(crate) fn money_text(amount: Amount, ru: bool) -> String {
    let rounded = amount.0.round_dp(2).normalize().to_string();
    let (sign, digits) = rounded
        .strip_prefix('-')
        .map_or(("", rounded.as_str()), |rest| ("-", rest));
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push('\u{a0}');
        }
        grouped.push(digit);
    }
    let point = if ru { ',' } else { '.' };
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{point}{fraction:0<2}")
    }
}

/// Whether notices of this kind are wanted at all.
pub fn notices_enabled(doc: &VaultDoc, category: NoticeCategory) -> bool {
    doc.settings.notifications.category(category).enabled
}

/// Every notice shown so far, newest first.
pub fn notifications(doc: &VaultDoc) -> Vec<NoticeRecord> {
    let mut out = doc.notification_log.clone();
    out.reverse();
    out
}

pub fn unread_notifications(doc: &VaultDoc) -> u32 {
    doc.notification_log
        .iter()
        .filter(|record| !record.read)
        .count() as u32
}

/// Mark notices of the log read: the ones named, or all of them.
pub fn mark_read(log: &mut [NoticeRecord], keys: Option<&[String]>) {
    for record in log {
        if keys.is_none_or(|keys| keys.contains(&record.key)) {
            record.read = true;
        }
    }
}

/// The records the log does not know yet and that are wanted, each key once.
pub fn fresh_notices(doc: &VaultDoc, records: Vec<NoticeRecord>) -> Vec<NoticeRecord> {
    let mut known: HashSet<String> = doc
        .notification_log
        .iter()
        .map(|record| record.key.clone())
        .collect();
    records
        .into_iter()
        .filter(|record| notices_enabled(doc, record.category))
        .filter(|record| known.insert(record.key.clone()))
        .collect()
}

/// Add notices to the end of the log and drop the oldest past [`MAX_NOTICE_LOG`].
pub fn append_notices(log: &mut Vec<NoticeRecord>, fresh: &[NoticeRecord]) {
    log.extend(fresh.iter().cloned());
    let overflow = log.len().saturating_sub(MAX_NOTICE_LOG);
    log.drain(..overflow);
}

/// Reminders whose time came and that are not stale, as notices.
pub fn reminder_notices(
    reminders: Vec<Reminder>,
    now: i64,
    ru: bool,
    delivered_at: &str,
) -> Vec<NoticeRecord> {
    let mut out = Vec::new();
    for reminder in reminders
        .into_iter()
        .filter(|reminder| reminder_is_due(reminder, now))
    {
        let secs = reminder_secs(&reminder);
        if now - secs > REMINDER_STALE_SECS {
            continue;
        }
        let category = reminder
            .source
            .as_ref()
            .map_or(NoticeCategory::Reminder, |source| category_of(source.kind));
        let at = calendar::iso(secs);
        out.push(notice_at(
            category,
            format!("reminder:{}:{at}", reminder.id),
            notice_title(category, ru),
            reminder.text.clone(),
            Some(format!("open:reminder:{}", reminder.id)),
            at,
            delivered_at.to_string(),
        ));
    }
    out
}

/// Days ahead a planned payment is checked for a shortfall.
pub fn shortfall_lead_days(doc: &VaultDoc) -> u32 {
    let lead = i64::from(
        doc.settings
            .notifications
            .category(NoticeCategory::Shortfall)
            .lead_days,
    );
    lead.clamp(0, 60) as u32
}

/// Planned money going out that the balance will not cover, as notices.
pub fn shortfall_notices(
    items: Vec<UpcomingItem>,
    ru: bool,
    delivered_at: &str,
) -> Vec<NoticeRecord> {
    let mut out = Vec::new();
    for item in items {
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
        out.push(notice_at(
            NoticeCategory::Shortfall,
            format!("shortfall:{:?}:{}:{}", item.kind, item.source_id, item.date),
            notice_title(NoticeCategory::Shortfall, ru),
            body,
            item.store_id
                .map(|id| format!("{}:{id}", crate::routes::WALLET)),
            item.at.clone(),
            delivered_at.to_string(),
        ));
    }
    out
}

/// Budgets near or over their limit, as notices.
pub fn budget_notices(
    statuses: Vec<BudgetStatus>,
    now: i64,
    ru: bool,
    delivered_at: &str,
) -> Vec<NoticeRecord> {
    let mut out = Vec::new();
    let hundred = Amount::new(rust_decimal::Decimal::from(100));
    for status in statuses {
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
        let (spent, limit, remaining) = (
            money_text(status.spent, ru),
            money_text(status.budget.limit, ru),
            money_text(status.remaining, ru),
        );
        let body = match (level, ru) {
            (100, true) => format!("Лимит превышен: потрачено {spent} из {limit} {code}"),
            (100, false) => format!("Over the limit: {spent} of {limit} {code} spent"),
            (_, true) => format!("Почти исчерпан: {percent}% — осталось {remaining} {code}"),
            (_, false) => format!("Nearly used up: {percent}%, {remaining} {code} left"),
        };
        out.push(notice_at(
            NoticeCategory::Budget,
            format!(
                "budget:{}:{}:{level}",
                status.budget.id, status.period_start
            ),
            notice_title(NoticeCategory::Budget, ru),
            body,
            Some(format!("open:budget:{}", status.budget.id)),
            calendar::iso(now),
            delivered_at.to_string(),
        ));
    }
    out
}
