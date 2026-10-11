//! Conditions, math, text and variables.

use rust_decimal::Decimal;

use crate::Amount;
use crate::workflow::catalog::NodeDef;
use crate::workflow::exec::{Executor, Fault, Halt, Step, StepStatus};
use crate::workflow::graph::Node;
use crate::workflow::model::VarScope;
use crate::workflow::types::WfValue;

use super::{Outputs, json_number, single};

fn money_currency(value: &WfValue) -> Option<String> {
    match value {
        WfValue::Money { currency, .. } => Some(currency.clone()),
        _ => None,
    }
}

fn number_of(value: &WfValue, side: &str) -> Result<Decimal, String> {
    value
        .as_number()
        .map(Amount::into_inner)
        .ok_or_else(|| format!("{side} is not a number: «{}»", value.as_text()))
}

/// The currency both sides share; two different currencies are an error.
fn shared_currency(left: &WfValue, right: &WfValue) -> Result<Option<String>, String> {
    match (money_currency(left), money_currency(right)) {
        (Some(one), Some(other)) if one != other => Err(format!(
            "{} and {} are different currencies: convert one first",
            one.to_uppercase(),
            other.to_uppercase()
        )),
        (one, other) => Ok(one.or(other)),
    }
}

fn overflow() -> String {
    "the number is too large".into()
}

/// `A op B` keeping an amount's currency.
pub(crate) fn arithmetic(
    op: &str,
    left: &WfValue,
    right: &WfValue,
    places: u32,
) -> Result<WfValue, String> {
    let both_money = money_currency(left).is_some() && money_currency(right).is_some();
    let currency = shared_currency(left, right)?;
    let (one, other) = (number_of(left, "A")?, number_of(right, "B")?);
    let result = match op {
        "add" => one.checked_add(other).ok_or_else(overflow)?,
        "sub" => one.checked_sub(other).ok_or_else(overflow)?,
        "mul" => {
            if both_money {
                return Err("two amounts cannot be multiplied".into());
            }
            one.checked_mul(other).ok_or_else(overflow)?
        }
        "div" => {
            if other.is_zero() {
                return Err("division by zero".into());
            }
            let quotient = one.checked_div(other).ok_or_else(overflow)?;
            if both_money {
                return Ok(WfValue::Number(Amount::new(quotient).round(places)));
            }
            quotient
        }
        "percent" => one
            .checked_mul(other)
            .and_then(|product| product.checked_div(Decimal::ONE_HUNDRED))
            .ok_or_else(overflow)?,
        "min" => one.min(other),
        "max" => one.max(other),
        other_op => return Err(format!("unknown operation {other_op}")),
    };
    let result = Amount::new(result).round(places);
    Ok(match currency {
        Some(code) => WfValue::money(result, &code),
        None => WfValue::Number(result),
    })
}

/// `A op B` as yes or no.
pub(crate) fn compare(op: &str, left: &WfValue, right: &WfValue) -> Result<bool, String> {
    if op == "contains" {
        return Ok(left
            .as_text()
            .to_lowercase()
            .contains(&right.as_text().to_lowercase()));
    }
    if let (Some(one), Some(other)) = (left.as_number(), right.as_number()) {
        shared_currency(left, right)?;
        return Ok(match op {
            "gt" => one > other,
            "gte" => one >= other,
            "lt" => one < other,
            "lte" => one <= other,
            "eq" => one == other,
            "ne" => one != other,
            other_op => return Err(format!("unknown comparison {other_op}")),
        });
    }
    match op {
        "eq" => Ok(left.as_text() == right.as_text()),
        "ne" => Ok(left.as_text() != right.as_text()),
        _ => Err(format!(
            "«{}» and «{}» are not numbers to compare",
            left.as_text(),
            right.as_text()
        )),
    }
}

fn truth(value: &WfValue) -> bool {
    value.as_bool().unwrap_or(true)
}

fn yes_no(value: bool) -> (&'static str, &'static str) {
    if value {
        ("да", "yes")
    } else {
        ("нет", "no")
    }
}

impl Executor<'_> {
    pub(crate) fn eval_logic(&mut self, node: &Node) -> Result<Outputs, String> {
        match node.kind.as_str() {
            "logic.compare" => {
                let left = self.input(node, "a")?;
                let right = self.input(node, "b")?;
                let op = self.text_param(node, "op");
                Ok(single(
                    "result",
                    WfValue::Bool(compare(&op, &left, &right)?),
                ))
            }
            "logic.and" | "logic.or" => {
                let left = truth(&self.input(node, "a")?);
                let right = truth(&self.input(node, "b")?);
                let result = if node.kind == "logic.and" {
                    left && right
                } else {
                    left || right
                };
                Ok(single("result", WfValue::Bool(result)))
            }
            "logic.not" => Ok(single(
                "result",
                WfValue::Bool(!truth(&self.input(node, "a")?)),
            )),
            "logic.math" => {
                let left = self.input(node, "a")?;
                let right = self.input(node, "b")?;
                let op = self.text_param(node, "op");
                let places = json_number(&self.raw_param(node, "round"))
                    .and_then(|places| places.into_inner().trunc().to_string().parse::<u32>().ok())
                    .unwrap_or(2)
                    .min(18);
                Ok(single("result", arithmetic(&op, &left, &right, places)?))
            }
            "logic.convert" => {
                let amount = self.input(node, "amount")?;
                let to = self
                    .currency_input(node, "currency")?
                    .ok_or("choose the currency")?;
                let Some(from) = money_currency(&amount) else {
                    return Err(format!(
                        "«{}» is not an amount with a currency",
                        amount.as_text()
                    ));
                };
                let value = amount.as_number().unwrap_or(Amount::ZERO);
                let converted = self
                    .vault
                    .wf_convert(value, &from, &to, self.ctx.now)
                    .ok_or_else(|| {
                        format!("no rate {}→{}", from.to_uppercase(), to.to_uppercase())
                    })?;
                let places = if self.vault.wf_asset(&to).is_fiat() {
                    2
                } else {
                    8
                };
                Ok(single(
                    "result",
                    WfValue::money(converted.round(places), &to),
                ))
            }
            "logic.text" => {
                let mut text = self.text_param(node, "template");
                for slot in ["a", "b", "c"] {
                    let pattern = format!("{{{slot}}}");
                    if text.contains(&pattern) {
                        let value = self.input(node, slot)?.as_text();
                        text = text.replace(&pattern, &value);
                    }
                }
                Ok(single("text", WfValue::Text(text)))
            }
            other => Err(format!("unknown node {other}")),
        }
    }

    pub(crate) fn run_logic(&mut self, node: &Node, def: &NodeDef) -> Result<Step, Fault> {
        match node.kind.as_str() {
            "logic.branch" => {
                let condition = truth(&self.input(node, "condition")?);
                let (ru, en) = yes_no(condition);
                Ok(Step::next(
                    if condition { "true" } else { "false" },
                    format!("«{}»: условие — {ru}", def.title.ru),
                    format!("«{}»: the condition is {en}", def.title.en),
                ))
            }
            "logic.guard" => {
                if truth(&self.input(node, "condition")?) {
                    return Ok(Step::next(
                        "out",
                        format!("«{}»: условие выполнено", def.title.ru),
                        format!("«{}»: the condition holds", def.title.en),
                    ));
                }
                let reason = self.text_param(node, "reason");
                let reason = if reason.trim().is_empty() {
                    "the guard's condition is not met".to_string()
                } else {
                    reason
                };
                Err(Halt::Stop(reason).into())
            }
            "logic.throttle" => {
                let hours = json_number(&self.raw_param(node, "hours")).unwrap_or(Amount::ZERO);
                let gap = (hours.to_f64() * 3600.0) as i64;
                let key = format!("throttle:{}", node.id);
                let last = self
                    .vault
                    .wf_mark(&self.ctx.workflow_id, &key)
                    .and_then(|mark| crate::pricing::history::unix_secs(&mark));
                if let Some(last) = last
                    && self.ctx.now - last < gap
                {
                    return Ok(Step::next(
                        "skipped",
                        format!(
                            "«{}»: с прошлого раза прошло меньше {hours} ч — пропуск",
                            def.title.ru
                        ),
                        format!(
                            "«{}»: less than {hours} h since last time: skipped",
                            def.title.en
                        ),
                    )
                    .with_status(StepStatus::Skipped));
                }
                let now = crate::calendar::iso(self.ctx.now);
                self.vault.wf_set_mark(&self.ctx.workflow_id, &key, &now);
                Ok(Step::next(
                    "out",
                    format!("«{}»: пропускаю дальше", def.title.ru),
                    format!("«{}»: letting through", def.title.en),
                ))
            }
            other => Err(Fault::Error(format!("unknown node {other}"))),
        }
    }

    fn var_scope(&self, node: &Node) -> VarScope {
        if self.text_param(node, "scope") == "global" {
            VarScope::Global
        } else {
            VarScope::Workflow(self.ctx.workflow_id.clone())
        }
    }

    fn var_name(&self, node: &Node) -> Result<String, String> {
        let name = self.text_param(node, "name").trim().to_string();
        if name.is_empty() {
            return Err("name the variable".into());
        }
        Ok(name)
    }

    pub(crate) fn eval_var(&mut self, node: &Node) -> Result<Outputs, String> {
        let name = self.var_name(node)?;
        let value = self
            .vault
            .wf_var(&self.var_scope(node), &name)
            .unwrap_or_default();
        Ok(single("value", value))
    }

    pub(crate) fn run_var(&mut self, node: &Node, def: &NodeDef) -> Result<Step, Fault> {
        let name = self.var_name(node)?;
        let scope = self.var_scope(node);
        let value = if node.kind == "var.add" {
            let delta = if self.wired(node, "delta") {
                self.input(node, "delta")?
            } else {
                self.param(node, "delta")
            };
            let current = self.vault.wf_var(&scope, &name).unwrap_or_default();
            let current = if current.is_null() {
                WfValue::Number(Amount::ZERO)
            } else {
                current
            };
            let sum = arithmetic("add", &current, &delta, 8)?;
            self.set_output(node, "value", sum.clone());
            sum
        } else {
            self.input(node, "value")?
        };
        self.vault.wf_set_var(scope, &name, &value);
        Ok(Step::next(
            "out",
            format!("«{}»: {name} = {}", def.title.ru, value.as_text()),
            format!("«{}»: {name} = {}", def.title.en, value.as_text()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(value: &str, currency: &str) -> WfValue {
        WfValue::money(Amount::parse(value).unwrap(), currency)
    }

    fn num(value: &str) -> WfValue {
        WfValue::Number(Amount::parse(value).unwrap())
    }

    #[test]
    fn math_keeps_the_currency_and_refuses_mixing() {
        assert_eq!(
            arithmetic("percent", &money("50000", "rub"), &num("10"), 2).unwrap(),
            money("5000", "rub")
        );
        assert_eq!(
            arithmetic("add", &money("1", "rub"), &money("2", "rub"), 2).unwrap(),
            money("3", "rub")
        );
        assert!(arithmetic("add", &money("1", "rub"), &money("2", "usd"), 2).is_err());
        assert!(arithmetic("div", &num("1"), &num("0"), 2).is_err());
        assert_eq!(
            arithmetic("div", &money("10", "rub"), &money("4", "rub"), 2).unwrap(),
            num("2.5")
        );
        assert_eq!(
            arithmetic("div", &num("10"), &num("3"), 2).unwrap(),
            num("3.33")
        );
    }

    #[test]
    fn comparisons_on_numbers_and_text() {
        assert!(compare("gte", &money("5000", "rub"), &num("5000")).unwrap());
        assert!(compare("lt", &money("1", "rub"), &money("1", "usd")).is_err());
        assert!(
            compare(
                "contains",
                &WfValue::Text("Зарплата за май".into()),
                &WfValue::Text("зарплата".into())
            )
            .unwrap()
        );
        assert!(compare("eq", &WfValue::Text("a".into()), &WfValue::Text("a".into())).unwrap());
        assert!(compare("gt", &WfValue::Text("a".into()), &WfValue::Text("b".into())).is_err());
    }
}
