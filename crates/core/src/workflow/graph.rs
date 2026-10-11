//! The graph a workflow is drawn as. Kept as JSON in the vault, so its shape
//! can grow without a vault format change; `version` lets a later app migrate
//! an older graph when it loads it.

use serde::{Deserialize, Serialize};

/// Current graph format.
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
}
