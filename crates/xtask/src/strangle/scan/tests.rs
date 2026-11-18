use super::*;

const FIXTURE: &str = r##"
use crate::UnlockedVault;

impl UnlockedVault {
    pub fn plain(&self) -> u32 {
        let text = "} } { not a block";
        let raw = r#"pub fn fake(&self) {"#;
        self.helper(text, raw)
    }
    pub async fn later(&mut self) {
        self.persist().await;
        vault.other();
    }
    pub(crate) fn crate_wide(&self) {}
    pub(super) fn super_wide(&self) {}
    fn helper(&self, _a: &str, _b: &str) -> u32 { Self::tail() }
    fn tail() -> u32 { 1 }
}

impl Other {
    pub fn not_ours(&self) {}
}

#[cfg(test)]
mod declared;

#[cfg(test)]
mod tests {
    impl UnlockedVault {
        pub fn only_in_tests(&self) {}
    }
    #[test]
    fn pins() {
        assert_eq!(v.plain(), 1);
    }
}
"##;

fn find<'facts>(facts: &'facts FileFacts, name: &str) -> &'facts Method {
    facts
        .methods
        .iter()
        .find(|method| method.name == name)
        .unwrap()
}

#[test]
fn braces_in_strings_do_not_confuse_the_scanner() {
    let facts = scan_source(FIXTURE).unwrap();
    assert!(
        facts
            .methods
            .iter()
            .all(|method| method.name != "fake" && method.name != "not_ours")
    );
    assert!(find(&facts, "plain").public);
    assert!(!find(&facts, "plain").mutating);
    assert!(
        facts
            .calls
            .iter()
            .any(|call| call.name == "helper" && call.on_vault && !call.in_test)
    );
}

#[test]
fn async_crate_and_super_methods_are_seen_with_their_visibility() {
    let facts = scan_source(FIXTURE).unwrap();
    assert!(find(&facts, "later").public && find(&facts, "later").mutating);
    for hidden in ["crate_wide", "super_wide", "helper", "tail"] {
        assert!(!find(&facts, hidden).public, "{hidden}");
    }
    assert!(facts.calls.iter().any(|call| call.name == "persist"));
    assert!(
        facts
            .calls
            .iter()
            .any(|call| call.name == "other" && call.on_vault)
    );
    assert!(
        facts
            .calls
            .iter()
            .any(|call| call.name == "tail" && call.on_vault)
    );
}

#[test]
fn the_inline_test_module_and_the_declared_test_module_are_found() {
    let facts = scan_source(FIXTURE).unwrap();
    assert!(find(&facts, "only_in_tests").in_test);
    assert!(!find(&facts, "plain").in_test);
    let inline_line = FIXTURE
        .lines()
        .position(|line| line == "mod tests {")
        .unwrap();
    assert_eq!(facts.first_test_line, Some(inline_line));
    let declared: Vec<_> = facts
        .mods
        .iter()
        .map(|decl| (decl.name.as_str(), decl.in_test))
        .collect();
    assert_eq!(declared, [("declared", true)]);
    assert!(facts.test_idents.contains("plain"));
    assert!(!facts.other_idents.contains("assert_eq"));
}

#[test]
fn calls_inside_macro_bodies_are_seen_and_unparsable_bodies_are_listed() {
    let source = "fn f(vault: V) {\n    let a = format!(\"{}\", vault.in_format());\n    assert!(vault.in_assert(), \"m\");\n    let b = vec![vault.in_vec(), 1];\n    odd!(=> vault.hidden());\n}\n";
    let facts = scan_source(source).unwrap();
    for name in ["in_format", "in_assert", "in_vec"] {
        assert!(
            facts
                .calls
                .iter()
                .any(|call| call.name == name && call.on_vault),
            "{name}"
        );
    }
    assert!(facts.calls.iter().all(|call| call.name != "hidden"));
    assert_eq!(facts.unscanned, [5]);
}

#[test]
fn tauri_commands_list_the_methods_they_call_in_order() {
    let source = r"
        #[tauri::command]
        fn show(vault: Vault) { vault.first().second(vault.third()); }
        fn plain(vault: Vault) { vault.skipped(); }
    ";
    let facts = scan_source(source).unwrap();
    assert_eq!(facts.commands, [["first", "second", "third"]]);
}

#[test]
fn an_ipc_attribute_marks_a_command_like_tauri_command() {
    let source = r"
        #[ipc(write)]
        fn save(vault: Vault) { vault.persisted(); }
        #[crate::ipc]
        fn read(vault: Vault) { vault.peek(); }
        #[other(write)]
        fn plain(vault: Vault) { vault.skipped(); }
    ";
    let facts = scan_source(source).unwrap();
    assert_eq!(facts.commands, [["persisted"], ["peek"]]);
}

#[test]
fn test_files_are_found_by_declaration_name_and_inheritance() {
    let corpus = Corpus::from_sources(&[
        ("svc/mod.rs", "#[cfg(test)] mod golden;\nmod live;"),
        ("svc/golden.rs", "mod nested;"),
        ("svc/golden/nested.rs", "fn deep() {}"),
        ("svc/live.rs", "fn prod() {}"),
        ("svc/live_tests.rs", "fn named() {}"),
        ("svc/peer/mod.rs", "#[cfg(test)] mod cases;"),
        ("svc/peer/cases.rs", "fn one() {}"),
    ])
    .unwrap();
    let tests: Vec<&str> = [
        "svc/mod.rs",
        "svc/golden.rs",
        "svc/golden/nested.rs",
        "svc/live.rs",
        "svc/live_tests.rs",
        "svc/peer/cases.rs",
    ]
    .into_iter()
    .filter(|path| corpus.get(path).unwrap().is_test)
    .collect();
    assert_eq!(
        tests,
        [
            "svc/golden.rs",
            "svc/golden/nested.rs",
            "svc/live_tests.rs",
            "svc/peer/cases.rs"
        ]
    );
}

#[test]
fn unscanned_macro_bodies_are_shown_as_file_line_notes() {
    let corpus = Corpus::from_sources(&[
        (
            "a.rs",
            "fn f(vault: V) {\n    odd!(=> vault.hidden());\n}\n",
        ),
        ("b.rs", "fn g() {}\n"),
    ])
    .unwrap();
    assert_eq!(corpus.unscanned_notes(), ["a.rs:2: macro body not scanned"]);
}

#[test]
fn macros_that_cannot_hold_a_vault_call_are_not_unscanned() {
    let source =
        "macro_rules! m {\n    () => {};\n}\nfn f() {\n    let v = json!({ \"a\": 1 });\n}\n";
    assert!(scan_source(source).unwrap().unscanned.is_empty());
}
