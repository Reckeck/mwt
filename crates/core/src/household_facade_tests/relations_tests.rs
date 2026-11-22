//! Tests of the relations area: the old inline tests (names and bodies unchanged),
//! then the pure card on a bare document and the delegate pins.

use crate::household::relations::*;
use crate::model::planning::{CategorizationRule, Goal};
use crate::model::store::StoreScope;
use crate::model::transaction::TxOrigin;
use crate::service::test_support::*;
use crate::workflow::model::Workflow;

fn goal_on(store_id: &str) -> Goal {
    Goal {
        id: "g1".into(),
        name: "Отпуск".into(),
        active: true,
        target_amount: amount("1000"),
        currency: "USD".into(),
        starts_at: "2026-01-01".into(),
        deadline: None,
        priority: 3,
        monthly_commitment: None,
        scope: StoreScope {
            store_ids: vec![store_id.into()],
            include_children: true,
        },
        created_at: "2026-01-01T00:00:00Z".into(),
        scenario_id: None,
        expected_return: None,
    }
}

fn kinds(groups: &[RelationGroup]) -> Vec<RelationKind> {
    groups.iter().map(|group| group.kind).collect()
}

#[test]
fn a_wallet_lists_its_goals_and_operations() {
    let (_dir, mut vault) = setup();
    let binance = vault.doc.stores[0].id.clone();
    vault
        .doc
        .transactions
        .push(income(&binance, "100", "зарплата"));
    vault.doc.goals.push(goal_on(&binance));
    let card = vault.entity_card(&EntityRef::new(EntityKind::Store, &binance));
    assert!(!card.missing);
    let goals = card
        .groups
        .iter()
        .find(|group| group.kind == RelationKind::Goals)
        .expect("goal group");
    assert_eq!(
        goals.items[0].target,
        EntityRef::new(EntityKind::Goal, "g1")
    );
    assert!(kinds(&card.groups).contains(&RelationKind::Transactions));
    // No budgets, alerts or children: their groups are not sent.
    assert!(!kinds(&card.groups).contains(&RelationKind::Budgets));
    assert!(!kinds(&card.groups).contains(&RelationKind::Children));
}

#[test]
fn a_goal_over_every_wallet_is_not_listed_on_each() {
    let (_dir, mut vault) = setup();
    let binance = vault.doc.stores[0].id.clone();
    let mut goal = goal_on(&binance);
    goal.scope.store_ids.clear();
    vault.doc.goals.push(goal);
    let groups = vault.relations(&EntityRef::new(EntityKind::Store, &binance));
    assert!(!kinds(&groups).contains(&RelationKind::Goals));
}

#[test]
fn a_category_lists_its_rules_and_operations() {
    let (_dir, mut vault) = setup();
    vault.doc.categorization_rules.push(CategorizationRule {
        id: "r1".into(),
        pattern: "пятёрочка".into(),
        category: "groceries".into(),
        active: true,
        order: 0,
    });
    let binance = vault.doc.stores[0].id.clone();
    let mut tx = payment(&binance, "10", "хлеб");
    tx.meta_mut().id = "t-food".into();
    vault.doc.transactions.push(tx);
    let groups = vault.relations(&EntityRef::new(EntityKind::Category, "groceries"));
    assert_eq!(
        kinds(&groups),
        vec![RelationKind::Rules, RelationKind::Transactions]
    );
    assert_eq!(groups[0].items[0].label, "пятёрочка");
    assert_eq!(groups[1].items[0].target.id, "t-food");
}

#[test]
fn an_operation_leads_to_the_workflow_that_made_it_and_its_flow() {
    let (_dir, mut vault) = setup();
    vault.doc.workflows.push(Workflow {
        id: "wf".into(),
        name: "Зарплата".into(),
        description: String::new(),
        active: true,
        graph: "{}".into(),
        created_at: String::new(),
        updated_at: String::new(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
        versions: Vec::new(),
        tests: Vec::new(),
        draft: None,
        automation: None,
    });
    let binance = vault.doc.stores[0].id.clone();
    let mut first = income(&binance, "100", "зарплата");
    first.meta_mut().id = "t1".into();
    first.meta_mut().flow_id = Some("flow".into());
    first.meta_mut().origin = TxOrigin::Workflow {
        workflow_id: "wf".into(),
        run_id: "run".into(),
        planned_at: None,
        planned_date: None,
        estimated: false,
        run_key: None,
    };
    let mut second = first.clone();
    second.meta_mut().id = "t2".into();
    vault.doc.transactions.extend([first, second]);

    let groups = vault.relations(&EntityRef::new(EntityKind::Tx, "t1"));
    let origin = &groups[0];
    assert_eq!(origin.kind, RelationKind::Origin);
    assert_eq!(
        origin.items[0].target,
        EntityRef::new(EntityKind::Workflow, "wf")
    );
    let flow = groups
        .iter()
        .find(|group| group.kind == RelationKind::Flow)
        .unwrap();
    assert_eq!(flow.total, 1);
    assert_eq!(flow.items[0].target.id, "t2");

    let made = vault.relations(&EntityRef::new(EntityKind::Workflow, "wf"));
    let created = made
        .iter()
        .find(|group| group.kind == RelationKind::Created)
        .unwrap();
    assert_eq!(created.total, 2);
}

#[test]
fn a_link_to_nothing_is_a_missing_card() {
    let (_dir, vault) = setup();
    let card = vault.entity_card(&EntityRef::new(EntityKind::Goal, "gone"));
    assert!(card.missing);
    assert!(card.groups.is_empty());
}

#[test]
fn groups_are_capped_but_count_everything() {
    let (_dir, mut vault) = setup();
    let binance = vault.doc.stores[0].id.clone();
    let sample = income(&binance, "1", "");
    for index in 0..30 {
        let mut tx = sample.clone();
        tx.meta_mut().id = format!("bulk-{index}");
        vault.doc.transactions.push(tx);
    }
    let groups = vault.relations(&EntityRef::new(EntityKind::Store, &binance));
    let txs = groups
        .iter()
        .find(|group| group.kind == RelationKind::Transactions)
        .unwrap();
    assert_eq!(txs.items.len(), RELATION_ITEMS_CAP);
    assert!(txs.total as usize > RELATION_ITEMS_CAP);
}

#[test]
fn a_wallet_card_takes_its_value_from_outside() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let wallet = crate::testkit::builders::store("Card", crate::model::store::StoreKind::Bank);
    let id = wallet.id.clone();
    doc.stores.push(wallet);
    let worth = |asked: &str| (asked == id).then(|| (amount("42"), "usd".to_string()));
    let target = EntityRef::new(EntityKind::Store, &id);
    let card = card_of(&doc, &target, &worth);
    assert!(!card.missing);
    assert_eq!(card.title, "Card");
    assert_eq!(card.value, Some(amount("42")));
    assert_eq!(card.currency.as_deref(), Some("usd"));
    // Without a worth the card still has its name.
    let bare = entity_card_head(&doc, &target, &|_| None);
    assert_eq!((bare.title.as_str(), bare.value), ("Card", None));
    let gone = card_of(&doc, &EntityRef::new(EntityKind::Store, "gone"), &worth);
    assert!(gone.missing && gone.groups.is_empty());
}

#[test]
fn the_delegates_equal_the_scenarios() {
    let (_dir, mut vault) = setup();
    let binance = vault.doc.stores[0].id.clone();
    vault
        .doc
        .transactions
        .push(income(&binance, "100", "зарплата"));
    vault.doc.goals.push(goal_on(&binance));
    let target = EntityRef::new(EntityKind::Store, &binance);
    assert_eq!(
        vault.entity_card(&target),
        crate::usecases::household::relations::entity_card(&vault, &target)
    );
    assert_eq!(vault.relations(&target), relations(&vault.doc, &target));
}
