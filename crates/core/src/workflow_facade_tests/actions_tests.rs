//! Wait nodes run on a vault: the bounds of the wait.

use crate::workflow::RunOutcome;
use serde_json::json;

fn outcome_of(kind: &str, params: serde_json::Value) -> RunOutcome {
    let dir = tempfile::tempdir().unwrap();
    let vault =
        crate::service::UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    let graph = serde_json::json!({
        "version": 1,
        "nodes": [
            { "id": "t", "kind": "trigger.manual", "params": {} },
            { "id": "n", "kind": kind, "params": params },
        ],
        "edges": [{
            "id": "t.out>n.in", "from": "t", "from_port": "out", "to": "n", "to_port": "in",
        }],
    })
    .to_string();
    let now = crate::calendar::now_secs() + 5;
    vault
        .dry_run_workflow(
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

fn failure(kind: &str, params: serde_json::Value) -> String {
    match outcome_of(kind, params) {
        RunOutcome::Failed { message } => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_huge_wait_is_an_error_not_a_wrapped_time() {
    for hours in [
        1_000_000_000_000_000_000_i64,
        9_000_000_000_000_000,
        876_001,
    ] {
        assert!(failure("action.wait", json!({ "hours": hours })).contains("the wait is too long"));
    }
    assert!(
        failure(
            "action.confirm",
            json!({ "timeout_hours": 1_000_000_000_000_000_000_i64 })
        )
        .contains("the wait is too long")
    );
}

#[test]
fn a_day_wait_runs_to_the_end() {
    assert!(matches!(
        outcome_of("action.wait", json!({ "hours": "24" })),
        RunOutcome::Done
    ));
}

#[test]
fn a_wait_beyond_the_number_bound_is_an_error_not_zero() {
    assert!(
        failure(
            "action.wait",
            json!({ "hours": 10_000_000_000_000_000_000_u64 })
        )
        .contains("out of range")
    );
    assert!(
        failure(
            "action.confirm",
            json!({ "timeout_hours": 10_000_000_000_000_000_000_u64 })
        )
        .contains("out of range")
    );
}
