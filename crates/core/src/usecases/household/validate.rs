//! Scenarios of the validate area: the pure rules of `money::validate` plus
//! what they cannot hold. The wall clock (V11) and the hash shapes of
//! `txinfo`, a sibling group of `money`, join them here.

use crate::error::{Error, Result};
use crate::model::transaction::Transaction;
use crate::model::vault::{iso_to_millis, now_iso};
use crate::money::validate::validate_transaction_at as check_rules;
use crate::service::UnlockedVault;
use crate::txinfo::{HashShape, hash_shape};

/// Plan §4.5, V1–V14 (V13 — negative balances — is a warning, not an error).
pub fn validate_transaction(vault: &UnlockedVault, tx: &Transaction) -> Result<()> {
    validate_transaction_at(vault, tx, iso_to_millis(&now_iso()))
}

/// The same rules, with V11 measured from `now_ms` instead of the wall
/// clock: an automation run is checked against the scheduler's clock,
/// which runs ahead of it in a simulation.
pub fn validate_transaction_at(vault: &UnlockedVault, tx: &Transaction, now_ms: i64) -> Result<()> {
    check_rules(&vault.doc, tx, now_ms)?;
    check_on_chain_refs(tx)
}

/// Shape-check every on-chain hash a transaction claims. A hash whose
/// network differs from the assets it moved would open the explorer on
/// somebody else's transaction, so both are refused.
fn check_on_chain_refs(tx: &Transaction) -> Result<()> {
    let chains = tx.asset_chains();
    for (network, hash) in tx.on_chain_refs() {
        let Some(hash) = hash.filter(|raw| !raw.is_empty()) else {
            continue;
        };
        if hash_shape(hash) == HashShape::Malformed {
            return Err(Error::TxHashMalformed(hash.to_string()));
        }
        if let Some(network) = network
            && chains.iter().flatten().any(|chain| chain != &network)
        {
            return Err(Error::TxHashNetworkMismatch(format!(
                "network {network:?} does not match the assets moved"
            )));
        }
    }
    Ok(())
}
