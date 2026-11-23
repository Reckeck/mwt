//! Tests of the goals area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::model::ids::StoreId;
use crate::model::store::StoreScope;
use crate::planning::goals::*;
use crate::service::Session;

#[test]
fn a_far_deadline_with_a_return_does_not_panic() {
    let huge = amount("1000000000000000000");
    assert_eq!(
        need_with_return(huge, huge * Amount::from(2), amount("0.5"), 1200),
        Amount::ZERO
    );
    let _ = need_with_return(Amount::ZERO, huge, amount("0.5"), 1200);
}

use crate::model::store::StoreKind;
use crate::model::transaction::AccountRef;
use crate::model::vault::PricePoint;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn secs(iso: &str) -> i64 {
    unix_secs(iso).unwrap()
}

fn goal(name: &str, target: &str, stores: &[&str], priority: u8) -> GoalDraft {
    GoalDraft {
        name: name.into(),
        active: true,
        target_amount: amount(target),
        currency: "usd".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: Some("2026-12-31T00:00:00Z".into()),
        priority,
        monthly_commitment: None,
        scope: StoreScope {
            store_ids: stores.iter().map(|id| id.to_string()).collect(),
            include_children: true,
        },
        scenario_id: None,
        expected_return: None,
    }
}

fn deposit(vault: &mut Session, store: &str, sum: &str, ts: &str) {
    let mut tx = income(store, sum, "salary");
    at(&mut tx, ts);
    crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
}

fn setup_savings() -> (tempfile::TempDir, Session, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    (dir, vault, savings)
}

#[test]
fn two_goals_on_one_account_split_it_and_never_exceed_their_target() {
    let (_dir, mut vault, savings) = setup_savings();
    deposit(&mut vault, &savings, "3000", "2026-02-01T00:00:00Z");
    // Same months: weights 1200 : 2400 → shares 1 : 2.
    crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal("Laptop", "1200", &[&savings], 3),
    )
    .unwrap();
    crate::usecases::planning::goals::create_goal(&mut vault, goal("Trip", "2400", &[&savings], 3))
        .unwrap();
    let progress = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-03-01T00:00:00Z"),
        "UTC",
    );
    let by_name = |name: &str| progress.iter().find(|item| item.goal.name == name).unwrap();
    assert_eq!(by_name("Laptop").saved, amount("1000"));
    assert_eq!(by_name("Trip").saved, amount("2000"));

    // 4000 is more than both need: each is capped at its target.
    deposit(&mut vault, &savings, "1000", "2026-02-02T00:00:00Z");
    deposit(&mut vault, &savings, "2000", "2026-02-03T00:00:00Z");
    let capped = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-03-01T00:00:00Z"),
        "UTC",
    );
    let laptop = capped
        .iter()
        .find(|item| item.goal.name == "Laptop")
        .unwrap();
    let trip = capped.iter().find(|item| item.goal.name == "Trip").unwrap();
    assert_eq!(laptop.saved, amount("1200"));
    assert_eq!(trip.saved, amount("2400"));
    assert_eq!(laptop.pct, amount("100"));
}

#[test]
fn need_per_month_and_both_realism_reasons() {
    let (_dir, mut vault, savings) = setup_savings();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    // A month with 1000 income and 900 spent: 100 left over.
    deposit(&mut vault, &card, "1000", "2026-01-05T00:00:00Z");
    let mut spend = payment(&card, "900", "food");
    at(&mut spend, "2026-01-06T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, spend).unwrap();

    let mut draft = goal("Car", "12000", &[&savings], 5);
    draft.monthly_commitment = Some(amount("500"));
    crate::usecases::planning::goals::create_goal(&mut vault, draft).unwrap();
    // February: 11 months left (Feb–Dec), nothing saved.
    let progress = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-02-10T00:00:00Z"),
        "UTC",
    );
    let car = &progress[0];
    assert_eq!(car.months_total, Some(12));
    assert_eq!(car.months_left, Some(11));
    assert_eq!(car.need_per_month.unwrap().round(2), amount("1090.91"));
    assert!(!car.realistic);
    assert_eq!(car.issues.len(), 2, "{:?}", car.issues);
    assert!(matches!(car.issues[0], RealismIssue::OverCommitment { .. }));
    let RealismIssue::OverSurplus { surplus, .. } = &car.issues[1] else {
        panic!()
    };
    assert_eq!(*surplus, amount("100"));
}

#[test]
fn crypto_goal_contributed_stays_while_saved_follows_the_rate() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
    let mut buy = income_of(&wallet, btc(), "1", "50000", "bought");
    at(&mut buy, "2026-01-10T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();
    vault.doc.price_cache.push(PricePoint {
        asset_key: btc().key(),
        value_usd: amount("80000"),
        ts: "2026-02-01T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    });
    crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal("House", "100000", &[&wallet], 3),
    )
    .unwrap();
    let progress = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-02-10T00:00:00Z"),
        "UTC",
    );
    assert_eq!(progress[0].contributed, amount("50000"));
    assert_eq!(progress[0].saved, amount("80000"));
    assert_eq!(progress[0].fx_gain, amount("30000"));
}

#[test]
fn need_with_return_is_an_annuity_and_linear_at_zero() {
    // 0.01 a month for 12 months from nothing to 12 682.50: 1000 a month.
    let need = need_with_return(Amount::ZERO, amount("12682.50"), amount("0.01"), 12);
    assert_eq!(need.round(2), amount("1000"));
    assert_eq!(
        need_with_return(amount("200"), amount("1400"), Amount::ZERO, 12),
        amount("100")
    );
    // Growth alone gets there: nothing to put aside.
    assert_eq!(
        need_with_return(amount("1000"), amount("1100"), amount("0.01"), 12),
        Amount::ZERO
    );
    assert_eq!(
        months_to_target(
            Amount::ZERO,
            amount("12682.50"),
            amount("0.01"),
            amount("1000")
        ),
        Some(12)
    );
    assert_eq!(
        months_to_target(Amount::ZERO, amount("100"), Amount::ZERO, Amount::ZERO),
        None
    );
}

#[test]
fn expected_return_lowers_the_need_and_dates_the_forecast() {
    let (_dir, mut vault, savings) = setup_savings();
    deposit(&mut vault, &savings, "1000", "2026-01-02T00:00:00Z");
    let mut draft = goal("House", "13000", &[&savings], 3);
    draft.monthly_commitment = Some(amount("1000"));
    let id = crate::usecases::planning::goals::create_goal(&mut vault, draft.clone())
        .unwrap()
        .id;
    let now = secs("2026-01-15T00:00:00Z");
    let flat = crate::usecases::planning::goals::goal_progress(&vault.books(), now, "UTC")[0]
        .need_per_month
        .unwrap();
    draft.expected_return = Some(amount("12"));
    crate::usecases::planning::goals::update_goal(&mut vault, &id, draft.clone()).unwrap();
    let with_return =
        &crate::usecases::planning::goals::goal_progress(&vault.books(), now, "UTC")[0];
    assert!(with_return.need_per_month.unwrap() < flat);
    // 1000 now plus 1000 a month at 1 %: twelve months to 13 000.
    assert_eq!(with_return.forecast.as_deref(), Some("2027-01-15"));
    draft.expected_return = Some(amount("101"));
    assert!(crate::usecases::planning::goals::update_goal(&mut vault, &id, draft).is_err());
}

#[test]
fn hand_entered_month_stands_in_for_a_short_history() {
    let (_dir, mut vault, savings) = setup_savings();
    let now = secs("2026-04-10T00:00:00Z");
    assert_eq!(
        crate::usecases::planning::goals::free_surplus_avg(&vault.books(), now, "UTC"),
        None
    );
    vault.doc.settings.manual_income = Some(amount("3000"));
    vault.doc.settings.manual_expenses = Some(amount("2200"));
    assert_eq!(
        crate::usecases::planning::goals::free_surplus_avg(&vault.books(), now, "UTC"),
        Some(amount("800"))
    );
    assert_eq!(
        crate::planning::budget::income_basis(&vault.books(), 3, now, "UTC")
            .unwrap()
            .months,
        0
    );
    // Three finished months of flows: the history wins.
    for ts in [
        "2026-01-05T00:00:00Z",
        "2026-02-05T00:00:00Z",
        "2026-03-05T00:00:00Z",
    ] {
        deposit(&mut vault, &savings, "500", ts);
    }
    assert_eq!(
        crate::usecases::planning::goals::free_surplus_avg(&vault.books(), now, "UTC"),
        Some(amount("500"))
    );
    assert_eq!(
        crate::planning::budget::income_basis(&vault.books(), 3, now, "UTC")
            .unwrap()
            .months,
        3
    );
}

#[test]
fn deposits_count_incoming_operations_since_the_start() {
    let (_dir, mut vault, savings) = setup_savings();
    deposit(&mut vault, &savings, "500", "2025-06-01T00:00:00Z");
    deposit(&mut vault, &savings, "100", "2026-02-01T00:00:00Z");
    deposit(&mut vault, &savings, "100", "2026-03-01T00:00:00Z");
    crate::usecases::planning::goals::create_goal(&mut vault, goal("Trip", "1000", &[&savings], 3))
        .unwrap();
    let progress = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-04-01T00:00:00Z"),
        "UTC",
    );
    assert_eq!(progress[0].deposits, 2);
}

#[test]
fn deleting_a_goal_frees_its_envelopes_and_stops_its_saver_until_undone() {
    let (_dir, mut vault, savings) = setup_savings();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    deposit(&mut vault, &savings, "500", "2026-01-01T00:00:00Z");
    let trip = crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal("Trip", "1000", &[&savings], 3),
    )
    .unwrap();
    crate::usecases::household::envelopes::fund_goal(
        &mut vault,
        &trip.id,
        &StoreId::from(&savings),
        amount("200"),
    )
    .unwrap();
    let params = |pairs: &[(&str, &str)]| -> std::collections::BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    };
    let saver = crate::usecases::workflow::records::workflow_from_template(
        &mut vault,
        "pay_yourself_first",
        &params(&[("category", "salary"), ("pct", "10"), ("target", &savings)]),
        "en",
        secs("2026-01-02T00:00:00Z"),
    )
    .unwrap()
    .workflow;
    let other = crate::usecases::workflow::records::workflow_from_template(
        &mut vault,
        "round_up",
        &params(&[
            ("from", &savings),
            ("to", &card),
            ("step", "100"),
            ("weekday", "6"),
        ]),
        "en",
        secs("2026-01-02T00:00:00Z"),
    )
    .unwrap()
    .workflow;
    let active = |vault: &Session, id: &str| {
        vault
            .doc
            .workflows
            .iter()
            .find(|each| each.id == id)
            .unwrap()
            .active
    };

    let deleted = crate::usecases::planning::goals::delete_goal(
        &mut vault,
        &trip.id,
        secs("2026-02-01T00:00:00Z"),
    )
    .unwrap();
    assert!(crate::planning::goals::goals(&vault.doc).is_empty());
    assert!(vault.doc.envelopes.is_empty());
    assert_eq!(deleted.workflow_ids, vec![saver.id.clone()]);
    assert!(!active(&vault, &saver.id));
    assert!(
        active(&vault, &other.id),
        "money out of the wallet is not the goal's"
    );

    crate::usecases::planning::goals::restore_goal(
        &mut vault,
        deleted,
        secs("2026-02-01T00:00:01Z"),
    )
    .unwrap();
    assert_eq!(crate::planning::goals::goals(&vault.doc).len(), 1);
    assert_eq!(vault.doc.envelopes.len(), 1);
    assert!(active(&vault, &saver.id));
}

#[test]
fn a_flat_or_falling_trend_has_no_forecast() {
    let (_dir, mut vault, savings) = setup_savings();
    deposit(&mut vault, &savings, "500", "2025-06-01T00:00:00Z");
    crate::usecases::planning::goals::create_goal(&mut vault, goal("Sofa", "1000", &[&savings], 2))
        .unwrap();
    let flat = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-02-10T00:00:00Z"),
        "UTC",
    );
    assert!(!flat[0].accumulating);
    assert_eq!(flat[0].forecast, None);

    // Steady deposits over the last 90 days: a forecast appears.
    for day in ["2026-01-01", "2026-01-15", "2026-02-01"] {
        deposit(&mut vault, &savings, "100", &format!("{day}T00:00:00Z"));
    }
    let growing = crate::usecases::planning::goals::goal_progress(
        &vault.books(),
        secs("2026-02-10T00:00:00Z"),
        "UTC",
    );
    assert!(growing[0].accumulating);
    assert!(growing[0].forecast.is_some());
}

#[test]
fn calendar_compares_planned_and_put_aside_by_month() {
    let (_dir, mut vault, savings) = setup_savings();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal("Laptop", "1200", &[&savings], 3),
    )
    .unwrap();
    deposit(&mut vault, &card, "5000", "2026-01-01T00:00:00Z");
    let mut put_aside = transfer_between(
        AccountRef::Store(card.clone()),
        AccountRef::Store(savings.clone()),
        usd(),
        "150",
    );
    at(&mut put_aside, "2026-01-20T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, put_aside).unwrap();
    let mut little = transfer_between(
        AccountRef::Store(card.clone()),
        AccountRef::Store(savings.clone()),
        usd(),
        "40",
    );
    at(&mut little, "2026-02-20T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, little).unwrap();

    let calendar = crate::usecases::planning::goals::goal_calendar(
        &vault.books(),
        2026,
        secs("2026-03-01T00:00:00Z"),
        "UTC",
    )
    .unwrap();
    let jan = &calendar.months[0];
    assert_eq!(
        (jan.need, jan.saved, jan.done),
        (amount("100"), amount("150"), true)
    );
    let feb = &calendar.months[1];
    assert_eq!(feb.shortfall, amount("60"));
    assert!(!feb.done);
}

// -- the pure deciders on a bare document --

fn bare_draft(name: &str) -> GoalDraft {
    GoalDraft {
        name: name.into(),
        active: true,
        target_amount: amount("1000"),
        currency: "usd".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: None,
        priority: 3,
        monthly_commitment: None,
        scope: StoreScope {
            store_ids: vec!["wallet".into()],
            include_children: false,
        },
        scenario_id: None,
        expected_return: None,
    }
}

fn bare_doc() -> crate::model::vault::VaultDoc {
    let mut doc = crate::model::vault::VaultDoc::default();
    doc.stores.push(crate::model::store::Store {
        id: "wallet".into(),
        ..crate::model::store::Store::default()
    });
    doc
}

#[test]
fn a_bare_document_decides_create_and_update_without_a_vault() {
    let mut doc = bare_doc();
    let goal = plan_create_goal(
        &doc,
        bare_draft(" Trip "),
        "g1".into(),
        "2026-02-01T00:00:00Z",
    )
    .unwrap();
    assert_eq!((goal.id.as_str(), goal.name.as_str()), ("g1", "Trip"));
    assert_eq!(goal.created_at, "2026-02-01T00:00:00Z");
    assert!(doc.goals.is_empty(), "a decision writes nothing");
    assert!(plan_create_goal(&doc, bare_draft(" "), "g2".into(), "now").is_err());
    doc.goals.push(goal);
    let edited = plan_update_goal(&doc, "g1", bare_draft("Home")).unwrap();
    assert_eq!(edited.name, "Home");
    assert!(matches!(
        plan_update_goal(&doc, "nope", bare_draft("Home")),
        Err(crate::Error::Missing(_))
    ));
}

#[test]
fn deleting_and_restoring_are_decided_on_a_bare_document() {
    let mut doc = bare_doc();
    let goal = plan_create_goal(&doc, bare_draft("Trip"), "g1".into(), "now").unwrap();
    doc.goals.push(goal);
    let deleted = plan_delete_goal(&doc, "g1", |_| vec!["wf".into()]).unwrap();
    assert_eq!(deleted.workflow_ids, vec!["wf".to_string()]);
    assert!(plan_delete_goal(&doc, "none", |_| Vec::new()).is_err());
    assert!(
        plan_restore_goal(&doc, &deleted).is_err(),
        "the goal is still there"
    );
    doc.goals.clear();
    assert_eq!(
        plan_restore_goal(&doc, &deleted).unwrap(),
        Vec::<String>::new(),
        "a workflow deleted since then is not switched on"
    );
}

// -- one test per pure function, and the family pins --

fn goal_on(
    doc: &crate::model::vault::VaultDoc,
    id: &str,
    name: &str,
) -> crate::model::planning::Goal {
    plan_create_goal(doc, bare_draft(name), id.into(), "2026-01-01T00:00:00Z").unwrap()
}

#[test]
fn savers_are_active_ordinary_workflows_paying_into_an_unshared_wallet() {
    use crate::planning::automations::AutomationDraft;
    use crate::usecases::planning::automation_rules::new_automation_workflow;
    let mut doc = bare_doc();
    let goal = goal_on(&doc, "g1", "Trip");
    let template = {
        let crate::model::transaction::Transaction::Transfer(inner) =
            transfer("other", "wallet", "10")
        else {
            unreachable!()
        };
        crate::model::planning::TransactionDraft::Transfer(inner)
    };
    let draft = AutomationDraft {
        name: "Saver".into(),
        active: true,
        preset: None,
        schedule: crate::model::planning::Schedule::Monthly { day: 1 },
        starts_at: "2026-01-01T00:00:00Z".into(),
        template,
        amount_mode: Default::default(),
        workday_shift: Default::default(),
        property_id: None,
        time: None,
        asks: false,
        for_month_offset: None,
    };
    let mut saver = new_automation_workflow(&draft, "wf", "2026-01-01T00:00:00Z").unwrap();
    // An automation is the form's, not an ordinary workflow: it is no saver.
    doc.workflows.push(saver.clone());
    assert!(crate::usecases::planning::goals::goal_savers(&doc, &goal).is_empty());
    saver.automation = None;
    saver.graph = r#"{"version":1,"nodes":[{"id":"a","kind":"action.transaction","params":{"to":"wallet"},"position":{"x":0.0,"y":0.0},"disabled":false,"label":""}],"edges":[],"comments":[]}"#.into();
    doc.workflows[0] = saver;
    assert_eq!(
        crate::usecases::planning::goals::goal_savers(&doc, &goal),
        vec!["wf".to_string()]
    );
    // Another goal on the same wallet keeps the workflow working.
    doc.goals.push(goal.clone());
    doc.goals.push(goal_on(&doc, "g2", "Home"));
    assert!(crate::usecases::planning::goals::goal_savers(&doc, &goal).is_empty());
    doc.workflows[0].active = false;
    doc.goals.pop();
    assert!(crate::usecases::planning::goals::goal_savers(&doc, &goal).is_empty());
}

#[test]
fn deposits_count_incoming_principal_legs_in_the_scope_on_a_bare_document() {
    let mut doc = bare_doc();
    let mut early = income("wallet", "10", "a");
    at(&mut early, "2026-01-01T00:00:00Z");
    let mut late = income("wallet", "10", "b");
    at(&mut late, "2026-03-01T00:00:00Z");
    let mut elsewhere = income("other", "10", "c");
    at(&mut elsewhere, "2026-03-01T00:00:00Z");
    doc.transactions.extend([early, late, elsewhere]);
    let scope = std::collections::HashSet::from(["wallet".to_string()]);
    assert_eq!(deposits(&doc, &scope, secs("2025-01-01T00:00:00Z")), 2);
    assert_eq!(deposits(&doc, &scope, secs("2026-02-01T00:00:00Z")), 1);
    assert_eq!(deposits(&doc, &std::collections::HashSet::new(), 0), 0);
}

#[test]
fn contributions_net_the_money_put_into_a_scope_inside_a_window() {
    let (_dir, mut vault, savings) = setup_savings();
    deposit(&mut vault, &savings, "500", "2026-01-10T00:00:00Z");
    deposit(&mut vault, &savings, "200", "2026-02-10T00:00:00Z");
    let mut spend = payment(&savings, "50", "coffee");
    at(&mut spend, "2026-01-20T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, spend).unwrap();
    let scope = std::collections::HashSet::from([savings.clone()]);
    let books = vault.books();
    let total = |from: &str, to: &str| {
        crate::usecases::planning::goals::contributions(&books, &scope, "usd", secs(from), secs(to))
    };
    assert_eq!(
        total("2026-01-01T00:00:00Z", "2026-02-01T00:00:00Z"),
        amount("450")
    );
    assert_eq!(
        total("2026-01-01T00:00:00Z", "2026-03-01T00:00:00Z"),
        amount("650")
    );
    assert_eq!(
        total("2026-02-11T00:00:00Z", "2026-03-01T00:00:00Z"),
        Amount::ZERO
    );
}

#[test]
fn the_planned_now_reads_the_scenario_of_the_goal() {
    use crate::calc::invest;
    let (_dir, mut vault) = empty_vault();
    vault.doc.stores.push(crate::model::store::Store {
        id: "wallet".into(),
        ..crate::model::store::Store::default()
    });
    let plan = super::scenarios_tests::input("1000", "100", 12);
    vault
        .doc
        .calc_scenarios
        .push(crate::model::planning::CalcScenario {
            id: "sc".into(),
            name: "Flat".into(),
            input: plan.clone(),
            source: None,
            prefilled_at: None,
            created_at: "now".into(),
            updated_at: "now".into(),
        });
    let mut goal = goal_on(&vault.doc, "g1", "Trip");
    let start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
    let now = secs("2026-05-20T10:00:00Z");
    let books = vault.books();
    assert_eq!(planned_now(&books, &goal, start, today, now), None);
    goal.scenario_id = Some("sc".into());
    let expected = invest::simulate(&plan).map(|_| {
        crate::planning::scenarios::planned_balance(&plan, months_between(start, today) - 1)
            .unwrap()
            .round(2)
    });
    assert_eq!(planned_now(&books, &goal, start, today, now), expected.ok());
    goal.scenario_id = Some("gone".into());
    assert_eq!(planned_now(&books, &goal, start, today, now), None);
}

#[test]
fn the_goal_delegates_equal_the_scenarios() {
    use crate::usecases::planning::goals as scenario;
    let (_a, mut by_method, wallet_a) = setup_savings();
    let (_b, mut by_scenario, wallet_b) = setup_savings();
    let draft = |wallet: &str, name: &str| goal(name, "1000", &[wallet], 3);
    let method =
        crate::usecases::planning::goals::create_goal(&mut by_method, draft(&wallet_a, "Trip"))
            .unwrap();
    let made = scenario::create_goal(&mut by_scenario, draft(&wallet_b, "Trip")).unwrap();
    let blank = |goal: &crate::model::planning::Goal, wallet: &str| {
        super::automations_tests::without_ids(goal, &[goal.id.as_str(), wallet])
    };
    assert_eq!(blank(&method, &wallet_a), blank(&made, &wallet_b));
    let method = crate::usecases::planning::goals::update_goal(
        &mut by_method,
        &method.id,
        draft(&wallet_a, "Home"),
    )
    .unwrap();
    let made = scenario::update_goal(&mut by_scenario, &made.id, draft(&wallet_b, "Home")).unwrap();
    assert_eq!(blank(&method, &wallet_a), blank(&made, &wallet_b));
    let now = secs("2026-02-01T00:00:00Z");
    let gone_by_method =
        crate::usecases::planning::goals::delete_goal(&mut by_method, &method.id, now).unwrap();
    let gone_by_case = scenario::delete_goal(&mut by_scenario, &made.id, now).unwrap();
    assert_eq!(gone_by_method.workflow_ids, gone_by_case.workflow_ids);
    assert_eq!(
        blank(&gone_by_method.goal, &wallet_a),
        blank(&gone_by_case.goal, &wallet_b)
    );
    let back_by_method =
        crate::usecases::planning::goals::restore_goal(&mut by_method, gone_by_method, now + 1)
            .unwrap();
    let back_by_case = scenario::restore_goal(&mut by_scenario, gone_by_case, now + 1).unwrap();
    assert_eq!(
        blank(&back_by_method, &wallet_a),
        blank(&back_by_case, &wallet_b)
    );
    assert_eq!(
        crate::planning::goals::goals(&by_method.doc).len(),
        crate::planning::goals::goals(&by_scenario.doc).len()
    );
}

struct NoWake;

impl crate::kernel::session::SaveHook for NoWake {
    fn dirty(&self) {}
}

pub(super) fn has_key(keys: impl IntoIterator<Item = impl AsRef<str>>, id: &str) -> bool {
    keys.into_iter().any(|key| key.as_ref().contains(id))
}

/// Write what is pending to disk, so the sync registry sees the edit.
pub(super) fn flush(vault: &mut Session) {
    vault.set_write_behind(Some(std::sync::Arc::new(NoWake)));
    if let Some(job) = vault.prepare_flush().unwrap() {
        let done = job.write().unwrap();
        vault.finish_flush(&done);
    }
}

#[test]
fn deleting_a_goal_leaves_a_tombstone_for_each_of_its_envelopes() {
    let (_dir, mut vault, savings) = setup_savings();
    deposit(&mut vault, &savings, "500", "2026-01-01T00:00:00Z");
    let trip = crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal("Trip", "1000", &[&savings], 3),
    )
    .unwrap();
    crate::usecases::household::envelopes::fund_goal(
        &mut vault,
        &trip.id,
        &StoreId::from(&savings),
        amount("200"),
    )
    .unwrap();
    let envelope = vault.doc.envelopes[0].id.clone();
    flush(&mut vault);
    assert!(has_key(vault.doc.sync.stamps.keys(), &envelope));
    crate::usecases::planning::goals::delete_goal(
        &mut vault,
        &trip.id,
        secs("2026-02-01T00:00:00Z"),
    )
    .unwrap();
    flush(&mut vault);
    assert!(has_key(vault.doc.sync.tombstones.keys(), &envelope));
    assert!(!has_key(vault.doc.sync.stamps.keys(), &envelope));
}
