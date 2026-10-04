//! Preview calculations behind the wallet calculator.
//!
//! Pure and vault-independent on purpose: the UI sends raw inputs and gets
//! ready-to-render numbers back, so all decimal arithmetic stays in
//! [`Amount`](crate::amount::Amount) instead of drifting into `f64` on the
//! TypeScript side.
//!
//! Assumption: the commission is charged in the base currency and is deducted
//! from the proceeds (`net_out` / `exit_value` are net of it).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::amount::Amount;

/// Percent scaling constant (`fee_pct` / `roi_pct` are percentages).
const HUNDRED: Amount = Amount(Decimal::ONE_HUNDRED);

/// Which set of inputs/outputs [`preview`] should compute.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum CalcMode {
    /// `amount` of one asset priced into another.
    #[default]
    Converter,
    /// An open position: entry cost, take-profit result and stop-loss risk.
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
    /// `Converter`: amount to convert. `Position`: position size.
    pub amount: Amount,
    /// Price of one unit of the source asset. In `Position` mode this is the
    /// entry price of the position.
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
    /// `amount × price_in` — the notional put in.
    pub gross_in: Amount,
    /// `gross_in × fee_pct / 100`.
    pub fee: Amount,
    /// `gross_in + fee` — what the position really costs.
    pub spent: Amount,
    /// `Converter`: `amount × price_out` before fees.
    pub gross_out: Amount,
    /// `Converter`: `gross_out − fee`.
    pub net_out: Amount,
    /// `Position`: `amount × target_price`.
    pub exit_value: Amount,
    /// `Position`: `exit_value − spent`.
    pub pnl: Amount,
    /// `Position`: `pnl / spent × 100`.
    pub roi_pct: Amount,
    /// `Position`: `amount × (price_in − stop_price)`, floored at zero.
    pub risk: Amount,
    /// `Position`: `risk / spent × 100`.
    pub risk_pct: Amount,
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

    let gross_in = (amount * price_in).round(8);
    let fee = (gross_in * fee_pct / HUNDRED).round(8);
    let spent = (gross_in + fee).round(8);

    match input.mode {
        CalcMode::Converter => {
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
        CalcMode::Position => {
            let mut out = CalcResult {
                gross_in,
                fee,
                spent,
                ..CalcResult::default()
            };
            if let Some(target) = input.target_price.filter(|price| price.is_positive()) {
                out.exit_value = (amount * target.abs()).round(8);
                out.pnl = (out.exit_value - spent).round(8);
                out.roi_pct = ratio(out.pnl, spent);
            }
            if let Some(stop) = input.stop_price.filter(|price| price.is_positive()) {
                let per_unit = price_in - stop.abs();
                out.risk = (amount * per_unit.max(Amount::ZERO)).round(8);
                out.risk_pct = ratio(out.risk, spent);
            }
            out
        }
    }
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
            amount: amt("2"),
            price_in: amt("1000"),
            target_price: Some(amt("1500")),
            fee_pct: amt("0"),
            ..CalcInput::default()
        });
        assert_amt(&res.spent, "2000");
        assert_amt(&res.exit_value, "3000");
        assert_amt(&res.pnl, "1000");
        assert_amt(&res.roi_pct, "50");
    }

    #[test]
    fn position_stop_loss_risk() {
        let res = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("2"),
            price_in: amt("1000"),
            stop_price: Some(amt("900")),
            ..CalcInput::default()
        });
        assert_amt(&res.risk, "200");
        assert_amt(&res.risk_pct, "10");
        // a stop above entry must not produce negative risk
        let up = preview(&CalcInput {
            mode: CalcMode::Position,
            amount: amt("2"),
            price_in: amt("1000"),
            stop_price: Some(amt("1200")),
            ..CalcInput::default()
        });
        assert_amt(&up.risk, "0");
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
