//! What a position earned, as pure arithmetic (spec D0).
//!
//! Everything here is a function of numbers. No document, no clock, no
//! network — which is what lets the reference examples below be worked out by
//! hand and pinned in a test.
//!
//! # The identity
//!
//! One position's whole result is
//!
//! ```text
//! net = (value_now - invested) + fees_collected + fees_uncollected - gas
//! ```
//!
//! and nothing else. `value_now - invested` is the **value change**, and both
//! of the figures the screen shows next to it live *inside* it:
//!
//! * **impermanent loss against holding** ([`impermanent_loss`]) is how much
//!   worse the pool did than keeping the deposited coins. The pool's value is
//!   already `value_now`, so adding the loss to the net a second time would
//!   count it twice. It is informational, always.
//! * **what the protocol accrued** — an aToken's interest, stETH's rebase —
//!   grew the position's own balance, so it is already `value_now` too. Also
//!   informational.
//!
//! Fees are different: collected fees have *left* the position (so they are
//! not in `value_now` any more), and a protocol that keeps the earned fees
//! apart from the principal (Uniswap v3 and v4 do) reports the uncollected
//! ones outside the legs. Both are addends, exactly once.
//! [`net_result`] is the one place that spells the identity out, and
//! `the_net_result_counts_impermanent_loss_only_once` is the test that holds
//! it.
//!
//! # Nothing unknown becomes a zero
//!
//! Every figure is an `Option`, and `None` means "not known", never zero — the
//! screen prints a dash for it (decision of the D-stage mockups). A genuine
//! zero is a `Some(0)`: a position that earned no fees at all says so, and is
//! told apart from one whose fees nobody could read. `None` therefore
//! propagates: a net result that silently dropped an unknown gas bill would
//! read as a profit the user does not have.

use rust_decimal::Decimal;

use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::model::cryptotrack::CryptoAsset;

use super::position::AssetAmount;

/// Seconds in the year an APR is quoted against: 365 days, no leap day and no
/// compounding. An APR is a rate, not what the position will actually pay.
pub const SECONDS_PER_YEAR: i64 = 365 * 24 * 60 * 60;

/// The fees of one position, as the protocol reports them, in whole units of
/// each asset.
///
/// `None` is "the adapter could not read them" and reaches the screen as a
/// dash; an empty list is a real zero. Telling the two apart is the whole
/// reason this is not a `Vec`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeeReading {
    /// Fees the user has already taken out of the position.
    pub collected: Option<Vec<AssetAmount>>,
    /// Fees the position has earned and still holds, apart from its
    /// principal.
    pub uncollected: Option<Vec<AssetAmount>>,
}

impl FeeReading {
    /// Neither side read: what an adapter answers for a protocol that has no
    /// fees of its own (lending, staking) as well as for one it could not
    /// read.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// Both sides read as empty: the position really earned no fees.
    pub fn none_earned() -> Self {
        Self {
            collected: Some(Vec::new()),
            uncollected: Some(Vec::new()),
        }
    }
}

/// What a list of quantities is worth, in USD.
///
/// `None` as soon as one price is unknown: a partial sum would read as a
/// smaller fee rather than as a gap. An empty list is worth a real zero.
pub fn worth_usd(
    items: &[AssetAmount],
    unit_usd: &dyn Fn(&CryptoAsset) -> Option<Amount>,
) -> Option<Amount> {
    let mut total = Amount::ZERO;
    for item in items {
        let price = unit_usd(&item.asset)?;
        total = math::add(total, math::mul(item.amount, price)?)?;
    }
    Some(total)
}

/// The same for a reading that may not have been taken at all.
pub fn fees_usd(
    side: Option<&Vec<AssetAmount>>,
    unit_usd: &dyn Fn(&CryptoAsset) -> Option<Amount>,
) -> Option<Amount> {
    worth_usd(side?, unit_usd)
}

/// Impermanent loss against holding: what the pool's principal is worth now,
/// minus what the deposited quantities would be worth now if they had simply
/// been kept.
///
/// Negative is the usual case and means the pool did worse than holding.
/// **Informational**: the pool's value is already the position's value, so
/// this is a part of the value change and never an addend of
/// [`net_result`].
///
/// Both sides must be the principal only — fees are not in either, or the
/// figure would flatter the pool.
pub fn impermanent_loss(pool_value_usd: Amount, hold_value_usd: Amount) -> Option<Amount> {
    math::sub(pool_value_usd, hold_value_usd)
}

/// The same loss as a share of what holding would have been worth; `None`
/// when that is zero, because a share of nothing is not a number.
pub fn impermanent_loss_ratio(pool_value_usd: Amount, hold_value_usd: Amount) -> Option<Amount> {
    math::div(
        impermanent_loss(pool_value_usd, hold_value_usd)?,
        hold_value_usd,
    )
}

/// The simple annual rate of a position: what it returned on what was put
/// in, stretched to a year.
///
/// ```text
/// apr = gain / put in * SECONDS_PER_YEAR / held
/// ```
///
/// `put_in_usd` is the capital ever put in, at the price of each deposit's
/// own day — **not** what is still in. A withdrawal lowers what is still in,
/// and a position closed at a profit has less than nothing left in it, so a
/// rate on that figure would read as a loss on a position that earned
/// (`service::cryptotrack::positions`, `PositionHistory::put_in_usd`).
///
/// A fraction, not a percent: `0.125` is 12.5 %. Simple, not compounded — an
/// APR, deliberately not an APY, because a position's fees are not reinvested
/// by the protocol on the user's behalf.
///
/// `None` when nothing was put in (a rate on a zero stake is not a number) or
/// when no time has passed, which is also what a position read in the same
/// second it was opened answers.
pub fn annual_percentage_rate(
    gain_usd: Amount,
    put_in_usd: Amount,
    held_secs: i64,
) -> Option<Amount> {
    if held_secs <= 0 {
        return None;
    }
    let rate = math::div(gain_usd, put_in_usd)?;
    let year = Amount::new(Decimal::from(SECONDS_PER_YEAR));
    let held = Amount::new(Decimal::from(held_secs));
    math::mul(rate, math::div(year, held)?)
}

/// The parts the net result is made of. Every one of them is `None` when it
/// is not known, and then so is the result.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NetParts {
    /// What the position is worth now: collateral minus debt, principal only.
    pub value_usd: Option<Amount>,
    /// What went in, at the price of the day it went in.
    pub invested_usd: Option<Amount>,
    /// Fees already taken out, at the price of the day they were taken.
    pub fees_collected_usd: Option<Amount>,
    /// Fees the position still holds, at today's price.
    pub fees_uncollected_usd: Option<Amount>,
    /// What the network charged for the operations of this position.
    pub gas_usd: Option<Amount>,
}

/// The whole result of a position, by the identity at the top of this file.
///
/// Impermanent loss and the protocol's accruals are **not** arguments: they
/// are already inside `value_usd - invested_usd`, and this is the function
/// that refuses to count them twice.
pub fn net_result(parts: &NetParts) -> Option<Amount> {
    let change = math::sub(parts.value_usd?, parts.invested_usd?)?;
    let with_collected = math::add(change, parts.fees_collected_usd?)?;
    let with_fees = math::add(with_collected, parts.fees_uncollected_usd?)?;
    math::sub(with_fees, parts.gas_usd?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::TokenId;

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount parses")
    }

    const DAY: i64 = 24 * 60 * 60;

    fn usdc() -> CryptoAsset {
        CryptoAsset::Token(TokenId::new(
            Chain::Ethereum,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        ))
    }

    fn ether() -> CryptoAsset {
        CryptoAsset::Native {
            chain: Chain::Ethereum,
        }
    }

    #[test]
    fn impermanent_loss_matches_the_hand_worked_two_times_pool() {
        // Reference example, worked out by hand.
        //
        // Deposit into a 50/50 constant-product pool at ETH = 2000 USD:
        //   1 ETH + 2000 USDC, so 2000 + 2000 = 4000 USD invested, k = 1 * 2000 = 2000.
        // ETH then doubles to 4000 USD.
        //
        // Holding instead:  1 * 4000 + 2000 * 1            = 6000.000000 USD
        // In the pool:      x * y = 2000 and y / x = 4000
        //                   x = sqrt(2000 / 4000) = sqrt(0.5) = 0.7071067811865475
        //                   y = 2000 / x          = 2828.4271247461902
        //                   value = x * 4000 + y  = 2828.4271247461900 + 2828.4271247461902
        //                                         = 5656.8542494923802, i.e. 5656.8542 to 4 dp
        //
        // IL = 5656.8542 - 6000 = -343.1458 USD
        // as a share of holding: -343.1458 / 6000 = -0.0571909666…, i.e. -0.057191 to 6 dp
        //
        // The textbook figure for a 2x price move is 2 * sqrt(2) / (1 + 2) - 1
        //   = 2.8284271247461903 / 3 - 1 = -0.0571909584…, which is the same
        //   number to the precision the 4-dp pool value above carries.
        let pool = amount("5656.8542");
        let hold = amount("6000.0000");
        assert_eq!(impermanent_loss(pool, hold), Some(amount("-343.1458")));
        assert_eq!(
            impermanent_loss_ratio(pool, hold).map(|ratio| ratio.round(6)),
            Some(amount("-0.057191"))
        );
    }

    #[test]
    fn a_pool_that_kept_up_with_holding_has_no_loss_and_no_ratio_on_nothing() {
        // A real zero, not an absent value: the screen shows 0, not a dash.
        assert_eq!(
            impermanent_loss(amount("6000"), amount("6000")),
            Some(Amount::ZERO)
        );
        // Nothing to compare against: a share of a zero hold value is not a
        // number, so it is absent rather than zero.
        assert_eq!(impermanent_loss_ratio(amount("10"), Amount::ZERO), None);
    }

    #[test]
    fn apr_matches_the_hand_worked_quarter_of_a_year() {
        // Reference example, worked out by hand.
        //
        // 10 000 USD in, 250 USD earned, held 73 days.
        //   73 days is exactly a fifth of a 365-day year, so the annualizing
        //   factor is 365 / 73 = 5 exactly.
        //   apr = 250 / 10000 * 5 = 0.025 * 5 = 0.125, i.e. 12.5 %.
        assert_eq!(
            annual_percentage_rate(amount("250"), amount("10000"), 73 * DAY),
            Some(amount("0.125"))
        );
    }

    #[test]
    fn apr_of_a_loss_is_negative_and_hand_worked_too() {
        // Reference example, worked out by hand.
        //
        // The same 4000 USD pool as the IL example, now 182.5 days in and
        // 343.1458 USD down.
        //   182.5 days is half a 365-day year, so the factor is 2 exactly.
        //   343.1458 / 4000 = 0.08578645
        //   apr = -0.08578645 * 2 = -0.1715729, i.e. -17.15729 %.
        let half_a_year = 182 * DAY + DAY / 2;
        assert_eq!(
            annual_percentage_rate(amount("-343.1458"), amount("4000"), half_a_year),
            Some(amount("-0.1715729"))
        );
    }

    #[test]
    fn apr_on_nothing_and_in_no_time_is_absent_not_zero() {
        assert_eq!(
            annual_percentage_rate(amount("10"), Amount::ZERO, 30 * DAY),
            None
        );
        assert_eq!(
            annual_percentage_rate(amount("10"), amount("1000"), 0),
            None
        );
        assert_eq!(
            annual_percentage_rate(amount("10"), amount("1000"), -5),
            None
        );
        // A position that earned nothing has a real zero rate.
        assert_eq!(
            annual_percentage_rate(Amount::ZERO, amount("1000"), 73 * DAY),
            Some(Amount::ZERO)
        );
    }

    #[test]
    fn the_net_result_counts_impermanent_loss_only_once() {
        // Reference example, worked out by hand.
        //
        // The 4000 USD pool of the IL example: it is now worth 5656.8542 while
        // holding would have been worth 6000, it paid out 120 USD of fees and
        // holds another 30 USD of them, and the mint and the collect cost
        // 18.50 USD of gas.
        //
        //   value change     = 5656.8542 - 4000      = 1656.8542
        //   + fees collected                         =  120.0000  -> 1776.8542
        //   + fees uncollected                       =   30.0000  -> 1806.8542
        //   - gas                                    =   18.5000  -> 1788.3542
        //
        // The impermanent loss of -343.1458 is already inside the value
        // change (the pool's value is 5656.8542 *because* of it), so the net
        // result must not mention it. Adding it again would give 1445.2084,
        // which is the bug this test exists to catch.
        let parts = NetParts {
            value_usd: Some(amount("5656.8542")),
            invested_usd: Some(amount("4000")),
            fees_collected_usd: Some(amount("120")),
            fees_uncollected_usd: Some(amount("30")),
            gas_usd: Some(amount("18.50")),
        };
        let net = net_result(&parts).expect("every part is known");
        assert_eq!(net, amount("1788.3542"));
        let loss = impermanent_loss(amount("5656.8542"), amount("6000")).expect("a loss");
        assert_ne!(
            net,
            math::add(net, loss).expect("the wrong sum"),
            "the loss must not be an addend of the net result"
        );
    }

    #[test]
    fn a_debt_position_nets_what_is_left_after_the_loan() {
        // Collateral 10 000 USD, debt 4000 USD, so the position is worth
        // 6000; 5000 went in, nothing was ever collected, 12 USD of gas.
        //   6000 - 5000 + 0 + 0 - 12 = 988
        let parts = NetParts {
            value_usd: Some(amount("6000")),
            invested_usd: Some(amount("5000")),
            fees_collected_usd: Some(Amount::ZERO),
            fees_uncollected_usd: Some(Amount::ZERO),
            gas_usd: Some(amount("12")),
        };
        assert_eq!(net_result(&parts), Some(amount("988")));
    }

    #[test]
    fn an_unknown_part_leaves_the_net_result_unknown() {
        let known = NetParts {
            value_usd: Some(amount("100")),
            invested_usd: Some(amount("90")),
            fees_collected_usd: Some(Amount::ZERO),
            fees_uncollected_usd: Some(Amount::ZERO),
            gas_usd: Some(Amount::ZERO),
        };
        assert_eq!(net_result(&known), Some(amount("10")));
        for blank in [
            NetParts {
                value_usd: None,
                ..known
            },
            NetParts {
                invested_usd: None,
                ..known
            },
            NetParts {
                fees_collected_usd: None,
                ..known
            },
            NetParts {
                fees_uncollected_usd: None,
                ..known
            },
            NetParts {
                gas_usd: None,
                ..known
            },
        ] {
            assert_eq!(
                net_result(&blank),
                None,
                "an unknown part must not be read as a zero"
            );
        }
    }

    #[test]
    fn fees_are_worth_nothing_known_when_one_price_is_missing() {
        let items = vec![
            AssetAmount {
                asset: usdc(),
                amount: amount("120"),
            },
            AssetAmount {
                asset: ether(),
                amount: amount("0.05"),
            },
        ];
        let both = |asset: &CryptoAsset| match asset {
            CryptoAsset::Native { .. } => Some(amount("4000")),
            CryptoAsset::Token(_) => Some(amount("1")),
        };
        // 120 * 1 + 0.05 * 4000 = 120 + 200 = 320
        assert_eq!(worth_usd(&items, &both), Some(amount("320")));
        let only_token = |asset: &CryptoAsset| match asset {
            CryptoAsset::Native { .. } => None,
            CryptoAsset::Token(_) => Some(amount("1")),
        };
        assert_eq!(worth_usd(&items, &only_token), None);
        // No fees at all is a real zero.
        assert_eq!(worth_usd(&[], &both), Some(Amount::ZERO));
    }

    #[test]
    fn a_fee_side_nobody_read_is_absent_and_an_empty_one_is_zero() {
        let any = |_: &CryptoAsset| Some(amount("1"));
        let unknown = FeeReading::unknown();
        assert_eq!(fees_usd(unknown.collected.as_ref(), &any), None);
        assert_eq!(fees_usd(unknown.uncollected.as_ref(), &any), None);
        let zero = FeeReading::none_earned();
        assert_eq!(fees_usd(zero.collected.as_ref(), &any), Some(Amount::ZERO));
        assert_eq!(
            fees_usd(zero.uncollected.as_ref(), &any),
            Some(Amount::ZERO)
        );
    }
}
