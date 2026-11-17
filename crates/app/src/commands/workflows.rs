//! Node workflows (plan §16): the node catalog, saving and checking graphs,
//! running and emulating them, the journal, the confirmation queue and the
//! variables. Everything lives in core; days are cut in the zone from
//! [`current_zone`].

use tauri::{AppHandle, Emitter, State};

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
    Ok(vault.workflows())
}

#[ipc(read)]
pub fn get_workflow(vault: &UnlockedVault, id: String) -> trade_core::Result<Workflow> {
    vault.workflow(&id)
}

#[ipc(write)]
pub fn save_workflow(
    vault: &mut UnlockedVault,
    input: SaveWorkflowInput,
) -> trade_core::Result<SavedWorkflow> {
    vault.save_workflow(
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
    vault.save_workflow_draft(&input.id, &input.graph, now_secs())
}

#[ipc(write)]
pub fn discard_workflow_draft(
    vault: &mut UnlockedVault,
    id: String,
) -> trade_core::Result<Workflow> {
    vault.discard_workflow_draft(&id)
}

#[ipc(write)]
pub fn set_workflow_active(
    vault: &mut UnlockedVault,
    input: SetWorkflowActiveInput,
) -> trade_core::Result<Workflow> {
    vault.set_workflow_active(&input.id, input.active, now_secs())
}

#[ipc(write)]
pub fn delete_workflow(vault: &mut UnlockedVault, id: String) -> trade_core::Result<()> {
    vault.delete_workflow(&id)
}

#[ipc(write)]
pub fn restore_workflow_version(
    vault: &mut UnlockedVault,
    input: RestoreWorkflowVersionInput,
) -> trade_core::Result<SavedWorkflow> {
    vault.restore_workflow_version(&input.id, &input.saved_at, now_secs())
}

/// Emulate on a copy of the vault: nothing is written.
#[ipc(read)]
pub fn dry_run_workflow(
    vault: &UnlockedVault,
    app: &AppHandle,
    input: DryRunWorkflowInput,
) -> trade_core::Result<RunReport> {
    vault.dry_run_workflow(
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

#[ipc(read)]
pub fn list_workflow_runs(
    vault: &UnlockedVault,
    input: ListWorkflowRunsInput,
) -> trade_core::Result<Vec<WorkflowRun>> {
    let limit = input.limit.unwrap_or(DEFAULT_RUNS) as usize;
    Ok(vault.workflow_runs(input.workflow_id.as_deref(), limit))
}

#[ipc(read)]
pub fn list_workflow_queue(vault: &UnlockedVault) -> trade_core::Result<Vec<QueuedActions>> {
    Ok(vault.workflow_queue())
}

#[tauri::command]
pub async fn confirm_workflow_queued(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, IpcError> {
    let saved = with_vault_mut(&state, |vault| vault.confirm_queued(&id))?;
    let ids: Vec<&str> = saved.iter().map(|tx| tx.meta().id.as_str()).collect();
    let _ = app.emit("automation-created", serde_json::json!({ "tx_ids": ids }));
    Ok(saved)
}

#[ipc(write)]
pub fn discard_workflow_queued(vault: &mut UnlockedVault, id: String) -> trade_core::Result<()> {
    vault.discard_queued(&id)
}

/// Runs asleep: timers and questions waiting for an answer.
#[ipc(read)]
pub fn list_workflow_waits(vault: &UnlockedVault) -> trade_core::Result<Vec<WorkflowWait>> {
    Ok(vault.workflow_waits())
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

#[ipc(read)]
pub fn list_workflow_vars(vault: &UnlockedVault) -> trade_core::Result<Vec<WorkflowVar>> {
    Ok(vault.workflow_vars())
}

#[ipc(write)]
pub fn set_workflow_var(
    vault: &mut UnlockedVault,
    input: SetWorkflowVarInput,
) -> trade_core::Result<()> {
    vault.set_workflow_var(input.scope, &input.name, &input.value)
}

#[ipc(write)]
pub fn delete_workflow_var(
    vault: &mut UnlockedVault,
    input: DeleteWorkflowVarInput,
) -> trade_core::Result<()> {
    vault.delete_workflow_var(&input.scope, &input.name)
}

/// The workflow as a file's text.
#[ipc(read)]
pub fn export_workflow(vault: &UnlockedVault, id: String) -> trade_core::Result<String> {
    vault.export_workflow(&id)
}

/// Add a workflow from a file's text, switched off.
#[ipc(write)]
pub fn import_workflow(
    vault: &mut UnlockedVault,
    input: ImportWorkflowInput,
) -> trade_core::Result<SavedWorkflow> {
    vault.import_workflow(&input.json, &input.stores, now_secs())
}

/// What a workflow file holds and which of its wallets are missing here.
#[ipc(read)]
pub fn preview_workflow_import(
    vault: &UnlockedVault,
    json: String,
) -> trade_core::Result<WorkflowImportPreview> {
    vault.preview_workflow_import(&json)
}

/// The gallery of ready-made workflows with defaults from the vault.
#[ipc(read)]
pub fn workflow_templates(vault: &UnlockedVault) -> trade_core::Result<Vec<WorkflowTemplateInfo>> {
    Ok(vault.workflow_templates())
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
    vault.workflow_from_template(&input.template_id, &input.params, locale, now_secs())
}

#[ipc(write)]
pub fn save_workflow_test(
    vault: &mut UnlockedVault,
    input: SaveWorkflowTestInput,
) -> trade_core::Result<Workflow> {
    vault.save_workflow_test(&input.workflow_id, input.test)
}

#[ipc(write)]
pub fn delete_workflow_test(
    vault: &mut UnlockedVault,
    input: DeleteWorkflowTestInput,
) -> trade_core::Result<Workflow> {
    vault.delete_workflow_test(&input.workflow_id, &input.test_id)
}

/// Every test of a workflow by emulation; nothing is written.
#[ipc(read)]
pub fn run_workflow_tests(
    vault: &UnlockedVault,
    app: &AppHandle,
    id: String,
) -> trade_core::Result<Vec<WorkflowTestResult>> {
    vault.run_workflow_tests(&id, now_secs(), &current_zone(app))
}

ipc_commands!(workflows_commands, WORKFLOWS_NAMES, commands::workflows: [workflow_node_catalog, validate_workflow, list_workflows, get_workflow, save_workflow, save_workflow_draft, discard_workflow_draft, set_workflow_active, delete_workflow, restore_workflow_version, dry_run_workflow, run_workflow_now, list_workflow_runs, list_workflow_queue, confirm_workflow_queued, discard_workflow_queued, list_workflow_waits, answer_workflow_wait, list_workflow_vars, set_workflow_var, delete_workflow_var, export_workflow, import_workflow, preview_workflow_import, workflow_templates, workflow_from_template, save_workflow_test, delete_workflow_test, run_workflow_tests]);

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::ipc::run::{self, Gate, VaultHost};

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
        let gate = Gate {
            sandbox_deny: false,
        };
        let error = run::read(locked_host(), &__IPC_META_list_workflows, gate, |vault| {
            Ok(vault.workflows())
        })
        .await
        .unwrap_err();
        assert_eq!(error.code(), "REQUIRES_UNLOCK");
    }

    /// The pilot writes carry no sandbox gate (automations run on the demo
    /// data), so this pins the gate `#[ipc(write, sandbox = deny)]` sets, on a
    /// pilot command's metadata: the refusal comes before any vault is looked at.
    #[tokio::test]
    async fn a_pilot_write_command_refuses_in_the_sandbox() {
        let host = locked_host();
        host.0.sandbox.store(true, Ordering::SeqCst);
        let gate = Gate { sandbox_deny: true };
        let error = run::write(host, &__IPC_META_save_workflow, gate, |_| Ok(()))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "SANDBOX");
        assert!(error.to_string().contains("save_workflow"));
    }
}
