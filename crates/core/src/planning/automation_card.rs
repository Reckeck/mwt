//! The automation card and the journal (redesign Auto-1 · 3–4): what a
//! scheme or a repeat does in four lines — when, if, do, asks — when it fires
//! next and its latest runs; and every run and notice in one list. Words come
//! from the screen: the core gives facts, wallet and category ids included.
//! What reads a workflow graph (the trigger and the actions of a scheme) is in
//! `usecases::planning::automation_card`: `planning` may not use `workflow`.

use serde_json::{Map, Value};

use crate::Amount;
use crate::calendar;
use crate::model::notify::{NoticeCategory, NoticeRecord};
use crate::model::planning::{Automation, Schedule, TransactionDraft, WorkdayShift};
use crate::model::transaction::{AccountRef, Transaction};
use crate::model::vault::VaultDoc;
use crate::model::workflow::{RunOutcome, Workflow, WorkflowRun, WorkflowWait};
use crate::planning::my_automations::MineKind;

/// "Last runs" on the card.
pub const CARD_RUNS: usize = 5;

/// The money of one operation, made or about to be made.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CardMoney {
    /// What arrives (or leaves, for a payment), without a sign.
    pub amount: Option<Amount>,
    /// Lowercase code: `usd`, `usdt`.
    pub currency: Option<String>,
    pub from_store: Option<String>,
    pub to_store: Option<String>,
    /// The category key a payment goes to (the «where» of money that leaves).
    #[serde(default)]
    pub category: Option<String>,
}

impl CardMoney {
    pub(crate) fn of(tx: &Transaction) -> Self {
        let legs = tx.legs();
        let out = legs.iter().find(|leg| leg.amount.is_negative());
        let into = legs.iter().find(|leg| leg.amount.is_positive());
        let main = into.or(out);
        Self {
            amount: main.map(|leg| leg.amount.abs()),
            currency: main.map(|leg| leg.asset.symbol.to_lowercase()),
            from_store: out.map(|leg| leg.store_id.clone()),
            to_store: into.map(|leg| leg.store_id.clone()),
            category: tx.meta().categories.first().cloned(),
        }
    }

    /// What the operation template says, for what the node itself leaves empty.
    pub(crate) fn fill_from(&mut self, draft: &TransactionDraft) {
        let probe = crate::model::planning::draft_to_transaction(draft, "", "", "");
        let from_template = Self::of(&probe);
        self.currency = self.currency.take().or(from_template.currency);
        self.from_store = self.from_store.take().or(from_template.from_store);
        self.to_store = self.to_store.take().or(from_template.to_store);
        self.category = self.category.take().or(from_template.category);
    }
}

/// "If": what starts it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CardTrigger {
    /// Only the time: nothing more to say than "When".
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

/// "Do": one action of it.
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
    /// Waits for "yes".
    Asking,
    /// Asleep at "Wait".
    Waiting,
    Partial,
    Failed,
    Stopped,
    /// Not enough money: waits for "Retry" or "Skip".
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
    /// For "Confirm" in the row.
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
    /// "When"; for a scheme on an income, the schedule of the repeat that
    /// books it (see `follows`).
    pub schedule: Option<Schedule>,
    pub workday: WorkdayShift,
    /// The repeat whose schedule "When" borrows.
    pub follows: Option<String>,
    pub trigger: CardTrigger,
    pub actions: Vec<CardAction>,
    pub asks: bool,
    /// RFC3339.
    pub next_at: Option<String>,
    pub runs: Vec<JournalEntry>,
}

pub(crate) fn text(params: &Map<String, Value>, name: &str) -> Option<String> {
    match params.get(name)? {
        Value::String(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub(crate) fn whole(params: &Map<String, Value>, name: &str) -> Option<i64> {
    text(params, name)?.parse().ok()
}

pub(crate) fn store_of(value: Option<&Value>) -> Option<String> {
    let account: AccountRef = serde_json::from_value(value?.clone()).ok()?;
    account.store_id().map(str::to_string)
}

pub(crate) fn workday_of(params: &Map<String, Value>) -> WorkdayShift {
    match text(params, "workday").as_deref() {
        Some("back") => WorkdayShift::Back,
        Some("forward") => WorkdayShift::Forward,
        _ => WorkdayShift::None,
    }
}

/// "on schedule" read back as the form's schedule.
pub(crate) fn schedule_of(params: &Map<String, Value>) -> Option<Schedule> {
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

pub(crate) fn first_store(params: &Map<String, Value>) -> Option<String> {
    store_of(params.get("store")).or_else(|| text(params, "store"))
}

/// A run that asked "yes" and was answered: the answer is a run of its own,
/// so the question no longer belongs in the lists.
pub fn answered_question(doc: &VaultDoc, run: &WorkflowRun) -> bool {
    matches!(run.outcome, RunOutcome::Asking { .. })
        && !doc.workflow_waits.iter().any(|wait| wait.run_id == run.id)
}

/// One run as a journal row. `asked` reads the money a sleeping run will
/// move from the graph it slept in (`(graph, memo)`), which only the
/// scenario may parse.
pub fn journal_entry(
    doc: &VaultDoc,
    workflow: &Workflow,
    run: &WorkflowRun,
    asked: impl Fn(&str, &str) -> CardMoney,
) -> JournalEntry {
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
    let wait: Option<&WorkflowWait> = doc.workflow_waits.iter().find(|wait| wait.run_id == run.id);
    let money = match (run.created.first(), wait) {
        (Some(id), _) => doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == *id)
            .map(CardMoney::of)
            .unwrap_or_default(),
        (None, Some(wait)) => asked(
            if wait.graph.is_empty() {
                &workflow.graph
            } else {
                &wait.graph
            },
            &wait.memo,
        ),
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

/// The reminders and alerts that were shown, as journal rows.
pub fn notice_entries(notices: Vec<NoticeRecord>) -> Vec<JournalEntry> {
    let mut out = Vec::new();
    for notice in notices {
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
    out
}

/// The newest first, at most `limit` rows.
pub fn newest(mut entries: Vec<JournalEntry>, limit: usize) -> Vec<JournalEntry> {
    entries.sort_by(|left, right| right.at.cmp(&left.at));
    entries.truncate(limit);
    entries
}

/// The card of a repeat: its schedule, its one operation and the next one.
pub fn fill_repeat(
    card: &mut AutomationCard,
    automation: Automation,
    next: Option<(i64, Transaction)>,
) {
    card.kind = MineKind::Repeat;
    card.schedule = Some(automation.schedule);
    card.workday = automation.workday_shift;
    card.trigger = CardTrigger::Schedule;
    card.asks = automation.asks;
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
}

/// A scheme on an income fires when that income comes: "When" and "Next
/// time" follow the soonest repeat that books its category.
pub fn follow(card: &mut AutomationCard, moment: i64, automation: Automation) {
    card.schedule = Some(automation.schedule);
    card.workday = automation.workday_shift;
    card.follows = Some(automation.name.clone());
    card.next_at = Some(calendar::iso(moment));
}
