//! Node workflows (plan §16): the node catalog, saving and checking graphs,
//! running and emulating them, the journal, the confirmation queue and the
//! variables. Everything lives in core; days are cut in the zone from
//! [`current_zone`].

use tauri::{AppHandle, Emitter, State};

use trade_core::calendar::now_secs;
use trade_core::ipc_contract::{
    AnswerWorkflowWaitInput, DeleteWorkflowTestInput, DeleteWorkflowVarInput, DryRunWorkflowInput,
    ListWorkflowRunsInput, RestoreWorkflowVersionInput, RunWorkflowInput, SaveWorkflowDraftInput,
    SaveWorkflowInput, SaveWorkflowTestInput, SetWorkflowActiveInput, SetWorkflowVarInput,
    ValidateWorkflowInput,
};
use trade_core::model::transaction::Transaction;
use trade_core::service::{SavedWorkflow, UnlockedVault, WorkflowTestResult};
use trade_core::workflow::{
    GraphIssue, NodeDef, QueuedActions, RunReport, Workflow, WorkflowRun, WorkflowVar,
    WorkflowWait, catalog, graph::Graph, validate,
};

use crate::commands::app_settings::current_zone;
use crate::commands::{AppState, err_to_string, require_vault};

/// Most runs one listing returns by default.
const DEFAULT_RUNS: u32 = 100;

fn with_vault<T>(
    state: &State<'_, AppState>,
    read: impl FnOnce(&UnlockedVault) -> trade_core::Result<T>,
) -> Result<T, String> {
    let guard = require_vault(state).map_err(err_to_string)?;
    read(guard.as_ref().unwrap()).map_err(err_to_string)
}

fn with_vault_mut<T>(
    state: &State<'_, AppState>,
    write: impl FnOnce(&mut UnlockedVault) -> trade_core::Result<T>,
) -> Result<T, String> {
    let mut guard = require_vault(state).map_err(err_to_string)?;
    write(guard.as_mut().unwrap()).map_err(err_to_string)
}

/// Every node there is, with its ports, fields and help (works locked too).
#[tauri::command]
pub async fn workflow_node_catalog() -> Result<Vec<NodeDef>, String> {
    Ok(catalog().to_vec())
}

#[tauri::command]
pub async fn validate_workflow(input: ValidateWorkflowInput) -> Result<Vec<GraphIssue>, String> {
    let graph = Graph::parse(&input.graph).map_err(err_to_string)?;
    Ok(validate(&graph))
}

#[tauri::command]
pub async fn list_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String> {
    with_vault(&state, |vault| Ok(vault.workflows()))
}

#[tauri::command]
pub async fn get_workflow(id: String, state: State<'_, AppState>) -> Result<Workflow, String> {
    with_vault(&state, |vault| vault.workflow(&id))
}

#[tauri::command]
pub async fn save_workflow(
    input: SaveWorkflowInput,
    state: State<'_, AppState>,
) -> Result<SavedWorkflow, String> {
    with_vault_mut(&state, |vault| {
        vault.save_workflow(
            input.id.as_deref().filter(|id| !id.is_empty()),
            input.workflow,
            now_secs(),
        )
    })
}

/// Keep the editor's unsaved edits aside, to come back to them.
#[tauri::command]
pub async fn save_workflow_draft(
    input: SaveWorkflowDraftInput,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    with_vault_mut(&state, |vault| {
        vault.save_workflow_draft(&input.id, &input.graph, now_secs())
    })
}

#[tauri::command]
pub async fn discard_workflow_draft(
    id: String,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    with_vault_mut(&state, |vault| vault.discard_workflow_draft(&id))
}

#[tauri::command]
pub async fn set_workflow_active(
    input: SetWorkflowActiveInput,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    with_vault_mut(&state, |vault| {
        vault.set_workflow_active(&input.id, input.active, now_secs())
    })
}

#[tauri::command]
pub async fn delete_workflow(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_workflow(&id))
}

#[tauri::command]
pub async fn restore_workflow_version(
    input: RestoreWorkflowVersionInput,
    state: State<'_, AppState>,
) -> Result<SavedWorkflow, String> {
    with_vault_mut(&state, |vault| {
        vault.restore_workflow_version(&input.id, &input.saved_at, now_secs())
    })
}

/// Emulate on a copy of the vault: nothing is written.
#[tauri::command]
pub async fn dry_run_workflow(
    input: DryRunWorkflowInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RunReport, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        vault.dry_run_workflow(
            &input.graph,
            input.workflow_id.as_deref(),
            input.trigger_node.as_deref(),
            input.answers.unwrap_or_default(),
            now_secs(),
            &zone,
        )
    })
}

/// Run for real now; its notices are shown like any other.
#[tauri::command]
pub async fn run_workflow_now(
    input: RunWorkflowInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RunReport, String> {
    let zone = current_zone(&app);
    let report = with_vault_mut(&state, |vault| {
        vault.run_workflow_now(&input.id, input.trigger_node.as_deref(), now_secs(), &zone)
    })?;
    crate::scheduler::deliver(&app, report.notices.clone(), false);
    if !report.run.created.is_empty() {
        let _ = app.emit(
            "automation-created",
            serde_json::json!({ "tx_ids": report.run.created }),
        );
    }
    Ok(report)
}

#[tauri::command]
pub async fn list_workflow_runs(
    input: ListWorkflowRunsInput,
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowRun>, String> {
    let limit = input.limit.unwrap_or(DEFAULT_RUNS) as usize;
    with_vault(&state, |vault| {
        Ok(vault.workflow_runs(input.workflow_id.as_deref(), limit))
    })
}

#[tauri::command]
pub async fn list_workflow_queue(state: State<'_, AppState>) -> Result<Vec<QueuedActions>, String> {
    with_vault(&state, |vault| Ok(vault.workflow_queue()))
}

#[tauri::command]
pub async fn confirm_workflow_queued(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let saved = with_vault_mut(&state, |vault| vault.confirm_queued(&id))?;
    let ids: Vec<&str> = saved.iter().map(|tx| tx.meta().id.as_str()).collect();
    let _ = app.emit("automation-created", serde_json::json!({ "tx_ids": ids }));
    Ok(saved)
}

#[tauri::command]
pub async fn discard_workflow_queued(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.discard_queued(&id))
}

/// Runs asleep: timers and questions waiting for an answer.
#[tauri::command]
pub async fn list_workflow_waits(state: State<'_, AppState>) -> Result<Vec<WorkflowWait>, String> {
    with_vault(&state, |vault| Ok(vault.workflow_waits()))
}

/// Answer a «Ждать подтверждения» question; the run goes on at once.
#[tauri::command]
pub async fn answer_workflow_wait(
    input: AnswerWorkflowWaitInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RunReport, String> {
    let zone = current_zone(&app);
    let report = with_vault_mut(&state, |vault| {
        vault.answer_workflow_wait(&input.id, input.yes, now_secs(), &zone)
    })?;
    crate::scheduler::deliver(&app, report.notices.clone(), false);
    if !report.run.created.is_empty() {
        let _ = app.emit(
            "automation-created",
            serde_json::json!({ "tx_ids": report.run.created }),
        );
    }
    let _ = app.emit("workflows-ran", serde_json::json!({}));
    Ok(report)
}

#[tauri::command]
pub async fn list_workflow_vars(state: State<'_, AppState>) -> Result<Vec<WorkflowVar>, String> {
    with_vault(&state, |vault| Ok(vault.workflow_vars()))
}

#[tauri::command]
pub async fn set_workflow_var(
    input: SetWorkflowVarInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| {
        vault.set_workflow_var(input.scope, &input.name, &input.value)
    })
}

#[tauri::command]
pub async fn delete_workflow_var(
    input: DeleteWorkflowVarInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| {
        vault.delete_workflow_var(&input.scope, &input.name)
    })
}

/// The workflow as a file's text.
#[tauri::command]
pub async fn export_workflow(id: String, state: State<'_, AppState>) -> Result<String, String> {
    with_vault(&state, |vault| vault.export_workflow(&id))
}

/// Add a workflow from a file's text, switched off.
#[tauri::command]
pub async fn import_workflow(
    json: String,
    state: State<'_, AppState>,
) -> Result<SavedWorkflow, String> {
    with_vault_mut(&state, |vault| vault.import_workflow(&json, now_secs()))
}

#[tauri::command]
pub async fn save_workflow_test(
    input: SaveWorkflowTestInput,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    with_vault_mut(&state, |vault| {
        vault.save_workflow_test(&input.workflow_id, input.test)
    })
}

#[tauri::command]
pub async fn delete_workflow_test(
    input: DeleteWorkflowTestInput,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    with_vault_mut(&state, |vault| {
        vault.delete_workflow_test(&input.workflow_id, &input.test_id)
    })
}

/// Every test of a workflow by emulation; nothing is written.
#[tauri::command]
pub async fn run_workflow_tests(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowTestResult>, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        vault.run_workflow_tests(&id, now_secs(), &zone)
    })
}
