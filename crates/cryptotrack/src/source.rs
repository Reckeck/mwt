//! Where the crypto module's operations and positions come from.
//!
//! The trait is pure in the same sense as
//! `crate::pricing::PricingService::fetch_prices` (`AGENTS.md`, rule 4):
//! it reads the network and hands back what it found, it never touches the
//! document. Applying the answer is the vault's job, so a fetch can run with
//! no lock held.
//!
//! E0 declared both [`Source`] and a placeholder protocol adapter here. The
//! DeFi stage needed three calls rather than one, so the final adapter moved
//! to [`crate::protocols`] (spec D0) and this file is the
//! address-history side alone: a source is added by E5 (chains) and E7
//! (exchanges).

use std::future::Future;

use mwt_base::error::Result;
use mwt_model::Chain;
use mwt_model::CryptoTx;

/// What to read: one account of one network, continuing where the device
/// stopped last time.
#[derive(Debug, Clone, Copy)]
pub struct FetchRequest<'req> {
    pub chain: Chain,
    /// The address as the account stores it (already normalized).
    pub address: &'req str,
    /// The cursor this device kept for that source, if any
    /// ([`mwt_model::CryptoCursor`]).
    pub cursor: Option<&'req str>,
    /// Most records to ask for in one call.
    pub limit: usize,
}

/// One page of what a source found.
#[derive(Debug, Clone, Default)]
pub struct FetchPage {
    pub events: Vec<CryptoTx>,
    /// The cursor to continue from, to keep on the device.
    pub cursor: Option<String>,
    /// Nothing more to read for now.
    pub done: bool,
}

/// A provider of an address's history (an explorer, an exchange).
///
/// The future is `Send` on purpose: a command fetches with the vault lock
/// released, on the Tauri runtime (`AGENTS.md`, rule 3).
pub trait Source {
    /// Stable id, as it is written into a cursor and a settings record.
    fn id(&self) -> &'static str;

    /// Whether this source can read that network.
    fn supports(&self, chain: Chain) -> bool;

    /// Read the next page. Touches no document.
    fn fetch(&self, request: FetchRequest<'_>) -> impl Future<Output = Result<FetchPage>> + Send;
}
