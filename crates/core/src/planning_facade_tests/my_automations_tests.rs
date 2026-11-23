//! Tests of the my_automations area: the old inline tests (names and bodies unchanged).

use crate::model::planning::AlertCondition;
use crate::model::transaction::Transaction;
use crate::planning::my_automations::*;
use crate::time::unix_secs;
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
    crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
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
        },
    )
    .unwrap();
    crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Pay rent",
        "2026-11-01T09:00:00Z",
        None,
    )
    .unwrap();
    crate::usecases::planning::agenda::create_price_alert(
        &mut vault,
        PriceAlertDraft {
            asset: btc(),
            condition: AlertCondition::Below,
            price: amount("60000"),
            currency: "usd".into(),
            active: true,
            store_id: None,
            repeats: false,
        },
    )
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
    crate::usecases::workflow::records::save_workflow(&mut vault, None, draft, now).unwrap();

    let mine = crate::usecases::planning::my_automations::my_automations(&vault, now, "UTC");
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
    crate::usecases::planning::my_automations::set_my_automation_active(
        &mut vault,
        MineKind::Reminder,
        &reminder,
        false,
        now,
    )
    .unwrap();
    let alert = mine[3].id.clone();
    crate::usecases::planning::my_automations::set_my_automation_active(
        &mut vault,
        MineKind::Alert,
        &alert,
        false,
        now,
    )
    .unwrap();
    let repeat = mine[1].id.clone();
    crate::usecases::planning::my_automations::set_my_automation_active(
        &mut vault,
        MineKind::Repeat,
        &repeat,
        false,
        now,
    )
    .unwrap();
    let after = crate::usecases::planning::my_automations::my_automations(&vault, now, "UTC");
    let off: Vec<bool> = after.iter().map(|item| item.active).collect();
    assert_eq!(off, vec![true, false, false, false]);
}

#[test]
fn the_switch_delegate_equals_the_scenario() {
    use crate::usecases::planning::my_automations as scenario;
    let (_a, mut by_method) = empty_vault();
    let (_b, mut by_scenario) = empty_vault();
    let now = unix_secs("2026-10-24T12:00:00Z").unwrap();
    let mut ids = Vec::new();
    for vault in [&mut by_method, &mut by_scenario] {
        let reminder = crate::usecases::planning::agenda::create_reminder(
            vault,
            "Pay rent",
            "2026-11-01T09:00:00Z",
            None,
        )
        .unwrap();
        let alert = crate::usecases::planning::agenda::create_price_alert(
            vault,
            PriceAlertDraft {
                asset: btc(),
                condition: AlertCondition::Below,
                price: amount("60000"),
                currency: "usd".into(),
                active: true,
                store_id: None,
                repeats: false,
            },
        )
        .unwrap();
        ids.push((reminder.id, alert.id));
    }
    crate::usecases::planning::my_automations::set_my_automation_active(
        &mut by_method,
        MineKind::Reminder,
        &ids[0].0,
        false,
        now,
    )
    .unwrap();
    crate::usecases::planning::my_automations::set_my_automation_active(
        &mut by_method,
        MineKind::Alert,
        &ids[0].1,
        false,
        now,
    )
    .unwrap();
    scenario::set_my_automation_active(&mut by_scenario, MineKind::Reminder, &ids[1].0, false, now)
        .unwrap();
    scenario::set_my_automation_active(&mut by_scenario, MineKind::Alert, &ids[1].1, false, now)
        .unwrap();
    let rows = |vault: &crate::service::UnlockedVault| -> Vec<(MineKind, bool)> {
        scenario::my_automations(vault, now, "UTC")
            .iter()
            .map(|item| (item.kind, item.active))
            .collect()
    };
    assert_eq!(rows(&by_method), rows(&by_scenario));
    assert_eq!(
        rows(&by_method),
        vec![(MineKind::Reminder, false), (MineKind::Alert, false)]
    );
    assert!(
        scenario::set_my_automation_active(&mut by_scenario, MineKind::Alert, "missing", true, now)
            .is_err()
    );
}

#[test]
fn the_switch_rules_work_on_plain_lists() {
    let mut reminders = vec![crate::model::planning::Reminder::new(
        "Pay",
        "2026-11-01T09:00:00Z",
    )];
    let id = reminders[0].id.clone();
    set_reminder_active(&mut reminders, &id, false).unwrap();
    assert!(reminders[0].muted);
    set_reminder_active(&mut reminders, &id, true).unwrap();
    assert!(!reminders[0].muted);
    assert!(set_reminder_active(&mut reminders, "nope", true).is_err());
    assert!(set_alert_active(&mut [], "nope", true).is_err());
}

#[test]
fn rows_of_an_empty_document_are_empty_and_sorting_puts_undated_last() {
    let doc = crate::model::vault::VaultDoc::default();
    assert!(reminder_items(&doc).is_empty());
    assert!(alert_items(&doc).is_empty());
    let mut items = vec![
        MineItem::new(MineKind::Alert, "a", "BTC", true),
        MineItem::new(MineKind::Reminder, "r", "Rent", true),
    ];
    items[1].next_at = Some("2026-11-01T09:00:00Z".into());
    sort_items(&mut items);
    assert_eq!(items[0].id, "r");
    assert_eq!(lookahead_until(0), 400 * 86_400);
}
