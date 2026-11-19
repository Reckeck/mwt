//! The graph steps of versions 1 to 3, frozen: a copy of what `Graph::migrate` did.
//!
//! A v4 vault may hold graphs written by any build of the workflow engine. The steps bring an older
//! graph to the content of version 3 (the last one before the format reset); the result is stamped
//! with the version the live code expects ([`GRAPH_VERSION`]), so the live code needs no migration.

use crate::workflow::exec::FREE_RUNNING;
use crate::workflow::graph::{GRAPH_VERSION, Graph};

/// The last graph version the steps below know. A graph above it is left alone.
const LAST_LEGACY_VERSION: u32 = 3;

/// The graph JSON brought to the live version, or `None` when it needs nothing (empty, not a
/// graph, newer than the steps know, or already what the live code writes).
///
/// The stored number is the old counter's, so it is never compared with the live one except when
/// the two counters still agree on version 3.
pub(crate) fn migrate_graph_json(json: &str) -> Option<String> {
    if json.trim().is_empty() {
        return None;
    }
    let mut graph: Graph = serde_json::from_str(json).ok()?;
    let current = graph.version == LAST_LEGACY_VERSION && GRAPH_VERSION == LAST_LEGACY_VERSION;
    if graph.version > LAST_LEGACY_VERSION || current {
        return None;
    }
    if graph.version < 2 {
        wire_confirmations_to_yes(&mut graph);
    }
    if graph.version < 3 {
        switch_off_nodes_the_data_order_wakes(&mut graph);
    }
    graph.version = GRAPH_VERSION;
    Some(graph.to_json())
}

/// Version 2: "Wait for confirmation" answers "Yes"/"No"; a wire from its old "Next" leaves "Yes".
fn wire_confirmations_to_yes(graph: &mut Graph) {
    let confirms: Vec<String> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == "action.confirm")
        .map(|node| node.id.clone())
        .collect();
    for edge in &mut graph.edges {
        if edge.from_port == "out" && confirms.contains(&edge.from) {
            edge.from_port = "true".into();
        }
    }
}

/// Version 3: data sets the order. A node that this brings to life in an older graph (it never ran
/// there) is switched off, so nothing changes.
fn switch_off_nodes_the_data_order_wakes(graph: &mut Graph) {
    let woken: Vec<String> = graph
        .data_order()
        .into_keys()
        .filter(|id| {
            graph
                .node(id)
                .is_some_and(|node| !FREE_RUNNING.contains(&node.kind.as_str()))
        })
        .collect();
    for node in &mut graph.nodes {
        if woken.contains(&node.id) {
            node.disabled = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_json(version: u32, edge_port: &str) -> String {
        serde_json::json!({
            "version": version,
            "nodes": [
                {"id": "ask", "kind": "action.confirm"},
                {"id": "note", "kind": "action.notify"}
            ],
            "edges": [{
                "id": "wire", "from": "ask", "from_port": edge_port,
                "to": "note", "to_port": "in"
            }]
        })
        .to_string()
    }

    fn parse(json: &str) -> Graph {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_version_1_wire_from_next_leaves_yes() {
        let migrated = migrate_graph_json(&graph_json(1, "out")).unwrap();
        let graph = parse(&migrated);
        assert_eq!(graph.version, GRAPH_VERSION);
        assert_eq!(graph.edges[0].from_port, "true");
    }

    #[test]
    fn a_version_2_graph_keeps_its_wires_and_is_restamped() {
        let migrated = migrate_graph_json(&graph_json(2, "out")).unwrap();
        let graph = parse(&migrated);
        assert_eq!(graph.version, GRAPH_VERSION);
        assert_eq!(graph.edges[0].from_port, "out");
    }

    #[test]
    fn the_version_3_step_switches_off_what_the_data_order_would_wake() {
        // `book` takes data from `first` but has no flow wire into it: in a version 2 graph it
        // never ran, so the step switches it off. `first` is on the flow and stays on.
        let json = serde_json::json!({
            "version": 2,
            "nodes": [
                {"id": "start", "kind": "trigger.manual"},
                {"id": "first", "kind": "action.transaction"},
                {"id": "book", "kind": "action.transaction"}
            ],
            "edges": [
                {"id": "flow", "from": "start", "from_port": "out", "to": "first", "to_port": "in"},
                {"id": "data", "from": "first", "from_port": "transaction", "to": "book", "to_port": "amount"}
            ]
        })
        .to_string();
        let graph = parse(&migrate_graph_json(&json).unwrap());
        assert_eq!(graph.version, GRAPH_VERSION);
        let disabled: Vec<&str> = graph
            .nodes
            .iter()
            .filter(|node| node.disabled)
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(disabled, vec!["book"]);
    }

    #[test]
    fn nothing_to_do_returns_none() {
        assert_eq!(migrate_graph_json(""), None);
        assert_eq!(migrate_graph_json("not json"), None);
        assert_eq!(
            migrate_graph_json(&graph_json(LAST_LEGACY_VERSION + 1, "out")),
            None
        );
    }
}
