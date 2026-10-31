use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use super::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "xtask-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn run(tasks: Vec<Task>, heavy: usize) -> (Vec<Report>, PathBuf) {
    let dir = scratch();
    let reports = run_graph(
        tasks,
        Opts { heavy, browser: 1 },
        CancellationToken::new(),
        Arc::new(Sink::new(None)),
        &dir,
        &dir.join("logs"),
    )
    .await;
    (reports, dir)
}

fn shell(name: &str, class: Class, deps: &[&str], script: &str) -> Task {
    Task::cmd(name, class, deps, Cmd::new("sh").args(["-c", script]))
}

fn status(reports: &[Report], name: &str) -> Status {
    reports
        .iter()
        .find(|report| report.name == name)
        .unwrap()
        .status
}

#[tokio::test]
async fn dependencies_run_in_order() {
    let dir = scratch();
    let trace = dir.join("trace");
    let write = |label: &str| format!("sleep 0.1; echo {label} >> {}", trace.display());
    let tasks = vec![
        shell("last", Class::Light, &["middle"], &write("last")),
        shell("middle", Class::Light, &["first"], &write("middle")),
        shell("first", Class::Light, &[], &write("first")),
    ];
    let reports = run_graph(
        tasks,
        Opts {
            heavy: 2,
            browser: 2,
        },
        CancellationToken::new(),
        Arc::new(Sink::new(None)),
        &dir,
        &dir.join("logs"),
    )
    .await;
    assert!(reports.iter().all(|report| report.status == Status::Ok));
    assert_eq!(
        std::fs::read_to_string(trace).unwrap(),
        "first\nmiddle\nlast\n"
    );
}

#[tokio::test]
async fn fail_fast_kills_running_and_cancels_the_rest() {
    let began = Instant::now();
    let tasks = vec![
        shell("slow", Class::Light, &[], "sleep 30"),
        shell("later", Class::Light, &["slow"], "true"),
        shell("bad", Class::Light, &[], "sleep 0.3; echo boom; exit 1"),
    ];
    let (reports, _) = run(tasks, 2).await;
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "took {:?}",
        began.elapsed()
    );
    assert_eq!(status(&reports, "bad"), Status::Fail);
    assert_eq!(status(&reports, "slow"), Status::Cancelled);
    assert_eq!(status(&reports, "later"), Status::Cancelled);
}

#[tokio::test]
async fn skip_is_not_a_failure() {
    let tasks = vec![
        Task::func("skipped", Class::Light, &[], |_ctx| async {
            Outcome::Skip("no tool".into())
        }),
        shell("after", Class::Light, &["skipped"], "true"),
    ];
    let (reports, _) = run(tasks, 2).await;
    assert_eq!(status(&reports, "skipped"), Status::Skip);
    assert_eq!(status(&reports, "after"), Status::Ok);
}

#[tokio::test]
async fn heavy_tasks_respect_the_limit() {
    let dir = scratch();
    let trace = dir.join("trace");
    let script = format!("echo + >> {0}; sleep 0.4; echo - >> {0}", trace.display());
    let tasks: Vec<Task> = (0..5)
        .map(|index| shell(&format!("heavy{index}"), Class::Heavy, &[], &script))
        .collect();
    let reports = run_graph(
        tasks,
        Opts {
            heavy: 2,
            browser: 2,
        },
        CancellationToken::new(),
        Arc::new(Sink::new(None)),
        &dir,
        &dir.join("logs"),
    )
    .await;
    assert!(reports.iter().all(|report| report.status == Status::Ok));
    let (mut now, mut peak) = (0_i32, 0_i32);
    for line in std::fs::read_to_string(trace).unwrap().lines() {
        now += if line == "+" { 1 } else { -1 };
        peak = peak.max(now);
    }
    assert_eq!(peak, 2);
}

#[test]
fn select_pulls_in_dependencies() {
    let tasks = vec![
        shell("a", Class::Light, &[], "true"),
        shell("b", Class::Light, &["a"], "true"),
        shell("c", Class::Light, &[], "true"),
    ];
    let picked = select(tasks, &["b".to_string()]).unwrap();
    let names: Vec<&str> = picked.iter().map(|task| task.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
}

#[tokio::test]
async fn after_waits_when_present_and_is_ignored_when_absent() {
    let dir = scratch();
    let trace = dir.join("trace");
    let write = |label: &str| format!("sleep 0.2; echo {label} >> {}", trace.display());
    let tasks = vec![
        shell("late", Class::Light, &[], &write("late")).after(&["early", "not-in-graph"]),
        shell("early", Class::Light, &[], &write("early")),
    ];
    let reports = run_graph(
        tasks,
        Opts {
            heavy: 2,
            browser: 1,
        },
        CancellationToken::new(),
        Arc::new(Sink::new(None)),
        &dir,
        &dir.join("logs"),
    )
    .await;
    assert!(reports.iter().all(|report| report.status == Status::Ok));
    assert_eq!(std::fs::read_to_string(trace).unwrap(), "early\nlate\n");
}

#[test]
fn last_line_skips_blank_lines_and_progress_returns() {
    let dir = scratch();
    let log = dir.join("task.log");
    std::fs::write(&log, "Compiling a\nDownloading 10%\rDownloading 90%\n\n").unwrap();
    assert_eq!(last_line(&log, 160), "Downloading 90%");
    assert_eq!(last_line(&log, 4), "Down");
    assert_eq!(last_line(&dir.join("missing.log"), 160), "");
}
