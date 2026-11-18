//! Tests of the reports area: the old inline tests (names and bodies unchanged).

use super::overview::*;
use std::collections::BTreeMap;

use crate::Amount;
use crate::model::category::CategoryGroup;
use crate::model::store::AssetRef;
use crate::model::store::{LiabilityKind, LiabilityTerms};
use crate::model::store::{StoreKind, StoreScope};
use crate::model::transaction::PriceStamp;
use crate::model::transaction::{AccountRef, DebtEffect, Transaction};
use crate::pricing::history::unix_secs;
use crate::service::UnlockedVault;
use crate::service::WhereKind;
use crate::service::test_support::*;
use crate::service::{ContactDraft, StoreExtras, TxFilter};

fn secs(iso: &str) -> i64 {
    unix_secs(iso).unwrap()
}

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

fn spend(store: &str, sum: &str, category: &str, ts: &str) -> Transaction {
    let mut tx = payment(store, sum, "spend");
    tx.meta_mut().categories = vec![category.into()];
    dated(tx, ts)
}

fn earn(store: &str, sum: &str, ts: &str) -> Transaction {
    dated(income(store, sum, "salary"), ts)
}

fn usd_move(from: &str, to: &str, sum: &str, ts: &str) -> Transaction {
    dated(
        transfer_between(
            AccountRef::Store(from.into()),
            AccountRef::Store(to.into()),
            usd(),
            sum,
        ),
        ts,
    )
}

fn vault_with_card() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    (dir, vault, card)
}

fn range(from: &str, to: &str) -> ReportFilter {
    ReportFilter {
        from: Some(from.into()),
        to: Some(to.into()),
        ..ReportFilter::default()
    }
}

const NOW: &str = "2026-06-15T12:00:00Z";

fn run(vault: &UnlockedVault, kind: ReportKind, filter: &ReportFilter) -> Report {
    vault.report(kind, filter, secs(NOW), "UTC").unwrap()
}

#[test]
fn where_lists_spending_moves_and_what_is_left() {
    let (_dir, mut vault, card) = vault_with_card();
    let other = store(&mut vault, "Exchange", StoreKind::Bank, None);
    vault
        .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "100", "groceries", "2026-03-02T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(usd_move(&card, &other, "300", "2026-03-03T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "50", "groceries", "2026-04-02T10:00:00Z"))
        .unwrap();
    let Report::Where(report) = run(&vault, ReportKind::Where, &range("2026-03", "2026-03")) else {
        panic!()
    };
    assert_eq!(report.income, amount("1000"));
    assert_eq!(report.left, amount("900"));
    let kinds: Vec<_> = report.rows.iter().map(|row| row.kind).collect();
    assert_eq!(kinds, [WhereKind::Spending, WhereKind::Moved]);
    assert_eq!(report.rows[0].value, amount("100"));
    assert_eq!(report.rows[0].items[0].label, "groceries");
    assert_eq!(report.rows[1].count, 1);
    assert_eq!(report.rows[1].items[0].to.as_deref(), Some("Exchange"));
}

#[test]
fn cashflow_leaves_internal_moves_out() {
    let (_dir, mut vault, card) = vault_with_card();
    let pocket = store(&mut vault, "Pocket", StoreKind::Bank, Some(&card));
    vault
        .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(usd_move(&card, &pocket, "300", "2026-03-02T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "100", "groceries", "2026-04-02T10:00:00Z"))
        .unwrap();
    let Report::Cashflow(report) = run(&vault, ReportKind::Cashflow, &range("2026-03", "2026-04"))
    else {
        panic!()
    };
    assert_eq!(report.meta.months, vec!["2026-03", "2026-04"]);
    assert_eq!(report.rows[0].income, amount("1000"));
    assert_eq!(report.rows[0].daily, Amount::ZERO);
    assert_eq!(report.rows[1].daily, amount("100"));
    assert_eq!(report.total.net, amount("900"));
}

#[test]
fn categories_compare_with_the_previous_period_and_keep_ties_stable() {
    let (_dir, mut vault, card) = vault_with_card();
    vault
        .add_transaction(spend(&card, "100", "groceries", "2026-02-10T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "150", "groceries", "2026-03-10T10:00:00Z"))
        .unwrap();
    for key in ["taxi", "fuel", "sport", "pets", "gifts", "beauty"] {
        vault
            .add_transaction(spend(&card, "50", key, "2026-03-11T10:00:00Z"))
            .unwrap();
    }
    let Report::Categories(report) =
        run(&vault, ReportKind::Categories, &range("2026-03", "2026-03"))
    else {
        panic!()
    };
    let daily = report
        .groups
        .iter()
        .find(|block| block.group == Some(CategoryGroup::Daily))
        .unwrap();
    let groceries = daily
        .rows
        .iter()
        .find(|row| row.key == "groceries")
        .unwrap();
    assert_eq!(groceries.delta_pct, Some(amount("50")));
    assert_eq!(groceries.share, amount("33.3"));
    assert_eq!(
        daily
            .rows
            .iter()
            .find(|row| row.key == "taxi")
            .unwrap()
            .delta_pct,
        None
    );
    let top: Vec<&str> = report.top.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(top, vec!["groceries", "beauty", "fuel", "gifts", "pets"]);
}

#[test]
fn categories_name_the_spending_left_out_without_a_rate() {
    let (_dir, mut vault, card) = vault_with_card();
    vault
        .add_transaction(spend(&card, "100", "groceries", "2026-10-02T10:00:00Z"))
        .unwrap();
    for day in ["2026-10-03", "2026-10-05"] {
        let mut tx = spend(&card, "500", "groceries", &format!("{day}T10:00:00Z"));
        if let Transaction::Payment(inner) = &mut tx {
            inner.asset = AssetRef::fiat("THB", "Thai Baht");
            inner.price = PriceStamp::default();
        }
        vault.add_transaction(tx).unwrap();
    }
    let Report::Categories(report) =
        run(&vault, ReportKind::Categories, &range("2026-10", "2026-10"))
    else {
        panic!()
    };
    assert_eq!(report.unpriced.count, 2);
    assert_eq!(report.unpriced.days, vec!["2026-10-03", "2026-10-05"]);
    assert_eq!(report.unpriced.currencies, vec!["THB"]);
    let groceries = &report.top[0];
    assert_eq!((groceries.total, groceries.unpriced), (amount("100"), 2));

    let filter = TxFilter {
        unpriced: true,
        ..TxFilter::default()
    };
    assert_eq!(vault.list_transactions(&filter, "UTC").unwrap().len(), 2);
}

#[test]
fn year_average_skips_empty_months_and_names_the_best_and_worst() {
    let (_dir, mut vault, card) = vault_with_card();
    vault.doc.settings.savings_rate = Amount::ZERO;
    vault
        .add_transaction(earn(&card, "1000", "2026-01-05T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "900", "groceries", "2026-01-06T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(earn(&card, "1000", "2026-03-05T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "200", "groceries", "2026-03-06T10:00:00Z"))
        .unwrap();
    let filter = ReportFilter {
        year: Some(2026),
        ..ReportFilter::default()
    };
    let Report::Year(report) = run(&vault, ReportKind::Year, &filter) else {
        panic!()
    };
    assert_eq!(report.rows.len(), 12);
    assert!(report.rows[1].empty);
    assert_eq!(report.average.income, amount("1000"));
    assert_eq!(report.average.daily, amount("550"));
    assert_eq!(report.best_month.as_deref(), Some("2026-03"));
    assert_eq!(report.worst_month.as_deref(), Some("2026-01"));
    assert_eq!(report.total.savings_fact, amount("900"));
}

#[test]
fn net_worth_counts_loans_and_debts() {
    let (_dir, mut vault, card) = vault_with_card();
    let loan = vault
        .create_store_with(
            "Loan",
            StoreKind::Liability,
            "",
            "",
            None,
            StoreExtras {
                liability: Some(LiabilityTerms {
                    kind: LiabilityKind::Loan,
                    rate: None,
                    limit: None,
                    due_day: None,
                    monthly_payment: None,
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    vault
        .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(usd_move(&loan, &card, "400", "2026-04-01T10:00:00Z"))
        .unwrap();
    let friend = vault
        .create_contact(ContactDraft {
            group: String::new(),
            name: "Friend".into(),
            kind: crate::model::contact::ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
        })
        .unwrap()
        .id;
    let mut lend = transfer_between(
        AccountRef::Store(card.clone()),
        AccountRef::Contact {
            contact_id: friend,
            account_id: None,
        },
        usd(),
        "100",
    );
    if let Transaction::Transfer(tx) = &mut lend {
        tx.debt = Some(DebtEffect::Lend);
    }
    vault
        .add_transaction(dated(lend, "2026-05-01T10:00:00Z"))
        .unwrap();

    let Report::Networth(report) = run(&vault, ReportKind::Networth, &range("2026-03", "2026-05"))
    else {
        panic!()
    };
    let [march, april, may] = &report.points[..] else {
        panic!()
    };
    assert_eq!(march.net, amount("1000"));
    assert_eq!(
        (april.financial, april.liabilities, april.net),
        (amount("1400"), amount("400"), amount("1000"))
    );
    assert_eq!(
        (may.financial, may.receivables, may.net),
        (amount("1300"), amount("100"), amount("1000"))
    );
}

#[test]
fn portfolio_shares_add_up_to_a_hundred() {
    let (_dir, mut vault, card) = vault_with_card();
    let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
    vault
        .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(dated(
            income_of(&wallet, btc(), "0.02", "60000", "bought"),
            "2026-03-02T10:00:00Z",
        ))
        .unwrap();
    let Report::Portfolio(report) =
        run(&vault, ReportKind::Portfolio, &range("2026-05", "2026-06"))
    else {
        panic!()
    };
    assert_eq!(report.assets, vec!["BTC", "USD"]);
    for point in &report.points {
        let sum = point
            .shares
            .iter()
            .fold(Amount::ZERO, |acc, share| acc + *share);
        assert_eq!(sum, amount("100"), "{point:?}");
    }
}

#[test]
fn real_terms_restate_old_months_in_todays_money() {
    let (_dir, mut vault, card) = vault_with_card();
    vault
        .add_transaction(earn(&card, "1000", "2025-06-05T10:00:00Z"))
        .unwrap();
    vault
        .update_settings(crate::ipc_contract::UpdateSettingsInput {
            inflation_manual: Some(BTreeMap::from([("usd".to_string(), amount("10"))])),
            ..Default::default()
        })
        .unwrap();
    let filter = ReportFilter {
        real_terms: true,
        ..range("2025-06", "2026-06")
    };
    let Report::Cashflow(report) = vault
        .report(ReportKind::Cashflow, &filter, secs(NOW), "UTC")
        .unwrap()
    else {
        panic!()
    };
    // A year ago: 1000 × 1.1.
    assert_eq!(report.rows[0].income, amount("1100"));
    assert_eq!(report.meta.inflation_rate, Some(amount("10")));
    let Report::Inflation(inflation_report) = vault
        .report(ReportKind::Inflation, &filter, secs(NOW), "UTC")
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(inflation_report.points[0].coefficient, amount("1.1"));
    assert_eq!(
        inflation_report.points.last().unwrap().coefficient,
        amount("1")
    );
    assert_eq!(inflation_report.nominal_change_pct, Some(Amount::ZERO));
    assert!(inflation_report.real_change_pct.unwrap().is_negative());
}

#[test]
fn pnl_real_terms_round_once_per_month() {
    let (_dir, mut vault, card) = vault_with_card();
    for day in ["05", "06", "07"] {
        let mut tx = spend(&card, "1", "groceries", &format!("2025-06-{day}T10:00:00Z"));
        if let Transaction::Payment(inner) = &mut tx {
            inner.fee = fee_usd("0.333");
        }
        vault.add_transaction(tx).unwrap();
    }
    vault
        .update_settings(crate::ipc_contract::UpdateSettingsInput {
            inflation_manual: Some(BTreeMap::from([("usd".to_string(), amount("10"))])),
            ..Default::default()
        })
        .unwrap();
    let filter = ReportFilter {
        real_terms: true,
        ..range("2025-06", "2025-06")
    };
    let Report::Pnl(report) = vault
        .report(ReportKind::Pnl, &filter, secs(NOW), "UTC")
        .unwrap()
    else {
        panic!()
    };
    // 0.999 × 1.1 = 1.0989 → 1.10; rounding each fee first gave 3 × 0.37 = 1.11.
    assert_eq!(report.rows[0].fees, amount("1.10"));
    assert!(report.has_data);
}

#[test]
fn an_empty_vault_has_no_pnl_or_networth_data() {
    let (_dir, vault, _card) = vault_with_card();
    let filter = range("2026-01", "2026-06");
    let Report::Pnl(pnl) = run(&vault, ReportKind::Pnl, &filter) else {
        panic!()
    };
    assert_eq!(pnl.rows.len(), 6);
    assert!(!pnl.has_data);
    let Report::Networth(networth) = run(&vault, ReportKind::Networth, &filter) else {
        panic!()
    };
    assert!(!networth.has_data);
    let Report::Portfolio(portfolio) = run(&vault, ReportKind::Portfolio, &filter) else {
        panic!()
    };
    assert!(!portfolio.has_data);
}

#[test]
fn networth_splits_out_what_sits_in_crypto_wallets_and_exchanges() {
    let (_dir, mut vault, card) = vault_with_card();
    let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
    let ledger = store(&mut vault, "Ledger", StoreKind::Crypto, None);
    vault
        .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(earn(&exchange, "300", "2026-03-02T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(earn(&ledger, "200", "2026-03-03T10:00:00Z"))
        .unwrap();
    let Report::Networth(networth) =
        run(&vault, ReportKind::Networth, &range("2026-03", "2026-03"))
    else {
        panic!()
    };
    let point = &networth.points[0];
    assert_eq!(point.financial, amount("1500"));
    assert_eq!(point.crypto, amount("500"));
    assert_eq!(point.net, amount("1500"));
}

#[test]
fn the_current_networth_point_equals_the_summary_net_worth() {
    let (_dir, mut vault, card) = vault_with_card();
    let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
    vault
        .add_transaction(earn(&card, "1000", "2026-05-01T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(earn(&exchange, "300", "2026-06-02T10:00:00Z"))
        .unwrap();
    let Report::Networth(networth) =
        run(&vault, ReportKind::Networth, &range("2026-04", "2026-06"))
    else {
        panic!()
    };
    let last = networth.points.last().unwrap();
    assert_eq!(last.month, "2026-06");
    assert_eq!(last.net, vault.portfolio_summary().net_worth_base.round(2));
    assert_eq!(last.crypto, amount("300"));
}

#[test]
fn the_year_report_follows_the_wallet_scope() {
    let (_dir, mut vault, card) = vault_with_card();
    let other = store(&mut vault, "Other", StoreKind::Bank, None);
    vault
        .add_transaction(earn(&card, "1000", "2026-01-05T10:00:00Z"))
        .unwrap();
    vault
        .add_transaction(earn(&other, "500", "2026-01-05T10:00:00Z"))
        .unwrap();
    let filter = ReportFilter {
        year: Some(2026),
        scope: StoreScope {
            store_ids: vec![other.clone()],
            include_children: true,
        },
        ..ReportFilter::default()
    };
    let Report::Year(report) = run(&vault, ReportKind::Year, &filter) else {
        panic!()
    };
    assert_eq!(report.rows[0].income, amount("500"));
}

#[test]
fn csv_has_a_bom_semicolons_and_quotes_what_needs_it() {
    let report = Report::Stores(StoresReport {
        meta: ReportMeta {
            currency: "usd".into(),
            ..ReportMeta::default()
        },
        rows: vec![StoreReportRow {
            name: "Card; \"main\"".into(),
            ..StoreReportRow::default()
        }],
    });
    let csv = report_csv(&report);
    assert!(csv.starts_with('\u{feff}'));
    let lines: Vec<&str> = csv.trim_start_matches('\u{feff}').split("\r\n").collect();
    assert!(lines[0].starts_with("wallet;depth;"));
    assert!(lines[1].starts_with("\"Card; \"\"main\"\"\";0;"));
    assert!(lines[1].ends_with(";usd"));
}

#[test]
fn the_calendar_cuts_days_in_the_local_zone() {
    let (_dir, mut vault, card) = vault_with_card();
    vault
        .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
        .unwrap();
    // 20:00 UTC on the 1st is the 2nd in Bangkok.
    vault
        .add_transaction(spend(&card, "150", "groceries", "2026-03-01T20:00:00Z"))
        .unwrap();
    let now = secs("2026-03-20T12:00:00Z");
    let utc = vault.calendar_month("2026-03", now, "UTC").unwrap();
    let bangkok = vault
        .calendar_month("2026-03", now, "Asia/Bangkok")
        .unwrap();
    assert_eq!(utc.days[0].daily, amount("150"));
    assert_eq!(bangkok.days[0].daily, Amount::ZERO);
    assert_eq!(bangkok.days[1].daily, amount("150"));
    // 3100 a month = 100 a day (savings 10 %: 90); 150 is over.
    assert_eq!(utc.allowance, Some(amount("90")));
    assert_eq!(utc.days[0].status, DayStatus::Over);
    assert_eq!(utc.days[2].status, DayStatus::Under);
    assert_eq!(utc.days[25].status, DayStatus::Future);
    assert_eq!(utc.days[0].tx_count, 2);
    // The cell's label: the earliest operation's kind, the biggest flow's category.
    assert_eq!(
        utc.days[0].lead_kind,
        Some(crate::model::transaction::TxKind::Topup)
    );
    assert_eq!(bangkok.days[1].lead_category.as_deref(), Some("groceries"));
    assert_eq!(utc.days[2].lead_kind, None);
    assert_eq!(utc.days[2].lead_category, None);
}

#[test]
fn the_calendar_keeps_a_running_balance_and_the_days_notices() {
    let (_dir, mut vault, card) = vault_with_card();
    vault
        .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "150", "groceries", "2026-03-01T10:00:00Z"))
        .unwrap();
    vault
        .doc
        .notification_log
        .push(crate::model::notify::NoticeRecord {
            key: "alert:btc:2026-03-05T09:00:00Z".into(),
            category: crate::model::notify::NoticeCategory::PriceAlert,
            title: "BTC ≥ 90 000".into(),
            body: String::new(),
            link: Some("/automations/alerts".into()),
            at: "2026-03-05T09:00:00Z".into(),
            delivered_at: "2026-03-05T09:00:01Z".into(),
            read: false,
        });
    let now = secs("2026-03-20T12:00:00Z");
    let month = vault.calendar_month("2026-03", now, "UTC").unwrap();
    // 90 a day × 31 days = the pool; 150 spent on the 1st.
    assert_eq!(month.budget, Some(amount("2790")));
    assert_eq!(month.days[0].left, Some(amount("2640")));
    assert_eq!(month.days[0].per_day_left, Some(amount("88")));
    // A mandatory payment (utilities) does not count in the balance.
    vault
        .add_transaction(spend(&card, "1000", "utilities", "2026-03-03T10:00:00Z"))
        .unwrap();
    let month = vault.calendar_month("2026-03", now, "UTC").unwrap();
    assert_eq!(month.budget, Some(amount("2790")));
    assert_eq!(month.days[2].left, Some(amount("2640")));
    // Nor does one above the income: no day allowance is left, the balance stays.
    vault
        .add_transaction(spend(&card, "5000", "utilities", "2026-03-04T10:00:00Z"))
        .unwrap();
    let month = vault.calendar_month("2026-03", now, "UTC").unwrap();
    assert_eq!(month.allowance, None);
    assert_eq!(month.budget, Some(amount("2790")));
    assert_eq!(month.days[3].left, Some(amount("2640")));
    // Today: nothing more spent, the same rest over the 11 days after it.
    assert_eq!(month.days[19].left, Some(amount("2640")));
    assert_eq!(month.days[19].per_day_left, Some(amount("240")));
    // Ahead of today: a forecast, today's 240 a day spent through each day.
    assert!(month.days[20].forecast && !month.days[19].forecast);
    assert_eq!(month.days[20].left, Some(amount("2400")));
    assert_eq!(month.days[20].per_day_left, Some(amount("240")));
    assert_eq!(month.days[30].left, Some(amount("0")));
    assert_eq!(month.days[30].per_day_left, None);
    assert_eq!(
        month.pool,
        Some(CalendarPool {
            income: amount("3100"),
            planned_fixed: Amount::ZERO,
            savings: amount("310"),
        })
    );
    // A month wholly ahead with no income yet has no pool: nothing to forecast.
    let april = vault.calendar_month("2026-04", now, "UTC").unwrap();
    assert!(april.days[0].forecast);
    assert_eq!(april.days[0].left, None);
    // With income booked ahead, the pool is spread evenly from its first day.
    vault
        .add_transaction(earn(&card, "3000", "2026-04-01T08:00:00Z"))
        .unwrap();
    let april = vault.calendar_month("2026-04", now, "UTC").unwrap();
    let pool = april.budget.unwrap();
    assert_eq!(april.days[0].left, Some(pool - april.allowance.unwrap()));
    assert_eq!(april.days[0].per_day_left, april.allowance);
    assert_eq!(april.days[29].left, Some(Amount::ZERO));
    assert_eq!(month.days[4].notices.len(), 1);
    assert_eq!(
        month.days[4].notices[0].category,
        crate::model::notify::NoticeCategory::PriceAlert
    );
    assert!(month.days[3].notices.is_empty());
    // Past the pool: nothing to spend per day, and the forecast stays where it is.
    vault
        .add_transaction(spend(&card, "3000", "groceries", "2026-03-19T10:00:00Z"))
        .unwrap();
    let month = vault.calendar_month("2026-03", now, "UTC").unwrap();
    assert_eq!(month.days[19].left, Some(amount("-360")));
    assert_eq!(month.days[19].per_day_left, Some(Amount::ZERO));
    assert_eq!(month.days[25].left, Some(amount("-360")));
    assert_eq!(month.days[30].left, Some(amount("-360")));
}

#[test]
fn a_range_over_the_report_limit_is_refused_not_cut() {
    let (_dir, vault, _card) = vault_with_card();
    let result = vault.report(
        ReportKind::Cashflow,
        &range("2010-01", "2026-05"),
        secs(NOW),
        "UTC",
    );
    assert!(matches!(result, Err(crate::Error::InvalidInput(_))));
    // Exactly the limit is still a report.
    let Report::Cashflow(report) = run(&vault, ReportKind::Cashflow, &range("2016-07", "2026-06"))
    else {
        panic!()
    };
    assert_eq!(report.meta.months.len(), MAX_REPORT_MONTHS);
}
