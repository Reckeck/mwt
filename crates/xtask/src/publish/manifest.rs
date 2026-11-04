//! The updater manifest (`latest.json`, `latest-dev.json` on the dev channel): Tauri's static
//! JSON format plus two fields of ours (`release_url`, `apk_url`) for installs that only notify.
//! Built from the `.sig` files next to the bundles; a platform without one is left out.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// The pre-release that holds the dev manifest under a stable URL. Never pruned.
pub const DEV_RELEASE_TAG: &str = "dev-updater";

const PREFIX: &str = "multi-wallet-tracker";

pub fn file_name(pre: bool) -> &'static str {
    if pre {
        "latest-dev.json"
    } else {
        "latest.json"
    }
}

pub struct Input<'a> {
    pub repo: &'a str,
    pub tag: &'a str,
    /// Without the leading `v`.
    pub version: &'a str,
    pub notes: &'a str,
    /// RFC 3339.
    pub pub_date: &'a str,
    /// Every file name of the release directory.
    pub names: &'a [String],
    /// The text of `<name>.sig`, by the name of the bundle (not of the `.sig`).
    pub signatures: &'a BTreeMap<String, String>,
}

/// The bundle names whose `.sig` exists, to be read by the caller.
pub fn signed_names(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter_map(|name| name.strip_suffix(".sig"))
        .map(String::from)
        .collect()
}

fn download_url(input: &Input, name: &str) -> String {
    format!(
        "https://github.com/{}/releases/download/{}/{name}",
        input.repo, input.tag
    )
}

pub fn build(input: &Input) -> (Value, Vec<String>) {
    let version = input.version;
    let mut warnings = Vec::new();
    let mut platforms = serde_json::Map::new();
    let mut entry = |keys: &[&str], name: Option<String>, label: &str| {
        let signed = name
            .as_ref()
            .and_then(|name| input.signatures.get(name).map(|sig| (name, sig)));
        let Some((name, signature)) = signed else {
            warnings.push(format!("no signed {label}: its updater entry is left out"));
            return;
        };
        for key in keys {
            platforms.insert(
                (*key).to_string(),
                json!({ "signature": signature.trim(), "url": download_url(input, name) }),
            );
        }
    };
    entry(
        &["linux-x86_64"],
        Some(format!("{PREFIX}-{version}-x86_64.AppImage")),
        "AppImage",
    );
    let setup = format!("{PREFIX}-{version}-x64-setup.exe");
    entry(
        &["windows-x86_64", "windows-x86_64-nsis"],
        Some(setup),
        "NSIS setup",
    );
    // One msi per language; the first signed one by name (en-US sorts before ru-RU).
    let msi_prefix = format!("{PREFIX}-{version}-x64-");
    let msi = input
        .names
        .iter()
        .filter(|name| name.starts_with(&msi_prefix) && name.ends_with(".msi"))
        .find(|name| input.signatures.contains_key(*name))
        .cloned();
    entry(&["windows-x86_64-msi"], msi, "MSI");

    let apk = format!("{PREFIX}-{version}-android-universal.apk");
    let apk_url = input
        .names
        .contains(&apk)
        .then(|| download_url(input, &apk));
    let manifest = json!({
        "version": version,
        "notes": input.notes,
        "pub_date": input.pub_date,
        "platforms": platforms,
        "release_url": format!("https://github.com/{}/releases/tag/{}", input.repo, input.tag),
        "apk_url": apk_url,
    });
    (manifest, warnings)
}

/// `gh` arguments that put the dev manifest on `dev-updater`: upload over the old one when the
/// release exists, create it (a pre-release) otherwise.
pub fn dev_release_args(repo: &str, sha: &str, file: &str, exists: bool) -> Vec<String> {
    let mut args: Vec<String> = if exists {
        ["release", "upload", DEV_RELEASE_TAG, file, "--clobber"]
            .map(String::from)
            .into()
    } else {
        [
            "release",
            "create",
            DEV_RELEASE_TAG,
            file,
            "--target",
            sha,
            "--prerelease",
            "--title",
            "Updater manifest (dev)",
            "--notes",
            "latest-dev.json for the dev channel updater. Updated by every pre-release; not a build.",
        ]
        .map(String::from)
        .into()
    };
    args.extend(["--repo".to_string(), repo.to_string()]);
    args
}
