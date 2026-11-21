//! The node kinds of the workflow catalog as they were when the transition was frozen, reduced to
//! the facts the graph step of version 3 reads. The live catalog may change; this table does not.

/// One frozen node kind.
pub(super) struct Kind {
    pub(super) kind: &'static str,
    pub(super) trigger: bool,
    /// Names of the flow inputs.
    pub(super) flow_inputs: &'static [&'static str],
    /// Names of the inputs that carry data.
    pub(super) data_inputs: &'static [&'static str],
    pub(super) flow_output: bool,
}

impl Kind {
    /// Whether the node takes part in the flow (has a flow input or output).
    pub(super) fn has_flow(&self) -> bool {
        !self.flow_inputs.is_empty() || self.flow_output
    }
}

/// The frozen kind named `kind`, if it was in the catalog.
pub(super) fn kind(kind: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|frozen| frozen.kind == kind)
}

#[rustfmt::skip]
static KINDS: [Kind; 80] = [
    Kind { kind: "trigger.manual", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.schedule", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.transaction", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.balance", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.price", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.source", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.event", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "trigger.called", trigger: true, flow_inputs: &[], data_inputs: &[], flow_output: true },
    Kind { kind: "data.balance", trigger: false, flow_inputs: &[], data_inputs: &["account", "asset"], flow_output: false },
    Kind { kind: "data.price", trigger: false, flow_inputs: &[], data_inputs: &["asset", "currency"], flow_output: false },
    Kind { kind: "data.fx", trigger: false, flow_inputs: &[], data_inputs: &["from", "to"], flow_output: false },
    Kind { kind: "data.constant", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.sum", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.recent_average", trigger: false, flow_inputs: &[], data_inputs: &["fallback"], flow_output: false },
    Kind { kind: "data.source", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.budget", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.goal", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.now", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.tx_fields", trigger: false, flow_inputs: &[], data_inputs: &["transaction"], flow_output: false },
    Kind { kind: "data.workday", trigger: false, flow_inputs: &[], data_inputs: &["date"], flow_output: false },
    Kind { kind: "logic.compare", trigger: false, flow_inputs: &[], data_inputs: &["a", "b"], flow_output: false },
    Kind { kind: "logic.and", trigger: false, flow_inputs: &[], data_inputs: &["a", "b"], flow_output: false },
    Kind { kind: "logic.or", trigger: false, flow_inputs: &[], data_inputs: &["a", "b"], flow_output: false },
    Kind { kind: "logic.not", trigger: false, flow_inputs: &[], data_inputs: &["a"], flow_output: false },
    Kind { kind: "logic.branch", trigger: false, flow_inputs: &["in"], data_inputs: &["condition"], flow_output: true },
    Kind { kind: "logic.guard", trigger: false, flow_inputs: &["in"], data_inputs: &["condition"], flow_output: true },
    Kind { kind: "logic.math", trigger: false, flow_inputs: &[], data_inputs: &["a", "b"], flow_output: false },
    Kind { kind: "logic.convert", trigger: false, flow_inputs: &[], data_inputs: &["amount", "currency"], flow_output: false },
    Kind { kind: "logic.text", trigger: false, flow_inputs: &[], data_inputs: &["a", "b", "c"], flow_output: false },
    Kind { kind: "logic.throttle", trigger: false, flow_inputs: &["in"], data_inputs: &[], flow_output: true },
    Kind { kind: "var.get", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "var.set", trigger: false, flow_inputs: &["in"], data_inputs: &["value"], flow_output: true },
    Kind { kind: "var.add", trigger: false, flow_inputs: &["in"], data_inputs: &["delta"], flow_output: true },
    Kind { kind: "action.transaction", trigger: false, flow_inputs: &["in"], data_inputs: &["account", "to", "asset", "amount", "to_asset", "to_amount", "category", "comment", "date"], flow_output: true },
    Kind { kind: "action.distribute", trigger: false, flow_inputs: &["in"], data_inputs: &["amount", "from"], flow_output: true },
    Kind { kind: "action.notify", trigger: false, flow_inputs: &["in"], data_inputs: &["text"], flow_output: true },
    Kind { kind: "action.reminder", trigger: false, flow_inputs: &["in"], data_inputs: &["text", "date"], flow_output: true },
    Kind { kind: "action.confirm", trigger: false, flow_inputs: &["in"], data_inputs: &["question"], flow_output: true },
    Kind { kind: "action.stop", trigger: false, flow_inputs: &["in"], data_inputs: &[], flow_output: false },
    Kind { kind: "action.emit", trigger: false, flow_inputs: &["in"], data_inputs: &["payload"], flow_output: true },
    Kind { kind: "action.wait", trigger: false, flow_inputs: &["in"], data_inputs: &["hours"], flow_output: true },
    Kind { kind: "action.call", trigger: false, flow_inputs: &["in"], data_inputs: &["payload"], flow_output: true },
    Kind { kind: "action.return", trigger: false, flow_inputs: &["in"], data_inputs: &["value"], flow_output: false },
    Kind { kind: "list.accounts", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "list.goals", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "list.for_each", trigger: false, flow_inputs: &["in"], data_inputs: &["items"], flow_output: true },
    Kind { kind: "flow.queue", trigger: false, flow_inputs: &["in"], data_inputs: &[], flow_output: true },
    Kind { kind: "list.count", trigger: false, flow_inputs: &[], data_inputs: &["items"], flow_output: false },
    Kind { kind: "list.field", trigger: false, flow_inputs: &[], data_inputs: &["record"], flow_output: false },
    Kind { kind: "date.parts", trigger: false, flow_inputs: &[], data_inputs: &["date"], flow_output: false },
    Kind { kind: "date.make", trigger: false, flow_inputs: &[], data_inputs: &["base", "year", "month", "day"], flow_output: false },
    Kind { kind: "date.add", trigger: false, flow_inputs: &[], data_inputs: &["date", "amount", "duration"], flow_output: false },
    Kind { kind: "date.diff", trigger: false, flow_inputs: &[], data_inputs: &["a", "b"], flow_output: false },
    Kind { kind: "date.boundary", trigger: false, flow_inputs: &[], data_inputs: &["date"], flow_output: false },
    Kind { kind: "date.shift_workday", trigger: false, flow_inputs: &[], data_inputs: &["date"], flow_output: false },
    Kind { kind: "date.format", trigger: false, flow_inputs: &[], data_inputs: &["date", "pattern"], flow_output: false },
    Kind { kind: "text.extract", trigger: false, flow_inputs: &[], data_inputs: &["text", "pattern"], flow_output: false },
    Kind { kind: "text.number", trigger: false, flow_inputs: &[], data_inputs: &["text"], flow_output: false },
    Kind { kind: "text.split", trigger: false, flow_inputs: &[], data_inputs: &["text", "separator"], flow_output: false },
    Kind { kind: "text.replace", trigger: false, flow_inputs: &[], data_inputs: &["text", "find", "with"], flow_output: false },
    Kind { kind: "text.case", trigger: false, flow_inputs: &[], data_inputs: &["text"], flow_output: false },
    Kind { kind: "text.join", trigger: false, flow_inputs: &[], data_inputs: &["items", "separator"], flow_output: false },
    Kind { kind: "convert.to", trigger: false, flow_inputs: &[], data_inputs: &["value"], flow_output: false },
    Kind { kind: "logic.select", trigger: false, flow_inputs: &[], data_inputs: &["condition", "a", "b"], flow_output: false },
    Kind { kind: "logic.round", trigger: false, flow_inputs: &[], data_inputs: &["value", "places"], flow_output: false },
    Kind { kind: "logic.switch", trigger: false, flow_inputs: &["in"], data_inputs: &["value"], flow_output: true },
    Kind { kind: "list.make", trigger: false, flow_inputs: &[], data_inputs: &["a", "b", "c", "d"], flow_output: false },
    Kind { kind: "list.at", trigger: false, flow_inputs: &[], data_inputs: &["items", "index"], flow_output: false },
    Kind { kind: "list.sum", trigger: false, flow_inputs: &[], data_inputs: &["items"], flow_output: false },
    Kind { kind: "list.filter", trigger: false, flow_inputs: &[], data_inputs: &["items", "value"], flow_output: false },
    Kind { kind: "list.record", trigger: false, flow_inputs: &[], data_inputs: &["v1", "v2", "v3"], flow_output: false },
    Kind { kind: "loop.repeat", trigger: false, flow_inputs: &["in"], data_inputs: &["count"], flow_output: true },
    Kind { kind: "loop.range", trigger: false, flow_inputs: &["in"], data_inputs: &["from", "to", "step"], flow_output: true },
    Kind { kind: "loop.while", trigger: false, flow_inputs: &["in"], data_inputs: &["condition"], flow_output: true },
    Kind { kind: "loop.break", trigger: false, flow_inputs: &["in"], data_inputs: &[], flow_output: false },
    Kind { kind: "data.emergency_gap", trigger: false, flow_inputs: &[], data_inputs: &["amount"], flow_output: false },
    Kind { kind: "data.key_rate", trigger: false, flow_inputs: &[], data_inputs: &[], flow_output: false },
    Kind { kind: "data.inflation", trigger: false, flow_inputs: &[], data_inputs: &["currency"], flow_output: false },
    Kind { kind: "data.recurring", trigger: false, flow_inputs: &[], data_inputs: &["transaction"], flow_output: false },
    Kind { kind: "data.topup_advice", trigger: false, flow_inputs: &[], data_inputs: &["amount"], flow_output: false },
];
