//! Node workflows (plan §16): the node catalog, saving and checking graphs,
//! running and emulating them, the journal, the confirmation queue and the
//! variables. Everything lives in core; days are cut in the zone from
//! [`current_zone`].

use crate::ipc::events::{AppEvent, AutomationCreated, WorkflowsRan};
use tauri::{AppHandle, State};
use trade_core::ipc_contract::events::{AutomationCreatedPayload, WorkflowsRanPayload};

use trade_core::calendar::now_secs;
use trade_core::ipc_contract::{
    AnswerWorkflowWaitInput, DeleteWorkflowTestInput, DeleteWorkflowVarInput, DryRunWorkflowInput,
    ImportWorkflowInput, ListWorkflowRunsInput, RestoreWorkflowVersionInput, RunWorkflowInput,
    SaveWorkflowDraftInput, SaveWorkflowInput, SaveWorkflowTestInput, SetWorkflowActiveInput,
    SetWorkflowVarInput, ValidateWorkflowInput, WorkflowFromTemplateInput,
};
use trade_core::model::transaction::Transaction;
use trade_core::service::{
    SavedWorkflow, UnlockedVault, WorkflowImportPreview, WorkflowTestResult,
};
use trade_core::usecases::workflow::{records, runs};
use trade_core::workflow::service::{records as workflow_records, runs as run_reads, vars};
use trade_core::workflow::templates::WorkflowTemplateInfo;
use trade_core::workflow::{
    GraphIssue, NodeDef, QueuedActions, RunReport, Workflow, WorkflowRun, WorkflowVar,
    WorkflowWait, catalog, graph::Graph, validate,
};

use crate::commands::app_settings::current_zone;
use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use crate::ipc::ipc;

/// Most runs one listing returns by default.
const DEFAULT_RUNS: u32 = 100;

/// The write step of the commands that also emit or deliver after it (they
/// stay plain commands until the emit sites move, T21).
fn with_vault_mut<T>(
    state: &State<'_, AppState>,
    write: impl FnOnce(&mut UnlockedVault) -> trade_core::Result<T>,
) -> Result<T, IpcError> {
    let mut guard = require_vault(state)?;
    write(guard.as_mut().unwrap()).map_err(IpcError::from)
}

/// Every node there is, with its ports, fields and help (works locked too).
#[tauri::command]
pub async fn workflow_node_catalog() -> Result<Vec<NodeDef>, IpcError> {
    Ok(catalog().to_vec())
}

#[tauri::command]
pub async fn validate_workflow(input: ValidateWorkflowInput) -> Result<Vec<GraphIssue>, IpcError> {
    let graph = Graph::parse(&input.graph)?;
    Ok(validate(&graph))
}

#[ipc(read)]
pub fn list_workflows(vault: &UnlockedVault) -> trade_core::Result<Vec<Workflow>> {
    Ok(workflow_records::workflows(&vault.doc))
}

#[ipc(read)]
pub fn get_workflow(vault: &UnlockedVault, id: String) -> trade_core::Result<Workflow> {
    workflow_records::workflow(&vault.doc, &id)
}

#[ipc(write)]
pub fn save_workflow(
    vault: &mut UnlockedVault,
    input: SaveWorkflowInput,
) -> trade_core::Result<SavedWorkflow> {
    records::save_workflow(
        vault,
        input.id.as_deref().filter(|id| !id.is_empty()),
        input.workflow,
        now_secs(),
    )
}

/// Keep the editor's unsaved edits aside, to come back to them.
#[ipc(write)]
pub fn save_workflow_draft(
    vault: &mut UnlockedVault,
    input: SaveWorkflowDraftInput,
) -> trade_core::Result<Workflow> {
    records::save_workflow_draft(vault, &input.id, &input.graph, now_secs())
}

#[ipc(write)]
pub fn discard_workflow_draft(
    vault: &mut UnlockedVault,
    id: String,
) -> trade_core::Result<Workflow> {
    records::discard_workflow_draft(vault, &id)
}

#[ipc(write)]
pub fn set_workflow_active(
    vault: &mut UnlockedVault,
    input: SetWorkflowActiveInput,
) -> trade_core::Result<Workflow> {
    records::set_workflow_active(vault, &input.id, input.active, now_secs())
}

#[ipc(write)]
pub fn delete_workflow(vault: &mut UnlockedVault, id: String) -> trade_core::Result<()> {
    records::delete_workflow(vault, &id)
}

#[ipc(write)]
pub fn restore_workflow_version(
    vault: &mut UnlockedVault,
    input: RestoreWorkflowVersionInput,
) -> trade_core::Result<SavedWorkflow> {
    records::restore_workflow_version(vault, &input.id, &input.saved_at, now_secs())
}

/// Emulate on a copy of the vault: nothing is written.
#[ipc(read)]
pub fn dry_run_workflow(
    vault: &UnlockedVault,
    app: &AppHandle,
    input: DryRunWorkflowInput,
) -> trade_core::Result<RunReport> {
    runs::dry_run_workflow(
        vault,
        &input.graph,
        input.workflow_id.as_deref(),
        input.trigger_node.as_deref(),
        input.answers.unwrap_or_default(),
        now_secs(),
        &current_zone(app),
    )
}

/// Run for real now; its notices are shown like any other.
#[tauri::command]
pub async fn run_workflow_now(
    input: RunWorkflowInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RunReport, IpcError> {
    let zone = current_zone(&app);
    let ru = crate::scheduler::russian(&app);
    let report = with_vault_mut(&state, |vault| {
        vault.set_notice_language(ru);
        runs::run_workflow_now(
            vault,
            &input.id,
            input.trigger_node.as_deref(),
            now_secs(),
            &zone,
        )
    })?;
    crate::scheduler::deliver(&app, report.notices.clone(), false);
    if !report.run.created.is_empty() {
        let _ = AutomationCreated::emit(
            &app,
            &AutomationCreatedPayload {
                tx_ids: report.run.created.clone(),
            },
        );
    }
    Ok(report)
}

#[ipc(read)]
pub fn list_workflow_runs(
    vault: &UnlockedVault,
    input: ListWorkflowRunsInput,
) -> trade_core::Result<Vec<WorkflowRun>> {
    let limit = input.limit.unwrap_or(DEFAULT_RUNS) as usize;
    Ok(run_reads::runs(
        &vault.doc,
        input.workflow_id.as_deref(),
        limit,
    ))
}

#[ipc(read)]
pub fn list_workflow_queue(vault: &UnlockedVault) -> trade_core::Result<Vec<QueuedActions>> {
    Ok(run_reads::queue(&vault.doc))
}

#[tauri::command]
pub async fn confirm_workflow_queued(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, IpcError> {
    let saved = with_vault_mut(&state, |vault| records::confirm_queued(vault, &id))?;
    let ids: Vec<&str> = saved.iter().map(|tx| tx.meta().id.as_str()).collect();
    let _ = AutomationCreated::emit(
        &app,
        &AutomationCreatedPayload {
            tx_ids: ids.iter().map(ToString::to_string).collect(),
        },
    );
    Ok(saved)
}

#[ipc(write)]
pub fn discard_workflow_queued(vault: &mut UnlockedVault, id: String) -> trade_core::Result<()> {
    records::discard_queued(vault, &id)
}

/// Runs asleep: timers and questions waiting for an answer.
#[ipc(read)]
pub fn list_workflow_waits(vault: &UnlockedVault) -> trade_core::Result<Vec<WorkflowWait>> {
    Ok(run_reads::waits(&vault.doc))
}

/// Answer a "Wait for confirmation" question; the run goes on at once.
#[tauri::command]
pub async fn answer_workflow_wait(
    input: AnswerWorkflowWaitInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RunReport, IpcError> {
    let zone = current_zone(&app);
    let ru = crate::scheduler::russian(&app);
    let report = with_vault_mut(&state, |vault| {
        vault.set_notice_language(ru);
        runs::answer_workflow_wait(vault, &input.id, input.yes, now_secs(), &zone)
    })?;
    crate::scheduler::deliver(&app, report.notices.clone(), false);
    if !report.run.created.is_empty() {
        let _ = AutomationCreated::emit(
            &app,
            &AutomationCreatedPayload {
                tx_ids: report.run.created.clone(),
            },
        );
    }
    let _ = WorkflowsRan::emit(&app, &WorkflowsRanPayload::default());
    Ok(report)
}

#[ipc(read)]
pub fn list_workflow_vars(vault: &UnlockedVault) -> trade_core::Result<Vec<WorkflowVar>> {
    Ok(vars::variables(&vault.doc))
}

#[ipc(write)]
pub fn set_workflow_var(
    vault: &mut UnlockedVault,
    input: SetWorkflowVarInput,
) -> trade_core::Result<()> {
    records::set_workflow_var(vault, input.scope, &input.name, &input.value)
}

#[ipc(write)]
pub fn delete_workflow_var(
    vault: &mut UnlockedVault,
    input: DeleteWorkflowVarInput,
) -> trade_core::Result<()> {
    records::delete_workflow_var(vault, &input.scope, &input.name)
}

/// The workflow as a file's text.
#[ipc(read)]
pub fn export_workflow(vault: &UnlockedVault, id: String) -> trade_core::Result<String> {
    workflow_records::export_workflow(&vault.doc, &id)
}

/// Add a workflow from a file's text, switched off.
#[ipc(write)]
pub fn import_workflow(
    vault: &mut UnlockedVault,
    input: ImportWorkflowInput,
) -> trade_core::Result<SavedWorkflow> {
    records::import_workflow(vault, &input.json, &input.stores, now_secs())
}

/// What a workflow file holds and which of its wallets are missing here.
#[ipc(read)]
pub fn preview_workflow_import(
    vault: &UnlockedVault,
    json: String,
) -> trade_core::Result<WorkflowImportPreview> {
    workflow_records::preview_import(&vault.doc, &json)
}

/// The gallery of ready-made workflows with defaults from the vault.
#[ipc(read)]
pub fn workflow_templates(vault: &UnlockedVault) -> trade_core::Result<Vec<WorkflowTemplateInfo>> {
    Ok(records::workflow_templates(&vault.books()))
}

/// Build a template's workflow from its form and save it switched on.
#[ipc(write)]
pub fn workflow_from_template(
    vault: &mut UnlockedVault,
    app: &AppHandle,
    input: WorkflowFromTemplateInput,
) -> trade_core::Result<SavedWorkflow> {
    let locale = if crate::scheduler::russian(app) {
        "ru"
    } else {
        "en"
    };
    records::workflow_from_template(vault, &input.template_id, &input.params, locale, now_secs())
}

#[ipc(write)]
pub fn save_workflow_test(
    vault: &mut UnlockedVault,
    input: SaveWorkflowTestInput,
) -> trade_core::Result<Workflow> {
    records::save_workflow_test(vault, &input.workflow_id, input.test)
}

#[ipc(write)]
pub fn delete_workflow_test(
    vault: &mut UnlockedVault,
    input: DeleteWorkflowTestInput,
) -> trade_core::Result<Workflow> {
    records::delete_workflow_test(vault, &input.workflow_id, &input.test_id)
}

/// Every test of a workflow by emulation; nothing is written.
#[ipc(read)]
pub fn run_workflow_tests(
    vault: &UnlockedVault,
    app: &AppHandle,
    id: String,
) -> trade_core::Result<Vec<WorkflowTestResult>> {
    runs::run_workflow_tests(vault, &id, now_secs(), &current_zone(app))
}

ipc_commands!(workflows_commands, WORKFLOWS_NAMES, commands::workflows: [workflow_node_catalog, validate_workflow, list_workflows, get_workflow, save_workflow, save_workflow_draft, discard_workflow_draft, set_workflow_active, delete_workflow, restore_workflow_version, dry_run_workflow, run_workflow_now, list_workflow_runs, list_workflow_queue, confirm_workflow_queued, discard_workflow_queued, list_workflow_waits, answer_workflow_wait, list_workflow_vars, set_workflow_var, delete_workflow_var, export_workflow, import_workflow, preview_workflow_import, workflow_templates, workflow_from_template, save_workflow_test, delete_workflow_test, run_workflow_tests]);

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::ipc::run::VaultHost;

    #[derive(Clone)]
    struct TestHost(Arc<AppState>);

    impl VaultHost for TestHost {
        fn app_state(&self) -> &AppState {
            &self.0
        }
    }

    fn locked_host() -> TestHost {
        TestHost(Arc::new(AppState::new()))
    }

    #[tokio::test]
    async fn a_pilot_read_command_refuses_a_locked_vault() {
        let error = __ipc_run_list_workflows(locked_host()).await.unwrap_err();
        assert_eq!(error.code(), "REQUIRES_UNLOCK");
    }

    /// The pilot writes carry no sandbox gate (automations run on the demo
    /// data): in the sandbox the command reaches the vault check instead of
    /// being refused with `SANDBOX`.
    #[tokio::test]
    async fn a_pilot_write_command_has_no_sandbox_gate() {
        let host = locked_host();
        host.0.sandbox.store(true, Ordering::SeqCst);
        let input = DeleteWorkflowTestInput {
            workflow_id: String::new(),
            test_id: String::new(),
        };
        let error = __ipc_run_delete_workflow_test(host, input)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "REQUIRES_UNLOCK");
    }
}
