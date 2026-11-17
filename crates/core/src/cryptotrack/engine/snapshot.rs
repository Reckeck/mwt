//! Snapshots of the module's value, for the capital history.
//!
//! Balances are never stored (`model::cryptotrack`): they are folded out of
//! the operations every time, and a refetch can change them. A history of
//! what the module was worth cannot be rebuilt that way — the price of a
//! token on a day long past may be gone from every provider — so the value
//! itself is written down, one record per day, per account and for the module
//! as a whole ([`CryptoSnapshot`]).
//!
//! The ids are a function of the day and the account
//! ([`super::ids::snapshot`]), so a second snapshot of the same day replaces
//! the first instead of leaving two values for one day in the history.
//!
//! Known step: snapshots written before this rule are not rewritten, so the
//! first new snapshot may step against the old history (adapter-only
//! positions and debts now count, receipts are dropped). There is no
//! back-fill.
//!
//! Three rules keep a snapshot honest:
//!
//! * **The parts add up to the whole.** An account's value counts what its
//!   wallet holds *and* what it has inside protocols, so the module's value
//!   is the sum of the accounts' and the capital history cannot disagree with
//!   the wallet pages.
//! * **The quantities are the present capital's.** What an account holds,
//!   net of debt, comes from `position_value::held_by_account`, the function
//!   the present capital uses, so an adapter-only position counts, a debt
//!   lowers the value, and a position receipt is not counted twice. The value
//!   of an account may come out negative (debt over collateral) and is
//!   written as is.
//! * **What is not priced is named, not guessed.** An asset with no known
//!   price contributes nothing and comes back in [`Snapshots::unpriced`]; a
//!   negative wallet part (the history does not reach back far enough yet)
//!   contributes nothing either and comes back in
//!   [`Snapshots::incomplete`] — nobody owes those coins, so they are not a
//!   debt against the capital, but the gap must be visible.

use std::collections::{BTreeMap, BTreeSet};

use crate::Amount;
use crate::model::cryptotrack::{CryptoAsset, CryptoSnapshot};

use super::basis::{Ledger, Prices};
use super::position_value::{PositionInputs, held_by_account};
use super::{ids, math};

/// What [`take`] made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshots {
    /// One per account, then one for the module (`account_id: None`).
    pub records: Vec<CryptoSnapshot>,
    /// Asset keys left out because no price was known.
    pub unpriced: BTreeSet<String>,
    /// Asset keys whose holding came out negative: more left than the module
    /// ever saw arrive.
    pub incomplete: BTreeSet<String>,
}

/// The value of each account and of the module at `at`.
///
/// The quantities come from [`held_by_account`], the same function the
/// present capital uses; this module only prices them.
pub fn take<P: Prices>(
    ledger: &Ledger,
    inputs: &PositionInputs<'_>,
    at: &str,
    prices: &P,
) -> Snapshots {
    let mut out = Snapshots::default();
    let mut by_account: BTreeMap<String, Amount> = ledger
        .accounts()
        .into_iter()
        .map(|account_id| (account_id.to_owned(), Amount::ZERO))
        .collect();
    let mut total = Amount::ZERO;
    for held in held_by_account(ledger, inputs) {
        let Some(price) = priced(prices, &held.asset, at, &mut out) else {
            continue;
        };
        let Some(worth) = math::mul(held.net(), price) else {
            out.unpriced.insert(held.asset.key());
            continue;
        };
        let slot = by_account.entry(held.account_id).or_insert(Amount::ZERO);
        *slot = math::add(*slot, worth).unwrap_or(*slot);
        total = math::add(total, worth).unwrap_or(total);
    }
    note_incomplete(ledger, at, prices, &mut out);
    for (account_id, value) in by_account {
        out.records.push(CryptoSnapshot {
            id: ids::snapshot(Some(&account_id), at),
            at: at.to_owned(),
            account_id: Some(account_id),
            value_usd: math::usd(value),
        });
    }
    out.records.push(CryptoSnapshot {
        id: ids::snapshot(None, at),
        at: at.to_owned(),
        account_id: None,
        value_usd: math::usd(total),
    });
    out
}

/// Name the priced assets whose wallet part is negative: the history does not
/// reach back far enough. They add nothing to the value (the shared function
/// clamps them), but the gap must be visible.
fn note_incomplete<P: Prices>(ledger: &Ledger, at: &str, prices: &P, out: &mut Snapshots) {
    for (asset_key, state) in &ledger.assets {
        if prices.unit_usd(&state.asset, at, "").value.is_none() {
            continue;
        }
        for account_id in ledger.accounts() {
            let wallet = math::add(
                ledger.balance(account_id, asset_key),
                ledger.position(account_id, asset_key),
            )
            .unwrap_or(Amount::ZERO);
            if wallet.is_negative() {
                out.incomplete.insert(asset_key.clone());
            }
        }
    }
}

/// The price of an asset for a snapshot, remembering what had none.
fn priced<P: Prices>(
    prices: &P,
    asset: &CryptoAsset,
    at: &str,
    out: &mut Snapshots,
) -> Option<Amount> {
    let quote = prices.unit_usd(asset, at, "");
    if quote.value.is_none() {
        out.unpriced.insert(asset.key());
    }
    quote.value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cryptotrack::engine::basis::{Inputs, Quote, fold};
    use crate::cryptotrack::engine::overrides;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAsset, CryptoDirection, CryptoLeg, CryptoPosition, CryptoTx, CryptoTxKind, TokenId,
        TrackingRef,
    };

    const POOL: &str = "0x5555555555555555555555555555555555555555";
    const OTHER: &str = "0x2222222222222222222222222222222222222222";

    struct Book(BTreeMap<String, Amount>);

    impl Book {
        fn of(pairs: &[(CryptoAsset, &str)]) -> Self {
            Self(
                pairs
                    .iter()
                    .map(|(asset, price)| (asset.key(), amount(price)))
                    .collect(),
            )
        }
    }

    impl Prices for Book {
        fn unit_usd(&self, asset: &CryptoAsset, _time: &str, _event_id: &str) -> Quote {
            Quote {
                value: self.0.get(&asset.key()).copied(),
                stale: false,
                linked: false,
            }
        }
    }

    fn always(_asset: &CryptoAsset) -> bool {
        true
    }

    fn none() -> PositionInputs<'static> {
        static RECEIPTS: BTreeSet<String> = BTreeSet::new();
        PositionInputs {
            positions: &[],
            receipts: &RECEIPTS,
            counts: &always,
        }
    }

    fn aave_position(legs: Vec<CryptoLeg>) -> CryptoPosition {
        CryptoPosition {
            id: "p1".into(),
            account_id: "acc-1".into(),
            chain: Chain::Ethereum,
            protocol: "aave-v3".into(),
            name: String::new(),
            legs,
            receipts: Vec::new(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn leg(direction: CryptoDirection, asset: CryptoAsset, quantity: &str) -> CryptoLeg {
        CryptoLeg {
            direction,
            asset,
            amount: amount(quantity),
            counterparty: None,
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

    fn event(
        id: &str,
        account_id: &str,
        kind: CryptoTxKind,
        direction: CryptoDirection,
        asset: CryptoAsset,
        quantity: &str,
        with: &str,
    ) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: account_id.into(),
            time: "2026-03-01T10:00:00Z".into(),
            block: Some(1),
            from: OTHER.into(),
            to: OTHER.into(),
            legs: vec![CryptoLeg {
                direction,
                asset,
                amount: amount(quantity),
                counterparty: Some(with.into()),
            }],
            gas: None,
            protocol: None,
            method: None,
            kind,
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
                prices: &Book::of(&[(eth(), "2000"), (usdc(), "1")]),
                counts: &|_asset| true,
            },
        )
    }

    #[test]
    fn the_accounts_add_up_to_the_module_and_the_ids_are_the_day() {
        let events = vec![
            event(
                "ev-1",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                eth(),
                "1.5",
                OTHER,
            ),
            event(
                "ev-2",
                "acc-2",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                usdc(),
                "250",
                OTHER,
            ),
        ];
        let book = Book::of(&[(eth(), "2000"), (usdc(), "1")]);
        let taken = take(&ledger_of(&events), &none(), "2026-03-09T12:00:00Z", &book);
        assert_eq!(taken.records.len(), 3);
        let module = taken.records.last().expect("the module's own");
        assert_eq!(module.id, "csnap:2026-03-09:all");
        assert_eq!(module.account_id, None);
        assert_eq!(module.value_usd, amount("3250.00"));
        let parts: Amount = taken
            .records
            .iter()
            .filter(|record| record.account_id.is_some())
            .fold(Amount::ZERO, |sum, record| sum + record.value_usd);
        assert_eq!(parts, module.value_usd, "the parts add up to the whole");
        assert_eq!(taken.records[0].id, "csnap:2026-03-09:acc-1");
        assert!(taken.records[0].validate().is_ok());
        assert!(taken.unpriced.is_empty());
    }

    #[test]
    fn what_is_inside_a_protocol_still_belongs_to_its_account() {
        let events = vec![
            event(
                "ev-1",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                usdc(),
                "1000",
                OTHER,
            ),
            event(
                "ev-2",
                "acc-1",
                CryptoTxKind::Stake,
                CryptoDirection::Out,
                usdc(),
                "600",
                POOL,
            ),
        ];
        let book = Book::of(&[(usdc(), "1")]);
        let taken = take(&ledger_of(&events), &none(), "2026-03-09T12:00:00Z", &book);
        let account = taken
            .records
            .iter()
            .find(|record| record.account_id.as_deref() == Some("acc-1"))
            .expect("the account");
        assert_eq!(account.value_usd, amount("1000.00"), "wallet plus protocol");
        assert_eq!(taken.records.last().unwrap().value_usd, amount("1000.00"));
    }

    #[test]
    fn an_asset_with_no_price_is_named_and_not_guessed() {
        let events = vec![
            event(
                "ev-1",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                eth(),
                "1",
                OTHER,
            ),
            event(
                "ev-2",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                usdc(),
                "500",
                OTHER,
            ),
        ];
        let book = Book::of(&[(eth(), "2000")]);
        let taken = take(&ledger_of(&events), &none(), "2026-03-09T12:00:00Z", &book);
        assert_eq!(taken.records.last().unwrap().value_usd, amount("2000.00"));
        assert!(taken.unpriced.contains(&usdc().key()));
        assert_eq!(taken.unpriced.len(), 1);
    }

    #[test]
    fn a_holding_that_went_negative_is_not_a_debt_against_the_capital() {
        let events = vec![event(
            "ev-1",
            "acc-1",
            CryptoTxKind::Transfer,
            CryptoDirection::Out,
            eth(),
            "1",
            OTHER,
        )];
        let book = Book::of(&[(eth(), "2000")]);
        let taken = take(&ledger_of(&events), &none(), "2026-03-09T12:00:00Z", &book);
        assert_eq!(taken.records.last().unwrap().value_usd, Amount::ZERO);
        assert!(taken.incomplete.contains(&eth().key()));
    }

    #[test]
    fn the_value_is_written_down_in_cents() {
        let events = vec![event(
            "ev-1",
            "acc-1",
            CryptoTxKind::Transfer,
            CryptoDirection::In,
            eth(),
            "0.333333333333333333",
            OTHER,
        )];
        let book = Book::of(&[(eth(), "3000.005")]);
        let taken = take(&ledger_of(&events), &none(), "2026-03-09T12:00:00Z", &book);
        // 0.333333333333333333 * 3000.005 = 1000.0033333...
        assert_eq!(taken.records.last().unwrap().value_usd, amount("1000.00"));
    }

    #[test]
    fn a_second_snapshot_of_the_same_day_is_the_same_record() {
        let events = vec![event(
            "ev-1",
            "acc-1",
            CryptoTxKind::Transfer,
            CryptoDirection::In,
            eth(),
            "1",
            OTHER,
        )];
        let book = Book::of(&[(eth(), "2000")]);
        let ledger = ledger_of(&events);
        let morning = take(&ledger, &none(), "2026-03-09T08:00:00Z", &book);
        let evening = take(&ledger, &none(), "2026-03-09T20:00:00Z", &book);
        let ids: Vec<_> = morning.records.iter().map(|one| one.id.clone()).collect();
        let same: Vec<_> = evening.records.iter().map(|one| one.id.clone()).collect();
        assert_eq!(ids, same);
        assert_eq!(morning.records[0].value_usd, evening.records[0].value_usd);
    }

    #[test]
    fn present_value_equals_the_snapshot() {
        // Lido-style: an adapter-only position, no operation explains it.
        let lido = CryptoPosition {
            id: "p-lido".into(),
            protocol: "lido".into(),
            ..aave_position(vec![leg(CryptoDirection::In, eth(), "2")])
        };
        // Aave: 3 000 USDC supplied, 1 000 borrowed -> 2 000 net.
        let aave = CryptoPosition {
            id: "p-aave".into(),
            ..aave_position(vec![
                leg(CryptoDirection::In, usdc(), "3000"),
                leg(CryptoDirection::Out, usdc(), "1000"),
            ])
        };
        let positions = [&lido, &aave];
        let receipts = BTreeSet::new();
        let inputs = PositionInputs {
            positions: &positions,
            receipts: &receipts,
            counts: &|_asset| true,
        };
        let book = Book::of(&[(eth(), "2000"), (usdc(), "1")]);
        let taken = take(&Ledger::default(), &inputs, "2026-03-09T12:00:00Z", &book);
        // 2 ETH * 2 000 = 4 000, plus (3 000 - 1 000) USDC = 2 000.
        assert_eq!(taken.records.last().unwrap().value_usd, amount("6000.00"));
        assert_eq!(taken.records[0].value_usd, amount("6000.00"));
    }

    #[test]
    fn a_receipt_in_the_wallet_is_not_in_the_snapshot() {
        let events = vec![
            event(
                "ev-1",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                eth(),
                "1",
                OTHER,
            ),
            event(
                "ev-2",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                usdc(),
                "1000",
                OTHER,
            ),
        ];
        let receipts: BTreeSet<String> = [usdc().key()].into();
        let inputs = PositionInputs {
            positions: &[],
            receipts: &receipts,
            counts: &|_asset| true,
        };
        let book = Book::of(&[(eth(), "2000"), (usdc(), "1")]);
        let taken = take(&ledger_of(&events), &inputs, "2026-03-09T12:00:00Z", &book);
        assert_eq!(taken.records.last().unwrap().value_usd, amount("2000.00"));
    }

    #[test]
    fn the_old_wallet_only_snapshot_is_unchanged() {
        let events = vec![
            event(
                "ev-1",
                "acc-1",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                eth(),
                "1.5",
                OTHER,
            ),
            event(
                "ev-2",
                "acc-2",
                CryptoTxKind::Transfer,
                CryptoDirection::In,
                usdc(),
                "250",
                OTHER,
            ),
        ];
        let book = Book::of(&[(eth(), "2000"), (usdc(), "1")]);
        let taken = take(&ledger_of(&events), &none(), "2026-03-09T12:00:00Z", &book);
        let values: Vec<Amount> = taken.records.iter().map(|one| one.value_usd).collect();
        assert_eq!(
            values,
            vec![amount("3000.00"), amount("250.00"), amount("3250.00")]
        );
    }

    #[test]
    fn a_negative_net_is_written_as_is_and_a_negative_wallet_part_is_incomplete() {
        let events = vec![event(
            "ev-1",
            "acc-1",
            CryptoTxKind::Transfer,
            CryptoDirection::Out,
            eth(),
            "1",
            OTHER,
        )];
        // The wallet holds -1 ETH, so the 0.2 ETH collateral nets to -0.8 ETH, clamped to 0
        // per asset: it does not offset the 1 000 USDC of debt. The net is -1 000, written as is.
        let aave = aave_position(vec![
            leg(CryptoDirection::In, eth(), "0.2"),
            leg(CryptoDirection::Out, usdc(), "1000"),
        ]);
        let positions = [&aave];
        let receipts = BTreeSet::new();
        let inputs = PositionInputs {
            positions: &positions,
            receipts: &receipts,
            counts: &|_asset| true,
        };
        let book = Book::of(&[(eth(), "2000"), (usdc(), "1")]);
        let taken = take(&ledger_of(&events), &inputs, "2026-03-09T12:00:00Z", &book);
        assert_eq!(taken.records.last().unwrap().value_usd, amount("-1000.00"));
        assert!(taken.incomplete.contains(&eth().key()));
    }
}
