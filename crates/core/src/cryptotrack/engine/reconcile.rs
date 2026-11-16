//! The network has the last word: what the node says an address holds beats
//! what the operations add up to.
//!
//! The operations can be incomplete for honest reasons — the history is read
//! from a start date, a provider lost a page, a transfer arrived through a
//! contract the module cannot read — and a holding that disagrees with the
//! node is a wrong holding. So the difference is written down as an operation
//! of its own, tracked by the address ([`TrackingRef::Address`]) rather than
//! by a transaction that does not exist, and from then on the sum matches the
//! network.
//!
//! Two of these exist, and they are the same arithmetic with different
//! meanings:
//!
//! * the **opening** adjustment, dated on the account's start date: what the
//!   address already held before the module started following it. One per
//!   account and asset, for ever ([`super::ids::opening`]).
//! * a **difference** against a later reading, dated on the day it was read
//!   ([`super::ids::reconciled`]). It marks the asset's profit imprecise,
//!   because a quantity whose history is unknown has no honest cost.
//!
//! Both ids are made of the account, the asset and (for a difference) the day
//! of the reading, and of nothing else: no clock, no counter, no random part.
//! Two devices that read the same balance on the same day write one record
//! with the same bytes, and reading again the same day replaces it instead of
//! stacking a second difference on top of the first.
//!
//! **An adjustment is absolute, not a delta.** The adjustment already stored
//! is part of what the operations add up to, so the new one is written as
//! `what was there + what is still missing`; replacing it with the bare
//! difference would undo itself on the next pass. [`adjust`] is therefore
//! idempotent: running it twice on the same facts writes the same record.
//!
//! **A partial reading never takes a holding to zero.** A reading that does
//! not list every asset of the address ([`Reading::complete`]) corrects only
//! the assets it names; otherwise a balance call that happened to cover the
//! native coin alone would wipe every token.

use std::collections::{BTreeMap, BTreeSet};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, CryptoTxKind, TrackingRef,
};

use super::basis::Ledger;
use super::ids::{self, Adjustment};
use super::math;
use super::normalize::net_of;

/// What the network said one account holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading<'read> {
    pub account_id: &'read str,
    pub chain: Chain,
    /// The account's address, normalized.
    pub address: &'read str,
    /// What each asset's balance is, in whole units.
    pub balances: Vec<(CryptoAsset, Amount)>,
    /// Whether this lists every asset the address holds. A partial reading
    /// corrects only what it names.
    pub complete: bool,
}

/// What was made of a comparison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reconciled {
    /// Adjustment operations to store: an existing id is replaced, never
    /// added to.
    pub events: Vec<CryptoTx>,
    /// Asset keys whose quantity the network had to correct.
    pub corrected: BTreeSet<String>,
}

/// Compare a reading with what the operations add up to and write the
/// difference down.
///
/// `stored` is the module's events as they are now: the adjustments among
/// them are read back so the new record stays absolute.
pub fn adjust(
    reading: &Reading<'_>,
    ledger: &Ledger,
    stored: &[CryptoTx],
    kind: Adjustment,
    at: &str,
) -> Reconciled {
    let mut out = Reconciled::default();
    let mut network: BTreeMap<String, (CryptoAsset, Amount)> = BTreeMap::new();
    for (asset, quantity) in &reading.balances {
        let slot = network
            .entry(asset.key())
            .or_insert_with(|| (asset.clone(), Amount::ZERO));
        slot.1 = math::add(slot.1, *quantity).unwrap_or(slot.1);
    }
    // A complete reading also answers for what it does not list: the module
    // may be holding something the address no longer has.
    if reading.complete {
        for ((account_id, asset_key), held) in &ledger.balances {
            if account_id != reading.account_id || held.is_zero() {
                continue;
            }
            if let Some(state) = ledger.state(asset_key) {
                network
                    .entry(asset_key.clone())
                    .or_insert_with(|| (state.asset.clone(), Amount::ZERO));
            }
        }
    }

    for (asset_key, (asset, quantity)) in network {
        let computed = ledger.balance(reading.account_id, &asset_key);
        let Some(missing) = math::sub(quantity, computed) else {
            continue;
        };
        let id = match kind {
            Adjustment::Opening => ids::opening(reading.account_id, &asset_key),
            Adjustment::Reconciled => ids::reconciled(reading.account_id, &asset_key, at),
        };
        let before = stored
            .iter()
            .find(|event| event.id == id)
            .map(|event| net_of(event, &asset));
        let standing = match before {
            Some(had) => math::add(had, missing).unwrap_or(had),
            None => missing,
        };
        if missing.is_zero() && before.is_none() {
            continue;
        }
        out.corrected.insert(asset_key);
        out.events.push(CryptoTx {
            id,
            chain: reading.chain,
            account_id: reading.account_id.to_owned(),
            // Midnight of the day, not the hour it was computed: the id is a
            // function of the facts and so must the record be, or two devices
            // that reconciled the same day would hold different bytes for the
            // same correction. Midnight also puts the correction before that
            // day's own operations.
            time: format!("{}T00:00:00Z", ids::day(at)),
            block: None,
            from: String::new(),
            to: reading.address.to_owned(),
            legs: legs_of(&asset, standing),
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::Address {
                chain: reading.chain,
                address: reading.address.to_owned(),
            },
        });
    }
    out
}

/// The single leg an adjustment carries: inbound for what the module did not
/// see arrive, outbound for what it thought was there and is not. A zero
/// correction carries none, which is how a record that turned out to be
/// unnecessary is emptied instead of left standing.
fn legs_of(asset: &CryptoAsset, quantity: Amount) -> Vec<CryptoLeg> {
    if quantity.is_zero() {
        return Vec::new();
    }
    vec![CryptoLeg {
        direction: if quantity.is_negative() {
            CryptoDirection::Out
        } else {
            CryptoDirection::In
        },
        asset: asset.clone(),
        amount: quantity.abs(),
        counterparty: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cryptotrack::engine::basis::{Doubt, Inputs, Prices, Quote, fold};
    use crate::cryptotrack::engine::overrides;
    use crate::model::cryptotrack::TokenId;

    const ME: &str = "0x1111111111111111111111111111111111111111";
    const OTHER: &str = "0x2222222222222222222222222222222222222222";

    struct Flat(&'static str);

    impl Prices for Flat {
        fn unit_usd(&self, _asset: &CryptoAsset, _time: &str, _event_id: &str) -> Quote {
            Quote {
                value: Amount::parse(self.0).ok(),
                stale: false,
                linked: false,
            }
        }
    }

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount")
    }

    fn eth() -> CryptoAsset {
        CryptoAsset::Native {
            chain: Chain::Ethereum,
        }
    }

    fn usdc() -> CryptoAsset {
        CryptoAsset::Token(TokenId::new(
            Chain::Ethereum,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        ))
    }

    fn arrival(id: &str, day: &str, asset: CryptoAsset, quantity: &str) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: format!("{day}T10:00:00Z"),
            block: Some(1),
            from: OTHER.into(),
            to: ME.into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset,
                amount: amount(quantity),
                counterparty: Some(OTHER.into()),
            }],
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{id}"),
            },
        }
    }

    fn ledger_of(events: &[CryptoTx]) -> Ledger {
        let views = overrides::apply(events, &[]);
        let own = BTreeSet::new();
        fold(
            &views,
            &Inputs {
                own: &own,
                prices: &Flat("1000"),
                counts: &|_asset| true,
            },
        )
    }

    fn reading(balances: Vec<(CryptoAsset, Amount)>, complete: bool) -> Reading<'static> {
        Reading {
            account_id: "acc-1",
            chain: Chain::Ethereum,
            address: ME,
            balances,
            complete,
        }
    }

    #[test]
    fn the_starting_amount_plus_the_operations_equal_the_network_balance() {
        // The module has seen 1 ETH arrive; the node says the address holds 3.
        let events = vec![arrival("ev-1", "2026-03-05", eth(), "1")];
        let ledger = ledger_of(&events);
        let opened = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger,
            &events,
            Adjustment::Opening,
            "2026-03-01T00:00:00Z",
        );
        assert_eq!(opened.events.len(), 1);
        let adjustment = &opened.events[0];
        assert_eq!(adjustment.id, "copen:acc-1:ethereum|native");
        assert_eq!(adjustment.time, "2026-03-01T00:00:00Z");
        assert_eq!(adjustment.legs[0].direction, CryptoDirection::In);
        assert_eq!(adjustment.legs[0].amount, amount("2"));
        assert!(matches!(adjustment.tracking, TrackingRef::Address { .. }));
        assert!(adjustment.validate().is_ok());

        // Recomputed with the adjustment in place, the module agrees with the
        // network.
        let mut all = events.clone();
        all.extend(opened.events.clone());
        let after = ledger_of(&all);
        assert_eq!(after.balance("acc-1", &eth().key()), amount("3"));
        assert_eq!(
            after.state(&eth().key()).map(|have| have.quantity),
            Some(amount("3"))
        );
    }

    #[test]
    fn reconciling_twice_on_the_same_facts_writes_the_same_record() {
        let events = vec![arrival("ev-1", "2026-03-05", eth(), "1")];
        let ledger = ledger_of(&events);
        let network = reading(vec![(eth(), amount("3"))], true);
        let first = adjust(
            &network,
            &ledger,
            &events,
            Adjustment::Reconciled,
            "2026-03-09T08:00:00Z",
        );
        let mut all = events.clone();
        all.extend(first.events.clone());

        // Same day, same balance, nothing new: the record holds the same
        // quantity and is not a second difference on top of the first.
        let again = adjust(
            &network,
            &ledger_of(&all),
            &all,
            Adjustment::Reconciled,
            "2026-03-09T20:00:00Z",
        );
        assert_eq!(again.events.len(), 1);
        assert_eq!(again.events[0].id, first.events[0].id);
        assert_eq!(again.events[0].legs, first.events[0].legs);
        assert_eq!(
            serde_json::to_string(&again.events[0]).unwrap(),
            serde_json::to_string(&first.events[0]).unwrap()
        );
    }

    #[test]
    fn a_later_reading_corrects_the_adjustment_absolutely() {
        let events = vec![arrival("ev-1", "2026-03-05", eth(), "1")];
        let first = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger_of(&events),
            &events,
            Adjustment::Opening,
            "2026-03-01T00:00:00Z",
        );
        let mut all = events.clone();
        all.extend(first.events);
        // A page that was missing turns up: another 4 ETH arrived.
        all.push(arrival("ev-2", "2026-03-06", eth(), "4"));
        // The node still says 3: two of the opening ETH were never there.
        let fixed = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger_of(&all),
            &all,
            Adjustment::Opening,
            "2026-03-01T00:00:00Z",
        );
        assert_eq!(fixed.events.len(), 1);
        assert_eq!(fixed.events[0].legs[0].direction, CryptoDirection::Out);
        assert_eq!(fixed.events[0].legs[0].amount, amount("2"));
        let mut settled = all.clone();
        settled.retain(|event| event.id != fixed.events[0].id);
        settled.extend(fixed.events);
        assert_eq!(
            ledger_of(&settled).balance("acc-1", &eth().key()),
            amount("3")
        );
    }

    #[test]
    fn a_difference_marks_the_assets_profit_imprecise() {
        let events = vec![arrival("ev-1", "2026-03-05", eth(), "1")];
        let difference = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger_of(&events),
            &events,
            Adjustment::Reconciled,
            "2026-03-09T08:00:00Z",
        );
        let mut all = events.clone();
        all.extend(difference.events);
        let after = ledger_of(&all);
        let state = after.state(&eth().key()).expect("the coin");
        assert!(
            state.doubts.contains(&Doubt::Reconciled),
            "{:?}",
            state.doubts
        );
        assert!(state.imprecise());

        // The opening adjustment is a guess about the cost, not about the
        // quantity, and says so differently.
        let opened = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger_of(&events),
            &events,
            Adjustment::Opening,
            "2026-03-01T00:00:00Z",
        );
        let mut with_opening = events.clone();
        with_opening.extend(opened.events);
        let state = ledger_of(&with_opening)
            .state(&eth().key())
            .expect("the coin")
            .clone();
        assert!(state.doubts.contains(&Doubt::OpeningAtMarket));
        assert!(!state.doubts.contains(&Doubt::Reconciled));
    }

    #[test]
    fn a_balance_that_agrees_writes_nothing() {
        let events = vec![arrival("ev-1", "2026-03-05", eth(), "1")];
        let agreed = adjust(
            &reading(vec![(eth(), amount("1"))], true),
            &ledger_of(&events),
            &events,
            Adjustment::Reconciled,
            "2026-03-09T08:00:00Z",
        );
        assert_eq!(agreed, Reconciled::default());
    }

    #[test]
    fn a_partial_reading_never_takes_a_holding_to_zero() {
        let events = vec![
            arrival("ev-1", "2026-03-05", eth(), "1"),
            arrival("ev-2", "2026-03-05", usdc(), "500"),
        ];
        let ledger = ledger_of(&events);
        // Only the native coin was read, and it agrees.
        let partial = adjust(
            &reading(vec![(eth(), amount("1"))], false),
            &ledger,
            &events,
            Adjustment::Reconciled,
            "2026-03-09T08:00:00Z",
        );
        assert_eq!(partial.events, Vec::new());

        // A complete reading that does not list the token says the token is
        // gone, and that is written down.
        let complete = adjust(
            &reading(vec![(eth(), amount("1"))], true),
            &ledger,
            &events,
            Adjustment::Reconciled,
            "2026-03-09T08:00:00Z",
        );
        assert_eq!(complete.events.len(), 1);
        assert_eq!(complete.events[0].legs[0].amount, amount("500"));
        assert_eq!(complete.events[0].legs[0].direction, CryptoDirection::Out);
    }

    #[test]
    fn a_token_the_module_never_met_comes_in_as_an_opening_holding() {
        let ledger = ledger_of(&[]);
        let opened = adjust(
            &reading(vec![(usdc(), amount("250.5"))], true),
            &ledger,
            &[],
            Adjustment::Opening,
            "2026-03-01T00:00:00Z",
        );
        assert_eq!(opened.events.len(), 1);
        assert_eq!(opened.events[0].legs[0].amount, amount("250.5"));
        assert_eq!(opened.corrected.len(), 1);
        assert!(opened.corrected.contains(&usdc().key()));
    }

    #[test]
    fn two_devices_write_the_same_bytes_for_one_difference() {
        let events = vec![arrival("ev-1", "2026-03-05", eth(), "1")];
        let ledger = ledger_of(&events);
        let first = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger,
            &events,
            Adjustment::Reconciled,
            "2026-03-09T08:00:00Z",
        );
        // Another device, same day, a different hour, the events read in
        // another order.
        let mut shuffled = events.clone();
        shuffled.reverse();
        let second = adjust(
            &reading(vec![(eth(), amount("3"))], true),
            &ledger_of(&shuffled),
            &shuffled,
            Adjustment::Reconciled,
            "2026-03-09T21:30:00Z",
        );
        assert_eq!(
            serde_json::to_string(&first.events).unwrap(),
            serde_json::to_string(&second.events).unwrap()
        );
    }
}
