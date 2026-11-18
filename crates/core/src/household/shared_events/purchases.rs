//! Purchases of an event: what was really bought, by whom, in which currency.
//! A purchase of mine is also a real payment in the vault; an event deletes
//! the payments it made and only unlinks the ones it adopted.
//!
//! Pure part: each `plan_*` checks a request against a document and returns the
//! event as it should become, with what the vault has to do about the payment.
//! The writes are the scenarios in `usecases::household::shared_events`.

use super::view::shared_tx_links;
use super::{find_event, item_category, normalize_moment, split};
use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{AttachPaymentInput, SharedPurchaseDraft};
use crate::model::VaultDoc;
use crate::model::shared_event::{EventItem, ItemSplit, ME, PriceMode, Purchase, SharedEvent};
use crate::model::store::AssetRef;
use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, Transaction, TxMeta};
use crate::money::rates::RateBook;
use crate::pricing::history::secs_or_now;

/// What the vault has to do about the payment of a changed purchase.
pub enum PaymentStep {
    /// Nothing: the purchase has no payment of mine, or keeps the one it has.
    Keep,
    /// Write the payment, then link it to the purchase (`link_purchase`).
    Add {
        payment: Transaction,
        purchase_id: String,
    },
    /// The payment the event made follows the edit.
    Update { tx_id: String, payment: Transaction },
    /// The payment the event made goes with the purchase.
    Drop { tx_id: String },
}

/// A purchase changed: the event as it should become and the payment step.
pub struct PurchaseEdit {
    pub event: SharedEvent,
    pub payment: PaymentStep,
}

/// A purchase draft, checked and completed.
struct Prepared {
    buyer: String,
    qty: Amount,
    amount: Amount,
    currency: String,
    price_mode: PriceMode,
    rate_to_event: Amount,
    rate_manual: bool,
    at: String,
    store_id: Option<String>,
}

impl Prepared {
    fn into_purchase(self, id: String) -> (Purchase, Option<String>) {
        let purchase = Purchase {
            id,
            buyer: self.buyer,
            qty: self.qty,
            amount: self.amount,
            currency: self.currency,
            price_mode: self.price_mode,
            rate_to_event: self.rate_to_event,
            rate_manual: self.rate_manual,
            tx_id: None,
            tx_owned: false,
            at: self.at,
        };
        (purchase, self.store_id)
    }
}

/// When a purchase happened if nobody said: `now` when that is inside the
/// event's days (or it has none), else the first day.
fn default_moment(event: &SharedEvent, now: &str) -> String {
    let Some(start) = event.starts_on.as_deref() else {
        return now.to_string();
    };
    let today: String = now.chars().take(10).collect();
    let end = event.ends_on.as_deref().unwrap_or(start);
    // Inside the days, or before them (a payment cannot be dated in the
    // future): now. After them: the first day.
    if today.as_str() <= end {
        now.to_string()
    } else {
        format!("{start}T12:00:00Z")
    }
}

fn find_purchase(event: &SharedEvent, purchase_id: &str) -> Result<(usize, usize)> {
    event
        .items
        .iter()
        .enumerate()
        .find_map(|(item_index, item)| {
            item.purchases
                .iter()
                .position(|purchase| purchase.id == purchase_id)
                .map(|purchase_index| (item_index, purchase_index))
        })
        .ok_or_else(|| Error::Missing(format!("purchase {purchase_id}")))
}

/// The rate of `currency` to the event currency at `at`, from the book.
fn book_rate(book: &RateBook, currency: &str, event: &SharedEvent, at: &str) -> Result<Amount> {
    match book.convert(split::one(), currency, &event.currency, secs_or_now(at)) {
        Some(rate) if rate.is_positive() => Ok(split::round_places(rate, 6)),
        _ => Err(Error::FxMissing(currency.to_uppercase())),
    }
}

fn prepare_purchase(
    book: &RateBook,
    event: &SharedEvent,
    draft: &SharedPurchaseDraft,
    now: &str,
) -> Result<Prepared> {
    let buyer = match draft.buyer.trim() {
        "" => ME.to_string(),
        other => other.to_string(),
    };
    if buyer == ME {
        if !event.i_participate {
            return Err(Error::InvalidInput(
                "you do not take part in this event: you cannot buy for it".into(),
            ));
        }
    } else if !event.participants.iter().any(|person| person.id == buyer) {
        return Err(Error::Missing(format!("participant {buyer}")));
    }
    if !draft.qty.is_positive() {
        return Err(Error::InvalidInput("the quantity must be positive".into()));
    }
    if !draft.amount.is_positive() {
        return Err(Error::InvalidInput("the amount must be positive".into()));
    }
    let currency = match draft.currency.trim().to_lowercase() {
        code if code.is_empty() => event.currency.clone(),
        code => code,
    };
    let at = match draft
        .at
        .as_deref()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
    {
        Some(raw) => normalize_moment(raw)?,
        None => default_moment(event, now),
    };
    let (rate_to_event, rate_manual) = if currency == event.currency {
        (split::one(), false)
    } else {
        match draft.rate_to_event {
            Some(rate) if rate.is_positive() => (rate, true),
            Some(_) => {
                return Err(Error::InvalidInput("the rate must be positive".into()));
            }
            None => (book_rate(book, &currency, event, &at)?, false),
        }
    };
    let store_id = draft
        .store_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    if buyer == ME && store_id.is_none() {
        return Err(Error::InvalidInput(
            "choose the wallet the payment comes from".into(),
        ));
    }
    Ok(Prepared {
        buyer,
        qty: draft.qty,
        amount: draft.amount,
        currency,
        price_mode: draft.price_mode,
        rate_to_event,
        rate_manual,
        at,
        store_id,
    })
}

/// The item a purchase goes to; a new one is made when none is named.
/// Returns its index. Works on a copy of the event, so nothing is kept if
/// the purchase is refused later.
fn resolve_purchase_item(
    event: &mut SharedEvent,
    item_id: Option<&str>,
    new_title: Option<&str>,
    new_unit: Option<&str>,
) -> Result<usize> {
    if let Some(id) = item_id.map(str::trim).filter(|id| !id.is_empty()) {
        let index = event
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| Error::Missing(format!("item {id}")))?;
        if event.items[index].cancelled {
            return Err(Error::InvalidInput("this item is cancelled".into()));
        }
        return Ok(index);
    }
    let title = new_title.map(str::trim).unwrap_or_default();
    if title.is_empty() {
        return Err(Error::InvalidInput(
            "choose an item or name the new one".into(),
        ));
    }
    event.items.push(EventItem {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.to_string(),
        unit: new_unit
            .map(str::trim)
            .filter(|unit| !unit.is_empty())
            .unwrap_or("шт")
            .to_string(),
        category_id: None,
        plan: None,
        purchases: Vec::new(),
        cancelled: false,
        split: ItemSplit::default(),
    });
    Ok(event.items.len() - 1)
}

/// My payment for a purchase.
fn shared_payment(
    event: &SharedEvent,
    item: &EventItem,
    purchase: &Purchase,
    store_id: &str,
    tx_id: &str,
) -> Transaction {
    let mut meta = TxMeta::new(&format!("{} · {}", event.title, item.title));
    meta.id = tx_id.to_string();
    meta.timestamp = purchase.at.clone();
    meta.categories = vec![item_category(event, item)];
    let code = purchase.currency.to_uppercase();
    Transaction::Payment(PaymentTx {
        meta,
        store_id: store_id.to_string(),
        asset: AssetRef::fiat(&code, &code),
        amount: purchase.total(),
        fee: None,
        to: None,
        details: KindDetails::default(),
        price: PriceStamp::default(),
    })
}

/// Put the id of the payment the vault saved into its purchase.
pub(crate) fn link_purchase(event: &mut SharedEvent, purchase_id: &str, tx_id: &str) {
    if let Some(purchase) = event
        .items
        .iter_mut()
        .flat_map(|item| &mut item.purchases)
        .find(|purchase| purchase.id == purchase_id)
    {
        purchase.tx_id = Some(tx_id.to_string());
        purchase.tx_owned = true;
    }
}

/// A new purchase of the event, with my payment when I bought it.
pub fn plan_add_purchase(
    book: &RateBook,
    doc: &VaultDoc,
    event_id: &str,
    draft: &SharedPurchaseDraft,
    now: &str,
) -> Result<PurchaseEdit> {
    let mut event = find_event(doc, event_id)?.clone();
    let prepared = prepare_purchase(book, &event, draft, now)?;
    let item_index = resolve_purchase_item(
        &mut event,
        draft.item_id.as_deref(),
        draft.new_item_title.as_deref(),
        draft.new_item_unit.as_deref(),
    )?;
    let (purchase, store_id) = prepared.into_purchase(uuid::Uuid::new_v4().to_string());
    let payment = match store_id.filter(|_| purchase.buyer == ME) {
        Some(store_id) => PaymentStep::Add {
            payment: shared_payment(
                &event,
                &event.items[item_index],
                &purchase,
                &store_id,
                &uuid::Uuid::new_v4().to_string(),
            ),
            purchase_id: purchase.id.clone(),
        },
        None => PaymentStep::Keep,
    };
    event.items[item_index].purchases.push(purchase);
    Ok(PurchaseEdit { event, payment })
}

/// Change a purchase; its item stays. A payment the event made follows the
/// change; an adopted payment keeps its own amount and only the number of
/// units can change.
pub fn plan_update_purchase(
    book: &RateBook,
    doc: &VaultDoc,
    event_id: &str,
    purchase_id: &str,
    draft: &SharedPurchaseDraft,
    now: &str,
) -> Result<PurchaseEdit> {
    let mut event = find_event(doc, event_id)?.clone();
    let (item_index, purchase_index) = find_purchase(&event, purchase_id)?;
    let old = event.items[item_index].purchases[purchase_index].clone();
    if old.tx_id.is_some() && !old.tx_owned {
        if !draft.qty.is_positive() {
            return Err(Error::InvalidInput("the quantity must be positive".into()));
        }
        event.items[item_index].purchases[purchase_index].qty = draft.qty;
        return Ok(PurchaseEdit {
            event,
            payment: PaymentStep::Keep,
        });
    }
    let prepared = prepare_purchase(book, &event, draft, now)?;
    let (mut purchase, store_id) = prepared.into_purchase(old.id.clone());
    let item = event.items[item_index].clone();
    let payment = match (
        old.tx_id.as_deref(),
        purchase.buyer == ME,
        store_id.as_deref(),
    ) {
        (Some(tx_id), true, Some(store_id)) => {
            purchase.tx_id = Some(tx_id.to_string());
            purchase.tx_owned = true;
            PaymentStep::Update {
                tx_id: tx_id.to_string(),
                payment: shared_payment(&event, &item, &purchase, store_id, tx_id),
            }
        }
        (Some(tx_id), false, _) => PaymentStep::Drop {
            tx_id: tx_id.to_string(),
        },
        (None, true, Some(store_id)) => PaymentStep::Add {
            payment: shared_payment(
                &event,
                &item,
                &purchase,
                store_id,
                &uuid::Uuid::new_v4().to_string(),
            ),
            purchase_id: purchase.id.clone(),
        },
        _ => PaymentStep::Keep,
    };
    event.items[item_index].purchases[purchase_index] = purchase;
    Ok(PurchaseEdit { event, payment })
}

/// Delete a purchase with the payment the event made for it; an adopted
/// payment stays in the vault, unlinked.
pub fn plan_delete_purchase(
    doc: &VaultDoc,
    event_id: &str,
    purchase_id: &str,
) -> Result<PurchaseEdit> {
    let mut event = find_event(doc, event_id)?.clone();
    let (item_index, purchase_index) = find_purchase(&event, purchase_id)?;
    let old = event.items[item_index].purchases.remove(purchase_index);
    let payment = match (old.tx_id, old.tx_owned) {
        (Some(tx_id), true) => PaymentStep::Drop { tx_id },
        _ => PaymentStep::Keep,
    };
    Ok(PurchaseEdit { event, payment })
}

/// Take a payment that is already in the vault as my purchase of the event;
/// `purchase_id` is the one id per payment (two devices that link it make one
/// record).
pub fn plan_attach_payment(
    book: &RateBook,
    doc: &VaultDoc,
    input: &AttachPaymentInput,
    purchase_id: String,
) -> Result<SharedEvent> {
    let mut event = find_event(doc, &input.event_id)?.clone();
    if !event.i_participate {
        return Err(Error::InvalidInput(
            "you do not take part in this event".into(),
        ));
    }
    let tx = doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == input.tx_id)
        .ok_or_else(|| Error::Missing("transaction".into()))?;
    let Transaction::Payment(payment) = tx else {
        return Err(Error::InvalidInput(
            "only a payment can be a purchase of an event".into(),
        ));
    };
    if shared_tx_links(doc)
        .iter()
        .any(|link| link.tx_id == input.tx_id)
    {
        return Err(Error::InvalidInput(
            "this payment belongs to an event already".into(),
        ));
    }
    if !payment.asset.is_fiat() {
        return Err(Error::InvalidInput(
            "a purchase is paid in a currency, not in an asset".into(),
        ));
    }
    let currency = payment.asset.symbol.to_lowercase();
    let at = tx.meta().timestamp.clone();
    let rate_to_event = if currency == event.currency {
        split::one()
    } else {
        book_rate(book, &currency, &event, &at)?
    };
    let qty = input
        .qty
        .filter(|qty| qty.is_positive())
        .unwrap_or_else(split::one);
    let item_index = resolve_purchase_item(
        &mut event,
        input.item_id.as_deref(),
        input.new_item_title.as_deref(),
        None,
    )?;
    let purchase = Purchase {
        id: purchase_id,
        buyer: ME.to_string(),
        qty,
        amount: payment.amount,
        currency,
        price_mode: PriceMode::Total,
        rate_to_event,
        rate_manual: false,
        tx_id: Some(input.tx_id.clone()),
        tx_owned: false,
        at,
    };
    event.items[item_index].purchases.push(purchase);
    Ok(event)
}
