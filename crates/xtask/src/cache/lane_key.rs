//! `cargo xtask lane-key <lane>`: the cache key of one lane, `<lane>-<16 hex>` (replaces
//! lane-key.sh). The hash is everything that changes how the DEPENDENCIES compile and nothing
//! about our own code:
//!   - the rustc and sccache versions (a `build-*` lane has no rustc on the runner: the image tag
//!     and SCCACHE_PIN, the sccache the Windows job installs),
//!   - the channel, the CI image tag and the profile switches set in the workflow env,
//!   - `.cargo/config.toml` and the text of the deps-cook recipes,
//!   - the external packages of Cargo.lock (workspace crates, whose version a release bumps,
//!     have no `source =` line and are left out),
//!   - every Cargo.toml without its `version =` line (features, [profile.*], dependency lists).
//!
//! The `node` lane hashes the pnpm inputs instead: the lock file, the manifests, the image and the
//! node and pnpm versions of mise.toml.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use super::capture_checked;

/// The recipes of deps_cook.rs: editing them renews every key.
const COOK_SOURCE: &str = include_str!("deps_cook.rs");

/// Directory names never searched for manifests.
const SKIPPED_DIRS: [&str; 7] = [
    "node_modules",
    ".rust-target",
    ".cache",
    "target",
    ".git",
    ".jj",
    "dist",
];

/// Values from the workflow environment that reach the hash.
#[derive(Default, Clone, Debug)]
pub struct EnvSwitches {
    pub sccache_pin: String,
    pub image: String,
    pub debug: String,
    pub channel: String,
    pub lto: String,
    pub units: String,
}

impl EnvSwitches {
    fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).unwrap_or_default();
        Self {
            sccache_pin: var("SCCACHE_PIN"),
            image: var("CI_IMAGE_TAG"),
            debug: var("CARGO_PROFILE_DEV_DEBUG"),
            channel: var("RELEASE_CHANNEL"),
            lto: var("CARGO_PROFILE_RELEASE_LTO"),
            units: var("CARGO_PROFILE_RELEASE_CODEGEN_UNITS"),
        }
    }
}

/// Everything a Rust lane hashes, as text.
pub struct RustInputs<'a> {
    /// `rustc -V` and `sccache --version` output, or for `build-*` the image tag (from images.rs).
    pub toolchain: String,
    pub env: EnvSwitches,
    pub cargo_config: &'a str,
    pub cook_source: &'a str,
    pub cargo_lock: &'a str,
    /// (path, content) in any order.
    pub manifests: Vec<(String, String)>,
}

/// The blocks of Cargo.lock that have a `source = ` line: registry and git packages.
pub fn external_packages(lock: &str) -> String {
    let normalized = lock.replace("\r\n", "\n");
    let blocks: Vec<&str> = normalized
        .split("\n\n")
        .filter(|block| block.lines().any(|line| line.starts_with("source = ")))
        .collect();
    let mut text = blocks.join("\n\n");
    text.push('\n');
    text
}

/// A manifest without its `version = ` lines.
fn strip_version(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.starts_with("version = "))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn digest(lane: &str, text: &str) -> String {
    let hash = Sha256::digest(text.as_bytes());
    let hex: String = hash
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{lane}-{hex}")
}

fn rust_text(inputs: &RustInputs) -> String {
    let env = &inputs.env;
    let mut text = String::new();
    text.push_str(&inputs.toolchain);
    text.push_str(&format!(
        "image={}\ndebug={}\nchannel={}\nlto={} units={}\n",
        env.image, env.debug, env.channel, env.lto, env.units
    ));
    text.push_str(inputs.cargo_config);
    text.push_str(inputs.cook_source);
    text.push_str(&external_packages(inputs.cargo_lock));
    let mut manifests: Vec<&(String, String)> = inputs.manifests.iter().collect();
    manifests.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    for (path, content) in manifests {
        text.push_str(&format!("# {path}\n{}", strip_version(content)));
    }
    text
}

/// The key of a Rust lane from already-read inputs.
pub fn rust_key(lane: &str, inputs: &RustInputs) -> String {
    digest(lane, &rust_text(inputs))
}

/// Everything the `node` lane hashes.
pub struct NodeInputs<'a> {
    pub pnpm_lock: &'a str,
    pub package_json: &'a str,
    pub pnpm_workspace: Option<&'a str>,
    pub npmrc: Option<&'a str>,
    pub image: &'a str,
    /// The `node = ...` and `pnpm = ...` entries of mise.toml `[tools]`.
    pub node_version: &'a str,
    pub pnpm_version: &'a str,
}

pub fn node_key(inputs: &NodeInputs) -> String {
    let mut text = String::new();
    text.push_str(inputs.pnpm_lock);
    text.push_str(inputs.package_json);
    text.push_str(inputs.pnpm_workspace.unwrap_or_default());
    text.push_str(inputs.npmrc.unwrap_or_default());
    text.push_str(&format!(
        "image={}\nnode={}\npnpm={}\n",
        inputs.image, inputs.node_version, inputs.pnpm_version
    ));
    digest("node", &text)
}

/// `[tools]` entry `name` of mise.toml: a string, or a table's `version`.
pub fn tool_version(mise_toml: &str, name: &str) -> Result<String> {
    let table: toml::Table = mise_toml.parse().context("parse mise.toml")?;
    let value = table
        .get("tools")
        .and_then(|tools| tools.get(name))
        .with_context(|| format!("mise.toml [tools] has no {name}"))?;
    let version = value
        .as_str()
        .or_else(|| value.get("version").and_then(toml::Value::as_str))
        .with_context(|| format!("mise.toml [tools] {name} is not a version"))?;
    Ok(version.to_string())
}

fn collect_manifests(root: &Path, dir: &Path, found: &mut Vec<(String, String)>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            if SKIPPED_DIRS.contains(&name.as_str()) || relative == "crates/app/gen" {
                continue;
            }
            collect_manifests(root, &path, found)?;
        } else if name == "Cargo.toml" {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            found.push((format!("./{relative}"), std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

fn read_optional(root: &Path, name: &str) -> Result<Option<String>> {
    let path = root.join(name);
    path.is_file()
        .then(|| std::fs::read_to_string(&path).with_context(|| format!("read {name}")))
        .transpose()
}

fn read(root: &Path, name: &str) -> Result<String> {
    std::fs::read_to_string(root.join(name)).with_context(|| format!("read {name}"))
}

fn node_lane(root: &Path) -> Result<String> {
    let mise = read(root, "mise.toml")?;
    let workspace = read_optional(root, "pnpm-workspace.yaml")?;
    let npmrc = read_optional(root, ".npmrc")?;
    let lock = read(root, "pnpm-lock.yaml")?;
    let package = read(root, "package.json")?;
    let image = std::env::var("CI_IMAGE_TAG").unwrap_or_default();
    let node = tool_version(&mise, "node")?;
    let pnpm = tool_version(&mise, "pnpm")?;
    Ok(node_key(&NodeInputs {
        pnpm_lock: &lock,
        package_json: &package,
        pnpm_workspace: workspace.as_deref(),
        npmrc: npmrc.as_deref(),
        image: &image,
        node_version: &node,
        pnpm_version: &pnpm,
    }))
}

fn rust_lane(root: &Path, lane: &str) -> Result<String> {
    let env = EnvSwitches::from_env();
    let toolchain = if lane.starts_with("build-") {
        format!(
            "{}\nsccache={}\n",
            crate::images::tag(root)?,
            env.sccache_pin
        )
    } else {
        let rustc = capture_checked(Command::new("rustc").arg("-V"))?;
        let sccache = capture_checked(Command::new("sccache").arg("--version"))?;
        format!("{rustc}{sccache}")
    };
    let mut manifests = Vec::new();
    collect_manifests(root, root, &mut manifests)?;
    let config = read(root, ".cargo/config.toml")?;
    let lock = read(root, "Cargo.lock")?;
    Ok(rust_key(
        lane,
        &RustInputs {
            toolchain,
            env,
            cargo_config: &config,
            cook_source: COOK_SOURCE,
            cargo_lock: &lock,
            manifests,
        },
    ))
}

pub fn run(root: &Path, lane: &str) -> Result<String> {
    if lane.is_empty() {
        bail!("lane name is empty");
    }
    if lane == "node" {
        node_lane(root)
    } else {
        rust_lane(root, lane)
    }
}
