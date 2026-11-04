//! Automations (plan §7.2): operations that repeat on a schedule.
//!
//! An automation is a workflow the app builds from the automation form
//! (`Workflow::automation`, see `workflow::automation`): "on schedule" →
//! "Create transaction". A planned date fires at 09:00 local time, moved off a
//! day off when asked. Each processed date leaves a workflow run — done,
//! refused for lack of funds (D26), or skipped — and a refused one waits for
//! the user to retry or skip it. With an amount that differs every time the
//! operation is booked at the average of the latest corrected amounts and
//! marked approximate until the user puts in the real one.

use std::collections::HashSet;

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::model::planning::{
    AmountMode, Automation, AutomationOutcome, AutomationPreset, AutomationRun, AutomationSpec,
    RejectReason, Schedule, TransactionDraft, WorkdayShift,
};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::{PriceStamp, Transaction, TxOrigin};
use crate::model::vault::now_iso;
use crate::pricing::history::{secs_or_now, unix_secs};
use crate::workflow::automation::{self as graph_of, automation_graph};
use crate::workflow::exec::{StepStatus, TraceStep};
use crate::workflow::model::{GraphVersion, MAX_GRAPH_VERSIONS, RunOutcome, Workflow, WorkflowRun};
use crate::workflow::nodes::triggers::schedule_moments_limited;

use super::UnlockedVault;

/// Most planned dates one pass creates per automation; older missed dates
/// are recorded as skipped, so a long time away does not flood the history
/// (plan §7.2).
pub const MAX_CATCH_UP: usize = 24;

/// What an automation is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct AutomationDraft {
    pub name: String,
    pub active: bool,
    pub preset: Option<AutomationPreset>,
    pub schedule: Schedule,
    pub starts_at: String,
    pub template: TransactionDraft,
    pub amount_mode: AmountMode,
    pub workday_shift: WorkdayShift,
    pub property_id: Option<String>,
    pub time: Option<String>,
    pub asks: bool,
    pub for_month_offset: Option<i8>,
}

impl AutomationDraft {
    fn spec(&self) -> AutomationSpec {
        AutomationSpec {
            preset: self.preset,
            schedule: self.schedule,
            starts_at: self.starts_at.clone(),
            template: self.template.clone(),
            amount_mode: self.amount_mode,
            workday_shift: self.workday_shift,
            property_id: self.property_id.clone(),
            time: self.time.clone(),
            asks: self.asks,
            for_month_offset: self.for_month_offset,
        }
    }
}

fn spec_of(automation: &Automation) -> AutomationSpec {
    AutomationSpec {
        preset: automation.preset,
        schedule: automation.schedule,
        starts_at: automation.starts_at.clone(),
        template: automation.template.clone(),
        amount_mode: automation.amount_mode,
        workday_shift: automation.workday_shift,
        property_id: automation.property_id.clone(),
        time: automation.time.clone(),
        asks: automation.asks,
        for_month_offset: automation.for_month_offset,
    }
}

/// Longest emulation: a year ahead.
pub const MAX_SIMULATION_DAYS: u32 = 366;

/// One planned date as the emulation saw it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SimulatedRun {
    pub automation_id: String,
    pub automation_name: String,
    pub planned_at: String,
    pub outcome: AutomationOutcome,
    /// The operation it would create.
    pub transaction: Option<Transaction>,
}

/// One balance the emulated runs change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SimulatedBalance {
    pub store_id: String,
    pub asset: AssetRef,
    pub before: Amount,
    pub after: Amount,
}

/// What the automations would do over the next days, worked out on a copy
/// of the vault: nothing is written.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AutomationSimulation {
    pub from: String,
    pub until: String,
    pub runs: Vec<SimulatedRun>,
    pub balances: Vec<SimulatedBalance>,
    pub created: u32,
    pub rejected: u32,
}

/// A transaction from a template, dated `ts`; prices are left to the stamp.
/// It names the automation and the planned moment it was made for.
pub(crate) fn draft_to_transaction(
    draft: &TransactionDraft,
    automation_id: &str,
    planned_at: &str,
    ts: &str,
) -> Transaction {
    let mut tx = match draft.clone() {
        TransactionDraft::Topup(inner) => Transaction::Topup(inner),
        TransactionDraft::Payment(inner) => Transaction::Payment(inner),
        TransactionDraft::Transfer(inner) => Transaction::Transfer(inner),
        TransactionDraft::Convert(inner) => Transaction::Convert(inner),
    };
    let meta = tx.meta_mut();
    meta.id = uuid::Uuid::new_v4().to_string();
    meta.timestamp = ts.to_string();
    meta.origin = TxOrigin::Workflow {
        workflow_id: automation_id.to_string(),
        run_id: String::new(),
        planned_at: (!planned_at.is_empty()).then(|| planned_at.to_string()),
        estimated: false,
        run_key: None,
    };
    for (_, stamp) in tx.stamps_mut() {
        *stamp = PriceStamp::default();
    }
    tx
}

/// The amount a top-up, a payment or a transfer moves (an exchange: what it gives).
pub(crate) fn main_amount(tx: &Transaction) -> Option<Amount> {
    match tx {
        Transaction::Topup(inner) => Some(inner.amount),
        Transaction::Payment(inner) => Some(inner.amount),
        Transaction::Transfer(inner) => Some(inner.amount),
        Transaction::Convert(inner) => Some(inner.from.amount),
        _ => None,
    }
}

fn set_main_amount(tx: &mut Transaction, amount: Amount) -> Result<()> {
    match tx {
        Transaction::Topup(inner) => inner.amount = amount,
        Transaction::Payment(inner) => inner.amount = amount,
        Transaction::Transfer(inner) => inner.amount = amount,
        _ => {
            return Err(Error::InvalidInput(
                "only a top-up, a payment or a transfer has one amount to correct".into(),
            ));
        }
    }
    Ok(())
}

fn with_amount(draft: &TransactionDraft, amount: Amount) -> TransactionDraft {
    let mut draft = draft.clone();
    match &mut draft {
        TransactionDraft::Topup(inner) => inner.amount = amount,
        TransactionDraft::Payment(inner) => inner.amount = amount,
        TransactionDraft::Transfer(inner) => inner.amount = amount,
        TransactionDraft::Convert(inner) => inner.from.amount = amount,
    }
    draft
}

/// What a transaction takes from a store of the user's.
pub(super) struct Outflow {
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub fee: Option<(AssetRef, Amount)>,
}

/// The store and asset a transaction draws from, the principal it takes and
/// its fee: `None` when nothing leaves a store of the user's.
pub(super) fn outflow(tx: &Transaction) -> Option<Outflow> {
    let fee = tx.fee().map(|fee| (fee.asset.clone(), fee.amount.abs()));
    let (store_id, asset, amount) = match tx {
        Transaction::Payment(inner) => (inner.store_id.clone(), &inner.asset, inner.amount),
        Transaction::Transfer(inner) => (
            inner.from.store_id()?.to_string(),
            &inner.asset,
            inner.amount,
        ),
        Transaction::Convert(inner) => (
            inner.from.account.store_id()?.to_string(),
            &inner.from.asset,
            inner.from.amount,
        ),
        _ => return None,
    };
    Some(Outflow {
        store_id,
        asset: asset.clone(),
        amount: amount.abs(),
        fee,
    })
}

/// Human text of a rejection, for errors and logs; the UI words its own.
pub fn describe_reject(reason: &RejectReason) -> String {
    match reason {
        RejectReason::InsufficientFunds {
            store_id,
            asset,
            required,
            available,
        } => format!(
            "store {store_id}: {available} {sym} of {required} {sym}",
            sym = asset.symbol
        ),
        RejectReason::CreditLimitExceeded {
            store_id,
            limit,
            required,
        } => format!("store {store_id}: credit limit {limit}, needed {required}"),
        RejectReason::Invalid { message } => message.clone(),
    }
}

impl UnlockedVault {
    // -- views --------------------------------------------------------------

    fn automation_of(&self, workflow: &Workflow) -> Option<Automation> {
        let spec = workflow.automation.as_ref()?;
        let estimate = (spec.amount_mode == AmountMode::Varies)
            .then(|| self.wf_recent_average(&workflow.id, graph_of::ESTIMATE_OF))
            .flatten()
            .map(|(average, _)| average);
        let last_planned_at = self
            .wf_mark(&workflow.id, &graph_of::schedule_key())
            .filter(|mark| *mark != graph_of::start_mark(spec));
        Some(Automation {
            id: workflow.id.clone(),
            name: workflow.name.clone(),
            active: workflow.active,
            preset: spec.preset,
            schedule: spec.schedule,
            starts_at: spec.starts_at.clone(),
            template: spec.template.clone(),
            last_planned_at,
            created_at: workflow.created_at.clone(),
            amount_mode: spec.amount_mode,
            workday_shift: spec.workday_shift,
            estimate,
            property_id: spec.property_id.clone(),
            time: spec.time.clone(),
            asks: spec.asks,
            for_month_offset: spec.for_month_offset,
        })
    }

    pub fn automations(&self) -> Vec<Automation> {
        let mut out: Vec<Automation> = self
            .doc
            .workflows
            .iter()
            .filter_map(|workflow| self.automation_of(workflow))
            .collect();
        out.sort_by_key(|automation| automation.name.to_lowercase());
        out
    }

    pub fn automation(&self, id: &str) -> Result<Automation> {
        self.doc
            .workflows
            .iter()
            .find(|workflow| workflow.id == id)
            .and_then(|workflow| self.automation_of(workflow))
            .ok_or_else(|| Error::Missing(format!("automation {id}")))
    }

    /// The template the next run books: with an amount that differs, at the
    /// current estimate.
    pub(crate) fn automation_draft(&self, automation: &Automation) -> TransactionDraft {
        match automation.estimate {
            Some(estimate) => with_amount(&automation.template, estimate),
            None => automation.template.clone(),
        }
    }

    /// The planned moments of an automation in `(after, until]`, moved off
    /// days off as it is set to, each with the operation it would make: what
    /// the forecast, the calendar and the daily budget count on.
    pub(crate) fn automation_occurrences(
        &self,
        automation: &Automation,
        cal: &Calendar,
        after: i64,
        until: i64,
    ) -> Vec<(i64, Transaction)> {
        let spec = spec_of(automation);
        let start = unix_secs(&graph_of::start_mark(&spec)).unwrap_or(after);
        let (moments, _) = schedule_moments_limited(
            &graph_of::schedule_params(&spec),
            cal,
            after.max(start),
            until,
            usize::MAX,
        );
        let draft = self.automation_draft(automation);
        moments
            .into_iter()
            .map(|moment| {
                let iso = calendar::iso(moment);
                let mut tx = draft_to_transaction(&draft, &automation.id, &iso, &iso);
                if let TxOrigin::Workflow { estimated, .. } = &mut tx.meta_mut().origin {
                    *estimated = automation.amount_mode == AmountMode::Varies;
                }
                (moment, tx)
            })
            .collect()
    }

    // -- CRUD ---------------------------------------------------------------

    /// Reject a draft that could never run: a bad schedule, or a template
    /// that fails the transaction rules on its own.
    fn check_automation(&self, draft: &AutomationDraft) -> Result<()> {
        if draft.name.trim().is_empty() {
            return Err(Error::InvalidInput("name the automation".into()));
        }
        match draft.schedule {
            Schedule::Monthly { day } if !(1..=31).contains(&day) => {
                return Err(Error::InvalidInput("day of month is 1–31".into()));
            }
            Schedule::Weekly { weekday } if weekday > 6 => {
                return Err(Error::InvalidInput("weekday is 0–6".into()));
            }
            Schedule::EveryNDays { days: 0 } => {
                return Err(Error::InvalidInput("repeat at least every day".into()));
            }
            _ => {}
        }
        if draft.amount_mode == AmountMode::Varies
            && matches!(draft.template, TransactionDraft::Convert(_))
        {
            return Err(Error::InvalidInput(
                "an amount that differs fits a top-up, a payment or a transfer, not an exchange"
                    .into(),
            ));
        }
        self.check_property_link(draft.property_id.as_deref())?;
        if draft
            .time
            .as_deref()
            .is_some_and(|time| graph_of::parse_time(time).is_none())
        {
            return Err(Error::InvalidInput("time is HH:MM".into()));
        }
        let Some(starts) = unix_secs(&draft.starts_at) else {
            return Err(Error::InvalidInput("choose a start date".into()));
        };
        // The probe is dated at the start, which may be months ahead: it is
        // checked against the rules, not against today.
        let mut probe = draft_to_transaction(&draft.template, "", "", &draft.starts_at);
        let reference = starts.max(secs_or_now(&now_iso()));
        self.prepare_transaction_at(&mut probe, reference * 1000)
    }

    pub fn create_automation(&mut self, draft: AutomationDraft) -> Result<Automation> {
        self.check_automation(&draft)?;
        let spec = draft.spec();
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_iso();
        self.doc.workflows.push(Workflow {
            id: id.clone(),
            name: draft.name.trim().to_string(),
            description: String::new(),
            active: draft.active,
            graph: serde_json::to_string(&automation_graph(&spec))
                .map_err(|err| Error::InvalidInput(err.to_string()))?,
            created_at: now.clone(),
            updated_at: now,
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
            versions: Vec::new(),
            tests: Vec::new(),
            draft: None,
            automation: Some(spec.clone()),
        });
        self.wf_set_mark(&id, &graph_of::schedule_key(), &graph_of::start_mark(&spec));
        self.persist()?;
        self.automation(&id)
    }

    /// Change what an automation does: its graph is built again, the old one
    /// kept as a version. Dates already processed stay processed.
    pub fn update_automation(&mut self, id: &str, draft: AutomationDraft) -> Result<Automation> {
        self.check_automation(&draft)?;
        let spec = draft.spec();
        let graph = serde_json::to_string(&automation_graph(&spec))
            .map_err(|err| Error::InvalidInput(err.to_string()))?;
        let now = now_iso();
        let workflow = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id && workflow.automation.is_some())
            .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
        if workflow.graph != graph {
            workflow.versions.push(GraphVersion {
                saved_at: now.clone(),
                graph: std::mem::replace(&mut workflow.graph, graph),
            });
            let overflow = workflow.versions.len().saturating_sub(MAX_GRAPH_VERSIONS);
            workflow.versions.drain(..overflow);
        }
        workflow.name = draft.name.trim().to_string();
        workflow.active = draft.active;
        workflow.updated_at = now;
        workflow.automation = Some(spec.clone());
        // A start moved later holds the schedule back until then.
        let key = graph_of::schedule_key();
        let start = graph_of::start_mark(&spec);
        if self
            .wf_mark(id, &key)
            .is_none_or(|mark| secs_or_now(&mark) < secs_or_now(&start))
        {
            self.wf_set_mark(id, &key, &start);
        }
        self.persist()?;
        self.automation(id)
    }

    /// Remove an automation and its run history. Transactions it created stay.
    pub fn delete_automation(&mut self, id: &str) -> Result<()> {
        self.automation(id)?;
        self.delete_workflow(id)
    }

    /// Make an automation an ordinary workflow: the graph stays and is edited
    /// in the editor from now on; the form no longer shows it.
    pub fn detach_automation(&mut self, id: &str) -> Result<Workflow> {
        let workflow = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id && workflow.automation.is_some())
            .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
        workflow.automation = None;
        workflow.updated_at = now_iso();
        let out = workflow.clone();
        self.persist()?;
        Ok(out)
    }

    /// Run history, newest planned date first.
    pub fn automation_runs(&self, automation_id: Option<&str>) -> Vec<AutomationRun> {
        let ids: HashSet<&str> = self
            .doc
            .workflows
            .iter()
            .filter(|workflow| workflow.automation.is_some())
            .map(|workflow| workflow.id.as_str())
            .filter(|id| automation_id.is_none_or(|only| only == *id))
            .collect();
        let mut out: Vec<AutomationRun> = self
            .doc
            .workflow_runs
            .iter()
            .filter(|run| !run.dry_run && ids.contains(run.workflow_id.as_str()))
            .filter_map(|run| self.automation_run_of(run))
            .collect();
        out.sort_by(|left, right| right.planned_at.cmp(&left.planned_at));
        out
    }

    fn automation_run_of(&self, run: &WorkflowRun) -> Option<AutomationRun> {
        let planned_at = match &run.outcome {
            RunOutcome::Rejected {
                planned_at: Some(at),
                ..
            } => at.clone(),
            _ => run
                .trigger
                .strip_prefix("schedule ")
                .map_or_else(|| run.started_at.clone(), str::to_string),
        };
        let outcome = match &run.outcome {
            RunOutcome::Done | RunOutcome::Partial { .. } => AutomationOutcome::Created {
                tx_id: run.created.first()?.clone(),
            },
            RunOutcome::Rejected { reason, .. } => AutomationOutcome::Rejected {
                reason: reason.clone(),
            },
            RunOutcome::Failed { message } => AutomationOutcome::Rejected {
                reason: RejectReason::Invalid {
                    message: message.clone(),
                },
            },
            RunOutcome::Dropped { reason } if reason == "skipped" => AutomationOutcome::Skipped,
            _ => return None,
        };
        let made = match &outcome {
            AutomationOutcome::Created { tx_id } => self
                .doc
                .transactions
                .iter()
                .find(|tx| &tx.meta().id == tx_id),
            _ => None,
        };
        let estimated = made.is_some_and(|tx| tx.meta().origin.is_estimated());
        let amount = made.and_then(main_amount);
        let symbol = made
            .and_then(|tx| {
                outflow(tx).map(|flow| flow.asset.symbol).or_else(|| {
                    tx.legs()
                        .into_iter()
                        .find(|leg| leg.amount.is_positive())
                        .map(|leg| leg.asset.symbol)
                })
            })
            .unwrap_or_default();
        Some(AutomationRun {
            id: run.id.clone(),
            automation_id: run.workflow_id.clone(),
            planned_at,
            processed_at: run.started_at.clone(),
            outcome,
            estimated,
            amount,
            symbol,
        })
    }

    // -- funds (D26) --------------------------------------------------------

    /// Quantity of an asset on one store from transactions up to `secs`.
    pub(crate) fn balance_at(&self, store_id: &str, asset_key: &str, secs: i64) -> Amount {
        self.doc
            .transactions
            .iter()
            .filter(|tx| secs_or_now(&tx.meta().timestamp) <= secs)
            .flat_map(|tx| tx.legs())
            .filter(|leg| leg.store_id == store_id && leg.asset.key() == asset_key)
            .fold(Amount::ZERO, |sum, leg| sum + leg.amount)
    }

    /// Whether the source of a transaction can pay for it at `secs`: its
    /// balance (principal and a fee in the same asset; a fee in another asset
    /// separately), or for a credit card its limit. Money coming in, and money
    /// leaving someone else's side, is not checked.
    /// `None` when it can.
    pub(crate) fn funds_shortfall(&self, tx: &Transaction, secs: i64) -> Option<RejectReason> {
        let Outflow {
            store_id,
            asset,
            amount,
            fee,
        } = outflow(tx)?;
        let Some(store) = self.find_store(&store_id) else {
            return Some(RejectReason::Invalid {
                message: format!("store {store_id} not found"),
            });
        };
        let same_asset_fee = fee
            .as_ref()
            .filter(|(fee_asset, _)| fee_asset.key() == asset.key())
            .map_or(Amount::ZERO, |(_, fee_amount)| *fee_amount);
        if store.kind == StoreKind::Liability {
            let limit = store.liability.as_ref().and_then(|terms| terms.limit)?;
            let debt =
                (Amount::ZERO - self.balance_at(&store_id, &asset.key(), secs)).max(Amount::ZERO);
            let required = debt + amount + same_asset_fee;
            return (required > limit).then_some(RejectReason::CreditLimitExceeded {
                store_id,
                limit,
                required,
            });
        }
        let mut needs = vec![(asset.clone(), amount + same_asset_fee)];
        if let Some((fee_asset, fee_amount)) = fee
            && fee_asset.key() != asset.key()
        {
            needs.push((fee_asset, fee_amount));
        }
        for (need_asset, required) in needs {
            let available = self.balance_at(&store_id, &need_asset.key(), secs);
            if available < required {
                return Some(RejectReason::InsufficientFunds {
                    store_id,
                    asset: need_asset,
                    required,
                    available,
                });
            }
        }
        None
    }

    /// Emulate the automations over the next `days` (one of them, or all):
    /// day by day on a sandbox copy, so each run sees the balance the earlier
    /// ones left, exactly as the scheduler would. The vault is not touched.
    pub fn simulate_automations(
        &self,
        automation_id: Option<&str>,
        days: u32,
        now: i64,
        zone: &str,
    ) -> Result<AutomationSimulation> {
        if let Some(id) = automation_id {
            self.automation(id)?;
        }
        let days = days.clamp(1, MAX_SIMULATION_DAYS);
        let mut sandbox = self.sandbox();
        let known: HashSet<String> = sandbox
            .doc
            .workflow_runs
            .iter()
            .map(|run| run.id.clone())
            .collect();
        let before = sandbox.doc.holdings.clone();
        for day in 0..=i64::from(days) {
            sandbox.run_due_workflows(now + day * 86_400, zone)?;
        }
        let wanted = |id: &str| automation_id.is_none_or(|only| only == id);
        let mut out = AutomationSimulation {
            from: calendar::iso(now),
            until: calendar::iso(now + i64::from(days) * 86_400),
            ..AutomationSimulation::default()
        };
        let mut touched: HashSet<(String, String)> = HashSet::new();
        for run in sandbox
            .automation_runs(None)
            .into_iter()
            .filter(|run| !known.contains(&run.id) && wanted(&run.automation_id))
        {
            let transaction = match &run.outcome {
                AutomationOutcome::Created { tx_id } => sandbox
                    .doc
                    .transactions
                    .iter()
                    .find(|tx| &tx.meta().id == tx_id)
                    .cloned(),
                _ => None,
            };
            match &run.outcome {
                AutomationOutcome::Created { .. } => out.created += 1,
                AutomationOutcome::Rejected { .. } => out.rejected += 1,
                AutomationOutcome::Skipped => {}
            }
            if let Some(tx) = &transaction {
                for leg in tx.legs() {
                    touched.insert((leg.store_id.clone(), leg.asset.key()));
                }
            }
            out.runs.push(SimulatedRun {
                automation_name: sandbox
                    .doc
                    .workflows
                    .iter()
                    .find(|workflow| workflow.id == run.automation_id)
                    .map(|workflow| workflow.name.clone())
                    .unwrap_or_default(),
                automation_id: run.automation_id,
                planned_at: run.planned_at,
                outcome: run.outcome,
                transaction,
            });
        }
        out.runs
            .sort_by(|left, right| left.planned_at.cmp(&right.planned_at));
        let quantity = |holdings: &[crate::model::store::Holding], key: &(String, String)| {
            holdings
                .iter()
                .find(|holding| holding.store_id == key.0 && holding.asset.key() == key.1)
                .map(|holding| (holding.asset.clone(), holding.quantity))
        };
        let mut balances: Vec<SimulatedBalance> = touched
            .iter()
            .filter_map(|key| {
                let after = quantity(&sandbox.doc.holdings, key);
                let was = quantity(&before, key);
                let asset = after.as_ref().or(was.as_ref())?.0.clone();
                Some(SimulatedBalance {
                    store_id: key.0.clone(),
                    asset,
                    before: was.map_or(Amount::ZERO, |(_, value)| value),
                    after: after.map_or(Amount::ZERO, |(_, value)| value),
                })
            })
            .collect();
        balances.sort_by(|left, right| {
            (&left.store_id, &left.asset.symbol).cmp(&(&right.store_id, &right.asset.symbol))
        });
        out.balances = balances;
        Ok(out)
    }

    /// Validate and append without persisting.
    /// `now` is the caller's clock, ahead of the wall clock in a simulation.
    fn insert_transaction(&mut self, mut tx: Transaction, now: i64) -> Result<Transaction> {
        self.prepare_transaction_at(&mut tx, now * 1000)?;
        self.doc.transactions.push(tx.clone());
        Ok(tx)
    }

    /// Create the transaction of an automation now ("Create now"), with
    /// the same funds check as the schedule.
    pub fn run_automation(&mut self, id: &str, now: i64) -> Result<Transaction> {
        let automation = self.automation(id)?;
        let planned_at = calendar::iso(now);
        let mut tx = draft_to_transaction(
            &self.automation_draft(&automation),
            id,
            &planned_at,
            &planned_at,
        );
        let run_id = uuid::Uuid::new_v4().to_string();
        if let TxOrigin::Workflow {
            estimated,
            run_id: made_by,
            ..
        } = &mut tx.meta_mut().origin
        {
            *estimated = automation.amount_mode == AmountMode::Varies;
            made_by.clone_from(&run_id);
        }
        let saved = self.create_checked(tx, now)?;
        self.doc.workflow_runs.push(WorkflowRun {
            id: run_id,
            workflow_id: id.to_string(),
            trigger: format!("manual {planned_at}"),
            started_at: planned_at,
            outcome: RunOutcome::Done,
            created: vec![saved.meta().id.clone()],
            queued: 0,
            trace: vec![TraceStep {
                node_id: graph_of::ACTION.into(),
                kind: "action.transaction".into(),
                status: StepStatus::Ok,
                text_ru: "Создано вручную: «Создать сейчас»".into(),
                text_en: "Made by hand: «Create now»".into(),
                values: Default::default(),
            }],
            dry_run: false,
        });
        self.persist()?;
        Ok(saved)
    }

    /// Retry a refused run: funds are checked at the current moment and the
    /// transaction is dated now.
    pub fn retry_automation_run(
        &mut self,
        run_id: &str,
        now: i64,
        zone: &str,
    ) -> Result<Transaction> {
        let pass = self.retry_workflow_run(run_id, now, zone)?;
        let made = pass
            .created
            .first()
            .ok_or_else(|| Error::InvalidInput("the run made no operation".into()))?;
        self.doc
            .transactions
            .iter()
            .find(|tx| &tx.meta().id == made)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("transaction {made}")))
    }

    /// Dismiss a refused run.
    pub fn skip_automation_run(&mut self, run_id: &str) -> Result<AutomationRun> {
        let run = self.skip_workflow_run(run_id)?;
        self.automation_run_of(&run)
            .ok_or_else(|| Error::Missing(format!("automation run {run_id}")))
    }

    /// Put in the real amount of an operation booked approximately: it is
    /// corrected with the usual checks and counts toward the next estimate.
    pub fn confirm_estimated_amount(&mut self, tx_id: &str, amount: Amount) -> Result<Transaction> {
        if !amount.is_positive() {
            return Err(Error::InvalidInput("the amount must be above zero".into()));
        }
        let mut tx = self
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == tx_id)
            .cloned()
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        if !tx.meta().origin.is_estimated() {
            return Err(Error::InvalidInput(
                "this operation was not booked with an approximate amount".into(),
            ));
        }
        set_main_amount(&mut tx, amount)?;
        self.update_transaction(tx_id, tx)?;
        self.clear_estimated(tx_id)
    }

    /// Drop the «approximate» mark of an operation; persisted when it changed.
    fn clear_estimated(&mut self, tx_id: &str) -> Result<Transaction> {
        let tx = self
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().id == tx_id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        let mut changed = false;
        if let TxOrigin::Workflow { estimated, .. } = &mut tx.meta_mut().origin
            && *estimated
        {
            *estimated = false;
            changed = true;
        }
        let out = tx.clone();
        if changed {
            self.persist()?;
        }
        Ok(out)
    }

    /// Funds check, then write; a shortfall is an `INSUFFICIENT_FUNDS` error.
    fn create_checked(&mut self, tx: Transaction, now: i64) -> Result<Transaction> {
        if let Some(reason) = self.funds_shortfall(&tx, now) {
            return Err(match reason {
                RejectReason::Invalid { message } => Error::InvalidInput(message),
                other => Error::InsufficientFunds(self.describe_shortfall(&other)),
            });
        }
        let saved = self.insert_transaction(tx, now)?;
        self.recompute_holdings();
        Ok(saved)
    }

    /// «Card: 500 USD of 800 USD» — with the store's name, for the error text.
    pub(crate) fn describe_shortfall(&self, reason: &RejectReason) -> String {
        let name = |id: &str| {
            self.find_store(id)
                .map_or_else(|| id.to_string(), |store| store.name.clone())
        };
        match reason {
            RejectReason::InsufficientFunds {
                store_id,
                asset,
                required,
                available,
            } => format!(
                "{}: {available} {sym} / {required} {sym}",
                name(store_id),
                sym = asset.symbol
            ),
            RejectReason::CreditLimitExceeded {
                store_id,
                limit,
                required,
            } => format!("{}: {required} > {limit}", name(store_id)),
            RejectReason::Invalid { message } => message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{LiabilityKind, LiabilityTerms};
    use crate::model::transaction::{AccountRef, PaymentTx, TopupTx};
    use crate::service::StoreExtras;
    use crate::service::WorkflowPass;
    use crate::service::test_support::*;

    fn secs(iso: &str) -> i64 {
        unix_secs(iso).unwrap()
    }

    fn salary_draft(store: &str, sum: &str) -> TransactionDraft {
        let Transaction::Topup(inner) = income(store, sum, "salary") else {
            unreachable!()
        };
        TransactionDraft::Topup(TopupTx { ..inner })
    }

    fn rent_draft(store: &str, sum: &str) -> TransactionDraft {
        let Transaction::Payment(mut inner) = payment(store, sum, "rent") else {
            unreachable!()
        };
        inner.meta.categories = vec!["rent_mortgage".into()];
        TransactionDraft::Payment(PaymentTx { ..inner })
    }

    fn monthly(
        vault: &mut UnlockedVault,
        name: &str,
        day: u8,
        template: TransactionDraft,
    ) -> Automation {
        vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: name.into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template,
                amount_mode: Default::default(),
                workday_shift: Default::default(),
                property_id: None,
            })
            .unwrap()
    }

    fn rejected_of(pass: &WorkflowPass) -> Vec<RejectReason> {
        pass.runs
            .iter()
            .filter_map(|run| match &run.outcome {
                RunOutcome::Rejected { reason, .. } => Some(reason.clone()),
                _ => None,
            })
            .collect()
    }

    fn outcomes(vault: &UnlockedVault, id: &str) -> Vec<&'static str> {
        let mut runs = vault.automation_runs(Some(id));
        runs.reverse();
        runs.iter()
            .map(|run| match run.outcome {
                AutomationOutcome::Created { .. } => "created",
                AutomationOutcome::Rejected { .. } => "rejected",
                AutomationOutcome::Skipped => "skipped",
            })
            .collect()
    }

    /// An emulation from today reaches months ahead: the runs are dated on
    /// the emulated days and must not trip the future-date rule (V11).
    #[test]
    fn an_emulation_from_today_runs_a_year_ahead() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
        let now = secs(&now_iso());

        let sim = vault
            .simulate_automations(Some(&salary.id), 365, now, "UTC")
            .unwrap();
        assert!(sim.runs.len() >= 12, "{} runs", sim.runs.len());
        assert_eq!(sim.rejected, 0, "{:?}", sim.runs);
        assert_eq!(sim.created as usize, sim.runs.len());
    }

    /// An automation may start months from now; a manual entry that far
    /// ahead is still refused.
    #[test]
    fn a_future_start_is_accepted_but_a_future_entry_is_not() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let later = calendar::iso(secs(&now_iso()) + 30 * 86_400);
        vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "Later".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 1 },
                starts_at: later.clone(),
                template: salary_draft(&card, "100"),
                amount_mode: Default::default(),
                workday_shift: Default::default(),
                property_id: None,
            })
            .unwrap();

        let mut tx = income(&card, "100", "salary");
        tx.meta_mut().timestamp = later;
        let err = vault.add_transaction(tx).unwrap_err();
        assert!(err.to_string().contains("too far in the future"), "{err}");
    }

    /// Emulation runs on a copy: what would happen is reported, nothing of it
    /// reaches the vault, and a later real run is unaffected.
    #[test]
    fn an_emulation_reports_the_runs_and_writes_nothing() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
        let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));
        let before = vault.doc.clone();
        let now = secs("2026-01-01T00:00:00Z");

        let all = vault.simulate_automations(None, 50, now, "UTC").unwrap();
        // Jan 5 rent: nothing yet. Jan 10 salary. Feb 5 rent. Feb 10 salary.
        assert_eq!(all.runs.len(), 4);
        assert_eq!((all.created, all.rejected), (3, 1));
        assert!(
            all.runs
                .windows(2)
                .all(|pair| pair[0].planned_at <= pair[1].planned_at)
        );
        let balance = all
            .balances
            .iter()
            .find(|row| row.store_id == card)
            .unwrap();
        assert_eq!(
            (balance.before, balance.after),
            (amount("0"), amount("1200"))
        );

        let only_rent = vault
            .simulate_automations(Some(&rent.id), 50, now, "UTC")
            .unwrap();
        assert!(
            only_rent
                .runs
                .iter()
                .all(|run| run.automation_id == rent.id)
        );
        assert_eq!(
            only_rent.runs.len(),
            2,
            "the salary still runs, it is just not listed"
        );

        assert_eq!(vault.doc, before, "the vault is untouched");
        let pass = vault
            .run_due_workflows(secs("2026-02-20T12:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(pass.created.len(), 3);
        assert!(
            vault
                .simulate_automations(Some("nope"), 10, now, "UTC")
                .is_err()
        );
        let _ = salary;
    }

    #[test]
    fn the_31st_runs_on_the_last_day_of_february() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let rent = monthly(&mut vault, "Rent", 31, rent_draft(&card, "1"));
        let dates: Vec<String> = vault
            .automation_occurrences(
                &rent,
                &Calendar::new("UTC"),
                secs("2026-01-01T00:00:00Z"),
                secs("2026-03-31T23:00:00Z"),
            )
            .into_iter()
            .map(|(moment, _)| calendar::iso(moment))
            .collect();
        assert_eq!(
            dates,
            vec![
                "2026-01-31T09:00:00Z",
                "2026-02-28T09:00:00Z",
                "2026-03-31T09:00:00Z"
            ]
        );
        let every = vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "Fortnight".into(),
                active: true,
                preset: None,
                schedule: Schedule::EveryNDays { days: 14 },
                starts_at: "2026-03-04T00:00:00Z".into(),
                template: rent_draft(&card, "1"),
                amount_mode: AmountMode::Fixed,
                workday_shift: WorkdayShift::None,
                property_id: None,
            })
            .unwrap();
        let dates: Vec<String> = vault
            .automation_occurrences(
                &every,
                &Calendar::new("UTC"),
                secs("2026-03-01T00:00:00Z"),
                secs("2026-04-01T00:00:00Z"),
            )
            .into_iter()
            .map(|(moment, _)| calendar::iso(moment))
            .collect();
        // Counted from the first date, not from some fixed day.
        assert_eq!(dates, vec!["2026-03-04T09:00:00Z", "2026-03-18T09:00:00Z"]);
    }

    #[test]
    fn salary_before_rent_covers_it_and_rent_before_salary_is_rejected() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
        let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));

        let pass = vault
            .run_due_workflows(secs("2026-02-20T12:00:00Z"), "UTC")
            .unwrap();
        // Jan 5 rent: nothing yet. Jan 10 salary. Feb 5 rent: 1000 ≥ 800. Feb 10 salary.
        assert_eq!(outcomes(&vault, &rent.id), vec!["rejected", "created"]);
        assert_eq!(outcomes(&vault, &salary.id), vec!["created", "created"]);
        assert_eq!(pass.created.len(), 3);
        let rejected = rejected_of(&pass);
        assert_eq!(rejected.len(), 1);
        let RejectReason::InsufficientFunds {
            required,
            available,
            ..
        } = &rejected[0]
        else {
            panic!("{rejected:?}");
        };
        assert_eq!((*required, *available), (amount("800"), amount("0")));
        assert_eq!(holding_of(&vault, &card, &usd()), amount("1200"));

        // The processed dates are not processed again.
        let again = vault
            .run_due_workflows(secs("2026-02-20T13:00:00Z"), "UTC")
            .unwrap();
        assert!(again.runs.is_empty() && again.created.is_empty());
    }

    #[test]
    fn the_fee_counts_toward_the_required_amount() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let mut seed = income(&card, "100", "seed");
        at(&mut seed, "2025-12-01T00:00:00Z");
        vault.add_transaction(seed).unwrap();
        let TransactionDraft::Payment(mut bill) = rent_draft(&card, "95") else {
            unreachable!()
        };
        bill.fee = fee_usd("10");
        let automation = monthly(&mut vault, "Bill", 1, TransactionDraft::Payment(bill));
        let pass = vault
            .run_due_workflows(secs("2026-01-02T00:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(outcomes(&vault, &automation.id), vec!["rejected"]);
        let RejectReason::InsufficientFunds { required, .. } = &rejected_of(&pass)[0] else {
            panic!()
        };
        assert_eq!(*required, amount("105"));
    }

    #[test]
    fn catch_up_creates_at_most_24_and_skips_the_rest() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let daily = vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "Pocket money".into(),
                active: true,
                preset: None,
                schedule: Schedule::EveryNDays { days: 1 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: salary_draft(&card, "1"),
                amount_mode: Default::default(),
                workday_shift: Default::default(),
                property_id: None,
            })
            .unwrap();
        let pass = vault
            .run_due_workflows(secs("2026-01-30T12:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(pass.created.len(), MAX_CATCH_UP);
        assert_eq!(pass.skipped, 30 - MAX_CATCH_UP);
        assert_eq!(vault.automation_runs(Some(&daily.id)).len(), 30);

        // Even with the processed marker rolled back nothing is created twice.
        vault.wf_set_mark(&daily.id, "schedule:t", "2025-12-31T00:00:00Z");
        let again = vault
            .run_due_workflows(secs("2026-01-30T12:00:00Z"), "UTC")
            .unwrap();
        assert!(
            again.runs.is_empty() && again.created.is_empty(),
            "{:?}",
            again
                .runs
                .iter()
                .map(|run| (&run.trigger, &run.outcome))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn credit_card_checks_the_limit_and_without_one_passes() {
        let (_dir, mut vault) = empty_vault();
        let terms = |limit: Option<&str>| LiabilityTerms {
            kind: LiabilityKind::CreditCard,
            rate: None,
            limit: limit.map(amount),
            due_day: None,
            monthly_payment: None,
        };
        let limited = vault
            .create_store_with(
                "Card",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(terms(Some("100"))),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let open = vault
            .create_store_with(
                "Open card",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(terms(None)),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let over = draft_to_transaction(
            &rent_draft(&limited, "150"),
            "a",
            "",
            "2026-01-01T09:00:00Z",
        );
        assert!(matches!(
            vault.funds_shortfall(&over, secs("2026-01-01T09:00:00Z")),
            Some(RejectReason::CreditLimitExceeded { .. })
        ));
        let within =
            draft_to_transaction(&rent_draft(&limited, "90"), "a", "", "2026-01-01T09:00:00Z");
        assert!(
            vault
                .funds_shortfall(&within, secs("2026-01-01T09:00:00Z"))
                .is_none()
        );
        let unlimited =
            draft_to_transaction(&rent_draft(&open, "5000"), "a", "", "2026-01-01T09:00:00Z");
        assert!(
            vault
                .funds_shortfall(&unlimited, secs("2026-01-01T09:00:00Z"))
                .is_none()
        );
    }

    #[test]
    fn incoming_money_and_a_contact_source_are_not_checked() {
        let (_dir, vault) = empty_vault();
        let topup = draft_to_transaction(&salary_draft("x", "100"), "a", "", "");
        assert!(vault.funds_shortfall(&topup, 0).is_none());
        let from_contact = transfer_between(
            AccountRef::Raw("Mom".into()),
            AccountRef::Store("x".into()),
            usd(),
            "100",
        );
        assert!(vault.funds_shortfall(&from_contact, 0).is_none());
    }

    #[test]
    fn retry_checks_the_current_balance_and_skip_dismisses() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));
        vault
            .run_due_workflows(secs("2026-02-20T12:00:00Z"), "UTC")
            .unwrap();
        let runs = vault.automation_runs(Some(&rent.id));
        assert_eq!(runs.len(), 2);

        let now = secs("2026-02-21T12:00:00Z");
        let err = vault
            .retry_automation_run(&runs[0].id, now, "UTC")
            .unwrap_err();
        assert_eq!(err.code(), "INSUFFICIENT_FUNDS");

        let mut salary = income(&card, "1000", "salary");
        at(&mut salary, "2026-02-21T10:00:00Z");
        vault.add_transaction(salary).unwrap();
        let created = vault.retry_automation_run(&runs[0].id, now, "UTC").unwrap();
        assert_eq!(created.meta().timestamp, "2026-02-21T12:00:00Z");
        assert!(matches!(
            vault.automation_runs(Some(&rent.id))[0].outcome,
            AutomationOutcome::Created { .. }
        ));

        let skipped = vault.skip_automation_run(&runs[1].id).unwrap();
        assert_eq!(skipped.outcome, AutomationOutcome::Skipped);
    }

    #[test]
    fn the_planned_moment_is_nine_in_the_local_zone() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        monthly(&mut vault, "Salary", 10, salary_draft(&card, "1"));
        // 09:00 in Bangkok on Jan 10 is 02:00 UTC.
        let pass = vault
            .run_due_workflows(secs("2026-01-10T02:30:00Z"), "Asia/Bangkok")
            .unwrap();
        assert_eq!(pass.created.len(), 1);
        let tx = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == pass.created[0])
            .unwrap();
        assert_eq!(tx.meta().timestamp, "2026-01-10T02:00:00Z");
    }

    #[test]
    fn a_run_made_days_after_its_plan_is_dated_when_it_ran() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        monthly(&mut vault, "Salary", 10, salary_draft(&card, "1"));
        // The app was closed on the 10th: the run is caught up on the 14th.
        let now = secs("2026-01-14T12:00:00Z");
        let pass = vault.run_due_workflows(now, "UTC").unwrap();
        assert_eq!(pass.created.len(), 1);
        let tx = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == pass.created[0])
            .unwrap();
        assert_eq!(tx.meta().timestamp, "2026-01-14T12:00:00Z");
        assert!(matches!(
            &tx.meta().origin,
            TxOrigin::Workflow { planned_at: Some(planned), .. } if planned == "2026-01-10T09:00:00Z"
        ));
    }

    #[test]
    fn a_template_that_breaks_the_rules_is_refused() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let TransactionDraft::Payment(mut bill) = rent_draft(&card, "10") else {
            unreachable!()
        };
        bill.meta.categories.clear();
        let err = vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "Bill".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 1 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Payment(bill),
                amount_mode: Default::default(),
                workday_shift: Default::default(),
                property_id: None,
            })
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");
    }

    fn advance(vault: &mut UnlockedVault, card: &str) -> Automation {
        vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "Advance".into(),
                active: true,
                preset: Some(AutomationPreset::Salary),
                schedule: Schedule::Monthly { day: 20 },
                starts_at: "2025-06-01T00:00:00Z".into(),
                template: salary_draft(card, "1000"),
                amount_mode: AmountMode::Varies,
                workday_shift: WorkdayShift::Back,
                property_id: None,
            })
            .unwrap()
    }

    fn made(vault: &UnlockedVault, pass: &WorkflowPass) -> Transaction {
        vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == pass.created[0])
            .unwrap()
            .clone()
    }

    fn salary_for_month(
        vault: &mut UnlockedVault,
        card: &str,
        day: u8,
        offset: i8,
        starts_at: &str,
    ) -> Automation {
        vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: Some(offset),
                name: format!("Pay {day}"),
                active: true,
                preset: Some(AutomationPreset::Salary),
                schedule: Schedule::Monthly { day },
                starts_at: starts_at.into(),
                template: salary_draft(card, "1000"),
                amount_mode: AmountMode::Fixed,
                workday_shift: WorkdayShift::Back,
                property_id: None,
            })
            .unwrap()
    }

    /// "For which month": the advance of Sunday July 20, 2025 comes on Friday
    /// the 18th and is for July; the salary of the 5th, a Sunday in Oct 2025
    /// (paid on Friday the 3rd), is for the month before. The shift changes
    /// neither.
    #[test]
    fn an_income_made_by_an_automation_carries_its_for_month() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let advance = salary_for_month(&mut vault, &card, 20, 0, "2025-07-01T00:00:00Z");
        assert_eq!(advance.for_month_offset, Some(0));
        let pass = vault
            .run_due_workflows(secs("2025-07-18T12:00:00Z"), "UTC")
            .unwrap();
        let Transaction::Topup(july) = made(&vault, &pass) else {
            panic!("a top-up")
        };
        assert_eq!(july.meta.timestamp, "2025-07-18T09:00:00Z");
        assert_eq!(july.details.for_month.as_deref(), Some("2025-07"));
        vault.delete_automation(&advance.id).unwrap();

        salary_for_month(&mut vault, &card, 5, -1, "2025-10-01T00:00:00Z");
        let pass = vault
            .run_due_workflows(secs("2025-10-03T12:00:00Z"), "UTC")
            .unwrap();
        let Transaction::Topup(october) = made(&vault, &pass) else {
            panic!("a top-up")
        };
        assert_eq!(october.meta.timestamp, "2025-10-03T09:00:00Z");
        assert_eq!(october.details.for_month.as_deref(), Some("2025-09"));
    }

    /// The advance of July 20, 2025, a Sunday, comes on Friday the 18th at the
    /// template's amount, marked approximate; the real amount put in makes the
    /// next estimate.
    #[test]
    fn a_salary_that_differs_is_booked_approximately_and_corrected() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let salary = advance(&mut vault, &card);
        assert_eq!(salary.estimate, None);

        let forecast = vault.automation_occurrences(
            &salary,
            &Calendar::new("UTC"),
            secs("2025-07-01T00:00:00Z"),
            secs("2025-07-31T00:00:00Z"),
        );
        assert_eq!(calendar::iso(forecast[0].0), "2025-07-18T09:00:00Z");
        assert!(forecast[0].1.meta().origin.is_estimated());

        // June 20, 2025 is a Friday; July 20 a Sunday.
        let pass = vault
            .run_due_workflows(secs("2025-06-20T10:00:00Z"), "UTC")
            .unwrap();
        let june = made(&vault, &pass);
        assert!(june.meta().origin.is_estimated());
        assert_eq!(main_amount(&june), Some(amount("1000")));
        let runs = vault.automation_runs(Some(&salary.id));
        assert!(runs[0].estimated);

        let corrected = vault
            .confirm_estimated_amount(&june.meta().id, amount("1300"))
            .unwrap();
        assert!(!corrected.meta().origin.is_estimated());
        assert_eq!(main_amount(&corrected), Some(amount("1300")));
        assert!(!vault.automation_runs(Some(&salary.id))[0].estimated);
        assert_eq!(
            vault.automation(&salary.id).unwrap().estimate,
            Some(amount("1300"))
        );
        // Only an approximate operation takes a real amount.
        assert!(
            vault
                .confirm_estimated_amount(&june.meta().id, amount("1"))
                .is_err()
        );

        let pass = vault
            .run_due_workflows(secs("2025-07-18T10:00:00Z"), "UTC")
            .unwrap();
        let july = made(&vault, &pass);
        assert_eq!(july.meta().timestamp, "2025-07-18T09:00:00Z");
        assert_eq!(main_amount(&july), Some(amount("1300")));
        // Not again on the Sunday it was planned for.
        let pass = vault
            .run_due_workflows(secs("2025-07-20T10:00:00Z"), "UTC")
            .unwrap();
        assert!(pass.created.is_empty());

        // An ordinary edit of the amount counts as the real one too.
        let mut edited = july.clone();
        if let Transaction::Topup(inner) = &mut edited {
            inner.amount = amount("1500");
        }
        let saved = vault.update_transaction(&july.meta().id, edited).unwrap();
        assert!(!saved.meta().origin.is_estimated());
        assert_eq!(
            vault.automation(&salary.id).unwrap().estimate,
            Some(amount("1400"))
        );
    }

    #[test]
    fn an_automation_is_edited_in_its_form_until_made_an_ordinary_workflow() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let salary = advance(&mut vault, &card);
        let workflow = vault.workflow(&salary.id).unwrap();
        let draft = crate::service::WorkflowDraft {
            name: workflow.name.clone(),
            description: String::new(),
            active: true,
            graph: workflow.graph.clone(),
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
        };
        let err = vault
            .save_workflow(Some(&salary.id), draft.clone(), secs(&now_iso()))
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");

        // A change in the form rebuilds the graph and keeps the old one.
        let mut changed = AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Advance".into(),
            active: true,
            preset: Some(AutomationPreset::Salary),
            schedule: Schedule::Monthly { day: 20 },
            starts_at: "2025-06-01T00:00:00Z".into(),
            template: salary_draft(&card, "1000"),
            amount_mode: AmountMode::Fixed,
            workday_shift: WorkdayShift::Forward,
            property_id: None,
        };
        vault
            .update_automation(&salary.id, changed.clone())
            .unwrap();
        let workflow = vault.workflow(&salary.id).unwrap();
        assert_eq!(workflow.versions.len(), 1);
        assert!(workflow.graph.contains("\"forward\""), "{}", workflow.graph);

        let ordinary = vault.detach_automation(&salary.id).unwrap();
        assert!(ordinary.automation.is_none());
        assert!(vault.automations().is_empty());
        vault
            .save_workflow(Some(&salary.id), draft, secs(&now_iso()))
            .unwrap();
        changed.name = "Again".into();
        assert!(vault.update_automation(&salary.id, changed).is_err());
    }

    #[test]
    fn an_exchange_cannot_have_an_amount_that_differs() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let Some(Transaction::Convert(mut inner)) = crate::model::fixtures::full_doc()
            .transactions
            .into_iter()
            .find(|tx| matches!(tx, Transaction::Convert(_)))
        else {
            unreachable!()
        };
        inner.from.account = crate::model::transaction::AccountRef::Store(card.clone());
        inner.to.account = crate::model::transaction::AccountRef::Store(card);
        let err = vault
            .create_automation(AutomationDraft {
                time: None,
                asks: false,
                for_month_offset: None,
                name: "DCA".into(),
                active: true,
                preset: None,
                schedule: Schedule::Weekly { weekday: 4 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Convert(inner),
                amount_mode: AmountMode::Varies,
                workday_shift: WorkdayShift::None,
                property_id: None,
            })
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");
    }

    #[test]
    fn a_repeat_fires_at_its_time_and_asks_before_booking() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let draft = AutomationDraft {
            time: Some("18:30".into()),
            asks: true,
            for_month_offset: None,
            name: "Asks".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 10 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: salary_draft(&card, "1000"),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        };
        let automation = vault.create_automation(draft.clone()).unwrap();
        assert_eq!(automation.time.as_deref(), Some("18:30"));
        assert!(automation.asks);

        let workflow = vault
            .doc
            .workflows
            .iter()
            .find(|wf| wf.id == automation.id)
            .unwrap();
        let graph = crate::workflow::graph::Graph::parse(&workflow.graph).unwrap();
        assert!(graph.nodes.iter().any(|node| node.kind == "action.confirm"));
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from_port == "true" && edge.to == graph_of::ACTION)
        );

        let cal = Calendar::new("UTC");
        let after = secs("2026-03-01T00:00:00Z");
        let moments = vault.automation_occurrences(&automation, &cal, after, after + 40 * 86_400);
        assert_eq!(calendar::iso(moments[0].0), "2026-03-10T18:30:00Z");

        let bad = AutomationDraft {
            time: Some("25:00".into()),
            ..draft
        };
        assert!(vault.create_automation(bad).is_err());
    }

    #[test]
    fn the_form_lists_the_next_three_dates_moved_off_days_off() {
        let input = crate::ipc_contract::AutomationDatesInput {
            schedule: Schedule::Monthly { day: 1 },
            starts_at: "2026-10-24T00:00:00Z".into(),
            workday_shift: WorkdayShift::Back,
            time: None,
        };
        let dates = input.upcoming(secs("2026-10-24T12:00:00Z"), "UTC");
        assert_eq!(dates.len(), 3);
        // 1 November 2026 is a Sunday: back to Friday.
        assert_eq!(dates[0], "2026-10-30T09:00:00Z");
        assert_eq!(dates[1], "2026-12-01T09:00:00Z");
    }
}
