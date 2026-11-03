use super::*;

#[test]
fn version_splits_at_the_first_dash() {
    assert_eq!(
        split_version("0.1.0"),
        Split {
            base: "0.1.0",
            pre: None
        }
    );
    assert_eq!(
        split_version("0.1.0-12"),
        Split {
            base: "0.1.0",
            pre: Some("12")
        }
    );
    assert_eq!(split_version("1.2.3-rc-4").pre, Some("rc-4"));
}

#[test]
fn rpm_of_a_pre_release_sorts_before_the_release() {
    let split = split_version("0.1.0-12");
    let json: serde_json::Value = serde_json::from_str(&rpm_config(&split, "12")).unwrap();
    assert_eq!(json["version"], "0.1.0");
    assert_eq!(json["bundle"]["linux"]["rpm"]["release"], "0.12");
}

#[test]
fn release_names_are_stable() {
    let name = names("0.1.0-12");
    assert_eq!(name.deb, "multi-wallet-tracker_0.1.0-12_amd64.deb");
    assert_eq!(name.rpm, "multi-wallet-tracker-0.1.0-12-1.x86_64.rpm");
    assert_eq!(
        name.appimage,
        "multi-wallet-tracker-0.1.0-12-x86_64.AppImage"
    );
    assert_eq!(
        name.sig,
        "multi-wallet-tracker-0.1.0-12-x86_64.AppImage.sig"
    );
    assert_eq!(name.root_dir, "multi-wallet-tracker-0.1.0-12-linux-x86_64");
    assert_eq!(
        name.tarball,
        "multi-wallet-tracker-0.1.0-12-linux-x86_64.tar.gz"
    );
}

#[test]
fn exactly_one_file_is_required() {
    let only = exactly_one(vec![PathBuf::from("a/b.deb")], "a/*.deb").unwrap();
    assert_eq!(only, PathBuf::from("a/b.deb"));
    let none = exactly_one(Vec::new(), "a/*.deb").unwrap_err().to_string();
    assert_eq!(none, "expected one file for a/*.deb, got: ");
    let two = exactly_one(vec!["a/1.deb".into(), "a/2.deb".into()], "a/*.deb")
        .unwrap_err()
        .to_string();
    assert!(two.contains("a/1.deb a/2.deb"), "{two}");
}

#[test]
fn suffix_match_keeps_the_signature_apart() {
    let dir = std::env::temp_dir().join(format!("xtask-suffix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["My App.AppImage", "My App.AppImage.sig", ".hidden.AppImage"] {
        std::fs::write(dir.join(file), "").unwrap();
    }
    assert_eq!(
        with_suffix(&dir, ".AppImage"),
        [dir.join("My App.AppImage")]
    );
    assert_eq!(
        with_suffix(&dir, ".AppImage.sig"),
        [dir.join("My App.AppImage.sig")]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
