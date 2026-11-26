//! `flows_for_basis` (op-period R1, R2, R6, R7, P3, P4, P7, Q3): the two
//! readings of the same books. Money is USD at price 1, the zone is fixed.

use std::collections::HashMap;

use mwt_base::Amount;
use mwt_base::calendar::Calendar;
use mwt_base::error::Result;
use mwt_model::{
    AccountRef, AssetRef, CryptoHistory, DeviceSecrets, Fee, KindDetails, MonthShare, OpPeriod,
    PaymentTx, PeriodKind, PriceStamp, ReceivedPart, Settlement, SettlementKind, SharedEvent,
    Spread, SpreadMode, Store, StoreKind, Transaction, TransferTx, TxMeta, VaultDoc,
};
use mwt_money::books::{Books, BooksSource};
use mwt_money::valuation::ValuationCurve;
use mwt_testkit::builders::{store, topup};

use crate::flows::{Basis, Flow, flows_between, flows_for_basis, may_reach_window};
use mwt_money::worth::FlowClass;

const ZONE: &str = "Europe/Moscow";

struct Source(VaultDoc);

impl BooksSource for Source {
    fn doc(&self) -> &VaultDoc {
        &self.0
    }
    fn base_and_fx(&self) -> (String, Amount) {
        ("USD".into(), Amount::parse("1").expect("one"))
    }
    fn price_usd_for(&self, _asset_key: &str) -> Amount {
        Amount::parse("1").expect("one")
    }
    fn price_base_for(&self, _asset_key: &str, fx: Amount) -> Amount {
        fx
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
    fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        Ok(DeviceSecrets::default())
    }
    fn crypto_history(&self, _cutoffs: &[i64], _in_scope: &dyn Fn(&str) -> bool) -> CryptoHistory {
        CryptoHistory::default()
    }
}

fn money(raw: &str) -> Amount {
    Amount::parse(raw).expect("amount")
}

fn bank() -> Store {
    store("Bank", StoreKind::Bank)
}

fn salary(store_id: &str, raw: &str, timestamp: &str) -> Transaction {
    let mut tx = topup(store_id, raw);
    tx.meta_mut().timestamp = timestamp.into();
    tx
}

fn expense(store_id: &str, raw: &str, fee: Option<&str>, timestamp: &str) -> Transaction {
    let mut meta = TxMeta::new("rent");
    meta.categories = vec!["rent_mortgage".into()];
    meta.timestamp = timestamp.into();
    Transaction::Payment(PaymentTx {
        meta,
        store_id: store_id.into(),
        asset: AssetRef::fiat("USD", "US Dollar"),
        amount: money(raw),
        fee: fee.map(|fee_raw| Fee {
            asset: AssetRef::fiat("USD", "US Dollar"),
            amount: money(fee_raw),
            price: PriceStamp {
                price_usd: Some(money("1")),
                ..PriceStamp::default()
            },
        }),
        to: None,
        details: KindDetails::default(),
        price: PriceStamp {
            price_usd: Some(money("1")),
            ..PriceStamp::default()
        },
    })
}

fn period(kind: PeriodKind, start: &str, end: &str) -> OpPeriod {
    OpPeriod {
        kind,
        start: start.into(),
        end: end.into(),
    }
}

fn spread(shares: &[(&str, &str)]) -> Spread {
    Spread {
        mode: SpreadMode::Manual,
        shares: shares
            .iter()
            .map(|(month, raw)| MonthShare {
                month: (*month).into(),
                amount: money(raw),
            })
            .collect(),
    }
}

fn read(transactions: Vec<Transaction>, basis: Basis) -> Vec<Flow> {
    let mut doc = VaultDoc::default();
    let account = bank();
    doc.stores.push(account.clone());
    for mut tx in transactions {
        set_store(&mut tx, &account.id);
        doc.transactions.push(tx);
    }
    let source = Source(doc);
    let books = Books::new(&source);
    let cal = Calendar::new(ZONE);
    let from = cal.start_of(date(2026, 1, 1));
    let to = cal.start_of(date(2027, 1, 1));
    flows_for_basis(&books, &cal, "USD", from, to, basis)
}

fn set_store(tx: &mut Transaction, id: &str) {
    match tx {
        Transaction::Topup(inner) => inner.store_id = id.into(),
        Transaction::Payment(inner) => inner.store_id = id.into(),
        _ => {}
    }
}

fn date(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).expect("date")
}

fn sum(flows: &[Flow]) -> Amount {
    flows
        .iter()
        .fold(Amount::ZERO, |total, flow| total + flow.value)
}

#[test]
fn p3_no_period_counts_in_the_month_of_the_money_date_in_the_zone() {
    // 22:00 UTC on 30 Nov is 01:00 on 1 Dec in Moscow.
    let flows = read(
        vec![expense("", "50", None, "2026-11-30T22:00:00Z")],
        Basis::Period,
    );
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].month, "2026-12");
    assert_eq!(flows[0].date, date(2026, 12, 1));
}

#[test]
fn p4_period_inside_one_month_counts_whole_there() {
    let mut tx = salary("", "1000", "2026-11-05T09:00:00Z");
    tx.meta_mut().period = Some(period(PeriodKind::Half, "2026-10-16", "2026-10-31"));
    let flows = read(vec![tx], Basis::Period);
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].month, "2026-10");
    assert_eq!(flows[0].date, date(2026, 11, 5));
    assert_eq!(flows[0].class, FlowClass::Income);
}

#[test]
fn q3_period_over_months_without_spread_counts_whole_in_the_money_month() {
    let mut tx = expense("", "300", None, "2026-11-20T09:00:00Z");
    tx.meta_mut().period = Some(period(PeriodKind::Quarter, "2026-10-01", "2026-12-31"));
    let flows = read(vec![tx], Basis::Period);
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].month, "2026-11");
    assert_eq!(flows[0].value, money("300"));
}

#[test]
fn r7_p7_spread_counts_by_shares_and_the_fee_follows_pro_rata() {
    let mut tx = expense("", "100", Some("10"), "2026-11-20T09:00:00Z");
    tx.meta_mut().period = Some(period(PeriodKind::Range, "2026-10-01", "2026-12-31"));
    tx.meta_mut().spread = Some(spread(&[
        ("2026-10", "33.33"),
        ("2026-11", "33.33"),
        ("2026-12", "33.34"),
    ]));
    let flows = read(vec![tx], Basis::Period);
    assert_eq!(flows.len(), 3);
    let months: Vec<&str> = flows.iter().map(|flow| flow.month.as_str()).collect();
    assert_eq!(months, ["2026-10", "2026-11", "2026-12"]);
    assert_eq!(flows[0].value, money("36.66"));
    assert_eq!(flows[1].value, money("36.66"));
    assert_eq!(sum(&flows), money("110"));
    assert!(flows.iter().all(|flow| flow.date == date(2026, 11, 20)));
}

#[test]
fn r6_income_in_parts_is_one_income_flow_of_the_total() {
    let mut tx = salary("", "120000", "2026-11-05T09:00:00Z");
    if let Transaction::Topup(inner) = &mut tx {
        inner.parts = vec![
            ReceivedPart {
                store_id: String::new(),
                amount: money("100000"),
                timestamp: None,
            },
            ReceivedPart {
                store_id: String::new(),
                amount: money("20000"),
                timestamp: Some("2026-11-07T09:00:00Z".into()),
            },
        ];
    }
    for basis in [Basis::Period, Basis::MoneyDate] {
        let mut doc_tx = tx.clone();
        // The parts name the one store of the test book.
        let mut doc = VaultDoc::default();
        let account = bank();
        doc.stores.push(account.clone());
        set_store(&mut doc_tx, &account.id);
        if let Transaction::Topup(inner) = &mut doc_tx {
            for part in &mut inner.parts {
                part.store_id = account.id.clone();
            }
        }
        doc.transactions.push(doc_tx);
        let source = Source(doc);
        let books = Books::new(&source);
        let cal = Calendar::new(ZONE);
        let flows = flows_for_basis(
            &books,
            &cal,
            "USD",
            cal.start_of(date(2026, 1, 1)),
            cal.start_of(date(2027, 1, 1)),
            basis,
        );
        assert_eq!(flows.len(), 1, "{basis:?}");
        assert_eq!(flows[0].class, FlowClass::Income);
        assert_eq!(flows[0].value, money("120000"));
    }
}

/// A vault without periods: a transfer with a fee in another currency, an
/// income and an expense in two months, and a write-off of a shared event.
fn plain_vault() -> VaultDoc {
    let (bank_store, cash_store) = (bank(), store("Cash", StoreKind::Cash));
    let mut doc = VaultDoc::default();
    let mut transfer_meta = TxMeta::new("move");
    transfer_meta.timestamp = "2026-11-08T09:00:00Z".into();
    doc.transactions.extend([
        salary(&bank_store.id, "1000", "2026-11-05T09:00:00Z"),
        expense(&bank_store.id, "40", Some("1"), "2026-11-06T09:00:00Z"),
        Transaction::Transfer(TransferTx {
            meta: transfer_meta,
            from: AccountRef::Store(bank_store.id.clone()),
            to: AccountRef::Store(cash_store.id.clone()),
            asset: AssetRef::fiat("USD", "US Dollar"),
            amount: money("200"),
            fee: Some(Fee {
                asset: AssetRef::fiat("EUR", "Euro"),
                amount: money("3"),
                price: PriceStamp {
                    price_usd: Some(money("1.1")),
                    ..PriceStamp::default()
                },
            }),
            network: None,
            tx_hash: None,
            debt: None,
            price: PriceStamp {
                price_usd: Some(money("1")),
                ..PriceStamp::default()
            },
        }),
        salary(&bank_store.id, "900", "2026-12-01T09:00:00Z"),
        expense(&cash_store.id, "70", None, "2026-12-02T09:00:00Z"),
    ]);
    doc.stores.extend([bank_store, cash_store]);
    doc.shared_events.push(SharedEvent {
        id: "trip".into(),
        currency: "usd".into(),
        i_participate: true,
        settlements: vec![Settlement {
            id: "writeoff".into(),
            kind: SettlementKind::WriteOff,
            participant_id: "friend".into(),
            amount: money("25"),
            at: "2026-11-12T09:00:00Z".into(),
            ..Settlement::default()
        }],
        ..SharedEvent::default()
    });
    doc
}

#[test]
fn r1_r8_money_date_basis_is_flows_between_and_no_period_reads_the_same() {
    let source = Source(plain_vault());
    let books = Books::new(&source);
    let cal = Calendar::new(ZONE);
    for (from, to) in [
        (date(2026, 11, 1), date(2026, 12, 1)),
        (date(2026, 11, 1), date(2027, 1, 1)),
        (date(2026, 11, 10), date(2026, 12, 2)),
    ] {
        let (from, to) = (cal.start_of(from), cal.start_of(to));
        let between = flows_between(&books, &cal, "USD", from, to);
        assert!(between.len() >= 2, "the fixture reaches the window");
        let by_money = flows_for_basis(&books, &cal, "USD", from, to, Basis::MoneyDate);
        let by_period = flows_for_basis(&books, &cal, "USD", from, to, Basis::Period);
        for flows in [&by_money, &by_period] {
            assert_eq!(flows.len(), between.len());
            for (left, right) in flows.iter().zip(&between) {
                assert_eq!(
                    (left.date, &left.month, left.class, left.value, &left.tx_id),
                    (
                        right.date,
                        &right.month,
                        right.class,
                        right.value,
                        &right.tx_id
                    )
                );
            }
        }
    }
}

#[test]
fn a_month_overlapping_the_window_counts_even_when_it_starts_before_it() {
    // 16-31 Oct, paid 5 Nov; the window 10 Oct .. 10 Nov holds the rest of October.
    let mut tx = expense("", "90", None, "2026-11-05T09:00:00Z");
    tx.meta_mut().period = Some(period(PeriodKind::Range, "2026-10-16", "2026-10-31"));
    let mut doc = VaultDoc::default();
    let account = bank();
    doc.stores.push(account.clone());
    set_store(&mut tx, &account.id);
    doc.transactions.push(tx);
    let source = Source(doc);
    let books = Books::new(&source);
    let cal = Calendar::new(ZONE);
    let (from, to) = (
        cal.start_of(date(2026, 10, 10)),
        cal.start_of(date(2026, 11, 10)),
    );
    let flows = flows_for_basis(&books, &cal, "USD", from, to, Basis::Period);
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].month, "2026-10");
}

#[test]
fn r2_spread_reaches_a_month_window_the_money_date_is_outside_of() {
    let mut tx = expense("", "90", None, "2026-11-20T09:00:00Z");
    tx.meta_mut().period = Some(period(PeriodKind::Range, "2026-11-01", "2026-12-31"));
    tx.meta_mut().spread = Some(spread(&[("2026-11", "30"), ("2026-12", "60")]));
    let mut doc = VaultDoc::default();
    let account = bank();
    doc.stores.push(account.clone());
    set_store(&mut tx, &account.id);
    doc.transactions.push(tx);
    let source = Source(doc);
    let books = Books::new(&source);
    let cal = Calendar::new(ZONE);
    let (from, to) = (
        cal.start_of(date(2026, 12, 1)),
        cal.start_of(date(2027, 1, 1)),
    );
    let december = flows_for_basis(&books, &cal, "USD", from, to, Basis::Period);
    assert_eq!(december.len(), 1);
    assert_eq!(december[0].month, "2026-12");
    assert_eq!(december[0].value, money("60"));
    assert!(flows_for_basis(&books, &cal, "USD", from, to, Basis::MoneyDate).is_empty());
}

#[test]
fn an_operation_outside_the_window_in_money_and_months_is_skipped_before_valuing() {
    let cal = Calendar::new(ZONE);
    let (from, to) = (
        cal.start_of(date(2026, 11, 1)),
        cal.start_of(date(2026, 12, 1)),
    );
    let far = expense("", "5", None, "2026-03-05T09:00:00Z");
    assert!(!may_reach_window(&cal, &far, from, to));
    let mut spread_in = far.clone();
    spread_in.meta_mut().spread = Some(spread(&[("2026-03", "2"), ("2026-11", "3")]));
    assert!(may_reach_window(&cal, &spread_in, from, to));
    let inside = expense("", "5", None, "2026-11-05T09:00:00Z");
    assert!(may_reach_window(&cal, &inside, from, to));
}

#[test]
fn r3_last_month_savings_keeps_the_money_date() {
    // Salary paid 3 Nov for 16-31 Oct: on 5 Nov it is November's, not October's.
    let mut tx = salary("", "1000", "2026-11-03T09:00:00Z");
    tx.meta_mut().period = Some(period(PeriodKind::Half, "2026-10-16", "2026-10-31"));
    let mut doc = VaultDoc::default();
    let account = bank();
    doc.stores.push(account.clone());
    set_store(&mut tx, &account.id);
    doc.transactions.push(tx);
    let source = Source(doc);
    let books = Books::new(&source);
    let now = Calendar::new(ZONE).start_of(date(2026, 11, 5));
    let (saved, month) = crate::allocation::last_month_savings(&books, now, ZONE, "USD");
    assert_eq!(month, "2026-10");
    assert_eq!(saved, Amount::ZERO);
}
