//! The automation and goal scenarios announce their events from inside the
//! transaction; the workflow reactions run after the commit.

use crate::model::planning::{Schedule, TransactionDraft};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::service::AutomationDraft;
use crate::service::test_support::*;

fn salary_automation(store: &str) -> AutomationDraft {
    let Transaction::Topup(inner) = income(store, "100", "salary") else {
        unreachable!()
    };
    AutomationDraft {
        time: None,
        asks: false,
        for_month_offset: None,
        name: "Pay".into(),
        active: true,
        preset: None,
        schedule: Schedule::Monthly { day: 5 },
        starts_at: "2026-01-01T00:00:00Z".into(),
        template: TransactionDraft::Topup(inner),
        amount_mode: Default::default(),
        workday_shift: Default::default(),
        property_id: None,
    }
}

#[test]
fn deleting_an_automation_announces_it_and_the_reaction_removes_the_workflow() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let made = crate::usecases::planning::automations::create_automation(
        &mut vault,
        salary_automation(&card),
    )
    .unwrap();
    crate::usecases::planning::automations::delete_automation(&mut vault, &made.id).unwrap();
    assert!(vault.doc.workflows.is_empty());
}
