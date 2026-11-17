//! `forward!`: one-line delegates that keep the old `UnlockedVault` methods alive while the
//! logic lives elsewhere (spec D3, OD7). Entries sit between `// forward:begin` and
//! `// forward:end` markers in a shim so `cargo xtask strangle forward` can regenerate them.
//!
//! ```ignore
//! forward! {
//!     /// docs copied
//!     pub fn set_info(&mut self, id: &str, info: Info) -> Result<Store>
//!         => crate::usecases::household::loyalty::set_info;   // vault first
//!     pub fn symbol_clash(&self, id: &str) -> bool
//!         => crate::household::loyalty::symbol_clash @books;  // `&self.books()` first
//! }
//! ```
//!
//! With the cargo feature `deprecated-forwards` every delegate is `#[deprecated]`.

/// Expands each entry into a delegate method; `-> R` is optional. Without a marker the function
/// receives the vault (`self`) first; `@books` passes `&self.books()` first, `@doc` passes
/// `&self.doc` first.
#[allow(unused_macros)]
macro_rules! forward {
    () => {};
    // Shared receiver.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(&self $(, $arg:ident : $ty:ty)* $(,)?) $(-> $ret:ty)?
            => $($target:ident)::+ $(@$books:ident)?;
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        #[cfg_attr(
            feature = "deprecated-forwards",
            deprecated(note = "moved out of `UnlockedVault`; call the new function directly")
        )]
        $vis fn $name(&self $(, $arg: $ty)*) $(-> $ret)? {
            $crate::service::forward::forward!(@call [$($target)::+], self, [$($books)?] $(, $arg)*)
        }
        $crate::service::forward::forward!($($rest)*);
    };
    // Exclusive receiver.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(&mut self $(, $arg:ident : $ty:ty)* $(,)?) $(-> $ret:ty)?
            => $($target:ident)::+ $(@$books:ident)?;
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        #[cfg_attr(
            feature = "deprecated-forwards",
            deprecated(note = "moved out of `UnlockedVault`; call the new function directly")
        )]
        $vis fn $name(&mut self $(, $arg: $ty)*) $(-> $ret)? {
            $crate::service::forward::forward!(@call [$($target)::+], self, [$($books)?] $(, $arg)*)
        }
        $crate::service::forward::forward!($($rest)*);
    };
    (@call [$($target:ident)::+], $this:ident, [] $(, $arg:ident)*) => {
        $($target)::+($this $(, $arg)*)
    };
    (@call [$($target:ident)::+], $this:ident, [books] $(, $arg:ident)*) => {
        $($target)::+(&$this.books() $(, $arg)*)
    };
    (@call [$($target:ident)::+], $this:ident, [doc] $(, $arg:ident)*) => {
        $($target)::+(&$this.doc $(, $arg)*)
    };
}

#[allow(unused_imports)]
pub(crate) use forward;

#[cfg(test)]
#[cfg_attr(feature = "deprecated-forwards", allow(deprecated))]
mod tests {
    struct Dummy {
        base: i64,
        doc: Doc,
        seen: std::cell::Cell<i64>,
    }

    struct Books {
        base: i64,
    }

    struct Doc {
        total: i64,
    }

    fn dummy(base: i64) -> Dummy {
        Dummy {
            base,
            doc: Doc { total: base * 2 },
            seen: std::cell::Cell::new(0),
        }
    }

    impl Dummy {
        fn books(&self) -> Books {
            Books { base: self.base }
        }
    }

    fn add_to_vault(vault: &Dummy, extra: i64) -> i64 {
        vault.base + extra
    }

    fn bump_vault(vault: &mut Dummy, extra: i64) -> i64 {
        vault.base += extra;
        vault.base
    }

    fn read_doc(doc: &Doc, extra: i64) -> i64 {
        doc.total + extra
    }

    fn note_vault(vault: &Dummy, value: i64) {
        vault.seen.set(value);
    }

    fn add_to_books(books: &Books, extra: i64) -> i64 {
        books.base + extra
    }

    impl Dummy {
        forward! {
            /// Vault-first, shared receiver.
            fn read_forward(&self, extra: i64) -> i64 => add_to_vault;
            /// Vault-first, exclusive receiver.
            fn write_forward(&mut self, extra: i64) -> i64 => bump_vault;
            /// Books-first.
            fn books_forward(&self, extra: i64) -> i64 => add_to_books @books;
            /// Doc-first.
            fn doc_forward(&self, extra: i64) -> i64 => read_doc @doc;
            /// No return type.
            fn note_forward(&self, value: i64) => note_vault;
        }
    }

    #[test]
    fn vault_first_delegate_returns_what_the_function_returns() {
        let mut vault = dummy(10);
        assert_eq!(vault.read_forward(5), 15);
        assert_eq!(vault.write_forward(5), 15);
        assert_eq!(vault.base, 15);
    }

    #[test]
    fn books_delegate_returns_what_the_function_returns() {
        let vault = dummy(10);
        assert_eq!(vault.books_forward(7), 17);
    }

    #[test]
    fn doc_first_delegate_passes_the_doc() {
        assert_eq!(dummy(10).doc_forward(5), 25);
    }

    #[test]
    fn delegate_without_a_return_type_returns_unit() {
        let vault = dummy(1);
        vault.note_forward(9);
        assert_eq!(vault.seen.get(), 9);
    }

    /// Feature on: the delegate is deprecated, so the expected lint fires (an unfulfilled
    /// expectation would be a warning, which `-D warnings` turns into an error).
    #[cfg(feature = "deprecated-forwards")]
    #[test]
    #[expect(deprecated)]
    fn delegate_is_deprecated_when_the_feature_is_on() {
        assert_eq!(dummy(1).read_forward(1), 2);
    }

    /// Feature off: the same call raises no deprecation (`-D warnings` would fail otherwise).
    #[cfg(not(feature = "deprecated-forwards"))]
    #[test]
    fn delegate_is_not_deprecated_by_default() {
        assert_eq!(dummy(1).read_forward(1), 2);
    }
}
