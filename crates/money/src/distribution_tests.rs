//! Tests of `verify_distribution` (spec S11 V1-V4) and the own-target rules (S6, S14), RUB.

use std::collections::HashMap;

use mwt_base::Amount;
use mwt_base::error::Result;
use mwt_model::{
    AccountRef, AssetRef, CryptoHistory, DistKind, DistTarget, DistributionPart, EffectRef,
    Envelope, EnvelopeMove, KindDetails, PartStatus, PriceStamp, ReceivedPart, Store, StoreKind,
    TopupTx, Transaction, TransferTx, TxKind, TxMeta, VaultDoc,
};

use super::books::{Books, BooksSource};
use super::distribution::*;
use super::valuation::ValuationCurve;

struct Source(VaultDoc);

impl BooksSource for Source {
    fn doc(&self) -> &VaultDoc {
        &self.0
    }
    fn base_and_fx(&self) -> (String, Amount) {
        ("RUB".into(), amount("1"))
    }
    fn price_usd_for(&self, _asset_key: &str) -> Amount {
        amount("1")
    }
    fn price_base_for(&self, _asset_key: &str, _fx: Amount) -> Amount {
        amount("1")
    }
    fn valuation_curves(&self) -> HashMap<String, ValuationCurve> {
        HashMap::new()
    }
    fn prepare_transaction(&self, _tx: &mut Transaction) -> Result<()> {
        Ok(())
    }
    fn earmarked(&self, _store_id: &str, _asset_key: &str) -> Amount {
        Amount::ZERO
    }
    fn keys_on_device(&self) -> bool {
        false
    }
    fn device_secrets_for_request(&self) -> Result<mwt_model::DeviceSecrets> {
        Ok(mwt_model::DeviceSecrets::default())
    }
    fn crypto_history(&self, _cutoffs: &[i64], _in_scope: &dyn Fn(&str) -> bool) -> CryptoHistory {
        CryptoHistory::default()
    }
}

fn amount(raw: &str) -> Amount {
    Amount::parse(raw).expect("a test amount")
}

fn rub() -> AssetRef {
    AssetRef::fiat("RUB", "Ruble")
}

fn wallet(id: &str) -> Store {
    let mut store = Store::new(id, StoreKind::Bank, "#000");
    store.id = id.into();
    store
}

fn part(
    kind: DistKind,
    target: DistTarget,
    raw: &str,
    effect: Option<EffectRef>,
) -> DistributionPart {
    DistributionPart {
        kind,
        target,
        amount: amount(raw),
        status: PartStatus::Done,
        effect,
    }
}

fn income(parts: Vec<DistributionPart>) -> Transaction {
    let mut meta = TxMeta::new("salary");
    meta.id = "income".into();
    Transaction::Topup(TopupTx {
        meta,
        store_id: "card".into(),
        asset: rub(),
        amount: amount("100"),
        fee: None,
        from: None,
        details: KindDetails::default(),
        price: PriceStamp::default(),
        parts: Vec::new(),
        distribution: Some(parts),
    })
}

fn transfer(id: &str, to: &str, raw: &str, income_ref: Option<&str>) -> Transaction {
    let mut meta = TxMeta::new("");
    meta.id = id.into();
    meta.income_ref = income_ref.map(str::to_string);
    Transaction::Transfer(TransferTx {
        meta,
        from: AccountRef::Store("card".into()),
        to: AccountRef::Store(to.into()),
        asset: rub(),
        amount: amount(raw),
        fee: None,
        network: None,
        tx_hash: None,
        debt: None,
        price: PriceStamp::default(),
    })
}

fn store_part(raw: &str, tx_id: &str) -> DistributionPart {
    part(
        DistKind::Sum {
            amount: amount(raw),
        },
        DistTarget::Store {
            id: "deposit".into(),
        },
        raw,
        Some(EffectRef::Transfer {
            tx_id: tx_id.into(),
        }),
    )
}

fn remainder(raw: &str) -> DistributionPart {
    part(DistKind::Remainder, DistTarget::None, raw, None)
}

fn doc(parts: Vec<DistributionPart>, transfers: Vec<Transaction>) -> VaultDoc {
    let mut doc = VaultDoc {
        stores: vec![wallet("card"), wallet("deposit")],
        ..VaultDoc::default()
    };
    doc.transactions.push(income(parts));
    doc.transactions.extend(transfers);
    doc
}

fn clean_doc() -> VaultDoc {
    doc(
        vec![store_part("20", "t1"), remainder("80")],
        vec![transfer("t1", "deposit", "20", Some("income"))],
    )
}

fn codes(doc: &VaultDoc) -> Vec<&'static str> {
    check_distribution(doc, "income")
        .iter()
        .map(|found| found.code)
        .collect()
}

#[test]
fn distribution_clean_case_has_no_violations() {
    let source = Source(clean_doc());
    assert!(verify_distribution(&Books::new(&source), "income").is_empty());
}

#[test]
fn distribution_v1_sum_must_equal_the_income() {
    let broken = doc(
        vec![store_part("20", "t1"), remainder("70")],
        vec![transfer("t1", "deposit", "20", Some("income"))],
    );
    assert_eq!(codes(&broken), vec![SUM_MISMATCH]);
}

#[test]
fn distribution_v1_exactly_one_remainder_and_positive_parts() {
    let two = doc(
        vec![store_part("20", "t1"), remainder("40"), remainder("40")],
        vec![transfer("t1", "deposit", "20", Some("income"))],
    );
    assert_eq!(codes(&two), vec![REMAINDER_COUNT]);
    let zero = doc(
        vec![store_part("0", "t1"), remainder("100")],
        vec![transfer("t1", "deposit", "0", Some("income"))],
    );
    assert_eq!(codes(&zero), vec![PART_NOT_POSITIVE]);
}

#[test]
fn distribution_v2_effect_must_exist_and_agree() {
    let missing = doc(vec![store_part("20", "t1"), remainder("80")], vec![]);
    assert_eq!(codes(&missing), vec![EFFECT_MISSING]);
    let wrong_amount = doc(
        vec![store_part("20", "t1"), remainder("80")],
        vec![transfer("t1", "deposit", "25", Some("income"))],
    );
    assert_eq!(codes(&wrong_amount), vec![EFFECT_MISMATCH]);
    let no_effect = doc(
        vec![
            part(
                DistKind::Sum {
                    amount: amount("20"),
                },
                DistTarget::Store {
                    id: "deposit".into(),
                },
                "20",
                None,
            ),
            remainder("80"),
        ],
        vec![],
    );
    assert_eq!(codes(&no_effect), vec![EFFECT_SHAPE]);
}

#[test]
fn distribution_v2_envelope_move_is_read_through_its_envelope() {
    let mut envelope = Envelope {
        id: "trip".into(),
        store_id: "card".into(),
        name: "Trip".into(),
        asset: rub(),
        allocated: amount("20"),
        moves: vec![EnvelopeMove {
            id: "m1".into(),
            delta: amount("20"),
            at: "2026-01-01T00:00:00Z".into(),
            income_ref: Some("income".into()),
        }],
        target: None,
        goal_id: None,
        created_at: "2026-01-01T00:00:00Z".into(),
    };
    let envelope_part = part(
        DistKind::Sum {
            amount: amount("20"),
        },
        DistTarget::Envelope { id: "trip".into() },
        "20",
        Some(EffectRef::EnvelopeMove {
            envelope_id: "trip".into(),
            move_id: "m1".into(),
        }),
    );
    let mut vault = doc(vec![envelope_part, remainder("80")], vec![]);
    vault.envelopes.push(envelope.clone());
    assert!(codes(&vault).is_empty());
    envelope.moves[0].income_ref = None;
    vault.envelopes = vec![envelope];
    assert_eq!(codes(&vault), vec![EFFECT_MISMATCH]);
}

#[test]
fn distribution_v3_orphan_shared_and_missing_income() {
    let orphan = doc(
        vec![store_part("20", "t1"), remainder("80")],
        vec![
            transfer("t1", "deposit", "20", Some("income")),
            transfer("t2", "deposit", "5", Some("income")),
        ],
    );
    assert_eq!(codes(&orphan), vec![EFFECT_ORPHAN]);
    let shared = doc(
        vec![
            store_part("20", "t1"),
            store_part("20", "t1"),
            remainder("60"),
        ],
        vec![transfer("t1", "deposit", "20", Some("income"))],
    );
    assert!(codes(&shared).contains(&EFFECT_SHARED));
    let mut gone = clean_doc();
    gone.transactions.remove(0);
    assert_eq!(codes(&gone), vec![INCOME_MISSING]);
}

#[test]
fn distribution_v4_no_parts_together_with_distribution() {
    let mut vault = clean_doc();
    if let Transaction::Topup(topup) = &mut vault.transactions[0] {
        topup.parts = vec![ReceivedPart {
            store_id: "card".into(),
            amount: amount("100"),
            timestamp: None,
        }];
    }
    assert_eq!(codes(&vault), vec![WITH_PARTS]);
}

#[test]
fn distribution_targets_must_be_own_live_and_the_remainder_alone_has_none() {
    let vault = clean_doc();
    let ok = [store_part("20", "t1"), remainder("80")];
    assert!(validate_distribution_targets(&vault, TxKind::Topup, &ok).is_ok());
    assert!(validate_distribution_targets(&vault, TxKind::Payment, &ok).is_err());
    let none_on_sum = part(
        DistKind::Sum {
            amount: amount("20"),
        },
        DistTarget::None,
        "20",
        None,
    );
    assert!(validate_distribution_targets(&vault, TxKind::Topup, &[none_on_sum]).is_err());
    let unknown = part(
        DistKind::Sum {
            amount: amount("20"),
        },
        DistTarget::Store {
            id: "nowhere".into(),
        },
        "20",
        None,
    );
    assert!(validate_distribution_targets(&vault, TxKind::Topup, &[unknown]).is_err());
    let mut archived = vault.clone();
    archived.stores[1].archived = true;
    assert!(validate_distribution_targets(&archived, TxKind::Topup, &ok).is_err());
}

#[test]
fn distribution_goal_row_store_is_the_goals_own_store() {
    let mut vault = clean_doc();
    let mut goal = mwt_testkit::builders::goal("Trip", "500");
    goal.id = "trip".into();
    goal.scope = mwt_model::StoreScope::store("deposit", false);
    vault.goals.push(goal);
    let row = |store: &str| {
        part(
            DistKind::Sum {
                amount: amount("20"),
            },
            DistTarget::Goal {
                goal_id: "trip".into(),
                store_id: store.into(),
            },
            "20",
            None,
        )
    };
    assert!(validate_distribution_targets(&vault, TxKind::Topup, &[row("deposit")]).is_ok());
    assert!(validate_distribution_targets(&vault, TxKind::Topup, &[row("card")]).is_err());
}
