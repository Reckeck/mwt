//! The facade side of [`Books`]: the open vault is the source of a read
//! context. `Books` itself lives in `money/books.rs` and names no vault type.

use std::collections::HashMap;

use crate::Amount;
use crate::error::Result;
use crate::model::device_secrets::DeviceSecrets;
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;
use crate::money::books::{Books, BooksSource};
use mwt_model::CryptoHistory;

use super::UnlockedVault;
use crate::money::valuation::ValuationCurve;

impl BooksSource for UnlockedVault {
    fn doc(&self) -> &VaultDoc {
        &self.doc
    }

    fn base_and_fx(&self) -> (String, Amount) {
        crate::usecases::money::portfolio::base_and_fx(self)
    }

    fn price_usd_for(&self, asset_key: &str) -> Amount {
        crate::money::portfolio::price_usd_for(&self.books(), asset_key)
    }

    fn price_base_for(&self, asset_key: &str, fx: Amount) -> Amount {
        crate::money::portfolio::price_base_for(&self.books(), asset_key, fx)
    }

    fn valuation_curves(&self) -> HashMap<String, ValuationCurve> {
        crate::money::valuation::valuation_curves(&self.books())
    }

    fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()> {
        crate::usecases::money::transactions::prepare_transaction(self, tx)
    }

    fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount {
        crate::planning::envelopes::earmarked(
            &self.doc,
            &crate::model::ids::StoreId::from(store_id),
            asset_key,
        )
    }

    fn keys_on_device(&self) -> bool {
        UnlockedVault::keys_on_device(self)
    }

    fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        UnlockedVault::device_secrets_for_request(self)
    }

    fn crypto_history(&self, cutoffs: &[i64], in_scope: &dyn Fn(&str) -> bool) -> CryptoHistory {
        crate::cryptotrack::service::totals::crypto_history(&Books::new(self), cutoffs, in_scope)
    }
}

#[cfg(test)]
mod tests {
    use crate::money::books::BUILDS;

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
        let before = builds();
        let books = vault.books();
        assert_eq!(builds(), before, "nothing is built before the first ask");
        books.rate_book();
        books.curves().count();
        books.base();
        books.rate_book();
        assert_eq!(builds(), before + 1);
        // The other fields are built once too: a repeated ask is the same value.
        assert!(std::ptr::eq(books.shared_ratios(), books.shared_ratios()));
        assert!(std::ptr::eq(books.shared_extras(), books.shared_extras()));
        assert_eq!(builds(), before + 1, "they do not build another book");
    }

    #[test]
    fn books_equals_per_call_values() {
        let (_dir, vault) = open_vault();
        let books = vault.books();
        let fresh = crate::money::rates::rate_book(&vault.books());
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

    #[test]
    fn books_base_and_fx_equals_vault() {
        let (_dir, vault) = open_vault();
        let books = vault.books();
        let (vault_base, vault_fx) = crate::usecases::money::portfolio::base_and_fx(&vault);
        assert_eq!(books.base_and_fx(), (vault_base.as_str(), vault_fx));
        assert_eq!(
            books.price_usd("fiat:usd"),
            crate::money::portfolio::price_usd_for(&vault.books(), "fiat:usd")
        );
        let fx = crate::usecases::money::portfolio::base_and_fx(&vault).1;
        assert_eq!(
            books.price_base("coin:bitcoin"),
            crate::money::portfolio::price_base_for(&vault.books(), "coin:bitcoin", fx)
        );
        assert!(std::ptr::eq(books.doc(), &vault.doc));
    }
}
