//! The values a run reads: the workflow variables, and the lookups the nodes
//! make in the vault (an asset by symbol, a balance, a price, what a period
//! brought in or took out). Pure functions over the document or the read
//! context; nothing here writes.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;
use crate::money::books::Books;
use crate::workflow::model::{VarScope, WorkflowVar};
use crate::workflow::types::WfValue;

use super::runs::made_by;

/// A stored variable value (a `WfValue` in JSON); an unreadable one is empty.
pub fn value_of(json: &str) -> WfValue {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn var(doc: &VaultDoc, scope: &VarScope, name: &str) -> Option<WfValue> {
    doc.workflow_vars
        .iter()
        .find(|stored| stored.scope == *scope && stored.name == name)
        .map(|stored| value_of(&stored.value))
}

pub fn variables(doc: &VaultDoc) -> Vec<WorkflowVar> {
    doc.workflow_vars.clone()
}

/// Set a variable: replace the one with this scope and name, else add it.
pub fn set_var(
    doc: &mut VaultDoc,
    scope: VarScope,
    name: &str,
    value: &WfValue,
    updated_at: String,
) {
    let json = serde_json::to_string(value).unwrap_or_default();
    match doc
        .workflow_vars
        .iter_mut()
        .find(|stored| stored.scope == scope && stored.name == name)
    {
        Some(stored) => {
            stored.value = json;
            stored.updated_at = updated_at;
        }
        None => doc.workflow_vars.push(WorkflowVar {
            scope,
            name: name.to_string(),
            value: json,
            updated_at,
        }),
    }
}

pub fn remove_var(doc: &mut VaultDoc, scope: &VarScope, name: &str) {
    doc.workflow_vars
        .retain(|stored| !(stored.scope == *scope && stored.name == name));
}

/// The name and value a user typed for a variable: the name trimmed and not
/// empty, the value a `WfValue` in JSON.
pub fn parse_input(name: &str, value: &str) -> Result<(String, WfValue)> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::InvalidInput("name the variable".into()));
    }
    let parsed: WfValue = serde_json::from_str(value)
        .map_err(|err| Error::InvalidInput(format!("variable value: {err}")))?;
    Ok((name.to_string(), parsed))
}

// -- lookups for the executor ------------------------------------------------

/// An asset by its symbol: one the vault holds, else a currency.
pub fn asset(doc: &VaultDoc, symbol: &str) -> AssetRef {
    let symbol = symbol.trim();
    doc.holdings
        .iter()
        .map(|holding| &holding.asset)
        .find(|asset| asset.symbol.eq_ignore_ascii_case(symbol) && !asset.is_derivative())
        .cloned()
        .unwrap_or_else(|| AssetRef::fiat(symbol, &symbol.to_uppercase()))
}

/// Price of one unit of an asset in a currency.
pub fn price(books: &Books<'_>, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount> {
    let book = books.rate_book();
    let usd = book.price_at(&asset.key(), secs)?;
    book.convert(usd, "usd", currency, secs)
}

pub fn convert(
    books: &Books<'_>,
    amount: Amount,
    from: &str,
    to: &str,
    secs: i64,
) -> Option<Amount> {
    books.rate_book().convert(amount, from, to, secs)
}

/// The average amount of the latest `count` operations a workflow made (or the
/// automation it was before, which kept its id), leaving out the ones still
/// booked with an approximate amount; with the asset symbol.
pub fn recent_average(doc: &VaultDoc, workflow_id: &str, count: usize) -> Option<(Amount, String)> {
    let mut made: Vec<&Transaction> = doc
        .transactions
        .iter()
        .filter(|tx| !tx.meta().origin.is_estimated() && made_by(tx, workflow_id))
        .collect();
    made.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    made.truncate(count.max(1));
    let first = made.first()?;
    let (_, asset, _) = crate::workflow::nodes::triggers::tx_main(first);
    let symbol = match asset {
        WfValue::Asset(asset) => asset.symbol,
        _ => String::new(),
    };
    let total = made.iter().fold(Amount::ZERO, |sum, tx| {
        sum + crate::workflow::nodes::triggers::tx_main(tx)
            .0
            .as_number()
            .unwrap_or(Amount::ZERO)
    });
    let len = Amount::new(rust_decimal::Decimal::from(made.len()));
    Some(((total / len).round(2), symbol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_variable_is_replaced_by_scope_and_name() {
        let mut doc = VaultDoc::default();
        let scope = VarScope::Global;
        set_var(
            &mut doc,
            scope.clone(),
            "count",
            &WfValue::Text("1".into()),
            "t1".into(),
        );
        set_var(
            &mut doc,
            scope.clone(),
            "count",
            &WfValue::Text("2".into()),
            "t2".into(),
        );
        set_var(
            &mut doc,
            VarScope::Workflow("wf".into()),
            "count",
            &WfValue::Text("own".into()),
            "t3".into(),
        );
        assert_eq!(doc.workflow_vars.len(), 2);
        assert_eq!(var(&doc, &scope, "count").unwrap().as_text(), "2");
        remove_var(&mut doc, &scope, "count");
        assert!(var(&doc, &scope, "count").is_none());
        assert_eq!(variables(&doc).len(), 1);
    }

    #[test]
    fn an_unnamed_variable_or_a_value_that_is_not_json_is_refused() {
        assert!(matches!(
            parse_input("  ", "1"),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            parse_input("count", "not json"),
            Err(Error::InvalidInput(_))
        ));
        let json = serde_json::to_string(&WfValue::Text("x".into())).unwrap();
        assert_eq!(parse_input(" count ", &json).unwrap().0, "count");
    }

    #[test]
    fn an_unknown_symbol_is_a_currency() {
        let asset = asset(&VaultDoc::default(), " usd ");
        assert!(asset.is_fiat());
        assert_eq!(asset.symbol, "USD");
    }
}
