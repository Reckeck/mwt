//! What a node does to the world, as a value (ADR-01, Command and Effect).
//!
//! A node works out an [`Effect`] and hands it to the run; the run applies it
//! through the [`WorkflowHost`] (what the vault holds) or into the report (what
//! the caller keeps: notices, queue, events). The nodes never write to the vault
//! themselves, so the same effect can be applied again later: a refused
//! operation sits in the queue as an `Effect::Insert` batch, and a retry books
//! it through `retry_queued_refusal` with the same checks as `Effect::Insert`
//! (the money and the rules, all or none).

use mwt_model::NoticeRecord;
use mwt_model::Reminder;
use mwt_model::Transaction;

use super::exec::{EmittedEvent, Executor};
use super::host::WorkflowHost;
use super::model::{QueuedActions, VarScope};
use super::types::WfValue;

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Create operations, all or none, dated by the clock `now`.
    Insert {
        txs: Vec<Transaction>,
        now: i64,
    },
    /// Remember a moment or a side of a threshold for a workflow.
    Mark {
        workflow_id: String,
        key: String,
        value: String,
    },
    SetVar {
        scope: VarScope,
        name: String,
        value: WfValue,
    },
    Remind(Reminder),
    Notify(NoticeRecord),
    /// Keep operations for the user to confirm or retry.
    Queue(QueuedActions),
    Emit(EmittedEvent),
}

impl Effect {
    /// Do the part that changes the host; the operations it booked.
    /// `Notify`, `Queue` and `Emit` only travel in the run's report.
    pub fn apply_to(self, host: &mut dyn WorkflowHost) -> Result<Vec<Transaction>, String> {
        match self {
            Self::Insert { txs, now } => return host.insert_transactions(txs, now),
            Self::Mark {
                workflow_id,
                key,
                value,
            } => host.set_mark(&workflow_id, &key, &value),
            Self::SetVar { scope, name, value } => host.set_var(scope, &name, &value),
            Self::Remind(reminder) => host.add_reminder(reminder),
            Self::Notify(_) | Self::Queue(_) | Self::Emit(_) => {}
        }
        Ok(Vec::new())
    }
}

impl Executor<'_> {
    /// Apply what a node worked out: through the host, then into the report.
    /// Returns the operations an `Insert` booked.
    pub(crate) fn perform(&mut self, effect: Effect) -> Result<Vec<Transaction>, String> {
        match &effect {
            Effect::Remind(reminder) => self.reminders.push(reminder.clone()),
            Effect::Notify(notice) => self.notices.push(notice.clone()),
            Effect::Queue(batch) => self.queued.push(batch.clone()),
            Effect::Emit(event) => self.events.push(event.clone()),
            Effect::Insert { .. } | Effect::Mark { .. } | Effect::SetVar { .. } => {}
        }
        let booked = effect.apply_to(&mut *self.host)?;
        self.created.extend(booked.iter().cloned());
        Ok(booked)
    }
}
