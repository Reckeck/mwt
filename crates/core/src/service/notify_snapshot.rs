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
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;
use super::agenda::{UpcomingKind, next_occurrence, reminder_secs};
use super::auto_reminders::category_of;
use super::notifications::{notice, notice_title};

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
    /// field and counts as not a client.
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

impl UnlockedVault {
    /// Every notice ahead for the next [`SNAPSHOT_DAYS`], to be told while
    /// the vault is locked.
    pub fn notification_snapshot(&self, now: i64, zone: &str, ru: bool) -> NotificationSnapshot {
        let settings = &self.doc.settings.notifications;
        let horizon = now + i64::from(SNAPSHOT_DAYS) * 86_400;
        let mut notices = Vec::new();

        for reminder in self
            .doc
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
            for item in self
                .upcoming(SNAPSHOT_DAYS, now, zone)
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
            self.doc
                .price_alerts
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
        for point in &self.doc.fx_cache {
            let newer = fx.get(&point.currency).is_none_or(|(ts, _)| point.ts > *ts);
            if newer && point.per_usd.is_positive() {
                fx.insert(point.currency.clone(), (point.ts.clone(), point.per_usd));
            }
        }

        NotificationSnapshot {
            vault_id: self.doc.id.clone(),
            created_at: now_iso(),
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

    /// [`Self::apply_locked_deliveries_seen`] with no record of what the
    /// alerts looked like: every firing applies.
    pub fn apply_locked_deliveries(
        &mut self,
        records: Vec<NoticeRecord>,
        fired_alerts: &[(String, String)],
    ) -> crate::Result<()> {
        self.apply_locked_deliveries_seen(records, fired_alerts, &HashMap::new())
    }

    /// Take in what was shown while the vault was locked: logged so it never
    /// repeats; alerts that fired are marked triggered. `seen` holds each
    /// fired alert's threshold as the locked app saw it: an alert that was
    /// changed since (a newer threshold came by sync) or switched off is left
    /// alone, so the firing does not silence the new threshold. A firing with
    /// no entry in `seen` (a record kept by an older build) applies as before.
    pub fn apply_locked_deliveries_seen(
        &mut self,
        records: Vec<NoticeRecord>,
        fired_alerts: &[(String, String)],
        seen: &HashMap<String, AlertBase>,
    ) -> crate::Result<()> {
        for (alert_id, at) in fired_alerts {
            if let Some(alert) = self
                .doc
                .price_alerts
                .iter_mut()
                .find(|alert| &alert.id == alert_id && !alert.is_triggered())
            {
                let unchanged = seen.get(alert_id).is_none_or(|base| {
                    alert.active
                        && alert.condition == base.condition
                        && alert.price == base.price
                        && alert.currency.eq_ignore_ascii_case(&base.currency)
                });
                if unchanged {
                    alert.triggered_at = Some(at.clone());
                    alert.triggered_for = Some(alert.fingerprint());
                }
            }
        }
        self.log_notices(records)?;
        self.persist()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::planning::{ReminderRepeat, RepeatUnit};

    #[test]
    fn a_snapshot_holds_every_repeat_and_reminds_once_without_the_vault() {
        let (_dir, mut vault) = empty_vault();
        vault
            .create_reminder(
                "Аренда",
                "2026-03-05T09:00:00Z",
                Some(ReminderRepeat {
                    every: 1,
                    unit: RepeatUnit::Month,
                }),
            )
            .unwrap();
        let now = secs_or_now("2026-03-01T00:00:00Z");
        let snapshot = vault.notification_snapshot(now, "UTC", true);
        // March 5, April 5 within 60 days.
        assert_eq!(snapshot.notices.len(), 2);

        let mut shown = HashSet::new();
        let april = secs_or_now("2026-04-05T09:01:00Z");
        let due = snapshot.due(april, &shown);
        assert_eq!(due.len(), 2);
        shown.extend(due.iter().map(|record| record.key.clone()));
        assert!(snapshot.due(april + 60, &shown).is_empty());

        // At unlock the vault takes them in: the same notices are not told again.
        vault.apply_locked_deliveries(due, &[]).unwrap();
        vault.doc.reminders[0].due_at = "2026-04-05T09:00:00Z".into();
        assert!(
            vault
                .due_notices(april + 120, "UTC", true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_locked_client_snapshot_fires_nothing() {
        let (_dir, mut vault) = empty_vault();
        vault
            .create_reminder("Аренда", "2026-03-05T09:00:00Z", None)
            .unwrap();
        vault
            .doc
            .price_alerts
            .push(crate::model::planning::PriceAlert {
                id: "alert".into(),
                asset: btc(),
                condition: AlertCondition::Above,
                price: amount("60000"),
                currency: "usd".into(),
                active: true,
                triggered_at: None,
                triggered_for: None,
                store_id: None,
                repeats: false,
            });
        let mut snapshot =
            vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
        let later = secs_or_now("2026-03-05T10:00:00Z");
        let prices = HashMap::from([(btc().key(), amount("61000"))]);
        assert_eq!(snapshot.due(later, &HashSet::new()).len(), 1);
        assert_eq!(snapshot.alert_hits(&prices, &HashSet::new(), true).len(), 1);
        snapshot.role_client = true;
        assert!(snapshot.due(later, &HashSet::new()).is_empty());
        assert!(
            snapshot
                .alert_hits(&prices, &HashSet::new(), true)
                .is_empty()
        );
    }

    #[test]
    fn an_old_snapshot_without_role_still_fires() {
        let (_dir, mut vault) = empty_vault();
        vault
            .create_reminder("Аренда", "2026-03-05T09:00:00Z", None)
            .unwrap();
        let snapshot =
            vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
        let mut value = serde_json::to_value(&snapshot).unwrap();
        value.as_object_mut().unwrap().remove("role_client");
        let old: NotificationSnapshot = serde_json::from_value(value).unwrap();
        assert!(!old.role_client);
        assert_eq!(
            old.due(secs_or_now("2026-03-05T10:00:00Z"), &HashSet::new())
                .len(),
            1
        );
    }

    #[test]
    fn hidden_details_and_an_alert_checked_in_another_currency() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.settings.notifications.hide_details_when_locked = true;
        vault
            .create_reminder("Секретный платёж", "2026-03-05T09:00:00Z", None)
            .unwrap();
        vault.doc.fx_cache.push(crate::model::vault::FxPoint {
            currency: "rub".into(),
            per_usd: amount("100"),
            ts: "2026-03-01T00:00:00Z".into(),
            source: "test".into(),
        });
        vault
            .doc
            .price_alerts
            .push(crate::model::planning::PriceAlert {
                id: "alert".into(),
                asset: btc(),
                condition: AlertCondition::Above,
                price: amount("6000000"),
                currency: "rub".into(),
                active: true,
                triggered_at: None,
                triggered_for: None,
                store_id: None,
                repeats: false,
            });
        let snapshot =
            vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
        let due = snapshot.due(secs_or_now("2026-03-05T10:00:00Z"), &HashSet::new());
        assert!(!due[0].body.contains("Секретный"));

        let prices = HashMap::from([(btc().key(), amount("59000"))]);
        assert!(
            snapshot
                .alert_hits(&prices, &HashSet::new(), true)
                .is_empty()
        );
        let prices = HashMap::from([(btc().key(), amount("61000"))]);
        let hits = snapshot.alert_hits(&prices, &HashSet::new(), true);
        assert_eq!(hits.len(), 1);

        vault
            .apply_locked_deliveries(vec![hits[0].1.clone()], &[(hits[0].0.clone(), now_iso())])
            .unwrap();
        assert!(vault.doc.price_alerts[0].triggered_at.is_some());
        assert_eq!(vault.doc.notification_log.len(), 1);
    }

    #[test]
    fn a_locked_firing_does_not_override_a_newer_threshold() {
        let (_dir, mut vault) = empty_vault();
        vault
            .doc
            .price_alerts
            .push(crate::model::planning::PriceAlert {
                id: "alert".into(),
                asset: btc(),
                condition: AlertCondition::Above,
                price: amount("60000"),
                currency: "usd".into(),
                active: true,
                triggered_at: None,
                triggered_for: None,
                store_id: None,
                repeats: false,
            });
        let snapshot =
            vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
        let prices = HashMap::from([(btc().key(), amount("61000"))]);
        let hits = snapshot.alert_hits(&prices, &HashSet::new(), true);
        let fired = [(hits[0].0.clone(), now_iso())];
        let seen = HashMap::from([(
            snapshot.alerts[0].alert_id.clone(),
            snapshot.alerts[0].base(),
        )]);

        // A newer threshold arrived by sync before the unlock merged the firing.
        vault.doc.price_alerts[0].price = amount("70000");
        vault
            .apply_locked_deliveries_seen(vec![hits[0].1.clone()], &fired, &seen)
            .unwrap();
        assert!(
            vault.doc.price_alerts[0].triggered_at.is_none(),
            "the old firing switched the new threshold off"
        );
        // It is still told: the notice was shown.
        assert_eq!(vault.doc.notification_log.len(), 1);

        // Unchanged since it was seen: marked as before.
        vault.doc.price_alerts[0].price = amount("60000");
        vault
            .apply_locked_deliveries_seen(vec![], &fired, &seen)
            .unwrap();
        assert!(vault.doc.price_alerts[0].triggered_at.is_some());
    }
}
