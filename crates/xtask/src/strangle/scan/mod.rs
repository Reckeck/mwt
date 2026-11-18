//! The one `syn` scanner: facts of a Rust source (methods of `impl UnlockedVault`, calls, tauri
//! commands, test code, inline test modules, `mod x;` declarations) and the set of scanned files.

use std::collections::BTreeSet;

mod corpus;
mod visitor;

#[cfg(test)]
mod tests;

pub(in crate::strangle) use corpus::{Corpus, Scanned, UnscannedMacro};
pub use visitor::scan_source;
pub(in crate::strangle) use visitor::{is_test_attr, macro_arguments, macro_line, names_type};

/// The type whose impl blocks the areas are made of.
pub(in crate::strangle) const VAULT_TYPE: &str = "UnlockedVault";

/// The receiver of a method on the vault itself.
pub(in crate::strangle) const SELF_RECEIVER: &str = "self";

/// The receivers `on_vault` counts as the vault: `self` and `vault`.
pub(in crate::strangle) const VAULT_RECEIVERS: [&str; 2] = [SELF_RECEIVER, "vault"];

/// A method of `impl UnlockedVault`.
#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    /// Plain `pub`; `pub(crate)`, `pub(super)` and private are all hidden.
    pub public: bool,
    /// The receiver is `&mut self`: a command, not a query.
    pub mutating: bool,
    pub line: usize,
    pub in_test: bool,
}

/// A method call `x.name(..)` or `Self::name(..)`.
#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub line: usize,
    /// The receiver is `self` or `vault`, or the path starts with `Self::`.
    pub on_vault: bool,
    pub in_test: bool,
}

/// A `mod name;` declaration (the module lives in another file).
#[derive(Debug, Clone)]
pub struct ModDecl {
    pub name: String,
    pub in_test: bool,
}

/// A function marked `#[test]` (or `#[tokio::test]` and the like).
#[derive(Debug, Clone)]
pub struct TestFn {
    pub name: String,
}

#[derive(Debug, Default)]
pub struct FileFacts {
    /// Newlines, as `wc -l` counts them.
    pub lines: usize,
    /// The line of the `#[cfg(test)]` of the first inline `mod x { .. }`.
    pub first_test_line: Option<usize>,
    pub mods: Vec<ModDecl>,
    pub methods: Vec<Method>,
    pub tests: Vec<TestFn>,
    pub calls: Vec<Call>,
    /// Per `#[tauri::command]` function: the method names its body calls, in source order.
    pub commands: Vec<Vec<String>>,
    /// Every identifier written in test code.
    pub test_idents: BTreeSet<String>,
    /// Every identifier written outside test code (all of it matter when the whole file is a test).
    pub other_idents: BTreeSet<String>,
    /// Lines of macro calls whose body is not a comma-separated list of expressions: their calls
    /// are not seen.
    pub unscanned: Vec<usize>,
}
