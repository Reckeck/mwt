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
//! Two rules keep a snapshot honest:
//!
//! * **The parts add up to the whole.** An account's value counts what its
//!   wallet holds *and* what it has inside protocols, so the module's value
//!   is the sum of the accounts' and the capital history cannot disagree with
//!   the wallet pages.
//! * **What is not priced is named, not guessed.** An asset with no known
//!   price contributes nothing and comes back in [`Snapshots::unpriced`]; a
//!   negative holding (the history does not reach back far enough yet)
//!   contributes nothing either and comes back in
//!   [`Snapshots::incomplete`] — nobody owes those coins, so they are not a
//!   debt against the capital, but the gap must be visible.

use std::collections::{BTreeMap, BTreeSet};

use crate::Amount;
use crate::model::cryptotrack::CryptoSnapshot;

use super::basis::{Ledger, Prices};
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
pub fn take<P: Prices>(ledger: &Ledger, at: &str, prices: &P) -> Snapshots {
    let mut out = Snapshots::default();
    let mut by_account: BTreeMap<&str, Amount> = BTreeMap::new();
    let mut total = Amount::ZERO;
    for account_id in ledger.accounts() {
        by_account.insert(account_id, Amount::ZERO);
    }
    for (asset_key, state) in &ledger.assets {
        let Some(price) = priced(prices, state, at, &mut out) else {
            continue;
        };
        for account_id in ledger.accounts() {
            let held = math::add(
                ledger.balance(account_id, asset_key),
                ledger.position(account_id, asset_key),
            )
            .unwrap_or(Amount::ZERO);
            if !held.is_positive() {
                if held.is_negative() {
                    out.incomplete.insert(asset_key.clone());
                }
                continue;
            }
            let Some(worth) = math::mul(held, price) else {
                out.unpriced.insert(asset_key.clone());
                continue;
            };
            let slot = by_account.entry(account_id).or_insert(Amount::ZERO);
            *slot = math::add(*slot, worth).unwrap_or(*slot);
            total = math::add(total, worth).unwrap_or(total);
        }
    }
    for (account_id, value) in by_account {
        out.records.push(CryptoSnapshot {
            id: ids::snapshot(Some(account_id), at),
            at: at.to_owned(),
            account_id: Some(account_id.to_owned()),
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

/// The price of an asset for a snapshot, remembering what had none.
fn priced<P: Prices>(
    prices: &P,
    state: &super::basis::AssetState,
    at: &str,
    out: &mut Snapshots,
) -> Option<Amount> {
    let quote = prices.unit_usd(&state.asset, at, "");
    match quote.value {
        Some(price) => Some(price),
        None => {
            out.unpriced.insert(state.asset.key());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cryptotrack::engine::basis::{Inputs, Quote, fold};
    use crate::cryptotrack::engine::overrides;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, CryptoTxKind, TokenId, TrackingRef,
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
        let taken = take(&ledger_of(&events), "2026-03-09T12:00:00Z", &book);
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
        let taken = take(&ledger_of(&events), "2026-03-09T12:00:00Z", &book);
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
        let taken = take(&ledger_of(&events), "2026-03-09T12:00:00Z", &book);
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
        let taken = take(&ledger_of(&events), "2026-03-09T12:00:00Z", &book);
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
        let taken = take(&ledger_of(&events), "2026-03-09T12:00:00Z", &book);
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
        let morning = take(&ledger, "2026-03-09T08:00:00Z", &book);
        let evening = take(&ledger, "2026-03-09T20:00:00Z", &book);
        let ids: Vec<_> = morning.records.iter().map(|one| one.id.clone()).collect();
        let same: Vec<_> = evening.records.iter().map(|one| one.id.clone()).collect();
        assert_eq!(ids, same);
        assert_eq!(morning.records[0].value_usd, evening.records[0].value_usd);
    }
}
