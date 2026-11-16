//! Every id the engine makes, in one place, because every one of them must be
//! derived from the facts and from nothing else.
//!
//! Two devices read the same address and must end up with the same bytes in
//! `crypto_events`: an id built from a clock, a counter or a random number
//! would give the same operation two records, each with its own sync stamp,
//! and the module would count it twice. So an id is a function of the account
//! and of what the operation is — and a refetch, which rewrites the events,
//! lands on the same ids and overwrites rather than appends.
//!
//! The prefixes also say what a record is, which is how a reconciliation
//! adjustment is told from an operation of the network ([`adjustment`]):
//!
//! | Prefix   | What                                                |
//! |----------|-----------------------------------------------------|
//! | `cev:`   | an operation of a source (a chain transaction, a fill) |
//! | `copen:` | the opening adjustment of an account's start date   |
//! | `crec:`  | a difference against the network's own balance      |
//! | `csnap:` | a snapshot of the module's value                    |

use crate::model::chain::Chain;
use crate::model::cryptotrack::{TrackingRef, chain_slug};

/// An operation of a source.
pub const EVENT: &str = "cev";
/// The one adjustment that stands for everything before the start date.
pub const OPENING: &str = "copen";
/// A difference against the network's own balance.
pub const RECONCILED: &str = "crec";
/// A snapshot of the module's value.
pub const SNAPSHOT: &str = "csnap";

/// The `YYYY-MM-DD` of an RFC3339 moment; the text itself when it is not one,
/// so the id stays a function of its input either way.
pub fn day(time: &str) -> String {
    let time = time.trim();
    // `get` and not a slice: a character may straddle the tenth byte.
    let date = time.get(..10).unwrap_or("");
    let shaped = date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit());
    if shaped {
        date.to_owned()
    } else {
        time.to_owned()
    }
}

/// The id of an operation of `account`, from what it was found through.
///
/// The account is part of it on purpose: a transfer between two followed
/// addresses is one transaction of the network and two operations of the
/// module (one leaves, one arrives), and they must not collide.
pub fn event(account_id: &str, tracking: &TrackingRef) -> String {
    match tracking {
        TrackingRef::ChainTx { chain, hash } => {
            format!("{EVENT}:{account_id}:{}:{hash}", chain_slug(*chain))
        }
        TrackingRef::ExchangeHistory { exchange, exec_id } => {
            format!("{EVENT}:{account_id}:{exchange}:{exec_id}")
        }
        TrackingRef::Position {
            protocol,
            chain,
            id,
        } => {
            format!(
                "{EVENT}:{account_id}:{}:{protocol}:{id}",
                chain_slug(*chain)
            )
        }
        TrackingRef::Address { chain, address } => {
            format!("{EVENT}:{account_id}:{}:{address}", chain_slug(*chain))
        }
    }
}

/// The id of the opening adjustment of one asset of one account: what the
/// address already held on the start date. One per account and asset, for
/// ever, so recomputing it rewrites the same record.
pub fn opening(account_id: &str, asset_key: &str) -> String {
    format!("{OPENING}:{account_id}:{asset_key}")
}

/// The id of a difference against the network's balance: one per account,
/// asset and day of the reading.
///
/// The day comes from the moment the balance was read, never from "now at the
/// time of the calculation": two devices that read the same balance on the
/// same day write one record, and a second reconciliation on that day
/// replaces it instead of adding a second difference on top of the first.
pub fn reconciled(account_id: &str, asset_key: &str, read_at: &str) -> String {
    format!("{RECONCILED}:{account_id}:{asset_key}:{}", day(read_at))
}

/// The id of a snapshot: one per day, for one account or for the module.
pub fn snapshot(account_id: Option<&str>, at: &str) -> String {
    format!("{SNAPSHOT}:{}:{}", day(at), account_id.unwrap_or("all"))
}

/// What an adjustment id says about a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Adjustment {
    /// What the address held before its start date.
    Opening,
    /// A difference the network's own balance showed.
    Reconciled,
}

/// Whether an event is an adjustment the engine wrote, and which kind. An
/// operation of a source is `None`.
pub fn adjustment(event_id: &str) -> Option<Adjustment> {
    if event_id.starts_with(&format!("{OPENING}:")) {
        Some(Adjustment::Opening)
    } else if event_id.starts_with(&format!("{RECONCILED}:")) {
        Some(Adjustment::Reconciled)
    } else {
        None
    }
}

/// The address an account's own records are tracked by.
pub fn address_ref(chain: Chain, address: &str) -> TrackingRef {
    TrackingRef::Address {
        chain,
        address: address.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0xdead";

    fn chain_tx() -> TrackingRef {
        TrackingRef::ChainTx {
            chain: Chain::Ethereum,
            hash: HASH.into(),
        }
    }

    #[test]
    fn one_transaction_of_two_followed_addresses_is_two_operations() {
        let sender = event("acc-a", &chain_tx());
        let receiver = event("acc-b", &chain_tx());
        assert_ne!(sender, receiver);
        assert_eq!(sender, "cev:acc-a:ethereum:0xdead");
        // And nothing else: the same facts give the same id again.
        assert_eq!(sender, event("acc-a", &chain_tx()));
    }

    #[test]
    fn an_adjustment_id_holds_no_clock_beyond_the_day_of_the_reading() {
        let morning = reconciled("acc-a", "ethereum|native", "2026-11-11T08:00:00Z");
        let evening = reconciled("acc-a", "ethereum|native", "2026-11-11T23:59:59Z");
        assert_eq!(morning, evening);
        assert_eq!(morning, "crec:acc-a:ethereum|native:2026-11-11");
        assert_ne!(
            morning,
            reconciled("acc-a", "ethereum|native", "2026-11-12T00:00:00Z")
        );
        // The opening adjustment has no day at all: it is the account's one.
        assert_eq!(
            opening("acc-a", "ethereum|native"),
            "copen:acc-a:ethereum|native"
        );
    }

    #[test]
    fn an_adjustment_is_told_apart_from_an_operation_by_its_id() {
        assert_eq!(
            adjustment(&opening("acc-a", "ethereum|native")),
            Some(Adjustment::Opening)
        );
        assert_eq!(
            adjustment(&reconciled(
                "acc-a",
                "ethereum|native",
                "2026-11-11T00:00:00Z"
            )),
            Some(Adjustment::Reconciled)
        );
        assert_eq!(adjustment(&event("acc-a", &chain_tx())), None);
        assert_eq!(adjustment(""), None);
    }

    #[test]
    fn a_day_is_the_date_of_a_moment_and_never_a_panic() {
        assert_eq!(day("2026-11-11T08:00:00Z"), "2026-11-11");
        assert_eq!(day("2026-11-11"), "2026-11-11");
        assert_eq!(day(""), "");
        assert_eq!(day("not a time"), "not a time");
        // Multi-byte text must not split a character boundary: here the
        // tenth byte falls inside the last character.
        assert_eq!(day("123456789\u{20ac}"), "123456789\u{20ac}");
        assert_eq!(day("календарь"), "календарь");
        assert_eq!(
            snapshot(None, "2026-11-11T08:00:00Z"),
            "csnap:2026-11-11:all"
        );
        assert_eq!(
            snapshot(Some("acc-a"), "2026-11-11T08:00:00Z"),
            "csnap:2026-11-11:acc-a"
        );
    }
}
