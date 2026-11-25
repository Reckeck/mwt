//! Tests of the rebase area: the three-way merge of an edit onto the current entity.

use serde::{Deserialize, Serialize};

use crate::household::contacts::ContactDraft;
use crate::ipc_contract::{GoalInput, UpdateGoalInput};
use crate::ipc_contract::{UpdateCategoryInput, UpdateContactInput, UpdateStoreInput};
use crate::model::contact::ContactKind;
use crate::model::store::StoreKind;
use crate::model::store::StoreScope;
use crate::model::transaction::Transaction;
use crate::money::rebase::*;
use crate::service::test_support::*;
use crate::usecases::money::rebase::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Inner {
    provider: String,
    import: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sample {
    name: String,
    note: String,
    tags: Vec<String>,
    inner: Inner,
}

fn sample() -> Sample {
    Sample {
        name: "a".into(),
        note: "n".into(),
        tags: vec!["x".into()],
        inner: Inner {
            provider: "p".into(),
            import: "i".into(),
        },
    }
}

#[test]
fn three_way_takes_a_changed_field() {
    let base = sample();
    let edited = Sample {
        name: "b".into(),
        ..base.clone()
    };
    let merged = three_way(&base, &edited, &base).unwrap();
    assert_eq!(merged.name, "b");
}

#[test]
fn three_way_keeps_current_for_an_unchanged_field() {
    let base = sample();
    let current = Sample {
        note: "remote".into(),
        tags: vec!["y".into()],
        ..base.clone()
    };
    let edited = Sample {
        name: "b".into(),
        ..base.clone()
    };
    let merged = three_way(&base, &edited, &current).unwrap();
    assert_eq!(merged.name, "b");
    assert_eq!(merged.note, "remote");
    assert_eq!(merged.tags, vec!["y".to_string()]);
}

#[test]
fn three_way_recurses_into_nested_objects() {
    let base = sample();
    let mut current = base.clone();
    current.inner.import = "remote".into();
    let mut edited = base.clone();
    edited.inner.provider = "mine".into();
    let merged = three_way(&base, &edited, &current).unwrap();
    assert_eq!(merged.inner.provider, "mine");
    assert_eq!(merged.inner.import, "remote");
}

#[test]
fn three_way_edit_wins_when_both_sides_changed_a_field() {
    let base = sample();
    let current = Sample {
        name: "remote".into(),
        ..base.clone()
    };
    let edited = Sample {
        name: "mine".into(),
        ..base.clone()
    };
    assert_eq!(three_way(&base, &edited, &current).unwrap().name, "mine");
}

fn goal_input(wallet: &str, name: &str, priority: u8) -> GoalInput {
    GoalInput {
        name: name.into(),
        active: true,
        target_amount: amount("1000"),
        currency: "usd".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: Some("2026-12-31T00:00:00Z".into()),
        priority,
        monthly_commitment: None,
        scope: StoreScope {
            store_ids: vec![wallet.to_string()],
            include_children: true,
        },
        scenario_id: None,
        expected_return: None,
    }
}

#[test]
fn patch_update_keeps_remote_field_goal() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Savings", StoreKind::Bank, None);
    let created = crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal_input(&wallet, "Trip", 3).into(),
    )
    .unwrap();
    let base = goal_input(&wallet, "Trip", 3);
    // Sync applies a remote edit of the priority while the form is open.
    let mut remote = goal_input(&wallet, "Trip", 5);
    remote.target_amount = amount("1000");
    crate::usecases::planning::goals::update_goal(&mut vault, &created.id, remote.into()).unwrap();
    let input = crate::usecases::money::rebase::rebase_goal_input(
        &vault.doc,
        UpdateGoalInput {
            id: created.id.clone(),
            goal: goal_input(&wallet, "Trip 2026", 3),
            base: Some(base),
        },
    )
    .unwrap();
    let saved =
        crate::usecases::planning::goals::update_goal(&mut vault, &input.id, input.goal.into())
            .unwrap();
    assert_eq!(saved.name, "Trip 2026");
    assert_eq!(saved.priority, 5);
}

#[test]
fn update_without_base_overwrites_like_before() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Savings", StoreKind::Bank, None);
    let created = crate::usecases::planning::goals::create_goal(
        &mut vault,
        goal_input(&wallet, "Trip", 3).into(),
    )
    .unwrap();
    crate::usecases::planning::goals::update_goal(
        &mut vault,
        &created.id,
        goal_input(&wallet, "Trip", 5).into(),
    )
    .unwrap();
    let input = crate::usecases::money::rebase::rebase_goal_input(
        &vault.doc,
        UpdateGoalInput {
            id: created.id.clone(),
            goal: goal_input(&wallet, "Trip 2026", 3),
            base: None,
        },
    )
    .unwrap();
    let saved =
        crate::usecases::planning::goals::update_goal(&mut vault, &input.id, input.goal.into())
            .unwrap();
    assert_eq!(saved.priority, 3);
}

#[test]
fn patch_update_keeps_remote_field_contact() {
    let (_dir, mut vault) = empty_vault();
    let draft = ContactDraft {
        name: "Anna".into(),
        kind: ContactKind::Person,
        note: "old".into(),
        accounts: Vec::new(),
        group: String::new(),
    };
    let created =
        crate::usecases::household::contacts::create_contact(&mut vault, draft.clone()).unwrap();
    let remote = ContactDraft {
        note: "remote note".into(),
        ..draft.clone()
    };
    crate::usecases::household::contacts::update_contact(&mut vault, &created.id, remote).unwrap();
    let input = crate::usecases::money::rebase::rebase_contact_input(
        &vault.doc,
        UpdateContactInput {
            id: created.id.clone(),
            contact: ContactDraft {
                name: "Anna K".into(),
                ..draft.clone()
            },
            base: Some(draft),
        },
    )
    .unwrap();
    let saved =
        crate::usecases::household::contacts::update_contact(&mut vault, &input.id, input.contact)
            .unwrap();
    assert_eq!(saved.name, "Anna K");
    assert_eq!(saved.note, "remote note");
}

#[test]
fn patch_update_keeps_remote_field_store() {
    let (_dir, mut vault) = empty_vault();
    let id = store(&mut vault, "Bank", StoreKind::Bank, None);
    let base = crate::money::tree::find_store(&vault.doc, &id)
        .unwrap()
        .clone();
    vault
        .doc
        .stores
        .iter_mut()
        .find(|item| item.id == id)
        .unwrap()
        .note = "remote note".into();
    let input = crate::usecases::money::rebase::rebase_store_input(
        &vault.doc,
        UpdateStoreInput {
            id: id.clone(),
            name: "Bank 2".into(),
            color: base.color.clone(),
            note: base.note.clone(),
            base: Some(base),
            ..Default::default()
        },
    )
    .unwrap();
    let saved = crate::usecases::money::stores::update_store(
        &mut vault,
        &input.id,
        &input.name,
        &input.color,
        &input.note,
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.name, "Bank 2");
    assert_eq!(saved.note, "remote note");
}

#[test]
fn patch_update_keeps_remote_field_transaction() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Bank", StoreKind::Bank, None);
    let first = crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&wallet, "100", "salary"),
    )
    .unwrap();
    let id = first.meta().id.clone();
    let base = first.clone();
    // Remote edit of the comment.
    let mut remote = base.clone();
    remote.meta_mut().comment = "remote comment".into();
    crate::usecases::money::transactions::update_transaction(&mut vault, &id, remote).unwrap();
    // The form changed only the amount.
    let mut edited = base.clone();
    if let Transaction::Topup(tx) = &mut edited {
        tx.amount = amount("150");
    }
    let merged =
        crate::money::rebase::rebase_transaction(&vault.doc, &id, Some(&base), edited).unwrap();
    let saved =
        crate::usecases::money::transactions::update_transaction(&mut vault, &id, merged).unwrap();
    assert_eq!(saved.meta().comment, "remote comment");
    let Transaction::Topup(tx) = saved else {
        panic!("a top-up stays a top-up")
    };
    assert_eq!(tx.amount, amount("150"));
}

// -- the methods no test named before the move: without a base the edit is
// returned as sent, `base` cleared, whatever the document holds ----------------

/// Run `rebase` on `json` against an empty document: it must come back equal
/// to the input once `base` is cleared (it is `None` here already).
fn comes_back_as_sent<Input>(
    json: serde_json::Value,
    rebase: impl Fn(&crate::model::vault::VaultDoc, Input) -> crate::error::Result<Input>,
) where
    Input: serde::Serialize + serde::de::DeserializeOwned,
{
    let input: Input = serde_json::from_value(json.clone()).unwrap();
    let back = rebase(&crate::model::vault::VaultDoc::default(), input).unwrap();
    assert_eq!(
        serde_json::to_value(&back).unwrap()["base"],
        serde_json::Value::Null
    );
    let mut sent = json;
    sent["base"] = serde_json::Value::Null;
    assert_eq!(serde_json::to_value(&back).unwrap(), sent);
}

#[test]
fn a_category_edit_without_a_base_is_returned_as_sent() {
    comes_back_as_sent(
        serde_json::json!({"key": "food", "name": "Food", "group": "daily",
            "essential": null, "passive": null, "period_rule": null, "base": null}),
        rebase_category_input,
    );
}

#[test]
fn a_category_edit_keeps_the_remote_change_of_an_untouched_field() {
    use crate::model::category::Category;
    use crate::model::category::CategoryGroup;
    let base = Category {
        key: "food".into(),
        name: Some("Food".into()),
        group: CategoryGroup::Daily,
        ..Category::default()
    };
    let doc = crate::model::vault::VaultDoc {
        categories: vec![Category {
            group: CategoryGroup::Fixed,
            ..base.clone()
        }],
        ..crate::model::vault::VaultDoc::default()
    };
    let edited = UpdateCategoryInput {
        key: "food".into(),
        name: Some("Groceries".into()),
        group: CategoryGroup::Daily,
        essential: None,
        passive: None,
        period_rule: None,
        base: Some(base),
    };
    let merged = rebase_category_input(&doc, edited).unwrap();
    assert_eq!(merged.name.as_deref(), Some("Groceries"));
    assert_eq!(merged.group, CategoryGroup::Fixed, "the remote group stays");
    assert!(merged.base.is_none());
}

#[test]
fn a_budget_edit_without_a_base_is_returned_as_sent() {
    comes_back_as_sent(
        serde_json::json!({"id": "b", "base": null, "budget": {"active": true,
            "target": "total", "period": "month", "limit": "100", "currency": "usd",
            "warn": null}}),
        rebase_budget_input,
    );
}

#[test]
fn a_reminder_edit_without_a_base_is_returned_as_sent() {
    comes_back_as_sent(
        serde_json::json!({"id": "r", "base": null, "reminder": {"text": "pay",
            "due_at": "2026-02-01T09:00:00Z", "repeat": null, "pay": null}}),
        rebase_reminder_input,
    );
}

#[test]
fn a_price_alert_edit_without_a_base_is_returned_as_sent() {
    comes_back_as_sent(
        serde_json::json!({"id": "a", "base": null, "alert": {"asset": {"symbol": "BTC",
            "name": "Bitcoin", "coingecko_id": null, "chain": null, "contract": null},
            "condition": "above", "price": "100", "currency": "usd", "active": true,
            "store_id": null, "repeats": false}}),
        rebase_price_alert_input,
    );
}

#[test]
fn a_utility_edit_without_a_base_is_returned_as_sent() {
    comes_back_as_sent(
        serde_json::json!({"id": "u", "base": null, "item": {"name": "Rent", "active": true,
            "kind": {"static": {"amount": "10"}}, "currency": "usd", "property_id": "p",
            "store_id": null, "category": "housing"}}),
        rebase_utility_input,
    );
}

#[test]
fn an_envelope_edit_without_a_base_is_returned_as_sent() {
    comes_back_as_sent(
        serde_json::json!({"id": "e", "base": null, "draft": {"store_id": "s", "name": "Trip",
            "asset": {"symbol": "USD", "name": "USD", "quote": "USD", "coingecko_id": null,
                "chain": null, "contract": null},
            "target": null, "goal_id": null}}),
        rebase_envelope_input,
    );
}

#[test]
fn an_automation_edit_without_a_base_is_returned_as_sent() {
    let (_dir, vault) = crate::service::test_support::empty_vault();
    let json = serde_json::json!({"id": "x", "base": null, "automation": {"name": "Rent",
        "active": true, "preset": null, "schedule": {"monthly": {"day": 1}},
        "starts_at": "2026-02-01T09:00:00Z", "time": null, "asks": false,
        "template": {"type": "payment",
            "common": {"timestamp": "2026-02-01T09:00:00Z", "comment": "", "categories": [],
                "flow_id": null, "contact_id": null, "price_usd": null, "fx_override": null,
                "price": null, "price_currency": null},
            "store_id": "s", "asset": {"symbol": "USD", "name": "", "coingecko_id": null,
                "chain": null, "contract": null},
            "amount": "1", "fee": null, "to": null,
            "details": {"cash_breakdown": null, "unit_price": null, "network": null,
                "tx_hash": null}}}});
    let input: crate::ipc_contract::UpdateAutomationInput = serde_json::from_value(json).unwrap();
    let back = rebase_automation_input(&vault, input.clone()).unwrap();
    assert_eq!(back.automation, input.automation);
    assert!(back.base.is_none());
}
