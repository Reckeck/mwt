//! Record builders for tests: each takes the minimum a test names and fills the
//! rest from the shared fixtures (`model::fixtures`, `service::test_support`),
//! so no record is spelled out twice. Compiled under `cfg(test)` and the
//! `testkit` feature, like both of those.

use crate::model::fixtures;
use crate::model::planning::Goal;
use crate::model::store::{Store, StoreKind};
use crate::model::transaction::Transaction;
use crate::service::test_support;

/// A top-level store of `kind` with a neutral colour.
pub fn store(name: &str, kind: StoreKind) -> Store {
    Store::new(name, kind, "#000")
}

/// A USD salary of `amount` credited to `store_id`, priced at 1.
pub fn topup(store_id: &str, amount: &str) -> Transaction {
    test_support::income(store_id, amount, "salary")
}

/// The fixture goal under the given name and target.
pub fn goal(name: &str, target: &str) -> Goal {
    Goal {
        name: name.into(),
        target_amount: test_support::amount(target),
        ..fixtures::goal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::VaultDoc;
    use crate::service::UnlockedVault;

    #[test]
    fn builders_fill_the_rest_from_the_shared_fixtures() {
        let wallet = store(" Bank ", StoreKind::Bank);
        assert_eq!(wallet.name, "Bank");
        assert_eq!(wallet.kind, StoreKind::Bank);
        let income = topup(&wallet.id, "25");
        assert_eq!(income.meta().comment, "salary");
        let saving = goal("Trip", "500");
        assert_eq!(saving.name, "Trip");
        assert_eq!(saving.target_amount, test_support::amount("500"));
        assert_eq!(saving.priority, fixtures::goal().priority);
    }

    #[test]
    fn built_records_sit_in_an_in_memory_vault() {
        let wallet = store("Bank", StoreKind::Bank);
        let income = topup(&wallet.id, "25");
        let mut doc = VaultDoc::default();
        doc.stores.push(wallet);
        doc.transactions.push(income);
        doc.goals.push(goal("Trip", "500"));
        let vault = UnlockedVault::in_memory(doc);
        assert_eq!(vault.doc.stores.len(), 1);
        assert_eq!(vault.doc.transactions.len(), 1);
        assert_eq!(vault.doc.goals[0].name, "Trip");
    }
}
