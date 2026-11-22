//! Tests of the adjustments area that need the vault (`UnlockedVault`): moved from
//! `money/adjustments_tests.rs` with their assertions unchanged (crate-split T13).

use crate::Amount;
use crate::ipc_contract::AdjustmentDraftInput;
use crate::model::store::AssetRef;
use crate::model::store::StoreKind;
use crate::model::transaction::{AdjustmentLine, PriceStamp, Transaction};
use crate::model::transaction::{AdjustmentTx, TxMeta};
use crate::model::vault::now_iso;
use crate::money::adjustments::{balances_at, complete_adjustment};
use crate::money::worth::FlowClass;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::usecases::money::adjustments::adjustment_draft;

fn correction(store_id: &str, lines: &[(AssetRef, &str)], ts: &str) -> Transaction {
    let mut meta = TxMeta::new("Сверка");
    meta.timestamp = ts.into();
    Transaction::Adjustment(AdjustmentTx {
        meta,
        store_id: store_id.into(),
        lines: lines
            .iter()
            .map(|(asset, actual)| AdjustmentLine {
                asset: asset.clone(),
                before: Amount::ZERO,
                after: amount(actual),
                price: PriceStamp::default(),
            })
            .collect(),
        fee: None,
    })
}

#[test]
fn a_correction_sets_the_balance_and_remembers_what_was_there() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    vault.add_transaction(salary).unwrap();

    let saved = vault
        .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
        .unwrap();
    let Transaction::Adjustment(inner) = &saved else {
        panic!("a correction");
    };
    assert_eq!(inner.lines[0].before, amount("100"));
    assert_eq!(inner.lines[0].delta(), amount("-20"));
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("80"));
}

#[test]
fn a_correction_is_neither_income_nor_spending_nor_a_sale() {
    let (_dir, mut vault) = setup();
    let wallet = vault.doc.stores[1].id.clone();
    let mut buy = income_of(&wallet, btc(), "1", "30000", "bought");
    at(&mut buy, "2026-01-01T00:00:00Z");
    vault.add_transaction(buy).unwrap();
    let before = vault.realized_pnl().total_base;

    let tx = vault
        .add_transaction(correction(
            &wallet,
            &[(btc(), "0.5")],
            "2026-02-01T00:00:00Z",
        ))
        .unwrap();
    assert_eq!(vault.classify_flow(&tx), FlowClass::Ignored);
    assert_eq!(vault.realized_pnl().total_base, before, "nothing was sold");
    assert_eq!(holding_of(&vault, &wallet, &btc()), amount("0.5"));
}

#[test]
fn a_back_dated_correction_sees_the_balance_of_its_moment() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    for (sum, ts) in [
        ("100", "2026-03-01T10:00:00Z"),
        ("50", "2026-03-10T10:00:00Z"),
    ] {
        let mut tx = income(&cash, sum, "salary");
        at(&mut tx, ts);
        vault.add_transaction(tx).unwrap();
    }
    let saved = vault
        .add_transaction(correction(&cash, &[(usd(), "90")], "2026-03-05T10:00:00Z"))
        .unwrap();
    let Transaction::Adjustment(inner) = &saved else {
        panic!("a correction");
    };
    assert_eq!(inner.lines[0].before, amount("100"));
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("140"));

    // Edited: it does not see itself as part of the ledger.
    let id = saved.meta().id.clone();
    let edited = vault
        .update_transaction(
            &id,
            correction(&cash, &[(usd(), "95")], "2026-03-05T10:00:00Z"),
        )
        .unwrap();
    let Transaction::Adjustment(inner) = &edited else {
        panic!("a correction");
    };
    assert_eq!(inner.lines[0].before, amount("100"));
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("145"));
}

fn before_of(vault: &UnlockedVault, id: &str) -> Vec<Amount> {
    let Some(Transaction::Adjustment(inner)) =
        vault.doc.transactions.iter().find(|tx| tx.meta().id == id)
    else {
        panic!("a correction");
    };
    inner.lines.iter().map(|line| line.before).collect()
}

#[test]
fn a_correction_follows_changes_to_the_history_before_it() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    let salary = vault.add_transaction(salary).unwrap();
    let fixed = vault
        .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
        .unwrap();
    let fixed = fixed.meta().id.clone();
    let mut later = income(&cash, "10", "salary");
    at(&mut later, "2026-03-10T10:00:00Z");
    vault.add_transaction(later).unwrap();
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));

    // Added before it: the counted 80 still stands.
    let mut forgotten = payment(&cash, "15", "lunch");
    at(&mut forgotten, "2026-03-03T10:00:00Z");
    let forgotten = vault.add_transaction(forgotten).unwrap();
    assert_eq!(before_of(&vault, &fixed), [amount("85")]);
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));

    // Edited before it.
    let mut bigger = income(&cash, "120", "salary");
    at(&mut bigger, "2026-03-01T10:00:00Z");
    vault.update_transaction(&salary.meta().id, bigger).unwrap();
    assert_eq!(before_of(&vault, &fixed), [amount("105")]);
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));

    // Deleted before it; the difference may come to nothing, the line stays.
    vault.delete_transaction(&forgotten.meta().id).unwrap();
    let mut spent = payment(&cash, "40", "rent");
    at(&mut spent, "2026-03-02T10:00:00Z");
    vault.add_transaction(spent).unwrap();
    assert_eq!(before_of(&vault, &fixed), [amount("80")]);
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));
}

#[test]
fn corrections_in_a_row_are_recalculated_in_order() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    vault.add_transaction(salary).unwrap();
    let first = vault
        .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
        .unwrap();
    let mut between = payment(&cash, "10", "lunch");
    at(&mut between, "2026-03-07T10:00:00Z");
    let between = vault.add_transaction(between).unwrap();
    let second = vault
        .add_transaction(correction(&cash, &[(usd(), "60")], "2026-03-09T10:00:00Z"))
        .unwrap();
    assert_eq!(before_of(&vault, &second.meta().id), [amount("70")]);

    // A change before both moves only the first one's difference.
    let mut bonus = income(&cash, "50", "bonus");
    at(&mut bonus, "2026-03-02T10:00:00Z");
    vault.add_transaction(bonus).unwrap();
    assert_eq!(before_of(&vault, &first.meta().id), [amount("150")]);
    assert_eq!(before_of(&vault, &second.meta().id), [amount("70")]);

    // A change between them moves only the second one's.
    vault.delete_transaction(&between.meta().id).unwrap();
    assert_eq!(before_of(&vault, &first.meta().id), [amount("150")]);
    assert_eq!(before_of(&vault, &second.meta().id), [amount("80")]);
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("60"));
}

#[test]
fn corrections_at_the_same_moment_settle() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    vault.add_transaction(salary).unwrap();
    let moment = "2026-03-05T10:00:00Z";
    vault
        .add_transaction(correction(&cash, &[(usd(), "80")], moment))
        .unwrap();
    vault
        .add_transaction(correction(&cash, &[(usd(), "70")], moment))
        .unwrap();
    let held = holding_of(&vault, &cash, &usd());
    vault.rebase_adjustments();
    vault.recompute_holdings();
    assert_eq!(holding_of(&vault, &cash, &usd()), held, "stable");
    assert_eq!(held, amount("70"));
}

#[test]
fn the_draft_lists_what_the_account_holds() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    vault
        .add_transaction(income(&cash, "100", "salary"))
        .unwrap();
    let draft = vault
        .adjustment_draft(&AdjustmentDraftInput {
            store_id: cash.clone(),
            at: None,
            replaces: None,
        })
        .unwrap();
    assert_eq!(draft.lines.len(), 1);
    assert_eq!(draft.lines[0].before, amount("100"));
    assert_eq!(draft.lines[0].after, amount("100"));
}

#[test]
fn a_correction_keeps_to_the_rules_of_the_account() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    vault
        .add_transaction(income(&bank, "100", "salary"))
        .unwrap();
    let now = now_iso();
    assert!(
        vault
            .add_transaction(correction(&bank, &[(btc(), "1")], &now))
            .is_err(),
        "a bank holds only fiat"
    );
    assert!(
        vault
            .add_transaction(correction(&bank, &[(usd(), "-5")], &now))
            .is_err(),
        "money cannot be below zero"
    );
    assert!(
        vault
            .add_transaction(correction(&bank, &[(usd(), "100")], &now))
            .is_err(),
        "nothing to correct"
    );
    assert!(
        vault
            .add_transaction(correction(&bank, &[(usd(), "1"), (usd(), "2")], &now))
            .is_err(),
        "an asset twice"
    );
}

#[test]
fn re_saving_a_correction_keeps_its_assets_even_when_they_match() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    vault.add_transaction(salary).unwrap();
    let saved = vault
        .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
        .unwrap();
    let id = saved.meta().id.clone();

    // A later change of history before it: the correction absorbs it.
    let mut refund = income(&cash, "20", "refund");
    at(&mut refund, "2026-03-02T10:00:00Z");
    vault.add_transaction(refund).unwrap();
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("80"));

    // Opened and saved as it is: the line stays, so does the balance.
    let resaved = vault
        .update_transaction(
            &id,
            correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"),
        )
        .unwrap();
    let Transaction::Adjustment(inner) = &resaved else {
        panic!("a correction");
    };
    assert_eq!(inner.lines.len(), 1);
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("80"));

    // A brand-new correction that changes nothing is still refused.
    assert!(
        vault
            .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-06T10:00:00Z"))
            .is_err()
    );
}

#[test]
fn re_saving_an_operation_unchanged_does_not_move_the_balance() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:30Z");
    let salary = vault.add_transaction(salary).unwrap();
    vault
        .add_transaction(correction(&cash, &[(usd(), "90")], "2026-03-01T10:00:45Z"))
        .unwrap();
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));
    // The same moment, seconds and all: nothing moves past the correction.
    vault
        .update_transaction(&salary.meta().id.clone(), salary.clone())
        .unwrap();
    assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));
}

// -- the pure parts, on a bare document ---------------------------------------

#[test]
fn balances_at_reads_a_bare_document() {
    let mut early = income("card", "100", "salary");
    at(&mut early, "2026-01-01T00:00:00Z");
    let mut late = payment("card", "30", "shop");
    at(&mut late, "2026-03-01T00:00:00Z");
    let late_id = late.meta().id.clone();
    let doc = crate::model::vault::VaultDoc {
        transactions: vec![early, late],
        ..crate::model::vault::VaultDoc::default()
    };
    let in_february = balances_at(&doc, "card", "2026-02-01T00:00:00Z", None);
    assert_eq!(in_february, vec![(usd(), amount("100"))]);
    let in_april = balances_at(&doc, "card", "2026-04-01T00:00:00Z", None);
    assert_eq!(in_april, vec![(usd(), amount("70"))]);
    let without_the_payment = balances_at(&doc, "card", "2026-04-01T00:00:00Z", Some(&late_id));
    assert_eq!(without_the_payment, vec![(usd(), amount("100"))]);
}

#[test]
fn the_draft_of_a_missing_store_is_refused() {
    let doc = crate::model::vault::VaultDoc::default();
    let input = AdjustmentDraftInput {
        store_id: "nope".into(),
        at: None,
        replaces: None,
    };
    assert!(adjustment_draft(&doc, &input).is_err());
}

#[test]
fn complete_adjustment_leaves_other_kinds_alone() {
    let doc = crate::model::vault::VaultDoc::default();
    let mut tx = payment("card", "5", "a");
    let before = tx.clone();
    complete_adjustment(&doc, &mut tx).unwrap();
    assert_eq!(tx, before);
}

/// The rebase is a rewrite of the document and nothing else: the scenario over
/// the vault saves nothing by itself.
#[test]
fn rebasing_the_corrections_writes_nothing() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "100", "salary"))
        .unwrap();
    let commits = super::bulk::Commits::watch(&mut vault);
    vault.rebase_adjustments();
    assert_eq!(commits.count(), 0);
}
