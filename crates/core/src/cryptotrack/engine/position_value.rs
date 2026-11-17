//! What each account holds, net of debt: the one function the present capital
//! and the snapshots both take a position's quantities from.
//!
//! Per `(account, asset)`:
//!
//! 1. `held` = the ledger's wallet balance + its position entry + the `In`
//!    legs of the account's stored positions, the legs only when the ledger
//!    has no position entry for that asset (the operations stay the source of
//!    truth wherever they exist). The sum is kept only when positive: a
//!    negative wallet part is a missing history, not a debt.
//! 2. `debt` = the `Out` legs of the stored positions. The ledger models no
//!    debt, so it is never read from there.
//! 3. [`Held::net`] = `held - debt`; it may be negative and is kept as is.
//! 4. A token that does not count contributes nothing; a receipt contributes
//!    no wallet or ledger quantity, but its adapter legs and debt stay (a
//!    Lido stake is held as stETH legs, and stETH is a receipt).
//!
//! Which accounts and records take part is the caller's decision, passed in
//! through [`PositionInputs`]; this module does not know the vault.

use std::collections::{BTreeMap, BTreeSet};

use crate::Amount;
use crate::model::cryptotrack::{CryptoAsset, CryptoDirection, CryptoPosition};

use super::basis::Ledger;
use super::math;

/// What one account holds of one asset.
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub account_id: String,
    pub asset: CryptoAsset,
    /// Wallet, ledger position and unfolded adapter legs; never negative.
    pub held: Amount,
    /// The part of `held` that comes from adapter legs, whose cost no
    /// operation records.
    pub unfolded: Amount,
    /// The debt legs of the account's positions; never negative.
    pub debt: Amount,
    /// The wallet part (ledger balance and position) is negative: the history
    /// does not reach back far enough. It adds nothing to `held`, but the gap
    /// must be visible.
    pub short: bool,
}

impl Held {
    /// `held - debt`; negative when the debt is larger.
    pub fn net(&self) -> Amount {
        math::sub(self.held, self.debt).unwrap_or(self.held)
    }
}

/// What the caller decided takes part.
pub struct PositionInputs<'a> {
    /// Already filtered by the caller (followed, live, not shadowed).
    pub positions: &'a [&'a CryptoPosition],
    /// Asset keys of position receipts.
    pub receipts: &'a BTreeSet<String>,
    /// `tokens::counts_in_totals`.
    pub counts: &'a dyn Fn(&CryptoAsset) -> bool,
}

#[derive(Default)]
struct Slot {
    wallet: Amount,
    unfolded: Amount,
    debt: Amount,
}

/// Net holdings by account and asset, in a stable order.
pub fn held_by_account(ledger: &Ledger, inputs: &PositionInputs<'_>) -> Vec<Held> {
    let mut assets: BTreeMap<String, CryptoAsset> = ledger
        .assets
        .iter()
        .map(|(key, state)| (key.clone(), state.asset.clone()))
        .collect();
    let mut slots: BTreeMap<(String, String), Slot> = BTreeMap::new();
    for (key, quantity) in ledger.balances.iter().chain(ledger.positions.iter()) {
        let slot = slots.entry(key.clone()).or_default();
        slot.wallet = math::add(slot.wallet, *quantity).unwrap_or(slot.wallet);
    }
    for record in inputs.positions {
        for leg in &record.legs {
            if !leg.amount.is_positive() {
                continue;
            }
            let key = (record.account_id.clone(), leg.asset.key());
            let folded = ledger.positions.contains_key(&key);
            assets
                .entry(leg.asset.key())
                .or_insert_with(|| leg.asset.clone());
            let slot = slots.entry(key).or_default();
            match leg.direction {
                CryptoDirection::In if !folded => {
                    slot.unfolded = math::add(slot.unfolded, leg.amount).unwrap_or(slot.unfolded);
                }
                CryptoDirection::In => {}
                CryptoDirection::Out => {
                    slot.debt = math::add(slot.debt, leg.amount).unwrap_or(slot.debt);
                }
            }
        }
    }
    let mut out = Vec::new();
    for ((account_id, key), slot) in slots {
        let Some(asset) = assets.get(&key) else {
            continue;
        };
        if !(inputs.counts)(asset) {
            continue;
        }
        // A receipt's wallet balance is the position it stands for, already
        // counted through the legs; only that part is dropped.
        let wallet = if inputs.receipts.contains(&key) {
            Amount::ZERO
        } else {
            slot.wallet
        };
        let gross = math::add(wallet, slot.unfolded).unwrap_or(wallet);
        let held = if gross.is_positive() {
            gross
        } else {
            Amount::ZERO
        };
        let short = wallet.is_negative();
        if held.is_zero() && slot.debt.is_zero() && !short {
            continue;
        }
        out.push(Held {
            account_id,
            asset: asset.clone(),
            held,
            unfolded: slot.unfolded,
            debt: slot.debt,
            short,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::basis::AssetState;
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoLeg, TokenId};

    const ME: &str = "acct";

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

    fn leg(direction: CryptoDirection, asset: CryptoAsset, quantity: &str) -> CryptoLeg {
        CryptoLeg {
            direction,
            asset,
            amount: amount(quantity),
            counterparty: None,
        }
    }

    fn position(legs: Vec<CryptoLeg>) -> CryptoPosition {
        CryptoPosition {
            id: "p1".into(),
            account_id: ME.into(),
            chain: Chain::Ethereum,
            protocol: "aave-v3".into(),
            name: String::new(),
            legs,
            receipts: Vec::new(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn run(
        ledger: &Ledger,
        positions: &[CryptoPosition],
        receipts: &BTreeSet<String>,
        counts: &dyn Fn(&CryptoAsset) -> bool,
    ) -> Vec<Held> {
        let refs: Vec<&CryptoPosition> = positions.iter().collect();
        held_by_account(
            ledger,
            &PositionInputs {
                positions: &refs,
                receipts,
                counts,
            },
        )
    }

    fn find<'a>(held: &'a [Held], asset: &CryptoAsset) -> &'a Held {
        held.iter().find(|row| &row.asset == asset).expect("a row")
    }

    fn value(held: &[Held], price_eth: &str, price_usdc: &str) -> Amount {
        held.iter().fold(Amount::ZERO, |sum, row| {
            let price = if row.asset == eth() {
                price_eth
            } else {
                price_usdc
            };
            sum + row.net() * amount(price)
        })
    }

    #[test]
    fn debt_is_subtracted() {
        let aave = position(vec![
            leg(CryptoDirection::In, eth(), "1"),
            leg(CryptoDirection::Out, usdc(), "1000"),
        ]);
        let rows = run(&Ledger::default(), &[aave], &BTreeSet::new(), &|_| true);
        assert_eq!(find(&rows, &eth()).net(), amount("1"));
        assert_eq!(find(&rows, &usdc()).net(), amount("-1000"));
        assert_eq!(value(&rows, "3000", "1"), amount("2000"));
    }

    #[test]
    fn debt_may_exceed_collateral() {
        let aave = position(vec![
            leg(CryptoDirection::In, eth(), "0.2"),
            leg(CryptoDirection::Out, usdc(), "1000"),
        ]);
        let rows = run(&Ledger::default(), &[aave], &BTreeSet::new(), &|_| true);
        assert_eq!(value(&rows, "3000", "1"), amount("-400"));
    }

    #[test]
    fn a_ledger_position_entry_wins_over_adapter_legs() {
        let mut ledger = Ledger::default();
        ledger.assets.insert(eth().key(), AssetState::new(eth()));
        ledger
            .positions
            .insert((ME.into(), eth().key()), amount("2"));
        let aave = position(vec![
            leg(CryptoDirection::In, eth(), "5"),
            leg(CryptoDirection::Out, eth(), "0.5"),
        ]);
        let rows = run(&ledger, &[aave], &BTreeSet::new(), &|_| true);
        let row = find(&rows, &eth());
        assert_eq!(row.held, amount("2"));
        assert_eq!(row.debt, amount("0.5"));
    }

    #[test]
    fn receipt_wallet_and_uncounted_tokens_are_dropped() {
        let mut ledger = Ledger::default();
        ledger.assets.insert(eth().key(), AssetState::new(eth()));
        ledger
            .balances
            .insert((ME.into(), eth().key()), amount("7"));
        let receipts: BTreeSet<String> = [eth().key()].into();
        let rows = run(&ledger, &[], &receipts, &|_| true);
        assert!(rows.is_empty());
        let aave = position(vec![
            leg(CryptoDirection::In, eth(), "1"),
            leg(CryptoDirection::In, usdc(), "10"),
        ]);
        let rows = run(&Ledger::default(), &[aave], &BTreeSet::new(), &|asset| {
            *asset != usdc()
        });
        assert!(rows.iter().all(|row| row.asset != usdc()));
        assert_eq!(find(&rows, &eth()).held, amount("1"));
    }

    #[test]
    fn a_receipt_keeps_its_adapter_legs() {
        let lido = position(vec![leg(CryptoDirection::In, eth(), "2")]);
        let receipts: BTreeSet<String> = [eth().key()].into();
        let mut ledger = Ledger::default();
        ledger.assets.insert(eth().key(), AssetState::new(eth()));
        ledger
            .balances
            .insert((ME.into(), eth().key()), amount("7"));
        let rows = run(&ledger, &[lido], &receipts, &|_| true);
        let row = find(&rows, &eth());
        assert_eq!(row.held, amount("2"));
        assert_eq!(row.unfolded, amount("2"));
        assert_eq!(value(&rows, "2000", "1"), amount("4000"));
    }

    #[test]
    fn a_negative_wallet_part_is_not_a_debt() {
        let mut ledger = Ledger::default();
        ledger.assets.insert(eth().key(), AssetState::new(eth()));
        ledger
            .balances
            .insert((ME.into(), eth().key()), amount("-3"));
        let rows = run(&ledger, &[], &BTreeSet::new(), &|_| true);
        assert!(rows.is_empty());
        let aave = position(vec![leg(CryptoDirection::Out, eth(), "1")]);
        let rows = run(&ledger, &[aave], &BTreeSet::new(), &|_| true);
        let row = find(&rows, &eth());
        assert_eq!(row.held, Amount::ZERO);
        assert_eq!(row.net(), amount("-1"));
    }
}
