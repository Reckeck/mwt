//! The run lock: two real xtask processes on the hidden `selftest-sleep` command.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn working_copy() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "xtask-lock-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    dir
}

fn spawn(dir: &Path, args: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("selftest-sleep")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn wait_started(dir: &Path) {
    let events = dir.join(".cache/xtask/run.events");
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if std::fs::read_to_string(&events).is_ok_and(|text| text.contains("start")) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the first run did not start");
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn same_command_attaches_and_gets_the_same_exit_code() {
    let dir = working_copy();
    let first = spawn(&dir, &["2", "--code", "3"]);
    wait_started(&dir);
    let second = spawn(&dir, &["2", "--code", "3"])
        .wait_with_output()
        .unwrap();
    let first = first.wait_with_output().unwrap();
    assert_eq!(first.status.code(), Some(3));
    assert_eq!(second.status.code(), Some(3));
    let out = text(&second);
    assert!(out.contains("attaching to the run of pid"), "{out}");
    assert!(out.contains("start  sleep"), "{out}");
}

#[test]
fn another_command_waits_then_runs() {
    let dir = working_copy();
    let first = spawn(&dir, &["2"]);
    wait_started(&dir);
    let began = Instant::now();
    let second = spawn(&dir, &["1", "--code", "4"])
        .wait_with_output()
        .unwrap();
    assert!(
        began.elapsed() >= Duration::from_millis(1200),
        "did not wait: {:?}",
        began.elapsed()
    );
    assert_eq!(second.status.code(), Some(4));
    assert!(
        text(&second).contains("waiting for `xtask selftest-sleep 2`"),
        "{}",
        text(&second)
    );
    assert_eq!(first.wait_with_output().unwrap().status.code(), Some(0));
}

#[test]
fn killed_run_is_noticed() {
    let dir = working_copy();
    let mut first = spawn(&dir, &["30"]);
    wait_started(&dir);
    let second = spawn(&dir, &["30"]);
    std::thread::sleep(Duration::from_millis(800));
    first.kill().unwrap();
    first.wait().unwrap();
    let second = second.wait_with_output().unwrap();
    assert_eq!(second.status.code(), Some(1));
    assert!(
        text(&second).contains("ended without a result"),
        "{}",
        text(&second)
    );
}

#[test]
fn different_working_copies_do_not_know_each_other() {
    let (left, right) = (working_copy(), working_copy());
    let began = Instant::now();
    let first = spawn(&left, &["1"]);
    let second = spawn(&right, &["1"]);
    let (first, second) = (
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    );
    assert!(
        began.elapsed() < Duration::from_millis(1900),
        "ran one after another: {:?}",
        began.elapsed()
    );
    for output in [first, second] {
        assert_eq!(output.status.code(), Some(0));
        assert!(!text(&output).contains("attaching") && !text(&output).contains("waiting"));
    }
}

#[test]
fn a_subdirectory_is_the_same_working_copy() {
    let dir = working_copy();
    let sub = dir.join("deep/er");
    std::fs::create_dir_all(&sub).unwrap();
    let first = spawn(&dir, &["2"]);
    wait_started(&dir);
    let second = spawn(&sub, &["2"]).wait_with_output().unwrap();
    assert!(text(&second).contains("attaching"), "{}", text(&second));
    first.wait_with_output().unwrap();
}
