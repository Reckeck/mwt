//! The report basis (op-period R4, Q10): the same books read by the money date
//! and by the period. Money is USD at price 1, the zone is fixed.

use std::collections::HashMap;

use mwt_base::Amount;
use mwt_base::error::Result;
use mwt_model::{
    CryptoHistory, DeviceSecrets, OpPeriod, PeriodKind, StoreKind, Transaction, VaultDoc,
};
use mwt_money::books::{Books, BooksSource};
use mwt_money::valuation::ValuationCurve;
use mwt_planning::flows::Basis;
use mwt_testkit::builders::{store, topup};

use crate::overview::{CashflowReport, Report, ReportFilter, ReportKind, report};
use crate::where_went::WhereKind;

const ZONE: &str = "Europe/Moscow";
/// 2026-11-20 12:00 UTC.
const NOW: i64 = 1_795_262_400;

struct Source(VaultDoc);

impl BooksSource for Source {
    fn doc(&self) -> &VaultDoc {
        &self.0
    }
    fn base_and_fx(&self) -> (String, Amount) {
        ("USD".into(), money("1"))
    }
    fn price_usd_for(&self, _asset_key: &str) -> Amount {
        money("1")
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

/// An advance paid on 5 Nov «for» 16-31 Oct, and a plain income on 10 Nov.
fn doc() -> VaultDoc {
    let mut doc = VaultDoc::default();
    let bank = store("Bank", StoreKind::Bank);
    let mut advance = topup(&bank.id, "1000");
    advance.meta_mut().timestamp = "2026-11-05T09:00:00Z".into();
    advance.meta_mut().period = Some(OpPeriod {
        kind: PeriodKind::Half,
        start: "2026-10-16".into(),
        end: "2026-10-31".into(),
    });
    let mut plain = topup(&bank.id, "500");
    plain.meta_mut().timestamp = "2026-11-10T09:00:00Z".into();
    doc.stores.push(bank);
    doc.transactions.push(advance);
    doc.transactions.push(plain);
    doc
}

fn filter(basis: Option<Basis>) -> ReportFilter {
    ReportFilter {
        from: Some("2026-10".into()),
        to: Some("2026-11".into()),
        currency: Some("usd".into()),
        basis,
        ..ReportFilter::default()
    }
}

fn read(kind: ReportKind, basis: Option<Basis>) -> Report {
    let source = Source(doc());
    report(&Books::new(&source), kind, &filter(basis), NOW, ZONE).expect("report")
}

fn cashflow(basis: Option<Basis>) -> CashflowReport {
    match read(ReportKind::Cashflow, basis) {
        Report::Cashflow(data) => data,
        other => panic!("not a cashflow report: {other:?}"),
    }
}

#[test]
fn period_basis_puts_the_income_in_the_month_it_is_for() {
    for basis in [None, Some(Basis::Period)] {
        let data = cashflow(basis);
        assert_eq!(data.rows[0].month, "2026-10");
        assert_eq!(data.rows[0].income, money("1000"));
        assert_eq!(data.rows[1].income, money("500"));
        assert_eq!(data.rows[0].income_by_money_date, Amount::ZERO);
        assert_eq!(data.rows[1].income_by_money_date, money("1500"));
        assert_eq!(data.total.income, money("1500"));
        assert_eq!(data.total.income_by_money_date, money("1500"));
    }
}

#[test]
fn money_date_basis_puts_the_income_in_the_month_of_its_date() {
    let data = cashflow(Some(Basis::MoneyDate));
    assert_eq!(data.rows[0].income, Amount::ZERO);
    assert_eq!(data.rows[1].income, money("1500"));
    assert_eq!(data.rows[0].income_by_money_date, Amount::ZERO);
    assert_eq!(data.rows[1].income_by_money_date, money("1500"));
}

#[test]
fn year_report_follows_the_basis() {
    let year = |basis| {
        let source = Source(doc());
        let filter = ReportFilter {
            year: Some(2026),
            basis,
            ..filter(basis)
        };
        match report(&Books::new(&source), ReportKind::Year, &filter, NOW, ZONE).expect("year") {
            Report::Year(data) => data,
            other => panic!("not a year report: {other:?}"),
        }
    };
    assert_eq!(year(Some(Basis::Period)).rows[9].income, money("1000"));
    assert_eq!(year(Some(Basis::MoneyDate)).rows[9].income, Amount::ZERO);
    assert_eq!(year(Some(Basis::MoneyDate)).rows[10].income, money("1500"));
}

#[test]
fn categories_follow_the_basis() {
    let cells = |basis| match read(ReportKind::Categories, basis) {
        Report::Categories(data) => data.groups[0].rows[0].cells.clone(),
        other => panic!("not a categories report: {other:?}"),
    };
    assert_eq!(cells(Some(Basis::Period)), [money("1000"), money("500")]);
    assert_eq!(cells(Some(Basis::MoneyDate)), [Amount::ZERO, money("1500")]);
}

#[test]
fn where_went_income_is_the_same_total_in_both_readings() {
    // The whole range holds both months, so only the split between months
    // differs; the total income of the range does not.
    for basis in [Some(Basis::Period), Some(Basis::MoneyDate)] {
        match read(ReportKind::Where, basis) {
            Report::Where(data) => {
                assert_eq!(data.income, money("1500"));
                assert!(data.rows.iter().all(|row| row.kind != WhereKind::Lent));
            }
            other => panic!("not a where report: {other:?}"),
        }
    }
}
