//! Tests of the automation_card area: the old inline tests (names and bodies unchanged).

use super::automation_card::*;
use crate::model::planning::{Schedule, TransactionDraft, WorkdayShift};
use crate::model::workflow::{RunOutcome, WorkflowRun};
use crate::planning::my_automations::MineKind;
use crate::workflow::types::WfValue;
use serde_json::json;

use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use crate::service::test_support::*;
use crate::service::{AutomationDraft, WorkflowDraft};
use crate::workflow::model::{WaitKind, WorkflowWait};

#[test]
fn card_follows_the_salary_and_the_journal_shows_the_asked_share() {
    let (_dir, mut vault) = empty_vault();
    let card_store = store(&mut vault, "Card", StoreKind::Bank, None);
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let Transaction::Topup(mut salary) = income(&card_store, "1575", "salary") else {
        unreachable!()
    };
    salary.meta.categories = vec!["salary".into()];
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
            workday_shift: WorkdayShift::Back,
            property_id: None,
        })
        .unwrap();
    let to_cash = json!({ "store": cash });
    let graph = json!({
        "version": 1,
        "nodes": [
            { "id": "income", "kind": "trigger.transaction",
              "params": { "tx_kind": "topup", "category": "salary", "store": { "store": card_store } } },
            { "id": "share", "kind": "logic.math", "params": { "op": "percent", "b": "10", "round": "2" } },
            { "id": "ask", "kind": "action.confirm", "params": { "timeout_hours": 72 } },
            { "id": "save", "kind": "action.transaction", "params": { "tx_kind": "transfer", "to": to_cash } },
        ],
        "edges": [
            { "id": "e1", "from": "income", "from_port": "amount", "to": "share", "to_port": "a" },
            { "id": "e2", "from": "income", "from_port": "out", "to": "ask", "to_port": "in" },
            { "id": "e3", "from": "ask", "from_port": "true", "to": "save", "to_port": "in" },
            { "id": "e4", "from": "share", "from_port": "result", "to": "save", "to_port": "amount" },
            { "id": "e5", "from": "income", "from_port": "account", "to": "save", "to_port": "account" },
        ],
    });
    let draft = WorkflowDraft {
        name: "10% of salary".into(),
        description: String::new(),
        active: true,
        graph: graph.to_string(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    };
    let now = unix_secs("2026-10-24T12:00:00Z").unwrap();
    let scheme = vault.save_workflow(None, draft, now).unwrap().workflow.id;

    let card = vault.automation_card(&scheme, now, "UTC").unwrap();
    assert_eq!(card.kind, MineKind::Scheme);
    assert_eq!(card.follows.as_deref(), Some("Salary"));
    assert_eq!(card.schedule, Some(Schedule::Monthly { day: 25 }));
    assert_eq!(card.workday, WorkdayShift::Back);
    // 25 Oct 2026 is a Sunday: the salary came on Friday; the next is 25 Nov.
    assert_eq!(
        card.next_at.as_deref().map(|at| &at[..10]),
        Some("2026-11-25")
    );
    assert!(card.asks);
    assert!(matches!(
        &card.trigger,
        CardTrigger::Transaction { category: Some(category), store: Some(store), .. }
            if category == "salary" && *store == card_store
    ));
    assert_eq!(card.actions.len(), 1);
    let save = &card.actions[0];
    assert_eq!(save.kind, "transfer");
    assert_eq!(save.percent, Some(amount("10")));
    assert_eq!(save.money.to_store.as_deref(), Some(cash.as_str()));
    assert_eq!(save.money.from_store.as_deref(), Some(card_store.as_str()));

    let memo = serde_json::to_string(&vec![(
        "income".to_string(),
        "amount".to_string(),
        WfValue::Money {
            amount: amount("1575"),
            currency: "usd".into(),
        },
    )])
    .unwrap();
    vault.doc.workflow_runs.push(WorkflowRun {
        id: "run1".into(),
        workflow_id: scheme.clone(),
        trigger: "income".into(),
        started_at: "2026-09-25T10:00:00Z".into(),
        outcome: RunOutcome::Asking {
            node_id: "ask".into(),
            question: String::new(),
            until: None,
        },
        created: Vec::new(),
        queued: 0,
        trace: Vec::new(),
        dry_run: false,
        planned_date: None,
    });
    vault.doc.workflow_waits.push(WorkflowWait {
        id: "wait1".into(),
        workflow_id: scheme.clone(),
        run_id: "run1".into(),
        node_id: "ask".into(),
        resume_at: String::new(),
        memo,
        queue_mode: false,
        trigger: "income".into(),
        kind: WaitKind::Confirm,
        question: String::new(),
        created_at: "2026-09-25T10:00:00Z".into(),
        graph: String::new(),
    });

    let journal = vault.automation_journal(20);
    let asked = journal.iter().find(|entry| entry.id == "run1").unwrap();
    assert_eq!(asked.status, JournalStatus::Asking);
    assert_eq!(asked.wait_id.as_deref(), Some("wait1"));
    assert_eq!(asked.money.amount, Some(amount("157.5")));
    assert_eq!(asked.money.currency.as_deref(), Some("usd"));
    assert_eq!(asked.money.to_store.as_deref(), Some(cash.as_str()));
    assert_eq!(
        vault
            .automation_card(&scheme, now, "UTC")
            .unwrap()
            .runs
            .len(),
        1
    );
}

#[test]
fn notice_entries_keep_reminders_and_alerts_and_newest_cuts_the_list() {
    use crate::model::notify::{NoticeCategory, NoticeRecord};
    let notice = |category, key: &str, delivered_at: &str| NoticeRecord {
        key: key.into(),
        category,
        title: "Title".into(),
        body: "Body".into(),
        link: None,
        at: "2026-03-01T00:00:00Z".into(),
        delivered_at: delivered_at.into(),
        read: false,
    };
    let entries = notice_entries(vec![
        notice(
            NoticeCategory::Reminder,
            "reminder:r1:x",
            "2026-03-02T00:00:00Z",
        ),
        notice(NoticeCategory::PriceAlert, "alert:a1:x", ""),
        notice(NoticeCategory::Rates, "source:s1:x", "2026-03-03T00:00:00Z"),
    ]);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].kind, MineKind::Reminder);
    assert_eq!(entries[0].source_id, "r1");
    assert_eq!(entries[1].kind, MineKind::Alert);
    assert_eq!(entries[1].at, "2026-03-01T00:00:00Z");
    let newest = newest(entries, 1);
    assert_eq!(newest.len(), 1);
    assert_eq!(newest[0].source_id, "r1");
}

#[test]
fn nothing_was_answered_in_an_empty_document() {
    let doc = crate::model::vault::VaultDoc::default();
    let run = WorkflowRun {
        id: "run".into(),
        workflow_id: "flow".into(),
        trigger: String::new(),
        started_at: "2026-03-01T00:00:00Z".into(),
        outcome: RunOutcome::Done,
        created: Vec::new(),
        queued: 0,
        trace: Vec::new(),
        dry_run: false,
        planned_date: None,
    };
    assert!(!answered_question(&doc, &run));
}
