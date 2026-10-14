//! Checks a graph before it is saved or run: every node known, every wire
//! between ports that exist and fit, nothing looping, every required input
//! given. Errors keep a workflow from being switched on; warnings only tell.

use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::catalog::{NodeDef, node_def};
use super::graph::Graph;
use super::types::PortType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueLevel {
    Error,
    Warning,
}

/// One problem of a graph, pinned to the node or the wire it is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct GraphIssue {
    pub level: IssueLevel,
    /// Machine code: `unknown_node`, `no_trigger`, `type_mismatch`, …
    pub code: String,
    pub node_id: Option<String>,
    pub edge_id: Option<String>,
    pub text_ru: String,
    pub text_en: String,
}

impl GraphIssue {
    fn error(code: &str, ru: String, en: String) -> Self {
        Self {
            level: IssueLevel::Error,
            code: code.to_string(),
            node_id: None,
            edge_id: None,
            text_ru: ru,
            text_en: en,
        }
    }

    fn warning(code: &str, ru: String, en: String) -> Self {
        Self {
            level: IssueLevel::Warning,
            ..Self::error(code, ru, en)
        }
    }

    fn on_node(mut self, id: &str) -> Self {
        self.node_id = Some(id.to_string());
        self
    }

    fn on_edge(mut self, id: &str) -> Self {
        self.edge_id = Some(id.to_string());
        self
    }
}

/// Inputs a node works without: a text template may leave a slot empty, an
/// event may carry nothing, a comparison or a sum may take B from its field.
fn optional_input(kind: &str, port: &str) -> bool {
    matches!(
        (kind, port),
        ("logic.text", "a" | "b" | "c")
            | ("logic.compare" | "logic.math", "b")
            | ("action.emit" | "action.call", "payload")
            | ("action.return", "value")
            | ("var.set", "value")
            | ("var.add", "delta")
            // Dates default to now.
            | ("date.parts" | "date.add" | "date.boundary" | "date.shift_workday" | "date.format", "date")
            | ("date.make", "base")
            | ("date.add", "duration")
            | ("date.diff", "a" | "b")
            | ("list.make", "a" | "b" | "c" | "d")
            | ("logic.select", "a" | "b")
    )
}

/// Every problem of the graph, errors first.
pub fn validate(graph: &Graph) -> Vec<GraphIssue> {
    let mut out = Vec::new();
    let order = graph.data_order();
    let mut defs: HashMap<&str, &'static NodeDef> = HashMap::new();
    let mut ids = HashSet::new();
    for node in &graph.nodes {
        if !ids.insert(node.id.as_str()) {
            out.push(
                GraphIssue::error(
                    "duplicate_node",
                    format!("Узел {} встречается дважды", node.id),
                    format!("Node {} appears twice", node.id),
                )
                .on_node(&node.id),
            );
        }
        match node_def(&node.kind) {
            Some(def) => {
                defs.insert(node.id.as_str(), def);
            }
            None => out.push(
                GraphIssue::error(
                    "unknown_node",
                    format!(
                        "Неизвестный узел «{}» — возможно, из более новой версии",
                        node.kind
                    ),
                    format!("Unknown node «{}», maybe from a newer version", node.kind),
                )
                .on_node(&node.id),
            ),
        }
    }
    let triggers: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            !node.disabled
                && defs
                    .get(node.id.as_str())
                    .is_some_and(|def| def.is_trigger())
        })
        .map(|node| node.id.as_str())
        .collect();
    if triggers.is_empty() && !graph.nodes.is_empty() {
        out.push(GraphIssue::error(
            "no_trigger",
            "Нет триггера: добавьте узел, с которого схема начинается (например, «По расписанию»)"
                .into(),
            "No trigger: add a node the workflow starts from (for instance «On a schedule»)".into(),
        ));
    }

    let mut fed: HashSet<(&str, &str)> = HashSet::new();
    for edge in &graph.edges {
        let (Some(from_def), Some(to_def)) =
            (defs.get(edge.from.as_str()), defs.get(edge.to.as_str()))
        else {
            if !ids.contains(edge.from.as_str()) || !ids.contains(edge.to.as_str()) {
                out.push(
                    GraphIssue::error(
                        "dangling_edge",
                        "Связь ведёт к удалённому узлу".into(),
                        "A wire leads to a removed node".into(),
                    )
                    .on_edge(&edge.id),
                );
            }
            continue;
        };
        let Some(from_port) = from_def.output(&edge.from_port) else {
            out.push(
                GraphIssue::error(
                    "unknown_port",
                    format!("У «{}» нет выхода «{}»", from_def.title.ru, edge.from_port),
                    format!("«{}» has no output «{}»", from_def.title.en, edge.from_port),
                )
                .on_edge(&edge.id)
                .on_node(&edge.from),
            );
            continue;
        };
        let Some(to_port) = to_def.input(&edge.to_port) else {
            out.push(
                GraphIssue::error(
                    "unknown_port",
                    format!("У «{}» нет входа «{}»", to_def.title.ru, edge.to_port),
                    format!("«{}» has no input «{}»", to_def.title.en, edge.to_port),
                )
                .on_edge(&edge.id)
                .on_node(&edge.to),
            );
            continue;
        };
        if !to_port.ty.accepts(from_port.ty) {
            out.push(
                GraphIssue::error(
                    "type_mismatch",
                    format!(
                        "«{}» → «{}»: выход «{}» не подходит ко входу «{}»",
                        from_def.title.ru, to_def.title.ru, from_port.label.ru, to_port.label.ru
                    ),
                    format!(
                        "«{}» → «{}»: output «{}» does not fit input «{}»",
                        from_def.title.en, to_def.title.en, from_port.label.en, to_port.label.en
                    ),
                )
                .on_edge(&edge.id)
                .on_node(&edge.to),
            );
        }
        if edge.from == edge.to {
            out.push(
                GraphIssue::error(
                    "self_loop",
                    "Узел соединён сам с собой".into(),
                    "A node is wired to itself".into(),
                )
                .on_edge(&edge.id)
                .on_node(&edge.to),
            );
        }
        // A flow input may be reached from several places; a data input has one source.
        if to_port.ty != PortType::Flow && !fed.insert((edge.to.as_str(), edge.to_port.as_str())) {
            out.push(
                GraphIssue::error(
                    "input_twice",
                    format!(
                        "Ко входу «{}» подключено несколько связей",
                        to_port.label.ru
                    ),
                    format!("Input «{}» has more than one wire", to_port.label.en),
                )
                .on_edge(&edge.id)
                .on_node(&edge.to),
            );
        }
    }

    // Required inputs: a data input without a wire and without a field of the same name.
    for node in &graph.nodes {
        let Some(def) = defs.get(node.id.as_str()) else {
            continue;
        };
        if node.disabled {
            continue;
        }
        for port in def.inputs.iter().filter(|port| port.ty != PortType::Flow) {
            let wired = fed.contains(&(node.id.as_str(), port.name));
            if wired || def.param(port.name).is_some() || optional_input(def.kind, port.name) {
                continue;
            }
            out.push(
                GraphIssue::error(
                    "input_missing",
                    format!("«{}»: подключите вход «{}»", def.title.ru, port.label.ru),
                    format!("«{}»: wire the «{}» input", def.title.en, port.label.en),
                )
                .on_node(&node.id),
            );
        }
        // A write drawn apart from the flow runs on its own at the end of the run.
        let free = super::exec::FREE_RUNNING.contains(&def.kind);
        if def.has_flow() && def.input("in").is_some() && !free {
            let reached = graph
                .edges
                .iter()
                .any(|edge| edge.to == node.id && edge.to_port == "in");
            // One that takes data from an action runs after it by data order.
            if !reached && !order.contains_key(&node.id) {
                out.push(
                    GraphIssue::warning(
                        "unreachable",
                        format!(
                            "«{}» ни с чем не связан по потоку и не выполнится",
                            def.title.ru
                        ),
                        format!("«{}» has no incoming flow and never runs", def.title.en),
                    )
                    .on_node(&node.id),
                );
            }
        }
    }

    out.extend(order_issues(graph, &order));
    out.extend(queue_issues(graph));

    if let Some(node_id) = find_cycle(graph, &defs, true) {
        out.push(
            GraphIssue::error(
                "flow_cycle",
                "Поток замкнут в кольцо — для повторов используйте «Повторить N раз», «Цикл по числам», «Пока условие да» или «Для каждого»".into(),
                "The flow loops back on itself; to repeat use «Repeat N times», «Count from … to», «While» or «For each»".into(),
            )
            .on_node(&node_id),
        );
    }
    if let Some(node_id) = find_cycle(graph, &defs, false) {
        out.push(
            GraphIssue::error(
                "data_cycle",
                "Данные зависят сами от себя".into(),
                "Data depends on itself".into(),
            )
            .on_node(&node_id),
        );
    }

    out.sort_by_key(|issue| issue.level == IssueLevel::Warning);
    out
}

/// A node on a cycle of flow wires (`flow`) or of data wires, if any.
fn find_cycle(graph: &Graph, defs: &HashMap<&str, &'static NodeDef>, flow: bool) -> Option<String> {
    let mut next: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        let Some(def) = defs.get(edge.from.as_str()) else {
            continue;
        };
        let is_flow = def
            .output(&edge.from_port)
            .is_some_and(|port| port.ty == PortType::Flow);
        if is_flow == flow {
            next.entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
        }
    }
    // 0 = not seen, 1 = on the current path, 2 = done.
    let mut state: HashMap<&str, u8> = HashMap::new();
    for start in graph.nodes.iter().map(|node| node.id.as_str()) {
        if state.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
        state.insert(start, 1);
        while let Some((node, index)) = stack.pop() {
            let targets = next.get(node).map(Vec::as_slice).unwrap_or_default();
            if let Some(&target) = targets.get(index) {
                stack.push((node, index + 1));
                match state.get(target).copied().unwrap_or(0) {
                    1 => return Some(target.to_string()),
                    0 => {
                        state.insert(target, 1);
                        stack.push((target, 0));
                    }
                    _ => {}
                }
            } else {
                state.insert(node, 2);
            }
        }
    }
    None
}

/// Whether a graph has errors that keep it from running.
/// A node's name for a message: its label, else its catalog title.
fn name_of(graph: &Graph, id: &str) -> (String, String) {
    let node = graph.node(id);
    let label = node
        .map(|node| node.label.trim().to_string())
        .unwrap_or_default();
    match node.and_then(|node| node_def(&node.kind)) {
        _ if !label.is_empty() => (label.clone(), label),
        Some(def) => (def.title.ru.to_string(), def.title.en.to_string()),
        None => (id.to_string(), id.to_string()),
    }
}

/// Whether `port` of node `id` is a flow input.
fn is_flow_input(graph: &Graph, id: &str, port: &str) -> bool {
    graph
        .node(id)
        .and_then(|node| node_def(&node.kind))
        .and_then(|def| def.input(port))
        .is_some_and(|input| input.ty == PortType::Flow)
}

/// Everything that runs before `id`: back along flow wires and data order.
fn ancestors(graph: &Graph, order: &HashMap<String, Vec<String>>, id: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut todo = vec![id.to_string()];
    while let Some(current) = todo.pop() {
        let flow_in = graph
            .edges
            .iter()
            .filter(|edge| edge.to == current && is_flow_input(graph, &edge.to, &edge.to_port))
            .map(|edge| edge.from.clone());
        let by_data = order.get(&current).cloned().unwrap_or_default();
        for before in flow_in.chain(by_data) {
            if seen.insert(before.clone()) {
                todo.push(before);
            }
        }
    }
    seen
}

/// Data order mistakes: an input read from an action that only runs later,
/// and a flow wire that repeats what the data wires already say.
fn order_issues(graph: &Graph, order: &HashMap<String, Vec<String>>) -> Vec<GraphIssue> {
    let mut out = Vec::new();
    for node in graph
        .nodes
        .iter()
        .filter(|node| !node.disabled && graph.has_flow_in(node))
    {
        let before = ancestors(graph, order, &node.id);
        for source in graph.action_sources(&node.id) {
            if !before.contains(&source) {
                let ((who_ru, who_en), (from_ru, from_en)) =
                    (name_of(graph, &node.id), name_of(graph, &source));
                out.push(
                    GraphIssue::warning(
                        "late_data",
                        format!("«{who_ru}» берёт данные у «{from_ru}», но поток доходит до него раньше — значение будет пустым"),
                        format!("«{who_en}» takes data from «{from_en}», but the flow reaches it first: the value will be empty"),
                    )
                    .on_node(&node.id),
                );
            }
        }
        let flow_in: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.to == node.id && is_flow_input(graph, &edge.to, &edge.to_port))
            .collect();
        if let [only] = flow_in.as_slice()
            && only.from_port == "out"
            && graph.action_sources(&node.id).contains(&only.from)
        {
            let ((from_ru, from_en), (to_ru, to_en)) =
                (name_of(graph, &only.from), name_of(graph, &node.id));
            out.push(
                GraphIssue::warning(
                    "redundant_flow",
                    format!("Поток «{from_ru}» → «{to_ru}» повторяет связь по данным: его можно убрать, порядок останется"),
                    format!("The flow «{from_en}» → «{to_en}» repeats the data wire: it can go, the order stays"),
                )
                .on_edge(&only.id),
            );
        }
    }
    out
}

/// Nodes reached by flow wires from `start` leaving through `port` (or any port).
fn flow_reach(graph: &Graph, start: &str, port: Option<&str>) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack: Vec<String> = graph
        .edges
        .iter()
        .filter(|edge| edge.from == start && port.is_none_or(|name| edge.from_port == name))
        .filter(|edge| is_flow_input(graph, &edge.to, &edge.to_port))
        .map(|edge| edge.to.clone())
        .collect();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        stack.extend(
            graph
                .edges
                .iter()
                .filter(|edge| edge.from == id && is_flow_input(graph, &edge.to, &edge.to_port))
                .map(|edge| edge.to.clone()),
        );
    }
    seen
}

/// «Очередь» hands on the firings of a trigger, so it stands after one; a
/// wait inside it would stop the items that are left.
fn queue_issues(graph: &Graph) -> Vec<GraphIssue> {
    let mut out = Vec::new();
    for queue in graph
        .nodes
        .iter()
        .filter(|node| !node.disabled && node.kind == "flow.queue")
    {
        let after_trigger = graph
            .nodes
            .iter()
            .filter(|node| !node.disabled && node.kind.starts_with("trigger."))
            .any(|trigger| flow_reach(graph, &trigger.id, None).contains(&queue.id));
        if !after_trigger {
            out.push(
                GraphIssue::error(
                    "queue_without_trigger",
                    "«Очередь» должна стоять после триггера по потоку — она раздаёт его срабатывания".into(),
                    "«Queue» must follow a trigger by flow: it hands on that trigger's firings".into(),
                )
                .on_node(&queue.id),
            );
        }
        for id in flow_reach(graph, &queue.id, Some("each")) {
            let sleeps = graph
                .node(&id)
                .is_some_and(|node| matches!(node.kind.as_str(), "action.wait" | "action.confirm"));
            if sleeps {
                let (who_ru, who_en) = name_of(graph, &id);
                out.push(
                    GraphIssue::warning(
                        "queue_sleeps",
                        format!("«{who_ru}» внутри «Очереди» усыпит запуск: оставшиеся срабатывания не пройдут"),
                        format!("«{who_en}» inside «Queue» puts the run to sleep: the firings left won't go through"),
                    )
                    .on_node(&id),
                );
            }
        }
    }
    out
}

pub fn has_errors(issues: &[GraphIssue]) -> bool {
    issues.iter().any(|issue| issue.level == IssueLevel::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::graph::{Edge, Node, Position};

    pub(crate) fn node(id: &str, kind: &str) -> Node {
        Node {
            id: id.into(),
            kind: kind.into(),
            params: serde_json::Map::new(),
            position: Position::default(),
            disabled: false,
            label: String::new(),
        }
    }

    pub(crate) fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
        Edge {
            id: format!("{from}.{from_port}-{to}.{to_port}"),
            from: from.into(),
            from_port: from_port.into(),
            to: to.into(),
            to_port: to_port.into(),
        }
    }

    fn codes(graph: &Graph) -> Vec<String> {
        validate(graph)
            .into_iter()
            .map(|issue| issue.code)
            .collect()
    }

    #[test]
    fn a_simple_chain_is_clean() {
        let graph = Graph {
            nodes: vec![node("t", "trigger.manual"), node("n", "action.notify")],
            edges: vec![edge("t", "out", "n", "in")],
            ..Graph::default()
        };
        assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    }

    #[test]
    fn no_trigger_and_unknown_nodes_are_errors() {
        let graph = Graph {
            nodes: vec![
                node("n", "action.transaction"),
                node("x", "nope.nope"),
                node("free", "action.notify"),
            ],
            ..Graph::default()
        };
        let found = codes(&graph);
        assert!(found.contains(&"no_trigger".to_string()));
        assert!(found.contains(&"unknown_node".to_string()));
        assert!(found.contains(&"unreachable".to_string()));
        // A notice apart from the flow runs at the end: not a mistake.
        assert!(
            !validate(&graph).iter().any(
                |issue| issue.code == "unreachable" && issue.node_id.as_deref() == Some("free")
            )
        );
        assert_eq!(validate(&graph).last().unwrap().level, IssueLevel::Warning);
    }

    #[test]
    fn wires_must_fit_and_feed_an_input_once() {
        let graph = Graph {
            nodes: vec![
                node("t", "trigger.manual"),
                node("now", "data.now"),
                node("b", "logic.branch"),
                node("tx", "trigger.transaction"),
            ],
            edges: vec![
                edge("t", "out", "b", "in"),
                // A date is not a yes/no.
                edge("now", "now", "b", "condition"),
                edge("tx", "out", "b", "in"),
                // Flow into data.
                edge("t", "out", "b", "condition"),
            ],
            ..Graph::default()
        };
        let found = codes(&graph);
        assert!(found.contains(&"type_mismatch".to_string()), "{found:?}");
        assert!(found.contains(&"input_twice".to_string()), "{found:?}");
    }

    #[test]
    fn a_required_input_without_a_wire_is_reported() {
        let graph = Graph {
            nodes: vec![node("t", "trigger.manual"), node("b", "logic.branch")],
            edges: vec![edge("t", "out", "b", "in")],
            ..Graph::default()
        };
        assert_eq!(codes(&graph), vec!["input_missing".to_string()]);
    }

    #[test]
    fn cycles_are_found() {
        let graph = Graph {
            nodes: vec![
                node("t", "trigger.manual"),
                node("a", "action.notify"),
                node("b", "action.notify"),
                node("m1", "logic.math"),
                node("m2", "logic.math"),
            ],
            edges: vec![
                edge("t", "out", "a", "in"),
                edge("a", "out", "b", "in"),
                edge("b", "out", "a", "in"),
                edge("m1", "result", "m2", "a"),
                edge("m2", "result", "m1", "a"),
            ],
            ..Graph::default()
        };
        let found = codes(&graph);
        assert!(found.contains(&"flow_cycle".to_string()), "{found:?}");
        assert!(found.contains(&"data_cycle".to_string()), "{found:?}");
    }
}
