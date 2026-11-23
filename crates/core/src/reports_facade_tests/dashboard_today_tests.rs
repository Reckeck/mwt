//! Tests of the dashboard_today area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::calc::invest::{CapFreq, ContributionStage, Freq, InvestInput, TaxRule, Timing};
use crate::household::utilities::UtilityDraft;
use crate::model::planning::ScenarioSource;
use crate::model::planning::UtilityKind;
use crate::model::store::StoreScope;
use crate::model::store::{LoyaltyInfo, PropertyInfo, StoreKind};
use crate::model::vault::DashPeriod;
use crate::planning::scenarios::CalcScenarioDraft;
use crate::service::StoreExtras;
use crate::service::test_support::*;
use crate::time::unix_secs;
use chrono::NaiveDate;
use mwt_reports::dashboard_today::*;

#[test]
fn months_since_counts_whole_months_from_the_start() {
    let day = |raw: &str| NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap();
    assert_eq!(months_since("2026-01", day("2026-04-10")), 3);
    assert_eq!(months_since("2026-05", day("2026-04-10")), 0);
    assert_eq!(months_since("bad", day("2026-04-10")), 0);
}

#[test]
fn a_scenario_shows_its_plan_to_date_against_its_wallets() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(&mut vault, income(&card, "1500", "pay"))
        .unwrap();
    let scenario = crate::usecases::planning::scenarios::save_calc_scenario(
        &mut vault,
        CalcScenarioDraft {
            id: None,
            name: "Plan".into(),
            input: InvestInput {
                currency: "usd".into(),
                start: "2026-01".into(),
                initial: amount("1000"),
                rate_pct: Amount::ZERO,
                term_months: 12,
                capitalization: CapFreq::Monthly,
                stages: vec![ContributionStage {
                    from_month: 1,
                    amount: amount("100"),
                    freq: Freq::Monthly,
                    timing: Timing::End,
                }],
                indexation_pct: Amount::ZERO,
                inflation_pct: Amount::ZERO,
                tax: TaxRule::default(),
                withdrawal: None,
            },
            source: Some(ScenarioSource {
                scope: StoreScope {
                    store_ids: vec![card],
                    include_children: false,
                },
                income_months: 3,
                income_pct: amount("10"),
            }),
            prefilled_at: None,
        },
    )
    .unwrap();
    let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
    let status =
        crate::reports::dashboard_today::scenario_status(&vault.books(), &scenario.id, now, "UTC")
            .unwrap();
    assert_eq!(status.elapsed_months, 3);
    assert_eq!(status.planned_now, amount("1300"));
    assert_eq!(status.final_nominal, amount("2200"));
    assert_eq!(status.fact, Some(amount("1500")));
    assert!(
        crate::reports::dashboard_today::scenario_status(&vault.books(), "missing", now, "UTC")
            .is_err()
    );
}

#[test]
fn the_strip_keeps_three_todos_due_first() {
    let (_dir, mut vault) = empty_vault();
    let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
    for (name, date) in [
        ("Late", "2026-04-20"),
        ("Soon", "2026-04-12"),
        ("Far", "2026-06-01"),
        ("Mid", "2026-04-15"),
    ] {
        let id = store(&mut vault, name, StoreKind::Loyalty, None);
        let found = vault
            .doc
            .stores
            .iter_mut()
            .find(|item| item.id == id)
            .unwrap();
        found.loyalty = Some(LoyaltyInfo {
            points_symbol: name.to_uppercase(),
            point_value: amount("1"),
            currency: "rub".into(),
            expires_at: Some(date.into()),
            expiring_points: Some(amount("10")),
            login: None,
            source_store_ids: Vec::new(),
            unit: Default::default(),
        });
    }
    let todos = mwt_reports::dashboard_today::all_today_todos(&vault.books(), now, "UTC", None);
    let titles: Vec<_> = todos.iter().map(|todo| todo.title.as_str()).collect();
    assert_eq!(titles, ["Soon", "Mid", "Late"]);
    assert!(
        todos
            .iter()
            .all(|todo| todo.kind == TodayTodoKind::PointsExpire)
    );

    let today = crate::reports::dashboard_today::dashboard_today(
        &vault.books(),
        DashPeriod::Month,
        now,
        "UTC",
    );
    assert_eq!(today.todos.len(), TODAY_TODOS);
    assert_eq!(today.todos_total, 3);
}

#[test]
fn unpaid_utilities_past_the_pay_day_and_todays_reminder_are_things_to_do() {
    let (_dir, mut vault) = empty_vault();
    let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
    let flat = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Квартира",
        StoreKind::Property,
        "#555",
        "",
        None,
        StoreExtras {
            property: Some(PropertyInfo {
                utilities_pay_day: Some(5),
                ..PropertyInfo::default()
            }),
            ..StoreExtras::default()
        },
    )
    .unwrap();
    crate::usecases::household::utilities::create_utility_item(
        &mut vault,
        UtilityDraft {
            name: "ЖКХ".into(),
            active: true,
            kind: UtilityKind::Static {
                amount: amount("120"),
            },
            currency: "usd".into(),
            property_id: flat.id.clone(),
            store_id: None,
            category: "utilities".into(),
        },
    )
    .unwrap();
    crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Зарплата",
        "2026-04-10T08:00:00Z",
        None,
    )
    .unwrap();
    crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Потом",
        "2026-04-12T08:00:00Z",
        None,
    )
    .unwrap();

    let todos =
        mwt_reports::dashboard_today::all_today_todos(&vault.books(), now, "UTC", Some("usd"));
    let kinds: Vec<_> = todos
        .iter()
        .map(|todo| (todo.kind, todo.title.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (TodayTodoKind::UtilityOverdue, "Квартира"),
            (TodayTodoKind::Reminder, "Зарплата"),
        ]
    );
    assert_eq!(todos[0].date.as_deref(), Some("2026-04-05"));
    assert_eq!(todos[0].amount, Some(amount("120")));
}
