//! The graph a workflow is drawn as. Kept as JSON in the vault, so its shape
//! can grow without a vault format change; `version` rejects a graph from a newer app.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::catalog::node_def;
use super::types::PortType;

/// Current graph format. Older files are converted once by the frozen
/// `storage::legacy_v4` transition, not here.
pub const GRAPH_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "WorkflowPosition")]
pub struct Position {
    #[serde(rename = "x")]
    pub left: f64,
    #[serde(rename = "y")]
    pub top: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "WorkflowNode")]
pub struct Node {
    pub id: String,
    /// Catalog kind, e.g. `trigger.schedule`.
    pub kind: String,
    /// Parameter values by name; a connected input of the same name wins.
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub params: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub position: Position,
    /// Switched off: the flow passes through it untouched.
    #[serde(default)]
    pub disabled: bool,
    /// A label the user gave it.
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "WorkflowEdge")]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub from_port: String,
    pub to: String,
    pub to_port: String,
}

/// A note or a frame on the canvas: no effect on a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "WorkflowComment")]
pub struct Comment {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "WorkflowGraph")]
pub struct Graph {
    #[serde(default = "graph_version")]
    pub version: u32,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

fn graph_version() -> u32 {
    GRAPH_VERSION
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            version: GRAPH_VERSION,
            nodes: Vec::new(),
            edges: Vec::new(),
            comments: Vec::new(),
        }
    }
}

impl Graph {
    pub fn parse(json: &str) -> crate::Result<Self> {
        if json.trim().is_empty() {
            return Ok(Self::default());
        }
        let graph: Graph = serde_json::from_str(json)
            .map_err(|err| crate::Error::InvalidInput(format!("workflow graph: {err}")))?;
        if graph.version > GRAPH_VERSION {
            return Err(crate::Error::InvalidInput(
                "the workflow was made by a newer app".into(),
            ));
        }
        Ok(graph)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Edges leaving `node` from `port`, in the order they were drawn.
    pub fn edges_from<'graph>(
        &'graph self,
        node: &'graph str,
        port: &'graph str,
    ) -> impl Iterator<Item = &'graph Edge> + 'graph {
        self.edges
            .iter()
            .filter(move |edge| edge.from == node && edge.from_port == port)
    }

    /// The edge feeding `node`'s input `port`, if any.
    pub fn edge_into(&self, node: &str, port: &str) -> Option<&Edge> {
        self.edges
            .iter()
            .find(|edge| edge.to == node && edge.to_port == port)
    }

    /// Data order: each node with a flow input but no flow wire into it,
    /// and the actions it takes data from (`action_sources`). It runs once
    /// they all have; one that takes data from no action is left out.
    pub fn data_order(&self) -> HashMap<String, Vec<String>> {
        let mut out = HashMap::new();
        for node in self.nodes.iter().filter(|node| !node.disabled) {
            let Some(def) = node_def(&node.kind) else {
                continue;
            };
            let flow_input = def.inputs.iter().any(|port| port.ty == PortType::Flow);
            if def.is_trigger() || !flow_input || self.has_flow_in(node) {
                continue;
            }
            let sources = self.action_sources(&node.id);
            if !sources.is_empty() {
                out.insert(node.id.clone(), sources);
            }
        }
        out
    }

    /// Whether a wire comes into one of `node`'s flow inputs.
    pub fn has_flow_in(&self, node: &Node) -> bool {
        let Some(def) = node_def(&node.kind) else {
            return false;
        };
        self.edges.iter().any(|edge| {
            edge.to == node.id
                && def
                    .input(&edge.to_port)
                    .is_some_and(|port| port.ty == PortType::Flow)
        })
    }

    /// The actions (flow nodes other than triggers) whose outputs reach the
    /// data inputs of `node_id`, directly or through pure data nodes.
    pub fn action_sources(&self, node_id: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        self.collect_sources(node_id, &mut seen, &mut found);
        found
    }

    fn collect_sources(&self, node_id: &str, seen: &mut HashSet<String>, found: &mut Vec<String>) {
        let Some(def) = self.node(node_id).and_then(|node| node_def(&node.kind)) else {
            return;
        };
        for edge in self.edges.iter().filter(|edge| edge.to == node_id) {
            if def
                .input(&edge.to_port)
                .is_none_or(|port| port.ty == PortType::Flow)
            {
                continue;
            }
            if !seen.insert(edge.from.clone()) {
                continue;
            }
            let Some(from_def) = self.node(&edge.from).and_then(|from| node_def(&from.kind)) else {
                continue;
            };
            if !from_def.has_flow() {
                self.collect_sources(&edge.from, seen, found);
            } else if !from_def.is_trigger() {
                found.push(edge.from.clone());
            }
        }
    }
}
