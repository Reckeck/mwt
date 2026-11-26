//! `ipc_commands!`: one declaration per context lists its commands once and
//! generates everything the registration needs from that list.
//!
//! ```ignore
//! ipc_commands!(auth_commands, AUTH_NAMES, commands::auth: [vault_status, create_vault]);
//! ```
//!
//! The declaration defines, in the module where it stands:
//! - `macro_rules! auth_commands`, a list macro that forwards the command
//!   paths (with their `#[cfg]` attributes) to the next callback in a chain,
//! - `pub const AUTH_NAMES: &[&str]` with the command names,
//!
//! and re-exports the list macro so it can be called by path.
//! `macro_rules!` cannot join identifiers, so the list macro and the constant
//! are named explicitly instead of being derived from one context name.
//!
//! Composition: a list macro takes `[next, rest...]; accumulated items`,
//! appends its own items and calls `next!{ [rest...]; items }`; the chain
//! ends in [`generate_handler`](crate::ipc::commands::generate_handler).
//! `lib.rs` writes, for contexts `a` and `b`:
//! `.invoke_handler(a::a_commands!([b::b_commands, ipc::commands::generate_handler];))`.

/// Declares a context's command list. See the module docs.
macro_rules! ipc_commands {
    (
        $list:ident, $names:ident, $($module:ident)::+:
        [ $( $(#[$attr:meta])* $cmd:ident ),* $(,)? ]
    ) => {
        /// The command names of this context, for the registration test.
        #[cfg_attr(not(test), allow(dead_code, reason = "read by the registration guard test"))]
        pub const $names: &[&str] = &[
            $( $(#[$attr])* stringify!($cmd), )*
        ];

        $crate::ipc::commands::ipc_commands!(@collect ($) $list, ($($module)::+), [], [
            $( $(#[$attr])* $cmd, )*
        ]);
    };
    // Walk the commands one by one: the module path and the command cannot
    // be repeated together in one template, and each item needs both.
    (
        @collect ($d:tt) $list:ident, ($($module:ident)::+), [$($done:tt)*],
        [ $(#[$attr:meta])* $cmd:ident, $($rest:tt)* ]
    ) => {
        $crate::ipc::commands::ipc_commands!(@collect ($d) $list, ($($module)::+), [
            $($done)*
            $(#[$attr])* $crate::$($module)::+::$cmd,
        ], [ $($rest)* ]);
    };
    (@collect ($d:tt) $list:ident, ($($module:ident)::+), [$($done:tt)*], []) => {
        // `$d` is a literal `$`, so the generated macro can declare its own variables.
        macro_rules! $list {
            (
                [ $d($d next:ident)::+ $d(, $d($d rest:tt)*)? ]
                ; $d($d items:tt)*
            ) => {
                $d($d next)::+!(
                    [ $d($d($d rest)*)? ]
                    ; $d($d items)*
                    $($done)*
                )
            };
        }
        pub(crate) use $list;
    };
}

/// End of a chain of list macros: hands the collected items to Tauri.
macro_rules! generate_handler {
    ( [] ; $($items:tt)* ) => {
        ::tauri::generate_handler![ $($items)* ]
    };
}

pub(crate) use {generate_handler, ipc_commands};

#[cfg(test)]
pub(crate) mod tests {
    // Stand-in command modules; the real ones declare the same way.
    mod first {
        #[tauri::command]
        pub fn alpha() {}
        #[tauri::command]
        pub fn beta() {}
        ipc_commands!(first_commands, FIRST_NAMES, ipc::commands::tests::first: [alpha, beta]);
    }
    mod second {
        #[tauri::command]
        pub fn gamma() {}
        #[cfg(any())]
        #[tauri::command]
        pub fn off() {}
        ipc_commands!(second_commands, SECOND_NAMES, ipc::commands::tests::second: [
            gamma,
            #[cfg(any())]
            off,
        ]);
    }

    #[test]
    fn two_context_lists_compose_into_one_generate_handler() {
        let _handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = first::first_commands!(
            [second::second_commands, crate::ipc::commands::generate_handler];
        );
    }

    #[test]
    fn names_follow_the_list_and_honour_cfg() {
        assert_eq!(first::FIRST_NAMES, ["alpha", "beta"]);
        assert_eq!(second::SECOND_NAMES, ["gamma"]);
    }

    /// Every context's name list: the one place the guard tests need to
    /// know the contexts, next to the chain in `lib.rs`.
    fn registered_names() -> Vec<&'static str> {
        let contexts: &[&[&str]] = &[
            crate::commands::analytics::ANALYTICS_NAMES,
            crate::commands::app_settings::APP_SETTINGS_NAMES,
            crate::commands::attachments::ATTACHMENTS_NAMES,
            crate::commands::auth::AUTH_NAMES,
            crate::commands::calc::CALC_NAMES,
            crate::commands::categories::CATEGORIES_NAMES,
            crate::commands::contacts::CONTACTS_NAMES,
            crate::commands::cryptotrack::accounts::CRYPTOTRACK_ACCOUNTS_NAMES,
            crate::commands::cryptotrack::links::CRYPTOTRACK_LINKS_NAMES,
            crate::commands::cryptotrack::network::CRYPTOTRACK_NETWORK_NAMES,
            crate::commands::cryptotrack::operations::CRYPTOTRACK_OPERATIONS_NAMES,
            crate::commands::cryptotrack::positions::CRYPTOTRACK_POSITIONS_NAMES,
            crate::commands::cryptotrack::relations::CRYPTOTRACK_RELATIONS_NAMES,
            crate::commands::cryptotrack::sync::CRYPTOTRACK_SYNC_NAMES,
            crate::commands::cryptotrack::tokens::CRYPTOTRACK_TOKENS_NAMES,
            crate::commands::cryptotrack::transition::CRYPTOTRACK_TRANSITION_NAMES,
            crate::commands::cryptotrack::wipe::CRYPTOTRACK_WIPE_NAMES,
            crate::commands::enrichment::ENRICHMENT_NAMES,
            crate::commands::exchange::EXCHANGE_NAMES,
            crate::commands::losses::LOSSES_NAMES,
            crate::commands::mobile::MOBILE_NAMES,
            crate::commands::notifications::NOTIFICATIONS_NAMES,
            crate::commands::planning::PLANNING_NAMES,
            crate::commands::portfolio::PORTFOLIO_NAMES,
            crate::commands::pricing::PRICING_NAMES,
            crate::commands::rates::RATES_NAMES,
            crate::commands::sandbox::SANDBOX_NAMES,
            crate::commands::security::autolock::SECURITY_AUTOLOCK_NAMES,
            crate::commands::security::biometric::SECURITY_BIOMETRIC_NAMES,
            crate::commands::security::keys::SECURITY_KEYS_NAMES,
            crate::commands::security::raise::SECURITY_RAISE_NAMES,
            crate::commands::security::state::SECURITY_STATE_NAMES,
            crate::commands::settings::SETTINGS_NAMES,
            crate::commands::shared_events::SHARED_EVENTS_NAMES,
            crate::commands::sources::SOURCES_NAMES,
            crate::commands::stores::STORES_NAMES,
            crate::commands::sync::SYNC_NAMES,
            crate::commands::transactions_bulk::TRANSACTIONS_BULK_NAMES,
            crate::commands::transactions::TRANSACTIONS_NAMES,
            crate::commands::transfer::TRANSFER_NAMES,
            crate::commands::updates::UPDATES_NAMES,
            crate::commands::vault_backups::VAULT_BACKUPS_NAMES,
            crate::commands::vault_link::VAULT_LINK_NAMES,
            crate::commands::vault_recents::VAULT_RECENTS_NAMES,
            crate::commands::window::WINDOW_NAMES,
            crate::commands::workflows::WORKFLOWS_NAMES,
        ];
        contexts
            .iter()
            .flat_map(|names| names.iter().copied())
            .collect()
    }

    fn source_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read the commands directory") {
            let path = entry.expect("read a directory entry").path();
            if path.is_dir() {
                source_files(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    fn command_sources() -> Vec<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
        let mut files = Vec::new();
        source_files(&root, &mut files);
        files
            .iter()
            .map(|file| std::fs::read_to_string(file).expect("read a command file"))
            .collect()
    }

    /// The names of the `#[tauri::command]` and `#[ipc]` functions in one source text.
    fn command_fns(source: &str) -> Vec<String> {
        attributed_fns(source, &["#[tauri::command", "#[ipc"])
    }

    /// The names of the `#[ipc]` functions only in one source text.
    pub(crate) fn ipc_fns(source: &str) -> Vec<String> {
        attributed_fns(source, &["#[ipc"])
    }

    /// The names of the functions whose attribute starts with one of `prefixes`.
    fn attributed_fns(source: &str, prefixes: &[&str]) -> Vec<String> {
        let lines: Vec<&str> = source.lines().collect();
        let mut names = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let attribute = line.trim_start();
            if !prefixes.iter().any(|prefix| attribute.starts_with(prefix)) {
                continue;
            }
            let declaration = lines[index..]
                .iter()
                .find_map(|candidate| candidate.split_once("fn ").map(|(_, rest)| rest))
                .expect("a command attribute is followed by a function");
            let name: String = declaration
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            names.push(name);
        }
        names
    }

    #[test]
    fn every_tauri_command_is_registered() {
        let registered: std::collections::BTreeSet<String> =
            registered_names().into_iter().map(String::from).collect();
        let defined: std::collections::BTreeSet<String> = command_sources()
            .iter()
            .flat_map(|source| command_fns(source))
            .collect();
        let missing: Vec<_> = defined.difference(&registered).collect();
        let extra: Vec<_> = registered.difference(&defined).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "commands without a list entry: {missing:?}; list entries without a command: {extra:?}"
        );
    }

    #[test]
    fn every_context_list_is_composed_in_lib() {
        let lib = include_str!("../lib.rs");
        for source in command_sources() {
            for line in source
                .lines()
                .filter(|line| line.starts_with("ipc_commands!("))
            {
                let mut fields = line.trim_start_matches("ipc_commands!(").split(',');
                let list = fields.next().expect("a declaration names its list macro");
                let module = fields
                    .nth(1)
                    .and_then(|field| field.split_once(": ["))
                    .map(|(module, _)| module.trim())
                    .expect("a declaration names its module");
                let path = format!("{module}::{list}");
                assert!(composes(lib, &path), "lib.rs does not compose `{path}`");
            }
        }
    }

    /// Whether `lib` names the full `path` as a whole path, not as the tail of a longer one.
    fn composes(lib: &str, path: &str) -> bool {
        lib.match_indices(path).any(|(start, _)| {
            let before = lib[..start].chars().next_back();
            let after = lib[start + path.len()..].chars().next();
            let in_path = |symbol: char| symbol.is_alphanumeric() || symbol == '_' || symbol == ':';
            !before.is_some_and(in_path) && !after.is_some_and(in_path)
        })
    }

    /// The number of distinct command names the flat list held on the day of the switch: 400.
    /// It is the count of a debug build with the `schema` feature on (the test run, through
    /// the dev self-dependency); a release build registers 394, since `vault_schema` and the
    /// five `dev_*` commands are debug-only.
    ///
    /// The spec's 402 counted `#[tauri::command]` attributes: `get_autostart` and
    /// `set_autostart` each carry one per platform (desktop and mobile twins), so two names
    /// are counted twice.
    const COMMAND_COUNT: usize = 401;

    #[test]
    fn the_command_count_is_unchanged() {
        let names = registered_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            COMMAND_COUNT,
            "the number of commands changed: whoever adds or removes one updates COMMAND_COUNT"
        );
        assert_eq!(unique.len(), names.len(), "a command is listed twice");
    }
}
