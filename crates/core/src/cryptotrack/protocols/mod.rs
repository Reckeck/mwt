//! DeFi protocols: what an address holds inside one (stage 3, spec D0).
//!
//! An adapter answers three questions and nothing else:
//!
//! 1. [`ProtocolAdapter::discover`] — which positions does this address have
//!    here? Keys only, so the expensive read is paid per position and a
//!    protocol with nothing there costs one call.
//! 2. [`ProtocolAdapter::read_position`] — what is in one of them now?
//!    Quantities and facts ([`PositionReading`]), never money: an adapter has
//!    no prices.
//! 3. [`ProtocolAdapter::events`] — what happened to it? Ordinary operations
//!    ([`CryptoTx`]) carrying [`crate::model::cryptotrack::TrackingRef::Position`],
//!    so the deposits, the collected fees and the gas go through the one
//!    existing path and are priced by the one existing price lookup.
//!
//! **Pure in the same sense as the rest of the module** (`AGENTS.md`, rule
//! 4, and [`crate::cryptotrack::source`]): an adapter reads the network and
//! hands back what it found. It never touches a document, so the vault lock
//! is not held across the network (rule 3) and the whole chain from a node's
//! answer to a net result is a function of its inputs.
//!
//! # Why these three and not one `positions()`
//!
//! Spec E0 declared a placeholder `ProtocolAdapter::positions`, one call per
//! address. All four protocols of wave 1 need the split:
//!
//! * **Uniswap v3** finds positions through the manager's `balanceOf` and
//!   `tokenOfOwnerByIndex` — a cheap enumeration — then reads each token id's
//!   fee growth, which is the expensive part (spec D1).
//! * **Uniswap v4** finds them by scanning `Transfer` logs in chunks and
//!   reads state through `StateView`; a pool behind a hook is read the same
//!   way and marked approximate (spec D2).
//! * **Aave v3** has one position per reserve, so discovery is the reserve
//!   list and the read is `getUserReserveData` (spec D3).
//! * **Lido** has at most two (stETH, wstETH): discovery is a balance check,
//!   the read is the balance and `stEthPerToken` (spec D4).
//!
//! `events` is separate for the same reason the history of an address is
//! separate from its balance: it is paged, it is cursored, and it is the
//! only part a position needs in order to say what it cost.
//!
//! # The registry
//!
//! [`Registry`] is the list of adapters this build has. It holds trait
//! objects, so the futures are boxed ([`AdapterFuture`]); the alternative, a
//! generic over four concrete adapters, would put every protocol's name into
//! every caller. [`Registry::builtin`] is the shipped list, and since wave 1
//! of stage 3 it holds all four: Lido, Aave v3, Uniswap v3 and Uniswap v4.

pub mod aave_v3;
pub mod fees;
pub mod lido;
pub mod node;
pub mod position;
pub mod tick_math;
pub mod uniswap_v3;
pub mod uniswap_v4;
pub mod wide;

use std::future::Future;
use std::pin::Pin;

use crate::cryptotrack::rpc::RpcClient;
use crate::error::Result;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoAsset, CryptoTx};

pub use fees::{
    FeeReading, NetParts, SECONDS_PER_YEAR, annual_percentage_rate, fees_usd, impermanent_loss,
    impermanent_loss_ratio, net_result, worth_usd,
};
pub use node::{Answer, Node};
pub use position::{
    AssetAmount, LegToken, PositionKey, PositionKind, PositionReading, PositionStatus,
    ProtocolReview, is_closed, kind_of, record_id_prefixes,
};
pub use wide::U256;

/// What an adapter's call answers with. Boxed because [`Registry`] holds
/// adapters as trait objects, and `Send` because a pass runs on the Tauri
/// runtime with no lock held (`AGENTS.md`, rule 3).
pub type AdapterFuture<'fut, Out> = Pin<Box<dyn Future<Output = Result<Out>> + Send + 'fut>>;

/// What to read: one address of one network, as the followed account stores
/// it.
#[derive(Debug, Clone, Copy)]
pub struct ProtocolRequest<'req> {
    pub chain: Chain,
    /// The address, already normalized
    /// ([`crate::model::cryptotrack::normalize_address`]).
    pub address: &'req str,
    /// The followed account the answer belongs to
    /// ([`PositionReading::account_id`]).
    pub account_id: &'req str,
    /// Nothing earlier than this is read (RFC3339 UTC): the date the user
    /// chose to follow the address from.
    pub since: Option<&'req str>,
    /// The JSON-RPC endpoint the user set for this network, when they set
    /// one; `None` means the presets of [`crate::cryptotrack::chains`], tried
    /// in order ([`node::Node::new`]).
    pub rpc: Option<&'req str>,
    /// What this adapter left for itself on an earlier pass
    /// ([`Discovery::memory`]): opaque to everyone but the adapter, kept by
    /// the device and handed back per protocol. `None` on the first pass and
    /// whenever the device has nothing (or lost it), so an adapter must work
    /// without it.
    pub memory: Option<&'req str>,
}

/// What one discovery found, and whether it is the whole answer.
///
/// [`ProtocolAdapter::discover`] returns the positions alone, which is right
/// for a protocol that can list everything an address holds. A protocol that
/// finds positions by scanning history (Uniswap v4) may stop before it has
/// seen all of it: then the list is still worth reading, but it is **not a
/// review** — a position missing from it may exist further back, and the
/// caller must not close a stored position on its silence (ADR-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub keys: Vec<PositionKey>,
    /// `false`: the history was not all seen, `keys` may lack positions.
    pub complete: bool,
    /// Opaque state for the next pass ([`ProtocolRequest::memory`]). It
    /// replaces what the device holds for the protocol, and `None` clears it.
    pub memory: Option<String>,
}

impl Discovery {
    /// A whole answer that leaves nothing for the next pass.
    pub fn complete(keys: Vec<PositionKey>) -> Self {
        Self {
            keys,
            complete: true,
            memory: None,
        }
    }
}

/// A DeFi protocol's reading of an address.
///
/// Dyn-compatible on purpose: [`Registry`] keeps every adapter of the build
/// in one list, and a caller names no protocol.
pub trait ProtocolAdapter: Send + Sync {
    /// Stable id of the protocol (`uniswap-v3`, `aave-v3`), as a position
    /// stores it in [`crate::model::cryptotrack::CryptoPosition::protocol`].
    fn protocol(&self) -> &'static str;

    /// Whether the protocol lives on that network.
    fn supports(&self, chain: Chain) -> bool;

    /// The positions this address has in the protocol. Touches no document.
    fn discover<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
    ) -> AdapterFuture<'fut, Vec<PositionKey>>;

    /// [`Self::discover`] with the means to say that the answer is partial
    /// and to remember where a scan stands. The default is a complete answer
    /// with no memory; an adapter that scans history overrides it, and its
    /// `discover` returns only a complete result.
    fn discover_scan<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
    ) -> AdapterFuture<'fut, Discovery> {
        Box::pin(async move { Ok(Discovery::complete(self.discover(request).await?)) })
    }

    /// What one position holds now. `None` when it is gone between the
    /// discovery and the read — a position that vanished is not an error.
    /// Touches no document.
    fn read_position<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
        key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Option<PositionReading>>;

    /// The operations of one position, each carrying
    /// [`PositionKey::tracking`], newest first. This is where "invested at
    /// the price of the deposit", the collected fees and the gas come from.
    /// Touches no document.
    fn events<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
        key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Vec<CryptoTx>>;
}

/// The tokens a protocol hands over as the receipt for a position — an
/// aToken, stETH, the position NFT.
///
/// Static, with no adapter and no network behind it, so a device that has
/// only ever synced knows them too. That matters: the receipt sits in the
/// wallet and a node reports it, and counting both it and the position would
/// double the money (`service::cryptotrack::positions`, the second half of
/// the rule).
///
/// Empty for every protocol until wave 1 fills it; the module gets the same
/// answer out of the stored operations meanwhile.
pub fn receipt_tokens(protocol: &str, chain: Chain) -> Vec<CryptoAsset> {
    match protocol {
        uniswap_v3::PROTOCOL => uniswap_v3::receipt_tokens(chain),
        uniswap_v4::PROTOCOL => uniswap_v4::receipt_tokens(chain),
        aave_v3::PROTOCOL => aave_v3::receipt_tokens(chain),
        lido::PROTOCOL => lido::receipt_tokens(chain),
        _ => Vec::new(),
    }
}

/// Every protocol this build can read.
#[derive(Default)]
pub struct Registry {
    adapters: Vec<Box<dyn ProtocolAdapter>>,
}

impl Registry {
    /// A registry of exactly these adapters: what a test builds.
    pub fn new(adapters: Vec<Box<dyn ProtocolAdapter>>) -> Self {
        Self { adapters }
    }

    /// The adapters of the shipped build, all reading through the one node
    /// client of the pass (its host limiter paces every protocol together).
    ///
    /// Wave 1 of stage 3, all four: Lido (spec D4), Aave v3 (spec D3),
    /// Uniswap v3 (spec D1) and Uniswap v4 (spec D2).
    ///
    /// The order is the order a pass reads them in, and it is deliberate: the
    /// cheap reads first (Lido is two balances, Aave one batch per market),
    /// then Uniswap v3's enumeration, and last v4, whose discovery scans
    /// `Transfer` logs backwards and is the only one that can walk away with
    /// nothing after many requests.
    ///
    /// An adapter keeps the client and builds a [`node::Node`] per request:
    /// the endpoints depend on the network and on what the user set for it.
    pub fn builtin(node: RpcClient) -> Self {
        Self::new(vec![
            lido::adapter(node.clone()),
            aave_v3::adapter(node.clone()),
            uniswap_v3::adapter(node.clone()),
            uniswap_v4::adapter(node),
        ])
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// The adapters that can read this network, in the order they were
    /// registered.
    pub fn for_chain(&self, chain: Chain) -> impl Iterator<Item = &dyn ProtocolAdapter> {
        self.adapters
            .iter()
            .map(Box::as_ref)
            .filter(move |adapter| adapter.supports(chain))
    }

    /// The adapter of one protocol, by the id a stored position carries.
    pub fn find(&self, protocol: &str) -> Option<&dyn ProtocolAdapter> {
        self.adapters
            .iter()
            .map(Box::as_ref)
            .find(|adapter| adapter.protocol() == protocol)
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Registry")
            .field(
                "protocols",
                &self
                    .adapters
                    .iter()
                    .map(|adapter| adapter.protocol())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::cryptotrack::{CryptoDirection, CryptoLeg};

    /// A protocol that exists only here: the registry and the pass are tested
    /// before any real adapter is written (wave 1).
    struct Stub {
        chain: Chain,
    }

    impl ProtocolAdapter for Stub {
        fn protocol(&self) -> &'static str {
            "stub-v1"
        }

        fn supports(&self, chain: Chain) -> bool {
            chain == self.chain
        }

        fn discover<'fut>(
            &'fut self,
            _request: ProtocolRequest<'fut>,
        ) -> AdapterFuture<'fut, Vec<PositionKey>> {
            Box::pin(async move { Ok(vec![PositionKey::new(self.protocol(), self.chain, "1")]) })
        }

        fn read_position<'fut>(
            &'fut self,
            request: ProtocolRequest<'fut>,
            key: &'fut PositionKey,
        ) -> AdapterFuture<'fut, Option<PositionReading>> {
            Box::pin(async move {
                Ok(Some(PositionReading {
                    key: key.clone(),
                    account_id: request.account_id.to_string(),
                    name: "Stub".into(),
                    kind: PositionKind::Staking,
                    status: PositionStatus::Open,
                    legs: vec![CryptoLeg {
                        direction: CryptoDirection::In,
                        asset: CryptoAsset::Native { chain: self.chain },
                        amount: crate::Amount::parse("1").expect("one"),
                        counterparty: None,
                    }],
                    fees: FeeReading::none_earned(),
                    receipts: Vec::new(),
                    approximate: true,
                    opened_at: None,
                    closed_at: None,
                    tokens: Vec::new(),
                    read_at: "2026-11-16T00:00:00Z".into(),
                }))
            })
        }

        fn events<'fut>(
            &'fut self,
            _request: ProtocolRequest<'fut>,
            _key: &'fut PositionKey,
        ) -> AdapterFuture<'fut, Vec<CryptoTx>> {
            Box::pin(async move { Ok(Vec::new()) })
        }
    }

    #[test]
    fn the_shipped_registry_holds_the_adapters_of_wave_one() {
        let registry = Registry::builtin(node::stub::client());
        assert!(!registry.is_empty());
        assert!(registry.find(lido::PROTOCOL).is_some());
        assert!(registry.find(aave_v3::PROTOCOL).is_some());
        assert!(registry.find(uniswap_v3::PROTOCOL).is_some());
        assert!(registry.find(uniswap_v4::PROTOCOL).is_some());
        // A protocol nobody registered is not silently answered by another
        // adapter.
        assert!(registry.find("nothing-like-it").is_none());
        // Every one of them reads Ethereum; a network nobody supports offers
        // no adapter rather than failing a pass.
        assert_eq!(registry.for_chain(Chain::Ethereum).count(), 4);
        assert_eq!(
            registry.for_chain(Chain::Tron).count(),
            0,
            "an EVM adapter does not claim a non-EVM network"
        );
    }

    #[test]
    fn the_registry_offers_only_the_adapters_of_the_network_asked_for() {
        let registry = Registry::new(vec![Box::new(Stub {
            chain: Chain::Ethereum,
        })]);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.for_chain(Chain::Ethereum).count(), 1);
        assert_eq!(registry.for_chain(Chain::Base).count(), 0);
        assert!(registry.find("stub-v1").is_some());
        assert!(registry.find("nothing-like-it").is_none());
    }

    #[tokio::test]
    async fn an_adapter_answers_all_three_questions_without_a_document() {
        let adapter = Stub {
            chain: Chain::Ethereum,
        };
        let request = ProtocolRequest {
            chain: Chain::Ethereum,
            address: "0xaaaa",
            account_id: "cacc-1",
            since: None,
            rpc: None,
            memory: None,
        };
        let found = adapter.discover(request).await.expect("a discovery");
        assert_eq!(
            found,
            vec![PositionKey::new("stub-v1", Chain::Ethereum, "1")]
        );
        let reading = adapter
            .read_position(request, &found[0])
            .await
            .expect("a reading")
            .expect("the position is there");
        assert_eq!(reading.account_id, "cacc-1");
        assert!(
            reading.approximate,
            "the flag comes from the adapter, not from the screen"
        );
        assert_eq!(reading.stored().id, "cacc-1|stub-v1|ethereum|1");
        assert!(
            adapter
                .events(request, &found[0])
                .await
                .expect("events")
                .is_empty()
        );
    }

    /// A registry of trait objects can be driven without naming a protocol:
    /// what the tick does on every pass.
    #[tokio::test]
    async fn the_registry_is_read_without_naming_a_protocol() {
        let registry = Registry::new(vec![Box::new(Stub {
            chain: Chain::Ethereum,
        })]);
        let request = ProtocolRequest {
            chain: Chain::Ethereum,
            address: "0xaaaa",
            account_id: "cacc-1",
            since: None,
            rpc: None,
            memory: None,
        };
        let mut read = Vec::new();
        for adapter in registry.for_chain(Chain::Ethereum) {
            for key in adapter.discover(request).await.expect("a discovery") {
                if let Some(reading) = adapter
                    .read_position(request, &key)
                    .await
                    .expect("a reading")
                {
                    read.push(reading.stored().id);
                }
            }
        }
        assert_eq!(read, ["cacc-1|stub-v1|ethereum|1"]);
    }
}
