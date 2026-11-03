use super::collect::*;
use super::*;

#[test]
fn release_names_are_stable() {
    assert_eq!(
        setup_name("0.1.0-3"),
        "multi-wallet-tracker-0.1.0-3-x64-setup.exe"
    );
    assert_eq!(
        msi_name("0.1.0", "ru-RU"),
        "multi-wallet-tracker-0.1.0-x64-ru-RU.msi"
    );
    assert_eq!(
        portable_name("0.1.0"),
        "multi-wallet-tracker-0.1.0-windows-x64-portable.zip"
    );
}

#[test]
fn msi_language_is_the_last_part_of_the_name() {
    assert_eq!(
        msi_lang("Multi Wallet Tracker_0.1.0_x64_en-US.msi").as_deref(),
        Some("en-US")
    );
    assert_eq!(msi_lang("app_x64_ru-RU.msi").as_deref(), Some("ru-RU"));
    assert_eq!(msi_lang("app_x64.msi"), None);
    assert_eq!(msi_lang("app_x64_en-us.msi"), None);
    assert_eq!(msi_lang("app_en-US.exe"), None);
}

#[test]
fn msvc_is_found_by_a_non_empty_path() {
    assert!(msvc_found(
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise\r\n"
    ));
    assert!(!msvc_found(""));
    assert!(!msvc_found("  \n"));
}

#[test]
fn sccache_is_used_only_when_present() {
    let root = Path::new("/w");
    let plain = build_env(root, None);
    assert!(plain.iter().all(|(name, _)| name != "RUSTC_WRAPPER"));
    assert!(plain.contains(&("CARGO_INCREMENTAL".into(), "0".into())));
    let cached = build_env(root, Some(Path::new("/bin/sccache")));
    for (name, value) in [
        ("RUSTC_WRAPPER", "/bin/sccache"),
        ("SCCACHE_CACHE_SIZE", "3G"),
        ("SCCACHE_IDLE_TIMEOUT", "0"),
    ] {
        assert!(cached.contains(&(name.into(), value.into())), "{name}");
    }
}

#[test]
fn binary_comes_from_the_overlay() {
    assert_eq!(
        binary_name(r#"{"mainBinaryName":"multi-wallet-tracker-dev"}"#).unwrap(),
        "multi-wallet-tracker-dev"
    );
    assert!(binary_name("{}").is_err());
}

#[test]
fn bundles_are_collected_under_release_names() {
    let dir = std::env::temp_dir().join(format!("xtask-win-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let release = dir.join("release");
    for (path, body) in [
        ("bundle/nsis/App_0.1.0_x64-setup.exe", "setup"),
        ("bundle/nsis/App_0.1.0_x64-setup.exe.sig", "sig"),
        ("bundle/msi/App_0.1.0_x64_en-US.msi", "msi"),
        ("bundle/msi/App_0.1.0_x64_ru-RU.msi", "msi"),
        ("bundle/msi/App_0.1.0_x64_ru-RU.msi.sig", "sig"),
        ("app-bin.exe", "exe"),
    ] {
        let file = release.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, body).unwrap();
    }
    let out = dir.join("out");
    run(&release, &out, "0.1.0", "app-bin").unwrap();
    for name in [
        "multi-wallet-tracker-0.1.0-x64-setup.exe",
        "multi-wallet-tracker-0.1.0-x64-setup.exe.sig",
        "multi-wallet-tracker-0.1.0-x64-en-US.msi",
        "multi-wallet-tracker-0.1.0-x64-ru-RU.msi",
        "multi-wallet-tracker-0.1.0-x64-ru-RU.msi.sig",
        "multi-wallet-tracker-0.1.0-windows-x64-portable.zip",
    ] {
        assert!(out.join(name).is_file(), "{name}");
    }
    assert!(
        !out.join("multi-wallet-tracker-0.1.0-x64-en-US.msi.sig")
            .exists()
    );
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(out.join(portable_name("0.1.0"))).unwrap())
            .unwrap();
    assert_eq!(archive.len(), 1);
    assert_eq!(archive.by_index(0).unwrap().name().unwrap(), "app-bin.exe");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_installer_is_a_loud_error() {
    let dir = std::env::temp_dir().join(format!("xtask-win-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = run(&dir, &dir.join("out"), "0.1.0", "x").unwrap_err();
    assert_eq!(error.to_string(), "NSIS installer not found");
    let _ = std::fs::remove_dir_all(&dir);
}
