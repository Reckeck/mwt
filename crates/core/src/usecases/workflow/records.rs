//! Scenarios over workflow records: save, switch on and off, delete, restore a
//! version, import, the tests kept with a workflow, variables set by hand, the
//! confirmation queue, and workflows made from the gallery templates. Each
//! writes once, in one `transact`, through `tx.collection` for the record kinds.

use std::collections::{BTreeMap, HashMap};

use rust_decimal::Decimal;

use crate::Amount;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::kernel::tx::Tx;
use crate::model::notify::NoticeCategory;
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::money::tree::find_store;
use crate::planning::notifications as notices;
use crate::service::UnlockedVault;
use crate::usecases::planning::notifications::notice;
use crate::workflow::graph::Graph;
use crate::workflow::model::{
    MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, VarScope, Workflow, WorkflowRun, WorkflowTest,
    WorkflowWait,
};
use crate::workflow::service::marks;
use crate::workflow::service::records::{
    self, SavedWorkflow, WorkflowDraft, activation_graph, apply_draft, cap_currency, check_draft,
    import_draft, keep_draft_aside, new_workflow, restoring_draft,
};
use crate::workflow::service::runs::switched_off_run;
use crate::workflow::service::tests_def::{checked_test, drop_test, keep_test};
use crate::workflow::service::vars::{parse_input, remove_var, set_var};
use crate::workflow::templates::{
    TemplateDefaults, TemplateFieldKind, WorkflowTemplateInfo, build_template, template_list,
};
use crate::workflow::validate::validate;

fn missing_workflow(id: &str) -> Error {
    Error::Missing(format!("workflow {id}"))
}

/// Edit the workflow with this id in place and hand back what `edit` returns;
/// an unknown id is `Missing`.
fn edit_workflow<R>(
    tx: &mut Tx<'_>,
    id: &str,
    edit: impl FnOnce(&mut Workflow) -> Result<R>,
) -> Result<R> {
    let mut workflows = tx.collection::<Workflow>();
    if workflows.get(id).is_none() {
        return Err(missing_workflow(id));
    }
    workflows.update(id, edit)?
}

/// Create (`id` = `None`) or update a workflow. A changed graph keeps the one
/// before it among the versions; one switched on starts watching from now.
pub fn save_workflow(
    vault: &mut UnlockedVault,
    id: Option<&str>,
    draft: WorkflowDraft,
    now: i64,
) -> Result<SavedWorkflow> {
    let (graph, issues) = check_draft(&draft)?;
    let graph_json = graph.to_json();
    let stamp = crate::calendar::iso(now);
    let cap_currency = cap_currency(&draft, vault.base_currency());
    vault.transact(|tx| {
        let (workflow, arm) = match id {
            None => {
                let workflow = new_workflow(
                    uuid::Uuid::new_v4().to_string(),
                    draft,
                    graph_json,
                    cap_currency,
                    stamp,
                );
                tx.collection::<Workflow>().insert(workflow.clone())?;
                let arm = workflow.active;
                (workflow, arm)
            }
            Some(id) => edit_workflow(tx, id, |workflow| {
                let arm = apply_draft(workflow, draft, graph_json, cap_currency, stamp)?;
                Ok((workflow.clone(), arm))
            })?,
        };
        if arm {
            marks::arm(tx.doc_mut(), &workflow.id, &graph, now);
        }
        Ok(SavedWorkflow { workflow, issues })
    })
}

/// Keep the editor's unsaved graph aside: the saved one keeps running.
pub fn save_workflow_draft(
    vault: &mut UnlockedVault,
    id: &str,
    graph_json: &str,
    now: i64,
) -> Result<Workflow> {
    let graph = Graph::parse(graph_json)?;
    let stamp = crate::calendar::iso(now);
    vault.transact(|tx| {
        edit_workflow(tx, id, |workflow| {
            keep_draft_aside(workflow, &graph, stamp)?;
            Ok(workflow.clone())
        })
    })
}

/// Throw the kept-aside edits away.
pub fn discard_workflow_draft(vault: &mut UnlockedVault, id: &str) -> Result<Workflow> {
    vault.transact(|tx| {
        edit_workflow(tx, id, |workflow| {
            workflow.draft = None;
            Ok(workflow.clone())
        })
    })
}

pub fn set_workflow_active(
    vault: &mut UnlockedVault,
    id: &str,
    active: bool,
    now: i64,
) -> Result<Workflow> {
    let workflow = records::workflow(&vault.doc, id)?;
    let graph = activation_graph(&workflow, active)?;
    let arm = active && !workflow.active;
    vault.transact(|tx| {
        if arm {
            marks::arm(tx.doc_mut(), id, &graph, now);
        }
        edit_workflow(tx, id, |stored| {
            stored.active = active;
            Ok(stored.clone())
        })
    })
}

/// Remove a workflow with its runs, own variables, queue, state and waits.
pub fn delete_workflow(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    vault.transact(|tx| {
        if tx.collection::<Workflow>().get(id).is_none() {
            return Err(missing_workflow(id));
        }
        tx.collection::<Workflow>().remove(id)?;
        // A run goes as a deletion that reaches the other devices: the change
        // tracker reads a missing entry of a capped log as a trim.
        let runs: Vec<String> = tx
            .collection::<WorkflowRun>()
            .iter()
            .filter(|run| run.workflow_id == id)
            .map(|run| run.id.clone())
            .collect();
        for run_id in &runs {
            tx.collection::<WorkflowRun>().remove(run_id)?;
        }
        let batches: Vec<String> = tx
            .collection::<QueuedActions>()
            .iter()
            .filter(|batch| batch.workflow_id == id)
            .map(|batch| batch.id.clone())
            .collect();
        for batch_id in &batches {
            tx.collection::<QueuedActions>().remove(batch_id)?;
        }
        let waits: Vec<String> = tx
            .collection::<WorkflowWait>()
            .iter()
            .filter(|wait| wait.workflow_id == id)
            .map(|wait| wait.id.clone())
            .collect();
        for wait_id in &waits {
            tx.collection::<WorkflowWait>().remove(wait_id)?;
        }
        let doc = tx.doc_mut();
        doc.workflow_vars
            .retain(|var| var.scope != VarScope::Workflow(id.to_string()));
        doc.workflow_state.retain(|state| state.workflow_id != id);
        Ok(())
    })
}

/// Put an earlier graph back; the current one becomes a version itself.
pub fn restore_workflow_version(
    vault: &mut UnlockedVault,
    id: &str,
    saved_at: &str,
    now: i64,
) -> Result<SavedWorkflow> {
    let workflow = records::workflow(&vault.doc, id)?;
    let draft = restoring_draft(&workflow, saved_at)?;
    save_workflow(vault, Some(id), draft, now)
}

/// Add an exported workflow, switched off: it is checked before it runs.
/// `stores` swaps wallet ids of the file for wallets of this vault.
pub fn import_workflow(
    vault: &mut UnlockedVault,
    json: &str,
    stores: &HashMap<String, String>,
    now: i64,
) -> Result<SavedWorkflow> {
    let draft = import_draft(&vault.doc, json, stores)?;
    save_workflow(vault, None, draft, now)
}

/// Add a test (empty id) or replace the one with the same id.
pub fn save_workflow_test(
    vault: &mut UnlockedVault,
    workflow_id: &str,
    test: WorkflowTest,
) -> Result<Workflow> {
    let test = checked_test(test, || uuid::Uuid::new_v4().to_string())?;
    vault.transact(|tx| {
        edit_workflow(tx, workflow_id, |workflow| {
            keep_test(workflow, test);
            Ok(workflow.clone())
        })
    })
}

pub fn delete_workflow_test(
    vault: &mut UnlockedVault,
    workflow_id: &str,
    test_id: &str,
) -> Result<Workflow> {
    vault.transact(|tx| {
        edit_workflow(tx, workflow_id, |workflow| {
            drop_test(workflow, test_id);
            Ok(workflow.clone())
        })
    })
}

/// Set a variable by hand; `value` is a `WfValue` in JSON.
pub fn set_workflow_var(
    vault: &mut UnlockedVault,
    scope: VarScope,
    name: &str,
    value: &str,
) -> Result<()> {
    let (name, parsed) = parse_input(name, value)?;
    let updated_at = now_iso();
    vault.transact(|tx| {
        set_var(tx.doc_mut(), scope, &name, &parsed, updated_at);
        Ok(())
    })
}

pub fn delete_workflow_var(vault: &mut UnlockedVault, scope: &VarScope, name: &str) -> Result<()> {
    vault.transact(|tx| {
        remove_var(tx.doc_mut(), scope, name);
        Ok(())
    })
}

/// Make the operations a workflow queued, all together or none.
pub fn confirm_queued(vault: &mut UnlockedVault, id: &str) -> Result<Vec<Transaction>> {
    let batch = vault
        .doc
        .workflow_queue
        .iter()
        .find(|batch| batch.id == id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("queued {id}")))?;
    let saved = if batch.transactions.len() == 1 {
        let mut txs = batch.transactions;
        vec![vault.add_transaction(txs.remove(0))?]
    } else {
        vault.add_linked_transactions(batch.transactions, None)?
    };
    let created: Vec<String> = saved.iter().map(|tx| tx.meta().id.clone()).collect();
    vault.transact(|tx| {
        tx.collection::<QueuedActions>().remove(id)?;
        // The refusal this batch waited for is answered: its run is done, so a
        // retry cannot book the operations a second time.
        if let Some(run) = refused_run(tx, &batch.run_id) {
            let done = WorkflowRun {
                id: uuid::Uuid::new_v4().to_string(),
                outcome: RunOutcome::Done,
                created,
                queued: 0,
                ..run
            };
            tx.collection::<WorkflowRun>().remove(&batch.run_id)?;
            tx.collection::<WorkflowRun>().insert(done)?;
        }
        Ok(())
    })?;
    Ok(saved)
}

/// The run a queued batch belongs to, while it is still a refusal.
fn refused_run(tx: &mut Tx, run_id: &str) -> Option<WorkflowRun> {
    tx.collection::<WorkflowRun>()
        .get(run_id)
        .filter(|run| matches!(run.outcome, RunOutcome::Rejected { .. }))
        .cloned()
}

pub fn discard_queued(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    vault.transact(|tx| {
        let Some(batch) = tx.collection::<QueuedActions>().get(id).cloned() else {
            return Err(Error::Missing(format!("queued {id}")));
        };
        tx.collection::<QueuedActions>().remove(id)?;
        // Nothing is left to retry: the refused run is skipped, as by `skip_workflow_run`.
        if refused_run(tx, &batch.run_id).is_some() {
            tx.collection::<WorkflowRun>()
                .update(&batch.run_id, |run| {
                    run.outcome = RunOutcome::Dropped {
                        reason: "skipped".into(),
                    };
                })?;
        }
        Ok(())
    })
}

/// Active workflows a required field now stops (they used to run with it
/// empty and fail, or never fire): switched off on open, each with a line in
/// the journal and a notice, so `run_due_workflows` does not skip them in
/// silence. Returns whether anything changed. Writes nothing itself: the open
/// that asks goes on to write.
pub fn disable_incomplete_workflows(vault: &mut UnlockedVault, now: i64) -> bool {
    vault
        .run(|tx| {
            let mut notices_made = Vec::new();
            let incomplete: Vec<(String, String, String, String, String)> = tx
                .collection::<Workflow>()
                .iter()
                .filter(|workflow| workflow.active)
                .filter_map(|workflow| {
                    let graph = Graph::parse(&workflow.graph).ok()?;
                    let issue = validate(&graph)
                        .into_iter()
                        .find(|issue| issue.code == "param_missing")?;
                    Some((
                        workflow.id.clone(),
                        workflow.name.clone(),
                        issue.node_id.clone().unwrap_or_default(),
                        issue.text_ru,
                        issue.text_en,
                    ))
                })
                .collect();
            for (id, name, node_id, text_ru, text_en) in incomplete {
                tx.collection::<Workflow>().update(&id, |workflow| {
                    workflow.active = false;
                })?;
                let run = switched_off_run(&id, node_id, &text_ru, &text_en, now);
                let mut runs = tx.collection::<WorkflowRun>();
                if runs.get(&run.id).is_some() {
                    let run_id = run.id.clone();
                    runs.update(&run_id, |known| *known = run)?;
                } else {
                    runs.insert(run)?;
                }
                notices_made.push(notice(
                    NoticeCategory::Automation,
                    format!("wf-off:{id}"),
                    &name,
                    format!("Схема выключена: {text_ru}"),
                    Some(format!("{}:{id}", crate::routes::WORKFLOW_EDIT)),
                    crate::calendar::iso(now),
                ));
            }
            if notices_made.is_empty() {
                return Ok(false);
            }
            let doc = tx.doc_mut();
            let overflow = doc.workflow_runs.len().saturating_sub(MAX_WORKFLOW_RUNS);
            doc.workflow_runs.drain(..overflow);
            if notices::notices_enabled(doc, NoticeCategory::Automation) {
                doc.notification_log.extend(notices_made);
            }
            Ok(true)
        })
        .is_ok_and(|(changed, _, _)| changed)
}

// -- the gallery -------------------------------------------------------------------

/// The gallery: the savings share and the budget currency come from the vault.
pub fn workflow_templates(books: &Books<'_>) -> Vec<WorkflowTemplateInfo> {
    let pct = (books.doc().settings.savings_rate * Amount::new(Decimal::ONE_HUNDRED))
        .round(2)
        .into_inner()
        .normalize();
    template_list(&TemplateDefaults {
        savings_pct: pct.to_string(),
        currency: crate::planning::budget::budget_currency(books).to_lowercase(),
    })
}

/// Whether `value` is a good answer to a template field of this kind.
fn field_is_valid(vault: &UnlockedVault, kind: &TemplateFieldKind, value: &str) -> bool {
    match kind {
        TemplateFieldKind::Store => find_store(&vault.doc, value).is_some(),
        TemplateFieldKind::Number => Amount::parse(value).is_ok_and(|number| number > Amount::ZERO),
        TemplateFieldKind::Weekday => value.parse::<u8>().is_ok_and(|day| day <= 6),
        TemplateFieldKind::Day => value.parse::<u8>().is_ok_and(|day| (1..=31).contains(&day)),
        TemplateFieldKind::Category | TemplateFieldKind::Currency | TemplateFieldKind::Budget => {
            true
        }
    }
}

/// Builds the template's graph from the form and saves it as an active workflow.
pub fn workflow_from_template(
    vault: &mut UnlockedVault,
    id: &str,
    params: &BTreeMap<String, String>,
    locale: &str,
    now: i64,
) -> Result<SavedWorkflow> {
    let info = workflow_templates(&vault.books())
        .into_iter()
        .find(|info| info.id == id)
        .ok_or_else(|| Error::Missing(format!("template {id}")))?;
    for field in &info.fields {
        let value = params.get(&field.key).map_or("", |value| value.trim());
        if value.is_empty() {
            if field.required {
                return Err(Error::InvalidInput(format!("{}: required", field.key)));
            }
            continue;
        }
        if !field_is_valid(vault, &field.kind, value) {
            return Err(Error::InvalidInput(format!(
                "{}: invalid value {value}",
                field.key
            )));
        }
    }
    let built = build_template(id, params, locale)
        .ok_or_else(|| Error::Missing(format!("template {id}")))?;
    let draft = WorkflowDraft {
        name: built.name,
        description: built.description,
        active: true,
        graph: built.graph,
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    };
    save_workflow(vault, None, draft, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    fn queued(id: &str, transactions: Vec<Transaction>) -> QueuedActions {
        QueuedActions {
            id: id.into(),
            workflow_id: "wf".into(),
            run_id: "run".into(),
            created_at: String::new(),
            summary: String::new(),
            transactions,
        }
    }

    fn text_json(text: &str) -> String {
        serde_json::to_string(&crate::workflow::types::WfValue::Text(text.into())).unwrap()
    }

    #[test]
    fn a_variable_set_by_hand_is_kept_and_then_deleted() {
        let (_dir, mut vault) = empty_vault();
        vault
            .set_workflow_var(VarScope::Global, " note ", &text_json("hello"))
            .unwrap();
        assert_eq!(vault.workflow_vars().len(), 1);
        assert_eq!(vault.workflow_vars()[0].name, "note");
        assert!(matches!(
            vault.set_workflow_var(VarScope::Global, " ", &text_json("x")),
            Err(Error::InvalidInput(_))
        ));
        vault
            .delete_workflow_var(&VarScope::Global, "note")
            .unwrap();
        assert!(vault.workflow_vars().is_empty());
    }

    #[test]
    fn a_discarded_batch_goes_and_an_unknown_one_is_missing() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.workflow_queue.push(queued("q1", Vec::new()));
        vault.discard_queued("q1").unwrap();
        assert!(vault.workflow_queue().is_empty());
        assert!(matches!(vault.discard_queued("q1"), Err(Error::Missing(_))));
    }

    #[test]
    fn a_confirmed_batch_makes_its_operations_and_leaves_the_queue() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Cash, None);
        vault
            .doc
            .workflow_queue
            .push(queued("q1", vec![income(&card, "25", "queued")]));
        let saved = vault.confirm_queued("q1").unwrap();
        assert_eq!(saved.len(), 1);
        assert!(vault.workflow_queue().is_empty());
        assert_eq!(vault.doc.transactions.len(), 1);
        assert!(matches!(vault.confirm_queued("q1"), Err(Error::Missing(_))));
    }

    #[test]
    fn the_delegate_equals_the_scenario() {
        let (_dir, mut vault) = empty_vault();
        let graph = r#"{"version":1,"nodes":[],"edges":[]}"#;
        let draft = || WorkflowDraft {
            name: "w".into(),
            description: String::new(),
            active: false,
            graph: graph.into(),
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
        };
        let through_delegate = vault.save_workflow(None, draft(), 1_000).unwrap();
        let through_scenario = save_workflow(&mut vault, None, draft(), 1_000).unwrap();
        assert_eq!(
            through_delegate.workflow.name,
            through_scenario.workflow.name
        );
        assert_eq!(
            through_delegate.workflow.graph,
            through_scenario.workflow.graph
        );
        assert_ne!(through_delegate.workflow.id, through_scenario.workflow.id);
        assert_eq!(vault.workflows().len(), 2);
    }

    #[test]
    fn deleting_a_workflow_takes_its_queue_and_runs_with_it() {
        let (_dir, mut vault) = empty_vault();
        let graph = r#"{"version":1,"nodes":[],"edges":[]}"#.to_string();
        let saved = vault
            .save_workflow(
                None,
                WorkflowDraft {
                    name: "w".into(),
                    description: String::new(),
                    active: false,
                    graph,
                    react_to_workflows: false,
                    max_amount: None,
                    max_amount_currency: None,
                },
                1_000,
            )
            .unwrap();
        let mut batch = queued("q1", Vec::new());
        batch.workflow_id = saved.workflow.id.clone();
        vault.doc.workflow_queue.push(batch);
        vault.delete_workflow(&saved.workflow.id).unwrap();
        assert!(vault.workflows().is_empty());
        assert!(vault.workflow_queue().is_empty());
        assert!(matches!(
            vault.delete_workflow(&saved.workflow.id),
            Err(Error::Missing(_))
        ));
    }
}
