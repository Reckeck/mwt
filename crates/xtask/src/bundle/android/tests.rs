use super::*;

#[test]
fn targets_default_to_all_abis() {
    assert_eq!(targets(None), ["aarch64", "armv7", "x86_64"]);
    assert_eq!(targets(Some("  ")), ["aarch64", "armv7", "x86_64"]);
}

#[test]
fn targets_split_on_any_whitespace() {
    assert_eq!(targets(Some("aarch64\tx86_64\n")), ["aarch64", "x86_64"]);
}

#[test]
fn publishing_needs_the_release_key() {
    assert!(publish_key_error("true", "").is_some());
    assert!(publish_key_error("true", "abc").is_none());
    assert!(publish_key_error("false", "").is_none());
    assert!(publish_key_error("", "").is_none());
}

#[test]
fn missing_variable_names_itself() {
    let error = required("MWT_NO_SUCH_VARIABLE_FOR_TEST", "a thing").unwrap_err();
    assert_eq!(error.to_string(), "MWT_NO_SUCH_VARIABLE_FOR_TEST: a thing");
}
