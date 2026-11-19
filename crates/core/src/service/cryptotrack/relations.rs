//! Shim of the `cryptotrack/relations` area: the tallies moved to `crate::cryptotrack::service::relations`,
//! the read to `crate::usecases::cryptotrack::relations` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::cryptotrack::relations::{CryptoRelations, CryptoRelationsInput};

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Everything the account dealt with, read from its events. An unknown
        /// account is `NOT_FOUND`; the hidden operations and the module's own
        /// entries are left out, as the list leaves them out by default.
        pub fn crypto_relations(&self, input: &CryptoRelationsInput) -> Result<CryptoRelations>
            => crate::usecases::cryptotrack::relations::crypto_relations @books;
    }
    // forward:end
}
