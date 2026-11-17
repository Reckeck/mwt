//! Tests of the cost basis fold ([`super::basis`]), kept beside it the way
//! the EVM source keeps its own (`sources/evm/tests.rs`): the fold is the
//! money core of the module and its cases are worth reading on their own.

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::super::basis::*;
    use crate::Amount;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAsset, CryptoDirection, CryptoFee, CryptoLeg, CryptoTx, CryptoTxKind, TokenId,
        TrackingRef,
    };

    const ME: &str = "0x1111111111111111111111111111111111111111";
    const MINE_TOO: &str = "0x4444444444444444444444444444444444444444";
    const OTHER: &str = "0x2222222222222222222222222222222222222222";
    const POOL: &str = "0x5555555555555555555555555555555555555555";

    fn amount(text: &str) -> Amount {
        Amount::new(text.parse().expect("a test amount"))
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

    /// A price book by `asset key @ day`.
    #[derive(Default)]
    struct Book(BTreeMap<String, Amount>);

    impl Book {
        fn with(mut self, asset: &CryptoAsset, day: &str, price: &str) -> Self {
            self.0
                .insert(format!("{}@{day}", asset.key()), amount(price));
            self
        }
    }

    impl Prices for Book {
        fn unit_usd(&self, asset: &CryptoAsset, time: &str, _event_id: &str) -> Quote {
            match self
                .0
                .get(&format!("{}@{}", asset.key(), super::super::ids::day(time)))
            {
                Some(price) => Quote {
                    value: Some(*price),
                    stale: false,
                    linked: false,
                },
                None => Quote::default(),
            }
        }
    }

    fn own(addresses: &[&str]) -> BTreeSet<String> {
        addresses.iter().map(|text| (*text).to_string()).collect()
    }

    fn leg(
        direction: CryptoDirection,
        asset: CryptoAsset,
        quantity: &str,
        with: &str,
    ) -> CryptoLeg {
        CryptoLeg {
            direction,
            asset,
            amount: amount(quantity),
            counterparty: Some(with.to_string()),
        }
    }

    fn event(id: &str, day: &str, kind: CryptoTxKind, legs: Vec<CryptoLeg>) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: format!("{day}T10:00:00Z"),
            block: Some(1),
            from: ME.into(),
            to: OTHER.into(),
            legs,
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

    fn ledger_of(events: &[CryptoTx], addresses: &[&str], book: &Book) -> Ledger {
        let views = super::super::overrides::apply(events, &[]);
        let own = own(addresses);
        fold(
            &views,
            &Inputs {
                own: &own,
                prices: book,
                counts: &|_asset| true,
            },
        )
    }

    fn state(ledger: &Ledger, asset: &CryptoAsset) -> AssetState {
        ledger
            .assets
            .get(&asset.key())
            .cloned()
            .unwrap_or_else(|| panic!("no state for {}", asset.key()))
    }

    #[test]
    fn two_purchases_average_and_a_sale_realizes_against_that_average() {
        let book = Book::default()
            .with(&eth(), "2026-03-01", "1000")
            .with(&eth(), "2026-03-02", "2000")
            .with(&eth(), "2026-03-03", "2500");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "1", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "1", OTHER)],
            ),
            event(
                "ev-3",
                "2026-03-03",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::Out, eth(), "1", OTHER)],
            ),
        ];
        let ledger = ledger_of(&events, &[ME], &book);
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.quantity, amount("1"));
        assert_eq!(eth_state.cost_usd, amount("1500"));
        assert_eq!(eth_state.average_usd(), Some(amount("1500")));
        assert_eq!(eth_state.realized_usd, amount("1000"));
        assert!(!eth_state.imprecise(), "{:?}", eth_state.doubts);
    }

    #[test]
    fn a_transfer_between_my_own_addresses_carries_the_basis_and_realizes_nothing() {
        // Decision 4 of the plan: it used to reset the cost basis.
        let book =
            Book::default()
                .with(&eth(), "2026-03-01", "1000")
                .with(&eth(), "2026-03-02", "2000");
        let mut out = event(
            "ev-2",
            "2026-03-02",
            CryptoTxKind::Transfer,
            vec![leg(CryptoDirection::Out, eth(), "1", MINE_TOO)],
        );
        out.account_id = "acc-1".into();
        let mut back = event(
            "ev-3",
            "2026-03-02",
            CryptoTxKind::Transfer,
            vec![leg(CryptoDirection::In, eth(), "1", ME)],
        );
        back.account_id = "acc-2".into();
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "1", OTHER)],
            ),
            out,
            back,
        ];
        let ledger = ledger_of(&events, &[ME, MINE_TOO], &book);
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.realized_usd, Amount::ZERO, "a move is not a sale");
        assert_eq!(eth_state.quantity, amount("1"));
        assert_eq!(eth_state.cost_usd, amount("1000"), "the basis came along");
        assert_eq!(eth_state.average_usd(), Some(amount("1000")));
        // The coins are where they were sent, and the module still has them all.
        assert_eq!(ledger.balance("acc-1", &eth().key()), Amount::ZERO);
        assert_eq!(ledger.balance("acc-2", &eth().key()), amount("1"));
        assert_eq!(ledger.drift(&eth().key()), Amount::ZERO);
        assert!(!eth_state.imprecise(), "{:?}", eth_state.doubts);
    }

    #[test]
    fn a_deposit_moves_coins_into_a_position_and_an_exit_returns_them() {
        // Decision 5: no realized profit either way.
        let book = Book::default()
            .with(&usdc(), "2026-03-01", "1")
            .with(&usdc(), "2026-03-02", "1")
            .with(&usdc(), "2026-03-03", "1");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, usdc(), "1000", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Stake,
                vec![leg(CryptoDirection::Out, usdc(), "600", POOL)],
            ),
        ];
        let deposited = ledger_of(&events, &[ME], &book);
        let state_after_deposit = state(&deposited, &usdc());
        assert_eq!(state_after_deposit.quantity, amount("400"), "in the wallet");
        assert_eq!(state_after_deposit.in_positions, amount("600"));
        assert_eq!(state_after_deposit.owned(), amount("1000"));
        assert_eq!(state_after_deposit.cost_usd, amount("1000"));
        assert_eq!(state_after_deposit.realized_usd, Amount::ZERO);

        let mut with_exit = events.clone();
        with_exit.push(event(
            "ev-3",
            "2026-03-03",
            CryptoTxKind::Unstake,
            vec![leg(CryptoDirection::In, usdc(), "630", POOL)],
        ));
        let exited = ledger_of(&with_exit, &[ME], &book);
        let after = state(&exited, &usdc());
        assert_eq!(after.in_positions, Amount::ZERO);
        assert_eq!(after.quantity, amount("1030"));
        assert_eq!(after.realized_usd, Amount::ZERO, "an exit is not a sale");
        // The 30 above what went in is what the protocol paid: income, and
        // it carries its own cost, so no profit appears out of nowhere.
        assert_eq!(after.income_usd, amount("30"));
        assert_eq!(after.cost_usd, amount("1030"));
    }

    #[test]
    fn a_receipt_token_of_a_deposit_is_not_counted_twice() {
        let a_usdc = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xaaaa"));
        let book = Book::default()
            .with(&usdc(), "2026-03-01", "1")
            .with(&usdc(), "2026-03-02", "1")
            .with(&a_usdc, "2026-03-02", "1");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, usdc(), "1000", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Stake,
                vec![
                    leg(CryptoDirection::Out, usdc(), "1000", POOL),
                    leg(CryptoDirection::In, a_usdc.clone(), "1000", POOL),
                ],
            ),
        ];
        let ledger = ledger_of(&events, &[ME], &book);
        assert_eq!(state(&ledger, &usdc()).in_positions, amount("1000"));
        assert_eq!(
            ledger.assets.get(&a_usdc.key()).map(|have| have.owned()),
            None,
            "the receipt is the position, not a second holding"
        );
    }

    #[test]
    fn gas_is_a_cost_of_its_own_and_not_a_trading_profit() {
        let book =
            Book::default()
                .with(&eth(), "2026-03-01", "1000")
                .with(&eth(), "2026-03-02", "2000");
        let mut spend = event("ev-2", "2026-03-02", CryptoTxKind::Contract, Vec::new());
        spend.gas = Some(CryptoFee {
            asset: eth(),
            amount: amount("0.1"),
        });
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "1", OTHER)],
            ),
            spend,
        ];
        let ledger = ledger_of(&events, &[ME], &book);
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.quantity, amount("0.9"));
        assert_eq!(
            eth_state.cost_usd,
            amount("900"),
            "cost left with the coins"
        );
        assert_eq!(eth_state.gas_usd, amount("200"), "what the gas was worth");
        assert_eq!(
            eth_state.realized_usd,
            amount("100"),
            "the gain the burnt coins carried is realized, the gas is not profit"
        );
        // Every dollar accounted once: 0.9 ETH at 2000 is 1800.
        let value = amount("1800");
        let total = eth_state.realized_usd + (value - eth_state.cost_usd) - eth_state.gas_usd;
        assert_eq!(total, amount("800"));
    }

    #[test]
    fn a_claim_is_income_at_the_price_of_the_day() {
        let book = Book::default().with(&usdc(), "2026-03-01", "2");
        let events = vec![event(
            "ev-1",
            "2026-03-01",
            CryptoTxKind::Claim,
            vec![leg(CryptoDirection::In, usdc(), "50", POOL)],
        )];
        let ledger = ledger_of(&events, &[ME], &book);
        let token = state(&ledger, &usdc());
        assert_eq!(token.income_usd, amount("100"));
        assert_eq!(token.cost_usd, amount("100"));
        assert_eq!(token.realized_usd, Amount::ZERO);
    }

    #[test]
    fn a_missing_price_is_never_a_zero_and_the_asset_says_so() {
        let book = Book::default().with(&eth(), "2026-03-02", "2000");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "1", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::Out, eth(), "1", OTHER)],
            ),
        ];
        let ledger = ledger_of(&events, &[ME], &book);
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.cost_usd, Amount::ZERO, "no cost was ever known");
        assert_eq!(eth_state.priced_quantity, Amount::ZERO);
        assert_eq!(eth_state.average_usd(), None, "not zero: unknown");
        assert!(eth_state.doubts.contains(&Doubt::MissingPrice));
        assert!(eth_state.imprecise());
    }

    #[test]
    fn disposing_of_everything_leaves_no_cost_behind() {
        // A cost that does not divide evenly by the quantity: the residue
        // must not stay in the pool as a phantom basis.
        let book = Book::default()
            .with(&eth(), "2026-03-01", "100")
            .with(&eth(), "2026-03-02", "100")
            .with(&eth(), "2026-03-03", "100");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "3", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::Out, eth(), "1", OTHER)],
            ),
            event(
                "ev-3",
                "2026-03-03",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::Out, eth(), "2", OTHER)],
            ),
        ];
        let ledger = ledger_of(&events, &[ME], &book);
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.quantity, Amount::ZERO);
        assert_eq!(eth_state.cost_usd, Amount::ZERO);
        assert_eq!(eth_state.priced_quantity, Amount::ZERO);
        assert_eq!(eth_state.realized_usd, Amount::ZERO);
        assert_eq!(eth_state.average_usd(), None);
    }

    #[test]
    fn a_junk_quantity_marks_the_asset_instead_of_panicking() {
        let junk = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xbeef"));
        let book = Book::default().with(&junk, "2026-03-01", "1000");
        let events = vec![event(
            "ev-1",
            "2026-03-01",
            CryptoTxKind::Transfer,
            vec![leg(
                CryptoDirection::In,
                junk.clone(),
                "1000000000000000000000000000",
                OTHER,
            )],
        )];
        let ledger = ledger_of(&events, &[ME], &book);
        let state = state(&ledger, &junk);
        assert!(state.doubts.contains(&Doubt::TooLarge));
        assert_eq!(state.cost_usd, Amount::ZERO);
        assert_eq!(state.quantity, amount("1000000000000000000000000000"));
    }

    #[test]
    fn selling_more_than_ever_arrived_is_marked() {
        let book = Book::default().with(&eth(), "2026-03-01", "1000");
        let events = vec![event(
            "ev-1",
            "2026-03-01",
            CryptoTxKind::Transfer,
            vec![leg(CryptoDirection::Out, eth(), "1", OTHER)],
        )];
        let ledger = ledger_of(&events, &[ME], &book);
        let eth_state = state(&ledger, &eth());
        assert!(eth_state.quantity.is_negative());
        assert!(eth_state.doubts.contains(&Doubt::MoreLeftThanArrived));
        assert!(eth_state.imprecise());
    }

    #[test]
    fn a_bridge_out_realizes_nothing_and_says_the_coins_left_the_module() {
        let book =
            Book::default()
                .with(&eth(), "2026-03-01", "1000")
                .with(&eth(), "2026-03-02", "2000");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "1", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Bridge,
                vec![leg(CryptoDirection::Out, eth(), "1", POOL)],
            ),
        ];
        let ledger = ledger_of(&events, &[ME], &book);
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.realized_usd, Amount::ZERO);
        assert_eq!(eth_state.quantity, Amount::ZERO);
        assert_eq!(eth_state.cost_usd, Amount::ZERO, "the cost left with them");
        assert!(eth_state.doubts.contains(&Doubt::LeftTheModule));
    }

    #[test]
    fn a_token_that_does_not_count_is_not_in_the_ledger_at_all() {
        let scam = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xdeadbeef"));
        let book =
            Book::default()
                .with(&scam, "2026-03-01", "1000000")
                .with(&eth(), "2026-03-01", "1000");
        let events = vec![event(
            "ev-1",
            "2026-03-01",
            CryptoTxKind::Swap,
            vec![
                leg(CryptoDirection::In, scam.clone(), "1000", OTHER),
                leg(CryptoDirection::Out, eth(), "1", OTHER),
            ],
        )];
        let views = super::super::overrides::apply(&events, &[]);
        let own = own(&[ME]);
        let counts = |asset: &CryptoAsset| *asset != scam;
        let ledger = fold(
            &views,
            &Inputs {
                own: &own,
                prices: &book,
                counts: &counts,
            },
        );
        assert!(!ledger.assets.contains_key(&scam.key()));
        // What was really given away is still gone. No profit is booked for
        // it, though: the module never saw it arrive, so its cost is unknown
        // and a profit of the full proceeds would be invented. The asset says
        // as much.
        let eth_state = state(&ledger, &eth());
        assert_eq!(eth_state.quantity, amount("-1"));
        assert_eq!(eth_state.realized_usd, Amount::ZERO);
        assert!(eth_state.doubts.contains(&Doubt::MissingPrice));
        assert!(eth_state.doubts.contains(&Doubt::MoreLeftThanArrived));
    }

    #[test]
    fn the_order_the_batches_arrived_in_does_not_change_a_total() {
        let book = Book::default()
            .with(&eth(), "2026-03-01", "1000")
            .with(&eth(), "2026-03-02", "2000")
            .with(&eth(), "2026-03-03", "2500")
            .with(&usdc(), "2026-03-02", "1")
            .with(&usdc(), "2026-03-03", "1");
        let events = vec![
            event(
                "ev-1",
                "2026-03-01",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::In, eth(), "2", OTHER)],
            ),
            event(
                "ev-2",
                "2026-03-02",
                CryptoTxKind::Swap,
                vec![
                    leg(CryptoDirection::Out, eth(), "1", OTHER),
                    leg(CryptoDirection::In, usdc(), "2000", OTHER),
                ],
            ),
            event(
                "ev-3",
                "2026-03-03",
                CryptoTxKind::Transfer,
                vec![leg(CryptoDirection::Out, usdc(), "500", OTHER)],
            ),
        ];
        let straight = ledger_of(&events, &[ME], &book);
        // Every order of the three batches gives the same ledger.
        for first in 0..3 {
            for second in 0..3 {
                for third in 0..3 {
                    if first == second || second == third || first == third {
                        continue;
                    }
                    let shuffled = vec![
                        events[first].clone(),
                        events[second].clone(),
                        events[third].clone(),
                    ];
                    assert_eq!(ledger_of(&shuffled, &[ME], &book), straight);
                }
            }
        }
    }
}
