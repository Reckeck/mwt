//! Record builders for tests: each takes the minimum a test names and fills the rest
//! from the shared goal fixture, so no record is spelled out twice. They need the model
//! only; the builders that need an open vault live in the facade's `testkit` module.

use mwt_base::Amount;
use mwt_model::{
    AssetRef, Goal, KindDetails, PriceStamp, Store, StoreKind, StoreScope, TopupTx, Transaction,
    TxMeta,
};

fn amount(raw: &str) -> Amount {
    Amount::parse(raw).expect("a test amount")
}

/// A top-level store of `kind` with a neutral colour.
pub fn store(name: &str, kind: StoreKind) -> Store {
    Store::new(name, kind, "#000")
}

/// A USD salary of `amount` credited to `store_id`, priced at 1.
pub fn topup(store_id: &str, raw: &str) -> Transaction {
    let mut meta = TxMeta::new("salary");
    meta.categories = vec!["salary".into()];
    Transaction::Topup(TopupTx {
        meta,
        store_id: store_id.into(),
        asset: AssetRef::fiat("USD", "US Dollar"),
        amount: amount(raw),
        fee: None,
        from: None,
        details: KindDetails::default(),
        price: PriceStamp {
            price_usd: Some(amount("1")),
            ..PriceStamp::default()
        },
    })
}

/// The goal the full fixture document carries (a copy of `fixtures::goal` of the facade, which
/// cannot depend on this crate's builders without touching `model/`).
fn fixture_goal() -> Goal {
    Goal {
        id: "goal".into(),
        name: "Car".into(),
        active: true,
        target_amount: amount("1000000"),
        currency: "rub".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: Some("2027-01-01T00:00:00Z".into()),
        priority: 5,
        monthly_commitment: Some(amount("50000")),
        scope: StoreScope::store("bank", false),
        created_at: "2026-01-01T00:00:00Z".into(),
        scenario_id: None,
        expected_return: None,
    }
}

/// The fixture goal under the given name and target.
pub fn goal(name: &str, target: &str) -> Goal {
    Goal {
        name: name.into(),
        target_amount: amount(target),
        ..fixture_goal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_fill_the_rest_from_the_shared_fixtures() {
        let wallet = store(" Bank ", StoreKind::Bank);
        assert_eq!(wallet.name, "Bank");
        assert_eq!(wallet.kind, StoreKind::Bank);
        let income = topup(&wallet.id, "25");
        assert_eq!(income.meta().comment, "salary");
        let saving = goal("Trip", "500");
        assert_eq!(saving.name, "Trip");
        assert_eq!(saving.target_amount, amount("500"));
        assert_eq!(saving.priority, fixture_goal().priority);
    }
}
