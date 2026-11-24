//! Period and spread rules (spec op-period P1, P2, P5, P6, A6).

use mwt_base::Amount;
use mwt_base::error::Error;
use mwt_model::{MonthShare, OpPeriod, PeriodKind, Spread, SpreadMode, TxKind, TxMeta};

use super::{check_period_meta, derive_shares};

fn amount(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

fn period(kind: PeriodKind, start: &str, end: &str) -> OpPeriod {
    OpPeriod {
        kind,
        start: start.into(),
        end: end.into(),
    }
}

fn shares(parts: &[(&str, &str)]) -> Vec<MonthShare> {
    parts
        .iter()
        .map(|(month, value)| MonthShare {
            month: (*month).into(),
            amount: amount(value),
        })
        .collect()
}

fn meta(period: Option<OpPeriod>, spread: Option<Spread>) -> TxMeta {
    TxMeta {
        period,
        spread,
        ..TxMeta::default()
    }
}

fn check(kind: TxKind, meta: &TxMeta) -> mwt_base::Result<()> {
    check_period_meta(kind, meta, Some(amount("100.00")))
}

fn refused(result: mwt_base::Result<()>) {
    assert!(matches!(result, Err(Error::InvalidInput(_))), "{result:?}");
}

fn q4() -> OpPeriod {
    period(PeriodKind::Range, "2026-10-16", "2026-12-15")
}

fn manual(parts: &[(&str, &str)]) -> TxMeta {
    meta(
        Some(q4()),
        Some(Spread {
            mode: SpreadMode::Manual,
            shares: shares(parts),
        }),
    )
}

#[test]
fn a_period_is_only_for_income_and_expense() {
    let labelled = meta(
        Some(period(PeriodKind::Month, "2026-10-01", "2026-10-31")),
        None,
    );
    check(TxKind::Topup, &labelled).unwrap();
    check(TxKind::Payment, &labelled).unwrap();
    for kind in [
        TxKind::Transfer,
        TxKind::Convert,
        TxKind::Adjustment,
        TxKind::AssetSell,
    ] {
        refused(check(kind, &labelled));
    }
}

#[test]
fn normal_forms_pass_and_malformed_ones_are_refused() {
    let good = [
        (PeriodKind::Month, "2026-02-01", "2026-02-28"),
        (PeriodKind::Month, "2028-02-01", "2028-02-29"),
        (PeriodKind::Half, "2026-10-01", "2026-10-15"),
        (PeriodKind::Half, "2026-10-16", "2026-10-31"),
        (PeriodKind::Quarter, "2026-07-01", "2026-09-30"),
        (PeriodKind::Quarter, "2026-10-01", "2026-12-31"),
        (PeriodKind::Year, "2026-01-01", "2026-12-31"),
        (PeriodKind::Range, "2026-10-16", "2026-10-16"),
        (PeriodKind::Range, "2026-01-01", "2027-01-01"),
    ];
    for (kind, start, end) in good {
        check(TxKind::Payment, &meta(Some(period(kind, start, end)), None)).unwrap();
    }
    let bad = [
        (PeriodKind::Month, "2026-10-02", "2026-10-31"),
        (PeriodKind::Month, "2026-02-01", "2026-02-29"),
        (PeriodKind::Half, "2026-10-02", "2026-10-15"),
        (PeriodKind::Half, "2026-10-01", "2026-10-16"),
        (PeriodKind::Quarter, "2026-02-01", "2026-04-30"),
        (PeriodKind::Quarter, "2026-10-01", "2026-12-30"),
        (PeriodKind::Year, "2026-01-01", "2027-12-31"),
        (PeriodKind::Range, "2026-10-16", "2026-10-15"),
        (PeriodKind::Range, "2026-01-01", "2027-01-02"),
        (PeriodKind::Range, "2026-1-01", "2026-10-15"),
        (PeriodKind::Range, "soon", "2026-10-15"),
    ];
    for (kind, start, end) in bad {
        refused(check(
            TxKind::Payment,
            &meta(Some(period(kind, start, end)), None),
        ));
    }
}

#[test]
fn a_quarter_without_a_spread_is_not_blocked() {
    let quarter = period(PeriodKind::Quarter, "2026-07-01", "2026-09-30");
    check(TxKind::Topup, &meta(Some(quarter), None)).unwrap();
}

#[test]
fn a_spread_needs_a_period_over_several_months() {
    let one_share = Spread {
        mode: SpreadMode::Manual,
        shares: shares(&[("2026-10", "100.00")]),
    };
    let inside = period(PeriodKind::Month, "2026-10-01", "2026-10-31");
    refused(check(
        TxKind::Payment,
        &meta(Some(inside), Some(one_share.clone())),
    ));
    refused(check(TxKind::Payment, &meta(None, Some(one_share))));
}

#[test]
fn even_spread_gives_the_rest_to_the_last_month() {
    let derived = derive_shares(amount("100.00"), &q4(), SpreadMode::Even, 2).unwrap();
    assert_eq!(
        derived,
        shares(&[
            ("2026-10", "33.33"),
            ("2026-11", "33.33"),
            ("2026-12", "33.34")
        ])
    );
    let stored = meta(
        Some(q4()),
        Some(Spread {
            mode: SpreadMode::Even,
            shares: derived,
        }),
    );
    check(TxKind::Payment, &stored).unwrap();
}

#[test]
fn by_days_spread_sums_to_the_amount() {
    let derived = derive_shares(amount("100.00"), &q4(), SpreadMode::ByDays, 2).unwrap();
    assert_eq!(derived.len(), 3);
    let total = derived.iter().fold(Amount::ZERO, |sum, share| {
        sum.checked_add(share.amount).unwrap()
    });
    assert_eq!(total, amount("100.00"));
    let stored = meta(
        Some(q4()),
        Some(Spread {
            mode: SpreadMode::ByDays,
            shares: derived,
        }),
    );
    check(TxKind::Payment, &stored).unwrap();
}

#[test]
fn manual_shares_must_sum_to_the_amount() {
    let months = [("2026-10", "10"), ("2026-11", "20"), ("2026-12", "70")];
    check(TxKind::Payment, &manual(&months)).unwrap();
    refused(check(
        TxKind::Payment,
        &manual(&[("2026-10", "10"), ("2026-11", "20"), ("2026-12", "60")]),
    ));
}

#[test]
fn shares_need_one_month_each_and_none_negative() {
    refused(check(
        TxKind::Payment,
        &manual(&[("2026-10", "40"), ("2026-11", "60")]),
    ));
    refused(check(
        TxKind::Payment,
        &manual(&[("2026-10", "10"), ("2026-12", "20"), ("2026-11", "70")]),
    ));
    refused(check(
        TxKind::Payment,
        &manual(&[("2026-10", "-10"), ("2026-11", "40"), ("2026-12", "70")]),
    ));
}
