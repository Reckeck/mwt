//! Pointing a fiat record at a crypto operation and back (spec E13): which
//! operations look like the same movement of money, and what a pointer change
//! does to the stored corrections.
//!
//! The pointer is [`CryptoOverride::fiat_tx_id`]. It is a visible reference
//! and nothing else: nothing here touches `flow_id`, a chain, a cost basis or
//! a sum, and nothing recomputes the holdings. The chain of money is written
//! by `crypto_set_override` (spec E10), and only there. Both ends may vanish
//! later (a deleted record, a wiped module); reading a pointer whose end is
//! gone gives `None` for that end, never an error.

use chrono::NaiveDate;

use crate::Amount;
use crate::cryptotrack::engine::{ids, math, overrides};
use crate::error::{Error, Result};
use crate::model::cryptotrack::CryptoOverride;
use crate::model::vault::VaultDoc;

/// How far apart the two sides may be dated, in days.
const WINDOW_DAYS: i64 = 3;
/// How far the fiat sum may be from the coin's value, in percent.
const TOLERANCE_PERCENT: u32 = 5;
/// The most suggestions offered.
pub const OPTION_CAP: usize = 40;

/// What setting a pointer asks the stored corrections to do.
#[derive(Debug)]
pub enum LinkPlan {
    /// The same pointer stands already: nothing to write.
    Unchanged,
    /// The operation has no correction yet: this one is added.
    Insert(CryptoOverride),
    /// The correction with this id points elsewhere now.
    Repoint(String),
}

/// The calendar day of an RFC 3339 moment.
pub fn day_of(time: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(time.get(..10)?, "%Y-%m-%d").ok()
}

/// The operations dated within the window of `day`, as `(days apart, id)`: not
/// the module's own entries and not the hidden ones.
pub fn nearby_operations(doc: &VaultDoc, day: NaiveDate) -> Vec<(u32, &str)> {
    let fixes = overrides::index(&doc.crypto_overrides);
    let mut found = Vec::new();
    for event in &doc.crypto_events {
        if ids::adjustment(&event.id).is_some()
            || fixes.get(event.id.as_str()).is_some_and(|fix| fix.hidden)
        {
            continue;
        }
        let Some(other) = day_of(&event.time) else {
            continue;
        };
        let days = (other - day).num_days().unsigned_abs();
        if days as i64 > WINDOW_DAYS {
            continue;
        }
        found.push((days as u32, event.id.as_str()));
    }
    found
}

/// Whether a fiat sum in USD is within the tolerance of a coin's value in USD.
/// Values come from the network and from prices: one a decimal cannot hold is
/// not a fit, and is not a panic either.
pub fn within_tolerance(usd: Amount, value: Amount) -> bool {
    let spread = math::sub(usd, value).and_then(|diff| math::mul(diff.abs(), Amount::from(100)));
    let allowed = math::mul(value, Amount::from(TOLERANCE_PERCENT));
    matches!((spread, allowed), (Some(spread), Some(allowed)) if spread <= allowed)
}

/// What pointing the operation at the fiat record changes. Both must exist
/// now. `now` stamps a correction that is made or moved.
pub fn plan_link(doc: &VaultDoc, event_id: &str, fiat_tx_id: &str, now: &str) -> Result<LinkPlan> {
    if !doc.crypto_events.iter().any(|event| event.id == event_id) {
        return Err(Error::NotFound(format!("operation {event_id}")));
    }
    if !doc.transactions.iter().any(|tx| tx.meta().id == fiat_tx_id) {
        return Err(Error::NotFound(format!("transaction {fiat_tx_id}")));
    }
    Ok(
        match overrides::index(&doc.crypto_overrides).get(event_id) {
            Some(fix) if fix.fiat_tx_id.as_deref() == Some(fiat_tx_id) => LinkPlan::Unchanged,
            Some(fix) => LinkPlan::Repoint(fix.id.clone()),
            None => LinkPlan::Insert(CryptoOverride {
                id: format!("cov:{event_id}"),
                event_id: event_id.to_string(),
                kind: None,
                comment: None,
                categories: Vec::new(),
                flow_id: None,
                contact_id: None,
                not_exchange: false,
                hidden: false,
                fiat_tx_id: Some(fiat_tx_id.to_string()),
                updated_at: now.to_string(),
            }),
        },
    )
}

/// The id of the correction that holds a pointer of this operation, if one
/// does: the one to clear.
pub fn pointer_of(doc: &VaultDoc, event_id: &str) -> Option<String> {
    overrides::index(&doc.crypto_overrides)
        .get(event_id)
        .filter(|fix| fix.fiat_tx_id.is_some())
        .map(|fix| fix.id.clone())
}
