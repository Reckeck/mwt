use super::ci_env::env_lines;
use super::deps_cook::{ndk_env, plan};
use super::deps_prune::stale_ids;
use super::install_chef::{parse_checksum, sha256_hex};
use super::lane_key::{
    EnvSwitches, NodeInputs, RustInputs, external_packages, node_key, rust_key, tool_version,
};

const LOCK: &str = "version = 4\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"abc\"\n\n[[package]]\nname = \"xtask\"\nversion = \"0.5.0\"\ndependencies = [\n \"serde\",\n]\n";

fn inputs(manifests: Vec<(String, String)>, lock: &str) -> RustInputs<'_> {
    RustInputs {
        toolchain: "rustc 1\nsccache 2\n".to_string(),
        env: EnvSwitches::default(),
        cargo_config: "[build]\n",
        cook_source: "recipes",
        cargo_lock: lock,
        manifests,
    }
}

fn manifest(path: &str, version: &str) -> (String, String) {
    (
        path.to_string(),
        format!("[package]\nname = \"a\"\nversion = \"{version}\"\n[dependencies]\nx = \"1\"\n"),
    )
}

#[test]
fn key_has_lane_and_sixteen_hex() {
    let key = rust_key(
        "check-linux",
        &inputs(vec![manifest("./Cargo.toml", "1")], LOCK),
    );
    let hex = key.strip_prefix("check-linux-").unwrap();
    assert_eq!(hex.len(), 16);
    assert!(hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn version_lines_do_not_change_the_key() {
    let one = rust_key("l", &inputs(vec![manifest("./Cargo.toml", "1.0.0")], LOCK));
    let two = rust_key("l", &inputs(vec![manifest("./Cargo.toml", "2.0.0")], LOCK));
    assert_eq!(one, two);
    let mut changed = manifest("./Cargo.toml", "1.0.0");
    changed.1.push_str("y = \"2\"\n");
    assert_ne!(one, rust_key("l", &inputs(vec![changed], LOCK)));
}

#[test]
fn workspace_crates_are_ignored() {
    assert!(external_packages(LOCK).contains("serde"));
    assert!(!external_packages(LOCK).contains("xtask"));
    let bumped = LOCK.replace("0.5.0", "0.6.0");
    let manifests = vec![manifest("./Cargo.toml", "1")];
    assert_eq!(
        rust_key("l", &inputs(manifests.clone(), LOCK)),
        rust_key("l", &inputs(manifests, &bumped))
    );
    let serde_bump = LOCK.replace("1.0.0", "1.0.1");
    assert_ne!(
        rust_key("l", &inputs(vec![], LOCK)),
        rust_key("l", &inputs(vec![], &serde_bump))
    );
}

#[test]
fn manifest_order_does_not_matter() {
    let first = vec![
        manifest("./a/Cargo.toml", "1"),
        manifest("./b/Cargo.toml", "1"),
    ];
    let second = vec![
        manifest("./b/Cargo.toml", "1"),
        manifest("./a/Cargo.toml", "1"),
    ];
    assert_eq!(
        rust_key("l", &inputs(first, LOCK)),
        rust_key("l", &inputs(second, LOCK))
    );
}

#[test]
fn recipes_and_env_change_the_key() {
    let base = rust_key("l", &inputs(vec![], LOCK));
    let mut other = inputs(vec![], LOCK);
    other.cook_source = "changed";
    assert_ne!(base, rust_key("l", &other));
    let mut channel = inputs(vec![], LOCK);
    channel.env.channel = "dev".into();
    assert_ne!(base, rust_key("l", &channel));
}

fn node<'a>(lock: &'a str, rc: Option<&'a str>, node_version: &'a str) -> NodeInputs<'a> {
    NodeInputs {
        pnpm_lock: lock,
        package_json: "{}",
        pnpm_workspace: None,
        npmrc: rc,
        image: "img",
        node_version,
        pnpm_version: "10",
    }
}

#[test]
fn node_lane_follows_its_inputs() {
    let base = node_key(&node("lock", None, "26"));
    assert!(base.starts_with("node-") && base.len() == "node-".len() + 16);
    assert_eq!(base, node_key(&node("lock", None, "26")));
    assert_ne!(base, node_key(&node("lock2", None, "26")));
    assert_ne!(base, node_key(&node("lock", Some("x=1"), "26")));
    assert_ne!(base, node_key(&node("lock", None, "27")));
}

#[test]
fn tool_versions_come_from_mise() {
    let mise = "[tools]\nnode = \"26.8.1\"\npnpm = { version = \"10\" }\n";
    assert_eq!(tool_version(mise, "node").unwrap(), "26.8.1");
    assert_eq!(tool_version(mise, "pnpm").unwrap(), "10");
    assert!(tool_version(mise, "rust").is_err());
}

#[test]
fn stale_ids_keep_only_the_saved_key() {
    let json = r#"{"total_count":3,"actions_caches":[
        {"id":1,"key":"check-linux-aaa"},{"id":2,"key":"check-linux-bbb"},{"id":3,"key":"check-linux-ccc"}]}"#;
    assert_eq!(stale_ids(json, "check-linux-bbb").unwrap(), [1, 3]);
    assert!(
        stale_ids(r#"{"actions_caches":[]}"#, "k")
            .unwrap()
            .is_empty()
    );
    assert!(stale_ids("{}", "k").unwrap().is_empty());
    assert!(stale_ids("nope", "k").is_err());
}

#[test]
fn ci_env_lines() {
    assert_eq!(
        env_lines("/ws"),
        [
            "SCCACHE_DIR=/ws/.cache/sccache",
            "RUSTC_WRAPPER=sccache",
            "SCCACHE_BASEDIRS=/ws",
            "RUST_LOG=info"
        ]
    );
}

#[test]
fn checksum_line_parsing() {
    let hash = "ab".repeat(32);
    assert_eq!(
        parse_checksum(&format!("{hash}  file.tar.xz\n")),
        Some(hash.clone())
    );
    assert_eq!(parse_checksum(&hash.to_uppercase()), Some(hash));
    assert_eq!(parse_checksum("short file"), None);
    assert_eq!(parse_checksum(""), None);
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn cook_plans() {
    let linux = plan("check-linux", "/t", None).unwrap();
    assert_eq!(linux.len(), 2);
    assert_eq!(linux[0].target_dir, "/t");
    assert!(linux[1].args.contains(&"--clippy".to_string()));
    assert!(plan("clippy-android", "/t", None).is_err());
    let android = plan("clippy-android", "/t", Some("/ndk")).unwrap();
    assert_eq!(android[1].env, ndk_env("/ndk"));
    assert_eq!(plan("clippy-windows", "/t", None).unwrap().len(), 1);
    assert_eq!(plan("clippy-macos", "/t", None).unwrap().len(), 2);
    let error = plan("nonsense", "/t", None).unwrap_err().to_string();
    assert!(error.contains("unknown lane: nonsense"));
}
