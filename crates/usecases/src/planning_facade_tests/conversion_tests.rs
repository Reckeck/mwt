//! op-period T24 (M5, M6, Q14, A9): the one-time conversion of the salary -> deposit automation.

use crate::model::planning::{AmountMode, AutomationPreset, Schedule, TransactionDraft};
use crate::model::store::{StoreKind, SubType};
use crate::model::transaction::Transaction;
use crate::planning::automations::AutomationDraft;
use crate::service::test_support::*;
use crate::service::{Session, StoreExtras};
use crate::storage::SnapshotFormat;
use crate::usecases::money::stores::create_store_with;
use crate::usecases::planning::automations::create_automation;
use crate::usecases::planning::conversion::SALARY_RULE_MARKER;
use mwt_model::{AccountRef, DistKind, DistTarget, OnOtherAmount, TemplateKind};
use serde_json::{Value, json};

struct Fixture {
    _dir: tempfile::TempDir,
    vault: Session,
    card: String,
    deposit: String,
}

fn deposit_in(vault: &mut Session, name: &str, asset: mwt_model::AssetRef) -> String {
    let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
    create_store_with(
        vault,
        name,
        StoreKind::Bank,
        "#000",
        "",
        Some(bank),
        StoreExtras {
            sub_type: Some(SubType::Deposit),
            binding: Some(mwt_model::Binding::Asset(asset)),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id
}

fn fixture() -> Fixture {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let deposit = deposit_in(&mut vault, "Deposit", usd());
    Fixture {
        _dir: dir,
        vault,
        card,
        deposit,
    }
}

fn draft(name: &str, preset: Option<AutomationPreset>, template: Transaction) -> AutomationDraft {
    let template = match template {
        Transaction::Topup(inner) => TransactionDraft::Topup(inner),
        Transaction::Transfer(inner) => TransactionDraft::Transfer(inner),
        _ => unreachable!(),
    };
    AutomationDraft {
        name: name.into(),
        active: true,
        preset,
        schedule: Schedule::Monthly { day: 20 },
        starts_at: "2026-11-01T00:00:00Z".into(),
        template,
        amount_mode: AmountMode::Fixed,
        workday_shift: Default::default(),
        property_id: None,
        time: None,
        asks: false,
        for_month_offset: None,
    }
}

fn salary(vault: &mut Session, card: &str) -> String {
    let template = income_of(card, usd(), "30000", "1", "salary");
    create_automation(
        vault,
        draft("Salary", Some(AutomationPreset::Salary), template),
    )
    .unwrap()
    .id
}

/// A transfer in the income's currency and category, so only the case under test differs.
fn transfer_tx(card: &str, deposit: &str, raw: &str) -> Transaction {
    let Transaction::Transfer(mut inner) = transfer_between(
        AccountRef::Store(card.into()),
        AccountRef::Store(deposit.into()),
        usd(),
        raw,
    ) else {
        unreachable!()
    };
    inner.meta.categories = vec!["salary".into()];
    Transaction::Transfer(inner)
}

fn move_to_deposit(vault: &mut Session, template: Transaction) -> String {
    create_automation(vault, draft("To deposit", None, template))
        .unwrap()
        .id
}

fn workflow<'a>(vault: &'a Session, id: &str) -> &'a mwt_model::Workflow {
    vault
        .doc
        .workflows
        .iter()
        .find(|item| item.id == id)
        .unwrap()
}

fn converted(vault: &Session) -> bool {
    vault
        .doc
        .settings
        .migrations_done
        .iter()
        .any(|done| done == SALARY_RULE_MARKER)
}

#[test]
fn one_fitting_pair_becomes_a_template_rule_and_the_old_transfer_goes_inactive() {
    let mut fix = fixture();
    let income = salary(&mut fix.vault, &fix.card);
    let old = move_to_deposit(&mut fix.vault, transfer_tx(&fix.card, &fix.deposit, "5000"));
    assert!(
        super::super::usecases::planning::conversion::convert_salary_to_rule(&mut fix.vault)
            .unwrap()
    );
    let linked = workflow(&fix.vault, &income)
        .automation
        .as_ref()
        .unwrap()
        .operation_template_id
        .clone()
        .expect("linked");
    let template = fix
        .vault
        .doc
        .operation_templates
        .iter()
        .find(|item| item.id == linked)
        .unwrap();
    assert_eq!(template.name, "Salary");
    assert_eq!(template.kind, TemplateKind::Income);
    assert!(template.period_rule.is_none());
    let rule = template.distribution.as_ref().unwrap();
    assert_eq!(rule.on_other_amount, OnOtherAmount::ShrinkRemainder);
    assert!(rule.keep_on_card_min.is_none());
    assert_eq!(rule.rows.len(), 2);
    assert_eq!(
        rule.rows[0].kind,
        DistKind::Sum {
            amount: amount("5000")
        }
    );
    assert_eq!(
        rule.rows[0].target,
        DistTarget::Store {
            id: fix.deposit.clone()
        }
    );
    assert_eq!(rule.rows[1].kind, DistKind::Remainder);
    // Inactive, not deleted.
    assert!(!workflow(&fix.vault, &old).active);
    assert!(converted(&fix.vault));
}

#[test]
fn a_second_open_changes_nothing() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    move_to_deposit(&mut fix.vault, transfer_tx(&fix.card, &fix.deposit, "5000"));
    let path = fix.vault.path.clone();
    let _ = crate::usecases::planning::conversion::convert_salary_to_rule(&mut fix.vault).unwrap();
    fix.vault.persist().unwrap();
    drop(fix.vault);
    let first = Session::open(path.clone(), "pw").unwrap();
    assert!(converted(&first));
    let templates = first.doc.operation_templates.clone();
    assert_eq!(templates.len(), 1);
    drop(first);
    let second = Session::open(path, "pw").unwrap();
    assert_eq!(second.doc.operation_templates, templates);
    assert_eq!(
        second.doc.settings.migrations_done,
        vec![SALARY_RULE_MARKER]
    );
}

fn assert_untouched(fix: &mut Fixture) {
    let before = fix.vault.doc.clone();
    assert!(
        !crate::usecases::planning::conversion::convert_salary_to_rule(&mut fix.vault).unwrap()
    );
    assert!(fix.vault.doc == before);
}

#[test]
fn no_transfer_automation_changes_nothing() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    assert_untouched(&mut fix);
}

#[test]
fn two_candidates_change_nothing() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    move_to_deposit(&mut fix.vault, transfer_tx(&fix.card, &fix.deposit, "5000"));
    move_to_deposit(&mut fix.vault, transfer_tx(&fix.card, &fix.deposit, "1000"));
    assert_untouched(&mut fix);
}

#[test]
fn a_variable_amount_does_not_fit() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    let mut spec = draft(
        "To deposit",
        None,
        transfer_tx(&fix.card, &fix.deposit, "5000"),
    );
    spec.amount_mode = AmountMode::Varies;
    create_automation(&mut fix.vault, spec).unwrap();
    assert_untouched(&mut fix);
}

#[test]
fn a_different_currency_does_not_fit() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    let euro = deposit_in(
        &mut fix.vault,
        "Euro",
        mwt_model::AssetRef::fiat("EUR", "Euro"),
    );
    let Transaction::Transfer(mut inner) = transfer_tx(&fix.card, &euro, "5000") else {
        unreachable!()
    };
    inner.asset = mwt_model::AssetRef::fiat("EUR", "Euro");
    move_to_deposit(&mut fix.vault, Transaction::Transfer(inner));
    assert_untouched(&mut fix);
}

#[test]
fn a_different_category_does_not_fit() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    let Transaction::Transfer(mut inner) = transfer_tx(&fix.card, &fix.deposit, "5000") else {
        unreachable!()
    };
    inner.meta.categories = vec!["bonus".into()];
    move_to_deposit(&mut fix.vault, Transaction::Transfer(inner));
    assert_untouched(&mut fix);
}

/// A workflow graph with one `action.distribute` node paying out of `card`.
fn distribute_workflow(vault: &mut Session, params: Value) {
    let graph = json!({
        "version": 1,
        "nodes": [
            { "id": "t", "kind": "trigger.transaction", "params": { "tx_kind": "topup", "category": "salary" } },
            { "id": "d", "kind": "action.distribute", "params": params },
        ],
        "edges": [
            { "id": "t.out>d.in", "from": "t", "from_port": "out", "to": "d", "to_port": "in" },
            { "id": "t.amount>d.amount", "from": "t", "from_port": "amount", "to": "d", "to_port": "amount" },
        ],
    })
    .to_string();
    crate::usecases::workflow::records::save_workflow(
        vault,
        None,
        crate::workflow::service::records::WorkflowDraft {
            name: "Split".into(),
            description: String::new(),
            active: true,
            graph,
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
        },
        crate::calendar::now_secs(),
    )
    .unwrap();
}

fn distribute_params(card: &str, mode: &str, targets: Value) -> Value {
    json!({
        "from": { "store": card },
        "mode": mode,
        "category": "salary",
        "targets": targets,
    })
}

#[test]
fn a_workflow_with_a_fixed_distribute_converts() {
    let mut fix = fixture();
    let income = salary(&mut fix.vault, &fix.card);
    distribute_workflow(
        &mut fix.vault,
        distribute_params(
            &fix.card,
            "fixed",
            json!([{ "account": { "store": fix.deposit }, "share": "5000" }]),
        ),
    );
    let old = fix
        .vault
        .doc
        .workflows
        .iter()
        .find(|item| item.automation.is_none())
        .unwrap()
        .id
        .clone();
    assert!(
        super::super::usecases::planning::conversion::convert_salary_to_rule(&mut fix.vault)
            .unwrap()
    );
    let linked = workflow(&fix.vault, &income)
        .automation
        .as_ref()
        .unwrap()
        .operation_template_id
        .clone()
        .expect("linked");
    let template = fix
        .vault
        .doc
        .operation_templates
        .iter()
        .find(|item| item.id == linked)
        .unwrap();
    let rule = template.distribution.as_ref().unwrap();
    assert_eq!(
        rule.rows[0].kind,
        DistKind::Sum {
            amount: amount("5000")
        }
    );
    assert_eq!(
        rule.rows[0].target,
        DistTarget::Store {
            id: fix.deposit.clone()
        }
    );
    assert!(!workflow(&fix.vault, &old).active);
    assert!(converted(&fix.vault));
}

#[test]
fn a_workflow_in_percent_mode_does_not_fit() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    distribute_workflow(
        &mut fix.vault,
        distribute_params(
            &fix.card,
            "percent",
            json!([{ "account": { "store": fix.deposit }, "share": "10" }]),
        ),
    );
    assert_untouched(&mut fix);
}

#[test]
fn a_workflow_with_several_targets_does_not_fit() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    let second = deposit_in(&mut fix.vault, "Second", usd());
    distribute_workflow(
        &mut fix.vault,
        distribute_params(
            &fix.card,
            "fixed",
            json!([
                { "account": { "store": fix.deposit }, "share": "3000" },
                { "account": { "store": second }, "share": "2000" },
            ]),
        ),
    );
    assert_untouched(&mut fix);
}

#[test]
fn a_transfer_with_several_targets_does_not_fit() {
    let mut fix = fixture();
    salary(&mut fix.vault, &fix.card);
    let second = deposit_in(&mut fix.vault, "Second", usd());
    move_to_deposit(&mut fix.vault, transfer_tx(&fix.card, &fix.deposit, "3000"));
    move_to_deposit(&mut fix.vault, transfer_tx(&fix.card, &second, "2000"));
    assert_untouched(&mut fix);
}

#[test]
fn an_old_income_and_manual_transfer_stay_unlinked_in_both_formats() {
    for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
        let mut fix = fixture();
        fix.vault.switch_format(fmt).unwrap();
        for tx in [
            income(&fix.card, "30000", "salary"),
            transfer_tx(&fix.card, &fix.deposit, "5000"),
        ] {
            crate::usecases::money::transactions::add_transaction(&mut fix.vault, tx).unwrap();
        }
        fix.vault.persist().unwrap();
        let path = fix.vault.path.clone();
        let before = fix.vault.doc.clone();
        drop(fix.vault);
        let opened = Session::open(path, "pw").unwrap();
        assert_eq!(opened.doc.transactions.len(), 2, "{fmt:?}");
        for tx in &opened.doc.transactions {
            assert_eq!(tx.meta().income_ref, None, "{fmt:?}");
            assert_eq!(tx.meta().flow_id, None, "{fmt:?}");
        }
        assert!(opened.doc.operation_templates == before.operation_templates);
        assert!(opened.doc.transactions == before.transactions, "{fmt:?}");
        assert!(!converted(&opened), "{fmt:?}");
    }
}
