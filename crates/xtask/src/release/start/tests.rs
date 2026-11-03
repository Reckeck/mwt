use super::*;

const BASE: &str = "0123456789abcdef0123456789abcdef01234567";

fn facts() -> Facts<'static> {
    Facts {
        repo: "Reckeck/mwt",
        remote: "origin",
        version: "0.2.0",
        base: BASE,
        main_exists: true,
        open_prs: Vec::new(),
        branch_exists: false,
        tag_exists: false,
    }
}

fn lines(steps: &[Step]) -> Vec<String> {
    steps.iter().map(flow::describe).collect()
}

fn position(lines: &[String], needle: &str) -> usize {
    lines
        .iter()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no step with {needle:?} in {lines:#?}"))
}

#[test]
fn a_normal_release_goes_branch_bump_commit_tag_push_pr_auto_merge() {
    let steps = plan(&facts()).unwrap();
    let text = lines(&steps);
    let order = [
        "switch -C release/v0.2.0",
        "version files",
        "commit --all -m 'chore(release): v0.2.0'",
        "tag --force release-base/v0.2.0",
        "push origin refs/heads/release/v0.2.0 refs/tags/release-base/v0.2.0",
        "pr create",
        "pr merge release/v0.2.0",
    ];
    let at: Vec<usize> = order.iter().map(|needle| position(&text, needle)).collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{text:#?}");
    assert_eq!(steps[0], git(&["switch", "-C", "release/v0.2.0", BASE]));
    assert!(text.last().unwrap().contains("--auto --squash"));
    assert!(text[position(&text, "pr create")].contains("--base main"));
    // nothing to close, delete or create
    assert!(
        !text
            .iter()
            .any(|line| line.contains("--delete") || line.contains("pr close"))
    );
    assert!(!text.iter().any(|line| line.contains("refs/heads/main")));
}

#[test]
fn the_first_run_creates_main_from_development_before_anything_else() {
    let steps = plan(&Facts {
        main_exists: false,
        ..facts()
    })
    .unwrap();
    assert_eq!(
        steps[0],
        git(&["push", "origin", &format!("{BASE}:refs/heads/main")])
    );
    assert_eq!(steps.len(), plan(&facts()).unwrap().len() + 1);
}

#[test]
fn an_open_pr_of_the_same_version_is_closed_and_its_branch_and_tag_deleted_first() {
    let steps = plan(&Facts {
        open_prs: vec![OpenPr {
            number: 7,
            head: "release/v0.2.0".into(),
        }],
        branch_exists: true,
        tag_exists: true,
        ..facts()
    })
    .unwrap();
    let text = lines(&steps);
    let close = position(&text, "pr close 7 --repo Reckeck/mwt");
    let branch = position(&text, "push origin --delete release/v0.2.0");
    let tag = position(&text, "push origin --delete refs/tags/release-base/v0.2.0");
    let start = position(&text, "switch -C release/v0.2.0");
    assert!(close < branch && branch < tag && tag < start, "{text:#?}");
    // and the release is made again from the current development
    assert!(text[start].ends_with(BASE));
}

#[test]
fn a_leftover_branch_without_a_pr_is_deleted_too() {
    let text = lines(
        &plan(&Facts {
            branch_exists: true,
            ..facts()
        })
        .unwrap(),
    );
    assert!(
        text.iter()
            .any(|line| line.contains("--delete release/v0.2.0"))
    );
    assert!(!text.iter().any(|line| line.contains("pr close")));
}

#[test]
fn another_open_release_stops_the_run() {
    let error = plan(&Facts {
        open_prs: vec![OpenPr {
            number: 3,
            head: "release/v0.1.5".into(),
        }],
        ..facts()
    })
    .unwrap_err();
    assert!(error.to_string().contains("#3"), "{error}");
}

#[test]
fn only_release_branches_count_as_open_releases() {
    let json = r#"[{"number":4,"headRefName":"release/v0.2.0"},{"number":5,"headRefName":"hotfix/x"},{"number":6,"headRefName":"backmerge/v0.1.0"}]"#;
    assert_eq!(
        parse_open_prs(json).unwrap(),
        vec![OpenPr {
            number: 4,
            head: "release/v0.2.0".into()
        }]
    );
    assert!(parse_open_prs("[]").unwrap().is_empty());
    assert!(parse_open_prs("not json").is_err());
}

#[test]
fn names_follow_the_version() {
    assert_eq!(branch_name("1.2.3"), "release/v1.2.3");
    assert_eq!(base_tag("1.2.3"), "release-base/v1.2.3");
}
