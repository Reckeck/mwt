//! Demo data for the sandbox: twelve months of everyday money, as the `year`
//! scenario of the browser mock shows it. A salary of about 1 500–1 650 $,
//! thirteen-odd categories, a December of gifts, an August trip that puts the
//! month in the red, and wallets of several kinds.
//!
//! Deterministic: a fixed seed, fixed ids, and the day «today» comes in as a
//! parameter, so two runs for the same day give the same document.

mod ids;
mod ledger;
mod plan;
mod rng;
mod spends;
mod wallets;

use std::sync::Arc;

use chrono::NaiveDate;

use crate::Result;
use crate::service::{SaveHook, UnlockedVault};

/// The seed of the generator; changing it changes every amount.
const SEED: u64 = 20_261_023;

/// Stands in for the app's writer: the seed is written once, at the end.
struct Deferred;

impl SaveHook for Deferred {
    fn dirty(&self) {}
}

/// Fill a new (empty) vault with the demo wallets and twelve months up to `today`.
pub fn fill(vault: &mut UnlockedVault, today: NaiveDate) -> Result<()> {
    vault.set_write_behind(Some(Arc::new(Deferred)));
    let result = book(vault, today);
    vault.set_write_behind(None);
    result?;
    vault.persist_now()
}

fn book(vault: &mut UnlockedVault, today: NaiveDate) -> Result<()> {
    vault.doc.id = ids::vault();
    vault.doc.created_at = format!("{}T00:00:00Z", today - chrono::Duration::days(365));
    vault.doc.settings.crypto = Some(true);
    let wallets = wallets::create_all(vault)?;
    for (number, planned) in ledger::plan(&wallets, today, SEED).into_iter().enumerate() {
        vault.add_transaction(planned.into_transaction(number as u128))?;
    }
    fix_holdings(vault);
    plan::add(vault, &wallets, today)?;
    Ok(())
}

/// The holdings are worked out from the history in hash order with random
/// ids; put them in a fixed order with fixed ids.
fn fix_holdings(vault: &mut UnlockedVault) {
    vault
        .doc
        .holdings
        .sort_by_key(|holding| (holding.store_id.clone(), holding.asset.key()));
    for (number, holding) in vault.doc.holdings.iter_mut().enumerate() {
        holding.id = ids::holding(number as u128);
    }
}
