//! The read context of one request: the rates at a moment, built from the
//! open vault the first time something asks and then shared (D4, OD7).
//! A request that needs the book more than once takes one `Books` and passes
//! it on, instead of building the book again for every step.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Result;
use crate::model::store::{AssetRef, Store, StoreScope};
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;
use crate::money::shared_ratios::{SharedExtra, shared_extra_flows, shared_payment_ratios};
use crate::vault_io::device_secrets::DeviceSecrets;

use crate::service::budget::{Flow, MonthAllowance, MonthAvg};
use crate::service::losses::LossReport;
use crate::service::reports::Ctx;
use crate::service::{
    ContactBalance, DailyBudget, DebtLoad, HealthReport, MonthSummary, NetWorth, RateBook,
    StoreOverview, UnlockedVault, UpcomingItem, UtilityBill,
};

#[cfg(test)]
thread_local! {
    /// How many books this thread built (the counter of `books_build_each_field_once`).
    pub(crate) static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Lazy read context over an open vault.
pub struct Books<'vault> {
    vault: &'vault UnlockedVault,
    rate_book: OnceCell<RateBook>,
    shared_ratios: OnceCell<HashMap<String, Amount>>,
    shared_extras: OnceCell<Vec<SharedExtra>>,
    base_and_fx: OnceCell<(String, Amount)>,
}

impl<'vault> Books<'vault> {
    pub(crate) fn new(vault: &'vault UnlockedVault) -> Self {
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
        &self.vault.doc
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
    pub(crate) fn build_rate_book(&self) -> RateBook {
        #[cfg(test)]
        BUILDS.with(|count| count.set(count.get() + 1));
        let curves = self
            .vault
            .valuation_curves()
            .into_iter()
            .map(|(key, curve)| (key, Arc::new(curve) as _))
            .collect();
        RateBook::build(&self.vault.doc, curves).with_base(self.base_and_fx().0)
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
    pub(crate) fn shared_extras(&self) -> &[SharedExtra] {
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

/// Reads that need the open vault itself: its security level, its shared
/// events, its envelopes. The areas ask the context; only the kernel holds
/// the vault (D2, OD6). Each one is the vault's own answer, unchanged.
impl Books<'_> {
    /// The writers' checks of a transaction (dates, envelopes, ownership).
    pub(crate) fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()> {
        self.vault.prepare_transaction(tx)
    }

    /// The part of the asset in the store that envelopes claim.
    pub(crate) fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount {
        self.vault.earmarked(store_id, asset_key)
    }

    /// Whether the exchange keys live on this device (security level 3).
    pub(crate) fn keys_on_device(&self) -> bool {
        self.vault.keys_on_device()
    }

    /// The whole device secrets file for one request (the caller drops it soon).
    pub(crate) fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        self.vault.device_secrets_for_request()
    }

    /// The loyalty programme shares its symbol with another one.
    pub(crate) fn loyalty_symbol_clash(&self, store_id: &str) -> bool {
        self.vault.loyalty_symbol_clash(store_id)
    }

    /// Today's valuation.
    pub(crate) fn net_worth(&self) -> NetWorth {
        self.vault.net_worth()
    }

    /// Today's valuation plus the part of `financial` held in crypto.
    pub(crate) fn net_worth_with_crypto(&self) -> (NetWorth, Amount) {
        self.vault.net_worth_with_crypto()
    }

    /// USD value of one unit of a hand-priced asset.
    pub(crate) fn manual_price_usd(&self, asset_key: &str) -> Option<Amount> {
        self.vault.manual_price_usd(asset_key)
    }

    /// Every store id a scope covers.
    pub(crate) fn resolve_scope(&self, scope: &StoreScope) -> HashSet<String> {
        self.vault.resolve_scope(scope)
    }

    /// The store and all stores below it.
    pub(crate) fn subtree_ids(&self, root_id: &str) -> Vec<String> {
        self.vault.subtree_ids(root_id)
    }

    /// One store with its positions and values.
    pub(crate) fn store_overview(
        &self,
        store_id: &str,
        include_children: bool,
    ) -> Result<StoreOverview> {
        self.vault.store_overview(store_id, include_children)
    }

    /// The key rate on `day`.
    pub(crate) fn key_rate_on(&self, day: NaiveDate) -> Option<Amount> {
        self.vault.key_rate_on(day)
    }

    /// The asset a deposit store earns its interest in.
    pub(crate) fn deposit_asset(&self, store: &Store) -> AssetRef {
        self.vault.deposit_asset(store)
    }

    /// Inflation of `currency` in `month`.
    pub(crate) fn inflation_on(&self, currency: &str, month: &str) -> Option<Amount> {
        self.vault.inflation_on(currency, month)
    }

    /// The budget of one month over this context.
    pub(crate) fn month_summary_in(&self, month: &str, zone: &str) -> Result<MonthSummary> {
        self.vault.month_summary_in(self, month, zone)
    }

    /// The average income and essential spending over finished months.
    pub(crate) fn income_basis(&self, months: usize, now: i64, zone: &str) -> Option<MonthAvg> {
        self.vault.income_basis(months, now, zone)
    }

    /// The budget flows of a period.
    pub(crate) fn flows_between(
        &self,
        cal: &Calendar,
        currency: &str,
        from: i64,
        to: i64,
    ) -> Vec<Flow> {
        self.vault.flows_between(cal, currency, from, to)
    }

    /// Planned income and fixed spending of the month containing `date`.
    pub(crate) fn month_plan(
        &self,
        cal: &Calendar,
        date: NaiveDate,
        currency: &str,
    ) -> (Amount, Amount) {
        self.vault.month_plan(cal, date, currency)
    }

    /// The month-mode allowance of the month containing `date`.
    pub(crate) fn month_allowance(
        &self,
        cal: &Calendar,
        date: NaiveDate,
        currency: &str,
        flows: &[Flow],
    ) -> MonthAllowance {
        self.vault.month_allowance(cal, date, currency, flows)
    }

    /// How much can be spent today.
    pub(crate) fn daily_budget(&self, now: i64, zone: &str) -> DailyBudget {
        self.vault.daily_budget(now, zone)
    }

    /// What is due in the next `days` days.
    pub(crate) fn upcoming(&self, days: u32, now: i64, zone: &str) -> Vec<UpcomingItem> {
        self.vault.upcoming(days, now, zone)
    }

    /// Losses by month and by kind, wallet, contact and chain.
    pub(crate) fn loss_report(&self, ctx: &Ctx) -> LossReport {
        self.vault.loss_report(ctx)
    }

    /// The utility bill of a property for a period.
    pub(crate) fn utility_bill(
        &self,
        property_id: &str,
        period: &str,
        zone: &str,
    ) -> Result<UtilityBill> {
        self.vault.utility_bill(property_id, period, zone)
    }

    /// The capital health report.
    pub(crate) fn capital_health(&self, now: i64, zone: &str) -> Result<HealthReport> {
        self.vault.capital_health(now, zone)
    }

    /// Debts against income.
    pub(crate) fn debt_load(&self, now: i64, zone: &str) -> Result<DebtLoad> {
        self.vault.debt_load(now, zone)
    }

    /// Personal debts and shared events as of `secs`.
    pub(crate) fn contact_balances_until(&self, secs: i64) -> Vec<ContactBalance> {
        self.vault.contact_balances_until(secs)
    }

    /// What the crypto module was worth at each cutoff in USD and whether it
    /// already counts a replayed wallet's coins.
    pub(crate) fn crypto_module(
        &self,
        cutoffs: &[i64],
        in_scope: impl Fn(&str) -> bool,
    ) -> (Vec<Option<Amount>>, impl Fn(&str, &AssetRef) -> bool) {
        crate::service::reports::crypto_module(self.vault, cutoffs, in_scope)
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

    #[test]
    fn books_base_and_fx_equals_vault() {
        let (_dir, vault) = open_vault();
        let books = vault.books();
        let (vault_base, vault_fx) = vault.base_and_fx();
        assert_eq!(books.base_and_fx(), (vault_base.as_str(), vault_fx));
        assert_eq!(books.price_usd("fiat:usd"), vault.price_usd_for("fiat:usd"));
        let fx = vault.base_and_fx().1;
        assert_eq!(
            books.price_base("coin:bitcoin"),
            vault.price_base_for("coin:bitcoin", fx)
        );
        assert!(std::ptr::eq(books.doc(), &vault.doc));
    }
}
