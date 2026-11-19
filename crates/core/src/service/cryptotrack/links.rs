//! Shim of the `cryptotrack/links` area: the rules moved to `crate::cryptotrack::service::links`,
//! the scenarios to `crate::usecases::cryptotrack::links` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::cryptotrack::links::{
    CryptoLinkCandidatesInput, CryptoLinkInput, CryptoLinkOption, CryptoPair, CryptoPairsInput,
    CryptoUnlinkInput,
};

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Crypto operations that look like the same movement of money as a fiat
        /// record: close in time, the ones whose value fits first. Only a
        /// suggestion, nothing is written.
        pub fn crypto_link_candidates(&self, input: &CryptoLinkCandidatesInput) -> Vec<CryptoLinkOption>
            => crate::usecases::cryptotrack::links::crypto_link_candidates @books;
        /// Point a crypto operation at a fiat record. Both must exist now.
        pub fn crypto_link_fiat(&mut self, input: &CryptoLinkInput) -> Result<()>
            => crate::usecases::cryptotrack::links::crypto_link_fiat;
        /// Remove the pointer. Nothing to remove is not an error.
        pub fn crypto_unlink_fiat(&mut self, input: &CryptoUnlinkInput) -> Result<()>
            => crate::usecases::cryptotrack::links::crypto_unlink_fiat;
        /// The pointers of one operation or one fiat record, both ends resolved.
        /// A vault without crypto data has none, so the fiat form shows nothing.
        pub fn crypto_pairs(&self, input: &CryptoPairsInput) -> Vec<CryptoPair>
            => crate::usecases::cryptotrack::links::crypto_pairs @books;
    }
    // forward:end
}
