//! Release notes and the `gh release create` arguments: pure, so they can be tested.

/// One commit of the pre-release list.
#[derive(Debug, PartialEq)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
}

/// `git log --format=%H%x1f%s` lines.
pub fn parse_log(log: &str) -> Vec<Commit> {
    log.lines()
        .filter_map(|line| {
            let (sha, subject) = line.split_once('\u{1f}')?;
            Some(Commit {
                sha: sha.to_string(),
                subject: subject.to_string(),
            })
        })
        .collect()
}

/// The CHANGELOG section headed `## [version]`, without the heading.
pub fn section(changelog: &str, version: &str) -> String {
    let heading = format!("## [{version}]");
    let mut inside = false;
    let mut lines = Vec::new();
    for line in changelog.lines() {
        if inside && line.starts_with("## [") {
            break;
        }
        if inside {
            lines.push(line);
        } else if line.starts_with(&heading) {
            inside = true;
        }
    }
    lines.join("\n").trim().to_string()
}

pub fn release_notes(changelog: &str, version: &str, repo: &str) -> String {
    let body = section(changelog, version);
    if body.is_empty() {
        format!(
            "Changes are listed in [CHANGELOG.md](https://github.com/{repo}/blob/v{version}/CHANGELOG.md)."
        )
    } else {
        body
    }
}

pub fn prerelease_notes(
    repo: &str,
    branch: &str,
    sha: &str,
    since: Option<&str>,
    commits: &[Commit],
) -> String {
    let link = |full: &str| {
        let short: String = full.chars().take(7).collect();
        format!("[`{short}`](https://github.com/{repo}/commit/{full})")
    };
    let list = if commits.is_empty() {
        "_no commits_".to_string()
    } else {
        commits
            .iter()
            .map(|commit| format!("- {} ({})", commit.subject, link(&commit.sha)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "**Pre-release** from `{branch}` (commit {}) - for testing, not for working with real data.\n\n### Changes since {}\n\n{list}",
        link(sha),
        since.unwrap_or("the start of the project")
    )
}

pub struct Release<'a> {
    pub repo: &'a str,
    pub tag: &'a str,
    pub files: &'a [String],
    pub sha: &'a str,
    pub title: &'a str,
    pub notes_file: &'a str,
    pub pre: bool,
}

pub fn release_args(release: &Release) -> Vec<String> {
    let mut args: Vec<String> = ["release", "create", release.tag].map(String::from).into();
    args.extend(release.files.iter().cloned());
    args.extend(
        [
            "--repo",
            release.repo,
            "--target",
            release.sha,
            "--title",
            release.title,
            "--notes-file",
            release.notes_file,
        ]
        .map(String::from),
    );
    if release.pre {
        args.push("--prerelease".into());
    }
    args
}
