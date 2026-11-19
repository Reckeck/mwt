//! Shim of the `enrichment` area: moved to `crate::money::enrichment` (spec
//! core-strangler, D1). The scenarios are in `usecases::money::enrichment`.

use crate::error::Result;
use crate::model::chain::Chain;
use crate::model::enrichment::EnrichmentRecord;
use crate::txinfo::TxInfo;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The hash and network to look up for a transaction, if it has a hash.
        pub fn enrichment_request(&self, tx_id: &str) -> Option<(String, Option<Chain>)>
            => crate::money::enrichment::enrichment_request @doc;
        /// Records for one transaction, or all of them, newest first.
        pub fn enrichments(&self, tx_id: Option<&str>) -> Vec<EnrichmentRecord>
            => crate::money::enrichment::enrichments @doc;
        /// Keep what the explorer found as a pending record (replacing an older
        /// pending one). `None` when there is nothing worth showing. Does not
        /// persist: the caller writes once.
        pub fn apply_enrichment_result(&mut self, tx_id: &str, info: &TxInfo) -> Option<EnrichmentRecord>
            => crate::usecases::money::enrichment::apply_enrichment_result;
        /// Merge a finding into its transaction — the network fee, when the
        /// operation has none yet — through the usual checks.
        pub fn apply_enrichment(&mut self, id: &str) -> Result<EnrichmentRecord>
            => crate::usecases::money::enrichment::apply_enrichment;
        /// Dismiss a finding; the transaction stays as it is.
        pub fn reject_enrichment(&mut self, id: &str) -> Result<EnrichmentRecord>
            => crate::usecases::money::enrichment::reject_enrichment;
    }
    // forward:end
}
