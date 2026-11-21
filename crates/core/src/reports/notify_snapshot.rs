//! What the app may tell while the vault is locked (plan §14, option B).
//!
//! At unlock and while the vault is open, the app takes a snapshot of every
//! notice ahead — each date of each reminder, the thresholds of price alerts
//! with the rates to check them in, the automations that will be due — and
//! keeps reminding from it after the vault is locked. At the next unlock what
//! was shown meanwhile is merged into the vault's log, so nothing repeats.
//! The snapshot is stored only encrypted by the app (key in the OS keyring).

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::calendar;
use crate::model::notify::{NoticeCategory, NoticeRecord};
use crate::model::planning::AlertCondition;
use crate::model::store::AssetRef;
use crate::model::vault::now_iso;
use crate::time::secs_or_now;

use crate::kernel::books::Books;
use crate::model::notify::{notice, notice_title};
use crate::planning::agenda::UpcomingKind;
use crate::planning::agenda::{next_occurrence, reminder_secs};
use crate::planning::auto_reminders::category_of;
use crate::planning::upcoming::upcoming;

/// How far ahead a snapshot looks.
pub const SNAPSHOT_DAYS: u32 = 60;

/// Most dates of one repeating reminder a snapshot holds.
const MAX_OCCURRENCES: usize = 60;

/// A notice due at a moment, ready to show without the vault.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScheduledNotice {
    pub key: String,
    pub category: NoticeCategory,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    /// RFC3339.
    pub at: String,
}

/// A price alert to check with no vault: its threshold and what to say.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotAlert {
    pub alert_id: String,
    pub asset: AssetRef,
    pub condition: AlertCondition,
    pub price: Amount,
    pub currency: String,
    pub body: String,
}

/// What an alert looked like when the locked app checked it: the threshold
/// is the smallest reliable base (an alert has no edit stamp of its own that
/// the app side can see). A firing seen against a threshold that has changed
/// since says nothing about the new one.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AlertBase {
    pub condition: AlertCondition,
    pub price: Amount,
    pub currency: String,
}

impl SnapshotAlert {
    /// The threshold this alert was checked with.
    pub fn base(&self) -> AlertBase {
        AlertBase {
            condition: self.condition,
            price: self.price,
            currency: self.currency.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NotificationSnapshot {
    pub vault_id: String,
    pub created_at: String,
    pub system: bool,
    /// Show "Loan payment" without the name and the sum.
    pub hide_details: bool,
    pub notices: Vec<ScheduledNotice>,
    pub alerts: Vec<SnapshotAlert>,
    /// Units of a currency per USD, the latest known: to check an alert set
    /// in rubles against a price in dollars.
    pub fx: HashMap<String, Amount>,
    /// Title shown for a notice whose details are hidden.
    pub hidden_body: String,
    /// The device was a sync client when this was written: the main shows
    /// the notices, so a locked client fires none. An older snapshot has no
    /// field and counts as not a client. The default stays after the v4
    /// reset: notify-snapshot.bin outlives app updates and is never
    /// transitioned, so a file from an older build must still load.
    #[serde(default)]
    pub role_client: bool,
}

impl NotificationSnapshot {
    /// The scheduled notices whose time came and that were not shown yet,
    /// stamped as shown now.
    pub fn due(&self, now: i64, shown: &HashSet<String>) -> Vec<NoticeRecord> {
        if self.role_client {
            return Vec::new();
        }
        self.notices
            .iter()
            .filter(|scheduled| {
                secs_or_now(&scheduled.at) <= now && !shown.contains(&scheduled.key)
            })
            .map(|scheduled| {
                self.record(
                    scheduled.category,
                    &scheduled.key,
                    &scheduled.title,
                    &scheduled.body,
                    scheduled.link.clone(),
                    &scheduled.at,
                )
            })
            .collect()
    }

    /// Alerts crossed by fresh USD prices (by `AssetRef::key`), each once.
    pub fn alert_hits(
        &self,
        prices_usd: &HashMap<String, Amount>,
        fired: &HashSet<String>,
        ru: bool,
    ) -> Vec<(String, NoticeRecord)> {
        if self.role_client {
            return Vec::new();
        }
        self.alerts
            .iter()
            .filter(|alert| !fired.contains(&alert.alert_id))
            .filter_map(|alert| {
                let usd = *prices_usd.get(&alert.asset.key())?;
                let currency = alert.currency.to_lowercase();
                let price = if currency == "usd" {
                    usd
                } else {
                    usd * *self.fx.get(&currency)?
                };
                let hit = match alert.condition {
                    AlertCondition::Above => price >= alert.price,
                    AlertCondition::Below => price <= alert.price,
                };
                hit.then(|| {
                    let at = now_iso();
                    let record = self.record(
                        NoticeCategory::PriceAlert,
                        &format!("alert:{}:{at}", alert.alert_id),
                        notice_title(NoticeCategory::PriceAlert, ru),
                        &alert.body,
                        Some(crate::routes::AUTOMATIONS_ALERTS.into()),
                        &at,
                    );
                    (alert.alert_id.clone(), record)
                })
            })
            .collect()
    }

    fn record(
        &self,
        category: NoticeCategory,
        key: &str,
        title: &str,
        body: &str,
        link: Option<String>,
        at: &str,
    ) -> NoticeRecord {
        let body = if self.hide_details {
            self.hidden_body.clone()
        } else {
            body.to_string()
        };
        notice(category, key.to_string(), title, body, link, at.to_string())
    }
}

/// Every notice ahead for the next [`SNAPSHOT_DAYS`], to be told while
/// the vault is locked.
pub fn notification_snapshot(
    books: &Books,
    now: i64,
    zone: &str,
    ru: bool,
) -> NotificationSnapshot {
    let doc = books.doc();
    let settings = &doc.settings.notifications;
    let horizon = now + i64::from(SNAPSHOT_DAYS) * 86_400;
    let mut notices = Vec::new();

    for reminder in doc
        .reminders
        .iter()
        .filter(|reminder| !reminder.done && !reminder.muted)
    {
        let category = reminder
            .source
            .as_ref()
            .map_or(NoticeCategory::Reminder, |source| category_of(source.kind));
        if !settings.category(category).enabled {
            continue;
        }
        let mut at = calendar::iso(reminder_secs(reminder));
        for _ in 0..MAX_OCCURRENCES {
            let secs = secs_or_now(&at);
            if secs > horizon {
                break;
            }
            notices.push(ScheduledNotice {
                key: format!("reminder:{}:{at}", reminder.id),
                category,
                title: notice_title(category, ru).to_string(),
                body: reminder.text.clone(),
                link: Some(crate::routes::AUTOMATIONS_REMINDERS.into()),
                at: at.clone(),
            });
            let Some(next) = reminder
                .repeat
                .and_then(|repeat| next_occurrence(repeat, &at, secs))
            else {
                break;
            };
            at = next;
        }
    }

    // An automation cannot run while the vault is locked: say it is waiting.
    if settings.category(NoticeCategory::Automation).enabled {
        for item in upcoming(books, SNAPSHOT_DAYS, now, zone)
            .into_iter()
            .filter(|item| item.kind == UpcomingKind::Automation)
        {
            notices.push(ScheduledNotice {
                key: format!("automation-locked:{}:{}", item.source_id, item.date),
                category: NoticeCategory::Automation,
                title: notice_title(NoticeCategory::Automation, ru).to_string(),
                body: if ru {
                    format!(
                        "«{}» ждёт: разблокируйте vault, чтобы она выполнилась",
                        item.title
                    )
                } else {
                    format!("«{}» is waiting: unlock the vault to run it", item.title)
                },
                link: Some(crate::routes::AUTOMATIONS.into()),
                at: item.at.clone(),
            });
        }
    }

    let alerts = if settings.category(NoticeCategory::PriceAlert).enabled {
        doc.price_alerts
            .iter()
            .filter(|alert| alert.active && !alert.is_triggered())
            .map(|alert| {
                let word = match (alert.condition, ru) {
                    (AlertCondition::Above, true) => "выше",
                    (AlertCondition::Below, true) => "ниже",
                    (AlertCondition::Above, false) => "above",
                    (AlertCondition::Below, false) => "below",
                };
                SnapshotAlert {
                    alert_id: alert.id.clone(),
                    asset: alert.asset.clone(),
                    condition: alert.condition,
                    price: alert.price,
                    currency: alert.currency.to_lowercase(),
                    body: format!(
                        "{} {word} {} {}",
                        alert.asset.symbol,
                        alert.price,
                        alert.currency.to_uppercase()
                    ),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let mut fx: HashMap<String, (String, Amount)> = HashMap::new();
    for point in &doc.fx_cache {
        let newer = fx.get(&point.currency).is_none_or(|(ts, _)| point.ts > *ts);
        if newer && point.per_usd.is_positive() {
            fx.insert(point.currency.clone(), (point.ts.clone(), point.per_usd));
        }
    }

    NotificationSnapshot {
        vault_id: doc.id.clone(),
        created_at: calendar::iso(now),
        system: settings.system,
        hide_details: settings.hide_details_when_locked,
        notices,
        alerts,
        fx: fx
            .into_iter()
            .map(|(code, (_, rate))| (code, rate))
            .collect(),
        hidden_body: if ru {
            "Подробности — после разблокировки".into()
        } else {
            "Details after unlocking".into()
        },
        role_client: false,
    }
}
