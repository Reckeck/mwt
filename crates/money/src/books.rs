//! The read context of one request: the rates at a moment, built from the
//! open vault the first time something asks and then shared (D4, OD7).
//! A request that needs the book more than once takes one `Books` and passes
//! it on, instead of building the book again for every step.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

use mwt_base::Amount;
use mwt_base::error::Result;
use mwt_model::DeviceSecrets;
use mwt_model::Transaction;
use mwt_model::VaultDoc;

use super::rates::RateBook;
use super::shared_ratios::{SharedExtra, shared_extra_flows, shared_payment_ratios};
use super::valuation::ValuationCurve;

/// What a [`Books`] reads from the open vault. The vault itself
/// (the vault of the facade) implements it in the facade (`service/books.rs`),
/// so this module names no vault type and the money rules stay below the vault.
pub trait BooksSource {
    /// The vault document.
    fn doc(&self) -> &VaultDoc;
    /// The base currency code (never empty) and its latest cached FX rate.
    fn base_and_fx(&self) -> (String, Amount);
    /// Current USD price of one unit of the asset.
    fn price_usd_for(&self, asset_key: &str) -> Amount;
    /// Base-currency price of one unit of the asset.
    fn price_base_for(&self, asset_key: &str, fx: Amount) -> Amount;
    /// Curves of every property asset that has a valuation or a rule.
    fn valuation_curves(&self) -> HashMap<String, ValuationCurve>;
    /// The writers' checks of a transaction (dates, envelopes, ownership).
    fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()>;
    /// The part of the asset in the store that envelopes claim.
    fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount;
    /// Whether the exchange keys live on this device (security level 3).
    fn keys_on_device(&self) -> bool;
    /// The whole device secrets file for one request.
    fn device_secrets_for_request(&self) -> Result<DeviceSecrets>;
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// How many books this thread built (the counter of `books_build_each_field_once`).
    pub static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Lazy read context over an open vault.
pub struct Books<'vault> {
    vault: &'vault dyn BooksSource,
    rate_book: OnceCell<RateBook>,
    shared_ratios: OnceCell<HashMap<String, Amount>>,
    shared_extras: OnceCell<Vec<SharedExtra>>,
    base_and_fx: OnceCell<(String, Amount)>,
}

impl<'vault> Books<'vault> {
    pub fn new(vault: &'vault dyn BooksSource) -> Self {
        Self {
            vault,
            rate_book: OnceCell::new(),
            shared_ratios: OnceCell::new(),
            shared_extras: OnceCell::new(),
            base_and_fx: OnceCell::new(),
        }
    }

    /// The vault document.
    pub fn doc(&self) -> &'vault VaultDoc {
        self.vault.doc()
    }

    /// The base currency code and its latest FX rate, computed on first use.
    pub fn base_and_fx(&self) -> (&str, Amount) {
        let (base, fx) = self.base_and_fx.get_or_init(|| self.vault.base_and_fx());
        (base.as_str(), *fx)
    }

    /// Current USD price of one unit of the asset.
    pub fn price_usd(&self, asset_key: &str) -> Amount {
        self.vault.price_usd_for(asset_key)
    }

    /// Base-currency price of one unit of the asset.
    pub fn price_base(&self, asset_key: &str) -> Amount {
        self.vault.price_base_for(asset_key, self.base_and_fx().1)
    }

    /// The rates at a moment, built on first use.
    pub fn rate_book(&self) -> &RateBook {
        self.rate_book.get_or_init(|| self.build_rate_book())
    }

    /// A fresh book of this vault: the curves are built here (`valuation_curves`)
    /// and go into `RateBook::build`, which reads nothing but its input. The one
    /// place a book is built; counts builds in tests.
    pub fn build_rate_book(&self) -> RateBook {
        #[cfg(any(test, feature = "testing"))]
        BUILDS.with(|count| count.set(count.get() + 1));
        let curves = self
            .vault
            .valuation_curves()
            .into_iter()
            .map(|(key, curve)| (key, Arc::new(curve) as _))
            .collect();
        RateBook::build(self.vault.doc(), curves).with_base(self.base_and_fx().0)
    }

    /// For each payment of mine linked to a shared event: the share of it
    /// that is my own expense, computed on first use.
    pub fn shared_ratios(&self) -> &HashMap<String, Amount> {
        self.shared_ratios
            .get_or_init(|| shared_payment_ratios(self.doc()))
    }

    /// The spending entries without a payment of mine behind them (my part of
    /// other people's purchases, write-offs), collected on first use. They are
    /// not valued yet: the currency and period are asked per use.
    pub fn shared_extras(&self) -> &[SharedExtra] {
        self.shared_extras
            .get_or_init(|| shared_extra_flows(self.doc()))
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

/// Reads that need the open vault itself: its security level, its envelopes,
/// its device secrets. The areas ask the context; only the kernel holds the
/// vault (D2, OD6). The reads after `device_secrets_for_request` are
/// transitional: their callee sits in a layer above the caller (or takes the
/// vault), so no legal direct call exists yet; each goes when its callee moves
/// down or gets a port (core-strangler T213 follow-up).
impl Books<'_> {
    /// The writers' checks of a transaction (dates, envelopes, ownership).
    pub fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()> {
        self.vault.prepare_transaction(tx)
    }

    /// The part of the asset in the store that envelopes claim.
    pub fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount {
        self.vault.earmarked(store_id, asset_key)
    }

    /// Whether the exchange keys live on this device (security level 3).
    pub fn keys_on_device(&self) -> bool {
        self.vault.keys_on_device()
    }

    /// The whole device secrets file for one request (the caller drops it soon).
    pub fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        self.vault.device_secrets_for_request()
    }
}

#[cfg(test)]
mod tests {
    /// The methods of the vault-reaching block (`impl Books<'_>`), the only list of them.
    /// A ratchet (core-strangler Z6f): it shrinks, it never grows. A method that must go
    /// away is deleted together with its entry here.
    const FROZEN_VAULT_READS: &[&str] = &[
        "prepare_transaction",
        "earmarked",
        "keys_on_device",
        "device_secrets_for_request",
    ];

    /// Names of the `pub`/`pub(crate)` fns declared in the block that opens with `header`.
    fn method_names(source: &str, header: &str) -> Vec<String> {
        let block = source
            .split_once(header)
            .map(|(_, rest)| rest)
            .unwrap_or_else(|| panic!("`{header}` is gone from books.rs"));
        let block = block.split_once("\n}\n").map_or(block, |(body, _)| body);
        block
            .lines()
            .filter_map(|line| {
                let rest = line
                    .strip_prefix("    pub fn ")
                    .or_else(|| line.strip_prefix("    pub fn "))?;
                let end = rest.find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))?;
                Some(rest[..end].to_string())
            })
            .collect()
    }

    fn unfrozen_and_stale(source: &str) -> (Vec<String>, Vec<String>) {
        let found = method_names(source, "impl Books<'_> {");
        let unfrozen = found
            .iter()
            .filter(|name| !FROZEN_VAULT_READS.contains(&name.as_str()))
            .cloned()
            .collect();
        let stale = FROZEN_VAULT_READS
            .iter()
            .filter(|name| !found.iter().any(|found| found == *name))
            .map(|name| name.to_string())
            .collect();
        (unfrozen, stale)
    }

    #[test]
    fn books_gains_no_vault_read_beyond_the_frozen_list() {
        let (unfrozen, stale) = unfrozen_and_stale(include_str!("books.rs"));
        assert!(
            unfrozen.is_empty(),
            "Books must not grow: new method(s) {unfrozen:?}; take `&Books` in the callee instead (Z6f)"
        );
        assert!(
            stale.is_empty(),
            "FROZEN_VAULT_READS lists removed method(s) {stale:?}; delete the entries"
        );
    }

    #[test]
    fn the_ratchet_names_a_new_method_and_accepts_a_removal() {
        let source = include_str!("books.rs");
        let added = source.replacen(
            "impl Books<'_> {",
            "impl Books<'_> {\n    pub fn dummy_read(&self) {}",
            1,
        );
        assert_eq!(unfrozen_and_stale(&added).0, vec!["dummy_read".to_string()]);
        let removed = source.replace("    pub fn earmarked(&self", "    fn earmarked(&self");
        assert_eq!(
            unfrozen_and_stale(&removed).1,
            vec!["earmarked".to_string()]
        );
    }
}
