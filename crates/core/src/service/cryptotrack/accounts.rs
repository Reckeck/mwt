//! Shim of the `cryptotrack/accounts` area: the rules moved to `crate::cryptotrack::service::accounts`,
//! the scenarios to `crate::usecases::cryptotrack::accounts` (spec core-strangler, D1).

use crate::error::Result;
use crate::ipc_contract::cryptotrack::accounts::{
    ConnectAddressInput, ConnectAddressResult, CryptoAccountView, UpdateAccountInput,
};

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The followed accounts, each with the wallet that shows it and the
        /// tokens of it that wait in quarantine.
        pub fn crypto_list_accounts(&self) -> Vec<CryptoAccountView>
            => crate::usecases::cryptotrack::accounts::crypto_list_accounts @books;
        /// Follow a public address: a wallet of kind `Crypto` and its account,
        /// made together. Nothing is stored when anything is refused.
        pub fn crypto_connect_address(&mut self, input: &ConnectAddressInput) -> Result<ConnectAddressResult>
            => crate::usecases::cryptotrack::accounts::crypto_connect_address;
        /// Rename an account, change its note, or stop counting it (archive) and
        /// start again.
        pub fn crypto_update_account(&mut self, input: &UpdateAccountInput) -> Result<()>
            => crate::usecases::cryptotrack::accounts::crypto_update_account;
        /// Stop following an address: the account, its read positions and its
        /// cursors go, the operations already read stay until the module is wiped
        /// (spec E12). The wallet that showed it is archived, because nothing drives
        /// it any more and it must not turn into a hand-kept wallet.
        pub fn crypto_forget_account(&mut self, account_id: &str) -> Result<()>
            => crate::usecases::cryptotrack::accounts::crypto_forget_account;
    }
    // forward:end
}
