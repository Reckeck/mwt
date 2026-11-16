//! IPC types of the DeFi positions an address holds inside a protocol.
//!
//! Owner: spec D0 (stage 3). The stored shape
//! ([`crate::model::cryptotrack::CryptoPosition`]) and its sync rule exist
//! from E0 so that the vault schema does not change again for DeFi.
//!
//! Commands: `crates/app/src/commands/cryptotrack/positions.rs`.
//! Computed in: `crates/core/src/service/cryptotrack/positions.rs`.
//!
//! # Every figure that is not known is absent
//!
//! Each money field is an `Option<Amount>`, and `None` means "not known":
//! the screen prints a dash and says why (decision of the D-stage mockups).
//! A real zero is a `Some("0")` — a position that earned no fees at all is
//! told apart from one whose fees nobody could read, and "no price for the
//! token" never reads as "worth nothing".
//!
//! # What must not be added up twice
//!
//! [`CryptoPositionView::il_usd`] and [`CryptoPositionView::accrued_usd`] are
//! **informational**: both are already inside
//! `value_usd - invested_usd`, which is itself already inside
//! [`CryptoPositionView::net_usd`]. A screen that adds either of them to the
//! net result counts it twice. The identity is written out in
//! [`crate::cryptotrack::protocols::fees`].

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoDirection, TrackingRef};

/// One asset a position holds or owes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "CryptoPositionAsset")]
pub struct PositionAssetView {
    /// The module's own key ([`crate::model::cryptotrack::CryptoAsset::key`]).
    pub asset_key: String,
    pub symbol: String,
    /// A supply is `in`, a debt `out`.
    pub direction: CryptoDirection,
    /// In whole units.
    pub quantity: Amount,
    /// What that quantity is worth now; `None` when no price is known.
    #[ts(optional)]
    pub value_usd: Option<Amount>,
}

/// One DeFi position as the screen reads it (spec D5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(rename = "CryptoPositionView")]
pub struct PositionView {
    /// Record key of `doc.crypto_positions`:
    /// `<account_id>|<protocol>|<chain>|<protocol's own id>`.
    pub id: String,
    /// The followed account that holds it.
    pub account_id: String,
    /// The wallet that shows that account; `None` while none does.
    #[ts(optional)]
    pub store_id: Option<String>,
    pub chain: Chain,
    /// `uniswap-v3`, `aave-v3`, …
    pub protocol: String,
    /// What the protocol calls the position.
    pub name: String,
    pub kind: CryptoPositionKind,
    pub status: CryptoPositionStatus,
    /// What it holds and owes.
    pub assets: Vec<PositionAssetView>,
    /// What went in, at the price of the day it went in, minus what has been
    /// taken back out at the price of its day: the capital still in the
    /// position, and the figure the change of value is measured against.
    #[ts(optional)]
    pub invested_usd: Option<Amount>,
    /// What was ever put in, at the price of each deposit's own day, never
    /// lowered by a withdrawal: what the user risked. This is the figure the
    /// mockups show as «вложено по цене депозита» and the one the rate is
    /// measured on. What came back out is the difference of the two.
    #[ts(optional)]
    pub put_in_usd: Option<Amount>,
    /// What it is worth now: supplies minus debts, principal only. A closed
    /// position is a real zero here and adds nothing to the portfolio.
    #[ts(optional)]
    pub value_usd: Option<Amount>,
    /// What the protocol accrued on the balance itself (an aToken's
    /// interest, stETH's rebase). **Informational**: already inside the value
    /// change. `None` for a pool, which accrues through fees instead.
    #[ts(optional)]
    pub accrued_usd: Option<Amount>,
    /// Fees already taken out of the position, at the price of the day they
    /// were taken.
    #[ts(optional)]
    pub fees_collected_usd: Option<Amount>,
    /// Fees the position still holds, at today's price.
    #[ts(optional)]
    pub fees_uncollected_usd: Option<Amount>,
    /// Against simply holding the deposited coins: negative is the usual
    /// case. **Informational**: already inside the value change. `None` for a
    /// one-sided position, which has nothing to compare against.
    #[ts(optional)]
    pub il_usd: Option<Amount>,
    /// The same as a fraction of what holding would have been worth
    /// (`-0.057191` is -5.7191 %).
    #[ts(optional)]
    pub il_ratio: Option<Amount>,
    /// What the network charged for this position's operations.
    #[ts(optional)]
    pub gas_usd: Option<Amount>,
    /// The whole result: `(value - invested) + fees collected + fees
    /// uncollected - gas`. Nothing else, and never the loss or the accrual a
    /// second time.
    #[ts(optional)]
    pub net_usd: Option<Amount>,
    /// The simple annual rate of that result on the capital ever put in — not
    /// on what is still in, which a withdrawal lowers — as a fraction
    /// (`0.125` is 12.5 %). `None` when nothing went in or no time has
    /// passed. A closed position keeps the rate of its own period.
    #[ts(optional)]
    pub apr: Option<Amount>,
    /// The numbers are an approximation and the row must say so. It comes
    /// from the adapter together with the numbers (a Uniswap v4 pool behind a
    /// hook); the screen never decides it on its own.
    pub approximate: bool,
    /// When the position was opened (RFC3339 UTC).
    #[ts(optional)]
    pub opened_at: Option<String>,
    /// When it was closed; `None` while it is open.
    #[ts(optional)]
    pub closed_at: Option<String>,
    /// When the stored reading was taken (RFC3339 UTC).
    pub updated_at: String,
    /// What the position's operations are found under: the row's link into
    /// the operations list (spec D5).
    pub tracking: TrackingRef,
}

/// What sort of position it is; re-exported from the protocol layer so the
/// generated TypeScript carries one name for it.
pub type CryptoPositionKind = crate::cryptotrack::protocols::PositionKind;

/// Whether the position is still open.
pub type CryptoPositionStatus = crate::cryptotrack::protocols::PositionStatus;
