//! Scenarios of the utilities area: each decides in `household::utilities`,
//! then ends in one `vault.transact`. The automations, the ledger rules and
//! the holdings are areas that have not moved yet; they are asked through the
//! vault.

use crate::error::Result;
use crate::household::utilities::{
    BillPlan, PayUtilityBillInput, PropertySummary, ReadingInput, UtilityBill, UtilityDraft,
    UtilityPaymentDraft, plan_add_reading, plan_create_utility, plan_delete_utility,
    plan_pay_utility_bill, plan_update_reading, plan_update_utility, require_reading,
    utility_bill as bill_of, utility_payment_draft as draft_of,
};
use crate::kernel::tx::Tx;
use crate::model::planning::{MeterReading, UtilityItem};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::service::Session;
use crate::sync::sync_ids::reading_id;
use crate::{Amount, calendar};

/// The property summary: value, debt, equity and monthly costs.
pub fn property_summary(vault: &Session, store_id: &str, zone: &str) -> Result<PropertySummary> {
    crate::household::utilities::property_summary(
        &vault.books(),
        store_id,
        zone,
        calendar::now_secs(),
    )
}

/// The "Pay" for one month of an item, dated by today.
pub fn utility_payment_draft(
    vault: &Session,
    item_id: &str,
    period: &str,
    zone: &str,
) -> Result<UtilityPaymentDraft> {
    draft_of(&vault.books(), item_id, period, zone, calendar::now_secs())
}

/// The month's bill of a property, due today when the month is this one.
pub fn utility_bill(
    vault: &Session,
    property_id: &str,
    period: &str,
    zone: &str,
) -> Result<UtilityBill> {
    bill_of(
        &vault.books(),
        property_id,
        period,
        zone,
        calendar::now_secs(),
    )
}

fn automation_known(vault: &Session) -> impl Fn(&str) -> bool + '_ {
    |id| crate::usecases::planning::automations::automation(vault, id).is_ok()
}

pub fn create_utility_item(vault: &mut Session, draft: UtilityDraft) -> Result<UtilityItem> {
    let item = plan_create_utility(&vault.doc, draft, &automation_known(vault))?;
    vault.transact(|tx| {
        tx.collection::<UtilityItem>().insert(item.clone())?;
        Ok(item)
    })
}

pub fn update_utility_item(
    vault: &mut Session,
    id: &str,
    draft: UtilityDraft,
) -> Result<UtilityItem> {
    let item = plan_update_utility(&vault.doc, id, draft, &automation_known(vault))?;
    vault.transact(|tx| {
        tx.collection::<UtilityItem>().update(id, |stored| {
            *stored = item.clone();
            item
        })
    })
}

/// Remove an item with its meter readings; payments already made stay.
pub fn delete_utility_item(vault: &mut Session, id: &str) -> Result<()> {
    let readings = plan_delete_utility(&vault.doc, id)?;
    vault.transact(|tx| {
        for reading_id in &readings {
            tx.collection::<MeterReading>().remove(reading_id)?;
        }
        tx.collection::<UtilityItem>().remove(id).map(|_| ())
    })
}

/// Record a meter reading for a month (`YYYY-MM`) and, optionally, a day of
/// it; one already there for the same month and day is replaced. A month
/// may hold several readings (a move-out on the 15th). A meter only counts
/// up: a reading below an earlier one or above a later one is refused.
pub fn add_meter_reading(
    vault: &mut Session,
    item_id: &str,
    value: Amount,
    period: &str,
    day: Option<u8>,
    zone: Option<&str>,
) -> Result<MeterReading> {
    let input = ReadingInput {
        item_id,
        value,
        period,
        day,
        zone,
    };
    let reading = plan_add_reading(&vault.doc, input, &reading_id, &now_iso())?;
    vault.transact(|tx| {
        store_reading(tx, reading.clone())?;
        Ok(reading)
    })
}

/// Correct a reading: its value and, if moved, its month and day. The
/// same rules as a new one apply against every other reading of the meter.
pub fn update_meter_reading(
    vault: &mut Session,
    id: &str,
    value: Amount,
    period: &str,
    day: Option<u8>,
) -> Result<MeterReading> {
    let moved = plan_update_reading(&vault.doc, id, value, period, day, &reading_id, &now_iso())?;
    vault.transact(|tx| {
        // The id follows the day: a reading moved to another day is the
        // record of that day (the old one is deleted).
        tx.collection::<MeterReading>().remove(&moved.old_id)?;
        store_reading(tx, moved.reading.clone())?;
        Ok(moved.reading)
    })
}

pub fn delete_meter_reading(vault: &mut Session, id: &str) -> Result<()> {
    require_reading(&vault.doc, id)?;
    vault.transact(|tx| tx.collection::<MeterReading>().remove(id).map(|_| ()))
}

/// Pay a month's bill in one go: record the meter readings, then one
/// payment per line, linked to its item. All or nothing — a line that
/// fails leaves the vault as it was.
pub fn pay_utility_bill(
    vault: &mut Session,
    input: PayUtilityBillInput,
    zone: &str,
) -> Result<Vec<Transaction>> {
    let bill = bill_of(
        &vault.books(),
        &input.property_id,
        &input.period,
        zone,
        calendar::now_secs(),
    )?;
    let plan = plan_pay_utility_bill(
        &vault.doc,
        &input,
        &bill,
        &|tx| crate::usecases::money::transactions::prepare_transaction(vault, tx),
        &reading_id,
        &now_iso(),
    )?;
    let saved = plan.payments.clone();
    // The holdings follow the payments and the ledger derives them from the whole
    // vault, so the plan is applied and the holdings recomputed before the one
    // commit. `write_bill` emits no events, so none is dropped by `run`.
    vault.run(|tx| write_bill(tx, plan))?;
    crate::usecases::money::portfolio::recompute_holdings(vault);
    vault.transact(|tx| {
        tx.touch();
        Ok(())
    })?;
    Ok(saved)
}

fn write_bill(tx: &mut Tx<'_>, plan: BillPlan) -> Result<()> {
    for reading in plan.readings {
        store_reading(tx, reading)?;
    }
    if !plan.payments.is_empty() {
        tx.doc_mut().transactions.extend(plan.payments);
    }
    Ok(())
}

/// Keep `reading`; one already stored under its id (same meter, zone and
/// day) is replaced.
fn store_reading(tx: &mut Tx<'_>, reading: MeterReading) -> Result<()> {
    let mut readings = tx.collection::<MeterReading>();
    if readings.get(&reading.id).is_some() {
        readings.remove(&reading.id)?;
    }
    readings.insert(reading)
}
