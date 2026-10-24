//! Operations that belong to a property (plan G4): rent received for a flat,
//! fuel and insurance for a car. The link lives beside the operation
//! (`VaultDoc::tx_properties`), and the property's summary sums it up.

use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::TxProperty;
use crate::pricing::history::secs_or_now;
use crate::{Amount, Error, Result};

use super::UnlockedVault;

/// The summary's look back: a year.
const YEAR_SECS: i64 = 365 * 86_400;

/// What a property brought and cost over the last year, in the base currency.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PropertyYear {
    pub rent: Amount,
    pub expenses: Amount,
    /// What its value lost over the year; zero when it grew.
    pub wear: Amount,
}

impl UnlockedVault {
    /// Every operation's property, for the operation form.
    pub fn tx_property_links(&self) -> Vec<TxProperty> {
        self.doc.tx_properties.clone()
    }

    pub fn property_of_tx(&self, tx_id: &str) -> Option<&str> {
        self.doc
            .tx_properties
            .iter()
            .find(|link| link.tx_id == tx_id)
            .map(|link| link.property_id.as_str())
    }

    /// A link may point only at a live property.
    pub(crate) fn check_property_link(&self, property_id: Option<&str>) -> Result<()> {
        let Some(id) = property_id else {
            return Ok(());
        };
        let store = self.live_store(id)?;
        if store.kind != StoreKind::Property {
            return Err(Error::InvalidInput(
                "an operation is linked only to property".into(),
            ));
        }
        Ok(())
    }

    /// Link the operation to a property, or unlink it with `None`. Not persisted.
    pub(crate) fn set_property_link(&mut self, tx_id: &str, property_id: Option<&str>) {
        self.doc.tx_properties.retain(|link| link.tx_id != tx_id);
        if let Some(id) = property_id {
            self.doc.tx_properties.push(TxProperty {
                tx_id: tx_id.to_string(),
                property_id: id.to_string(),
            });
        }
    }

    /// The rent, the expenses and the loss of value of a property over the
    /// year before `now`. Expenses are the operations linked to it, its
    /// utility bills and the payments into the loans it secures.
    pub(crate) fn property_year(&self, store_id: &str, now: i64) -> PropertyYear {
        let (base, _) = self.base_and_fx();
        let book = self.rate_book();
        let from = now - YEAR_SECS;
        let in_year = |tx: &Transaction| {
            let secs = secs_or_now(&tx.meta().timestamp);
            (from..=now).contains(&secs)
        };
        let to_base = |tx: &Transaction, store: Option<&str>, sign: i8| {
            let secs = secs_or_now(&tx.meta().timestamp);
            tx.legs()
                .iter()
                .filter(|leg| store.is_none_or(|id| leg.store_id == id))
                .filter(|leg| match sign {
                    1 => leg.amount.is_positive(),
                    _ => leg.amount.is_negative(),
                })
                .fold(Amount::ZERO, |sum, leg| {
                    let amount = leg.amount.abs();
                    let value = book
                        .convert(amount, &leg.asset.symbol, &base, secs)
                        .or_else(|| {
                            let usd = book.price_at(&leg.asset.key(), secs)?;
                            book.convert(amount * usd, "usd", &base, secs)
                        })
                        .unwrap_or(Amount::ZERO);
                    sum + value
                })
        };
        let mut out = PropertyYear::default();
        let mut counted: Vec<&str> = Vec::new();
        for tx in &self.doc.transactions {
            if self.property_of_tx(&tx.meta().id) != Some(store_id) || !in_year(tx) {
                continue;
            }
            match tx {
                // Rent comes to a money account; buying the thing itself is no rent.
                Transaction::Topup(topup) if topup.store_id != store_id => {
                    out.rent += to_base(tx, None, 1);
                }
                Transaction::Topup(_) => {}
                _ => out.expenses += to_base(tx, None, -1),
            }
            counted.push(tx.meta().id.as_str());
        }
        let items = self
            .doc
            .utility_items
            .iter()
            .filter(|item| item.property_id == store_id);
        for item in items {
            for payment in self.item_payments(item) {
                let tx = Transaction::Payment(payment.clone());
                if in_year(&tx) && !counted.contains(&payment.meta.id.as_str()) {
                    out.expenses += to_base(&tx, None, -1);
                }
            }
        }
        let loans: Vec<&str> = self
            .doc
            .stores
            .iter()
            .filter(|loan| loan.secured_by.as_deref() == Some(store_id))
            .map(|loan| loan.id.as_str())
            .collect();
        for tx in &self.doc.transactions {
            if !in_year(tx) || counted.contains(&tx.meta().id.as_str()) {
                continue;
            }
            for loan in &loans {
                out.expenses += to_base(tx, Some(loan), 1);
            }
        }
        out.wear = self.property_wear(store_id, from, now);
        out
    }

    /// How much the property's value fell between `from` and `now`, in the
    /// base currency; zero without a value curve or when it grew.
    fn property_wear(&self, store_id: &str, from: i64, now: i64) -> Amount {
        let (base, _) = self.base_and_fx();
        let book = self.rate_book();
        let mut lost = Amount::ZERO;
        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == store_id)
        {
            let Some(curve) = self.valuation_curve(&holding.asset.key()) else {
                continue;
            };
            let (Some(then), Some(today)) = (curve.value_at(from), curve.value_at(now)) else {
                continue;
            };
            let drop = (then - today) * holding.quantity;
            if drop.is_positive() {
                lost += book
                    .convert(drop, &curve.currency, &base, now)
                    .unwrap_or(Amount::ZERO);
            }
        }
        lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::planning::{Schedule, TransactionDraft};
    use crate::model::store::{AssetRef, PropertyCategory, PropertyInfo};
    use crate::model::transaction::{KindDetails, PriceStamp, TopupTx};
    use crate::model::vault::now_iso;
    use crate::pricing::history::unix_secs;
    use crate::service::StoreExtras;
    use crate::service::automations::AutomationDraft;
    use crate::service::test_support::*;

    fn property(vault: &mut UnlockedVault, category: PropertyCategory) -> String {
        vault
            .create_store_with(
                &format!("{category:?}"),
                StoreKind::Property,
                "#555",
                "",
                None,
                StoreExtras {
                    property: Some(PropertyInfo {
                        category,
                        ..PropertyInfo::default()
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    fn thing(symbol: &str) -> AssetRef {
        AssetRef {
            symbol: symbol.into(),
            name: symbol.into(),
            coingecko_id: None,
            chain: None,
            contract: None,
            quote: symbol.into(),
        }
    }

    fn buy(vault: &mut UnlockedVault, store_id: &str, symbol: &str, price: &str, when: &str) {
        let mut tx = Transaction::Topup(TopupTx {
            meta: meta("bought", &["income_other"]),
            store_id: store_id.to_string(),
            asset: thing(symbol),
            amount: amount("1"),
            fee: None,
            from: None,
            details: KindDetails {
                unit_price: Some(amount(price)),
                ..KindDetails::default()
            },
            price: PriceStamp::default(),
        });
        at(&mut tx, when);
        vault.add_transaction(tx).unwrap();
    }

    fn days_ago(days: i64) -> String {
        crate::calendar::iso(unix_secs(&now_iso()).unwrap() - days * 86_400)
    }

    #[test]
    fn a_link_follows_the_operation() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let flat = property(&mut vault, PropertyCategory::Flat);
        let mut rent = income(&card, "500", "rent");
        rent.meta_mut().categories = vec!["rent_income".into()];
        let saved = vault.add_transaction_for(rent, Some(&flat)).unwrap();
        let id = saved.meta().id.clone();
        assert_eq!(vault.property_of_tx(&id), Some(flat.as_str()));
        // A plain edit keeps the link; an edit with `None` drops it.
        vault.update_transaction(&id, saved.clone()).unwrap();
        assert_eq!(vault.property_of_tx(&id), Some(flat.as_str()));
        vault
            .update_transaction_for(&id, saved.clone(), None)
            .unwrap();
        assert_eq!(vault.property_of_tx(&id), None);
        vault
            .update_transaction_for(&id, saved, Some(&flat))
            .unwrap();
        vault.delete_transaction(&id).unwrap();
        assert!(vault.tx_property_links().is_empty());
    }

    #[test]
    fn only_a_live_property_takes_a_link() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let before = vault.doc.transactions.len();
        assert!(
            vault
                .add_transaction_for(income(&card, "1", "x"), Some(&card))
                .is_err()
        );
        assert!(
            vault
                .add_transaction_for(income(&card, "1", "x"), Some("nope"))
                .is_err()
        );
        assert_eq!(vault.doc.transactions.len(), before);
    }

    /// The old rent preset credited money to the property itself; the
    /// existing rule already refuses that.
    #[test]
    fn rent_into_the_property_itself_is_refused() {
        let (_dir, mut vault) = empty_vault();
        let flat = property(&mut vault, PropertyCategory::Flat);
        let mut rent = income(&flat, "500", "rent");
        rent.meta_mut().categories = vec!["rent_income".into()];
        assert!(vault.add_transaction(rent).is_err());
    }

    #[test]
    fn an_automation_links_what_it_books() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let flat = property(&mut vault, PropertyCategory::Flat);
        let Transaction::Topup(mut inner) = income(&card, "500", "rent") else {
            unreachable!()
        };
        inner.meta.categories = vec!["rent_income".into()];
        let draft = |property_id: Option<String>| AutomationDraft {
            name: "Rent".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 5 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(inner.clone()),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id,
        };
        assert!(vault.create_automation(draft(Some(card.clone()))).is_err());
        let rent = vault.create_automation(draft(Some(flat.clone()))).unwrap();
        assert_eq!(rent.property_id.as_deref(), Some(flat.as_str()));
        let pass = vault
            .run_due_workflows(unix_secs("2026-02-20T12:00:00Z").unwrap(), "UTC")
            .unwrap();
        assert_eq!(pass.created.len(), 2);
        let links = vault.tx_property_links();
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|link| link.property_id == flat));
    }

    #[test]
    fn the_summary_counts_rent_and_its_yield() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let flat = property(&mut vault, PropertyCategory::Flat);
        buy(&mut vault, &flat, "FLAT", "100000", &days_ago(400));
        for days in [20, 50, 400] {
            let mut rent = income(&card, "500", "rent");
            rent.meta_mut().categories = vec!["rent_income".into()];
            at(&mut rent, &days_ago(days));
            vault.add_transaction_for(rent, Some(&flat)).unwrap();
        }
        let mut repair = payment(&card, "300", "paint");
        repair.meta_mut().categories = vec!["repair".into()];
        at(&mut repair, &days_ago(10));
        vault.add_transaction_for(repair, Some(&flat)).unwrap();

        let summary = vault.property_summary(&flat, "UTC").unwrap();
        assert_eq!(summary.rent_12m_base, amount("1000"), "a year back only");
        assert_eq!(summary.expenses_12m_base, amount("300"));
        let value = summary.value_base;
        assert!(value.is_positive());
        assert_eq!(
            summary.rent_yield_pct,
            Some((amount("1000") / value * amount("100")).round(2))
        );
        assert_eq!(summary.ownership_month_base, None, "a flat is no car");
    }

    #[test]
    fn a_car_costs_its_expenses_and_its_wear() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let car = property(&mut vault, PropertyCategory::Car);
        buy(&mut vault, &car, "CAR", "10000", &days_ago(800));
        let mut fuel = payment(&card, "1200", "fuel");
        fuel.meta_mut().categories = vec!["fuel".into()];
        at(&mut fuel, &days_ago(30));
        vault.add_transaction_for(fuel, Some(&car)).unwrap();

        let summary = vault.property_summary(&car, "UTC").unwrap();
        assert_eq!(summary.expenses_12m_base, amount("1200"));
        assert!(summary.wear_12m_base.is_positive(), "a car loses value");
        // wear_12m_base is rounded, the month cost divides the exact wear: a cent apart at most.
        let month = summary
            .ownership_month_base
            .expect("a car has a month cost");
        let expected = (amount("1200") + summary.wear_12m_base) / amount("12");
        assert!(
            (month - expected).abs() <= amount("0.01"),
            "{month:?} vs {expected:?}"
        );
        assert_eq!(summary.rent_yield_pct, None);
    }
}
