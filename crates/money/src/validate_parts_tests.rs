//! Income that arrived in parts (spec op-period S1–S5, S14, A5).

use mwt_base::Amount;
use mwt_base::error::Error;
use mwt_model::{
    AssetRef, Category, CategoryGroup, DistKind, DistTarget, DistributionPart, KindDetails,
    MonthShare, OpPeriod, PartStatus, PeriodKind, PriceStamp, ReceivedPart, Spread, SpreadMode,
    Store, StoreKind, TopupTx, Transaction, TxMeta, VaultDoc,
};

use super::validate_transaction_at;
use crate::transactions::normalize_parts;

const NOW_MS: i64 = 1_800_000_000_000;

fn amount(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

fn store(id: &str, kind: StoreKind) -> Store {
    let mut store = Store::new(id, kind, "#000");
    store.id = id.into();
    store
}

fn doc() -> VaultDoc {
    VaultDoc {
        stores: vec![
            store("card", StoreKind::Bank),
            store("cash", StoreKind::Cash),
        ],
        categories: vec![Category {
            key: "salary".into(),
            name: None,
            group: CategoryGroup::Income,
            archived: false,
            essential: None,
            passive: None,
            period_rule: None,
        }],
        ..VaultDoc::default()
    }
}

fn part(store_id: &str, raw: &str, timestamp: Option<&str>) -> ReceivedPart {
    ReceivedPart {
        store_id: store_id.into(),
        amount: amount(raw),
        timestamp: timestamp.map(str::to_string),
    }
}

fn income(parts: Vec<ReceivedPart>) -> TopupTx {
    let mut meta = TxMeta::new("salary");
    meta.timestamp = "2026-11-05T09:00:00Z".into();
    meta.categories = vec!["salary".into()];
    TopupTx {
        meta,
        store_id: "card".into(),
        asset: AssetRef::fiat("RUB", "Rouble"),
        amount: amount("120000"),
        fee: None,
        from: None,
        details: KindDetails::default(),
        price: PriceStamp::default(),
        parts,
        distribution: None,
    }
}

fn two_parts() -> Vec<ReceivedPart> {
    vec![
        part("card", "100000", None),
        part("cash", "20000", Some("2026-11-07T09:00:00Z")),
    ]
}

fn check(tx: TopupTx) -> mwt_base::Result<()> {
    validate_transaction_at(&doc(), &Transaction::Topup(tx), NOW_MS)
}

fn refused(result: mwt_base::Result<()>) {
    assert!(matches!(result, Err(Error::InvalidInput(_))), "{result:?}");
}

#[test]
fn parts_that_add_up_pass() {
    check(income(two_parts())).unwrap();
}

#[test]
fn parts_must_sum_exactly_to_the_amount() {
    refused(check(income(vec![
        part("card", "100000", None),
        part("cash", "19999", None),
    ])));
    refused(check(income(vec![
        part("card", "100000", None),
        part("cash", "20001", None),
    ])));
}

#[test]
fn one_part_is_refused() {
    refused(check(income(vec![part("card", "120000", None)])));
}

#[test]
fn a_negative_part_is_refused() {
    refused(check(income(vec![
        part("card", "130000", None),
        part("cash", "-10000", None),
    ])));
}

#[test]
fn a_part_on_an_unknown_store_is_refused() {
    let result = check(income(vec![
        part("card", "100000", None),
        part("gone", "20000", None),
    ]));
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn a_part_with_a_broken_date_is_refused() {
    refused(check(income(vec![
        part("card", "100000", None),
        part("cash", "20000", Some("tomorrow")),
    ])));
}

#[test]
fn parts_together_with_a_distribution_are_refused() {
    let mut tx = income(two_parts());
    tx.distribution = Some(vec![DistributionPart {
        kind: DistKind::Remainder,
        target: DistTarget::None,
        amount: amount("120000"),
        status: PartStatus::Done,
        effect: None,
    }]);
    refused(check(tx));
}

#[test]
fn zero_parts_are_dropped_and_the_first_store_is_kept() {
    let mut tx = Transaction::Topup(income(vec![
        part("cash", "0", None),
        part("card", "100000", None),
        part("cash", "20000", None),
    ]));
    normalize_parts(&mut tx);
    let Transaction::Topup(inner) = &tx else {
        unreachable!()
    };
    assert_eq!(inner.parts.len(), 2);
    assert_eq!(inner.store_id, "card");
    validate_transaction_at(&doc(), &tx, NOW_MS).unwrap();
}

#[test]
fn a_period_and_a_spread_may_sit_on_an_income_in_parts() {
    let mut tx = income(two_parts());
    tx.meta.period = Some(OpPeriod {
        kind: PeriodKind::Range,
        start: "2026-10-16".into(),
        end: "2026-11-15".into(),
    });
    tx.meta.spread = Some(Spread {
        mode: SpreadMode::Manual,
        shares: vec![
            MonthShare {
                month: "2026-10".into(),
                amount: amount("60000"),
            },
            MonthShare {
                month: "2026-11".into(),
                amount: amount("60000"),
            },
        ],
    });
    check(tx).unwrap();
}

#[test]
fn two_legs_each_on_its_own_store_and_date_and_no_transfer() {
    let tx = Transaction::Topup(income(two_parts()));
    let legs = tx.legs();
    assert_eq!(legs.len(), 2);
    assert_eq!(
        (
            legs[0].store_id.as_str(),
            legs[0].amount,
            &legs[0].timestamp
        ),
        ("card", amount("100000"), &None)
    );
    assert_eq!(legs[1].store_id, "cash");
    assert_eq!(legs[1].amount, amount("20000"));
    assert_eq!(legs[1].timestamp.as_deref(), Some("2026-11-07T09:00:00Z"));
    assert!(legs.iter().all(|leg| leg.to_store_id.is_none()));
    assert!(matches!(tx, Transaction::Topup(_)));
}

#[test]
fn a_part_dated_far_in_the_future_is_refused_like_an_operation() {
    let mut dated = income(two_parts());
    dated.meta.timestamp = "2099-01-01T00:00:00Z".into();
    let operation = check(dated).unwrap_err().to_string();
    let part_error = check(income(vec![
        part("card", "100000", None),
        part("cash", "20000", Some("2099-01-01T00:00:00Z")),
    ]))
    .unwrap_err()
    .to_string();
    assert_eq!(part_error, operation);
}
