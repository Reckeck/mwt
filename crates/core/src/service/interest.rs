//! Shim of the `interest` area: moved to `crate::money::interest` (spec core-strangler, D1).
//! The scenarios that write terms, key rates and interest live in
//! `usecases::money::interest`.

pub(crate) use crate::money::interest::INTEREST_CATEGORY;
pub use crate::money::interest::{DepositReport, InterestTaxReport, PdsReport};

use chrono::NaiveDate;

use crate::Amount;
use crate::error::Result;
use crate::model::deposit::{DepositTerms, KeyRatePoint};
use crate::model::ids::StoreId;
use crate::model::store::{AssetRef, Store};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Set or clear the terms of a savings account, deposit or ПДС.
        pub fn set_deposit_terms(&mut self, store_id: &str, terms: Option<DepositTerms>) -> Result<Store>
            => crate::usecases::money::interest::set_deposit_terms with (store_id => &StoreId::from(store_id));
        /// The account at a glance: rate, earned so far, what is ahead.
        pub fn deposit_report(&self, store_id: &str, today: NaiveDate) -> Result<DepositReport>
            => crate::money::interest::deposit_report @books with (store_id => &StoreId::from(store_id));
        /// Merge fetched key-rate points into the cache. Does not persist.
        pub fn apply_key_rates(&mut self, fresh: &[KeyRatePoint])
            => crate::usecases::money::interest::apply_key_rates;
        /// The key rate in force on `day`.
        pub fn key_rate_on(&self, day: NaiveDate) -> Option<Amount>
            => crate::money::interest::key_rate_on @doc;
        /// НДФЛ on deposit interest received in `year`, in rubles: 13 % of what
        /// exceeds 1 000 000 ₽ × the highest key rate on the 1st of a month.
        pub fn interest_tax(&self, year: i32) -> InterestTaxReport
            => crate::money::interest::interest_tax @books;
        /// Book last month's interest on every account that asks for it, once.
        /// Only the month just ended is posted: older months may already be in
        /// the ledger by hand, and guessing them would count them twice.
        pub fn post_due_interest(&mut self, today: NaiveDate) -> Result<u32>
            => crate::usecases::money::interest::post_due_interest;
        /// The money an account holds: its bound currency, else the base one.
        pub(crate) fn deposit_asset(&self, store: &Store) -> AssetRef
            => crate::money::interest::deposit_asset @books;
    }
    // forward:end
}
