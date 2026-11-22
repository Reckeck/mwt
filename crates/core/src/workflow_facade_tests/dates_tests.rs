//! The "Add to a date" node run on a vault: the bounds of its amount.

use crate::workflow::RunOutcome;
use serde_json::json;

fn outcome_of(params: serde_json::Value) -> RunOutcome {
    let dir = tempfile::tempdir().unwrap();
    let vault =
        crate::service::UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    let graph = serde_json::json!({
        "version": 1,
        "nodes": [
            { "id": "t", "kind": "trigger.manual", "params": {} },
            { "id": "n", "kind": "date.add", "params": params },
            { "id": "e", "kind": "action.emit", "params": { "name": "shifted" } },
        ],
        "edges": [
            {
                "id": "t.out>e.in", "from": "t", "from_port": "out",
                "to": "e", "to_port": "in",
            },
            {
                "id": "n.date>e.payload", "from": "n", "from_port": "date",
                "to": "e", "to_port": "payload",
            },
        ],
    })
    .to_string();
    let now = crate::calendar::now_secs() + 5;
    crate::usecases::workflow::runs::dry_run_workflow(
        &vault,
        &graph,
        None,
        None,
        std::collections::BTreeMap::new(),
        now,
        "UTC",
    )
    .unwrap()
    .run
    .outcome
}

#[test]
fn date_add_past_the_calendar_is_an_error_not_a_panic() {
    let cases = [
        ("days", json!(100_000_000)),
        ("minutes", json!(1_000_000_000_000_000_000_i64)),
        ("weeks", json!(1_000_000_000_000_000_000_i64)),
        ("years", json!(1_000_000_000_000_000_000_i64)),
        ("months", json!(1_000_000_000_000_000_000_i64)),
    ];
    for (unit, amount) in cases {
        let outcome = outcome_of(json!({ "unit": unit, "amount": amount }));
        assert!(
            matches!(&outcome, RunOutcome::Failed { message } if message.contains("the date is out of range")),
            "{unit}: {outcome:?}"
        );
    }
    assert_eq!(
        outcome_of(json!({ "unit": "days", "amount": 3 })),
        RunOutcome::Done
    );
}

#[test]
fn date_add_of_a_number_beyond_the_bound_is_an_error_not_zero() {
    let outcome = outcome_of(json!({ "unit": "days", "amount": 10_000_000_000_000_000_000_u64 }));
    assert!(
        matches!(&outcome, RunOutcome::Failed { message } if message.contains("out of range")),
        "{outcome:?}"
    );
}
