//! Tests of the distribution usecase (op-period S8, S11, S14, S15, R5, A2, A2b, Q15): RUB, UTC.

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Error;
use crate::model::store::StoreKind;
use crate::money::bulk::TxStore;
use crate::money::distribution::{WITH_PARTS, verify_distribution};
use crate::money::worth::FlowClass;
use crate::planning::envelopes::EnvelopeDraft;
use crate::service::Session;
use crate::service::test_support::{amount, empty_vault, income_of, store};
use crate::usecases::household::envelopes::{create_envelope, delete_envelope, fund_envelope};
use crate::usecases::money::bulk::{delete_transactions, move_transactions};
use crate::usecases::money::transactions::{
    LINKED_TO_INCOME, add_linked_transactions, add_transaction, delete_transaction,
    update_transaction,
};
use crate::usecases::planning::distribution::{
    OVER_ALLOCATED, apply_distribution, remove_distribution, replace_distribution,
};
use mwt_model::{
    AssetRef, DistKind, DistTarget, DistributionRow, EffectRef, Envelope, ReceivedPart, Transaction,
};

fn rub() -> AssetRef {
    AssetRef::fiat("RUB", "Russian Ruble")
}

struct Fixture {
    _dir: tempfile::TempDir,
    vault: Session,
    card: String,
    deposit: String,
    vacation: String,
    gifts: String,
    income_id: String,
}

fn envelope_on(vault: &mut Session, card: &str, name: &str) -> String {
    let draft = EnvelopeDraft {
        store_id: card.into(),
        name: name.into(),
        asset: rub(),
        target: None,
        goal_id: None,
    };
    create_envelope(vault, draft).unwrap().id
}

/// A 120 000 salary on the card, a deposit and two envelopes.
fn fixture() -> Fixture {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let deposit = store(&mut vault, "Cushion", StoreKind::Bank, None);
    let vacation = envelope_on(&mut vault, &card, "Vacation");
    let gifts = envelope_on(&mut vault, &card, "Gifts");
    let salary = income_of(&card, rub(), "120000", "0.01", "salary");
    let income_id = add_transaction(&mut vault, salary)
        .unwrap()
        .meta()
        .id
        .clone();
    Fixture {
        _dir: dir,
        vault,
        card,
        deposit,
        vacation,
        gifts,
        income_id,
    }
}

fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
    DistributionRow { kind, target }
}

fn rows(fixture: &Fixture, deposit_sum: &str) -> Vec<DistributionRow> {
    vec![
        row(
            DistKind::Sum {
                amount: amount(deposit_sum),
            },
            DistTarget::Store {
                id: fixture.deposit.clone(),
            },
        ),
        row(
            DistKind::Percent { pct: amount("5") },
            DistTarget::Envelope {
                id: fixture.vacation.clone(),
            },
        ),
        row(
            DistKind::Sum {
                amount: amount("3000"),
            },
            DistTarget::Envelope {
                id: fixture.gifts.clone(),
            },
        ),
        row(DistKind::Remainder, DistTarget::None),
    ]
}

fn held(fixture: &Fixture, store_id: &str) -> Amount {
    crate::service::test_support::holding_of(&fixture.vault, store_id, &rub())
}

fn allocated(fixture: &Fixture, envelope_id: &str) -> Amount {
    let envelope = fixture
        .vault
        .doc
        .envelopes
        .iter()
        .find(|envelope| envelope.id == envelope_id);
    envelope
        .map(|envelope: &Envelope| envelope.allocated)
        .unwrap()
}

fn transfers(fixture: &Fixture) -> usize {
    let all = &fixture.vault.doc.transactions;
    all.iter()
        .filter(|tx| matches!(tx, Transaction::Transfer(_)))
        .count()
}

fn clean(fixture: &Fixture) {
    let found = verify_distribution(&fixture.vault.books(), &fixture.income_id);
    assert!(found.is_empty(), "{found:?}");
}

/// A2 (S6-S9, R5): one transfer, two envelope moves, four stored parts, Σ = the income.
#[test]
fn a2_apply_books_one_transfer_and_two_envelope_moves() {
    let mut fixture = fixture();
    let rows = rows(&fixture, "20000");
    let parts = apply_distribution(&mut fixture.vault, &fixture.income_id.clone(), &rows).unwrap();
    let amounts: Vec<Amount> = parts.iter().map(|part| part.amount).collect();
    assert_eq!(
        amounts,
        ["20000", "6000", "3000", "91000"].map(amount).to_vec()
    );
    assert_eq!(parts.iter().filter(|part| part.effect.is_some()).count(), 3);
    assert_eq!(transfers(&fixture), 1);
    assert_eq!(held(&fixture, &fixture.card), amount("100000"));
    assert_eq!(held(&fixture, &fixture.deposit), amount("20000"));
    assert_eq!(allocated(&fixture, &fixture.vacation), amount("6000"));
    assert_eq!(allocated(&fixture, &fixture.gifts), amount("3000"));
    clean(&fixture);

    let flows = crate::planning::flows::flows_between(
        &fixture.vault.books(),
        &Calendar::new("UTC"),
        "rub",
        i64::MIN,
        i64::MAX,
    );
    let incomes: Vec<_> = flows
        .iter()
        .filter(|flow| flow.class == FlowClass::Income)
        .collect();
    assert_eq!(incomes.len(), 1);
    assert_eq!(incomes[0].value, amount("120000"));
    assert!(
        flows
            .iter()
            .all(|flow| !matches!(flow.class, FlowClass::Daily | FlowClass::Fixed))
    );
}

/// A2 (S7): over-allocation is refused and nothing is written.
#[test]
fn a2_over_allocation_is_rejected_and_writes_nothing() {
    let mut fixture = fixture();
    let before = serde_json::to_string(&fixture.vault.doc).unwrap();
    let rows = rows(&fixture, "130000");
    let err = apply_distribution(&mut fixture.vault, &fixture.income_id.clone(), &rows);
    assert!(err.unwrap_err().to_string().contains(OVER_ALLOCATED));
    assert_eq!(serde_json::to_string(&fixture.vault.doc).unwrap(), before);
}

/// A2b (Q15): the transfer is booked, then an envelope row finds no free balance: nothing is kept.
#[test]
fn a2b_q15_not_enough_free_balance_leaves_the_vault_identical() {
    let mut fixture = fixture();
    // 98 000 of the 120 000 are earmarked by another envelope: 22 000 free, the transfer takes
    // 20 000, so the 6 000 envelope row finds only 2 000.
    let reserve = envelope_on(&mut fixture.vault, &fixture.card.clone(), "Reserve");
    fund_envelope(&mut fixture.vault, &reserve, amount("98000")).unwrap();
    let rows = rows(&fixture, "20000");
    let before = serde_json::to_string(&fixture.vault.doc).unwrap();
    let err = apply_distribution(&mut fixture.vault, &fixture.income_id.clone(), &rows);
    assert!(matches!(err, Err(Error::InsufficientFunds(_))), "{err:?}");
    assert_eq!(serde_json::to_string(&fixture.vault.doc).unwrap(), before);
    assert_eq!(transfers(&fixture), 0);
}

/// A2b (S11): editing 20 000 to 25 000 removes the old transfer, creates the new one, list clean.
#[test]
fn a2b_replace_swaps_the_transfer_and_keeps_unchanged_effects() {
    let mut fixture = fixture();
    let id = fixture.income_id.clone();
    let (old_rows, new_rows) = (rows(&fixture, "20000"), rows(&fixture, "25000"));
    let first = apply_distribution(&mut fixture.vault, &id, &old_rows).unwrap();
    let parts = replace_distribution(&mut fixture.vault, &id, &new_rows).unwrap();
    assert_eq!(transfers(&fixture), 1);
    assert_ne!(first[0].effect, parts[0].effect);
    assert_eq!(first[1].effect, parts[1].effect);
    assert_eq!(parts[3].amount, amount("86000"));
    assert_eq!(held(&fixture, &fixture.deposit), amount("25000"));
    assert_eq!(held(&fixture, &fixture.card), amount("95000"));
    clean(&fixture);
}

/// A2b (S11): undo removes every effect and the list.
#[test]
fn a2b_remove_undoes_every_effect() {
    let mut fixture = fixture();
    let id = fixture.income_id.clone();
    let asked = rows(&fixture, "20000");
    apply_distribution(&mut fixture.vault, &id, &asked).unwrap();
    remove_distribution(&mut fixture.vault, &id).unwrap();
    assert_eq!(transfers(&fixture), 0);
    assert_eq!(held(&fixture, &fixture.card), amount("120000"));
    assert_eq!(allocated(&fixture, &fixture.vacation), Amount::ZERO);
    let Some(Transaction::Topup(topup)) = fixture.vault.doc.transactions.first() else {
        panic!("the income is first");
    };
    assert!(topup.distribution.is_none());
    clean(&fixture);
}

/// S8: a goal row funds the goal's envelope on the card through `plan_fund_goal`.
#[test]
fn s8_a_goal_row_funds_the_goal_envelope() {
    use crate::model::store::StoreScope;
    use crate::planning::goals::GoalDraft;
    let mut fixture = fixture();
    let goal = crate::usecases::planning::goals::create_goal(
        &mut fixture.vault,
        GoalDraft {
            name: "Cushion".into(),
            active: true,
            target_amount: amount("500000"),
            currency: "rub".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![fixture.card.clone()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        },
    )
    .unwrap();
    let rows = vec![
        row(
            DistKind::Sum {
                amount: amount("7000"),
            },
            DistTarget::Goal {
                goal_id: goal.id.clone(),
                store_id: fixture.card.clone(),
            },
        ),
        row(DistKind::Remainder, DistTarget::None),
    ];
    let id = fixture.income_id.clone();
    let parts = apply_distribution(&mut fixture.vault, &id, &rows).unwrap();
    assert!(matches!(
        parts[0].effect,
        Some(EffectRef::EnvelopeMove { .. })
    ));
    let saved = fixture
        .vault
        .doc
        .envelopes
        .iter()
        .find(|envelope| envelope.goal_id.as_deref() == Some(&goal.id))
        .unwrap();
    assert_eq!(saved.allocated, amount("7000"));
    clean(&fixture);
}

/// Q15: a goal funded by `fund_goal` and then distributed to keeps one envelope.
#[test]
fn s8_distributing_to_a_funded_goal_reuses_its_envelope() {
    use crate::model::ids::StoreId;
    use crate::model::store::StoreScope;
    use crate::planning::goals::GoalDraft;
    let mut fixture = fixture();
    let goal = crate::usecases::planning::goals::create_goal(
        &mut fixture.vault,
        GoalDraft {
            name: "Cushion".into(),
            active: true,
            target_amount: amount("500000"),
            currency: "rub".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![fixture.card.clone()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        },
    )
    .unwrap();
    crate::usecases::household::envelopes::fund_goal(
        &mut fixture.vault,
        &goal.id,
        &StoreId::from(&fixture.card),
        amount("1000"),
    )
    .unwrap();
    let rows = vec![
        row(
            DistKind::Sum {
                amount: amount("7000"),
            },
            DistTarget::Goal {
                goal_id: goal.id.clone(),
                store_id: fixture.card.clone(),
            },
        ),
        row(DistKind::Remainder, DistTarget::None),
    ];
    let id = fixture.income_id.clone();
    apply_distribution(&mut fixture.vault, &id, &rows).unwrap();
    let of_goal: Vec<_> = fixture
        .vault
        .doc
        .envelopes
        .iter()
        .filter(|envelope| envelope.goal_id.as_deref() == Some(&goal.id))
        .collect();
    assert_eq!(of_goal.len(), 1);
    assert_eq!(of_goal[0].allocated, amount("8000"));
}

/// S14: parts and a distribution together are refused.
#[test]
fn s14_parts_and_distribution_are_mutually_exclusive() {
    let mut fixture = fixture();
    let id = fixture.income_id.clone();
    let card = fixture.card.clone();
    for tx in &mut fixture.vault.doc.transactions {
        if let Transaction::Topup(topup) = tx {
            topup.parts = vec![ReceivedPart {
                store_id: card.clone(),
                amount: amount("120000"),
                timestamp: None,
            }];
        }
    }
    let before = serde_json::to_string(&fixture.vault.doc).unwrap();
    let rows = rows(&fixture, "20000");
    let err = apply_distribution(&mut fixture.vault, &id, &rows).unwrap_err();
    assert!(err.to_string().contains(WITH_PARTS), "{err}");
    assert_eq!(serde_json::to_string(&fixture.vault.doc).unwrap(), before);
}

fn salary_with(fixture: &Fixture, deposit_sum: &str) -> Transaction {
    let mut salary = income_of(&fixture.card, rub(), "50000", "0.01", "bonus");
    if let Transaction::Topup(topup) = &mut salary {
        topup.distribution = Some(
            rows(fixture, deposit_sum)
                .into_iter()
                .map(|row| mwt_model::DistributionPart {
                    kind: row.kind,
                    target: row.target,
                    amount: Amount::ZERO,
                    status: mwt_model::PartStatus::Done,
                    effect: None,
                })
                .collect(),
        );
    }
    salary
}

/// S11: a topup added with a distribution is distributed by the usecase and passes V1-V4.
#[test]
fn s11_add_transaction_with_distribution_applies_it() {
    let mut fixture = fixture();
    let salary = salary_with(&fixture, "20000");
    let stored = add_transaction(&mut fixture.vault, salary).unwrap();
    let id = stored.meta().id.clone();
    let Transaction::Topup(topup) = &stored else {
        panic!("a topup")
    };
    assert_eq!(topup.distribution.as_ref().map(Vec::len), Some(4));
    assert!(verify_distribution(&fixture.vault.books(), &id).is_empty());
    assert_eq!(transfers(&fixture), 1);
}

/// S11: a distribution that fails leaves nothing booked, not even the income.
#[test]
fn s11_failed_distribution_on_add_books_nothing() {
    let mut fixture = fixture();
    let before = fixture.vault.doc.transactions.len();
    let salary = salary_with(&fixture, "900000");
    let err = add_transaction(&mut fixture.vault, salary).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert_eq!(fixture.vault.doc.transactions.len(), before);
}

/// S9: a caller cannot link an operation to an income itself.
#[test]
fn s9_add_transaction_refuses_a_supplied_income_ref() {
    let mut fixture = fixture();
    let mut salary = income_of(&fixture.card, rub(), "10", "0.01", "x");
    salary.meta_mut().income_ref = Some(fixture.income_id.clone());
    let err = add_transaction(&mut fixture.vault, salary).unwrap_err();
    assert!(err.to_string().contains("INCOME_REF_RESERVED"), "{err}");
}

/// S9: a forged income_ref in a batch is refused as in a single add.
#[test]
fn s9_batch_refuses_a_supplied_income_ref() {
    let mut fixture = fixture();
    let mut salary = income_of(&fixture.card, rub(), "10", "0.01", "x");
    salary.meta_mut().income_ref = Some(fixture.income_id.clone());
    let err = add_linked_transactions(&mut fixture.vault, vec![salary], None).unwrap_err();
    assert!(err.to_string().contains("INCOME_REF_RESERVED"), "{err}");
}

/// S11: a topup with a distribution in a batch is distributed and passes V1-V4.
#[test]
fn s11_batch_topup_with_distribution_applies_it() {
    let mut fixture = fixture();
    let salary = salary_with(&fixture, "20000");
    let saved = add_linked_transactions(&mut fixture.vault, vec![salary], None).unwrap();
    let id = saved[0].meta().id.clone();
    let Transaction::Topup(topup) = &saved[0] else {
        panic!("a topup")
    };
    assert_eq!(topup.distribution.as_ref().map(Vec::len), Some(4));
    assert!(verify_distribution(&fixture.vault.books(), &id).is_empty());
}

/// S8 "all or none": a distributed topup followed by a failing item leaves the vault unchanged.
#[test]
fn s8_batch_with_a_failing_item_after_a_distributed_topup_changes_nothing() {
    let mut fixture = fixture();
    let before = snapshot(&fixture);
    let salary = salary_with(&fixture, "20000");
    let mut forged = income_of(&fixture.card, rub(), "10", "0.01", "x");
    forged.meta_mut().income_ref = Some(fixture.income_id.clone());
    let err = add_linked_transactions(&mut fixture.vault, vec![salary, forged], None).unwrap_err();
    assert!(err.to_string().contains("INCOME_REF_RESERVED"), "{err}");
    assert_eq!(snapshot(&fixture), before);
}

/// The whole document; derived holdings are rebuilt with fresh ids in any order, so they are
/// compared without ids and sorted.
fn snapshot(fixture: &Fixture) -> String {
    let mut doc = serde_json::to_value(&fixture.vault.doc).unwrap();
    if let Some(holdings) = doc["holdings"].as_array_mut() {
        for holding in holdings.iter_mut() {
            holding["id"] = serde_json::Value::Null;
        }
        holdings.sort_by_key(ToString::to_string);
    }
    doc.to_string()
}

/// The 120 000 income distributed 20 000 / 5 % / 3 000 / rest; returns the transfer's id.
fn distributed() -> (Fixture, String) {
    let mut fixture = fixture();
    let asked = rows(&fixture, "20000");
    let income = fixture.income_id.clone();
    apply_distribution(&mut fixture.vault, &income, &asked).unwrap();
    let transfer = fixture
        .vault
        .doc
        .transactions
        .iter()
        .find(|tx| matches!(tx, Transaction::Transfer(_)))
        .map(|tx| tx.meta().id.clone())
        .unwrap();
    (fixture, transfer)
}

fn refused_as_linked<T: std::fmt::Debug>(result: crate::error::Result<T>) {
    let err = result.unwrap_err();
    assert!(err.to_string().contains(LINKED_TO_INCOME), "{err}");
    assert!(err.to_string().contains("replace_distribution"), "{err}");
}

fn stored(fixture: &Fixture, id: &str) -> Transaction {
    let all = &fixture.vault.doc.transactions;
    all.iter().find(|tx| tx.meta().id == id).cloned().unwrap()
}

/// A2b (S11 guards): deleting a linked transfer is refused and changes nothing.
#[test]
fn a2b_delete_transaction_of_a_linked_transfer_is_refused() {
    let (mut fixture, transfer) = distributed();
    let before = snapshot(&fixture);
    refused_as_linked(delete_transaction(&mut fixture.vault, &transfer));
    assert_eq!(snapshot(&fixture), before);
}

/// A2b (S11 guards): the bulk delete refuses a linked transfer and deletes none of the batch.
#[test]
fn a2b_bulk_delete_of_a_linked_transfer_is_refused() {
    let (mut fixture, transfer) = distributed();
    let before = snapshot(&fixture);
    let ids = vec![transfer, fixture.income_id.clone()];
    // The income is in the batch but the transfer is covered only when its own income goes too.
    delete_transactions(&mut fixture.vault, &ids).unwrap();
    assert!(fixture.vault.doc.transactions.is_empty());
    let (mut fixture, transfer) = distributed();
    let plain = add_transaction(
        &mut fixture.vault,
        income_of(&fixture.card.clone(), rub(), "10", "0.01", "plain"),
    )
    .unwrap();
    let before_second = snapshot(&fixture);
    refused_as_linked(delete_transactions(
        &mut fixture.vault,
        &[plain.meta().id.clone(), transfer],
    ));
    assert_eq!(snapshot(&fixture), before_second);
    assert_ne!(before, before_second);
}

/// A2b (S11 guards): amount, accounts and asset of a linked transfer are refused, vault unchanged.
#[test]
fn a2b_update_of_a_linked_transfer_amount_accounts_and_asset_is_refused() {
    let (mut fixture, transfer) = distributed();
    let before = snapshot(&fixture);
    let original = stored(&fixture, &transfer);
    let other = store(&mut fixture.vault, "Other", StoreKind::Bank, None);
    let before_other = snapshot(&fixture);
    let mut edits = Vec::new();
    for change in 0..3 {
        let mut edited = original.clone();
        if let Transaction::Transfer(inner) = &mut edited {
            match change {
                0 => inner.amount = amount("1"),
                1 => inner.to = mwt_model::AccountRef::Store(other.clone()),
                _ => inner.asset = AssetRef::fiat("USD", "US Dollar"),
            }
        }
        edits.push(edited);
    }
    for edited in edits {
        refused_as_linked(update_transaction(&mut fixture.vault, &transfer, edited));
        assert_eq!(snapshot(&fixture), before_other);
    }
    assert_ne!(before, before_other);
}

/// A2b (S11 guards): the comment of a linked transfer stays editable and the link survives.
#[test]
fn a2b_comment_of_a_linked_transfer_stays_editable() {
    let (mut fixture, transfer) = distributed();
    let mut edited = stored(&fixture, &transfer);
    edited.meta_mut().comment = "to the cushion".into();
    edited.meta_mut().income_ref = None;
    update_transaction(&mut fixture.vault, &transfer, edited).unwrap();
    let after = stored(&fixture, &transfer);
    assert_eq!(after.meta().comment, "to the cushion");
    assert_eq!(
        after.meta().income_ref.as_deref(),
        Some(fixture.income_id.as_str())
    );
    clean(&fixture);
}

/// A2b (S11 guards): an envelope holding a linked move cannot be deleted.
#[test]
fn a2b_delete_envelope_with_a_linked_move_is_refused() {
    let (mut fixture, _) = distributed();
    let before = snapshot(&fixture);
    refused_as_linked(delete_envelope(
        &mut fixture.vault,
        &fixture.vacation.clone(),
    ));
    assert_eq!(snapshot(&fixture), before);
}

/// A2b (S11 guards): a distributed income cannot be moved to another store on its own.
#[test]
fn a2b_move_of_a_distributed_income_is_refused() {
    let (mut fixture, _) = distributed();
    let before = snapshot(&fixture);
    let change = TxStore {
        id: fixture.income_id.clone(),
        store_id: fixture.deposit.clone(),
    };
    refused_as_linked(move_transactions(&mut fixture.vault, vec![change]));
    assert_eq!(snapshot(&fixture), before);
}

/// A2b (S11): deleting the income removes its effects in the same write; the envelope stays.
#[test]
fn a2b_deleting_the_income_removes_its_effects() {
    let (mut fixture, _) = distributed();
    let income = fixture.income_id.clone();
    delete_transaction(&mut fixture.vault, &income).unwrap();
    assert!(fixture.vault.doc.transactions.is_empty());
    assert_eq!(allocated(&fixture, &fixture.vacation), Amount::ZERO);
    assert_eq!(allocated(&fixture, &fixture.gifts), Amount::ZERO);
    assert!(verify_distribution(&fixture.vault.books(), &income).is_empty());
}

/// A2b (S11, D9): editing the income amount re-resolves by shrink_remainder, sum rows keep.
#[test]
fn a2b_editing_the_income_amount_re_resolves_the_rows() {
    let (mut fixture, _) = distributed();
    let income = fixture.income_id.clone();
    let mut edited = stored(&fixture, &income);
    if let Transaction::Topup(topup) = &mut edited {
        topup.amount = amount("118500");
        topup.distribution = None;
    }
    let saved = update_transaction(&mut fixture.vault, &income, edited).unwrap();
    let Transaction::Topup(topup) = saved else {
        panic!("a topup")
    };
    let amounts: Vec<Amount> = topup
        .distribution
        .unwrap()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(
        amounts,
        ["20000", "5925", "3000", "89575"].map(amount).to_vec()
    );
    assert_eq!(transfers(&fixture), 1);
    assert_eq!(allocated(&fixture, &fixture.vacation), amount("5925"));
    clean(&fixture);
}

/// A2b (S11): an income edit the rows cannot follow is refused as a whole, vault unchanged.
#[test]
fn a2b_income_edit_that_over_allocates_is_refused_whole() {
    let (mut fixture, _) = distributed();
    let income = fixture.income_id.clone();
    let before = snapshot(&fixture);
    let mut edited = stored(&fixture, &income);
    if let Transaction::Topup(topup) = &mut edited {
        topup.amount = amount("20000");
    }
    let err = update_transaction(&mut fixture.vault, &income, edited).unwrap_err();
    assert!(err.to_string().contains(OVER_ALLOCATED), "{err}");
    assert_eq!(snapshot(&fixture), before);
}
