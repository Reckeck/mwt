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

/// The CHANGELOG groups of cliff.toml, in its order; `None` drops the commit (release commits).
fn group(subject: &str) -> Option<&'static str> {
    let kind = subject.split([':', '(', '!']).next().unwrap_or_default();
    Some(match kind {
        "feat" => "Added",
        "fix" => "Fixed",
        "perf" => "Performance",
        "refactor" => "Internal changes",
        "docs" => "Documentation",
        "ci" | "build" => "Build and CI",
        "test" => "Tests",
        "chore" if subject.starts_with("chore(release)") => return None,
        _ => "Other",
    })
}

const GROUPS: [&str; 8] = [
    "Added",
    "Fixed",
    "Performance",
    "Internal changes",
    "Documentation",
    "Build and CI",
    "Tests",
    "Other",
];

/// `feat(ui): add x` -> `**ui:** Add x`, like the CHANGELOG lines of cliff.toml.
fn line(subject: &str) -> String {
    let Some((head, message)) = subject.split_once(": ") else {
        return subject.to_string();
    };
    if head.contains(' ') {
        return subject.to_string();
    }
    let mut chars = message.chars();
    let message = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect::<String>())
        .unwrap_or_default();
    let scope = head
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(scope, _)| scope);
    match scope {
        Some(scope) => format!("**{scope}:** {message}"),
        None => message,
    }
}

/// The pre-release notes: the commits since the closest published release tag (`since`),
/// grouped like the CHANGELOG. `more` says the log was cut, so the compare link has the rest.
pub fn prerelease_notes(
    repo: &str,
    branch: &str,
    sha: &str,
    since: Option<&str>,
    commits: &[Commit],
    more: bool,
) -> String {
    let link = |full: &str| {
        let short: String = full.chars().take(7).collect();
        format!("[`{short}`](https://github.com/{repo}/commit/{full})")
    };
    let mut sections = Vec::new();
    for name in GROUPS {
        let lines: Vec<String> = commits
            .iter()
            .filter(|commit| group(&commit.subject) == Some(name))
            .map(|commit| format!("- {} ({})", line(&commit.subject), link(&commit.sha)))
            .collect();
        if !lines.is_empty() {
            sections.push(format!("#### {name}\n\n{}", lines.join("\n")));
        }
    }
    let heading = match since {
        Some(tag) => format!(
            "### Changes since {tag}\n\n[Full diff {tag}...{short}](https://github.com/{repo}/compare/{tag}...{sha})",
            short = sha.chars().take(7).collect::<String>()
        ),
        None => "### Changes since the start of the project".to_string(),
    };
    let mut body = if sections.is_empty() {
        "_no changes_".to_string()
    } else {
        sections.join("\n\n")
    };
    if more {
        body.push_str("\n\n_Older commits are in the full diff._");
    }
    format!(
        "**Pre-release** from `{branch}` (commit {}) - for testing, not for working with real data.\n\n{heading}\n\n{body}",
        link(sha)
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
