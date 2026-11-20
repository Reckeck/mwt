//! The tests a user keeps with a workflow: what a test may expect, how a
//! saved test is checked and kept, and how an emulated run is held against
//! what the test expected.

use crate::error::{Error, Result};
use crate::pricing::history::unix_secs;
use crate::workflow::exec::RunReport;
use crate::workflow::model::{RunOutcome, TestExpect, Workflow, WorkflowTest};

/// The outcomes a test may expect.
const OUTCOMES: [&str; 5] = ["done", "stopped", "failed", "waiting", "partial"];

/// How a test went.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowTestResult {
    pub test_id: String,
    pub name: String,
    pub passed: bool,
    /// What differed, in both languages.
    pub problems_ru: Vec<String>,
    pub problems_en: Vec<String>,
    pub report: RunReport,
}

fn outcome_word(outcome: &RunOutcome) -> &'static str {
    use RunOutcome::*;
    match outcome {
        Done => "done",
        Stopped { .. } | Dropped { .. } => "stopped",
        Partial { .. } => "partial",
        Failed { .. } | Rejected { .. } => "failed",
        Waiting { .. } | Asking { .. } => "waiting",
    }
}

/// What a report did that a test did not expect.
fn test_problems(expect: &TestExpect, report: &RunReport) -> (Vec<String>, Vec<String>) {
    let mut ru = Vec::new();
    let mut en = Vec::new();
    let got = outcome_word(&report.run.outcome);
    if got != expect.outcome {
        ru.push(format!("итог «{got}» вместо «{}»", expect.outcome));
        en.push(format!("outcome «{got}» instead of «{}»", expect.outcome));
    }
    let queued = report
        .queued
        .iter()
        .map(|batch| batch.transactions.len() as u32)
        .sum::<u32>();
    let counts = [
        (
            expect.transactions,
            report.transactions.len() as u32,
            "операций",
            "operations",
        ),
        (expect.queued, queued, "в очереди", "queued"),
        (
            expect.notices,
            report.notices.len() as u32,
            "уведомлений",
            "notices",
        ),
    ];
    for (wanted, actual, word_ru, word_en) in counts {
        if let Some(wanted) = wanted
            && wanted != actual
        {
            ru.push(format!("{word_ru}: {actual} вместо {wanted}"));
            en.push(format!("{word_en}: {actual} instead of {wanted}"));
        }
    }
    (ru, en)
}

/// The result of one test, given the report of its emulated run.
pub fn result_of(test: &WorkflowTest, report: RunReport) -> WorkflowTestResult {
    let (problems_ru, problems_en) = test_problems(&test.expect, &report);
    WorkflowTestResult {
        test_id: test.id.clone(),
        name: test.name.clone(),
        passed: problems_en.is_empty(),
        problems_ru,
        problems_en,
        report,
    }
}

/// A test ready to keep: named, expecting a known outcome, starting at a real
/// moment (or none), with an id and a trimmed name.
pub fn checked_test(
    mut test: WorkflowTest,
    new_id: impl FnOnce() -> String,
) -> Result<WorkflowTest> {
    if test.name.trim().is_empty() {
        return Err(Error::InvalidInput("name the test".into()));
    }
    if !OUTCOMES.contains(&test.expect.outcome.as_str()) {
        return Err(Error::InvalidInput(format!(
            "unknown outcome {}",
            test.expect.outcome
        )));
    }
    if let Some(at) = &test.at
        && !at.trim().is_empty()
        && unix_secs(at).is_none()
    {
        return Err(Error::InvalidInput(format!("not a moment: {at}")));
    }
    if test.id.trim().is_empty() {
        test.id = new_id();
    }
    test.name = test.name.trim().to_string();
    Ok(test)
}

/// Replace the test with the same id, else add it.
pub fn keep_test(workflow: &mut Workflow, test: WorkflowTest) {
    match workflow.tests.iter_mut().find(|known| known.id == test.id) {
        Some(known) => *known = test,
        None => workflow.tests.push(test),
    }
}

pub fn drop_test(workflow: &mut Workflow, test_id: &str) {
    workflow.tests.retain(|test| test.id != test_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(name: &str, outcome: &str) -> WorkflowTest {
        WorkflowTest {
            id: String::new(),
            name: name.into(),
            trigger_node: None,
            at: None,
            answers: None,
            expect: TestExpect {
                outcome: outcome.into(),
                transactions: None,
                queued: None,
                notices: None,
            },
        }
    }

    fn workflow() -> Workflow {
        Workflow {
            id: "wf".into(),
            name: "w".into(),
            description: String::new(),
            active: false,
            graph: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            react_to_workflows: false,
            max_amount: None,
            max_amount_currency: None,
            versions: Vec::new(),
            tests: Vec::new(),
            draft: None,
            automation: None,
        }
    }

    #[test]
    fn a_test_needs_a_name_a_known_outcome_and_a_real_moment() {
        let id = || "new".to_string();
        assert!(matches!(
            checked_test(test(" ", "done"), id),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            checked_test(test("t", "exploded"), id),
            Err(Error::InvalidInput(_))
        ));
        let mut late = test("t", "done");
        late.at = Some("tomorrow".into());
        assert!(matches!(
            checked_test(late, id),
            Err(Error::InvalidInput(_))
        ));
        let kept = checked_test(test(" t ", "done"), id).unwrap();
        assert_eq!((kept.id.as_str(), kept.name.as_str()), ("new", "t"));
    }

    #[test]
    fn a_kept_test_replaces_the_one_with_its_id() {
        let mut workflow = workflow();
        let mut first = test("first", "done");
        first.id = "a".into();
        keep_test(&mut workflow, first.clone());
        first.name = "renamed".into();
        keep_test(&mut workflow, first);
        assert_eq!(workflow.tests.len(), 1);
        assert_eq!(workflow.tests[0].name, "renamed");
        drop_test(&mut workflow, "a");
        assert!(workflow.tests.is_empty());
    }
}
