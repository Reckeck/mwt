//! The CI images: the tag is a hash of everything baked into them, the registry is ghcr.io.

use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Where the images live unless `MWT_CI_REGISTRY` says otherwise; `ci-<kind>:<tag>` goes under it.
pub const DEFAULT_REGISTRY: &str = "ghcr.io/reckeck/mwt";

/// The files whose bytes go into the tag, in order.
const FILES: [&str; 3] = [
    "scripts/ci/ci.Dockerfile",
    "scripts/ci/apt-packages.sh",
    "scripts/ci/install-chrome.sh",
];

/// The non-blank lines of the `[tools]` section of mise.toml (tasks and env do not matter).
fn tools(mise_toml: &str) -> Vec<&str> {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in mise_toml.lines() {
        if line.starts_with('[') {
            inside = line.starts_with("[tools]");
        } else if inside && !line.trim().is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// 12 hex digits of the SHA-256 of the inputs, then the tools lines, newline-terminated.
pub fn hash(files: &[Vec<u8>], mise_toml: &str) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(file);
    }
    for line in tools(mise_toml) {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    let mut hex = String::new();
    for byte in digest.iter().take(6) {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

pub fn tag(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    for name in FILES {
        files.push(std::fs::read(root.join(name)).with_context(|| format!("read {name}"))?);
    }
    let mise = std::fs::read_to_string(root.join("mise.toml")).context("read mise.toml")?;
    Ok(hash(&files, &mise))
}

pub fn registry() -> String {
    std::env::var("MWT_CI_REGISTRY")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REGISTRY.into())
}

/// `<registry>/ci-<kind>:<tag>`.
pub fn image(kind: &str, tag: &str) -> String {
    format!("{}/ci-{kind}:{tag}", registry())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tools_section_counts() {
        let text = "[env]\nA = 1\n\n[tools]\nnode = \"1\"\n\n  \nrust = \"2\"\n[tasks]\nx = 3\n";
        assert_eq!(tools(text), ["node = \"1\"", "rust = \"2\""]);
    }

    #[test]
    fn hash_is_twelve_hex_digits_and_follows_the_inputs() {
        let one = hash(&[b"a".to_vec()], "[tools]\nnode = \"1\"\n");
        assert_eq!(one.len(), 12);
        assert!(one.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            one,
            hash(&[b"a".to_vec()], "[tools]\nnode = \"1\"\n[tasks]\nz = 1\n")
        );
        assert_ne!(one, hash(&[b"b".to_vec()], "[tools]\nnode = \"1\"\n"));
        assert_ne!(one, hash(&[b"a".to_vec()], "[tools]\nnode = \"2\"\n"));
    }

    /// GitHub's images job still computes the tag with scripts/ci/image-tag.sh (no cargo build
    /// there): the two must agree.
    #[cfg(unix)]
    #[test]
    fn the_shell_script_and_this_agree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = std::process::Command::new("scripts/ci/image-tag.sh")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            tag(&root).unwrap()
        );
    }
}
