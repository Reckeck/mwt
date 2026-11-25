//! The scenarios of the transactions area: an operation added, saved in a
//! group, edited or deleted. What an operation is filled in with comes from
//! `money::transactions`; here the rules of other areas are called, the
//! document is changed through one `run`, and the holdings are rebuilt and
//! committed once at the end (`rebase_and_commit`).

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::store::StoreScope;
use crate::model::transaction::Transaction;
use crate::model::vault::{iso_to_millis, now_iso};
use crate::money::distribution::{income_ref_reserved, linked_to_income};
use crate::money::transactions::{
    TransactionPatch, complete_transaction, ensure_fresh_id, patched, replacing,
};
use crate::service::Session;
use crate::usecases::household::property_links::{check_property_link, set_property_link};
use crate::usecases::household::shared_events::cascade_shared_delete;
use crate::usecases::money::portfolio::recompute_holdings;
use crate::usecases::planning::distribution::{
    apply_distribution, apply_distribution_unsaved, drop_distribution_of, rows_of,
};
use mwt_model::{AccountRef, DistributionRow, PeriodRule};
use mwt_planning::period_rule::{SpreadBefore, resolve_period_for_new, settle_spread};

use super::stores::rebase_and_commit;

/// Transaction history, newest first, limited to a scope.
pub fn transactions(vault: &Session, scope: &StoreScope) -> Vec<Transaction> {
    let ids =
        (!scope.store_ids.is_empty()).then(|| crate::money::tree::resolve_scope(&vault.doc, scope));
    let mut out: Vec<Transaction> = match ids {
        Some(ids) => crate::money::portfolio::scoped_txs(&vault.doc, &ids),
        None => vault.doc.transactions.clone(),
    };
    out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    out
}

/// Fill what the user does not type, at the moment a write happens.
pub fn prepare_transaction(vault: &Session, tx: &mut Transaction) -> Result<()> {
    prepare_transaction_at(vault, tx, iso_to_millis(&now_iso()))
}

/// Everything a write does before storing: categories normalized and
/// auto-filled, property prices completed, unpriced sides stamped, every
/// rule checked. The future-date rule (V11) is measured from `now_ms` — the
/// scheduler's clock for automation runs.
pub fn prepare_transaction_at(vault: &Session, tx: &mut Transaction, now_ms: i64) -> Result<()> {
    Session::normalize_categories(tx);
    crate::money::validate::apply_categorization_rules(&vault.doc, tx);
    complete_transaction(&vault.books(), tx);
    crate::money::adjustments::complete_adjustment(&vault.doc, tx)?;
    Session::stamp_prices(&crate::money::rates::rate_book(&vault.books()), tx);
    crate::usecases::household::validate::validate_transaction_at(vault, tx, now_ms)
}

/// Validate and append a transaction of any variant (plan §4.5, §8.1):
/// categories are normalized and auto-filled, then every rule is checked.
pub fn add_transaction(vault: &mut Session, tx: Transaction) -> Result<Transaction> {
    add_transaction_for(vault, tx, None)
}

/// [`add_transaction`], linked to a property (plan G4).
pub fn add_transaction_for(
    vault: &mut Session,
    tx: Transaction,
    property_id: Option<&str>,
) -> Result<Transaction> {
    add_transaction_ruled(vault, tx, property_id, &CreationRules::default())
}

/// What a new operation's period may come from besides its own input (spec D3).
#[derive(Debug, Clone, Copy, Default)]
pub struct CreationRules {
    /// The rule of the template the operation is made from.
    pub template_rule: Option<PeriodRule>,
    /// «Really none»: an operation without a period takes no rule.
    pub no_period_rule: bool,
}

/// [`add_transaction_for`] with the rules of the creation: a period the input does not carry
/// comes from the template's rule, else the rule of the main category, and is stored (D3, D4).
pub fn add_transaction_ruled(
    vault: &mut Session,
    tx: Transaction,
    property_id: Option<&str>,
    rules: &CreationRules,
) -> Result<Transaction> {
    add_transaction_distributed(vault, tx, property_id, rules, None)
}

/// [`add_transaction_ruled`] for an income whose distribution rows the caller resolved (a quick-add
/// from a template, T21): `rows` win over a list carried by `tx`; income and effects commit once.
pub(crate) fn add_transaction_distributed(
    vault: &mut Session,
    mut tx: Transaction,
    property_id: Option<&str>,
    rules: &CreationRules,
    resolved_rows: Option<Vec<DistributionRow>>,
) -> Result<Transaction> {
    refuse_forged_income_ref(&tx)?;
    apply_creation_rules(vault, &mut tx, rules);
    check_property_link(vault, property_id)?;
    let carried = take_distribution_rows(&mut tx);
    let rows = resolved_rows.or(carried);
    prepare_transaction(vault, &mut tx)?;
    ensure_fresh_id(&vault.doc, &mut tx);
    let backup = vault.doc.clone();
    vault.run(|run| {
        run.doc_mut().transactions.push(tx.clone());
        Ok(())
    })?;
    set_property_link(vault, &tx.meta().id, property_id);
    match rows {
        // The distribution commits once, with the income.
        Some(rows) => {
            if let Err(err) = apply_distribution(vault, &tx.meta().id, &rows) {
                vault.doc = backup;
                recompute_holdings(vault);
                return Err(err);
            }
            return stored_income(vault, &tx);
        }
        None => rebase_and_commit(vault)?,
    }
    Ok(tx)
}

/// What a new operation gets before it is prepared, on every creation path: categories settled
/// (the category rule of D3 reads them), the period by rule, the spread derived (P6).
pub(crate) fn apply_creation_rules(vault: &Session, tx: &mut Transaction, rules: &CreationRules) {
    apply_creation_rules_on(vault, tx, rules, None);
}

/// [`apply_creation_rules`] with the money date given when it is not the day of the timestamp:
/// an automation run made late still takes the period of the day it was planned for (D5).
pub(crate) fn apply_creation_rules_on(
    vault: &Session,
    tx: &mut Transaction,
    rules: &CreationRules,
    money_date: Option<chrono::NaiveDate>,
) {
    Session::normalize_categories(tx);
    crate::money::validate::apply_categorization_rules(&vault.doc, tx);
    let period_given = tx.meta().period.is_some();
    apply_period_rules(vault, tx, rules, money_date);
    settle_tx_spread(tx, period_given, None);
}

/// What a draft gets before a preview, the settling the save does: a new one takes the creation
/// rules (period by rule, spread derived); an edit of `replaced` settles against the stored
/// record, as [`update_transaction_for`] does. Writes nothing.
pub fn settle_for_preview(
    vault: &Session,
    tx: &mut Transaction,
    replaced: Option<&str>,
    no_period_rule: bool,
) {
    let stored = replaced.and_then(|id| {
        vault
            .doc
            .transactions
            .iter()
            .find(|stored| stored.meta().id == id)
    });
    match stored {
        Some(stored) => settle_tx_spread(tx, true, Some(stored)),
        None => {
            let rules = CreationRules {
                no_period_rule,
                ..CreationRules::default()
            };
            apply_creation_rules(vault, tx, &rules);
        }
    }
}

/// The amount a period spreads and the minor digits it is cut to; none for a kind with no period.
pub(crate) fn spreadable(tx: &Transaction) -> Option<(Amount, u32)> {
    match tx {
        Transaction::Topup(inner) => Some((inner.amount, inner.asset.minor_places())),
        Transaction::Payment(inner) => Some((inner.amount, inner.asset.minor_places())),
        _ => None,
    }
}

/// D3: give a new income or expense without a period the period of its rule, cut in the zone of
/// the group's schedule from the local date of its timestamp (or the given money date).
fn apply_period_rules(
    vault: &Session,
    tx: &mut Transaction,
    rules: &CreationRules,
    money_date: Option<chrono::NaiveDate>,
) {
    if spreadable(tx).is_none() || tx.meta().period.is_some() {
        return;
    }
    let Some(money_date) = money_date.or_else(|| {
        crate::money::interest::calendar_of(&vault.doc).date_of_iso(&tx.meta().timestamp)
    }) else {
        return;
    };
    let category_rule = tx.meta().categories.first().and_then(|key| {
        vault
            .doc
            .categories
            .iter()
            .find(|category| &category.key == key)
            .and_then(|category| category.period_rule)
    });
    tx.meta_mut().period = resolve_period_for_new(
        None,
        rules.no_period_rule,
        rules.template_rule,
        category_rule,
        money_date,
    );
}

/// P6, P8: keep the stored shares in step with the amount and the period; `before` is the
/// record as stored when this is an edit; `period_given` is false when a rule gave the period.
pub(crate) fn settle_tx_spread(
    tx: &mut Transaction,
    period_given: bool,
    before: Option<&Transaction>,
) {
    let Some((amount, places)) = spreadable(tx) else {
        return;
    };
    let stored = before.and_then(|old| {
        spreadable(old).map(|(old_amount, _)| SpreadBefore {
            amount: old_amount,
            period: old.meta().period.as_ref(),
            spread: old.meta().spread.as_ref(),
        })
    });
    let meta = tx.meta_mut();
    meta.spread = settle_spread(
        meta.period.as_ref(),
        meta.spread.take(),
        amount,
        places,
        period_given,
        stored.as_ref(),
    );
}

/// A record with an `income_ref` cannot be deleted on its own.
pub(crate) fn refuse_if_linked(tx: &Transaction) -> Result<()> {
    if tx.meta().income_ref.is_some() {
        return Err(linked_to_income("this operation"));
    }
    Ok(())
}

/// What V2 reads of an effect transfer: where it goes, what and how much.
fn effect_shape(tx: &Transaction) -> Option<(&AccountRef, &AccountRef, String, Amount)> {
    match tx {
        Transaction::Transfer(transfer) => Some((
            &transfer.from,
            &transfer.to,
            transfer.asset.key(),
            transfer.amount,
        )),
        _ => None,
    }
}

/// The edit of a record as it is stored: the links the form has no field for stay (the income
/// link, the distribution); an effect keeps what V2 reads, comment, categories and date stay open.
fn keep_income_links(stored: &Transaction, tx: &mut Transaction) -> Result<()> {
    if stored.meta().income_ref.is_some() && effect_shape(stored) != effect_shape(tx) {
        return Err(linked_to_income(
            "the amount, accounts and asset of this operation",
        ));
    }
    tx.meta_mut().income_ref = stored.meta().income_ref.clone();
    match (stored, tx) {
        (Transaction::Topup(old), Transaction::Topup(new)) => {
            new.distribution = old.distribution.clone()
        }
        (Transaction::Topup(old), _) if old.distribution.is_some() => {
            return Err(linked_to_income("an income with a distribution"));
        }
        (_, Transaction::Topup(new)) if new.distribution.is_some() => {
            return Err(linked_to_income("an operation that is not an income"));
        }
        _ => {}
    }
    Ok(())
}

/// The rows to re-resolve when the edit moves what the effects hang on (amount, store, asset,
/// timestamp); the distribution is taken off the edited income, the usecase writes it again.
fn rows_for_edited_income(
    stored: &Transaction,
    tx: &mut Transaction,
) -> Option<Vec<DistributionRow>> {
    let (Transaction::Topup(old), Transaction::Topup(new)) = (stored, tx) else {
        return None;
    };
    let parts = old.distribution.as_ref()?;
    let same = old.amount == new.amount
        && old.store_id == new.store_id
        && old.asset.key() == new.asset.key()
        && old.meta.timestamp == new.meta.timestamp;
    if same {
        return None;
    }
    new.distribution = None;
    Some(rows_of(parts.clone()))
}

/// The rows of a topup that arrives with a distribution; the list itself is taken off, as the
/// distribution usecase writes it (spec S11).
fn take_distribution_rows(tx: &mut Transaction) -> Option<Vec<DistributionRow>> {
    let Transaction::Topup(topup) = tx else {
        return None;
    };
    let parts = topup.distribution.take()?;
    Some(rows_of(parts))
}

/// The income as stored once its distribution is applied.
fn stored_income(vault: &Session, income: &Transaction) -> Result<Transaction> {
    let id = &income.meta().id;
    vault
        .doc
        .transactions
        .iter()
        .find(|tx| &tx.meta().id == id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("income {id}")))
}

/// Only a distribution may link an operation to an income; a caller's link is forged.
pub(crate) fn refuse_forged_income_ref(tx: &Transaction) -> Result<()> {
    if tx.meta().income_ref.is_some() {
        return Err(income_ref_reserved());
    }
    Ok(())
}

/// Save several operations as one movement of money (an exchange split
/// between recipients, a P2P deal): all join one chain — `flow_id`, or a
/// new one — and either all are saved or none is.
pub fn add_linked_transactions(
    vault: &mut Session,
    txs: Vec<Transaction>,
    flow_id: Option<&str>,
) -> Result<Vec<Transaction>> {
    if txs.is_empty() {
        return Err(Error::InvalidInput("nothing to save".into()));
    }
    let flow = flow_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let backup = vault.doc.clone();
    match book_linked(vault, txs, &flow) {
        Ok(saved) => {
            rebase_and_commit(vault)?;
            Ok(saved)
        }
        Err(err) => {
            vault.doc = backup;
            crate::usecases::money::portfolio::recompute_holdings(vault);
            Err(err)
        }
    }
}

fn book_linked(vault: &mut Session, txs: Vec<Transaction>, flow: &str) -> Result<Vec<Transaction>> {
    let mut saved = Vec::with_capacity(txs.len());
    for mut tx in txs {
        refuse_forged_income_ref(&tx)?;
        apply_creation_rules(vault, &mut tx, &CreationRules::default());
        let rows = take_distribution_rows(&mut tx);
        tx.meta_mut().flow_id = Some(flow.to_string());
        tx.meta_mut().id = uuid::Uuid::new_v4().to_string();
        prepare_transaction(vault, &mut tx)?;
        vault.run(|run| {
            run.doc_mut().transactions.push(tx.clone());
            Ok(())
        })?;
        // The next one sees this one's balances, as if saved in turn.
        crate::usecases::money::portfolio::recompute_holdings(vault);
        match rows {
            Some(rows) => {
                apply_distribution_unsaved(vault, &tx.meta().id, &rows)?;
                saved.push(stored_income(vault, &tx)?);
            }
            None => saved.push(tx),
        }
    }
    Ok(saved)
}

/// Replace a transaction entirely. Its id and origin are kept: an edit
/// changes what happened, not where the record came from, and never drops a
/// link the form has no field for.
pub fn update_transaction(vault: &mut Session, id: &str, tx: Transaction) -> Result<Transaction> {
    let property =
        crate::household::property_links::property_of_tx(&vault.doc, id).map(str::to_string);
    edit(vault, id, replacing(tx), property.as_deref(), true)
}

/// Change an operation by naming only what changes (F4): every link the patch
/// does not name keeps its stored value, the property link included. A scenario
/// only: it is new, so the vault has no delegate for it.
pub fn patch_transaction(
    vault: &mut Session,
    id: &str,
    patch: TransactionPatch,
) -> Result<Transaction> {
    let property =
        crate::household::property_links::property_of_tx(&vault.doc, id).map(str::to_string);
    edit(vault, id, patch, property.as_deref(), true)
}

/// [`update_transaction`] for the shared event that owns the operation: the
/// event has changed its own side already.
pub(crate) fn update_transaction_unsynced(
    vault: &mut Session,
    id: &str,
    tx: Transaction,
) -> Result<Transaction> {
    let property =
        crate::household::property_links::property_of_tx(&vault.doc, id).map(str::to_string);
    edit(vault, id, replacing(tx), property.as_deref(), false)
}

/// [`update_transaction`] that also sets the operation's property; `None`
/// unlinks it.
pub fn update_transaction_for(
    vault: &mut Session,
    id: &str,
    tx: Transaction,
    property_id: Option<&str>,
) -> Result<Transaction> {
    edit(vault, id, replacing(tx), property_id, true)
}

/// `follow_events`: an operation a shared event owns drags the event's
/// purchase or settlement along (or is refused).
fn edit(
    vault: &mut Session,
    id: &str,
    patch: TransactionPatch,
    property_id: Option<&str>,
    follow_events: bool,
) -> Result<Transaction> {
    check_property_link(vault, property_id)?;
    let index = position_of(&vault.doc.transactions, id)?;
    let stored = vault.doc.transactions[index].clone();
    let mut tx = patched(&stored, id, patch);
    keep_income_links(&stored, &mut tx)?;
    let redistribute = rows_for_edited_income(&stored, &mut tx);
    settle_tx_spread(&mut tx, true, Some(&stored));
    prepare_transaction(vault, &mut tx)?;
    let event_edit = if follow_events {
        crate::household::shared_events::ledger::plan_tx_edit(&vault.doc, id, &tx)?
    } else {
        None
    };
    let backup = vault.doc.clone();
    if redistribute.is_some() {
        drop_distribution_of(vault, id)?;
    }
    let index = position_of(&vault.doc.transactions, id)?;
    vault.run(|run| {
        run.doc_mut().transactions[index] = tx.clone();
        Ok(())
    })?;
    if let Some(rows) = redistribute {
        // A failure refuses the whole edit: the effects and the income are put back.
        match apply_distribution_unsaved(vault, id, &rows) {
            Ok(_) => tx = stored_income(vault, &tx)?,
            Err(err) => {
                vault.doc = backup;
                recompute_holdings(vault);
                return Err(err);
            }
        }
    }
    crate::usecases::household::shared_events::apply_shared_edit(vault, event_edit);
    set_property_link(vault, id, property_id);
    rebase_and_commit(vault)?;
    Ok(tx)
}

fn position_of(transactions: &[Transaction], id: &str) -> Result<usize> {
    transactions
        .iter()
        .position(|known| known.meta().id == id)
        .ok_or_else(|| Error::Missing("transaction".into()))
}

pub fn delete_transaction(vault: &mut Session, id: &str) -> Result<()> {
    let index = position_of(&vault.doc.transactions, id)?;
    refuse_if_linked(&vault.doc.transactions[index])?;
    drop_distribution_of(vault, id)?;
    let index = position_of(&vault.doc.transactions, id)?;
    vault.run(|run| {
        run.doc_mut().transactions.remove(index);
        Ok(())
    })?;
    cascade_shared_delete(vault, id);
    set_property_link(vault, id, None);
    rebase_and_commit(vault)
}
