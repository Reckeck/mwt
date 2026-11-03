use super::*;

fn lines(steps: &[Step]) -> Vec<String> {
    steps.iter().map(flow::describe).collect()
}

#[test]
fn equal_trees_mean_a_release_from_release_start() {
    assert_eq!(decide(Some("abc"), "abc"), Ok(Mode::Ours));
}

#[test]
fn a_release_without_a_base_tag_is_a_hotfix() {
    assert_eq!(decide(None, "abc"), Ok(Mode::Plain));
}

#[test]
fn different_trees_with_a_base_tag_are_refused() {
    let error = decide(Some("abc"), "def").unwrap_err();
    assert!(error.contains("differs"), "{error}");
}

#[test]
fn the_ours_merge_starts_at_the_release_tip_not_at_development() {
    let text = lines(&merge_steps(&Mode::Ours, "origin", "0.2.0"));
    assert_eq!(
        text[0],
        "$ git switch -C backmerge/v0.2.0 release-base/v0.2.0"
    );
    assert!(text[1].contains("merge -s ours"), "{}", text[1]);
    assert!(text[1].ends_with("origin/main"), "{}", text[1]);
    assert!(!text.iter().any(|line| line.contains("origin/development")));
}

#[test]
fn the_hotfix_merge_is_a_plain_merge_into_development() {
    let text = lines(&merge_steps(&Mode::Plain, "origin", "0.2.1"));
    assert_eq!(
        text[0],
        "$ git switch -C backmerge/v0.2.1 origin/development"
    );
    assert!(text[1].contains(" merge -m "), "{}", text[1]);
    assert!(!text[1].contains("ours"), "{}", text[1]);
    assert!(text[1].ends_with("origin/main"), "{}", text[1]);
}

#[test]
fn the_pr_goes_into_development_with_a_merge_commit() {
    let text = lines(&pr_steps("Reckeck/mwt", "0.2.0", "origin", false));
    assert_eq!(
        text[0],
        "$ git push --force origin refs/heads/backmerge/v0.2.0"
    );
    assert!(text[1].contains("pr create") && text[1].contains("--base development"));
    assert!(text[2].contains("pr merge backmerge/v0.2.0") && text[2].ends_with("--auto --merge"));
    // an open PR is not created again, but auto-merge is still asked for
    let again = lines(&pr_steps("Reckeck/mwt", "0.2.0", "origin", true));
    assert_eq!(again.len(), 2);
    assert!(again[1].contains("--auto --merge"));
}

#[test]
fn the_flag_less_command_keeps_its_commands() {
    let list = list_args("Reckeck/mwt", "main", "development");
    assert!(list.contains(&"main".to_string()));
}
