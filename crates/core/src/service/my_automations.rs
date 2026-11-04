//! "Mine" (redesign Auto-1 · 1): everything that fires on its own in one
//! list — schemes, repeats, reminders and price alerts — with when each fires
//! next and what it will do. Before, that took five screens.

use chrono::Duration;

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::model::planning::AlertCondition;
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use crate::workflow::graph::Graph;
use crate::workflow::nodes::triggers::schedule_moments_limited;

use super::UnlockedVault;

/// How far ahead "Next time" looks: a yearly schedule still has a date.
const LOOKAHEAD_DAYS: i64 = 400;

/// What kind of thing fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum MineKind {
    /// A workflow the user drew.
    Scheme,
    /// An automation: an operation on a schedule.
    Repeat,
    Reminder,
    Alert,
}

/// One row of "Mine".
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct MineItem {
    pub kind: MineKind,
    /// The workflow, automation, reminder or alert id.
    pub id: String,
    pub name: String,
    /// Switched on: a reminder that is not muted, an alert that has not fired.
    pub active: bool,
    /// When it fires next, RFC3339; `None` for an alert (it waits for the
    /// price) and for a scheme without a schedule.
    pub next_at: Option<String>,
    /// The operation it makes: signed as it moves the money (an income
    /// positive, a payment negative, a transfer by what arrives).
    pub amount: Option<Amount>,
    pub asset: Option<AssetRef>,
    /// The wallet the money leaves.
    pub from_store: Option<String>,
    /// The wallet the money comes to.
    pub to_store: Option<String>,
    /// It asks "yes" before it does anything.
    pub asks: bool,
    /// It only notifies: a reminder, an alert, a scheme without operations.
    pub notifies: bool,
    /// For an alert: above or below `amount` of `asset`, in `currency`.
    pub condition: Option<AlertCondition>,
    pub currency: Option<String>,
    /// The scheme's own description.
    pub description: String,
}

impl MineItem {
    fn new(kind: MineKind, id: &str, name: &str, active: bool) -> Self {
        Self {
            kind,
            id: id.to_string(),
            name: name.to_string(),
            active,
            next_at: None,
            amount: None,
            asset: None,
            from_store: None,
            to_store: None,
            asks: false,
            notifies: false,
            condition: None,
            currency: None,
            description: String::new(),
        }
    }

    /// The money of an operation: where it leaves, where it comes, how much.
    fn with_operation(mut self, tx: &Transaction) -> Self {
        let legs = tx.legs();
        let out = legs.iter().find(|leg| leg.amount.is_negative());
        let into = legs.iter().find(|leg| leg.amount.is_positive());
        self.from_store = out.map(|leg| leg.store_id.clone());
        self.to_store = into.map(|leg| leg.store_id.clone());
        if let Some(leg) = into.or(out) {
            self.amount = Some(leg.amount);
            self.asset = Some(leg.asset.clone());
        }
        self
    }
}

impl UnlockedVault {
    /// Everything that fires on its own, the soonest first; alerts and
    /// schemes without a date after them, by name.
    pub fn my_automations(&self, now: i64, zone: &str) -> Vec<MineItem> {
        let cal = Calendar::new(zone);
        let until = now + Duration::days(LOOKAHEAD_DAYS).num_seconds();
        let mut out = Vec::new();

        for automation in self.automations() {
            let mut item = MineItem::new(
                MineKind::Repeat,
                &automation.id,
                &automation.name,
                automation.active,
            );
            if let Some((moment, tx)) = self
                .automation_occurrences(&automation, &cal, now, until)
                .into_iter()
                .next()
            {
                item.next_at = Some(calendar::iso(moment));
                item = item.with_operation(&tx);
            }
            out.push(item);
        }

        for workflow in self
            .doc
            .workflows
            .iter()
            .filter(|workflow| workflow.automation.is_none())
        {
            let mut item = MineItem::new(
                MineKind::Scheme,
                &workflow.id,
                &workflow.name,
                workflow.active,
            );
            item.description = workflow.description.clone();
            if let Ok(graph) = Graph::parse(&workflow.graph) {
                let live = || graph.nodes.iter().filter(|node| !node.disabled);
                item.next_at = live()
                    .filter(|node| node.kind == "trigger.schedule")
                    .filter_map(|node| {
                        schedule_moments_limited(&node.params, &cal, now, until, usize::MAX)
                            .0
                            .first()
                            .copied()
                    })
                    .min()
                    .map(calendar::iso);
                item.asks = live().any(|node| node.kind == "action.confirm");
                let operates = live().any(|node| {
                    matches!(
                        node.kind.as_str(),
                        "action.transaction" | "action.distribute"
                    )
                });
                item.notifies = !operates
                    && live().any(|node| {
                        matches!(node.kind.as_str(), "action.notify" | "action.reminder")
                    });
            }
            out.push(item);
        }

        for reminder in self.doc.reminders.iter().filter(|reminder| !reminder.done) {
            let mut item = MineItem::new(
                MineKind::Reminder,
                &reminder.id,
                &reminder.text,
                !reminder.muted,
            );
            item.next_at = Some(
                reminder
                    .snoozed_until
                    .clone()
                    .unwrap_or_else(|| reminder.due_at.clone()),
            );
            item.notifies = true;
            out.push(item);
        }

        for alert in &self.doc.price_alerts {
            let name = alert.asset.symbol.to_uppercase();
            let mut item = MineItem::new(
                MineKind::Alert,
                &alert.id,
                &name,
                alert.active && (alert.repeats || !alert.is_triggered()),
            );
            item.amount = Some(alert.price);
            item.asset = Some(alert.asset.clone());
            item.condition = Some(alert.condition);
            item.currency = Some(alert.currency.clone());
            item.notifies = true;
            out.push(item);
        }

        let when = |item: &MineItem| item.next_at.as_deref().and_then(unix_secs);
        // `None` after every date, then by name.
        out.sort_by_cached_key(|item| (when(item).is_none(), when(item), item.name.to_lowercase()));
        out
    }

    /// The "On" switch of a row: a scheme or a repeat is switched like a
    /// workflow (one with errors stays off), a reminder is muted, an alert
    /// switched back on waits for the price again.
    pub fn set_my_automation_active(
        &mut self,
        kind: MineKind,
        id: &str,
        active: bool,
        now: i64,
    ) -> Result<()> {
        match kind {
            MineKind::Scheme | MineKind::Repeat => {
                self.set_workflow_active(id, active, now)?;
                return Ok(());
            }
            MineKind::Reminder => {
                let reminder = self
                    .doc
                    .reminders
                    .iter_mut()
                    .find(|reminder| reminder.id == id)
                    .ok_or_else(|| Error::Missing(format!("reminder {id}")))?;
                reminder.muted = !active;
            }
            MineKind::Alert => {
                let alert = self
                    .doc
                    .price_alerts
                    .iter_mut()
                    .find(|alert| alert.id == id)
                    .ok_or_else(|| Error::Missing(format!("price alert {id}")))?;
                alert.active = active;
                if active {
                    alert.triggered_at = None;
                    alert.triggered_for = None;
                }
            }
        }
        self.persist()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::planning::{Schedule, TransactionDraft};
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;
    use crate::service::{AutomationDraft, PriceAlertDraft, WorkflowDraft};

    #[test]
    fn mine_lists_everything_that_fires_the_soonest_first() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let Transaction::Topup(salary) = income(&card, "1575", "salary") else {
            unreachable!()
        };
        vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "Salary".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 25 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Topup(salary),
                amount_mode: Default::default(),
                workday_shift: Default::default(),
                property_id: None,
            })
            .unwrap();
        vault
            .create_reminder("Pay rent", "2026-11-01T09:00:00Z", None)
            .unwrap();
        vault
            .create_price_alert(PriceAlertDraft {
                asset: btc(),
                condition: AlertCondition::Below,
                price: amount("60000"),
                currency: "usd".into(),
                active: true,
                store_id: None,
                repeats: false,
            })
            .unwrap();
        let graph = json!({
            "version": 1,
            "nodes": [
                { "id": "s", "kind": "trigger.schedule", "params": { "every": "day", "time": "09:00" } },
                { "id": "q", "kind": "action.confirm", "params": { "question": "Отложить?" } },
                { "id": "n", "kind": "action.notify", "params": { "text": "Готово" } },
            ],
            "edges": [
                { "id": "s.out>q.in", "from": "s", "from_port": "out", "to": "q", "to_port": "in" },
                { "id": "q.true>n.in", "from": "q", "from_port": "true", "to": "n", "to_port": "in" },
            ],
        });
        let draft = WorkflowDraft {
            name: "Morning".into(),
            description: String::new(),
            active: true,
            graph: graph.to_string(),
            react_to_workflows: false,
            max_amount: None,
        };
        let now = unix_secs("2026-10-24T12:00:00Z").unwrap();
        vault.save_workflow(None, draft, now).unwrap();

        let mine = vault.my_automations(now, "UTC");
        let rows: Vec<(MineKind, &str, Option<&str>)> = mine
            .iter()
            .map(|item| (item.kind, item.name.as_str(), item.next_at.as_deref()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (MineKind::Scheme, "Morning", Some("2026-10-25T09:00:00Z")),
                (MineKind::Repeat, "Salary", Some("2026-10-25T09:00:00Z")),
                (MineKind::Reminder, "Pay rent", Some("2026-11-01T09:00:00Z")),
                (MineKind::Alert, "BTC", None),
            ]
        );
        let scheme = &mine[0];
        assert!(scheme.asks && scheme.notifies);
        let repeat = &mine[1];
        assert_eq!(repeat.to_store.as_deref(), Some(card.as_str()));
        assert_eq!(repeat.amount, Some(amount("1575")));
        assert_eq!(mine[3].condition, Some(AlertCondition::Below));

        let reminder = mine[2].id.clone();
        vault
            .set_my_automation_active(MineKind::Reminder, &reminder, false, now)
            .unwrap();
        let alert = mine[3].id.clone();
        vault
            .set_my_automation_active(MineKind::Alert, &alert, false, now)
            .unwrap();
        let repeat = mine[1].id.clone();
        vault
            .set_my_automation_active(MineKind::Repeat, &repeat, false, now)
            .unwrap();
        let after = vault.my_automations(now, "UTC");
        let off: Vec<bool> = after.iter().map(|item| item.active).collect();
        assert_eq!(off, vec![true, false, false, false]);
    }
}
