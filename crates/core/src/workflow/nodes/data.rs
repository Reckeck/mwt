//! Data nodes: balances, prices, rates, sums, budgets, goals, the date.

use chrono::Datelike;

use crate::Amount;
use crate::calendar::Calendar;
pub(crate) use crate::calendar::workdays::{WorkdayRules, parse_date, parse_dates};
use crate::model::transaction::AccountRef;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::{Outputs, json_number, single};

fn number(value: Amount) -> WfValue {
    WfValue::Number(value)
}

fn percent(ratio: Amount) -> Amount {
    (ratio * Amount::new(rust_decimal::Decimal::ONE_HUNDRED)).round(2)
}

impl Executor<'_> {
    pub(crate) fn eval_data(&mut self, node: &Node) -> Result<Outputs, String> {
        let now = self.now();
        match node.kind.as_str() {
            "data.balance" => {
                let account = self.account_input(node, "account")?;
                let Some(AccountRef::Store(store_id)) = account else {
                    return Err("choose one of your accounts".into());
                };
                let asset = self.asset_input(node, "asset")?;
                Ok(single(
                    "value",
                    self.vault.wf_balance(&store_id, asset.as_ref())?,
                ))
            }
            "data.price" => {
                let asset = self.asset_input(node, "asset")?.ok_or("choose an asset")?;
                let currency = self
                    .currency_input(node, "currency")?
                    .unwrap_or_else(|| "usd".into());
                let price = self.vault.wf_price(&asset, &currency, now).ok_or_else(|| {
                    format!(
                        "no price of {} in {}",
                        asset.symbol,
                        currency.to_uppercase()
                    )
                })?;
                Ok(single("price", WfValue::money(price, &currency)))
            }
            "data.fx" => {
                let from = self
                    .currency_input(node, "from")?
                    .ok_or("choose the currency to convert from")?;
                let to = self
                    .currency_input(node, "to")?
                    .ok_or("choose the currency to convert to")?;
                let rate = self
                    .vault
                    .wf_convert(Amount::new(rust_decimal::Decimal::ONE), &from, &to, now)
                    .ok_or_else(|| {
                        format!("no rate {}→{}", from.to_uppercase(), to.to_uppercase())
                    })?;
                Ok(single("rate", number(rate)))
            }
            "data.constant" => {
                let kind = self.text_param(node, "value_type");
                let value = match kind.as_str() {
                    "money" => {
                        let amount =
                            json_number(&self.raw_param(node, "number")).unwrap_or(Amount::ZERO);
                        WfValue::money(amount, &self.text_param(node, "currency"))
                    }
                    "text" => self.param(node, "text"),
                    "bool" => self.param(node, "bool"),
                    "account" => self.param(node, "account"),
                    "asset" => self.param(node, "asset"),
                    "category" => self.param(node, "category"),
                    _ => self.param(node, "number"),
                };
                Ok(single("value", value))
            }
            "data.sum" => {
                let period = self.text_param(node, "period");
                let flow = self.text_param(node, "flow");
                let category = self.text_param(node, "category");
                let category = (!category.trim().is_empty()).then_some(category);
                let total =
                    self.vault
                        .wf_sum(&period, &flow, category.as_deref(), now, &self.ctx.zone);
                Ok(single(
                    "total",
                    WfValue::money(total, self.vault.base_currency()),
                ))
            }
            "data.recent_average" => {
                let count = self
                    .text_param(node, "count")
                    .trim()
                    .parse::<usize>()
                    .unwrap_or(3)
                    .clamp(1, 24);
                match self.vault.wf_recent_average(&self.ctx.workflow_id, count) {
                    Some((average, symbol)) => {
                        Ok(single("amount", WfValue::money(average, &symbol)))
                    }
                    None => Ok(single("amount", self.input(node, "fallback")?)),
                }
            }
            "data.budget" => {
                let id = self.text_param(node, "budget");
                let status = self
                    .vault
                    .budget_status(now, &self.ctx.zone)
                    .into_iter()
                    .find(|status| status.budget.id == id)
                    .ok_or("choose an active limit")?;
                Ok(Outputs::from([
                    (
                        "remaining",
                        WfValue::money(status.remaining, &status.budget.currency),
                    ),
                    ("used", number(percent(status.used))),
                ]))
            }
            "data.goal" => {
                let id = self.text_param(node, "goal");
                let progress = self
                    .vault
                    .goal_progress(now, &self.ctx.zone)
                    .into_iter()
                    .find(|progress| progress.goal.id == id)
                    .ok_or("choose an active goal")?;
                Ok(goal_outputs(&progress))
            }
            "data.now" => {
                let date = Calendar::new(&self.ctx.zone).date_of(now);
                Ok(Outputs::from([
                    ("now", WfValue::Date(crate::calendar::iso(now))),
                    ("day", number(Amount::new(date.day().into()))),
                    (
                        "weekday",
                        number(Amount::new(date.weekday().num_days_from_monday().into())),
                    ),
                ]))
            }
            "data.workday" => {
                let cal = Calendar::new(&self.ctx.zone);
                let date = match self.input(node, "date")? {
                    WfValue::Date(text) | WfValue::Text(text) if !text.trim().is_empty() => {
                        parse_date(&text, &cal).ok_or_else(|| format!("«{text}» is not a date"))?
                    }
                    _ => cal.date_of(now),
                };
                let rules = WorkdayRules {
                    holidays: self.text_param(node, "calendar") != "weekends",
                    weekend: self.text_param(node, "weekend"),
                    extra_off: parse_dates(&self.text_param(node, "extra_off"), &cal),
                    extra_work: parse_dates(&self.text_param(node, "extra_work"), &cal),
                };
                let day = rules.classify(date);
                Ok(Outputs::from([
                    ("workday", WfValue::Bool(day.workday)),
                    ("weekend", WfValue::Bool(day.weekend)),
                    ("holiday", WfValue::Bool(day.holiday)),
                    (
                        "next_workday",
                        WfValue::Date(rules.next_workday(date).to_string()),
                    ),
                ]))
            }
            "data.tx_fields" => match self.input(node, "transaction")? {
                WfValue::Transaction(tx) => Ok(super::triggers::tx_fields(&tx)),
                WfValue::Null => Ok(Outputs::new()),
                other => Err(format!("not an operation: {}", other.as_text())),
            },
            other => Err(format!("unknown data node {other}")),
        }
    }
}

/// Saved, left and percent of a goal.
pub(crate) fn goal_outputs(progress: &crate::service::GoalProgress) -> Outputs {
    let currency = &progress.goal.currency;
    let left = (progress.goal.target_amount - progress.saved).max(Amount::ZERO);
    Outputs::from([
        ("saved", WfValue::money(progress.saved, currency)),
        ("left", WfValue::money(left, currency)),
        ("pct", number(progress.pct.round(2))),
    ])
}
