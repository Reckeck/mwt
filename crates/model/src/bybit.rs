//! The data of the Bybit exchange port: what a read-only Bybit account hands back (fills, moves
//! between its own accounts, Earn payouts) and the trade-form prefill a fill maps to. Plain data
//! and pure mapping, so the crypto module can name them while `mwt-net` keeps the HTTP client.

use serde::{Deserialize, Serialize};

/// Product type. Bybit requires this on most private endpoints.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", ts(rename = "BybitCategory"))]
pub enum BybitCategory {
    #[default]
    Spot,
    Linear,
    Inverse,
}

impl BybitCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spot => "spot",
            Self::Linear => "linear",
            Self::Inverse => "inverse",
        }
    }
}

/// A single fill from `GET /v5/execution/list`.
///
/// Field names mirror Bybit's response so the mapping stays auditable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub exec_price: String,
    pub exec_qty: String,
    pub exec_value: String,
    pub exec_fee: String,
    pub exec_time: String,
    pub is_maker: bool,
    pub closed_size: Option<String>,
    /// Bybit's unique execution ID (execId) for deduplication.
    pub exec_id: String,
    /// `spot` | `linear` | `inverse`: the category the fill was fetched under.
    /// Bybit does not echo it per row, so the client stamps it.
    #[serde(default)]
    pub category: String,
}

impl Execution {
    /// Split `BTCUSDT` into `(BTC, USDT)`.
    ///
    /// Bybit concatenates base and quote with no separator, so the quote leg
    /// is matched against the known USDT/USDC suffixes first and only then
    /// guessed from the remainder.
    pub fn split_symbol(symbol: &str) -> Option<(String, String)> {
        let upper = symbol.to_ascii_uppercase();
        for quote in ["USDT", "USDC", "USD", "BTC", "ETH"] {
            if let Some(base) = upper.strip_suffix(quote)
                && !base.is_empty()
                && base != quote
            {
                return Some((base.to_string(), quote.to_string()));
            }
        }
        // Perpetuals look like `BTCUSDT` but are `BTC`-margined; fall back to
        // treating the whole symbol as one leg rather than inventing a pair.
        None
    }

    /// `true` when the fill closed (reduced) a derivatives position.
    pub fn is_closing(&self) -> bool {
        self.closed_size
            .as_ref()
            .is_some_and(|size| !size.is_empty() && size != "0")
    }

    /// The product category: the one the fill was fetched under, else a
    /// guess from the symbol (Bybit names USDC perpetuals `…PERP`). The guess
    /// used to be all there was, and booked every USDT perpetual as spot.
    pub fn category_hint(&self) -> &str {
        match self.category.as_str() {
            "spot" | "linear" | "inverse" => &self.category,
            _ if self.symbol.to_ascii_uppercase().ends_with("PERP") => "linear",
            _ => "spot",
        }
    }
}

/// A fill shaped for the trade form: what was sold, what was bought, costs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct TradePrefill {
    /// `buy` when the user acquired the base asset, `sell` when they gave it up.
    pub side: String,
    pub symbol: String,
    pub asset_sold: Option<String>,
    pub qty_sold: Option<String>,
    pub asset_bought: Option<String>,
    pub qty_bought: Option<String>,
    pub price: Option<String>,
    pub fee: Option<String>,
    pub fee_asset: Option<String>,
    /// ISO-8601 UTC, ready for the form's date field.
    pub occurred_at: Option<String>,
    pub comment: Option<String>,
}

impl Execution {
    /// Map a fill onto trade-form fields.
    pub fn to_prefill(&self) -> TradePrefill {
        let is_buy = self.side.eq_ignore_ascii_case("buy");
        let (base, quote) = Self::split_symbol(&self.symbol).unwrap_or_else(|| {
            // Unknown layout: keep the raw symbol as the base so the user at
            // least sees something to correct.
            (self.symbol.to_ascii_uppercase(), String::new())
        });

        let nonempty = |raw: String| if raw.is_empty() { None } else { Some(raw) };

        let (asset_sold, qty_sold, asset_bought, qty_bought) = if is_buy {
            (
                nonempty(quote.clone()),
                nonempty(self.exec_value.clone()),
                nonempty(base.clone()),
                nonempty(self.exec_qty.clone()),
            )
        } else {
            (
                nonempty(base.clone()),
                nonempty(self.exec_qty.clone()),
                nonempty(quote.clone()),
                nonempty(self.exec_value.clone()),
            )
        };

        // Bybit reports fees in the fee coin; for the common case that is the
        // asset received (buy) or given up (sell).
        let fee_asset = if is_buy {
            nonempty(quote)
        } else {
            nonempty(base)
        };

        TradePrefill {
            side: if is_buy { "buy" } else { "sell" }.to_string(),
            symbol: self.symbol.to_ascii_uppercase(),
            asset_sold,
            qty_sold,
            asset_bought,
            qty_bought,
            price: Some(self.exec_price.clone()),
            fee: Some(self.exec_fee.clone()),
            fee_asset,
            occurred_at: self
                .exec_time
                .parse::<i64>()
                .ok()
                .map(|ms| crate::unix_to_iso(ms / 1000)),
            comment: Some(format!(
                "bybit {} {} · {} @ {}",
                self.category_label(),
                self.side.to_ascii_lowercase(),
                self.exec_qty,
                self.exec_price
            )),
        }
    }

    fn category_label(&self) -> &'static str {
        if self.symbol.ends_with("PERP") {
            "perp"
        } else {
            "spot"
        }
    }
}

/// A move between the user's own Bybit accounts (`/v5/asset/transfer/query-inter-transfer-list`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterTransfer {
    #[serde(default)]
    pub transfer_id: String,
    #[serde(default)]
    pub coin: String,
    #[serde(default)]
    pub amount: String,
    #[serde(default)]
    pub from_account_type: String,
    #[serde(default)]
    pub to_account_type: String,
    /// Milliseconds since the epoch, as a string.
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub status: String,
}

/// One payout of an Earn product (`/v5/earn/yield`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarnYield {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub coin: String,
    #[serde(default)]
    pub product_id: String,
    #[serde(default)]
    pub amount: String,
    /// Milliseconds since the epoch, as a string.
    #[serde(default)]
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill() -> Execution {
        Execution {
            symbol: "BTCUSDT".into(),
            side: "Buy".into(),
            order_type: "Market".into(),
            exec_price: "65000".into(),
            exec_qty: "0.5".into(),
            exec_value: "32500".into(),
            exec_fee: "16.25".into(),
            exec_time: "1700000000000".into(),
            is_maker: false,
            closed_size: None,
            exec_id: "exec-1".into(),
            category: "spot".into(),
        }
    }

    #[test]
    fn splits_symbols_into_base_and_quote() {
        assert_eq!(
            Execution::split_symbol("BTCUSDT"),
            Some(("BTC".into(), "USDT".into()))
        );
        assert_eq!(
            Execution::split_symbol("ethusdc"),
            Some(("ETH".into(), "USDC".into()))
        );
        assert_eq!(
            Execution::split_symbol("SOLUSDT"),
            Some(("SOL".into(), "USDT".into()))
        );
        assert_eq!(Execution::split_symbol("WEIRD"), None);
    }

    /// A buy spends quote to get base, so the *sold* leg must be the quote.
    #[test]
    fn buy_prefill_swaps_the_legs_correctly() {
        let prefill = fill().to_prefill();
        assert_eq!(prefill.side, "buy");
        assert_eq!(prefill.asset_sold.as_deref(), Some("USDT"));
        assert_eq!(prefill.qty_sold.as_deref(), Some("32500"));
        assert_eq!(prefill.asset_bought.as_deref(), Some("BTC"));
        assert_eq!(prefill.qty_bought.as_deref(), Some("0.5"));
        assert_eq!(prefill.price.as_deref(), Some("65000"));
        assert_eq!(prefill.fee.as_deref(), Some("16.25"));
        assert_eq!(prefill.fee_asset.as_deref(), Some("USDT"));
    }

    #[test]
    fn sell_prefill_swaps_the_legs_correctly() {
        let mut execution = fill();
        execution.side = "Sell".into();
        let prefill = execution.to_prefill();
        assert_eq!(prefill.side, "sell");
        assert_eq!(prefill.asset_sold.as_deref(), Some("BTC"));
        assert_eq!(prefill.qty_sold.as_deref(), Some("0.5"));
        assert_eq!(prefill.asset_bought.as_deref(), Some("USDT"));
        assert_eq!(prefill.qty_bought.as_deref(), Some("32500"));
        assert_eq!(prefill.fee_asset.as_deref(), Some("BTC"));
    }

    #[test]
    fn exec_time_millis_becomes_iso_seconds() {
        let prefill = fill().to_prefill();
        // 1700000000000 ms == 2023-11-14T22:13:20Z
        assert_eq!(prefill.occurred_at.as_deref(), Some("2023-11-14T22:13:20Z"));
    }

    #[test]
    fn comments_record_the_provenance() {
        let prefill = fill().to_prefill();
        let comment = prefill.comment.unwrap();
        assert!(comment.contains("bybit"), "{comment}");
        assert!(comment.contains("buy"), "{comment}");
        assert!(comment.contains("0.5"), "{comment}");
        assert!(comment.contains("65000"), "{comment}");
    }

    #[test]
    fn closing_positions_are_detected() {
        let mut execution = fill();
        assert!(!execution.is_closing());
        execution.closed_size = Some("0.5".into());
        assert!(execution.is_closing());
        execution.closed_size = Some("0".into());
        assert!(!execution.is_closing());
        execution.closed_size = None;
        assert!(!execution.is_closing());
    }

    #[test]
    fn unknown_symbol_layout_does_not_panic() {
        let mut execution = fill();
        execution.symbol = "MYSTERY".into();
        let prefill = execution.to_prefill();
        // Base leg is known; the quote leg is not, so it must stay empty
        // rather than be guessed at.
        assert_eq!(prefill.asset_bought.as_deref(), Some("MYSTERY"));
        assert_eq!(prefill.qty_bought.as_deref(), Some("0.5"));
        assert_eq!(prefill.asset_sold, None);
        assert_eq!(prefill.fee_asset, None);
    }

    #[test]
    fn categories_map_to_bybit_strings() {
        assert_eq!(BybitCategory::Spot.as_str(), "spot");
        assert_eq!(BybitCategory::Linear.as_str(), "linear");
        assert_eq!(BybitCategory::Inverse.as_str(), "inverse");
    }

    #[test]
    fn transfer_rows_read_camel_case() {
        let row: InterTransfer = serde_json::from_value(serde_json::json!({
            "transferId": "t1", "coin": "USDT", "amount": "50", "fromAccountType": "FUND",
            "toAccountType": "UNIFIED", "timestamp": "1700000000000", "status": "SUCCESS"}))
        .unwrap();
        assert_eq!(row.from_account_type, "FUND");
        assert_eq!(row.to_account_type, "UNIFIED");
    }
}
