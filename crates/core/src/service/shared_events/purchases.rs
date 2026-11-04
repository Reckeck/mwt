//! Purchases of an event: what was really bought, by whom, in which currency.
//! A purchase of mine is also a real payment in the vault; an event deletes
//! the payments it made and only unlinks the ones it adopted.

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{AttachPaymentInput, SharedPurchaseDraft};
use crate::model::shared_event::{EventItem, ItemSplit, ME, PriceMode, Purchase, SharedEvent};
use crate::model::store::AssetRef;
use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, Transaction, TxMeta};
use crate::pricing::history::{secs_or_now, unix_secs};

use super::super::UnlockedVault;
use super::split;
use super::view::SharedEventView;

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

/// An instant for a date or an RFC3339 time typed by the user.
pub(super) fn normalize_moment(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if let Ok(day) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(format!("{day}T12:00:00Z"));
    }
    if unix_secs(raw).is_some() {
        return Ok(raw.to_string());
    }
    Err(Error::InvalidInput(
        "a date is YYYY-MM-DD or an RFC3339 time".into(),
    ))
}

/// When a purchase happened if nobody said: now when that is inside the
/// event's days (or it has none), else the first day.
fn default_moment(event: &SharedEvent) -> String {
    let now = crate::model::vault::now_iso();
    let Some(start) = event.starts_on.as_deref() else {
        return now;
    };
    let today: String = now.chars().take(10).collect();
    let end = event.ends_on.as_deref().unwrap_or(start);
    // Inside the days, or before them (a payment cannot be dated in the
    // future): now. After them: the first day.
    if today.as_str() <= end {
        now
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

impl UnlockedVault {
    fn prepare_purchase(
        &self,
        event: &SharedEvent,
        draft: &SharedPurchaseDraft,
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
            None => default_moment(event),
        };
        let (rate_to_event, rate_manual) = if currency == event.currency {
            (split::one(), false)
        } else {
            match draft.rate_to_event {
                Some(rate) if rate.is_positive() => (rate, true),
                Some(_) => {
                    return Err(Error::InvalidInput("the rate must be positive".into()));
                }
                None => {
                    let found = self.rate_book().convert(
                        split::one(),
                        &currency,
                        &event.currency,
                        secs_or_now(&at),
                    );
                    match found {
                        Some(rate) if rate.is_positive() => (split::round_places(rate, 6), false),
                        _ => return Err(Error::FxMissing(currency.to_uppercase())),
                    }
                }
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
        meta.categories = vec![super::item_category(event, item)];
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

    /// Remove a transaction without saving; the caller saves.
    pub(super) fn drop_shared_tx(&mut self, tx_id: &str) {
        self.doc.transactions.retain(|tx| tx.meta().id != tx_id);
        self.set_property_link(tx_id, None);
    }

    pub fn add_shared_purchase(
        &mut self,
        event_id: &str,
        draft: SharedPurchaseDraft,
    ) -> Result<SharedEventView> {
        let mut event = self.find_shared_event(event_id)?.clone();
        let prepared = self.prepare_purchase(&event, &draft)?;
        let item_index = Self::resolve_purchase_item(
            &mut event,
            draft.item_id.as_deref(),
            draft.new_item_title.as_deref(),
            draft.new_item_unit.as_deref(),
        )?;
        let (mut purchase, store_id) = prepared.into_purchase(uuid::Uuid::new_v4().to_string());
        if let Some(store_id) = store_id.filter(|_| purchase.buyer == ME) {
            let tx_id = uuid::Uuid::new_v4().to_string();
            let payment = Self::shared_payment(
                &event,
                &event.items[item_index],
                &purchase,
                &store_id,
                &tx_id,
            );
            // Nothing of the event is saved if the vault refuses the payment.
            let saved = self.add_transaction(payment)?;
            purchase.tx_id = Some(saved.meta().id.clone());
            purchase.tx_owned = true;
        }
        event.items[item_index].purchases.push(purchase);
        *self.find_shared_event_mut(event_id)? = event;
        self.persist_event(event_id)?;
        self.shared_event_view(event_id)
    }

    /// Change a purchase; its item stays. A payment the event made follows the
    /// change; an adopted payment keeps its own amount and only the number of
    /// units can change.
    pub fn update_shared_purchase(
        &mut self,
        event_id: &str,
        purchase_id: &str,
        draft: SharedPurchaseDraft,
    ) -> Result<SharedEventView> {
        let mut event = self.find_shared_event(event_id)?.clone();
        let (item_index, purchase_index) = find_purchase(&event, purchase_id)?;
        let old = event.items[item_index].purchases[purchase_index].clone();
        if old.tx_id.is_some() && !old.tx_owned {
            if !draft.qty.is_positive() {
                return Err(Error::InvalidInput("the quantity must be positive".into()));
            }
            event.items[item_index].purchases[purchase_index].qty = draft.qty;
            *self.find_shared_event_mut(event_id)? = event;
            self.persist_event(event_id)?;
            return self.shared_event_view(event_id);
        }
        let prepared = self.prepare_purchase(&event, &draft)?;
        let (mut purchase, store_id) = prepared.into_purchase(old.id.clone());
        let item = event.items[item_index].clone();
        match (
            old.tx_id.as_deref(),
            purchase.buyer == ME,
            store_id.as_deref(),
        ) {
            (Some(tx_id), true, Some(store_id)) => {
                let payment = Self::shared_payment(&event, &item, &purchase, store_id, tx_id);
                self.update_transaction_unsynced(tx_id, payment)?;
                purchase.tx_id = Some(tx_id.to_string());
                purchase.tx_owned = true;
            }
            (Some(tx_id), false, _) => {
                self.drop_shared_tx(tx_id);
                self.recompute_holdings();
            }
            (None, true, Some(store_id)) => {
                let tx_id = uuid::Uuid::new_v4().to_string();
                let payment = Self::shared_payment(&event, &item, &purchase, store_id, &tx_id);
                let saved = self.add_transaction(payment)?;
                purchase.tx_id = Some(saved.meta().id.clone());
                purchase.tx_owned = true;
            }
            _ => {}
        }
        event.items[item_index].purchases[purchase_index] = purchase;
        *self.find_shared_event_mut(event_id)? = event;
        self.persist_event(event_id)?;
        self.shared_event_view(event_id)
    }

    /// Delete a purchase with the payment the event made for it; an adopted
    /// payment stays in the vault, unlinked.
    pub fn delete_shared_purchase(
        &mut self,
        event_id: &str,
        purchase_id: &str,
    ) -> Result<SharedEventView> {
        let event = self.find_shared_event(event_id)?;
        let (item_index, purchase_index) = find_purchase(event, purchase_id)?;
        let old = event.items[item_index].purchases[purchase_index].clone();
        if let (Some(tx_id), true) = (&old.tx_id, old.tx_owned) {
            self.drop_shared_tx(tx_id);
            self.recompute_holdings();
        }
        self.find_shared_event_mut(event_id)?.items[item_index]
            .purchases
            .remove(purchase_index);
        self.persist_event(event_id)?;
        self.shared_event_view(event_id)
    }

    /// Take a payment that is already in the vault as my purchase of the event.
    pub fn attach_payment_to_shared_event(
        &mut self,
        input: AttachPaymentInput,
    ) -> Result<SharedEventView> {
        let mut event = self.find_shared_event(&input.event_id)?.clone();
        if !event.i_participate {
            return Err(Error::InvalidInput(
                "you do not take part in this event".into(),
            ));
        }
        let tx = self
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == input.tx_id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        let Transaction::Payment(payment) = tx else {
            return Err(Error::InvalidInput(
                "only a payment can be a purchase of an event".into(),
            ));
        };
        if self
            .shared_tx_links()
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
            match self.rate_book().convert(
                split::one(),
                &currency,
                &event.currency,
                secs_or_now(&at),
            ) {
                Some(rate) if rate.is_positive() => split::round_places(rate, 6),
                _ => return Err(Error::FxMissing(currency.to_uppercase())),
            }
        };
        let total = payment.amount;
        let qty = input
            .qty
            .filter(|qty| qty.is_positive())
            .unwrap_or_else(split::one);
        let item_index = Self::resolve_purchase_item(
            &mut event,
            input.item_id.as_deref(),
            input.new_item_title.as_deref(),
            None,
        )?;
        let purchase = Purchase {
            // One id per payment: two devices that link it make one record.
            id: crate::service::sync_ids::payment_link_id(&input.tx_id),
            buyer: ME.to_string(),
            qty,
            amount: total,
            currency,
            price_mode: PriceMode::Total,
            rate_to_event,
            rate_manual: false,
            tx_id: Some(input.tx_id.clone()),
            tx_owned: false,
            at,
        };
        event.items[item_index].purchases.push(purchase);
        *self.find_shared_event_mut(&input.event_id)? = event;
        self.persist_event(&input.event_id)?;
        self.shared_event_view(&input.event_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_contract::{SharedEventDraft, SharedItemDraft, SharedParticipantDraft};
    use crate::model::shared_event::{ItemPlan, SharedEventStatus};
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    struct Setup {
        _dir: tempfile::TempDir,
        vault: UnlockedVault,
        wallet: String,
        event: String,
        petya: String,
    }

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "RUB")
    }

    fn setup_event() -> Setup {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("rub");
        let wallet = store(&mut vault, "Тинькофф", StoreKind::Bank, None);
        vault
            .add_transaction(income_of(&wallet, rub(), "50000", "0.0109", "salary"))
            .unwrap();
        let view = vault
            .create_shared_event(SharedEventDraft {
                title: "Шашлыки".into(),
                currency: "rub".into(),
                participants: vec![SharedParticipantDraft {
                    name: "Петя".into(),
                    ..SharedParticipantDraft::default()
                }],
                ..SharedEventDraft::default()
            })
            .unwrap();
        let petya = view.people[1].id.clone();
        Setup {
            _dir: dir,
            vault,
            wallet,
            event: view.event.id,
            petya,
        }
    }

    fn buy(setup: &Setup, item: Option<&str>, buyer: &str, total: &str) -> SharedPurchaseDraft {
        SharedPurchaseDraft {
            item_id: item.map(str::to_string),
            new_item_title: Some("Уголь".into()),
            buyer: buyer.into(),
            qty: amount("1"),
            amount: amount(total),
            currency: "rub".into(),
            price_mode: PriceMode::Total,
            store_id: Some(setup.wallet.clone()),
            ..SharedPurchaseDraft::default()
        }
    }

    fn rub_balance(setup: &Setup) -> Amount {
        holding_of(&setup.vault, &setup.wallet, &rub())
    }

    #[test]
    fn my_purchase_makes_a_payment_and_deleting_it_gives_the_money_back() {
        let mut setup = setup_event();
        let draft = buy(&setup, None, "", "1800");
        let view = setup
            .vault
            .add_shared_purchase(&setup.event, draft)
            .unwrap();
        assert_eq!(view.status, SharedEventStatus::InProgress);
        assert_eq!(view.totals.spent, amount("1800"));
        assert_eq!(rub_balance(&setup), amount("48200"));
        let purchase = view.event.items[0].purchases[0].clone();
        assert!(purchase.tx_owned);
        let tx_id = purchase.tx_id.clone().unwrap();
        let links = setup.vault.shared_tx_links();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].tx_id, tx_id);
        assert_eq!(links[0].link.event_id, setup.event);
        assert_eq!(
            links[0].link.purchase_id.as_deref(),
            Some(purchase.id.as_str())
        );
        assert_eq!(view.purchases[0].my_part, amount("900"));

        let view = setup
            .vault
            .delete_shared_purchase(&setup.event, &purchase.id)
            .unwrap();
        assert_eq!(view.totals.spent, amount("0"));
        assert_eq!(rub_balance(&setup), amount("50000"));
        assert!(
            setup
                .vault
                .doc
                .transactions
                .iter()
                .all(|tx| tx.meta().id != tx_id)
        );
    }

    #[test]
    fn someone_elses_purchase_leaves_my_wallets_alone() {
        let mut setup = setup_event();
        let before = setup.vault.doc.transactions.len();
        let draft = buy(&setup, None, &setup.petya.clone(), "500");
        let view = setup
            .vault
            .add_shared_purchase(&setup.event, draft)
            .unwrap();
        assert_eq!(setup.vault.doc.transactions.len(), before);
        assert_eq!(rub_balance(&setup), amount("50000"));
        assert!(view.event.items[0].purchases[0].tx_id.is_none());
        // Petya paid 500, half of it is mine: I owe him 250
        let petya = view
            .people
            .iter()
            .find(|row| row.id == setup.petya)
            .unwrap();
        assert_eq!(petya.saldo, amount("-250"));
    }

    #[test]
    fn a_refused_payment_leaves_the_event_untouched() {
        let mut setup = setup_event();
        let mut draft = buy(&setup, None, "", "100");
        draft.store_id = Some("no-such-wallet".into());
        let result = setup.vault.add_shared_purchase(&setup.event, draft);
        assert!(result.is_err());
        let view = setup.vault.shared_event_view(&setup.event).unwrap();
        assert!(view.event.items.is_empty());
        // I cannot buy without a wallet
        let mut no_wallet = buy(&setup, None, "", "100");
        no_wallet.store_id = None;
        assert!(
            setup
                .vault
                .add_shared_purchase(&setup.event, no_wallet)
                .is_err()
        );
    }

    #[test]
    fn a_purchase_in_another_currency_gets_a_rate_from_the_cache_or_by_hand() {
        let mut setup = setup_event();
        let mut draft = buy(&setup, None, &setup.petya.clone(), "10");
        draft.currency = "usd".into();
        // no rate known for usd -> rub
        let err = setup
            .vault
            .add_shared_purchase(&setup.event, draft.clone())
            .unwrap_err();
        assert_eq!(err.code(), "FX_MISSING");
        // by hand
        draft.rate_to_event = Some(amount("91.37"));
        let view = setup
            .vault
            .add_shared_purchase(&setup.event, draft)
            .unwrap();
        let purchase = &view.event.items[0].purchases[0];
        assert!(purchase.rate_manual);
        assert_eq!(view.totals.spent, amount("913.70"));
        // from the cache: 1 USD = 90 RUB
        setup.vault.doc.fx_cache.push(crate::model::vault::FxPoint {
            currency: "rub".into(),
            per_usd: amount("90"),
            ts: "2000-01-01T00:00:00Z".into(),
            source: "test".into(),
        });
        let mut cached = buy(&setup, None, &setup.petya.clone(), "10");
        cached.currency = "usd".into();
        let view = setup
            .vault
            .add_shared_purchase(&setup.event, cached)
            .unwrap();
        let second = view.event.items[1].purchases[0].clone();
        assert!(!second.rate_manual);
        assert_eq!(second.rate_to_event, amount("90"));
        // changing the cache afterwards does not rewrite the purchase
        setup.vault.doc.fx_cache[0].per_usd = amount("100");
        let again = setup.vault.shared_event_view(&setup.event).unwrap();
        assert_eq!(again.totals.spent, amount("1813.70"));
    }

    #[test]
    fn editing_a_purchase_follows_into_the_payment() {
        let mut setup = setup_event();
        let draft = buy(&setup, None, "", "1000");
        let view = setup
            .vault
            .add_shared_purchase(&setup.event, draft)
            .unwrap();
        let purchase = view.event.items[0].purchases[0].clone();
        let mut edit = buy(&setup, None, "", "1500");
        edit.new_item_title = None;
        let view = setup
            .vault
            .update_shared_purchase(&setup.event, &purchase.id, edit)
            .unwrap();
        assert_eq!(view.totals.spent, amount("1500"));
        assert_eq!(rub_balance(&setup), amount("48500"));
        // the payment keeps its id, so the link survives the edit
        assert_eq!(
            setup.vault.shared_tx_links()[0].tx_id,
            purchase.tx_id.clone().unwrap()
        );
        // handing the purchase to Petya takes my payment away
        let mut handed = buy(&setup, None, &setup.petya.clone(), "1500");
        handed.new_item_title = None;
        let view = setup
            .vault
            .update_shared_purchase(&setup.event, &purchase.id, handed)
            .unwrap();
        assert_eq!(rub_balance(&setup), amount("50000"));
        assert!(view.event.items[0].purchases[0].tx_id.is_none());
    }

    fn payment_rub(store: &str, raw: &str) -> Transaction {
        match payment(store, raw, "shop") {
            Transaction::Payment(inner) => Transaction::Payment(PaymentTx {
                asset: rub(),
                ..inner
            }),
            other => other,
        }
    }

    #[test]
    fn an_existing_payment_is_adopted_and_only_unlinked_on_delete() {
        let mut setup = setup_event();
        let payment = setup
            .vault
            .add_transaction(payment_rub(&setup.wallet, "700"))
            .unwrap();
        let tx_id = payment.meta().id.clone();
        let attach = |setup: &mut Setup| {
            setup
                .vault
                .attach_payment_to_shared_event(AttachPaymentInput {
                    event_id: setup.event.clone(),
                    tx_id: tx_id.clone(),
                    new_item_title: Some("Мангал".into()),
                    ..AttachPaymentInput::default()
                })
        };
        let view = attach(&mut setup).unwrap();
        assert_eq!(view.totals.spent, amount("700"));
        assert!(!view.event.items[0].purchases[0].tx_owned);
        // the same payment cannot be taken twice
        assert!(attach(&mut setup).is_err());
        let purchase_id = view.event.items[0].purchases[0].id.clone();
        setup
            .vault
            .delete_shared_purchase(&setup.event, &purchase_id)
            .unwrap();
        assert!(setup.vault.shared_tx_links().is_empty());
        assert!(
            setup
                .vault
                .doc
                .transactions
                .iter()
                .any(|tx| tx.meta().id == tx_id),
            "the payment is still there"
        );
        // deleting the event with a linked payment keeps it too
        attach(&mut setup).unwrap();
        setup.vault.delete_shared_event(&setup.event).unwrap();
        assert!(
            setup
                .vault
                .doc
                .transactions
                .iter()
                .any(|tx| tx.meta().id == tx_id)
        );
    }

    #[test]
    fn deleting_the_event_keeps_every_real_operation() {
        let mut setup = setup_event();
        let draft = buy(&setup, None, "", "1200");
        setup
            .vault
            .add_shared_purchase(&setup.event, draft)
            .unwrap();
        assert_eq!(rub_balance(&setup), amount("48800"));
        setup.vault.delete_shared_event(&setup.event).unwrap();
        assert_eq!(rub_balance(&setup), amount("48800"));
        assert!(setup.vault.shared_tx_links().is_empty());
    }

    #[test]
    fn totals_compare_the_plan_with_what_was_bought() {
        let mut setup = setup_event();
        let item = setup
            .vault
            .add_shared_item(
                &setup.event,
                SharedItemDraft {
                    title: "Пиво".into(),
                    plan: Some(ItemPlan {
                        qty: amount("6"),
                        price: amount("1200"),
                        price_mode: PriceMode::Total,
                    }),
                    ..SharedItemDraft::default()
                },
            )
            .unwrap()
            .event
            .items[0]
            .id
            .clone();
        // Not bought yet: in the plan, but not in the difference.
        setup
            .vault
            .add_shared_item(
                &setup.event,
                SharedItemDraft {
                    title: "Соус".into(),
                    plan: Some(ItemPlan {
                        qty: amount("2"),
                        price: amount("400"),
                        price_mode: PriceMode::Total,
                    }),
                    ..SharedItemDraft::default()
                },
            )
            .unwrap();
        let mut draft = buy(&setup, Some(&item), "", "1800");
        draft.qty = amount("8");
        setup
            .vault
            .add_shared_purchase(&setup.event, draft)
            .unwrap();
        // Off the plan: spent, but not in the difference either.
        let view = setup
            .vault
            .add_shared_purchase(&setup.event, buy(&setup, None, "", "500"))
            .unwrap();
        assert_eq!(view.totals.planned, amount("1600"));
        assert_eq!(view.totals.spent, amount("2300"));
        assert_eq!(view.totals.diff, amount("600"));
        assert_eq!(view.items[0].diff_money, amount("600"));
        assert_eq!(view.items[0].diff_qty, amount("2"));
    }
}
