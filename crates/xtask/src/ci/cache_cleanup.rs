//! `xtask cache-cleanup`: delete every Actions cache of a ref when its branch is deleted or its
//! pull request closed (cache-cleanup.yml). Env: EVENT, PR_NUMBER, BRANCH, GH_REPO, GH_TOKEN.

use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cache::capture_checked;

/// The refs whose caches go: a closed PR owns its merge and head refs, a deleted branch its own.
pub fn refs(event: &str, pr_number: &str, branch: &str) -> Result<Vec<String>> {
    if event == "pull_request" {
        if pr_number.is_empty() {
            bail!("PR_NUMBER is empty");
        }
        return Ok(vec![
            format!("refs/pull/{pr_number}/merge"),
            format!("refs/pull/{pr_number}/head"),
        ]);
    }
    if branch.is_empty() {
        bail!("BRANCH is empty");
    }
    Ok(vec![format!("refs/heads/{branch}")])
}

/// The ids on the lines `gh api --jq` printed.
pub fn parse_ids(output: &str) -> Vec<String> {
    output
        .split_whitespace()
        .filter(|word| word.bytes().all(|byte| byte.is_ascii_digit()))
        .map(String::from)
        .collect()
}

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

pub fn run() -> Result<()> {
    let repo = std::env::var("GH_REPO").context("GH_REPO is not set")?;
    for reference in refs(&var("EVENT"), &var("PR_NUMBER"), &var("BRANCH"))? {
        let listed = capture_checked(
            Command::new("gh")
                .args(["api", "--paginate", "-X", "GET"])
                .arg(format!("/repos/{repo}/actions/caches"))
                .arg("-f")
                .arg(format!("ref={reference}"))
                .args(["-F", "per_page=100", "--jq", ".actions_caches[].id"]),
        )?;
        for id in parse_ids(&listed) {
            println!("deleting cache {id} of {reference}");
            capture_checked(
                Command::new("gh")
                    .args(["api", "-X", "DELETE"])
                    .arg(format!("/repos/{repo}/actions/caches/{id}")),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_pr_owns_merge_and_head() {
        assert_eq!(
            refs("pull_request", "42", "").unwrap(),
            ["refs/pull/42/merge", "refs/pull/42/head"]
        );
    }

    #[test]
    fn a_deleted_branch_owns_its_head_ref() {
        assert_eq!(
            refs("delete", "", "feature/x").unwrap(),
            ["refs/heads/feature/x"]
        );
    }

    #[test]
    fn missing_names_are_errors() {
        assert!(refs("pull_request", "", "").is_err());
        assert!(refs("delete", "", "").is_err());
    }

    #[test]
    fn ids_are_the_numeric_lines() {
        assert_eq!(parse_ids("12\n345\n\n"), ["12", "345"]);
        assert!(parse_ids("").is_empty());
    }
}
