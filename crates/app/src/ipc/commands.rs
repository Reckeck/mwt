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
        #[allow(dead_code, reason = "composed in lib.rs by T16, which removes this allow")]
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
        #[allow(unused_macros, reason = "composed in lib.rs by T16, which removes this allow")]
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
        #[allow(unused_imports, reason = "composed in lib.rs by T16, which removes this allow")]
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
mod tests {
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
}
