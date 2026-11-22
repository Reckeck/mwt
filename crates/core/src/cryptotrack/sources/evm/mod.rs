//! The EVM source (stage 2, spec E3): history and held tokens from Blockscout
//! API v2, balances from a node, for the six networks of [`crate::cryptotrack::chains`].
//!
//! The source is pure like every source: it reads the network and returns a
//! [`SourceBatch`]; it never touches the document. The caller stores the
//! events and the `next_cursor` of one batch together and passes the cursor
//! back, so an interruption on page N is resumed at page N and a page that
//! arrives twice produces the same ids ([`RawEvent::id`]), not duplicates.
//!
//! Everything from the network is untrusted: quantities are exact
//! [`Uint256`] (never a float), unknown fields are ignored, an item that
//! fails its checks is listed in [`SourceBatch::rejected`] instead of
//! aborting the page, and a page that is not a page is an error.
//!
//! ## Blockscout limits (checked 2026-11-11)
//!
//! * Documented (<https://docs.blockscout.com/rate-limits>): the keyed API
//!   (`api.blockscout.com`, the successor of the per-instance API) gives
//!   5 requests/s and 100 000 credits/day on the free plan; the default call
//!   costs 20 credits, `addresses/:hash/token-transfers` 30,
//!   `addresses/:hash/internal-transactions` 40. That is about 3 300
//!   token-transfer pages (50 items each) a day.
//! * Observed on the key-less instance endpoints used here (`x-ratelimit-limit:
//!   180`, `x-ratelimit-reset` ~ 60 s window): 180 requests per minute per IP.
//!   The docs do not promise this and say the per-instance API is deprecated.
//!
//! An address with 5 000 transactions and 5 000 token transfers needs about
//! 200 pages: roughly 70 s keyless, 40 s with a key, 6 000 credits. Enough for
//! an ordinary active address; a very busy one (hundreds of thousands of
//! items) is not on the free key's daily credits, hence the start date bound.
//!
//! ## Transport
//!
//! Balances use the RPC client directly. Blockscout v2 is a REST API (GET), and
//! the RPC client only sends JSON-RPC POSTs, so history is read through the
//! [`ExplorerApi`] trait; its production implementation is a rate-limited GET
//! on the same client (same host limiter, same subject rule).

/// The hand-written ABI of this source. `pub(crate)` because the protocol
/// adapters read the same words through the same bounds-checked decoder
/// (`cryptotrack::protocols::node`, spec D0) instead of writing a second one.
pub(crate) mod abi;
mod balances;
mod blockscout;
mod cursor;
mod model;
mod uint;

#[cfg(test)]
mod tests;

pub use balances::{BalanceRequest, TOKENS_PER_CALL, read_balances};
pub use blockscout::{ExplorerApi, HistoryRequest, fetch_page};
pub use cursor::{Cursor, Stream};
pub use model::{BalanceSource, RawAsset, RawBalance, RawEvent, RawKind, Rejected, SourceBatch};
pub use uint::Uint256;

use crate::cryptotrack::ports::ChainSource;
use crate::error::Result;

/// Stable id, as it is written into a cursor record and a settings record.
pub const SOURCE_ID: &str = "evm";

/// The source with its transports.
pub struct EvmSource<A: ExplorerApi, C: ChainSource> {
    explorer: A,
    rpc: C,
}

impl<A: ExplorerApi, C: ChainSource> EvmSource<A, C> {
    pub fn new(explorer: A, rpc: C) -> Self {
        Self { explorer, rpc }
    }

    /// The next page of history or held tokens.
    pub async fn fetch_page(&self, request: &HistoryRequest<'_>) -> Result<SourceBatch> {
        fetch_page(&self.explorer, request).await
    }

    /// Native and token balances from a node.
    pub async fn read_balances(&self, request: &BalanceRequest<'_>) -> Result<SourceBatch> {
        read_balances(&self.rpc, request).await
    }
}
