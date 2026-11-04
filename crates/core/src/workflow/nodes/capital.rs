//! Data nodes of the capital plan: emergency fund gap, key rate, inflation,
//! recurring payments and the top-up advice.

use std::collections::BTreeMap;

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::model::transaction::AccountRef;
use crate::pricing::history::unix_secs;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::{Outputs, single};

/// How far through `start..end` the moment `now` is, percent, 0..100.
pub(crate) fn elapsed_pct(start: &str, end: &str, now: i64) -> Option<Amount> {
    let (start, end) = (unix_secs(start)?, unix_secs(end)?);
    if end <= start {
        return None;
    }
    let passed = (now - start).clamp(0, end - start);
    let ratio = Amount::new(passed.into()) / Amount::new((end - start).into());
    Some((ratio * Amount::new(rust_decimal::Decimal::ONE_HUNDRED)).round(2))
}

impl Executor<'_> {
    pub(crate) fn eval_capital(&mut self, node: &Node) -> Result<Outputs, String> {
        let now = self.now();
        let today = Calendar::new(&self.ctx.zone).date_of(now);
        match node.kind.as_str() {
            "data.emergency_gap" => self.emergency_gap(node, now),
            "data.key_rate" => Ok(single(
                "rate",
                self.vault
                    .key_rate_on(today)
                    .map_or(WfValue::Null, WfValue::Number),
            )),
            "data.inflation" => {
                let currency = self
                    .currency_input(node, "currency")?
                    .unwrap_or_else(|| "rub".into());
                let month = calendar::month_key(today);
                let value = self.vault.inflation_on(&currency, &month);
                let last = self
                    .vault
                    .inflation_series(&currency)
                    .into_iter()
                    .filter(|reading| reading.month <= month)
                    .map(|reading| reading.month)
                    .max();
                Ok(Outputs::from([
                    ("value", value.map_or(WfValue::Null, WfValue::Number)),
                    ("month", last.map_or(WfValue::Null, WfValue::Text)),
                ]))
            }
            "data.recurring" => match self.input(node, "transaction")? {
                WfValue::Transaction(tx) => {
                    let check = self.vault.recurring_check(&tx);
                    let currency = super::triggers::tx_fields(&tx)
                        .get("amount")
                        .and_then(|value| match value {
                            WfValue::Money { currency, .. } => Some(currency.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    Ok(Outputs::from([
                        ("new", WfValue::Bool(check.new)),
                        ("grew", WfValue::Bool(check.grew)),
                        (
                            "previous",
                            check
                                .previous
                                .map_or(WfValue::Null, |amount| WfValue::money(amount, &currency)),
                        ),
                        (
                            "change_pct",
                            check.change_pct.map_or(WfValue::Null, WfValue::Number),
                        ),
                    ]))
                }
                WfValue::Null => Ok(Outputs::new()),
                other => Err(format!("not an operation: {}", other.as_text())),
            },
            "data.topup_advice" => {
                let given = self
                    .number_input(node, "amount")?
                    .filter(|amount| !amount.is_zero());
                let advice = self.vault.topup_advice(given, now, &self.ctx.zone);
                let code = advice.currency.to_uppercase();
                let text = advice
                    .rows
                    .iter()
                    .filter(|row| row.add > Amount::ZERO)
                    .map(|row| format!("{}: {} {code}", row.class.name, row.add.round(2)))
                    .collect::<Vec<_>>()
                    .join("\n");
                let list = advice
                    .rows
                    .iter()
                    .map(|row| {
                        WfValue::Record(BTreeMap::from([
                            ("name".to_string(), WfValue::Text(row.class.name.clone())),
                            ("add".to_string(), WfValue::money(row.add, &advice.currency)),
                            ("capped".to_string(), WfValue::Bool(row.capped)),
                        ]))
                    })
                    .collect();
                Ok(Outputs::from([
                    ("text", WfValue::Text(text)),
                    ("total", WfValue::money(advice.amount, &advice.currency)),
                    ("list", WfValue::List(list)),
                ]))
            }
            other => Err(format!("unknown data node {other}")),
        }
    }

    fn emergency_gap(&mut self, node: &Node, now: i64) -> Result<Outputs, String> {
        let (amount, currency) = match self.input(node, "amount")? {
            WfValue::Money { amount, currency } => (amount, currency),
            WfValue::Number(amount) => (amount, String::new()),
            WfValue::Null => return Err("connect an amount".into()),
            other => return Err(format!("not an amount: {}", other.as_text())),
        };
        let amount = amount.max(Amount::ZERO);
        let status = self.vault.emergency_fund_status(now, &self.ctx.zone);
        let currency = if currency.is_empty() {
            status.currency.to_lowercase()
        } else {
            currency
        };
        let gap = status
            .target_amount
            .map(|target| (target - status.total).max(Amount::ZERO))
            .and_then(|gap| {
                if gap.is_zero() {
                    Some(gap)
                } else {
                    self.vault.wf_convert(gap, &status.currency, &currency, now)
                }
            })
            .unwrap_or(Amount::ZERO)
            .round(2);
        let to_cushion = amount.min(gap);
        let store = status.stores.first().map_or(WfValue::Null, |store| {
            WfValue::Account(AccountRef::Store(store.store_id.clone()))
        });
        Ok(Outputs::from([
            ("to_cushion", WfValue::money(to_cushion, &currency)),
            ("rest", WfValue::money(amount - to_cushion, &currency)),
            ("gap", WfValue::money(gap, &currency)),
            ("store", store),
        ]))
    }
}
