//! Staging of Bybit rows as `PendingImport`s: pure mapping of the port data of `bybit`.
// These become free functions when mwt-net is cut.

use crate::bybit::{EarnYield, Execution, InterTransfer};
use crate::vault::{ACCOUNT_EARN, ImportKind, PendingImport};
use mwt_base::Amount;

pub trait PendingImportExt: Sized {
    /// Shape a Bybit fill for staging. Returns `None` for fills we cannot make
    /// sense of (no execution id, or an unparseable quantity/price) — a garbled
    /// fill must never reach the user's transactions.
    fn from_execution(store_id: &str, category: &str, exec: &Execution) -> Option<Self>;
    /// Stage a move between the user's own Bybit accounts. The amount sits on
    /// both sides (`*_sold` leaves `from_account`, `*_bought` reaches `to_account`).
    fn from_transfer(store_id: &str, row: &InterTransfer) -> Option<Self>;
    /// Stage an Earn payout as income of the Earn account.
    fn from_earn_yield(store_id: &str, row: &EarnYield) -> Option<Self>;
    /// Everything one Bybit sweep stages for an exchange wallet: fills,
    /// moves between its accounts and Earn payouts. Rows that cannot be read
    /// are dropped, never guessed at.
    fn bybit_batch(
        root_id: &str,
        fills: &[Execution],
        transfers: &[InterTransfer],
        yields: &[EarnYield],
    ) -> Vec<Self>;
}

impl PendingImportExt for PendingImport {
    fn from_execution(store_id: &str, category: &str, exec: &Execution) -> Option<Self> {
        let exec_id = exec.exec_id.trim();
        if exec_id.is_empty() {
            return None;
        }
        let prefill = exec.to_prefill();
        // Both legs must be fully known: a trade with an unknown quote leg
        // cannot be turned into balanced holdings.
        let asset_bought = nonempty(prefill.asset_bought?);
        let asset_sold = nonempty(prefill.asset_sold?);
        let qty_bought = Amount::parse(&nonempty(prefill.qty_bought?)).ok()?;
        let qty_sold = Amount::parse(&nonempty(prefill.qty_sold?)).ok()?;
        let price = Amount::parse(&nonempty(prefill.price?)).ok()?;
        if !qty_bought.is_positive() || !qty_sold.is_positive() || !price.is_positive() {
            return None;
        }
        Some(Self {
            exec_id: exec_id.to_string(),
            store_id: store_id.to_string(),
            category: category.to_string(),
            symbol: nonempty(prefill.symbol),
            side: nonempty(prefill.side),
            asset_bought,
            qty_bought,
            asset_sold,
            qty_sold,
            price,
            fee: prefill
                .fee
                .as_deref()
                .and_then(|amount| Amount::parse(amount).ok())
                .unwrap_or(Amount::ZERO),
            fee_asset: prefill.fee_asset.map(nonempty).unwrap_or_default(),
            occurred_at: nonempty(prefill.occurred_at.unwrap_or_default()),
            comment: prefill.comment.unwrap_or_default(),
            is_maker: Some(exec.is_maker),
            closed_size: exec
                .closed_size
                .as_deref()
                .and_then(|raw| Amount::parse(raw).ok())
                .filter(|size| size.is_positive()),
            closed_pnl: None,
            kind: ImportKind::Fill,
            from_account: None,
            to_account: None,
        })
    }

    fn from_transfer(store_id: &str, row: &InterTransfer) -> Option<Self> {
        let id = row.transfer_id.trim();
        let amount = Amount::parse(row.amount.trim())
            .ok()
            .filter(|value| value.is_positive())?;
        let coin = row.coin.trim().to_ascii_uppercase();
        if id.is_empty() || coin.is_empty() {
            return None;
        }
        Some(Self {
            exec_id: format!("transfer:{id}"),
            store_id: store_id.to_string(),
            category: "transfer".into(),
            symbol: coin.clone(),
            side: String::new(),
            asset_bought: coin.clone(),
            qty_bought: amount,
            asset_sold: coin,
            qty_sold: amount,
            price: Amount::ZERO,
            occurred_at: millis_to_iso(&row.timestamp),
            comment: format!("Bybit: {} → {}", row.from_account_type, row.to_account_type),
            kind: ImportKind::Transfer,
            from_account: Some(row.from_account_type.trim().to_ascii_uppercase()),
            to_account: Some(row.to_account_type.trim().to_ascii_uppercase()),
            ..Self::default()
        })
    }

    fn from_earn_yield(store_id: &str, row: &EarnYield) -> Option<Self> {
        let amount = Amount::parse(row.amount.trim())
            .ok()
            .filter(|value| value.is_positive())?;
        let coin = row.coin.trim().to_ascii_uppercase();
        let id = if row.id.trim().is_empty() {
            format!("{}:{}:{}", row.product_id, row.created_at, row.amount)
        } else {
            row.id.trim().to_string()
        };
        if coin.is_empty() {
            return None;
        }
        Some(Self {
            exec_id: format!("earn:{id}"),
            store_id: store_id.to_string(),
            category: "earn".into(),
            symbol: coin.clone(),
            side: String::new(),
            asset_bought: coin,
            qty_bought: amount,
            occurred_at: millis_to_iso(&row.created_at),
            comment: "Bybit Earn".into(),
            kind: ImportKind::EarnIncome,
            to_account: Some(ACCOUNT_EARN.into()),
            ..Self::default()
        })
    }

    fn bybit_batch(
        root_id: &str,
        fills: &[Execution],
        transfers: &[InterTransfer],
        yields: &[EarnYield],
    ) -> Vec<Self> {
        fills
            .iter()
            .filter_map(|exec| Self::from_execution(root_id, exec.category_hint(), exec))
            .chain(
                transfers
                    .iter()
                    .filter_map(|row| Self::from_transfer(root_id, row)),
            )
            .chain(
                yields
                    .iter()
                    .filter_map(|row| Self::from_earn_yield(root_id, row)),
            )
            .collect()
    }
}

/// Bybit millisecond timestamps as RFC3339; empty when unreadable.
fn millis_to_iso(raw: &str) -> String {
    raw.trim()
        .parse::<i64>()
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|moment| moment.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

fn nonempty(raw: String) -> String {
    raw.trim().to_string()
}
