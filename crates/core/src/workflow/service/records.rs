//! Workflow records: what a user edits (the draft), how a draft is checked
//! and turned into a record, versions and the kept-aside edits, and the file a
//! workflow travels as. Pure functions over the document and the records; the
//! scenarios in `usecases::workflow::records` write.

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::store::Store;
use crate::model::vault::VaultDoc;
use crate::workflow::graph::Graph;
use crate::workflow::model::{GraphVersion, MAX_GRAPH_VERSIONS, Workflow};
use crate::workflow::validate::{GraphIssue, has_errors, validate};

/// The format name of an exported workflow.
pub const EXPORT_FORMAT: &str = "trade-app-workflow";
/// The file version written now. Files of version 1 come from builds before the format reset: their
/// graph goes through the frozen legacy steps on import. When `storage::legacy_v4` is removed,
/// files of version 1 are refused.
const EXPORT_VERSION: u32 = 2;

/// What the user edits of a workflow.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowDraft {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub active: bool,
    pub graph: String,
    #[serde(default)]
    pub react_to_workflows: bool,
    #[serde(default)]
    pub max_amount: Option<Amount>,
    /// Currency of the cap; empty = the base of the device that saves it.
    #[serde(default)]
    pub max_amount_currency: Option<String>,
}

/// A saved workflow with what is wrong with its graph.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SavedWorkflow {
    pub workflow: Workflow,
    pub issues: Vec<GraphIssue>,
}

/// An exported workflow: the graph and its name, without ids or history.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ExportedWorkflow {
    format: String,
    version: u32,
    name: String,
    #[serde(default)]
    description: String,
    graph: serde_json::Value,
    /// The wallets the graph mentions, by the id it holds them under.
    #[serde(default)]
    stores: Vec<ExportedStore>,
}

/// A wallet an exported workflow mentions: its id there and its name.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ExportedStore {
    pub id: String,
    pub name: String,
}

/// What a workflow file holds, before it is added.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowImportPreview {
    pub name: String,
    /// Nodes that are not triggers.
    pub steps: u32,
    /// Wallets of the file that this vault does not have.
    pub missing: Vec<ExportedStore>,
}

pub fn workflows(doc: &VaultDoc) -> Vec<Workflow> {
    doc.workflows.clone()
}

pub fn workflow(doc: &VaultDoc, id: &str) -> Result<Workflow> {
    doc.workflows
        .iter()
        .find(|workflow| workflow.id == id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("workflow {id}")))
}

/// An automation's graph is built from its form: the editor only shows it.
pub fn refuse_automation(workflow: &Workflow) -> Result<()> {
    if workflow.automation.is_some() {
        return Err(Error::InvalidInput(
            "this workflow is an automation: change it in the automation form, or make it an ordinary workflow first".into(),
        ));
    }
    Ok(())
}

/// A draft that may be saved: named, with a positive cap, a graph that parses
/// and, when it is to be switched on, one without errors.
pub fn check_draft(draft: &WorkflowDraft) -> Result<(Graph, Vec<GraphIssue>)> {
    if draft.name.trim().is_empty() {
        return Err(Error::InvalidInput("name the workflow".into()));
    }
    if draft.max_amount.is_some_and(|cap| !cap.is_positive()) {
        return Err(Error::InvalidInput("the cap must be above zero".into()));
    }
    let graph = Graph::parse(&draft.graph)?;
    let issues = validate(&graph);
    if draft.active && has_errors(&issues) {
        let first = issues
            .first()
            .map(|issue| issue.text_en.clone())
            .unwrap_or_default();
        return Err(Error::InvalidInput(format!(
            "fix the workflow's errors before switching it on: {first}"
        )));
    }
    Ok((graph, issues))
}

/// The currency a saved cap is in: the one chosen, else `base`, the base of
/// the device that saves it. No cap, no currency.
pub fn cap_currency(draft: &WorkflowDraft, base: &str) -> Option<String> {
    draft.max_amount?;
    let chosen = draft
        .max_amount_currency
        .as_deref()
        .map(|code| code.trim().to_lowercase())
        .filter(|code| !code.is_empty());
    Some(chosen.unwrap_or_else(|| base.trim().to_lowercase()))
}

/// The record of a new workflow made from a checked draft.
pub fn new_workflow(
    id: String,
    draft: WorkflowDraft,
    graph_json: String,
    cap_currency: Option<String>,
    stamp: String,
) -> Workflow {
    Workflow {
        id,
        name: draft.name.trim().to_string(),
        description: draft.description,
        active: draft.active,
        graph: graph_json,
        created_at: stamp.clone(),
        updated_at: stamp,
        react_to_workflows: draft.react_to_workflows,
        max_amount: draft.max_amount,
        max_amount_currency: cap_currency,
        versions: Vec::new(),
        tests: Vec::new(),
        draft: None,
        automation: None,
    }
}

/// Put a checked draft into an existing workflow: a changed graph keeps the one
/// before it among the versions, and the edits kept aside are in the save now.
/// Returns whether the workflow is switched on by this save (and so is armed).
pub fn apply_draft(
    workflow: &mut Workflow,
    draft: WorkflowDraft,
    graph_json: String,
    cap_currency: Option<String>,
    stamp: String,
) -> Result<bool> {
    refuse_automation(workflow)?;
    let arm = draft.active && !workflow.active;
    if workflow.graph != graph_json && !workflow.graph.is_empty() {
        workflow.versions.push(GraphVersion {
            saved_at: workflow.updated_at.clone(),
            graph: std::mem::take(&mut workflow.graph),
        });
        let overflow = workflow.versions.len().saturating_sub(MAX_GRAPH_VERSIONS);
        workflow.versions.drain(..overflow);
    }
    workflow.name = draft.name.trim().to_string();
    workflow.description = draft.description;
    workflow.draft = None;
    workflow.active = draft.active;
    workflow.graph = graph_json;
    workflow.updated_at = stamp;
    workflow.react_to_workflows = draft.react_to_workflows;
    workflow.max_amount = draft.max_amount;
    workflow.max_amount_currency = cap_currency;
    Ok(arm)
}

/// Keep the editor's unsaved graph aside: the saved one keeps running.
pub fn keep_draft_aside(workflow: &mut Workflow, graph: &Graph, stamp: String) -> Result<()> {
    refuse_automation(workflow)?;
    workflow.draft = Some(GraphVersion {
        saved_at: stamp,
        graph: graph.to_json(),
    });
    Ok(())
}

/// Whether `workflow` may be switched on, and its graph: a graph with errors
/// is refused.
pub fn activation_graph(workflow: &Workflow, active: bool) -> Result<Graph> {
    let graph = Graph::parse(&workflow.graph)?;
    if active && has_errors(&validate(&graph)) {
        return Err(Error::InvalidInput(
            "fix the workflow's errors before switching it on".into(),
        ));
    }
    Ok(graph)
}

/// The draft that puts the version saved at `saved_at` back; a version with
/// errors comes back switched off.
pub fn restoring_draft(workflow: &Workflow, saved_at: &str) -> Result<WorkflowDraft> {
    let version = workflow
        .versions
        .iter()
        .find(|version| version.saved_at == saved_at)
        .ok_or_else(|| Error::Missing(format!("version {saved_at}")))?;
    let graph = Graph::parse(&version.graph)?;
    let active = workflow.active && !has_errors(&validate(&graph));
    Ok(WorkflowDraft {
        name: workflow.name.clone(),
        description: workflow.description.clone(),
        active,
        graph: version.graph.clone(),
        react_to_workflows: workflow.react_to_workflows,
        max_amount: workflow.max_amount,
        max_amount_currency: workflow.max_amount_currency.clone(),
    })
}

// -- the file a workflow travels as ----------------------------------------------

/// Every string in the value, nested ones included, in document order.
fn collect_strings<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::String(text) => out.push(text),
        serde_json::Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        serde_json::Value::Object(map) => map.values().for_each(|item| collect_strings(item, out)),
        _ => {}
    }
}

/// Swap every string equal to a key of `map` for its value, at any depth.
fn replace_strings(value: &mut serde_json::Value, map: &HashMap<String, String>) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(new) = map.get(text.as_str()) {
                *text = new.clone();
            }
        }
        serde_json::Value::Array(items) => {
            items.iter_mut().for_each(|item| replace_strings(item, map))
        }
        serde_json::Value::Object(fields) => fields
            .values_mut()
            .for_each(|item| replace_strings(item, map)),
        _ => {}
    }
}

fn parse_export(json: &str) -> Result<ExportedWorkflow> {
    let exported: ExportedWorkflow = serde_json::from_str(json)
        .map_err(|err| Error::InvalidInput(format!("not a workflow file: {err}")))?;
    if exported.format != EXPORT_FORMAT {
        return Err(Error::InvalidInput("not a workflow file".into()));
    }
    Ok(exported)
}

/// The store with this id (the workflow area may not use `money::tree`).
fn store_ref<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc Store> {
    doc.stores.iter().find(|store| store.id == id)
}

pub fn export_workflow(doc: &VaultDoc, id: &str) -> Result<String> {
    let workflow = workflow(doc, id)?;
    let graph: serde_json::Value = serde_json::from_str(&workflow.graph).unwrap_or_default();
    let mut strings = Vec::new();
    collect_strings(&graph, &mut strings);
    let mut seen = HashSet::new();
    let stores = strings
        .into_iter()
        .filter(|text| seen.insert(*text))
        .filter_map(|text| store_ref(doc, text))
        .map(|store| ExportedStore {
            id: store.id.clone(),
            name: store.name.clone(),
        })
        .collect();
    serde_json::to_string_pretty(&ExportedWorkflow {
        format: EXPORT_FORMAT.into(),
        version: EXPORT_VERSION,
        name: workflow.name,
        description: workflow.description,
        graph,
        stores,
    })
    .map_err(|err| Error::InvalidInput(err.to_string()))
}

/// What a workflow file holds: its name, steps and the wallets this vault lacks.
pub fn preview_import(doc: &VaultDoc, json: &str) -> Result<WorkflowImportPreview> {
    let exported = parse_export(json)?;
    let steps = exported
        .graph
        .get("nodes")
        .and_then(|nodes| nodes.as_array())
        .map_or(0, |nodes| {
            nodes
                .iter()
                .filter(|node| {
                    node.get("kind")
                        .and_then(|kind| kind.as_str())
                        .is_none_or(|kind| !kind.starts_with("trigger."))
                })
                .count()
        }) as u32;
    let missing = exported
        .stores
        .into_iter()
        .filter(|store| store_ref(doc, &store.id).is_none())
        .collect();
    Ok(WorkflowImportPreview {
        name: exported.name,
        steps,
        missing,
    })
}

/// The draft an exported workflow becomes, switched off: it is checked before
/// it runs. `stores` swaps wallet ids of the file for wallets of this vault.
pub fn import_draft(
    doc: &VaultDoc,
    json: &str,
    stores: &HashMap<String, String>,
) -> Result<WorkflowDraft> {
    let mut exported = parse_export(json)?;
    if let Some(unknown) = stores.values().find(|id| store_ref(doc, id).is_none()) {
        return Err(Error::InvalidInput(format!("unknown wallet {unknown}")));
    }
    replace_strings(&mut exported.graph, stores);
    let graph = if exported.version < EXPORT_VERSION {
        let text = exported.graph.to_string();
        crate::storage::legacy_v4::graph::migrate_graph_json(&text).unwrap_or(text)
    } else {
        exported.graph.to_string()
    };
    Ok(WorkflowDraft {
        name: exported.name,
        description: exported.description,
        active: false,
        graph,
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str, graph: &str) -> WorkflowDraft {
        WorkflowDraft {
            name: name.into(),
            description: String::new(),
            active: false,
            graph: graph.into(),
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
        }
    }

    const EMPTY_GRAPH: &str = r#"{"version":1,"nodes":[],"edges":[]}"#;

    #[test]
    fn a_draft_needs_a_name_and_a_positive_cap() {
        assert!(matches!(
            check_draft(&draft("  ", EMPTY_GRAPH)),
            Err(Error::InvalidInput(_))
        ));
        let mut capped = draft("w", EMPTY_GRAPH);
        capped.max_amount = Some(Amount::ZERO);
        assert!(matches!(check_draft(&capped), Err(Error::InvalidInput(_))));
        assert!(check_draft(&draft("w", EMPTY_GRAPH)).is_ok());
    }

    #[test]
    fn a_cap_is_counted_in_the_chosen_currency_else_the_base() {
        let mut capped = draft("w", EMPTY_GRAPH);
        assert_eq!(cap_currency(&capped, "USD"), None);
        capped.max_amount = Some(Amount::new(rust_decimal::Decimal::ONE));
        assert_eq!(cap_currency(&capped, " USD ").as_deref(), Some("usd"));
        capped.max_amount_currency = Some(" EUR ".into());
        assert_eq!(cap_currency(&capped, "usd").as_deref(), Some("eur"));
    }

    #[test]
    fn a_changed_graph_keeps_the_one_before_it_as_a_version() {
        let mut stored = new_workflow(
            "id".into(),
            draft("w", EMPTY_GRAPH),
            EMPTY_GRAPH.into(),
            None,
            "t0".into(),
        );
        let other = r#"{"version":1,"nodes":[],"edges":[],"x":1}"#;
        let armed = apply_draft(
            &mut stored,
            draft("w", other),
            other.into(),
            None,
            "t1".into(),
        )
        .unwrap();
        assert!(!armed);
        assert_eq!(stored.versions.len(), 1);
        assert_eq!(stored.versions[0].saved_at, "t0");
        assert_eq!(stored.graph, other);
        apply_draft(
            &mut stored,
            draft("w", other),
            other.into(),
            None,
            "t2".into(),
        )
        .unwrap();
        assert_eq!(
            stored.versions.len(),
            1,
            "an unchanged graph adds no version"
        );
    }

    #[test]
    fn a_workflow_file_of_another_format_is_refused() {
        let doc = VaultDoc::default();
        assert!(matches!(
            preview_import(&doc, r#"{"format":"x","version":2,"name":"n","graph":{}}"#),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            preview_import(&doc, "not json"),
            Err(Error::InvalidInput(_))
        ));
    }
}
