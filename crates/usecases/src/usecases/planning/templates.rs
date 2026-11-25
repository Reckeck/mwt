//! Scenarios of the operation templates (spec op-period T3): each decides in `planning::templates`,
//! then ends in one `vault.transact`. Delete returns the template and restore takes it back, as
//! the other generic deletes do.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::transaction::{PaymentTx, TopupTx, Transaction};
use crate::model::vault::now_iso;
use crate::planning::automation_plan as plan;
use crate::planning::distribution::resolve_distribution;
use crate::planning::templates::{
    TemplateDraft, plan_archive_template, plan_create_template, plan_delete_template,
    plan_restore_template, plan_update_template,
};
use crate::service::Session;
use crate::usecases::money::transactions::{
    CreationRules, add_transaction_distributed, add_transaction_ruled,
};
use mwt_model::{
    DistKind, DistributionRow, DistributionRule, KindDetails, OperationTemplate, PriceStamp,
    TemplateKind, TxMeta,
};

/// Every template, live and archived, by name.
pub fn list_operation_templates(vault: &Session) -> Vec<OperationTemplate> {
    let mut templates = vault.doc.operation_templates.clone();
    templates.sort_by_key(|template| template.name.to_lowercase());
    templates
}

pub fn create_operation_template(
    vault: &mut Session,
    draft: TemplateDraft,
) -> Result<OperationTemplate> {
    let template = plan_create_template(
        &vault.doc,
        draft,
        uuid::Uuid::new_v4().to_string(),
        &now_iso(),
    )?;
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>()
            .insert(template.clone())?;
        Ok(template)
    })
}

pub fn update_operation_template(
    vault: &mut Session,
    id: &str,
    draft: TemplateDraft,
) -> Result<OperationTemplate> {
    let template = plan_update_template(&vault.doc, id, draft)?;
    replace(vault, template)
}

/// Archive (`archived = true`) or bring a template back; allowed while automations use it.
pub fn archive_operation_template(
    vault: &mut Session,
    id: &str,
    archived: bool,
) -> Result<OperationTemplate> {
    let template = plan_archive_template(&vault.doc, id, archived)?;
    replace(vault, template)
}

/// Delete a template, refused while an automation refers to it. The result undoes it.
pub fn delete_operation_template(vault: &mut Session, id: &str) -> Result<OperationTemplate> {
    let automations = plan::automations(&vault.books());
    let deleted = plan_delete_template(&vault.doc, id, &automations)?;
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>().remove(id)?;
        Ok(())
    })?;
    Ok(deleted)
}

/// Undo [`delete_operation_template`].
pub fn restore_operation_template(
    vault: &mut Session,
    deleted: OperationTemplate,
) -> Result<OperationTemplate> {
    plan_restore_template(&vault.doc, &deleted)?;
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>()
            .insert(deleted.clone())?;
        Ok(deleted)
    })
}

fn replace(vault: &mut Session, template: OperationTemplate) -> Result<OperationTemplate> {
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>()
            .update(&template.id.clone(), |slot| *slot = template.clone())?;
        Ok(template)
    })
}

/// What the user types over a template when quick-adding; every field left out comes from it.
#[derive(Debug, Clone, Default)]
pub struct QuickAdd {
    pub timestamp: Option<String>,
    pub amount: Option<Amount>,
    pub store_id: Option<String>,
    pub comment: Option<String>,
    /// «Really none» (D3): the operation takes no period rule.
    pub no_period_rule: bool,
}

/// An operation made from a template.
#[derive(Debug, Clone)]
pub struct QuickAdded {
    pub transaction: Transaction,
    /// The rule asks to confirm the distribution (D9): the income is saved, nothing is distributed.
    pub ask: bool,
}

/// Quick-add (spec T4, D3, S15): the operation of a template with explicit fields winning, its
/// period from the template's rule, and for an income with a distribution rule the resolved rows
/// applied in the same write; a refused distribution leaves the vault as it was.
pub fn create_from_template(
    vault: &mut Session,
    template_id: &str,
    input: QuickAdd,
) -> Result<QuickAdded> {
    let template = vault
        .doc
        .operation_templates
        .iter()
        .find(|template| template.id == template_id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("template {template_id}")))?;
    let tx = transaction_of(&template, &input)?;
    let rules = CreationRules {
        template_rule: template.period_rule,
        no_period_rule: input.no_period_rule,
    };
    let (rows, ask) = match (&template.distribution, &tx) {
        (Some(rule), Transaction::Topup(topup)) => {
            let expected = template.amount.unwrap_or(topup.amount);
            let held = held_quantity(vault, topup);
            rows_of_rule(rule, topup, expected, held)?
        }
        _ => (None, false),
    };
    let transaction = match rows {
        Some(rows) => add_transaction_distributed(vault, tx, None, &rules, Some(rows))?,
        None => add_transaction_ruled(vault, tx, None, &rules)?,
    };
    Ok(QuickAdded { transaction, ask })
}

fn transaction_of(template: &OperationTemplate, input: &QuickAdd) -> Result<Transaction> {
    let store_id = input
        .store_id
        .clone()
        .or_else(|| template.store_id.clone())
        .ok_or_else(|| Error::InvalidInput("TEMPLATE_NO_STORE: pick a store".into()))?;
    let asset = template.asset.clone().ok_or_else(|| {
        Error::InvalidInput("TEMPLATE_NO_ASSET: the template has no asset".into())
    })?;
    let amount = input
        .amount
        .or(template.amount)
        .ok_or_else(|| Error::InvalidInput("TEMPLATE_NO_AMOUNT: enter an amount".into()))?;
    let comment = input.comment.as_deref().unwrap_or(&template.comment);
    let mut meta = TxMeta::new(comment);
    if let Some(stamp) = input.timestamp.as_deref().filter(|stamp| !stamp.is_empty()) {
        meta.timestamp = stamp.to_string();
    }
    meta.categories = template.categories.clone();
    meta.contact_id = template.contact_id.clone();
    Ok(match template.kind {
        TemplateKind::Income => Transaction::Topup(TopupTx {
            parts: Vec::new(),
            distribution: None,
            meta,
            store_id,
            asset,
            amount,
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        }),
        TemplateKind::Expense => Transaction::Payment(PaymentTx {
            meta,
            store_id,
            asset,
            amount,
            fee: None,
            to: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        }),
    })
}

/// What the card holds once the payout is booked, before any row (the rule takes the rows off).
fn held_quantity(vault: &Session, topup: &TopupTx) -> Amount {
    let held = vault
        .doc
        .holdings
        .iter()
        .find(|holding| {
            holding.store_id == topup.store_id && holding.asset.key() == topup.asset.key()
        })
        .map_or(Amount::ZERO, |holding| holding.quantity);
    held + topup.amount
}

/// T14: the rule against the actual payout. `None` rows with `ask` — the user confirms later; a
/// rule that asks more than the payout is refused, so nothing is written. Rows cut to zero are
/// dropped; with none left the income is saved undistributed.
fn rows_of_rule(
    rule: &DistributionRule,
    topup: &TopupTx,
    expected: Amount,
    card_balance_before_rows: Amount,
) -> Result<(Option<Vec<DistributionRow>>, bool)> {
    let places = topup.asset.minor_places();
    let resolved = resolve_distribution(
        rule,
        topup.amount,
        expected,
        card_balance_before_rows,
        places,
    )?;
    if resolved.ask {
        return Ok((None, true));
    }
    if let Some(over) = resolved.over {
        return Err(Error::InvalidInput(format!(
            "DISTRIBUTION_OVER_ALLOCATED: the rows ask {over} more than the income"
        )));
    }
    // A row the floor cut to nothing leaves its money on the card (D10); V1 allows positive parts only.
    let rows: Vec<DistributionRow> = resolved
        .rows
        .iter()
        .enumerate()
        .filter(|(_, resolved_row)| {
            matches!(resolved_row.row.kind, DistKind::Remainder)
                || resolved_row.amount.is_positive()
        })
        .map(|(index, resolved_row)| {
            let cut = resolved
                .cut
                .iter()
                .any(|(cut_index, _)| *cut_index == index);
            match (&resolved_row.row.kind, cut) {
                (DistKind::Remainder, _) | (_, false) => resolved_row.row.clone(),
                (_, true) => DistributionRow {
                    kind: DistKind::Sum {
                        amount: resolved_row.amount,
                    },
                    target: resolved_row.row.target.clone(),
                },
            }
        })
        .collect();
    // Nothing but the remainder is left: the income is saved undistributed.
    let moves_money = rows
        .iter()
        .any(|row| !matches!(row.kind, DistKind::Remainder));
    Ok((moves_money.then_some(rows), false))
}
