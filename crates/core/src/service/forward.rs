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

/// Expands each entry into a delegate method. Without `@books` the function receives the vault
/// (`self`) first; with `@books` it receives `&self.books()` first.
#[allow(unused_macros)]
macro_rules! forward {
    () => {};
    // Shared receiver.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(&self $(, $arg:ident : $ty:ty)* $(,)?) -> $ret:ty
            => $($target:ident)::+ $(@$books:ident)?;
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        #[cfg_attr(
            feature = "deprecated-forwards",
            deprecated(note = "moved out of `UnlockedVault`; call the new function directly")
        )]
        $vis fn $name(&self $(, $arg: $ty)*) -> $ret {
            $crate::service::forward::forward!(@call [$($target)::+], self, [$($books)?] $(, $arg)*)
        }
        $crate::service::forward::forward!($($rest)*);
    };
    // Exclusive receiver.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(&mut self $(, $arg:ident : $ty:ty)* $(,)?) -> $ret:ty
            => $($target:ident)::+ $(@$books:ident)?;
        $($rest:tt)*
    ) => {
        $(#[$meta])*
        #[cfg_attr(
            feature = "deprecated-forwards",
            deprecated(note = "moved out of `UnlockedVault`; call the new function directly")
        )]
        $vis fn $name(&mut self $(, $arg: $ty)*) -> $ret {
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
}

#[allow(unused_imports)]
pub(crate) use forward;

#[cfg(test)]
#[cfg_attr(feature = "deprecated-forwards", allow(deprecated))]
mod tests {
    struct Dummy {
        base: i64,
    }

    struct Books {
        base: i64,
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
        }
    }

    #[test]
    fn vault_first_delegate_returns_what_the_function_returns() {
        let mut vault = Dummy { base: 10 };
        assert_eq!(vault.read_forward(5), 15);
        assert_eq!(vault.write_forward(5), 15);
        assert_eq!(vault.base, 15);
    }

    #[test]
    fn books_delegate_returns_what_the_function_returns() {
        let vault = Dummy { base: 10 };
        assert_eq!(vault.books_forward(7), 17);
    }

    /// Feature on: the delegate is deprecated, so the expected lint fires (an unfulfilled
    /// expectation would be a warning, which `-D warnings` turns into an error).
    #[cfg(feature = "deprecated-forwards")]
    #[test]
    #[expect(deprecated)]
    fn delegate_is_deprecated_when_the_feature_is_on() {
        assert_eq!(Dummy { base: 1 }.read_forward(1), 2);
    }

    /// Feature off: the same call raises no deprecation (`-D warnings` would fail otherwise).
    #[cfg(not(feature = "deprecated-forwards"))]
    #[test]
    fn delegate_is_not_deprecated_by_default() {
        assert_eq!(Dummy { base: 1 }.read_forward(1), 2);
    }
}
