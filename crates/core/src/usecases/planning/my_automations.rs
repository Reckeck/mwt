//! Scenarios of "Mine": the list of everything that fires on its own, and
//! the "On" switch of a row. The rows that come from the document are built
//! in `planning::my_automations`; the schemes are read here because their
//! graphs belong to `workflow`, which `planning` may not use.

use crate::calendar::{self, Calendar};
use crate::error::Result;
use crate::model::vault::VaultDoc;
use crate::planning::my_automations::{self as rules, MineItem, MineKind};
use crate::service::Session;
use crate::workflow::graph::Graph;
use crate::workflow::nodes::triggers::schedule_moments_limited;

/// The rows of the workflows that are not automations: schemes the user drew.
fn scheme_items(doc: &VaultDoc, cal: &Calendar, now: i64, until: i64) -> Vec<MineItem> {
    let mut out = Vec::new();
    for workflow in doc
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
                    schedule_moments_limited(&node.params, cal, now, until, usize::MAX)
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
                && live()
                    .any(|node| matches!(node.kind.as_str(), "action.notify" | "action.reminder"));
        }
        out.push(item);
    }
    out
}

/// Everything that fires on its own, the soonest first; alerts and
/// schemes without a date after them, by name.
pub fn my_automations(vault: &Session, now: i64, zone: &str) -> Vec<MineItem> {
    let cal = Calendar::new(zone);
    let until = rules::lookahead_until(now);
    let mut out: Vec<MineItem> = crate::usecases::planning::automations::automations(vault)
        .iter()
        .map(|automation| {
            let next = vault
                .automation_occurrences(automation, &cal, now, until)
                .into_iter()
                .next();
            rules::repeat_item(automation, next)
        })
        .collect();
    out.extend(scheme_items(&vault.doc, &cal, now, until));
    out.extend(rules::reminder_items(&vault.doc));
    out.extend(rules::alert_items(&vault.doc));
    rules::sort_items(&mut out);
    out
}

/// The "On" switch of a row: a scheme or a repeat is switched like a
/// workflow (one with errors stays off), a reminder is muted, an alert
/// switched back on waits for the price again.
pub fn set_my_automation_active(
    vault: &mut Session,
    kind: MineKind,
    id: &str,
    active: bool,
    now: i64,
) -> Result<()> {
    match kind {
        MineKind::Scheme | MineKind::Repeat => {
            crate::usecases::workflow::records::set_workflow_active(vault, id, active, now)?;
            Ok(())
        }
        MineKind::Reminder => {
            vault.transact(|tx| rules::set_reminder_active(&mut tx.doc_mut().reminders, id, active))
        }
        MineKind::Alert => {
            vault.transact(|tx| rules::set_alert_active(&mut tx.doc_mut().price_alerts, id, active))
        }
    }
}
