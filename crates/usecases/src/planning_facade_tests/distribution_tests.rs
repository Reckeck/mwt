//! Tests of the distribution usecase (op-period S8, S11, S14, S15, R5, A2, A2b, Q15): RUB, UTC.

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Error;
use crate::model::store::StoreKind;
use crate::money::distribution::{WITH_PARTS, verify_distribution};
use crate::money::worth::FlowClass;
use crate::planning::envelopes::EnvelopeDraft;
use crate::service::Session;
use crate::service::test_support::{amount, empty_vault, income_of, store};
use crate::usecases::household::envelopes::{create_envelope, fund_envelope};
use crate::usecases::money::transactions::{add_linked_transactions, add_transaction};
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
