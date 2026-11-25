//! The one usecase behind an income that distributes itself (spec S8, S11): apply, replace and
//! remove a distribution. Rows are resolved and planned in `planning::distribution`, the result is
//! checked by `money::distribution`; this module only books the effects. Everything runs on one
//! document and is committed once: on any error the document is put back as it was, so nothing
//! is written.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::ids::StoreId;
use crate::model::planning::Automation;
use crate::model::vault::now_iso;
use crate::money::distribution::{
    BAD_TARGET, NOT_INCOME, WITH_PARTS, validate_distribution_targets, verify_distribution,
};
use crate::money::transactions::ensure_fresh_id;
use crate::planning::distribution::{
    ResolvedDistribution, plan_distribution, resolve_distribution, resolve_rows,
};
use crate::planning::envelopes::{EnvelopeDraft, new_envelope, plan_fund_envelope, plan_fund_goal};
use crate::service::Session;
use crate::usecases::household::envelopes::goal_envelope_for;
use crate::usecases::money::portfolio::recompute_holdings;
use crate::usecases::money::stores::rebase_and_commit;
use crate::usecases::money::transactions::{
    CreationRules, apply_creation_rules, prepare_transaction, prepare_transaction_at,
};
use crate::usecases::planning::automation_rules::expected_payout;
use crate::usecases::planning::templates::held_quantity;
use mwt_model::{
    AccountRef, AssetRef, DistKind, DistTarget, DistributionPart, DistributionRow, EffectRef,
    OpPeriod, TopupTx, Transaction, TransferTx, TxKind, TxMeta, TxOrigin, VaultDoc,
};

/// Stable code of a distribution whose rows ask for more than the income.
pub const OVER_ALLOCATED: &str = "DISTRIBUTION_OVER_ALLOCATED";
/// Stable code of an income that already distributes (apply is not replace).
pub const ALREADY_DISTRIBUTED: &str = "DISTRIBUTION_ALREADY_DISTRIBUTED";

/// What the income needs to know to book effects.
struct Income {
    /// The clock of the run that books it, in milliseconds; the wall clock when `None`.
    now_ms: Option<i64>,
    id: String,
    store_id: String,
    asset: AssetRef,
    amount: Amount,
    timestamp: String,
    old_parts: Vec<DistributionPart>,
    /// Who made the income, when an automation run books it: its effects carry this identity so
    /// that the sync dedupe takes the effects of a removed twin with it.
    run: Option<RunIdentity>,
}

/// The run an income came from: its effects are named after it.
struct RunIdentity {
    workflow_id: String,
    run_id: String,
    /// The workflow and the income's run key (else its planned moment): the same on every device.
    key: String,
}

impl RunIdentity {
    /// Only the income of a run (`booked_by_run`) has one.
    fn of(origin: &TxOrigin, booked_by_run: bool) -> Option<Self> {
        let TxOrigin::Workflow {
            workflow_id,
            run_id,
            planned_at,
            run_key,
            ..
        } = origin
        else {
            return None;
        };
        let moment = run_key.as_ref().or(planned_at.as_ref())?;
        let key = format!("{workflow_id}:{moment}");
        booked_by_run.then(|| Self {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            key,
        })
    }

    /// The key of the effect of the row at `index`.
    fn effect_key(&self, index: usize) -> String {
        format!("{}:{index}", self.key)
    }

    fn effect_origin(&self, index: usize) -> TxOrigin {
        TxOrigin::Workflow {
            workflow_id: self.workflow_id.clone(),
            run_id: self.run_id.clone(),
            planned_at: None,
            planned_date: None,
            estimated: false,
            run_key: Some(self.effect_key(index)),
        }
    }
}

/// Distribute an income that does not distribute yet: one effect per non-remainder row, the list
/// stored on the income (spec S8). Returns the stored parts.
pub fn apply_distribution(
    vault: &mut Session,
    income_id: &str,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    if income_of(&vault.doc, income_id)?.distribution.is_some() {
        return Err(Error::InvalidInput(format!(
            "{ALREADY_DISTRIBUTED}: replace the distribution instead"
        )));
    }
    let parts = apply_distribution_unsaved(vault, income_id, rows)?;
    rebase_and_commit(vault)?;
    Ok(parts)
}

/// [`apply_distribution`] without the commit: the caller's write owns it and puts the document
/// back on its own error. The income must not distribute yet.
pub(crate) fn apply_distribution_unsaved(
    vault: &mut Session,
    income_id: &str,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    rewrite_unsaved(vault, income_id, Some(rows), None)
}

/// [`apply_distribution_unsaved`] for an automation run: its transfers are checked against the
/// run's own clock `now_ms`, not the wall one (an emulated run is days ahead).
pub(crate) fn apply_distribution_at(
    vault: &mut Session,
    income_id: &str,
    rows: &[DistributionRow],
    now_ms: i64,
) -> Result<Vec<DistributionPart>> {
    rewrite_unsaved(vault, income_id, Some(rows), Some(now_ms))
}

/// The rows a stored list was asked with: a rule re-resolves them against a new amount.
pub(crate) fn rows_of(parts: Vec<DistributionPart>) -> Vec<DistributionRow> {
    parts
        .into_iter()
        .map(|part| DistributionRow {
            kind: part.kind,
            target: part.target,
        })
        .collect()
}

/// Before an income is deleted: its distribution goes first, with every effect, uncommitted.
pub(crate) fn drop_distribution_of(vault: &mut Session, id: &str) -> Result<()> {
    let distributes = vault.doc.transactions.iter().any(|tx| {
        matches!(tx, Transaction::Topup(topup) if topup.meta.id == id && topup.distribution.is_some())
    });
    if distributes {
        rewrite_unsaved(vault, id, None, None)?;
    }
    Ok(())
}

/// Edit the distribution (spec S11, A2b): the effects of dropped or changed parts are removed, the
/// new ones created, an unchanged part keeps its effect. Distributes an income that did not.
pub fn replace_distribution(
    vault: &mut Session,
    income_id: &str,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    let parts = rewrite_unsaved(vault, income_id, Some(rows), None)?;
    rebase_and_commit(vault)?;
    Ok(parts)
}

/// Undo: every effect is removed and the income stores no distribution.
pub fn remove_distribution(vault: &mut Session, income_id: &str) -> Result<()> {
    rewrite_unsaved(vault, income_id, None, None)?;
    rebase_and_commit(vault)
}

fn income_of<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc TopupTx> {
    match doc.transactions.iter().find(|tx| tx.meta().id == id) {
        Some(Transaction::Topup(topup)) => Ok(topup),
        Some(_) => Err(Error::InvalidInput(format!(
            "{NOT_INCOME}: only an income can be distributed"
        ))),
        None => Err(Error::Missing(format!("income {id}"))),
    }
}

fn read_income(vault: &Session, income_id: &str, now_ms: Option<i64>) -> Result<Income> {
    let topup = income_of(&vault.doc, income_id)?;
    if !topup.parts.is_empty() {
        return Err(Error::InvalidInput(format!(
            "{WITH_PARTS}: an income cannot arrive in parts and distribute"
        )));
    }
    Ok(Income {
        now_ms,
        id: income_id.to_string(),
        store_id: topup.store_id.clone(),
        asset: topup.asset.clone(),
        amount: topup.amount,
        timestamp: topup.meta.timestamp.clone(),
        old_parts: topup.distribution.clone().unwrap_or_default(),
        run: RunIdentity::of(&topup.meta.origin, now_ms.is_some()),
    })
}

/// `rows` of `None` removes the distribution. Not committed; on an error the document is put back.
fn rewrite_unsaved(
    vault: &mut Session,
    income_id: &str,
    rows: Option<&[DistributionRow]>,
    now_ms: Option<i64>,
) -> Result<Vec<DistributionPart>> {
    let income = read_income(vault, income_id, now_ms)?;
    let wanted = match rows {
        Some(rows) => resolve(vault, &income, rows)?,
        None => Vec::new(),
    };
    let plan = plan_distribution(&income.old_parts, &wanted);
    let backup = vault.doc.clone();
    let stored = book(vault, &income, plan.remove, plan.parts, rows.is_some());
    if stored.is_err() {
        vault.doc = backup;
    }
    stored
}

/// The rows against the income, refused when they ask too much or name a bad target.
fn resolve(
    vault: &Session,
    income: &Income,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    let resolved = resolve_rows(income.amount, rows, income.asset.minor_places())?;
    if let Some(over) = resolved.over {
        return Err(Error::InvalidInput(format!(
            "{OVER_ALLOCATED}: the rows ask {over} more than the income"
        )));
    }
    if let Some(part) = resolved
        .parts
        .iter()
        .find(|part| !matches!(part.kind, DistKind::Remainder) && !part.amount.is_positive())
    {
        return Err(Error::InvalidInput(format!(
            "{}: a part of {} is not allowed",
            crate::money::distribution::PART_NOT_POSITIVE,
            part.amount
        )));
    }
    validate_distribution_targets(&vault.doc, TxKind::Topup, &resolved.parts)?;
    Ok(resolved.parts)
}

/// Removes the dropped effects, creates the missing ones, stores the list and checks V1-V4.
fn book(
    vault: &mut Session,
    income: &Income,
    remove: Vec<EffectRef>,
    mut parts: Vec<DistributionPart>,
    keep_list: bool,
) -> Result<Vec<DistributionPart>> {
    for effect in &remove {
        drop_effect(vault, effect);
    }
    recompute_holdings(vault);
    for (index, part) in parts
        .iter_mut()
        .enumerate()
        .filter(|(_, part)| part.effect.is_none() && !matches!(part.kind, DistKind::Remainder))
    {
        part.effect = Some(create_effect(vault, income, part, index)?);
    }
    store_list(vault, &income.id, keep_list.then(|| parts.clone()))?;
    let violations = verify_distribution(&vault.books(), &income.id);
    if violations.is_empty() {
        return Ok(parts);
    }
    let codes: Vec<&str> = violations.iter().map(|found| found.code).collect();
    Err(Error::InvalidInput(format!(
        "the distribution breaks its invariants: {}",
        codes.join(", ")
    )))
}

fn drop_effect(vault: &mut Session, effect: &EffectRef) {
    match effect {
        EffectRef::Transfer { tx_id } => vault.doc.transactions.retain(|tx| &tx.meta().id != tx_id),
        EffectRef::EnvelopeMove {
            envelope_id,
            move_id,
        } => {
            if let Some(envelope) = vault
                .doc
                .envelopes
                .iter_mut()
                .find(|envelope| &envelope.id == envelope_id)
            {
                envelope.moves.retain(|entry| &entry.id != move_id);
                envelope.allocated = envelope.moves_sum();
            }
        }
    }
}

fn create_effect(
    vault: &mut Session,
    income: &Income,
    part: &DistributionPart,
    index: usize,
) -> Result<EffectRef> {
    match &part.target {
        DistTarget::Store { id } => create_transfer(vault, income, id, part.amount, index),
        DistTarget::Envelope { id } => {
            let envelope_id = id.clone();
            fill_envelope(vault, income, &envelope_id, part.amount, index)
        }
        DistTarget::Goal { goal_id, store_id } => {
            let envelope_id = goal_envelope(vault, income, goal_id, store_id, part.amount)?;
            fill_envelope(vault, income, &envelope_id, part.amount, index)
        }
        DistTarget::None => Err(Error::InvalidInput(
            "only the remainder may stay on the card".into(),
        )),
    }
}

fn create_transfer(
    vault: &mut Session,
    income: &Income,
    target_store: &str,
    amount: Amount,
    index: usize,
) -> Result<EffectRef> {
    let mut meta = TxMeta::new("");
    if let Some(run) = &income.run {
        meta.origin = run.effect_origin(index);
    }
    meta.timestamp = income.timestamp.clone();
    meta.income_ref = Some(income.id.clone());
    let mut transfer = Transaction::Transfer(TransferTx {
        meta,
        from: AccountRef::Store(income.store_id.clone()),
        to: AccountRef::Store(target_store.to_string()),
        asset: income.asset.clone(),
        amount,
        fee: None,
        network: None,
        tx_hash: None,
        debt: None,
        price: Default::default(),
    });
    match income.now_ms {
        Some(now_ms) => prepare_transaction_at(vault, &mut transfer, now_ms)?,
        None => prepare_transaction(vault, &mut transfer)?,
    }
    ensure_fresh_id(&vault.doc, &mut transfer);
    let tx_id = transfer.meta().id.clone();
    vault.doc.transactions.push(transfer);
    recompute_holdings(vault);
    Ok(EffectRef::Transfer { tx_id })
}

/// The envelope of a goal on the income's card, made when missing (as `fund_goal`), without a save.
fn goal_envelope(
    vault: &mut Session,
    income: &Income,
    goal_id: &str,
    store_id: &str,
    amount: Amount,
) -> Result<String> {
    if store_id != income.store_id {
        return Err(Error::InvalidInput(format!(
            "{BAD_TARGET}: a goal row can only be funded on the card the income arrived on"
        )));
    }
    let funding = plan_fund_goal(&vault.doc, goal_id, &StoreId::from(store_id), amount)?;
    if funding.take < amount {
        return Err(not_enough_free(funding.take, &funding.asset.symbol));
    }
    let id = goal_envelope_for(&vault.doc, &funding, goal_id, store_id);
    if funding.envelope_id.is_some() {
        return Ok(id);
    }
    let draft = EnvelopeDraft {
        store_id: store_id.to_string(),
        name: funding.goal_name,
        asset: funding.asset,
        target: None,
        goal_id: Some(goal_id.to_string()),
    };
    vault
        .doc
        .envelopes
        .push(new_envelope(draft, id.clone(), now_iso()));
    Ok(id)
}

fn fill_envelope(
    vault: &mut Session,
    income: &Income,
    envelope_id: &str,
    amount: Amount,
    index: usize,
) -> Result<EffectRef> {
    let envelope = vault
        .doc
        .envelopes
        .iter()
        .find(|envelope| envelope.id == envelope_id)
        .ok_or_else(|| Error::Missing(format!("envelope {envelope_id}")))?;
    if envelope.store_id != income.store_id || envelope.asset.key() != income.asset.key() {
        return Err(Error::InvalidInput(
            "the envelope must sit on the card and the asset of the income".into(),
        ));
    }
    plan_fund_envelope(&vault.doc, envelope_id, amount)?;
    let envelope = vault
        .doc
        .envelopes
        .iter_mut()
        .find(|envelope| envelope.id == envelope_id)
        .ok_or_else(|| Error::Missing(format!("envelope {envelope_id}")))?;
    envelope.record_move(amount, &income.timestamp);
    let entry = envelope
        .moves
        .last_mut()
        .ok_or_else(|| Error::InvalidInput("the envelope kept no entry for the move".into()))?;
    entry.income_ref = Some(income.id.clone());
    // One id on every device: the same run's move merges into one instead of being added twice.
    if let Some(run) = &income.run {
        entry.id = run.effect_key(index);
    }
    Ok(EffectRef::EnvelopeMove {
        envelope_id: envelope_id.to_string(),
        move_id: entry.id.clone(),
    })
}

fn not_enough_free(free: Amount, symbol: &str) -> Error {
    Error::InsufficientFunds(format!("only {free} {symbol} is free on the card"))
}

fn store_list(
    vault: &mut Session,
    income_id: &str,
    list: Option<Vec<DistributionPart>>,
) -> Result<()> {
    let topup = vault
        .doc
        .transactions
        .iter_mut()
        .find_map(|tx| match tx {
            Transaction::Topup(topup) if topup.meta.id == income_id => Some(topup),
            _ => None,
        })
        .ok_or_else(|| Error::Missing(format!("income {income_id}")))?;
    topup.distribution = list;
    Ok(())
}

/// The next payout of an automation under its template's rules, as the run would book it (D11).
#[derive(Debug, Clone, PartialEq)]
pub struct DistributionPreview {
    /// The next planned moment (RFC3339), moved off days off as the schedule says.
    pub planned_at: String,
    /// The period the template's rule gives to that money date (D1, D2).
    pub period: Option<OpPeriod>,
    /// The payout the rows are resolved against: the given one, else the expected one.
    pub amount: Amount,
    pub resolved: ResolvedDistribution,
}

/// D11: the rows of the next planned payout of `automation`. A pure read: the payout is a copy of
/// the one the schedule would book, resolved with the very steps of a run (`apply_creation_rules`,
/// `resolve_distribution` against the card as held before the rows).
pub fn preview_rule(
    vault: &Session,
    automation: &Automation,
    amount: Option<Amount>,
    now: i64,
) -> Result<DistributionPreview> {
    let template = automation
        .operation_template_id
        .as_deref()
        .and_then(|id| {
            vault
                .doc
                .operation_templates
                .iter()
                .find(|known| known.id == id)
        })
        .ok_or_else(|| Error::Missing("the automation's operation template".into()))?;
    let rule = template
        .distribution
        .as_ref()
        .ok_or_else(|| Error::Missing("the template's distribution rule".into()))?;
    let calendar = crate::money::interest::calendar_of(&vault.doc);
    let (_, mut payout) = crate::planning::automation_plan::automation_occurrences(
        automation,
        &calendar,
        now,
        now + NEXT_DATE_LOOKAHEAD_SECS,
    )
    .into_iter()
    .next()
    .ok_or_else(|| Error::Missing("a next planned date of the automation".into()))?;
    let rules = CreationRules {
        template_rule: template.period_rule,
        no_period_rule: false,
    };
    apply_creation_rules(vault, &mut payout, &rules);
    let planned_at = payout.meta().timestamp.clone();
    let period = payout.meta().period.clone();
    let Transaction::Topup(topup) = &mut payout else {
        return Err(Error::InvalidInput(format!(
            "{NOT_INCOME}: only an income distributes"
        )));
    };
    if let Some(amount) = amount {
        topup.amount = amount;
    }
    let resolved = resolve_distribution(
        rule,
        topup.amount,
        expected_payout(automation),
        held_quantity(vault, topup),
        topup.asset.minor_places(),
    )?;
    Ok(DistributionPreview {
        planned_at,
        period,
        amount: topup.amount,
        resolved,
    })
}

/// A year and a bit: any schedule fires within it.
const NEXT_DATE_LOOKAHEAD_SECS: i64 = 367 * 86_400;
