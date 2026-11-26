//! S11 repair of a distributed income after a merge and on open: `verify_distribution` finds the
//! breaks, the fix is the smallest one. A part whose effect is gone is dropped and its amount goes
//! back to the remainder (the income stays whole); a back-reference to an income that is gone, or
//! that no part names, is cleared. A transfer or an envelope move is never deleted: the money
//! stays where the merge left it. Pure over the document, so two devices that merged the same
//! records make the same edit.

use std::collections::HashSet;

use crate::money::distribution::{
    EFFECT_MISSING, EFFECT_ORPHAN, INCOME_MISSING, check_distribution,
};
use crate::sync::repair::repair_report::RepairReport;
use mwt_model::{
    DistKind, DistTarget, DistributionPart, EffectRef, PartStatus, Transaction, VaultDoc,
};

/// Fix every income whose distribution `check_distribution` finds broken in the two ways above.
pub(super) fn repair_distributions(doc: &mut VaultDoc, report: &mut RepairReport) {
    for income_id in income_ids(doc) {
        if adopt_named_effects(doc, &income_id) {
            report.adopted_effects.push(income_id.clone());
        }
        settle_differing_amounts(doc, &income_id, report);
        let found = check_distribution(doc, &income_id);
        if found.iter().any(|item| item.code == EFFECT_MISSING) {
            drop_gone_parts(doc, &income_id, report);
        }
        if found
            .iter()
            .any(|item| matches!(item.code, INCOME_MISSING | EFFECT_ORPHAN))
        {
            clear_unnamed_refs(doc, &income_id, report);
        }
    }
}

/// The incomes that distribute and every income some record points back to, sorted.
fn income_ids(doc: &VaultDoc) -> Vec<String> {
    let distributing = doc.transactions.iter().filter_map(|tx| match tx {
        Transaction::Topup(topup) if topup.distribution.is_some() => Some(topup.meta.id.clone()),
        _ => None,
    });
    let from_transactions = doc
        .transactions
        .iter()
        .filter_map(|tx| tx.meta().income_ref.clone());
    let from_moves = doc
        .envelopes
        .iter()
        .flat_map(|envelope| envelope.moves.iter())
        .filter_map(|entry| entry.income_ref.clone());
    let mut ids: Vec<String> = distributing
        .chain(from_transactions)
        .chain(from_moves)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    ids.sort();
    ids
}

fn effect_exists(doc: &VaultDoc, effect: &EffectRef) -> bool {
    match effect {
        EffectRef::Transfer { tx_id } => doc.transactions.iter().any(|tx| &tx.meta().id == tx_id),
        EffectRef::EnvelopeMove {
            envelope_id,
            move_id,
        } => doc
            .envelopes
            .iter()
            .filter(|envelope| &envelope.id == envelope_id)
            .any(|envelope| envelope.moves.iter().any(|entry| &entry.id == move_id)),
    }
}

/// Drop the parts whose effect does not exist; their amounts join the remainder (one is added
/// when a merge left none).
fn drop_gone_parts(doc: &mut VaultDoc, income_id: &str, report: &mut RepairReport) {
    let gone: HashSet<usize> = {
        let Some(parts) = distribution_of(doc, income_id) else {
            return;
        };
        parts
            .iter()
            .enumerate()
            .filter(|(_, part)| {
                part.effect
                    .as_ref()
                    .is_some_and(|effect| !effect_exists(doc, effect))
            })
            .map(|(index, _)| index)
            .collect()
    };
    if gone.is_empty() {
        return;
    }
    let Some(Transaction::Topup(topup)) = doc
        .transactions
        .iter_mut()
        .find(|tx| tx.meta().id == income_id)
    else {
        return;
    };
    let Some(parts) = topup.distribution.as_mut() else {
        return;
    };
    let mut freed = mwt_base::Amount::ZERO;
    let mut index = 0;
    parts.retain(|part| {
        let keep = !gone.contains(&index);
        index += 1;
        if !keep {
            freed += part.amount;
        }
        keep
    });
    match parts
        .iter_mut()
        .find(|part| matches!(part.kind, DistKind::Remainder))
    {
        Some(remainder) => remainder.amount += freed,
        None => parts.push(DistributionPart {
            kind: DistKind::Remainder,
            target: DistTarget::None,
            amount: freed,
            status: PartStatus::Done,
            effect: None,
        }),
    }
    report.dropped_parts.push(income_id.to_string());
}

fn distribution_of<'doc>(doc: &'doc VaultDoc, income_id: &str) -> Option<&'doc [DistributionPart]> {
    doc.transactions.iter().find_map(|tx| match tx {
        Transaction::Topup(topup) if topup.meta.id == income_id => topup.distribution.as_deref(),
        _ => None,
    })
}

/// A part names an effect whose `income_ref` is empty or points to an income that is gone (two
/// devices booked the same planned moment under different income ids and the other income was
/// dropped): the naming income adopts the effect, so its distribution agrees again.
/// Returns whether any record was adopted.
fn adopt_named_effects(doc: &mut VaultDoc, income_id: &str) -> bool {
    let mut adopted = false;
    let named: Vec<EffectRef> = distribution_of(doc, income_id)
        .unwrap_or_default()
        .iter()
        .filter_map(|part| part.effect.clone())
        .collect();
    let known: HashSet<String> = doc
        .transactions
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect();
    let orphaned = |current: &Option<String>| {
        current
            .as_deref()
            .is_none_or(|income| income != income_id && !known.contains(income))
    };
    for effect in &named {
        match effect {
            EffectRef::Transfer { tx_id } => {
                for tx in &mut doc.transactions {
                    let meta = tx.meta_mut();
                    if &meta.id == tx_id && orphaned(&meta.income_ref) {
                        meta.income_ref = Some(income_id.to_string());
                        adopted = true;
                    }
                }
            }
            EffectRef::EnvelopeMove {
                envelope_id,
                move_id,
            } => {
                for envelope in doc
                    .envelopes
                    .iter_mut()
                    .filter(|each| &each.id == envelope_id)
                {
                    for entry in envelope.moves.iter_mut().filter(|each| &each.id == move_id) {
                        if orphaned(&entry.income_ref) {
                            entry.income_ref = Some(income_id.to_string());
                            adopted = true;
                        }
                    }
                }
            }
        }
    }
    adopted
}

/// The amount of the surviving record of `effect` when it points back to `income_id`.
fn effect_amount(doc: &VaultDoc, effect: &EffectRef, income_id: &str) -> Option<mwt_base::Amount> {
    match effect {
        EffectRef::Transfer { tx_id } => doc.transactions.iter().find_map(|tx| match tx {
            Transaction::Transfer(transfer)
                if &transfer.meta.id == tx_id
                    && transfer.meta.income_ref.as_deref() == Some(income_id) =>
            {
                Some(transfer.amount)
            }
            _ => None,
        }),
        EffectRef::EnvelopeMove {
            envelope_id,
            move_id,
        } => doc
            .envelopes
            .iter()
            .filter(|envelope| &envelope.id == envelope_id)
            .flat_map(|envelope| envelope.moves.iter())
            .find(|entry| &entry.id == move_id && entry.income_ref.as_deref() == Some(income_id))
            .map(|entry| entry.delta),
    }
}

/// Two devices booked one effect with two amounts: the part takes the amount of the surviving
/// record and the difference goes to the remainder. When the remainder cannot take it (it would
/// go below zero, or there is none to give back to) the distribution stays and the income is
/// reported. A record is never changed.
fn settle_differing_amounts(doc: &mut VaultDoc, income_id: &str, report: &mut RepairReport) {
    let wanted: Vec<Option<mwt_base::Amount>> = distribution_of(doc, income_id)
        .unwrap_or_default()
        .iter()
        .map(|part| {
            let effect = part.effect.as_ref()?;
            effect_amount(doc, effect, income_id).filter(|amount| *amount != part.amount)
        })
        .collect();
    if wanted.iter().all(Option::is_none) {
        return;
    }
    let Some(Transaction::Topup(topup)) = doc
        .transactions
        .iter_mut()
        .find(|tx| tx.meta().id == income_id)
    else {
        return;
    };
    let Some(parts) = topup.distribution.as_mut() else {
        return;
    };
    let remainder_at = parts
        .iter()
        .position(|part| matches!(part.kind, DistKind::Remainder));
    let mut remainder = remainder_at.map_or(mwt_base::Amount::ZERO, |at| parts[at].amount);
    let mut settled = parts.clone();
    for (part, amount) in settled.iter_mut().zip(&wanted) {
        if let Some(amount) = amount {
            remainder += part.amount - *amount;
            part.amount = *amount;
        }
    }
    let fits = !remainder.is_negative()
        && wanted.iter().flatten().all(|amount| amount.is_positive())
        && (remainder_at.is_some() || remainder == mwt_base::Amount::ZERO);
    if !fits {
        report.unsettled_parts.push(income_id.to_string());
        return;
    }
    if let Some(at) = remainder_at {
        settled[at].amount = remainder;
    }
    *parts = settled;
    report.settled_parts.push(income_id.to_string());
}

/// Clear `income_ref` on the records that point to `income_id` but that no part of it names.
fn clear_unnamed_refs(doc: &mut VaultDoc, income_id: &str, report: &mut RepairReport) {
    let named: HashSet<EffectKey> = distribution_of(doc, income_id)
        .unwrap_or_default()
        .iter()
        .filter_map(|part| part.effect.as_ref())
        .map(EffectKey::of)
        .collect();
    let mut cleared = Vec::new();
    for tx in &mut doc.transactions {
        let meta = tx.meta_mut();
        let key = EffectKey::Transfer(meta.id.clone());
        if meta.income_ref.as_deref() == Some(income_id) && !named.contains(&key) {
            meta.income_ref = None;
            cleared.push(meta.id.clone());
        }
    }
    for envelope in &mut doc.envelopes {
        for entry in &mut envelope.moves {
            let key = EffectKey::Move(envelope.id.clone(), entry.id.clone());
            if entry.income_ref.as_deref() == Some(income_id) && !named.contains(&key) {
                entry.income_ref = None;
                cleared.push(entry.id.clone());
            }
        }
    }
    report.cleared_income_refs.extend(cleared);
}

#[derive(PartialEq, Eq, Hash)]
enum EffectKey {
    Transfer(String),
    Move(String, String),
}

impl EffectKey {
    fn of(effect: &EffectRef) -> Self {
        match effect {
            EffectRef::Transfer { tx_id } => Self::Transfer(tx_id.clone()),
            EffectRef::EnvelopeMove {
                envelope_id,
                move_id,
            } => Self::Move(envelope_id.clone(), move_id.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::repair::repair_invariants;
    use super::*;
    use crate::Amount;
    use crate::model::store::StoreKind;
    use crate::money::distribution::verify_distribution;
    use crate::planning::envelopes::EnvelopeDraft;
    use crate::service::Session;
    use crate::service::test_support::{amount, empty_vault, income_of, store};
    use crate::usecases::household::envelopes::create_envelope;
    use crate::usecases::money::transactions::add_transaction;
    use crate::usecases::planning::distribution::apply_distribution;
    use mwt_model::{AssetRef, DistributionRow};

    struct Fixture {
        _dir: tempfile::TempDir,
        vault: Session,
        income_id: String,
    }

    fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
        DistributionRow { kind, target }
    }

    /// 1000 RUB income: 300 to a deposit (a transfer), 200 to an envelope (a move), 500 stays.
    fn fixture() -> Fixture {
        let (dir, mut vault) = empty_vault();
        let rub = AssetRef::fiat("RUB", "Russian Ruble");
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let deposit = store(&mut vault, "Cushion", StoreKind::Bank, None);
        let draft = EnvelopeDraft {
            store_id: card.clone(),
            name: "Vacation".into(),
            asset: rub.clone(),
            target: None,
            goal_id: None,
        };
        let envelope = create_envelope(&mut vault, draft).unwrap().id;
        let income = income_of(&card, rub, "1000", "0.01", "salary");
        let income_id = add_transaction(&mut vault, income)
            .unwrap()
            .meta()
            .id
            .clone();
        let rows = vec![
            row(
                DistKind::Sum {
                    amount: amount("300"),
                },
                DistTarget::Store { id: deposit },
            ),
            row(
                DistKind::Sum {
                    amount: amount("200"),
                },
                DistTarget::Envelope { id: envelope },
            ),
            row(DistKind::Remainder, DistTarget::None),
        ];
        apply_distribution(&mut vault, &income_id, &rows).unwrap();
        Fixture {
            _dir: dir,
            vault,
            income_id,
        }
    }

    fn parts_of(fixture: &Fixture) -> Vec<DistributionPart> {
        distribution_of(&fixture.vault.doc, &fixture.income_id)
            .unwrap()
            .to_vec()
    }

    fn remainder_of(fixture: &Fixture) -> Amount {
        parts_of(fixture)
            .iter()
            .find(|part| matches!(part.kind, DistKind::Remainder))
            .unwrap()
            .amount
    }

    fn transfer_count(fixture: &Fixture) -> usize {
        let all = &fixture.vault.doc.transactions;
        all.iter()
            .filter(|tx| matches!(tx, Transaction::Transfer(_)))
            .count()
    }

    fn clean(fixture: &Fixture) {
        let found = verify_distribution(&fixture.vault.books(), &fixture.income_id);
        assert!(found.is_empty(), "{found:?}");
    }

    /// S11 repair: a part whose transfer is gone is dropped, its amount goes to the remainder.
    #[test]
    fn s11_a_part_whose_transfer_is_gone_is_dropped_into_the_remainder() {
        let mut fixture = fixture();
        fixture
            .vault
            .doc
            .transactions
            .retain(|tx| !matches!(tx, Transaction::Transfer(_)));
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.dropped_parts, vec![fixture.income_id.clone()]);
        assert_eq!(parts_of(&fixture).len(), 2);
        assert_eq!(remainder_of(&fixture), amount("800"));
        clean(&fixture);
        // The second pass finds nothing left to fix.
        assert!(
            repair_invariants(&mut fixture.vault, None)
                .unwrap()
                .is_empty()
        );
    }

    /// S11 repair: a part whose envelope move is gone is dropped; the transfer is never touched.
    #[test]
    fn s11_a_part_whose_envelope_move_is_gone_is_dropped_and_the_transfer_stays() {
        let mut fixture = fixture();
        for envelope in &mut fixture.vault.doc.envelopes {
            envelope.moves.clear();
        }
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.dropped_parts.len(), 1);
        assert_eq!(remainder_of(&fixture), amount("700"));
        assert_eq!(transfer_count(&fixture), 1);
        clean(&fixture);
    }

    /// S11 repair: the income is gone, so the back-references are cleared and nothing is deleted.
    #[test]
    fn s11_a_dangling_income_ref_is_cleared_and_the_transfer_stays() {
        let mut fixture = fixture();
        let income_id = fixture.income_id.clone();
        fixture
            .vault
            .doc
            .transactions
            .retain(|tx| tx.meta().id != income_id);
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.cleared_income_refs.len(), 2, "{report:?}");
        assert_eq!(transfer_count(&fixture), 1);
        assert!(
            fixture
                .vault
                .doc
                .transactions
                .iter()
                .all(|tx| tx.meta().income_ref.is_none())
        );
        assert!(
            fixture
                .vault
                .doc
                .envelopes
                .iter()
                .flat_map(|envelope| envelope.moves.iter())
                .all(|entry| entry.income_ref.is_none())
        );
        clean(&fixture);
    }

    fn set_transfer(fixture: &mut Fixture, value: &str, income_ref: Option<String>) {
        for tx in &mut fixture.vault.doc.transactions {
            if let Transaction::Transfer(transfer) = tx {
                transfer.amount = amount(value);
                transfer.meta.income_ref = income_ref.clone();
            }
        }
    }

    fn set_move(fixture: &mut Fixture, value: &str) {
        for envelope in &mut fixture.vault.doc.envelopes {
            for entry in &mut envelope.moves {
                entry.delta = amount(value);
            }
        }
    }

    fn assert_second_pass_silent(fixture: &mut Fixture) {
        let before = serde_json::to_string(&fixture.vault.doc).unwrap();
        assert!(
            repair_invariants(&mut fixture.vault, None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(serde_json::to_string(&fixture.vault.doc).unwrap(), before);
    }

    /// S11 repair: a transfer of another amount (the surviving record) wins; the remainder pays.
    #[test]
    fn s11_a_differing_transfer_amount_moves_the_difference_to_the_remainder() {
        let mut fixture = fixture();
        let income_id = fixture.income_id.clone();
        set_transfer(&mut fixture, "350", Some(income_id));
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.settled_parts, vec![fixture.income_id.clone()]);
        assert!(report.adopted_effects.is_empty() && report.unsettled_parts.is_empty());
        assert_eq!(remainder_of(&fixture), amount("450"));
        clean(&fixture);
        assert_second_pass_silent(&mut fixture);
    }

    /// S11 repair: the same for an envelope move.
    #[test]
    fn s11_a_differing_envelope_move_amount_moves_the_difference_to_the_remainder() {
        let mut fixture = fixture();
        set_move(&mut fixture, "150");
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.settled_parts, vec![fixture.income_id.clone()]);
        assert_eq!(remainder_of(&fixture), amount("550"));
        clean(&fixture);
        assert_second_pass_silent(&mut fixture);
    }

    /// S11 repair: the remainder cannot take the difference, so nothing changes and it is reported.
    #[test]
    fn s11_a_difference_the_remainder_cannot_take_is_reported_and_left_alone() {
        let mut fixture = fixture();
        let income_id = fixture.income_id.clone();
        set_transfer(&mut fixture, "900", Some(income_id));
        let before = serde_json::to_string(&fixture.vault.doc.transactions).unwrap();
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.unsettled_parts, vec![fixture.income_id.clone()]);
        assert!(report.settled_parts.is_empty());
        assert_eq!(remainder_of(&fixture), amount("500"));
        assert_eq!(
            serde_json::to_string(&fixture.vault.doc.transactions).unwrap(),
            before
        );
    }

    /// S11 repair: an adoption is reported, and with the amount agreeing nothing else changes.
    #[test]
    fn s11_an_adopted_effect_is_reported() {
        let mut fixture = fixture();
        set_transfer(&mut fixture, "300", None);
        let report = repair_invariants(&mut fixture.vault, None).unwrap();
        assert_eq!(report.adopted_effects, vec![fixture.income_id.clone()]);
        assert!(report.settled_parts.is_empty());
        assert!(report.notice_body(false).is_some());
        clean(&fixture);
        assert_second_pass_silent(&mut fixture);
    }

    /// S11 repair: a consistent distribution is left alone (nothing stamped, nothing reported).
    #[test]
    fn s11_a_consistent_distribution_is_not_touched() {
        let mut fixture = fixture();
        let before = serde_json::to_string(&fixture.vault.doc).unwrap();
        assert!(
            repair_invariants(&mut fixture.vault, None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(serde_json::to_string(&fixture.vault.doc).unwrap(), before);
    }
}
