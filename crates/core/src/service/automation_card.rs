//! The automation card and the journal (redesign Auto-1 · 3–4): what a
//! scheme or a repeat does in four lines — when, if, do, asks — when it fires
//! next and its latest runs; and every run and notice in one list. Words come
//! from the screen: the core gives facts, wallet and category ids included.

use chrono::Duration;
use serde_json::{Map, Value};

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::model::notify::NoticeCategory;
use crate::model::planning::{Schedule, TransactionDraft, WorkdayShift};
use crate::model::transaction::{AccountRef, Transaction};
use crate::workflow::graph::{Graph, Node};
use crate::workflow::model::{RunOutcome, Workflow, WorkflowRun};
use crate::workflow::nodes::triggers::schedule_moments_limited;
use crate::workflow::types::WfValue;

use super::UnlockedVault;
use super::my_automations::MineKind;

const LOOKAHEAD_DAYS: i64 = 400;
/// «Последние запуски» on the card.
const CARD_RUNS: usize = 5;

/// The money of one operation, made or about to be made.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CardMoney {
    /// What arrives (or leaves, for a payment), without a sign.
    pub amount: Option<Amount>,
    /// Lowercase code: `usd`, `usdt`.
    pub currency: Option<String>,
    pub from_store: Option<String>,
    pub to_store: Option<String>,
}

impl CardMoney {
    pub(super) fn of(tx: &Transaction) -> Self {
        let legs = tx.legs();
        let out = legs.iter().find(|leg| leg.amount.is_negative());
        let into = legs.iter().find(|leg| leg.amount.is_positive());
        let main = into.or(out);
        Self {
            amount: main.map(|leg| leg.amount.abs()),
            currency: main.map(|leg| leg.asset.symbol.to_lowercase()),
            from_store: out.map(|leg| leg.store_id.clone()),
            to_store: into.map(|leg| leg.store_id.clone()),
        }
    }
}

/// «Если»: what starts it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CardTrigger {
    /// Only the time: nothing more to say than «Когда».
    Schedule,
    /// An operation arrives: `tx_kind` as the node has it (`any`, `topup` …),
    /// a category key and a wallet when set.
    Transaction {
        tx_kind: String,
        category: Option<String>,
        store: Option<String>,
    },
    Manual,
    /// Any other trigger node, by its kind.
    Other {
        node_kind: String,
    },
}

/// «Сделать»: one action of it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CardAction {
    /// The operation kind (`transfer`, `payment` …), `distribute`, `notify`,
    /// `reminder`, or another action node's kind.
    pub kind: String,
    /// The amount is this share of the incoming one, in percent.
    pub percent: Option<Amount>,
    pub money: CardMoney,
    /// A notice's title.
    pub title: String,
}

/// How a run ended, as the journal says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum JournalStatus {
    Done,
    /// Waits for «да».
    Asking,
    /// Asleep at «Подождать».
    Waiting,
    Partial,
    Failed,
    Stopped,
    /// Not enough money: waits for «Повторить» or «Пропустить».
    Rejected,
    Skipped,
    /// A reminder or an alert that was shown.
    Shown,
}

/// One row of the journal: a run or a notice shown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct JournalEntry {
    /// The run id, or the notice key.
    pub id: String,
    pub kind: MineKind,
    /// The workflow, reminder or alert.
    pub source_id: String,
    pub name: String,
    /// RFC3339.
    pub at: String,
    pub status: JournalStatus,
    /// What it did, or what it asks to do.
    pub money: CardMoney,
    /// For «Подтвердить» in the row.
    pub wait_id: Option<String>,
    /// The reason or the error, as the run wrote it.
    pub note: String,
}

/// The card of a scheme or a repeat.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AutomationCard {
    pub kind: MineKind,
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    /// «Когда»; for a scheme on an income, the schedule of the repeat that
    /// books it (see `follows`).
    pub schedule: Option<Schedule>,
    pub workday: WorkdayShift,
    /// The repeat whose schedule «Когда» borrows.
    pub follows: Option<String>,
    pub trigger: CardTrigger,
    pub actions: Vec<CardAction>,
    pub asks: bool,
    /// RFC3339.
    pub next_at: Option<String>,
    pub runs: Vec<JournalEntry>,
}

fn text(params: &Map<String, Value>, name: &str) -> Option<String> {
    match params.get(name)? {
        Value::String(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn whole(params: &Map<String, Value>, name: &str) -> Option<i64> {
    text(params, name)?.parse().ok()
}

fn store_of(value: Option<&Value>) -> Option<String> {
    let account: AccountRef = serde_json::from_value(value?.clone()).ok()?;
    account.store_id().map(str::to_string)
}

fn workday_of(params: &Map<String, Value>) -> WorkdayShift {
    match text(params, "workday").as_deref() {
        Some("back") => WorkdayShift::Back,
        Some("forward") => WorkdayShift::Forward,
        _ => WorkdayShift::None,
    }
}

/// «По расписанию» read back as the form's schedule.
fn schedule_of(params: &Map<String, Value>) -> Option<Schedule> {
    let clamp = |value: i64, low: i64, high: i64| value.clamp(low, high);
    match text(params, "every").as_deref()? {
        "month" => Some(Schedule::Monthly {
            day: clamp(whole(params, "day").unwrap_or(1), 1, 31) as u8,
        }),
        "week" => Some(Schedule::Weekly {
            weekday: clamp(whole(params, "weekday").unwrap_or(0), 0, 6) as u8,
        }),
        "day" => Some(Schedule::EveryNDays {
            days: clamp(whole(params, "interval").unwrap_or(1), 1, 3650) as u16,
        }),
        _ => None,
    }
}

fn first_store(params: &Map<String, Value>) -> Option<String> {
    store_of(params.get("store")).or_else(|| text(params, "store"))
}

fn trigger_of(node: &Node) -> CardTrigger {
    match node.kind.as_str() {
        "trigger.schedule" => CardTrigger::Schedule,
        "trigger.manual" => CardTrigger::Manual,
        "trigger.transaction" => CardTrigger::Transaction {
            tx_kind: text(&node.params, "tx_kind").unwrap_or_else(|| "any".into()),
            category: text(&node.params, "category"),
            store: first_store(&node.params),
        },
        other => CardTrigger::Other {
            node_kind: other.to_string(),
        },
    }
}

/// The node an input port is wired from, with its port.
fn source<'a>(graph: &'a Graph, node: &str, port: &str) -> Option<(&'a Node, &'a str)> {
    let edge = graph
        .edges
        .iter()
        .find(|edge| edge.to == node && edge.to_port == port)?;
    let from = graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == edge.from)?;
    Some((from, edge.from_port.as_str()))
}

/// The share a «Математика» node set to percent takes.
fn percent_of(graph: &Graph, node: &Node) -> Option<(Amount, String)> {
    let (math, _) = source(graph, &node.id, "amount")?;
    if math.kind != "logic.math" || text(&math.params, "op").as_deref() != Some("percent") {
        return None;
    }
    Some((
        Amount::parse(&text(&math.params, "b")?).ok()?,
        math.id.clone(),
    ))
}

fn action_of(graph: &Graph, node: &Node) -> CardAction {
    let params = &node.params;
    let mut action = CardAction {
        kind: node.kind.clone(),
        percent: None,
        money: CardMoney::default(),
        title: String::new(),
    };
    match node.kind.as_str() {
        "action.transaction" => {
            action.kind = text(params, "tx_kind").unwrap_or_else(|| "transfer".into());
            action.percent = percent_of(graph, node).map(|(share, _)| share);
            action.money.amount = text(params, "amount").and_then(|raw| Amount::parse(&raw).ok());
            action.money.currency = text(params, "asset").map(|code| code.to_lowercase());
            action.money.to_store = store_of(params.get("to"));
            action.money.from_store = store_of(params.get("account")).or_else(|| {
                match source(graph, &node.id, "account") {
                    Some((from, _)) if from.kind == "trigger.transaction" => {
                        first_store(&from.params)
                    }
                    _ => None,
                }
            });
        }
        "action.distribute" => action.money.from_store = store_of(params.get("from")),
        "action.notify" | "action.reminder" => {
            action.title = text(params, "title").unwrap_or_default()
        }
        _ => {}
    }
    if action.kind == "action.distribute" {
        action.kind = "distribute".into();
    } else if let Some(short) = action.kind.strip_prefix("action.") {
        action.kind = short.to_string();
    }
    action
}

/// The money a sleeping run will move, from what it computed before it slept.
fn asked_money(graph: &Graph, memo: &str) -> CardMoney {
    let saved: Vec<(String, String, WfValue)> = serde_json::from_str(memo).unwrap_or_default();
    let value = |node: &str, port: &str| {
        saved
            .iter()
            .find(|(from, out, _)| from == node && out == port)
            .map(|(_, _, value)| value.clone())
    };
    let Some(node) = graph
        .nodes
        .iter()
        .find(|node| !node.disabled && node.kind == "action.transaction")
    else {
        return CardMoney::default();
    };
    let mut money = action_of(graph, node).money;
    let wired = |port: &str| {
        source(graph, &node.id, port).and_then(|(from, from_port)| value(&from.id, from_port))
    };
    let amount = wired("amount").or_else(|| {
        // The share is counted after the answer: take it of the income.
        let (share, math) = percent_of(graph, node)?;
        let (income, port) = source(graph, &math, "a")?;
        match value(&income.id, port)? {
            WfValue::Money { amount, currency } => Some(WfValue::Money {
                amount: amount * share / Amount::from_f64(100.0),
                currency,
            }),
            _ => None,
        }
    });
    match amount {
        Some(WfValue::Money { amount, currency }) => {
            money.amount = Some(amount.abs());
            money.currency = Some(currency);
        }
        Some(WfValue::Number(amount)) => money.amount = Some(amount.abs()),
        _ => {}
    }
    if let Some(WfValue::Account(account)) = wired("account") {
        money.from_store = account.store_id().map(str::to_string).or(money.from_store);
    }
    if let Some(WfValue::Account(account)) = wired("to") {
        money.to_store = account.store_id().map(str::to_string).or(money.to_store);
    }
    money
}

impl UnlockedVault {
    fn journal_entry(&self, workflow: &Workflow, run: &WorkflowRun) -> JournalEntry {
        let (status, note) = match &run.outcome {
            RunOutcome::Done => (JournalStatus::Done, String::new()),
            RunOutcome::Asking { question, .. } => (JournalStatus::Asking, question.clone()),
            RunOutcome::Waiting { until } => (JournalStatus::Waiting, until.clone()),
            RunOutcome::Partial { failed } => (JournalStatus::Partial, failed.join("; ")),
            RunOutcome::Failed { message } => (JournalStatus::Failed, message.clone()),
            RunOutcome::Stopped { reason } => (JournalStatus::Stopped, reason.clone()),
            RunOutcome::Rejected { .. } => (JournalStatus::Rejected, String::new()),
            RunOutcome::Dropped { reason } => (JournalStatus::Skipped, reason.clone()),
        };
        let wait = self
            .doc
            .workflow_waits
            .iter()
            .find(|wait| wait.run_id == run.id);
        let money = match (run.created.first(), wait) {
            (Some(id), _) => self
                .doc
                .transactions
                .iter()
                .find(|tx| tx.meta().id == *id)
                .map(CardMoney::of)
                .unwrap_or_default(),
            (None, Some(wait)) => {
                let graph = Graph::parse(if wait.graph.is_empty() {
                    &workflow.graph
                } else {
                    &wait.graph
                });
                graph
                    .map(|graph| asked_money(&graph, &wait.memo))
                    .unwrap_or_default()
            }
            _ => CardMoney::default(),
        };
        JournalEntry {
            id: run.id.clone(),
            kind: if workflow.automation.is_some() {
                MineKind::Repeat
            } else {
                MineKind::Scheme
            },
            source_id: workflow.id.clone(),
            name: workflow.name.clone(),
            at: run.started_at.clone(),
            status,
            money,
            wait_id: wait
                .filter(|_| status == JournalStatus::Asking)
                .map(|wait| wait.id.clone()),
            note,
        }
    }

    /// Every run and every reminder or alert shown, the newest first.
    pub fn automation_journal(&self, limit: usize) -> Vec<JournalEntry> {
        let mut out: Vec<JournalEntry> = self
            .doc
            .workflow_runs
            .iter()
            .filter(|run| !run.dry_run)
            .filter_map(|run| {
                let workflow = self
                    .doc
                    .workflows
                    .iter()
                    .find(|workflow| workflow.id == run.workflow_id)?;
                Some(self.journal_entry(workflow, run))
            })
            .collect();
        for notice in self.notifications() {
            let kind = match notice.category {
                NoticeCategory::Reminder => MineKind::Reminder,
                NoticeCategory::PriceAlert => MineKind::Alert,
                _ => continue,
            };
            out.push(JournalEntry {
                source_id: notice.key.split(':').nth(1).unwrap_or_default().to_string(),
                id: notice.key,
                kind,
                name: notice.title,
                at: if notice.delivered_at.is_empty() {
                    notice.at
                } else {
                    notice.delivered_at
                },
                status: JournalStatus::Shown,
                money: CardMoney::default(),
                wait_id: None,
                note: notice.body,
            });
        }
        out.sort_by(|left, right| right.at.cmp(&left.at));
        out.truncate(limit);
        out
    }

    /// The card of a scheme or a repeat (both are workflows).
    pub fn automation_card(&self, id: &str, now: i64, zone: &str) -> Result<AutomationCard> {
        let workflow = self
            .doc
            .workflows
            .iter()
            .find(|workflow| workflow.id == id)
            .ok_or_else(|| Error::Missing(format!("workflow {id}")))?;
        let cal = Calendar::new(zone);
        let until = now + Duration::days(LOOKAHEAD_DAYS).num_seconds();
        let runs = self
            .doc
            .workflow_runs
            .iter()
            .rev()
            .filter(|run| run.workflow_id == id && !run.dry_run)
            .take(CARD_RUNS)
            .map(|run| self.journal_entry(workflow, run))
            .collect();
        let mut card = AutomationCard {
            kind: MineKind::Scheme,
            id: workflow.id.clone(),
            name: workflow.name.clone(),
            description: workflow.description.clone(),
            active: workflow.active,
            schedule: None,
            workday: WorkdayShift::None,
            follows: None,
            trigger: CardTrigger::Manual,
            actions: Vec::new(),
            asks: false,
            next_at: None,
            runs,
        };
        if let Some(automation) = self
            .automations()
            .into_iter()
            .find(|automation| automation.id == id)
        {
            card.kind = MineKind::Repeat;
            card.schedule = Some(automation.schedule);
            card.workday = automation.workday_shift;
            card.trigger = CardTrigger::Schedule;
            let next = self
                .automation_occurrences(&automation, &cal, now, until)
                .into_iter()
                .next();
            card.next_at = next.as_ref().map(|(moment, _)| calendar::iso(*moment));
            let money = next.map(|(_, tx)| CardMoney::of(&tx)).unwrap_or_default();
            let kind = match automation.template {
                TransactionDraft::Topup(_) => "topup",
                TransactionDraft::Payment(_) => "payment",
                TransactionDraft::Transfer(_) => "transfer",
                TransactionDraft::Convert(_) => "convert",
            };
            card.actions.push(CardAction {
                kind: kind.into(),
                percent: None,
                money,
                title: String::new(),
            });
            return Ok(card);
        }
        let graph = Graph::parse(&workflow.graph)?;
        let live: Vec<&Node> = graph.nodes.iter().filter(|node| !node.disabled).collect();
        if let Some(trigger) = live.iter().find(|node| node.kind.starts_with("trigger.")) {
            card.trigger = trigger_of(trigger);
            card.schedule = schedule_of(&trigger.params);
            card.workday = workday_of(&trigger.params);
        }
        card.next_at = live
            .iter()
            .filter(|node| node.kind == "trigger.schedule")
            .filter_map(|node| {
                schedule_moments_limited(&node.params, &cal, now, until, 1)
                    .0
                    .first()
                    .copied()
            })
            .min()
            .map(calendar::iso);
        if let CardTrigger::Transaction {
            category: Some(category),
            ..
        } = card.trigger.clone()
        {
            self.follow_income(&mut card, &category, &cal, now, until);
        }
        card.asks = live.iter().any(|node| node.kind == "action.confirm");
        card.actions = live
            .iter()
            .filter(|node| node.kind.starts_with("action.") && node.kind != "action.confirm")
            .map(|node| action_of(&graph, node))
            .collect();
        Ok(card)
    }

    /// A scheme on an income fires when that income comes: «Когда» and
    /// «Следующий раз» follow the soonest repeat that books its category.
    fn follow_income(
        &self,
        card: &mut AutomationCard,
        category: &str,
        cal: &Calendar,
        now: i64,
        until: i64,
    ) {
        let soonest = self
            .automations()
            .into_iter()
            .filter(|automation| automation.active)
            .filter_map(|automation| {
                let (moment, tx) = self
                    .automation_occurrences(&automation, cal, now, until)
                    .into_iter()
                    .next()?;
                tx.meta()
                    .categories
                    .iter()
                    .any(|key| key == category)
                    .then_some((moment, automation))
            })
            .min_by_key(|(moment, _)| *moment);
        if let Some((moment, automation)) = soonest {
            card.schedule = Some(automation.schedule);
            card.workday = automation.workday_shift;
            card.follows = Some(automation.name.clone());
            card.next_at = Some(calendar::iso(moment));
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
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
}
