//! Shim of the `valuation` area: moved to `crate::money::valuation` (spec core-strangler, D1).
//! The scenarios that write valuations and the revalue rule live in
//! `usecases::money::valuation`.

pub use crate::money::valuation::{
    PropertyPurchase, PropertyValue, ValuationBaseKind, ValuationInput,
};
pub(crate) use crate::money::valuation::{ValuationCurve, initial_rule};

use std::collections::HashMap;

use crate::error::Result;
use crate::model::ids::StoreId;
use crate::model::store::RevalueRule;
use crate::model::vault::Valuation;

use super::UnlockedVault;
use super::forward::forward;
use super::sync_ids::valuation_id;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Curves of every property asset that has a valuation or a rule.
        pub(crate) fn valuation_curves(&self) -> HashMap<String, ValuationCurve>
            => crate::money::valuation::valuation_curves @books;
        /// The curve of one asset, if it has one.
        pub(crate) fn valuation_curve(&self, asset_key: &str) -> Option<ValuationCurve>
            => crate::money::valuation::valuation_curve @books;
        /// The value story of each asset a property holds.
        pub fn property_values(&self, store_id: &str, zone: &str) -> Result<Vec<PropertyValue>>
            => crate::money::valuation::property_values @books with (store_id => &StoreId::from(store_id));
        /// Add a dated valuation; one on the same day replaces it.
        pub fn add_valuation(&mut self, input: ValuationInput) -> Result<Valuation>
            => crate::usecases::money::valuation::add_valuation;
        /// Remove a valuation; returns it so the UI can put it back.
        pub fn delete_valuation(&mut self, id: &str) -> Result<Valuation>
            => crate::usecases::money::valuation::delete_valuation;
        /// Set or switch off a property's rule. Switching it on where nothing was
        /// valued yet first values every asset at today's price, so the value
        /// continues from where it is instead of jumping (plan §11.4).
        pub fn set_revalue_rule(&mut self, store_id: &str, rule: Option<RevalueRule>, zone: &str) -> Result<()>
            => crate::usecases::money::valuation::set_revalue_rule with (store_id => &StoreId::from(store_id));
    }
    // forward:end
}

impl UnlockedVault {
    /// The record of a planned valuation, under its deterministic sync id
    /// (the id rule lives with the sync ids, which this shim may reach).
    pub(crate) fn valuation_record(input: ValuationInput) -> Valuation {
        Valuation {
            id: valuation_id(&input.asset_key, &input.date),
            store_id: input.store_id,
            asset_key: input.asset_key,
            date: input.date,
            unit_price: input.unit_price,
            currency: input.currency,
        }
    }
}
