//! The read context of one request: the rates at a moment, built from the
//! open vault the first time something asks and then shared (D4, OD7).
//! A request that needs the book more than once takes one `Books` and passes
//! it on, instead of building the book again for every step.

use std::cell::OnceCell;

use crate::service::RateBook;
use crate::service::UnlockedVault;

#[cfg(test)]
thread_local! {
    /// How many books this thread built (the counter of `books_build_each_field_once`).
    pub(crate) static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Lazy read context over an open vault.
pub struct Books<'vault> {
    vault: &'vault UnlockedVault,
    rate_book: OnceCell<RateBook>,
}

impl<'vault> Books<'vault> {
    pub(crate) fn new(vault: &'vault UnlockedVault) -> Self {
        Self {
            vault,
            rate_book: OnceCell::new(),
        }
    }

    /// The rates at a moment, built on first use.
    pub fn rate_book(&self) -> &RateBook {
        self.rate_book.get_or_init(|| self.vault.build_rate_book())
    }

    /// The base currency code, lower case.
    pub fn base(&self) -> &str {
        self.rate_book().base()
    }

    /// The asset keys that have a dated valuation curve (part of the book).
    pub fn curves(&self) -> impl Iterator<Item = &str> {
        self.rate_book().curves().keys().map(String::as_str)
    }
}

impl UnlockedVault {
    /// A fresh read context; the book inside is built once, when first used.
    pub fn books(&self) -> Books<'_> {
        Books::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::BUILDS;
    use super::*;

    fn open_vault() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
        (dir, vault)
    }

    fn builds() -> usize {
        BUILDS.with(std::cell::Cell::get)
    }

    #[test]
    fn books_build_each_field_once() {
        let (_dir, vault) = open_vault();
        let books = vault.books();
        let before = builds();
        assert_eq!(builds(), before, "nothing is built before the first ask");
        books.rate_book();
        books.curves().count();
        books.base();
        books.rate_book();
        assert_eq!(builds(), before + 1);
    }

    #[test]
    fn books_equals_per_call_values() {
        let (_dir, vault) = open_vault();
        let books = vault.books();
        let fresh = vault.rate_book();
        assert_eq!(books.base(), fresh.base());
        assert_eq!(books.curves().count(), fresh.curves().len());
        assert_eq!(
            books.rate_book().fx_at("eur", 1_700_000_000),
            fresh.fx_at("eur", 1_700_000_000)
        );
        assert_eq!(
            books.rate_book().price_at("coin:bitcoin", 1_700_000_000),
            fresh.price_at("coin:bitcoin", 1_700_000_000)
        );
    }
}
