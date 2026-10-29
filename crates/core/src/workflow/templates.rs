//! Ready-made workflows of the capital plan: a short form fills a few fields,
//! the rest of the graph is built here. Money only moves after «Ask first».

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::graph::GRAPH_VERSION;

/// What a form field picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum TemplateFieldKind {
    Store,
    Category,
    Number,
    Currency,
    Budget,
    Weekday,
    Day,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TemplateField {
    /// Key in the params and in `workflowTemplates.<id>.field.<key>`.
    pub key: String,
    pub kind: TemplateFieldKind,
    pub required: bool,
    /// Prefilled value; empty when the user has to choose.
    pub default: String,
}

/// One template of the gallery; titles live in the locales.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct WorkflowTemplateInfo {
    pub id: String,
    pub fields: Vec<TemplateField>,
    /// Creates transfers (always behind a confirmation).
    pub moves_money: bool,
}

/// What a template needs from the vault for its defaults.
pub struct TemplateDefaults {
    /// `savings_rate` in percent.
    pub savings_pct: String,
    /// Budget currency, lowercase.
    pub currency: String,
}

fn field(key: &str, kind: TemplateFieldKind, required: bool, default: &str) -> TemplateField {
    TemplateField {
        key: key.into(),
        kind,
        required,
        default: default.into(),
    }
}

pub const TEMPLATE_IDS: [&str; 7] = [
    "pay_yourself_first",
    "rates_watch",
    "budget_pace",
    "big_spend",
    "subscription_watch",
    "round_up",
    "topup_by_shares",
];

/// The gallery with defaults filled in.
pub fn template_list(defaults: &TemplateDefaults) -> Vec<WorkflowTemplateInfo> {
    use TemplateFieldKind as Kind;
    let info = |id: &str, moves_money: bool, fields: Vec<TemplateField>| WorkflowTemplateInfo {
        id: id.into(),
        fields,
        moves_money,
    };
    vec![
        info(
            "pay_yourself_first",
            true,
            vec![
                field("category", Kind::Category, true, "salary"),
                field("pct", Kind::Number, true, &defaults.savings_pct),
                field("target", Kind::Store, true, ""),
                field("source", Kind::Store, false, ""),
            ],
        ),
        info(
            "rates_watch",
            false,
            vec![field("currency", Kind::Currency, true, &defaults.currency)],
        ),
        info(
            "budget_pace",
            false,
            vec![
                field("budget", Kind::Budget, true, ""),
                field("margin", Kind::Number, true, "10"),
            ],
        ),
        info(
            "big_spend",
            false,
            vec![field("min_amount", Kind::Number, true, "10000")],
        ),
        info(
            "subscription_watch",
            false,
            vec![field("category", Kind::Category, true, "subscriptions")],
        ),
        info(
            "round_up",
            true,
            vec![
                field("from", Kind::Store, true, ""),
                field("to", Kind::Store, true, ""),
                field("step", Kind::Number, true, "100"),
                field("weekday", Kind::Weekday, true, "6"),
            ],
        ),
        info(
            "topup_by_shares",
            false,
            vec![field("day", Kind::Day, true, "1")],
        ),
    ]
}

/// A built workflow: name, description and the graph as JSON.
pub struct BuiltTemplate {
    pub name: String,
    pub description: String,
    pub graph: String,
}

struct Builder<'a> {
    ru: bool,
    params: &'a BTreeMap<String, String>,
    nodes: Vec<Value>,
    edges: Vec<Value>,
}

impl Builder<'_> {
    fn tr(&self, ru: &str, en: &str) -> String {
        if self.ru { ru } else { en }.to_string()
    }

    fn value(&self, key: &str) -> String {
        self.params
            .get(key)
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn number(&self, key: &str) -> Value {
        json!(self.value(key))
    }

    fn integer(&self, key: &str, fallback: u32) -> Value {
        json!(self.value(key).parse::<u32>().unwrap_or(fallback))
    }

    fn node(&mut self, id: &str, kind: &str, column: u32, row: u32, params: Value) {
        self.nodes.push(json!({
            "id": id, "kind": kind, "params": params,
            "position": { "x": column * 300, "y": row * 170 },
            "disabled": false, "label": "",
        }));
    }

    fn wire(&mut self, from: &str, from_port: &str, to: &str, to_port: &str) {
        self.edges.push(json!({
            "id": format!("{from}.{from_port}-{to}.{to_port}"),
            "from": from, "from_port": from_port, "to": to, "to_port": to_port,
        }));
    }

    fn notify(&mut self, id: &str, column: u32, row: u32, title: &str) {
        self.node(id, "action.notify", column, row, json!({ "title": title }));
    }

    fn graph(self) -> String {
        json!({
            "version": GRAPH_VERSION,
            "nodes": self.nodes,
            "edges": self.edges,
            "comments": [],
        })
        .to_string()
    }
}

/// Builds the workflow of a template; `None` for an unknown id.
pub fn build_template(
    id: &str,
    params: &BTreeMap<String, String>,
    locale: &str,
) -> Option<BuiltTemplate> {
    let mut build = Builder {
        ru: !locale.starts_with("en"),
        params,
        nodes: vec![],
        edges: vec![],
    };
    let (name, description) = match id {
        "pay_yourself_first" => pay_yourself_first(&mut build),
        "rates_watch" => rates_watch(&mut build),
        "budget_pace" => budget_pace(&mut build),
        "big_spend" => big_spend(&mut build),
        "subscription_watch" => subscription_watch(&mut build),
        "round_up" => round_up(&mut build),
        "topup_by_shares" => topup_by_shares(&mut build),
        _ => return None,
    };
    Some(BuiltTemplate {
        name,
        description,
        graph: build.graph(),
    })
}

type Texts = (String, String);

fn pay_yourself_first(build: &mut Builder) -> Texts {
    let category = build.value("category");
    let pct = build.number("pct");
    let target = build.value("target");
    // "From wallet" (Auto-2 · 12): only income to this wallet; empty — any.
    let source = build.value("source");
    let question = build.tr(
        "Пришёл доход. Отложить {a}: в подушку {b}, на счёт накоплений {c}?",
        "Income arrived. Put aside {a}: {b} to the emergency fund, {c} to savings?",
    );
    build.node(
        "income",
        "trigger.transaction",
        0,
        0,
        json!({ "tx_kind": "topup", "category": category, "store": source }),
    );
    build.node(
        "share",
        "logic.math",
        0,
        2,
        json!({ "op": "percent", "b": pct, "round": "2" }),
    );
    build.node("gap", "data.emergency_gap", 1, 2, json!({}));
    build.node("text", "logic.text", 1, 3, json!({ "template": question }));
    build.node(
        "ask",
        "action.confirm",
        1,
        0,
        json!({ "timeout_hours": 72 }),
    );
    build.node(
        "has_gap",
        "logic.compare",
        2,
        2,
        json!({ "op": "gt", "b": "0" }),
    );
    build.node("to_fund", "logic.branch", 2, 0, json!({}));
    build.node(
        "fund",
        "action.transaction",
        3,
        0,
        json!({ "tx_kind": "transfer" }),
    );
    build.node(
        "has_rest",
        "logic.compare",
        3,
        2,
        json!({ "op": "gt", "b": "0" }),
    );
    build.node("to_rest", "logic.branch", 4, 0, json!({}));
    build.node(
        "save",
        "action.transaction",
        5,
        0,
        json!({ "tx_kind": "transfer", "to": target }),
    );
    build.wire("income", "amount", "share", "a");
    build.wire("share", "result", "gap", "amount");
    build.wire("share", "result", "text", "a");
    build.wire("gap", "to_cushion", "text", "b");
    build.wire("gap", "rest", "text", "c");
    build.wire("income", "out", "ask", "in");
    build.wire("text", "text", "ask", "question");
    build.wire("gap", "to_cushion", "has_gap", "a");
    build.wire("ask", "true", "to_fund", "in");
    build.wire("has_gap", "result", "to_fund", "condition");
    build.wire("to_fund", "true", "fund", "in");
    build.wire("income", "account", "fund", "account");
    build.wire("gap", "store", "fund", "to");
    build.wire("gap", "to_cushion", "fund", "amount");
    build.wire("gap", "rest", "has_rest", "a");
    build.wire("fund", "out", "to_rest", "in");
    build.wire("to_fund", "false", "to_rest", "in");
    build.wire("has_rest", "result", "to_rest", "condition");
    build.wire("to_rest", "true", "save", "in");
    build.wire("income", "account", "save", "account");
    build.wire("gap", "rest", "save", "amount");
    (
        build.tr("Заплати себе первым", "Pay yourself first"),
        build.tr(
            "С каждого дохода — доля в подушку до цели, остальное на счёт накоплений. Переводы — после подтверждения.",
            "A share of each income goes to the emergency fund up to its target, the rest to savings. Transfers wait for your yes.",
        ),
    )
}

fn rates_watch(build: &mut Builder) -> Texts {
    let currency = build.value("currency");
    let message = build.tr(
        "Инфляция ({c}): {a}% г/г. Ключевая ставка: {b}%.",
        "Inflation ({c}): {a}% a year. Key rate: {b}%.",
    );
    let title = build.tr("Ставка и инфляция", "Rate and inflation");
    build.node(
        "daily",
        "trigger.schedule",
        0,
        0,
        json!({ "every": "day", "interval": "1", "time": "10:00" }),
    );
    build.node(
        "inflation",
        "data.inflation",
        0,
        2,
        json!({ "currency": currency }),
    );
    build.node("rate", "data.key_rate", 0, 3, json!({}));
    build.node("key", "logic.text", 1, 2, json!({ "template": "{a}|{b}" }));
    build.node(
        "seen",
        "var.get",
        1,
        3,
        json!({ "name": "rates_seen", "scope": "workflow" }),
    );
    build.node("changed", "logic.compare", 2, 2, json!({ "op": "ne" }));
    build.node(
        "guard",
        "logic.guard",
        1,
        0,
        json!({ "reason": build.tr("Ничего не изменилось", "Nothing changed") }),
    );
    build.node("text", "logic.text", 2, 3, json!({ "template": message }));
    build.notify("tell", 2, 0, &title);
    build.node(
        "remember",
        "var.set",
        3,
        0,
        json!({ "name": "rates_seen", "scope": "workflow" }),
    );
    build.wire("inflation", "value", "key", "a");
    build.wire("rate", "rate", "key", "b");
    build.wire("key", "text", "changed", "a");
    build.wire("seen", "value", "changed", "b");
    build.wire("daily", "out", "guard", "in");
    build.wire("changed", "result", "guard", "condition");
    build.wire("inflation", "value", "text", "a");
    build.wire("rate", "rate", "text", "b");
    build.wire("inflation", "month", "text", "c");
    build.wire("guard", "out", "tell", "in");
    build.wire("text", "text", "tell", "text");
    build.wire("tell", "out", "remember", "in");
    build.wire("key", "text", "remember", "value");
    (
        title,
        build.tr(
            "Раз в день проверяет инфляцию и ключевую ставку и сообщает, когда они изменились.",
            "Checks inflation and the key rate daily and tells you when they change.",
        ),
    )
}

fn budget_pace(build: &mut Builder) -> Texts {
    let budget = build.value("budget");
    let margin = build.number("margin");
    let message = build.tr(
        "Лимит израсходован на {a}%, а прошло только {b}% периода.",
        "{a}% of the limit is spent while only {b}% of the period has passed.",
    );
    build.node(
        "evening",
        "trigger.schedule",
        0,
        0,
        json!({ "every": "day", "interval": "1", "time": "20:00" }),
    );
    build.node("budget", "data.budget", 0, 2, json!({ "budget": budget }));
    build.node(
        "limit",
        "logic.math",
        1,
        2,
        json!({ "op": "add", "b": margin, "round": "2" }),
    );
    build.node("ahead", "logic.compare", 2, 2, json!({ "op": "gt" }));
    build.node(
        "guard",
        "logic.guard",
        1,
        0,
        json!({ "reason": build.tr("Траты идут по плану", "Spending is on pace") }),
    );
    build.node("text", "logic.text", 2, 3, json!({ "template": message }));
    let title = build.tr("Лимит тратится быстро", "The limit goes fast");
    build.notify("tell", 2, 0, &title);
    build.wire("budget", "elapsed", "limit", "a");
    build.wire("budget", "used", "ahead", "a");
    build.wire("limit", "result", "ahead", "b");
    build.wire("evening", "out", "guard", "in");
    build.wire("ahead", "result", "guard", "condition");
    build.wire("budget", "used", "text", "a");
    build.wire("budget", "elapsed", "text", "b");
    build.wire("guard", "out", "tell", "in");
    build.wire("text", "text", "tell", "text");
    (
        build.tr("Сторож лимита", "Limit pace watch"),
        build.tr(
            "Каждый вечер сравнивает, какая часть лимита потрачена и какая часть периода прошла.",
            "Every evening compares the share of the limit spent with the share of the period gone.",
        ),
    )
}

fn big_spend(build: &mut Builder) -> Texts {
    let min_amount = build.number("min_amount");
    let message = build.tr(
        "Крупная трата: {a}, категория {b}.",
        "Big spend: {a}, category {b}.",
    );
    build.node(
        "spend",
        "trigger.transaction",
        0,
        0,
        json!({ "tx_kind": "payment", "min_amount": min_amount }),
    );
    build.node("text", "logic.text", 1, 1, json!({ "template": message }));
    let title = build.tr("Крупная трата", "Big spend");
    build.notify("tell", 1, 0, &title);
    build.wire("spend", "amount", "text", "a");
    build.wire("spend", "category", "text", "b");
    build.wire("spend", "out", "tell", "in");
    build.wire("text", "text", "tell", "text");
    (
        title,
        build.tr(
            "Сообщает о каждой трате больше заданной суммы.",
            "Tells you about every spend above the amount.",
        ),
    )
}

fn subscription_watch(build: &mut Builder) -> Texts {
    let category = build.value("category");
    let message = build.tr(
        "Платёж {a}. Прошлый: {b}, изменение {c}%.",
        "Payment {a}. Last one: {b}, change {c}%.",
    );
    build.node(
        "payment",
        "trigger.transaction",
        0,
        0,
        json!({ "tx_kind": "payment", "category": category }),
    );
    build.node("check", "data.recurring", 0, 2, json!({}));
    build.node("either", "logic.or", 1, 2, json!({}));
    build.node(
        "guard",
        "logic.guard",
        1,
        0,
        json!({ "reason": build.tr("Платёж как обычно", "The usual payment") }),
    );
    build.node("text", "logic.text", 2, 2, json!({ "template": message }));
    let title = build.tr(
        "Новая или подорожавшая подписка",
        "New or dearer subscription",
    );
    build.notify("tell", 2, 0, &title);
    build.wire("payment", "transaction", "check", "transaction");
    build.wire("check", "new", "either", "a");
    build.wire("check", "grew", "either", "b");
    build.wire("payment", "out", "guard", "in");
    build.wire("either", "result", "guard", "condition");
    build.wire("payment", "amount", "text", "a");
    build.wire("check", "previous", "text", "b");
    build.wire("check", "change_pct", "text", "c");
    build.wire("guard", "out", "tell", "in");
    build.wire("text", "text", "tell", "text");
    (
        build.tr("Сторож подписок", "Subscription watch"),
        build.tr(
            "Сообщает о новой подписке и о подписке, которая подорожала.",
            "Tells you about a new subscription and one that got dearer.",
        ),
    )
}

fn round_up(build: &mut Builder) -> Texts {
    let (from, to) = (build.value("from"), build.value("to"));
    let step = build.number("step");
    let weekday = build.integer("weekday", 6);
    let question = build.tr(
        "Перевести {a} из копилки округлений?",
        "Move {a} from the round-up jar?",
    );
    let var = json!({ "name": "round_up", "scope": "workflow" });
    build.node(
        "payment",
        "trigger.transaction",
        0,
        0,
        json!({ "tx_kind": "payment", "store": from }),
    );
    build.node(
        "steps",
        "logic.math",
        0,
        1,
        json!({ "op": "div", "b": step, "round": "8" }),
    );
    build.node(
        "up",
        "logic.round",
        1,
        1,
        json!({ "places": "0", "mode": "ceil" }),
    );
    build.node(
        "rounded",
        "logic.math",
        2,
        1,
        json!({ "op": "mul", "b": step, "round": "2" }),
    );
    build.node(
        "change",
        "logic.math",
        3,
        1,
        json!({ "op": "sub", "round": "2" }),
    );
    build.node("collect", "var.add", 3, 0, var.clone());
    build.wire("payment", "amount", "steps", "a");
    build.wire("steps", "result", "up", "value");
    build.wire("up", "result", "rounded", "a");
    build.wire("rounded", "result", "change", "a");
    build.wire("payment", "amount", "change", "b");
    build.wire("payment", "out", "collect", "in");
    build.wire("change", "result", "collect", "delta");

    build.node(
        "weekly",
        "trigger.schedule",
        0,
        3,
        json!({ "every": "week", "interval": "1", "weekday": weekday, "time": "19:00" }),
    );
    build.node("jar", "var.get", 0, 4, var.clone());
    build.node(
        "some",
        "logic.compare",
        1,
        4,
        json!({ "op": "gt", "b": "0" }),
    );
    build.node(
        "guard",
        "logic.guard",
        1,
        3,
        json!({ "reason": build.tr("Копилка пуста", "The jar is empty") }),
    );
    build.node("text", "logic.text", 2, 4, json!({ "template": question }));
    build.node(
        "ask",
        "action.confirm",
        2,
        3,
        json!({ "timeout_hours": 48 }),
    );
    build.node(
        "move",
        "action.transaction",
        3,
        3,
        json!({ "tx_kind": "transfer", "account": from, "to": to }),
    );
    build.node(
        "minus",
        "logic.math",
        3,
        4,
        json!({ "op": "mul", "b": "-1", "round": "2" }),
    );
    build.node("empty", "var.add", 4, 3, var);
    build.wire("jar", "value", "some", "a");
    build.wire("weekly", "out", "guard", "in");
    build.wire("some", "result", "guard", "condition");
    build.wire("jar", "value", "text", "a");
    build.wire("guard", "out", "ask", "in");
    build.wire("text", "text", "ask", "question");
    build.wire("ask", "true", "move", "in");
    build.wire("jar", "value", "move", "amount");
    build.wire("jar", "value", "minus", "a");
    build.wire("move", "out", "empty", "in");
    build.wire("minus", "result", "empty", "delta");
    (
        build.tr("Копилка округлений", "Round-up jar"),
        build.tr(
            "Каждая трата округляется вверх до шага, разница копится и раз в неделю переводится на выбранный счёт после подтверждения.",
            "Each spend is rounded up to the step, the change adds up and moves to the chosen account once a week after your yes.",
        ),
    )
}

fn topup_by_shares(build: &mut Builder) -> Texts {
    let day = build.integer("day", 1);
    let message = build.tr(
        "Куда направить {a} в этом месяце:\n{b}",
        "Where to put {a} this month:\n{b}",
    );
    build.node(
        "monthly",
        "trigger.schedule",
        0,
        0,
        json!({ "every": "month", "interval": "1", "day": day, "time": "10:00" }),
    );
    build.node("advice", "data.topup_advice", 0, 2, json!({}));
    build.node(
        "some",
        "logic.compare",
        1,
        2,
        json!({ "op": "gt", "b": "0" }),
    );
    build.node(
        "guard",
        "logic.guard",
        1,
        0,
        json!({ "reason": build.tr("Нечего распределять", "Nothing to split") }),
    );
    build.node("text", "logic.text", 2, 2, json!({ "template": message }));
    let title = build.tr("Пополнение по долям", "Top-up by shares");
    build.notify("tell", 2, 0, &title);
    build.wire("advice", "total", "some", "a");
    build.wire("monthly", "out", "guard", "in");
    build.wire("some", "result", "guard", "condition");
    build.wire("advice", "total", "text", "a");
    build.wire("advice", "text", "text", "b");
    build.wire("guard", "out", "tell", "in");
    build.wire("text", "text", "tell", "text");
    (
        title,
        build.tr(
            "Раз в месяц подсказывает, как разложить накопленное по корзинам, чтобы доли шли к целевым.",
            "Once a month suggests how to split the savings over the baskets so the shares move to their targets.",
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::workflow::{Graph, node_def, validate};

    fn filled(id: &str) -> BTreeMap<String, String> {
        let defaults = TemplateDefaults {
            savings_pct: "10".into(),
            currency: "rub".into(),
        };
        let info = template_list(&defaults)
            .into_iter()
            .find(|info| info.id == id)
            .unwrap();
        info.fields
            .iter()
            .map(|field| {
                let value = if field.default.is_empty() {
                    format!("picked-{}", field.key)
                } else {
                    field.default.clone()
                };
                (field.key.clone(), value)
            })
            .collect()
    }

    fn built(id: &str) -> Graph {
        let built = build_template(id, &filled(id), "ru").unwrap();
        serde_json::from_str(&built.graph).unwrap()
    }

    #[test]
    fn every_template_builds_a_valid_graph() {
        for id in TEMPLATE_IDS {
            let graph = built(id);
            let errors: Vec<_> = validate(&graph)
                .into_iter()
                .filter(|issue| issue.level == crate::workflow::IssueLevel::Error)
                .collect();
            assert!(errors.is_empty(), "{id}: {errors:?}");
            for node in &graph.nodes {
                assert!(node_def(&node.kind).is_some(), "{id}: {}", node.kind);
            }
        }
        assert_eq!(
            template_list(&TemplateDefaults {
                savings_pct: "10".into(),
                currency: "rub".into(),
            })
            .len(),
            TEMPLATE_IDS.len()
        );
    }

    #[test]
    fn money_moves_only_after_a_yes() {
        for id in TEMPLATE_IDS {
            let graph = built(id);
            let mut reached: BTreeSet<String> = graph
                .nodes
                .iter()
                .filter(|node| node.kind.starts_with("trigger."))
                .map(|node| node.id.clone())
                .collect();
            let mut queue: Vec<String> = reached.iter().cloned().collect();
            while let Some(current) = queue.pop() {
                let kind = &graph.node(&current).unwrap().kind;
                for edge in graph.edges.iter().filter(|edge| edge.from == current) {
                    let flow = node_def(kind)
                        .and_then(|def| def.output(&edge.from_port))
                        .is_some_and(|port| port.ty == crate::workflow::PortType::Flow);
                    let past_yes = kind == "action.confirm" && edge.from_port == "true";
                    if flow && !past_yes && reached.insert(edge.to.clone()) {
                        queue.push(edge.to.clone());
                    }
                }
            }
            for node in &graph.nodes {
                if matches!(
                    node.kind.as_str(),
                    "action.transaction" | "action.distribute"
                ) {
                    assert!(
                        !reached.contains(&node.id),
                        "{id}: {} skips the question",
                        node.id
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_template_is_none() {
        assert!(build_template("nope", &BTreeMap::new(), "ru").is_none());
    }
}
