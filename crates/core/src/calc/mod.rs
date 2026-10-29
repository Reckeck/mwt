//! Preview calculations behind the wallet calculator.
//!
//! Pure and vault-independent on purpose: the UI sends raw inputs and gets
//! ready-to-render numbers back, so all decimal arithmetic stays in
//! [`Amount`](crate::amount::Amount) instead of drifting into `f64` on the
//! TypeScript side.
//!
//! Assumption: the commission is charged in the base currency. In `Converter`
//! it is deducted from the proceeds (`net_out` is net of it); in `Position` it
//! comes out of the amount given, so fewer units are bought.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::amount::Amount;

pub mod invest;

/// Percent scaling constant (`fee_pct` / `roi_pct` are percentages).
const HUNDRED: Amount = Amount(Decimal::ONE_HUNDRED);

/// Which set of inputs/outputs [`preview`] should compute.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum CalcMode {
    /// `amount` of one asset priced into another.
    #[default]
    Converter,
    /// A position opened with `amount` of the quote at `price_in`: the units
    /// bought, the take-profit result and the stop-loss risk.
    Position,
}

/// Raw calculator inputs. The UI is expected to pass empty/zero values while
/// the user is still typing — every field is optional in practice.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CalcInput {
    pub mode: CalcMode,
    /// Source asset symbol, display only.
    pub from_symbol: String,
    /// Target asset symbol, display only.
    pub to_symbol: String,
    /// `Converter`: amount to convert. `Position`: the quote given for the
    /// position ("You give 1 000 USDT"), commission included.
    pub amount: Amount,
    /// Price of one unit of the source asset. In `Position` mode this is the
    /// entry price of the bought asset, in the quote.
    pub price_in: Amount,
    /// `Converter` only: price of one unit of the target asset.
    pub price_out: Amount,
    /// Take-profit price (`Position` only).
    pub target_price: Option<Amount>,
    /// Stop-loss price (`Position` only).
    pub stop_price: Option<Amount>,
    /// Round-trip commission in percent (`"0.1"` = 0.1%).
    pub fee_pct: Amount,
}

/// Ready-to-render result. Fields that do not apply to the chosen mode stay
/// zero; the UI shows an em dash for them.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CalcResult {
    /// `Converter`: `amount × price_in` — the notional put in.
    /// `Position`: `amount − fee` — the part that buys units.
    pub gross_in: Amount,
    /// `Converter`: `gross_in × fee_pct / 100`. `Position`: `amount × fee_pct / 100`.
    pub fee: Amount,
    /// What the deal really costs: `gross_in + fee` (`Position`: `amount`).
    pub spent: Amount,
    /// `Position`: units bought, `gross_in / price_in`.
    #[serde(default)]
    pub quantity: Amount,
    /// `Converter`: `amount × price_out` before fees.
    pub gross_out: Amount,
    /// `Converter`: `gross_out − fee`.
    pub net_out: Amount,
    /// `Position`: `quantity × target_price`.
    pub exit_value: Amount,
    /// `Position`: `exit_value − spent`.
    pub pnl: Amount,
    /// `Position`: `pnl / spent × 100`.
    pub roi_pct: Amount,
    /// `Position`: `spent − quantity × stop_price`, floored at zero.
    pub risk: Amount,
    /// `Position`: `risk / spent × 100`.
    pub risk_pct: Amount,
    /// `Position`: `pnl / risk` — «1 : 2,2» reads as `2.2`. Zero without a
    /// risk or a profit.
    #[serde(default)]
    pub reward_risk: Amount,
}

/// Ratio guard: `0` when the divisor is not positive.
fn ratio(num: Amount, den: Amount) -> Amount {
    if den.is_zero() || den.is_negative() {
        return Amount::ZERO;
    }
    (num / den * HUNDRED).round(8)
}

/// Compute a calculator preview. Never panics and never divides by zero.
pub fn preview(input: &CalcInput) -> CalcResult {
    let amount = input.amount.abs();
    let price_in = input.price_in.abs();
    let price_out = input.price_out.abs();
    let fee_pct = input.fee_pct.abs();

    if amount.is_zero() || price_in.is_zero() {
        return CalcResult::default();
    }

    match input.mode {
        CalcMode::Converter => converter(amount, price_in, price_out, fee_pct),
        CalcMode::Position => position(input, amount, price_in, fee_pct),
    }
}

fn converter(amount: Amount, price_in: Amount, price_out: Amount, fee_pct: Amount) -> CalcResult {
    let gross_in = (amount * price_in).round(8);
    let fee = (gross_in * fee_pct / HUNDRED).round(8);
    let spent = (gross_in + fee).round(8);
    if price_out.is_zero() {
        return CalcResult {
            gross_in,
            fee,
            spent,
            ..CalcResult::default()
        };
    }
    let gross_out = (amount * price_out).round(8);
    CalcResult {
        gross_in,
        fee,
        spent,
        gross_out,
        net_out: (gross_out - fee).round(8),
        ..CalcResult::default()
    }
}

/// `amount` of the quote buys units at `price_in`; the commission comes out of it.
fn position(input: &CalcInput, amount: Amount, price_in: Amount, fee_pct: Amount) -> CalcResult {
    let fee = (amount * fee_pct / HUNDRED).round(8);
    let gross_in = (amount - fee).max(Amount::ZERO).round(8);
    let quantity = (gross_in / price_in).round(8);
    let mut out = CalcResult {
        gross_in,
        fee,
        spent: amount,
        quantity,
        ..CalcResult::default()
    };
    if let Some(target) = input.target_price.filter(|price| price.is_positive()) {
        out.exit_value = (quantity * target.abs()).round(8);
        out.pnl = (out.exit_value - amount).round(8);
        out.roi_pct = ratio(out.pnl, amount);
    }
    if let Some(stop) = input.stop_price.filter(|price| price.is_positive()) {
        out.risk = (amount - quantity * stop.abs()).max(Amount::ZERO).round(8);
        out.risk_pct = ratio(out.risk, amount);
    }
    if out.risk.is_positive() && out.pnl.is_positive() {
        out.reward_risk = (out.pnl / out.risk).round(2);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amt(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    /// Decimal keeps its scale in `Display` ("40" vs "40.0"), so compare by value.
    fn assert_amt(actual: &Amount, expected: &str) {
        assert_eq!(*actual, amt(expected), "expected {expected}, got {actual}");
    }

    #[test]
    fn converter_applies_fee() {
        let res = preview(&CalcInput {
            mode: CalcMode::Converter,
            amount: amt("2"),
            price_in: amt("20000"),
            price_out: amt("30000"),
            fee_pct: amt("0.1"),
            ..CalcInput::default()
        });
        assert_amt(&res.gross_in, "40000");
        assert_amt(&res.fee, "40");
        assert_amt(&res.spent, "40040");
        assert_amt(&res.gross_out, "60000");
        assert_amt(&res.net_out, "59960");
    }

    #[test]
    fn position_take_profit() {
        let res = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("2000"),
            price_in: amt("1000"),
            target_price: Some(amt("1500")),
            fee_pct: amt("0"),
            ..CalcInput::default()
        });
        assert_amt(&res.spent, "2000");
        assert_amt(&res.quantity, "2");
        assert_amt(&res.exit_value, "3000");
        assert_amt(&res.pnl, "1000");
        assert_amt(&res.roi_pct, "50");
    }

    #[test]
    fn position_fee_buys_fewer_units() {
        let res = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("1000"),
            price_in: amt("100"),
            fee_pct: amt("1"),
            ..CalcInput::default()
        });
        assert_amt(&res.fee, "10");
        assert_amt(&res.gross_in, "990");
        assert_amt(&res.quantity, "9.9");
        assert_amt(&res.spent, "1000");
    }

    #[test]
    fn position_stop_loss_risk() {
        let res = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("2000"),
            price_in: amt("1000"),
            target_price: Some(amt("1300")),
            stop_price: Some(amt("900")),
            ..CalcInput::default()
        });
        assert_amt(&res.risk, "200");
        assert_amt(&res.risk_pct, "10");
        assert_amt(&res.reward_risk, "3");
        // a stop above entry must not produce negative risk
        let up = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("2000"),
            price_in: amt("1000"),
            stop_price: Some(amt("1200")),
            ..CalcInput::default()
        });
        assert_amt(&up.risk, "0");
        assert_amt(&up.reward_risk, "0");
    }

    #[test]
    fn zero_price_is_safe() {
        let res = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("1"),
            price_in: amt("0"),
            target_price: Some(amt("1200")),
            ..CalcInput::default()
        });
        assert_eq!(res, CalcResult::default());
    }
}
