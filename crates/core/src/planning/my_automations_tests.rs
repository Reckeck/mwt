//! Tests of the my_automations area: the old inline tests (names and bodies unchanged).

use super::my_automations::*;
use crate::model::planning::AlertCondition;
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use serde_json::json;

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
        max_amount_currency: None,
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
