//! Average-cost basis and realized profit, one rule for every replay.
//!
//! Depends only on `Amount` and the leg model. Prices are per unit, in the
//! base currency; a zero price means "cost unknown".

use std::collections::HashMap;

use crate::amount::Amount;
use crate::model::transaction::{Leg, LegRole};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionKey {
    pub store_id: String,
    pub asset_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub quantity: Amount,
    /// Per unit, base currency; `None` when unknown.
    pub cost: Option<Amount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    Incoming { price_base: Amount },
    Outgoing { price_base: Amount, leaves: bool },
}

/// Profit on the covered part of a sale only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Realized {
    pub quantity: Amount,
    pub pnl_base: Amount,
}

#[derive(Debug, Default)]
pub struct CostBook {
    positions: HashMap<PositionKey, Position>,
}

impl CostBook {
    /// Applies a signed `delta` to the position; returns the realized part
    /// of a sale that leaves the user's wallets.
    pub fn apply(
        &mut self,
        key: &PositionKey,
        delta: Amount,
        movement: Movement,
    ) -> Option<Realized> {
        let position = self.positions.entry(key.clone()).or_default();
        match movement {
            Movement::Incoming { price_base } => {
                apply_incoming(position, delta, price_base);
                None
            }
            Movement::Outgoing { price_base, leaves } => {
                apply_outgoing(position, delta, price_base, leaves)
            }
        }
    }

    /// Zero quantity and unknown cost when the key was never seen.
    pub fn position(&self, key: &PositionKey) -> Position {
        self.positions.get(key).copied().unwrap_or_default()
    }
}

fn apply_incoming(position: &mut Position, delta: Amount, price_base: Amount) {
    let held = position.quantity.max(Amount::ZERO);
    let new_quantity = position.quantity + delta;
    if new_quantity.is_positive() && !price_base.is_zero() {
        position.cost = match (held.is_zero(), position.cost) {
            (true, _) => Some(price_base),
            (false, Some(cost)) => {
                Some((cost * held + price_base * (new_quantity - held)) / new_quantity)
            }
            // Held units of unknown cost: the mix stays unknown.
            (false, None) => None,
        };
    }
    position.quantity = new_quantity;
}

fn apply_outgoing(
    position: &mut Position,
    delta: Amount,
    price_base: Amount,
    leaves: bool,
) -> Option<Realized> {
    let outgoing = delta.abs();
    let covered = outgoing.min(position.quantity.max(Amount::ZERO));
    let realized = match position.cost {
        Some(cost) if leaves && !price_base.is_zero() && covered.is_positive() => Some(Realized {
            quantity: covered,
            pnl_base: (price_base - cost) * covered,
        }),
        _ => None,
    };
    position.quantity -= outgoing;
    if !position.quantity.is_positive() {
        position.cost = None;
    }
    realized
}

/// Rule 1: only principal and adjustment legs of non-fiat, non-derivative
/// assets take part in cost basis.
pub fn takes_part(leg: &Leg) -> bool {
    matches!(leg.role, LegRole::Principal | LegRole::Adjustment)
        && !leg.asset.is_fiat()
        && !leg.asset.is_derivative()
}

/// Zero for a non-positive quantity or an unknown cost.
pub fn unrealized(quantity: Amount, cost: Option<Amount>, price_base: Amount) -> Amount {
    match cost {
        Some(cost) if quantity.is_positive() => (price_base - cost) * quantity,
        _ => Amount::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::store::AssetRef;

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn key() -> PositionKey {
        PositionKey {
            store_id: "s".into(),
            asset_key: "BTC".into(),
        }
    }

    fn buy(book: &mut CostBook, quantity: &str, price: &str) {
        book.apply(
            &key(),
            amount(quantity),
            Movement::Incoming {
                price_base: amount(price),
            },
        );
    }

    fn sell(book: &mut CostBook, quantity: &str, price: &str) -> Option<Realized> {
        book.apply(
            &key(),
            -amount(quantity),
            Movement::Outgoing {
                price_base: amount(price),
                leaves: true,
            },
        )
    }

    fn short_book() -> CostBook {
        let mut book = CostBook::default();
        buy(&mut book, "1", "40000");
        sell(&mut book, "3", "60000");
        book
    }

    #[test]
    fn sale_realizes_only_the_covered_part() {
        let mut book = CostBook::default();
        buy(&mut book, "1", "40000");
        let realized = sell(&mut book, "3", "60000").unwrap();
        assert_eq!(realized.quantity, amount("1"));
        assert_eq!(realized.pnl_base, amount("20000"));
        let position = book.position(&key());
        assert_eq!(position.quantity, amount("-2"));
        assert_eq!(position.cost, None);
    }

    #[test]
    fn buy_that_stays_below_zero_keeps_no_cost() {
        let mut book = short_book();
        buy(&mut book, "1", "50000");
        let position = book.position(&key());
        assert_eq!(position.quantity, amount("-1"));
        assert_eq!(position.cost, None);
    }

    #[test]
    fn buy_that_crosses_zero_costs_only_the_part_above() {
        let mut book = short_book();
        buy(&mut book, "1", "50000");
        buy(&mut book, "3", "55000");
        let position = book.position(&key());
        assert_eq!(position.quantity, amount("2"));
        assert_eq!(position.cost, Some(amount("55000")));
    }

    #[test]
    fn unpriced_buy_does_not_dilute() {
        let mut book = CostBook::default();
        buy(&mut book, "1", "50000");
        buy(&mut book, "1", "0");
        assert_eq!(book.position(&key()).cost, Some(amount("50000")));
        let realized = sell(&mut book, "1", "60000").unwrap();
        assert_eq!(realized.pnl_base, amount("10000"));
    }

    #[test]
    fn held_units_of_unknown_cost_keep_the_mix_unknown() {
        let mut book = CostBook::default();
        buy(&mut book, "1", "0");
        buy(&mut book, "1", "50000");
        let position = book.position(&key());
        assert_eq!(position.quantity, amount("2"));
        assert_eq!(position.cost, None);
        assert_eq!(sell(&mut book, "1", "60000"), None);
    }

    #[test]
    fn own_wallet_move_realizes_nothing_but_moves_quantity() {
        let mut book = CostBook::default();
        buy(&mut book, "2", "40000");
        let moved = book.apply(
            &key(),
            amount("-1"),
            Movement::Outgoing {
                price_base: amount("60000"),
                leaves: false,
            },
        );
        assert_eq!(moved, None);
        let position = book.position(&key());
        assert_eq!(position.quantity, amount("1"));
        assert_eq!(position.cost, Some(amount("40000")));
    }

    #[test]
    fn unrealized_is_zero_for_non_positive_quantity_or_unknown_cost() {
        let cost = Some(amount("100"));
        assert_eq!(unrealized(amount("-2"), cost, amount("130")), Amount::ZERO);
        assert_eq!(unrealized(amount("2"), None, amount("130")), Amount::ZERO);
        assert_eq!(unrealized(amount("2"), cost, amount("130")), amount("60"));
    }

    fn leg(asset: AssetRef, role: LegRole) -> Leg {
        Leg {
            store_id: "s".into(),
            asset,
            amount: amount("1"),
            role,
            to_store_id: None,
            price_usd: None,
            price_base: None,
            price_base_currency: None,
            fx_override: None,
        }
    }

    #[test]
    fn fiat_never_takes_part() {
        let usd = || AssetRef::fiat("USD", "US Dollar");
        let btc = || AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin);
        assert!(!takes_part(&leg(usd(), LegRole::Principal)));
        assert!(takes_part(&leg(btc(), LegRole::Principal)));
        assert!(!takes_part(&leg(btc(), LegRole::Fee)));
        assert!(!takes_part(&leg(btc(), LegRole::Pnl)));
        assert!(!takes_part(&leg(
            AssetRef::perp("bybit", "BTCUSDT"),
            LegRole::Principal
        )));
    }
}

#[cfg(test)]
#[path = "cost_basis_tests.rs"]
mod strategy_tests;
