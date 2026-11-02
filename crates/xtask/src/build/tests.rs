use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::macos::MacFacts;
use super::*;
use crate::graph::{Opts, Status, run_graph};

fn env() -> Arc<Env> {
    Arc::new(
        Env::detect(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
            "dev",
            "0.1.0",
            false,
            1,
        )
        .unwrap(),
    )
}

fn facts(tarball: bool, host: Option<&str>) -> Facts {
    Facts {
        mac: MacFacts {
            is_mac: false,
            host: host.map(str::to_string),
            dir: "mwt-build".into(),
        },
        tarball,
        prebuilt_frontend: false,
    }
}

fn names(tasks: &[Task]) -> Vec<&str> {
    tasks.iter().map(|task| task.name.as_str()).collect()
}

fn deps<'a>(tasks: &'a [Task], name: &str) -> Vec<&'a str> {
    tasks
        .iter()
        .find(|task| task.name == name)
        .unwrap_or_else(|| panic!("no task {name}"))
        .deps
        .iter()
        .map(String::as_str)
        .collect()
}

fn chosen(list: &[&str]) -> Vec<String> {
    list.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn all_platforms_share_one_frontend() {
    let tasks = graph(&env(), &[], &facts(false, None)).unwrap();
    assert_eq!(
        names(&tasks),
        [
            "build:frontend",
            "build:android",
            "build:windows",
            "build:linux",
            "build:macos",
            "build:pacman",
            "build:flatpak"
        ]
    );
    for name in ["build:android", "build:windows", "build:linux"] {
        assert_eq!(deps(&tasks, name), ["build:frontend"], "{name}");
    }
    // Over ssh (or skipped) macOS does not wait for the host frontend: the Mac builds its own.
    assert!(deps(&tasks, "build:macos").is_empty());
    let heavy: Vec<&str> = tasks
        .iter()
        .filter(|task| task.class == Class::Heavy)
        .map(|task| task.name.as_str())
        .collect();
    assert_eq!(heavy, ["build:android", "build:windows", "build:linux"]);
}

#[test]
fn pacman_and_flatpak_wait_for_linux() {
    let tasks = graph(&env(), &[], &facts(false, None)).unwrap();
    assert_eq!(deps(&tasks, "build:pacman"), ["build:linux"]);
    assert_eq!(deps(&tasks, "build:flatpak"), ["build:linux"]);
}

#[test]
fn platform_filter_keeps_only_what_is_needed() {
    let tasks = graph(&env(), &chosen(&["linux", "pacman"]), &facts(false, None)).unwrap();
    assert_eq!(
        names(&tasks),
        ["build:frontend", "build:linux", "build:pacman"]
    );
    let tasks = graph(&env(), &chosen(&["pacman", "android"]), &facts(true, None)).unwrap();
    assert_eq!(
        names(&tasks),
        ["build:frontend", "build:android", "build:pacman"]
    );
    assert!(deps(&tasks, "build:pacman").is_empty());
}

#[test]
fn no_frontend_when_nothing_needs_it() {
    let tasks = graph(&env(), &chosen(&["flatpak"]), &facts(true, None)).unwrap();
    assert_eq!(names(&tasks), ["build:flatpak"]);
    let tasks = graph(&env(), &chosen(&["macos"]), &facts(false, None)).unwrap();
    assert_eq!(names(&tasks), ["build:macos"]);
}

#[test]
fn pacman_without_linux_or_tarball_is_an_error() {
    let error = graph(&env(), &chosen(&["pacman"]), &facts(false, None))
        .err()
        .expect("an error");
    assert!(error.to_string().contains("build it first"), "{error}");
}

#[test]
fn unknown_platform_is_an_error() {
    let error = graph(&env(), &chosen(&["ios"]), &facts(false, None))
        .err()
        .expect("an error");
    assert!(
        error.to_string().contains("unknown platform `ios`"),
        "{error}"
    );
}

#[test]
fn version_goes_into_shell_quotes_safely() {
    assert!(check_version("0.1.0-12").is_ok());
    assert!(check_version("1.2.3+build").is_ok());
    assert!(check_version("0.1.0'; rm -rf /").is_err());
    assert!(check_version("").is_err());
}

#[tokio::test]
async fn macos_without_a_mac_or_host_is_skipped() {
    let tasks = graph(&env(), &chosen(&["macos"]), &facts(false, None)).unwrap();
    let dir = std::env::temp_dir().join(format!("xtask-build-skip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
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
    assert_eq!(reports[0].status, Status::Skip);
    assert!(
        reports[0].note.contains("MACOS_BUILD_HOST"),
        "{}",
        reports[0].note
    );
}

#[test]
fn prebuilt_frontend_drops_the_frontend_task() {
    let mut prebuilt = facts(false, None);
    prebuilt.prebuilt_frontend = true;
    let tasks = graph(&env(), &chosen(&["linux", "android"]), &prebuilt).unwrap();
    assert_eq!(names(&tasks), ["build:android", "build:linux"]);
    assert!(deps(&tasks, "build:android").is_empty());
    assert!(deps(&tasks, "build:linux").is_empty());
    // pacman still waits for the Linux build it starts from.
    let tasks = graph(&env(), &chosen(&["linux", "pacman"]), &prebuilt).unwrap();
    assert_eq!(deps(&tasks, "build:pacman"), ["build:linux"]);
}

#[test]
fn android_overlay_carries_the_version_code() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shared = Env::detect(root, "dev", "0.1.0-7", false, 7).unwrap();
    let Ok(path) = targets::overlay(&shared, "android-unit-test", true, false) else {
        panic!("overlay not written");
    };
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(path);
    assert!(text.contains("\"versionCode\":7"), "{text}");
}

#[test]
fn container_gets_the_ci_build_switches_by_name() {
    let mut shared = env();
    Arc::get_mut(&mut shared).unwrap().engine = Some("docker".into());
    let Ok(cmd) = shared.run_ci("linux", "image", "true", Vec::new()) else {
        panic!("no command");
    };
    for name in [
        "CARGO_PROFILE_RELEASE_LTO",
        "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
        "APPIMAGE_EXTRACT_AND_RUN",
        "ANDROID_KEYSTORE_BASE64",
    ] {
        // `-e NAME` copies the value from the environment of the run, nothing is printed.
        let at = cmd.args.iter().position(|arg| arg == name);
        assert!(at.is_some_and(|at| cmd.args[at - 1] == "-e"), "{name}");
    }
}

#[test]
fn dev_container_builds_without_lto_unless_the_run_chose() {
    let mut shared = env();
    Arc::get_mut(&mut shared).unwrap().engine = Some("docker".into());
    let Ok(cmd) = shared.run_ci("linux", "image", "true", Vec::new()) else {
        panic!("no command");
    };
    for (name, value) in [
        ("CARGO_PROFILE_RELEASE_LTO", "off"),
        ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "16"),
    ] {
        if std::env::var_os(name).is_none() {
            let pair = format!("{name}={value}");
            assert!(cmd.args.contains(&pair), "{pair}");
        }
    }
}
