//! Automations (plan §7.2): operations that repeat on a schedule.
//!
//! An automation is a workflow the app builds from the automation form
//! (`Workflow::automation`, see `workflow::automation`): "on schedule" →
//! "Create transaction". A planned date fires at 09:00 local time, moved off a
//! day off when asked. Each processed date leaves a workflow run — done,
//! refused for lack of funds (D26), or skipped — and a refused one waits for
//! the user to retry or skip it. With an amount that differs every time the
//! operation is booked at the average of the latest corrected amounts and
//! marked approximate until the user puts in the real one.

use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_base::time::secs_or_now;
use mwt_model::VaultDoc;
use mwt_model::{
    AmountMode, Automation, AutomationOutcome, AutomationPreset, AutomationSpec, RejectReason,
    Schedule, TransactionDraft, WorkdayShift, draft_to_transaction,
};
use mwt_model::{AssetRef, Store, StoreKind};
use mwt_model::{Transaction, TxOrigin};

/// Most planned dates one pass creates per automation; older missed dates
/// are recorded as skipped, so a long time away does not flood the history
/// (plan §7.2).
pub const MAX_CATCH_UP: usize = 24;

/// What an automation is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct AutomationDraft {
    pub name: String,
    pub active: bool,
    pub preset: Option<AutomationPreset>,
    pub schedule: Schedule,
    pub starts_at: String,
    pub template: TransactionDraft,
    pub amount_mode: AmountMode,
    pub workday_shift: WorkdayShift,
    pub property_id: Option<String>,
    pub time: Option<String>,
    pub asks: bool,
    pub for_month_offset: Option<i8>,
}

impl AutomationDraft {
    pub fn spec(&self) -> AutomationSpec {
        AutomationSpec {
            preset: self.preset,
            schedule: self.schedule,
            starts_at: self.starts_at.clone(),
            template: self.template.clone(),
            amount_mode: self.amount_mode,
            workday_shift: self.workday_shift,
            property_id: self.property_id.clone(),
            time: self.time.clone(),
            asks: self.asks,
            for_month_offset: self.for_month_offset,
            operation_template_id: None,
        }
    }
}

pub fn spec_of(automation: &Automation) -> AutomationSpec {
    AutomationSpec {
        preset: automation.preset,
        schedule: automation.schedule,
        starts_at: automation.starts_at.clone(),
        template: automation.template.clone(),
        amount_mode: automation.amount_mode,
        workday_shift: automation.workday_shift,
        property_id: automation.property_id.clone(),
        time: automation.time.clone(),
        asks: automation.asks,
        for_month_offset: automation.for_month_offset,
        operation_template_id: automation.operation_template_id.clone(),
    }
}

/// Longest emulation: a year ahead.
pub const MAX_SIMULATION_DAYS: u32 = 366;

/// One planned date as the emulation saw it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SimulatedRun {
    pub automation_id: String,
    pub automation_name: String,
    pub planned_at: String,
    pub outcome: AutomationOutcome,
    /// The operation it would create.
    pub transaction: Option<Transaction>,
}

/// One balance the emulated runs change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SimulatedBalance {
    pub store_id: String,
    pub asset: AssetRef,
    pub before: Amount,
    pub after: Amount,
}

/// What the automations would do over the next days, worked out on a copy
/// of the vault: nothing is written.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AutomationSimulation {
    pub from: String,
    pub until: String,
    pub runs: Vec<SimulatedRun>,
    pub balances: Vec<SimulatedBalance>,
    pub created: u32,
    pub rejected: u32,
}

/// The amount a top-up, a payment or a transfer moves (an exchange: what it gives).
pub fn main_amount(tx: &Transaction) -> Option<Amount> {
    match tx {
        Transaction::Topup(inner) => Some(inner.amount),
        Transaction::Payment(inner) => Some(inner.amount),
        Transaction::Transfer(inner) => Some(inner.amount),
        Transaction::Convert(inner) => Some(inner.from.amount),
        _ => None,
    }
}

fn set_main_amount(tx: &mut Transaction, amount: Amount) -> Result<()> {
    match tx {
        Transaction::Topup(inner) => inner.amount = amount,
        Transaction::Payment(inner) => inner.amount = amount,
        Transaction::Transfer(inner) => inner.amount = amount,
        _ => {
            return Err(Error::InvalidInput(
                "only a top-up, a payment or a transfer has one amount to correct".into(),
            ));
        }
    }
    Ok(())
}

fn with_amount(draft: &TransactionDraft, amount: Amount) -> TransactionDraft {
    let mut draft = draft.clone();
    match &mut draft {
        TransactionDraft::Topup(inner) => inner.amount = amount,
        TransactionDraft::Payment(inner) => inner.amount = amount,
        TransactionDraft::Transfer(inner) => inner.amount = amount,
        TransactionDraft::Convert(inner) => inner.from.amount = amount,
    }
    draft
}

/// What a transaction takes from a store of the user's.
pub struct Outflow {
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub fee: Option<(AssetRef, Amount)>,
}

/// The store and asset a transaction draws from, the principal it takes and
/// its fee: `None` when nothing leaves a store of the user's.
pub fn outflow(tx: &Transaction) -> Option<Outflow> {
    let fee = tx.fee().map(|fee| (fee.asset.clone(), fee.amount.abs()));
    let (store_id, asset, amount) = match tx {
        Transaction::Payment(inner) => (inner.store_id.clone(), &inner.asset, inner.amount),
        Transaction::Transfer(inner) => (
            inner.from.store_id()?.to_string(),
            &inner.asset,
            inner.amount,
        ),
        Transaction::Convert(inner) => (
            inner.from.account.store_id()?.to_string(),
            &inner.from.asset,
            inner.from.amount,
        ),
        _ => return None,
    };
    Some(Outflow {
        store_id,
        asset: asset.clone(),
        amount: amount.abs(),
        fee,
    })
}

pub use mwt_model::describe_reject;

/// The template the next run books: with an amount that differs, at the
/// current estimate.
pub fn automation_draft(automation: &Automation) -> TransactionDraft {
    match automation.estimate {
        Some(estimate) => with_amount(&automation.template, estimate),
        None => automation.template.clone(),
    }
}

// -- draft rules ---------------------------------------------------------------

/// Reject a draft that could never run, for what its fields say on their
/// own: a name, a schedule, an amount mode. The property link is checked next
/// by the caller (it reads the stores), then the time and the start date.
pub fn check_draft_form(draft: &AutomationDraft) -> Result<()> {
    if draft.name.trim().is_empty() {
        return Err(Error::InvalidInput("name the automation".into()));
    }
    match draft.schedule {
        Schedule::Monthly { day } if !(1..=31).contains(&day) => {
            return Err(Error::InvalidInput("day of month is 1–31".into()));
        }
        Schedule::Weekly { weekday } if weekday > 6 => {
            return Err(Error::InvalidInput("weekday is 0–6".into()));
        }
        Schedule::EveryNDays { days: 0 } => {
            return Err(Error::InvalidInput("repeat at least every day".into()));
        }
        _ => {}
    }
    if draft.amount_mode == AmountMode::Varies
        && matches!(draft.template, TransactionDraft::Convert(_))
    {
        return Err(Error::InvalidInput(
            "an amount that differs fits a top-up, a payment or a transfer, not an exchange".into(),
        ));
    }
    Ok(())
}

/// Whether the edit changes when the automation plans: its schedule, time of
/// day or the shift off days off. Missing when `id` is no automation.
pub fn schedule_changed(doc: &VaultDoc, id: &str, draft: &AutomationDraft) -> Result<bool> {
    let old = doc
        .workflows
        .iter()
        .find(|workflow| workflow.id == id)
        .and_then(|workflow| workflow.automation.as_ref())
        .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
    Ok(old.schedule != draft.schedule
        || old.time != draft.time
        || old.workday_shift != draft.workday_shift)
}

// -- funds (D26) ---------------------------------------------------------------

/// The store with this id. `planning` may not use `money::tree`, whose
/// `find_store` is the same lookup.
fn find_store<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc Store> {
    doc.stores.iter().find(|store| store.id == id)
}

/// Quantity of an asset on one store from transactions up to `secs`.
pub fn balance_at(doc: &VaultDoc, store_id: &str, asset_key: &str, secs: i64) -> Amount {
    doc.transactions
        .iter()
        .filter(|tx| secs_or_now(&tx.meta().timestamp) <= secs)
        .flat_map(|tx| tx.legs())
        .filter(|leg| leg.store_id == store_id && leg.asset.key() == asset_key)
        .fold(Amount::ZERO, |sum, leg| sum + leg.amount)
}

/// Whether the source of a transaction can pay for it at `secs`: its
/// balance (principal and a fee in the same asset; a fee in another asset
/// separately), or for a credit card its limit. Money coming in, and money
/// leaving someone else's side, is not checked.
/// `None` when it can.
pub fn funds_shortfall(doc: &VaultDoc, tx: &Transaction, secs: i64) -> Option<RejectReason> {
    let Outflow {
        store_id,
        asset,
        amount,
        fee,
    } = outflow(tx)?;
    let Some(store) = find_store(doc, &store_id) else {
        return Some(RejectReason::Invalid {
            message: format!("store {store_id} not found"),
        });
    };
    let same_asset_fee = fee
        .as_ref()
        .filter(|(fee_asset, _)| fee_asset.key() == asset.key())
        .map_or(Amount::ZERO, |(_, fee_amount)| *fee_amount);
    if store.kind == StoreKind::Liability {
        let limit = store.liability.as_ref().and_then(|terms| terms.limit)?;
        let debt =
            (Amount::ZERO - balance_at(doc, &store_id, &asset.key(), secs)).max(Amount::ZERO);
        let required = debt + amount + same_asset_fee;
        return (required > limit).then_some(RejectReason::CreditLimitExceeded {
            store_id,
            limit,
            required,
        });
    }
    let mut needs = vec![(asset.clone(), amount + same_asset_fee)];
    if let Some((fee_asset, fee_amount)) = fee
        && fee_asset.key() != asset.key()
    {
        needs.push((fee_asset, fee_amount));
    }
    for (need_asset, required) in needs {
        let available = balance_at(doc, &store_id, &need_asset.key(), secs);
        if available < required {
            return Some(RejectReason::InsufficientFunds {
                store_id,
                asset: need_asset,
                required,
                available,
            });
        }
    }
    None
}

/// «Card: 500 USD of 800 USD» — with the store's name, for the error text.
pub fn describe_shortfall(doc: &VaultDoc, reason: &RejectReason) -> String {
    let name =
        |id: &str| find_store(doc, id).map_or_else(|| id.to_string(), |store| store.name.clone());
    match reason {
        RejectReason::InsufficientFunds {
            store_id,
            asset,
            required,
            available,
        } => format!(
            "{}: {available} {sym} / {required} {sym}",
            name(store_id),
            sym = asset.symbol
        ),
        RejectReason::CreditLimitExceeded {
            store_id,
            limit,
            required,
        } => format!("{}: {required} > {limit}", name(store_id)),
        RejectReason::Invalid { message } => message.clone(),
    }
}

/// The error a refused funds check is: an invalid operation, or a shortfall
/// worded with the store's name.
pub fn funds_error(doc: &VaultDoc, reason: RejectReason) -> Error {
    match reason {
        RejectReason::Invalid { message } => Error::InvalidInput(message),
        other => Error::InsufficientFunds(describe_shortfall(doc, &other)),
    }
}

// -- manual runs and corrections ----------------------------------------------

/// The operation "Create now" books for an automation at the moment `planned_at`
/// (RFC3339), made by the run `run_id`.
pub fn manual_run_transaction(
    automation: &Automation,
    planned_at: &str,
    run_id: &str,
) -> Transaction {
    let mut tx = draft_to_transaction(
        &automation_draft(automation),
        &automation.id,
        planned_at,
        planned_at,
    );
    if let TxOrigin::Workflow {
        estimated,
        run_id: made_by,
        ..
    } = &mut tx.meta_mut().origin
    {
        *estimated = automation.amount_mode == AmountMode::Varies;
        *made_by = run_id.to_string();
    }
    tx
}

/// The operation `tx_id` with the real `amount` put in; refused unless it was
/// booked with an approximate amount.
pub fn confirmed_transaction(doc: &VaultDoc, tx_id: &str, amount: Amount) -> Result<Transaction> {
    if !amount.is_positive() {
        return Err(Error::InvalidInput("the amount must be above zero".into()));
    }
    let mut tx = doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id)
        .cloned()
        .ok_or_else(|| Error::Missing("transaction".into()))?;
    if !tx.meta().origin.is_estimated() {
        return Err(Error::InvalidInput(
            "this operation was not booked with an approximate amount".into(),
        ));
    }
    set_main_amount(&mut tx, amount)?;
    Ok(tx)
}

/// Drop the «approximate» mark of an operation; whether it was set.
pub fn clear_estimated(tx: &mut Transaction) -> bool {
    match &mut tx.meta_mut().origin {
        TxOrigin::Workflow { estimated, .. } if *estimated => {
            *estimated = false;
            true
        }
        _ => false,
    }
}
