//! The portfolio scenarios. The holdings are folded from the transactions in
//! `money::portfolio::compute_holdings`; this file lays the result into the
//! document, between the two steps that still live with their own areas.

use crate::Amount;
use crate::appconfig::DEFAULT_BASE_CURRENCY;
use crate::money::portfolio::{base_and_fx as base_and_fx_of, compute_holdings};
use crate::service::Session;

/// Re-derive `doc.holdings` from the transactions. A correction keeps the real
/// balance, so the adjustments are rebased first; the crypto module's balances
/// (decision P5) are laid over the result last. Saves nothing: the caller that
/// changed the transactions persists.
pub fn recompute_holdings(vault: &mut Session) {
    crate::usecases::money::adjustments::rebase_adjustments(vault);
    let holdings = compute_holdings(&vault.books());
    vault.doc.holdings = holdings;
    crate::usecases::cryptotrack::recompute::apply_crypto_holdings(vault);
}

/// Base currency code (never empty, lower case) and its latest cached FX
/// rate; an empty setting means the default.
pub(crate) fn base_and_fx(vault: &Session) -> (String, Amount) {
    let code = vault.base_currency().to_ascii_lowercase();
    let code = if code.is_empty() {
        DEFAULT_BASE_CURRENCY.to_string()
    } else {
        code
    };
    base_and_fx_of(&vault.doc, &code)
}
