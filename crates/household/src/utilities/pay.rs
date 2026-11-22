//! Paying a month's bill in one go: the meter readings of the lines, then one
//! payment per line, linked to its item. The plan is all or nothing: a line
//! that fails leaves the document as it was, because nothing is written yet.

use mwt_base::error::{Error, Result};
use mwt_model::AssetRef;
use mwt_model::MeterReading;
use mwt_model::VaultDoc;
use mwt_model::{KindDetails, PaymentTx, PriceStamp, Transaction, TxMeta};

use super::items::utility_item;
use super::readings::{ReadingInput, ReadingNamer, plan_reading};
use super::types::{PayUtilityBillInput, UtilityBill};

/// What paying a bill writes: the readings, then the payments.
#[derive(Debug, Clone, PartialEq)]
pub struct BillPlan {
    pub readings: Vec<MeterReading>,
    pub payments: Vec<Transaction>,
}

/// The readings and payments of `input` against the `bill` of its month.
/// `prepare` is the transaction rules of the ledger (categories, prices,
/// validation), run on each payment in turn.
pub fn plan_pay_utility_bill(
    doc: &VaultDoc,
    input: &PayUtilityBillInput,
    bill: &UtilityBill,
    prepare: &dyn Fn(&mut Transaction) -> Result<()>,
    name: ReadingNamer<'_>,
    now: &str,
) -> Result<BillPlan> {
    let timestamp = input
        .timestamp
        .clone()
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or_else(|| bill.due_at.clone());
    // Readings of this bill count against each other, as if already stored.
    let mut stored = doc.meter_readings.clone();
    let mut plan = BillPlan {
        readings: Vec::new(),
        payments: Vec::new(),
    };
    for line in &input.lines {
        if !line.amount.is_positive() {
            continue;
        }
        let item = utility_item(doc, &line.item_id)?;
        if item.property_id != input.property_id {
            return Err(Error::InvalidInput(format!(
                "{} belongs to another property",
                item.name
            )));
        }
        let mut readings = Vec::new();
        if let Some(reading) = line.reading
            && line.zone_readings.is_empty()
        {
            readings.push((reading, None));
        }
        readings.extend(
            line.zone_readings
                .iter()
                .map(|zoned| (zoned.reading, Some(zoned.zone.as_str()))),
        );
        for (value, zone) in readings {
            let reading = plan_reading(
                &doc.utility_items,
                &stored,
                ReadingInput {
                    item_id: &item.id,
                    value,
                    period: &bill.period,
                    day: line.reading_day,
                    zone,
                },
                name,
                now,
            )?;
            stored.retain(|known| known.id != reading.id);
            stored.push(reading.clone());
            plan.readings.push(reading);
        }
        let store_id = item
            .store_id
            .clone()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| input.store_id.clone());
        if store_id.trim().is_empty() {
            return Err(Error::InvalidInput(format!(
                "{}: pick the store to pay from",
                item.name
            )));
        }
        let code = item.currency.to_uppercase();
        let mut meta = TxMeta::new(&item.name);
        meta.categories = vec![item.category.clone()];
        meta.utility_item_id = Some(item.id.clone());
        meta.timestamp = timestamp.clone();
        let mut tx = Transaction::Payment(PaymentTx {
            meta,
            store_id,
            asset: AssetRef::fiat(&code, &code),
            amount: line.amount,
            fee: None,
            to: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        });
        prepare(&mut tx)?;
        plan.payments.push(tx);
    }
    Ok(plan)
}
