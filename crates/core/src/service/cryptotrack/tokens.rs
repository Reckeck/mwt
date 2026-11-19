//! Shim of the `cryptotrack/tokens` area: the rules moved to `crate::cryptotrack::service::tokens`,
//! the scenarios to `crate::usecases::cryptotrack::tokens` (spec core-strangler, D1).

use crate::Amount;
use crate::cryptotrack::protocols::LegToken;
use crate::cryptotrack::tokens::TokenStatus;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::tokens::TokenView;
use crate::model::cryptotrack::TokenId;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Note a token met in an event. A new contract starts in quarantine
        /// (unless it is on the built-in list); a known record gets its text
        /// refreshed, sanitized. Returns the status after the call.
        pub fn crypto_register_token(&mut self, token: &TokenId, symbol: &str, name: &str, decimals: u8) -> Result<TokenStatus>
            => crate::usecases::cryptotrack::tokens::crypto_register_token;
        /// The tokens met so far, each with its status, its basis and what is
        /// suspicious about its text.
        pub fn crypto_tokens(&self) -> Vec<TokenView>
            => crate::usecases::cryptotrack::tokens::crypto_tokens @doc;
        /// Take a token into the totals.
        pub fn crypto_confirm_token(&mut self, token: &TokenId) -> Result<TokenStatus>
            => crate::usecases::cryptotrack::tokens::crypto_confirm_token;
        /// Hide a token: no list, no total, no price request.
        pub fn crypto_reject_token(&mut self, token: &TokenId) -> Result<TokenStatus>
            => crate::usecases::cryptotrack::tokens::crypto_reject_token;
        /// Take back the user's verdict on a token: it is judged by the built-in
        /// list or the quarantine again, so a confirmed token leaves the totals
        /// and a hidden one comes back to the quarantine list. Clearing a token
        /// that has no verdict changes nothing.
        pub fn crypto_clear_token_verdict(&mut self, token: &TokenId) -> Result<TokenStatus>
            => crate::usecases::cryptotrack::tokens::crypto_clear_token_verdict;
        /// Set (or, with `None`, clear) what the old purchases of this token
        /// cost, USD per unit.
        pub fn crypto_set_token_basis(&mut self, token: &TokenId, basis_usd: Option<Amount>) -> Result<()>
            => crate::usecases::cryptotrack::tokens::crypto_set_token_basis;
    }
    // forward:end

    // Called by the recompute and the position reads until those areas move; not in the
    // baseline (crate-private), so outside the regenerated block.
    forward! {
        /// [`Self::crypto_register_token`] without the save: the caller owns it
        /// and saves once for however many tokens it noted. Returns the status
        /// and whether the document changed.
        pub(crate) fn register_token_unsaved(&mut self, token: &TokenId, symbol: &str, name: &str, decimals: u8) -> Result<(TokenStatus, bool)>
            => crate::usecases::cryptotrack::tokens::register_token_unsaved;
        /// Register a leg token of a position read by a built-in adapter as
        /// trusted. Returns whether the document changed; the caller saves.
        pub(crate) fn trust_adapter_token(&mut self, leg: &LegToken) -> Result<bool>
            => crate::usecases::cryptotrack::tokens::trust_adapter_token;
    }
}
