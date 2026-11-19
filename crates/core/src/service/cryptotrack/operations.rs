//! Shim of the `cryptotrack/operations` area: the rules and the reads moved to
//! `crate::cryptotrack::service::operations`, the scenario to
//! `crate::usecases::cryptotrack::operations` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::cryptotrack::operations::{
    CryptoListOperationsInput, CryptoOperationDetail, CryptoOperationDetailInput, CryptoOpsPage,
    CryptoSetOverrideInput, CryptoSetOverrideResult,
};

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// One page of the operations, newest first, narrowed by the filter.
        /// Filtering and paging happen here, so a long history never travels
        /// whole to the screen.
        pub fn crypto_list_operations(&self, input: &CryptoListOperationsInput) -> CryptoOpsPage
            => crate::usecases::cryptotrack::operations::crypto_list_operations @books;
        /// One operation with its chain and, when asked, what could have paid
        /// for it.
        pub fn crypto_operation_detail(&self, input: &CryptoOperationDetailInput) -> Result<CryptoOperationDetail>
            => crate::usecases::cryptotrack::operations::crypto_operation_detail @books;
        /// Lay a correction over an operation (see [`CryptoSetOverrideInput`]).
        /// The event is left as it came; the correction survives a refetch.
        pub fn crypto_set_override(&mut self, input: &CryptoSetOverrideInput) -> Result<CryptoSetOverrideResult>
            => crate::usecases::cryptotrack::operations::crypto_set_override;
    }
    // forward:end
}

#[cfg(test)]
mod tests;
