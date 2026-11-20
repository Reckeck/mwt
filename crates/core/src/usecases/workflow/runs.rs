//! Scenarios that run workflows: the scheduler pass, a run by hand, an
//! emulation, answering a question, retrying or skipping a refused run. A run
//! applies its effects to the vault through the host while it goes, so a pass
//! keeps each run in the journal in a `transact` of its own and ends with one
//! commit of what the host applied (spec core-strangler, F6).

use std::collections::BTreeMap;

use chrono::{Datelike, Duration};

use super::records::{SKIPPED, close_refused_as_done, mark_skipped};
use crate::Amount;
use crate::calendar::{Calendar, first_of_month};
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::kernel::tx::Tx;
use crate::model::notify::NoticeCategory;
use crate::model::planning::{RejectReason, describe_reject};
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::now_iso;
use crate::money::worth::FlowClass;
use crate::planning::automations::{MAX_CATCH_UP, outflow};
use crate::planning::schedule::{fresh_moments, processed_dates};
use crate::pricing::history::secs_or_now;
use crate::pricing::history::unix_secs;
use crate::service::UnlockedVault;
use crate::time::{Clock, FixedClock};
use crate::usecases::planning::budget::flows_between;
use crate::usecases::planning::notifications::notice;
use crate::workflow::RunKey;
use crate::workflow::automation;
use crate::workflow::catalog::node_def;
use crate::workflow::exec::{self, EmittedEvent, Resume, RunContext, RunMode, RunReport};
use crate::workflow::graph::{Graph, Node};
use crate::workflow::model::{
    MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, VarScope, WaitKind, Workflow, WorkflowRun,
    WorkflowWait,
};
use crate::workflow::nodes::triggers::{
    schedule_moments, schedule_moments_limited, tx_matches, tx_trigger_values,
};
use crate::workflow::service::marks;
use crate::workflow::service::records::workflow as workflow_record;
use crate::workflow::service::runs::{
    MAX_EVENT_DEPTH, MAX_RUNS_PER_HOUR, WorkflowPass, active_workflows, already_run,
    automation_processed, context, dropped_run, has_queue, legacy_moment_booked, moment_booked,
    pick_trigger, run_key_booked, runs_last_hour, sample_batch, sample_trigger_values,
    source_trigger_values, stamp_planned_date, threshold_value, trigger_label, wait_graph,
    wait_is_new,
};
use crate::workflow::service::tests_def::{WorkflowTestResult, result_of};
use crate::workflow::service::vars::set_var;
use crate::workflow::types::WfValue;
use crate::workflow::validate::{has_errors, validate};

// -- what the host does for a run -----------------------------------------------

/// Set one mark of a workflow. Applied to the document at once: the run or
/// the caller that asked commits.
pub fn wf_set_mark(vault: &mut UnlockedVault, workflow_id: &str, key: &str, value: &str) {
    marks::set_mark(&mut vault.doc, workflow_id, key, value);
}

/// Set a variable. Applied to the document at once, like the marks.
pub fn wf_set_var(vault: &mut UnlockedVault, scope: VarScope, name: &str, value: &WfValue) {
    set_var(&mut vault.doc, scope, name, value, now_iso());
}

/// A schedule mark never stays below a moment whose run key is booked:
/// whatever moved it back (an undo), the booked moments do not fire again.
pub fn keep_schedule_marks(vault: &mut UnlockedVault) {
    for (workflow_id, key, moment) in marks::raised_schedule_marks(&vault.doc) {
        marks::set_mark(&mut vault.doc, &workflow_id, &key, &moment);
    }
}

/// Check money and rules, then add the operations; all of them or none.
/// Not persisted: the run's caller writes once.
pub fn wf_insert(
    vault: &mut UnlockedVault,
    txs: Vec<Transaction>,
    now: i64,
) -> std::result::Result<Vec<Transaction>, String> {
    insert_checked(vault, txs, now).map_err(|refusal| match refusal {
        InsertRefusal::Short(reason) => describe_reject(&reason),
        InsertRefusal::Invalid(message) => message,
    })
}

/// Why `insert_checked` booked nothing.
enum InsertRefusal {
    /// The money is short: the one place that says so.
    Short(Box<RejectReason>),
    Invalid(String),
}

fn insert_checked(
    vault: &mut UnlockedVault,
    txs: Vec<Transaction>,
    now: i64,
) -> std::result::Result<Vec<Transaction>, InsertRefusal> {
    let backup = vault.doc.clone();
    let flow = (txs.len() > 1).then(|| uuid::Uuid::new_v4().to_string());
    let mut saved = Vec::with_capacity(txs.len());
    for mut tx in txs {
        if let Some(flow) = &flow {
            tx.meta_mut().flow_id = Some(flow.clone());
        }
        let secs = secs_or_now(&tx.meta().timestamp);
        let result = match vault.funds_shortfall(&tx, secs) {
            Some(reason) => Err(InsertRefusal::Short(Box::new(reason))),
            // The run's clock, not the wall one: an emulated run days
            // ahead dates its operations days ahead too.
            None => vault
                .prepare_transaction_at(&mut tx, now.max(secs) * 1000)
                .map_err(|err| InsertRefusal::Invalid(err.to_string())),
        };
        if let Err(refusal) = result {
            vault.doc = backup;
            return Err(refusal);
        }
        vault.doc.transactions.push(tx.clone());
        vault.recompute_holdings();
        saved.push(tx);
    }
    Ok(saved)
}

// -- lookups that cross areas ----------------------------------------------------

/// Income or spending of the current period, in the base currency.
pub fn wf_sum(
    books: &Books<'_>,
    period: &str,
    flow: &str,
    category: Option<&str>,
    now: i64,
    zone: &str,
) -> Amount {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let start = match period {
        "day" => today,
        "week" => today - Duration::days(i64::from(today.weekday().num_days_from_monday())),
        "year" => today.with_ordinal(1).unwrap_or(today),
        _ => first_of_month(today),
    };
    let base = books.base_and_fx().0;
    flows_between(books, &cal, base, cal.start_of(start), now + 1)
        .into_iter()
        .filter(|item| match flow {
            "income" => item.class == FlowClass::Income,
            _ => matches!(item.class, FlowClass::Fixed | FlowClass::Daily),
        })
        .filter(|item| category.is_none_or(|key| item.categories.iter().any(|known| known == key)))
        .fold(Amount::ZERO, |total, item| total + item.value)
        .round(2)
}

/// What an operation takes out of an account, in the base currency; zero when
/// there is no price or rate (the caller has no cap to enforce).
pub fn wf_outflow_base(books: &Books<'_>, tx: &Transaction) -> Amount {
    wf_outflow_in(books, tx, books.base_and_fx().0).unwrap_or(Amount::ZERO)
}

/// What an operation takes out of an account, in `currency`. `None` when the
/// asset has no price or the currency no rate: the value is unknown, which is
/// not the same as nothing.
pub fn wf_outflow_in(books: &Books<'_>, tx: &Transaction, currency: &str) -> Option<Amount> {
    let Some(out) = outflow(tx) else {
        return Some(Amount::ZERO);
    };
    if out.asset.is_fiat() && out.asset.symbol.eq_ignore_ascii_case(currency.trim()) {
        return Some(out.amount);
    }
    let book = books.rate_book();
    let secs = secs_or_now(&tx.meta().timestamp);
    book.price_at(&out.asset.key(), secs)
        .and_then(|usd| book.convert(out.amount * usd, "usd", currency, secs))
}

/// The host applied the run's effects straight to the document: one touched
/// transaction commits them.
fn commit_applied(vault: &mut UnlockedVault) -> Result<()> {
    vault.transact(|tx| {
        tx.touch();
        Ok(())
    })
}

/// Take a run out of the journal as a deletion that reaches the other devices
/// (a tombstone). Not committed: the caller's commit writes it.
fn forget_run(vault: &mut UnlockedVault, run_id: &str) -> Result<()> {
    vault
        .run(|tx| tx.collection::<WorkflowRun>().remove(run_id).map(drop))
        .map(drop)
}

/// Take a wait out. Not committed: the caller's commit writes it.
fn forget_wait(vault: &mut UnlockedVault, wait_id: &str) -> Result<()> {
    vault
        .run(|tx| tx.collection::<WorkflowWait>().remove(wait_id).map(drop))
        .map(drop)
}

// -- keeping what a run left -----------------------------------------------------

/// The line a notice about a question tells, in the language of the device.
fn question_text(ru: bool, question: &str) -> String {
    match (ru, question.is_empty()) {
        (true, true) => "Нужен ваш ответ, чтобы продолжить".to_string(),
        (true, false) => format!("Нужен ваш ответ: {question}"),
        (false, true) => "Your answer is needed to go on".to_string(),
        (false, false) => format!("Your answer is needed: {question}"),
    }
}

/// Journal entry, queue and wait of one run, written into `tx`.
fn write_run(tx: &mut Tx<'_>, report: &RunReport, pass: &mut WorkflowPass, ru: bool) -> Result<()> {
    tx.collection::<WorkflowRun>().insert(report.run.clone())?;
    let doc = tx.doc_mut();
    let overflow = doc.workflow_runs.len().saturating_sub(MAX_WORKFLOW_RUNS);
    doc.workflow_runs.drain(..overflow);
    for batch in &report.queued {
        tx.collection::<QueuedActions>().insert(batch.clone())?;
    }
    if let Some(wait) = &report.wait
        && wait_is_new(tx.doc(), &wait.id)
    {
        tx.collection::<WorkflowWait>().insert(wait.clone())?;
        if wait.kind == WaitKind::Confirm && !report.run.dry_run {
            let title = tx
                .collection::<Workflow>()
                .get(&wait.workflow_id)
                .map(|workflow| workflow.name.clone())
                .unwrap_or_default();
            pass.notices.push(notice(
                NoticeCategory::Automation,
                format!("wf-ask:{}", wait.id),
                &title,
                question_text(ru, &wait.question),
                Some(crate::routes::AUTOMATIONS_JOURNAL.to_string()),
                wait.created_at.clone(),
            ));
        }
    }
    pass.absorb(report);
    Ok(())
}

/// Keep what a live run left: its journal entry, queue, wait and notices.
pub(crate) fn keep_run(
    vault: &mut UnlockedVault,
    report: &RunReport,
    pass: &mut WorkflowPass,
) -> Result<()> {
    let ru = vault.notice_in_russian();
    vault.transact(|tx| write_run(tx, report, pass, ru))
}

/// A run that never happened, in the journal with its reason, so nothing is
/// lost without a trace.
fn keep_dropped(
    vault: &mut UnlockedVault,
    workflow_id: &str,
    trigger: &str,
    reason: &str,
    now: i64,
    pass: &mut WorkflowPass,
) -> Result<()> {
    let run = dropped_run(
        workflow_id,
        trigger,
        reason,
        now,
        uuid::Uuid::new_v4().to_string(),
    );
    vault.transact(|tx| tx.collection::<WorkflowRun>().insert(run.clone()))?;
    pass.runs.push(run);
    Ok(())
}

/// Log the notices a pass made; the ones that are new come back.
fn log_pass_notices(vault: &mut UnlockedVault, pass: &mut WorkflowPass) -> Result<()> {
    if !pass.notices.is_empty() {
        pass.notices = vault.log_notices(std::mem::take(&mut pass.notices))?;
    }
    Ok(())
}

// -- firing ---------------------------------------------------------------------

/// Run one trigger for real and keep the result. Returns the events sent.
fn fire(
    vault: &mut UnlockedVault,
    workflow: &Workflow,
    graph: &Graph,
    mut ctx: RunContext,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    // Another device (or an earlier pass) already ran this occasion.
    if already_run(&vault.doc, &ctx) {
        return Ok(Vec::new());
    }
    if runs_last_hour(&vault.doc, &workflow.id, ctx.now) >= MAX_RUNS_PER_HOUR {
        keep_dropped(
            vault,
            &workflow.id,
            &ctx.trigger_label,
            "rate_limit",
            ctx.now,
            pass,
        )?;
        return Ok(Vec::new());
    }
    ctx.mode = RunMode::Live;
    let report = exec::run(vault, graph, &ctx);
    keep_run(vault, &report, pass)?;
    Ok(report.events)
}

/// Several firings of one trigger in a pass: with a "Queue" in the workflow,
/// one run that hands them on one by one; otherwise a run each.
fn fire_all(
    vault: &mut UnlockedVault,
    workflow: &Workflow,
    graph: &Graph,
    mut firings: Vec<RunContext>,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    firings.retain(|ctx| !already_run(&vault.doc, ctx));
    if firings.len() > 1 && has_queue(graph) {
        let batch: Vec<_> = firings
            .iter()
            .map(|ctx| ctx.trigger_values.clone())
            .collect();
        let mut first = firings.into_iter().next().unwrap_or_else(|| unreachable!());
        first.trigger_label = format!("{} ×{}", first.trigger_label, batch.len());
        first.batch = batch;
        return fire(vault, workflow, graph, first, pass);
    }
    let mut events = Vec::new();
    for ctx in firings {
        events.extend(fire(vault, workflow, graph, ctx, pass)?);
    }
    Ok(events)
}

/// Run the workflows listening for events, as deep as `MAX_EVENT_DEPTH`.
pub(crate) fn run_events(
    vault: &mut UnlockedVault,
    mut events: Vec<EmittedEvent>,
    now: i64,
    zone: &str,
    pass: &mut WorkflowPass,
) -> Result<()> {
    for _depth in 0..MAX_EVENT_DEPTH {
        if events.is_empty() {
            return Ok(());
        }
        let mut next = Vec::new();
        let round = std::mem::take(&mut events);
        for workflow in active_workflows(&vault.doc) {
            let Ok(graph) = Graph::parse(&workflow.graph) else {
                continue;
            };
            let listeners: Vec<Node> = graph
                .nodes
                .iter()
                .filter(|node| !node.disabled && node.kind == "trigger.event")
                .cloned()
                .collect();
            for trigger in listeners {
                let listens = trigger
                    .params
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                // The same event sent several times this round: one queue.
                let firings: Vec<RunContext> = round
                    .iter()
                    .filter(|event| event.name == listens)
                    .map(|event| {
                        let mut ctx = context(&workflow, &trigger, now, zone);
                        ctx.trigger_label = trigger_label("event", &event.name);
                        ctx.trigger_values =
                            BTreeMap::from([("payload".to_string(), event.payload.clone())]);
                        ctx
                    })
                    .collect();
                next.extend(fire_all(vault, &workflow, &graph, firings, pass)?);
            }
        }
        events = next;
    }
    Ok(())
}

// -- a refused run -----------------------------------------------------------------

/// Run a refused run again now: the money is checked now and the operation
/// dated now, still naming the moment it was planned for. On success the
/// refused entry gives way to the new run; still short, it stays and the
/// error says why.
pub fn retry_workflow_run(
    vault: &mut UnlockedVault,
    run_id: &str,
    now: i64,
    zone: &str,
) -> Result<WorkflowPass> {
    let run = vault
        .doc
        .workflow_runs
        .iter()
        .find(|run| run.id == run_id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("workflow run {run_id}")))?;
    let RunOutcome::Rejected {
        trigger_node,
        planned_at,
        ..
    } = &run.outcome
    else {
        return Err(Error::InvalidInput(
            "only a refused run can be retried".into(),
        ));
    };
    // Another device may have booked this moment already (the refusal
    // is still here because their news has not come yet, or came
    // after it): then the refusal is stale and only goes.
    if let Some(at) = planned_at
        && moment_booked(&vault.doc, &run.workflow_id, at)
    {
        forget_run(vault, run_id)?;
        commit_applied(vault)?;
        return Ok(WorkflowPass::default());
    }
    // A refusal after a question has no trigger to start from (it would
    // ask again): its operation waits in the queue, and the retry books it.
    if let Some(batch) = vault
        .doc
        .workflow_queue
        .iter()
        .find(|batch| batch.run_id == run.id)
        .cloned()
    {
        return retry_queued_refusal(vault, &run, batch, now);
    }
    let workflow = workflow_record(&vault.doc, &run.workflow_id)?;
    let graph = Graph::parse(&workflow.graph)?;
    let trigger = graph
        .node(trigger_node)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("trigger {trigger_node}")))?;
    let mut ctx = context(&workflow, &trigger, now, zone);
    ctx.trigger_label.clone_from(&run.trigger);
    ctx.retry = true;
    if let Some(at) = planned_at {
        ctx.trigger_values = BTreeMap::from([("at".to_string(), WfValue::Date(at.clone()))]);
    }
    ctx.mode = RunMode::Live;
    let report = exec::run(vault, &graph, &ctx);
    if let RunOutcome::Rejected { reason, .. } = &report.run.outcome {
        return Err(match reason {
            RejectReason::Invalid { message } => Error::InvalidInput(message.clone()),
            other => Error::InsufficientFunds(vault.describe_shortfall(other)),
        });
    }
    forget_run(vault, run_id)?;
    let mut pass = WorkflowPass::default();
    keep_run(vault, &report, &mut pass)?;
    run_events(vault, report.events.clone(), now, zone, &mut pass)?;
    vault.recompute_holdings();
    commit_applied(vault)?;
    log_pass_notices(vault, &mut pass)?;
    Ok(pass)
}

/// Retry a refused run from its queued operations: they are dated now and the
/// money checked now. Still short, the refusal and the queue stay.
fn retry_queued_refusal(
    vault: &mut UnlockedVault,
    run: &WorkflowRun,
    batch: QueuedActions,
    now: i64,
) -> Result<WorkflowPass> {
    let mut txs = batch.transactions;
    for tx in &mut txs {
        tx.meta_mut().timestamp = crate::calendar::iso(now);
    }
    let saved = insert_checked(vault, txs, now).map_err(|refusal| match refusal {
        InsertRefusal::Short(reason) => Error::InsufficientFunds(vault.describe_shortfall(&reason)),
        InsertRefusal::Invalid(message) => Error::InvalidInput(message),
    })?;
    let created: Vec<String> = saved.iter().map(|tx| tx.meta().id.clone()).collect();
    vault.recompute_holdings();
    let done = vault.transact(|tx| {
        tx.collection::<QueuedActions>().remove(&batch.id)?;
        close_refused_as_done(tx, &run.id, created)?
            .ok_or_else(|| Error::Missing(format!("workflow run {}", run.id)))
    })?;
    let mut pass = WorkflowPass::default();
    pass.created.clone_from(&done.created);
    pass.runs.push(done);
    Ok(pass)
}

/// Dismiss a refused run: it stays in the journal as skipped.
pub fn skip_workflow_run(vault: &mut UnlockedVault, run_id: &str) -> Result<WorkflowRun> {
    vault.transact(|tx| {
        let is_refused = match tx.collection::<WorkflowRun>().get(run_id) {
            Some(run) => matches!(run.outcome, RunOutcome::Rejected { .. }),
            None => return Err(Error::Missing(format!("workflow run {run_id}"))),
        };
        if !is_refused {
            return Err(Error::InvalidInput(
                "only a refused run can be skipped".into(),
            ));
        }
        let skipped = mark_skipped(tx, run_id)?;
        // What a refusal queued goes with it: nothing is left to retry.
        let queued: Vec<String> = tx
            .collection::<QueuedActions>()
            .iter()
            .filter(|queued| queued.run_id == run_id)
            .map(|queued| queued.id.clone())
            .collect();
        for id in &queued {
            tx.collection::<QueuedActions>().remove(id)?;
        }
        Ok(skipped)
    })
}

// -- a run by hand, an emulation, the tests -----------------------------------------

/// Run a workflow now from a trigger (a "Run manually" one by default).
pub fn run_workflow_now(
    vault: &mut UnlockedVault,
    id: &str,
    trigger_node: Option<&str>,
    now: i64,
    zone: &str,
) -> Result<RunReport> {
    let workflow = workflow_record(&vault.doc, id)?;
    let graph = Graph::parse(&workflow.graph)?;
    if let Some(issue) = validate(&graph)
        .into_iter()
        .find(|issue| issue.level == crate::workflow::IssueLevel::Error)
    {
        return Err(Error::InvalidInput(issue.text_en));
    }
    let trigger = pick_trigger(&graph, trigger_node)?;
    let mut ctx = context(&workflow, &trigger, now, zone);
    // "Run manually" is the button's own trigger; any other is started
    // by hand to try it, and the journal says so.
    ctx.trigger_label = if trigger.kind == "trigger.manual" {
        trigger_label("manual", "")
    } else {
        trigger_label(&trigger.kind, "by hand")
    };
    ctx.trigger_values =
        sample_trigger_values(&*vault, &vault.doc.transactions, &trigger, now, zone);
    ctx.batch = sample_batch(
        &*vault,
        &vault.doc.transactions,
        &graph,
        &trigger,
        now,
        zone,
    );
    let mut pass = WorkflowPass::default();
    let report = exec::run(vault, &graph, &ctx);
    keep_run(vault, &report, &mut pass)?;
    run_events(vault, report.events.clone(), now, zone, &mut pass)?;
    vault.recompute_holdings();
    commit_applied(vault)?;
    if !pass.notices.is_empty() {
        vault.log_notices(pass.notices)?;
    }
    Ok(report)
}

/// Emulate a graph — saved or still in the editor — on a copy of the vault.
pub fn dry_run_workflow(
    vault: &UnlockedVault,
    graph_json: &str,
    workflow_id: Option<&str>,
    trigger_node: Option<&str>,
    answers: BTreeMap<String, bool>,
    now: i64,
    zone: &str,
) -> Result<RunReport> {
    let graph = Graph::parse(graph_json)?;
    let trigger = pick_trigger(&graph, trigger_node)?;
    let mut sandbox = vault.sandbox();
    let known = workflow_id.and_then(|id| workflow_record(&vault.doc, id).ok());
    let (id, name) = match &known {
        Some(workflow) => (workflow.id.clone(), workflow.name.clone()),
        None => ("draft".to_string(), "Черновик".to_string()),
    };
    let mut ctx = RunContext::new(&id, &name, &trigger.id, now, zone);
    if let Some(workflow) = &known {
        ctx.set_cap(workflow);
    }
    ctx.mode = RunMode::DryRun;
    ctx.trigger_label = trigger_label(&trigger.kind, "(emulation)");
    ctx.trigger_values = sample_trigger_values(vault, &vault.doc.transactions, &trigger, now, zone);
    ctx.batch = sample_batch(vault, &vault.doc.transactions, &graph, &trigger, now, zone);
    ctx.answers = answers;
    let mut report = exec::run(&mut sandbox, &graph, &ctx);
    // The workflows its events start run too, on the same copy, so the
    // emulation shows the whole chain a real run would set off.
    let mut pass = WorkflowPass::default();
    run_events(&mut sandbox, report.events.clone(), now, zone, &mut pass)?;
    report.event_runs = pass
        .runs
        .into_iter()
        .map(|run| WorkflowRun {
            dry_run: true,
            ..run
        })
        .collect();
    Ok(report)
}

/// Run every test of a workflow by emulation; nothing is written.
pub fn run_workflow_tests(
    vault: &UnlockedVault,
    workflow_id: &str,
    now: i64,
    zone: &str,
) -> Result<Vec<WorkflowTestResult>> {
    let workflow = workflow_record(&vault.doc, workflow_id)?;
    workflow
        .tests
        .iter()
        .map(|test| {
            let at = test.at.as_deref().and_then(unix_secs).unwrap_or(now);
            let report = dry_run_workflow(
                vault,
                &workflow.graph,
                Some(workflow_id),
                test.trigger_node.as_deref(),
                test.answers.clone().unwrap_or_default(),
                at,
                zone,
            )?;
            Ok(result_of(test, report))
        })
        .collect()
}

// -- a question answered --------------------------------------------------------------

/// Answer a "Wait for confirmation" question: the run goes on from "Yes" or
/// "No" right away.
pub fn answer_workflow_wait(
    vault: &mut UnlockedVault,
    id: &str,
    yes: bool,
    now: i64,
    zone: &str,
) -> Result<RunReport> {
    let wait = vault
        .doc
        .workflow_waits
        .iter()
        .find(|wait| wait.id == id && wait.kind == WaitKind::Confirm)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("question {id}")))?;
    let workflow = workflow_record(&vault.doc, &wait.workflow_id)?;
    if !workflow.active {
        return Err(Error::InvalidInput(
            "the workflow is switched off: switch it on to answer".into(),
        ));
    }
    if runs_last_hour(&vault.doc, &workflow.id, now) >= MAX_RUNS_PER_HOUR {
        return Err(Error::InvalidInput(format!(
            "the workflow ran {MAX_RUNS_PER_HOUR} times this hour: answer a bit later"
        )));
    }
    let port = if yes { "true" } else { "false" };
    let run_key = RunKey::Wait {
        wait_id: &wait.id,
        port,
    }
    .build();
    // The same answer was already given on another device: its result is
    // here, so the question only goes.
    if run_key_booked(&vault.doc, &run_key) {
        forget_wait(vault, id)?;
        commit_applied(vault)?;
        return Err(Error::Missing(format!("question {id}: already answered")));
    }
    forget_wait(vault, id)?;
    let graph = wait_graph(&wait, &workflow)?;
    if graph.node(&wait.node_id).is_none() {
        commit_applied(vault)?;
        return Err(Error::InvalidInput(
            "the question's node was removed from the workflow; the run is dropped".into(),
        ));
    }
    let mut ctx = RunContext::new(&workflow.id, &workflow.name, &wait.node_id, now, zone);
    ctx.set_cap(&workflow);
    ctx.trigger_label = format!("{} → {}", wait.trigger, if yes { "yes" } else { "no" });
    ctx.run_key = Some(run_key);
    ctx.resume = Some(Resume {
        node_id: wait.node_id.clone(),
        port: port.into(),
        memo: wait.memo.clone(),
    });
    let mut pass = WorkflowPass::default();
    let report = exec::run(vault, &graph, &ctx);
    keep_run(vault, &report, &mut pass)?;
    run_events(vault, report.events.clone(), now, zone, &mut pass)?;
    vault.recompute_holdings();
    commit_applied(vault)?;
    if !pass.notices.is_empty() {
        vault.log_notices(pass.notices)?;
    }
    Ok(report)
}

// -- the scheduler pass ------------------------------------------------------------------

/// Fire every trigger that is due: waits that are over, schedules, new
/// operations, thresholds crossed, then the events they sent.
pub fn run_due_workflows(vault: &mut UnlockedVault, now: i64, zone: &str) -> Result<WorkflowPass> {
    run_due_workflows_at(vault, &FixedClock(now), zone)
}

/// [`run_due_workflows`] at the moment `clock` reads.
pub fn run_due_workflows_at(
    vault: &mut UnlockedVault,
    clock: &dyn Clock,
    zone: &str,
) -> Result<WorkflowPass> {
    let now = clock.now_unix();
    let mut pass = WorkflowPass::default();
    let mut events = resume_due_waits(vault, now, zone, &mut pass)?;
    events.extend(fire_due_automations(vault, now, zone, &mut pass)?);
    // Automations fired above, strictly by time across all of them.
    for workflow in active_workflows(&vault.doc)
        .into_iter()
        .filter(|workflow| workflow.automation.is_none())
    {
        events.extend(fire_triggers(vault, &workflow, now, zone, &mut pass)?);
    }
    run_events(vault, events, now, zone, &mut pass)?;
    vault.recompute_holdings();
    commit_applied(vault)?;
    log_pass_notices(vault, &mut pass)?;
    Ok(pass)
}

/// Continue the runs whose wait is over: a timer goes on, a question nobody
/// answered in time counts as «No».
fn resume_due_waits(
    vault: &mut UnlockedVault,
    now: i64,
    zone: &str,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    let due: Vec<WorkflowWait> = vault
        .doc
        .workflow_waits
        .iter()
        .filter(|wait| unix_secs(&wait.resume_at).is_some_and(|secs| secs <= now))
        .cloned()
        .collect();
    let mut events = Vec::new();
    for wait in due {
        let Ok(workflow) = workflow_record(&vault.doc, &wait.workflow_id) else {
            forget_wait(vault, &wait.id)?;
            continue;
        };
        // Over the hourly limit: the run sleeps on and goes on at a later tick.
        if workflow.active && runs_last_hour(&vault.doc, &workflow.id, now) >= MAX_RUNS_PER_HOUR {
            continue;
        }
        forget_wait(vault, &wait.id)?;
        let graph = match wait_graph(&wait, &workflow) {
            Ok(graph) if graph.node(&wait.node_id).is_some() => graph,
            _ => {
                keep_dropped(
                    vault,
                    &workflow.id,
                    &wait.trigger,
                    "node_removed",
                    now,
                    pass,
                )?;
                continue;
            }
        };
        if !workflow.active {
            keep_dropped(vault, &workflow.id, &wait.trigger, "off", now, pass)?;
            continue;
        }
        let mut ctx = RunContext::new(&workflow.id, &workflow.name, &wait.node_id, now, zone);
        ctx.set_cap(&workflow);
        ctx.trigger_label = format!("{} → wait", wait.trigger);
        let port = match wait.kind {
            WaitKind::Timer => "out",
            WaitKind::Confirm => "false",
        };
        ctx.run_key = Some(
            RunKey::Wait {
                wait_id: &wait.id,
                port,
            }
            .build(),
        );
        ctx.resume = Some(Resume {
            node_id: wait.node_id.clone(),
            port: port.into(),
            memo: wait.memo.clone(),
        });
        events.extend(fire(vault, &workflow, &graph, ctx, pass)?);
    }
    Ok(events)
}

/// Fire the due planned moments of every automation strictly by time across
/// all of them, so a salary on the 10th covers rent on the 15th (plan §7.2).
/// Only the newest [`MAX_CATCH_UP`] of an automation run; older ones are kept
/// in the journal as skipped. Automations are not held to the hourly limit of
/// workflows: a catch-up after a month away is expected to book many at once.
fn fire_due_automations(
    vault: &mut UnlockedVault,
    now: i64,
    zone: &str,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    // One zone for every moment and date of an automation, the one
    // ordinary workflows use too: the device may travel, a date stays.
    let zone = marks::schedule_zone(&mut vault.doc, zone);
    let zone = zone.as_str();
    let cal = Calendar::new(zone);
    let processed = automation_processed(&vault.doc);
    let key = automation::schedule_key();
    let mut due: Vec<(i64, Workflow, Graph, Node)> = Vec::new();
    for workflow in active_workflows(&vault.doc) {
        let Some(spec) = workflow.automation.clone() else {
            continue;
        };
        let Ok(graph) = Graph::parse(&workflow.graph) else {
            continue;
        };
        let Some(trigger) = graph.node(automation::TRIGGER).cloned() else {
            continue;
        };
        // Nothing is planned before the automation's start.
        let start = unix_secs(&automation::start_mark(&spec)).unwrap_or(now);
        let after = marks::mark(&vault.doc, &workflow.id, &key)
            .and_then(|mark| unix_secs(&mark))
            .map_or(start, |mark| mark.max(start));
        let (moments, _) = schedule_moments_limited(&trigger.params, &cal, after, now, usize::MAX);
        let done = processed_dates(&processed, &workflow.id, &cal);
        let fresh = fresh_moments(&moments, &done, &cal);
        let overflow = fresh.len().saturating_sub(MAX_CATCH_UP);
        for moment in &fresh[..overflow] {
            let label = trigger_label(&trigger.kind, &crate::calendar::iso(*moment));
            keep_dropped(vault, &workflow.id, &label, SKIPPED, now, pass)?;
            pass.skipped += 1;
        }
        for moment in &fresh[overflow..] {
            due.push((*moment, workflow.clone(), graph.clone(), trigger.clone()));
        }
        if let Some(last) = moments.last() {
            marks::set_mark(
                &mut vault.doc,
                &workflow.id,
                &key,
                &crate::calendar::iso(*last),
            );
        }
    }
    due.sort_by_key(|(moment, ..)| *moment);
    let mut events = Vec::new();
    for (moment, workflow, graph, trigger) in due {
        let at = crate::calendar::iso(moment);
        let mut ctx = context(&workflow, &trigger, now, zone);
        ctx.trigger_label = trigger_label(&trigger.kind, &at);
        ctx.trigger_values = BTreeMap::from([("at".to_string(), WfValue::Date(at))]);
        ctx.mode = RunMode::Live;
        let before = vault.doc.transactions.len();
        let mut report = exec::run(vault, &graph, &ctx);
        link_to_property(vault, &workflow, before);
        let planned = cal.date_of(moment);
        stamp_planned_date(&mut vault.doc, &report.run.created, planned);
        report.run.planned_date = Some(planned);
        keep_run(vault, &report, pass)?;
        events.extend(report.events);
    }
    Ok(events)
}

/// What an automation books belongs to its property (plan G4): the operations
/// added since `before` get the link.
fn link_to_property(vault: &mut UnlockedVault, workflow: &Workflow, before: usize) {
    let Some(property) = workflow
        .automation
        .as_ref()
        .and_then(|spec| spec.property_id.clone())
        .filter(|id| vault.check_property_link(Some(id)).is_ok())
    else {
        return;
    };
    let made: Vec<String> = vault.doc.transactions[before.min(vault.doc.transactions.len())..]
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect();
    for id in made {
        vault.set_property_link(&id, Some(&property));
    }
}

/// What the handlers of one workflow's triggers share in a pass.
struct Firing<'run> {
    workflow: &'run Workflow,
    graph: &'run Graph,
    /// The operations added since the workflow last looked.
    fresh: &'run [Transaction],
    now: i64,
    zone: &'run str,
}

/// Fire the triggers of an ordinary workflow that are due.
fn fire_triggers(
    vault: &mut UnlockedVault,
    workflow: &Workflow,
    now: i64,
    zone: &str,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    let Ok(graph) = Graph::parse(&workflow.graph) else {
        return Ok(Vec::new());
    };
    if has_errors(&validate(&graph)) {
        return Ok(Vec::new());
    }
    let triggers: Vec<Node> = graph
        .nodes
        .iter()
        .filter(|node| !node.disabled && node_def(&node.kind).is_some_and(|def| def.is_trigger()))
        .cloned()
        .collect();
    let (fresh, len, last) = marks::new_transactions(&vault.doc, &workflow.id);
    let firing = Firing {
        workflow,
        graph: &graph,
        fresh: &fresh,
        now,
        zone,
    };
    let mut events = Vec::new();
    for trigger in &triggers {
        events.extend(match trigger.kind.as_str() {
            "trigger.schedule" => fire_schedule(vault, &firing, trigger, pass)?,
            "trigger.transaction" => fire_transactions(vault, &firing, trigger, pass)?,
            "trigger.source" => fire_source(vault, &firing, trigger, pass)?,
            "trigger.balance" | "trigger.price" => fire_threshold(vault, &firing, trigger, pass)?,
            _ => Vec::new(),
        });
    }
    // Everything up to here was looked at; what the runs added is seen next pass.
    marks::mark_seen(&mut vault.doc, &workflow.id, len, last);
    Ok(events)
}

fn fire_schedule(
    vault: &mut UnlockedVault,
    firing: &Firing<'_>,
    trigger: &Node,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    let Firing {
        workflow,
        graph,
        now,
        ..
    } = *firing;
    let zone = marks::schedule_zone(&mut vault.doc, firing.zone);
    let zone = zone.as_str();
    let key = format!("schedule:{}", trigger.id);
    let Some(after) = marks::mark(&vault.doc, &workflow.id, &key).and_then(|mark| unix_secs(&mark))
    else {
        marks::set_mark(
            &mut vault.doc,
            &workflow.id,
            &key,
            &crate::calendar::iso(now),
        );
        return Ok(Vec::new());
    };
    let (moments, _skipped) = schedule_moments(&trigger.params, &Calendar::new(zone), after, now);
    let firings: Vec<RunContext> = moments
        .iter()
        .map(|moment| crate::calendar::iso(*moment))
        .filter(|at| !legacy_moment_booked(&vault.doc, &workflow.id, at))
        .map(|at| {
            let mut ctx = context(workflow, trigger, now, zone);
            ctx.run_key = Some(
                RunKey::Schedule {
                    workflow_id: &workflow.id,
                    trigger_id: &trigger.id,
                    moment: &at,
                }
                .build(),
            );
            ctx.trigger_label = trigger_label(&trigger.kind, &at);
            ctx.trigger_values = BTreeMap::from([("at".to_string(), WfValue::Date(at))]);
            ctx
        })
        .collect();
    let events = fire_all(vault, workflow, graph, firings, pass)?;
    if let Some(last) = moments.last() {
        marks::set_mark(
            &mut vault.doc,
            &workflow.id,
            &key,
            &crate::calendar::iso(*last),
        );
    }
    Ok(events)
}

fn fire_transactions(
    vault: &mut UnlockedVault,
    firing: &Firing<'_>,
    trigger: &Node,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    let Firing {
        workflow,
        graph,
        now,
        zone,
        fresh,
    } = *firing;
    let mut params = trigger.params.clone();
    if workflow.react_to_workflows {
        params.insert("include_workflows".into(), serde_json::Value::Bool(true));
    }
    let mut firings = Vec::new();
    for tx in fresh.iter().filter(|tx| tx_matches(&*vault, &params, tx)) {
        // A workflow never reacts to what it made itself.
        if matches!(&tx.meta().origin, TxOrigin::Workflow { workflow_id, .. } if *workflow_id == workflow.id)
        {
            continue;
        }
        let mut ctx = context(workflow, trigger, now, zone);
        ctx.run_key = Some(
            RunKey::Transaction {
                workflow_id: &workflow.id,
                trigger_id: &trigger.id,
                tx_id: &tx.meta().id,
            }
            .build(),
        );
        ctx.trigger_label = trigger_label(&trigger.kind, &tx.meta().id);
        ctx.trigger_values = tx_trigger_values(tx);
        firings.push(ctx);
    }
    fire_all(vault, workflow, graph, firings, pass)
}

fn fire_source(
    vault: &mut UnlockedVault,
    firing: &Firing<'_>,
    trigger: &Node,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    let Firing {
        workflow,
        graph,
        now,
        zone,
        ..
    } = *firing;
    let Some(id) = trigger
        .params
        .get("source")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(Vec::new());
    };
    let Some(last) = crate::planning::datasources::data_source(&vault.doc, id)
        .and_then(|source| source.last.clone())
    else {
        return Ok(Vec::new());
    };
    let wanted = trigger
        .params
        .get("condition")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("changed");
    let key = format!("source:{}", trigger.id);
    let previous = marks::mark(&vault.doc, &workflow.id, &key);
    // «Changed» remembers the moment of the last value seen, a threshold the
    // side it was on.
    let mark = if wanted == "changed" {
        let number = last.number.map(|number| number.to_string());
        format!("{}|{}", last.at, number.unwrap_or_default())
    } else {
        let Some(number) = last.number else {
            return Ok(Vec::new());
        };
        let threshold = trigger
            .params
            .get("value")
            .and_then(crate::workflow::nodes::json_number)
            .unwrap_or(Amount::ZERO);
        if number >= threshold {
            "above"
        } else {
            "below"
        }
        .to_string()
    };
    if previous.as_deref() == Some(mark.as_str()) {
        return Ok(Vec::new());
    }
    // The first look only remembers: a value that was there before the
    // workflow is not news.
    let fires = match wanted {
        "changed" => previous.is_some(),
        side => side == mark,
    };
    let mut events = Vec::new();
    if fires {
        let mut ctx = context(workflow, trigger, now, zone);
        ctx.trigger_label = trigger_label(&trigger.kind, &last.at);
        ctx.trigger_values = source_trigger_values(&*vault, trigger).unwrap_or_default();
        events = fire(vault, workflow, graph, ctx, pass)?;
    }
    marks::set_mark(&mut vault.doc, &workflow.id, &key, &mark);
    Ok(events)
}

fn fire_threshold(
    vault: &mut UnlockedVault,
    firing: &Firing<'_>,
    trigger: &Node,
    pass: &mut WorkflowPass,
) -> Result<Vec<EmittedEvent>> {
    let Firing {
        workflow,
        graph,
        now,
        zone,
        ..
    } = *firing;
    let Some((value, above)) = threshold_value(&*vault, trigger, now) else {
        return Ok(Vec::new());
    };
    let key = format!("threshold:{}", trigger.id);
    let side = if above { "above" } else { "below" };
    let previous = marks::mark(&vault.doc, &workflow.id, &key);
    if previous.as_deref() == Some(side) {
        return Ok(Vec::new());
    }
    let wanted = trigger
        .params
        .get("condition")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("below");
    let mut events = Vec::new();
    if wanted == side {
        let port = if trigger.kind == "trigger.price" {
            "price"
        } else {
            "balance"
        };
        let mut ctx = context(workflow, trigger, now, zone);
        ctx.trigger_label = trigger_label(&trigger.kind, &value.as_text());
        ctx.trigger_values = BTreeMap::from([(port.to_string(), value)]);
        events = fire(vault, workflow, graph, ctx, pass)?;
    }
    marks::set_mark(&mut vault.doc, &workflow.id, &key, side);
    Ok(events)
}
