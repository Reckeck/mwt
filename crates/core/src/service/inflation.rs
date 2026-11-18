//! Shim of the `inflation` area: moved to `crate::money::inflation` (spec core-strangler, D1).
//! The scenarios and the notices live in `usecases::money::inflation`.

pub use crate::money::inflation::{
    FetchedRates, INFLATION_CHART_MONTHS, InflationCurrency, InflationNews,
};
pub use crate::usecases::money::inflation::{inflation_notice, key_rate_notice};

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::Amount;
use crate::error::Result;
use crate::model::inflation::{InflationMode, InflationReading};
use crate::model::notify::NoticeRecord;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Readings of a currency, oldest first: the fetched ones, or in source
        /// mode the ones read from the user's data source.
        pub fn inflation_series(&self, currency: &str) -> Vec<InflationReading>
            => crate::money::inflation::inflation_series @doc;
        /// The flat annual inflation of the group, percent by currency: the shipped
        /// table with the vault's own rates over it. The only source of flat rates
        /// (screens, reports, workflow nodes, the monthly checkup), so every device
        /// of the group computes with one table.
        pub fn flat_inflation(&self) -> BTreeMap<String, Amount>
            => crate::money::inflation::flat_inflation @doc;
        /// Copy the rates this device had in its app config into the vault, once
        /// (`Settings.inflation_adopted`): only values that differ from the shipped
        /// table, and never over a rate the group already has. Returns whether
        /// anything was written. Two devices that adopt different values of one
        /// currency settle on the later write when they sync.
        pub fn adopt_flat_inflation(&mut self, local: &BTreeMap<String, Amount>) -> Result<bool>
            => crate::usecases::money::inflation::adopt_flat_inflation;
        /// The mode the user chose, else: no built-in source → manual; a flat rate
        /// the user changed from the shipped default → manual (they meant it);
        /// otherwise automatic.
        pub fn inflation_mode(&self, currency: &str) -> InflationMode
            => crate::money::inflation::inflation_mode @doc;
        /// Inflation of a currency in a month (`YYYY-MM`), percent: the reading of
        /// that month or the latest one before it in auto mode, else the flat
        /// rate. A month before the first reading takes the first one.
        pub fn inflation_on(&self, currency: &str, month: &str) -> Option<Amount>
            => crate::money::inflation::inflation_on @doc;
        /// Latest inflation per currency as reports and the calculator use it:
        /// every currency with a flat rate or a built-in source.
        pub fn effective_inflation(&self) -> BTreeMap<String, Amount>
            => crate::money::inflation::effective_inflation @doc;
        /// Monthly coefficients from `from` to `today` by the series, or `None`
        /// when the currency follows no series.
        pub(crate) fn real_coefficients(&self, currency: &str, from: NaiveDate, today: NaiveDate) -> Option<BTreeMap<NaiveDate, Amount>>
            => crate::money::inflation::real_coefficients @doc;
        /// Merge fetched readings: a revised month replaces the old value. Returns
        /// the months newer than what each currency had (for the notice). Does
        /// not persist.
        pub fn apply_inflation(&mut self, fresh: &[InflationReading]) -> Vec<InflationNews>
            => crate::usecases::money::inflation::apply_inflation;
        /// Merge a background fetch, persist, and return the notices it calls
        /// for (not yet logged): new inflation months and key-rate decisions
        /// that were not known before. The first fetch fills history silently.
        pub fn apply_rates(&mut self, fetched: &FetchedRates, ru: bool) -> Result<Vec<NoticeRecord>>
            => crate::usecases::money::inflation::apply_rates;
        /// Choose where a currency's inflation comes from; `None` returns it to
        /// the inferred default.
        pub fn set_inflation_mode(&mut self, currency: &str, mode: Option<InflationMode>) -> Result<()>
            => crate::usecases::money::inflation::set_inflation_mode;
        /// Read a currency's inflation from one of the user's data sources.
        pub fn set_inflation_source(&mut self, currency: &str, source_id: &str) -> Result<()>
            => crate::usecases::money::inflation::set_inflation_source;
        /// Every currency with a flat rate or a built-in source, for the settings
        /// screen.
        pub fn inflation_overview(&self) -> Vec<InflationCurrency>
            => crate::money::inflation::inflation_overview @doc;
    }
    // forward:end
}
