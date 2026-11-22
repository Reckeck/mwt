//! Uniswap v4 pools, hooks included (spec D2).
//!
//! A position is an NFT of the network's `PositionManager`. Every address,
//! selector and layout below is from `D-protocol-facts.md` (the Uniswap v4
//! sections 6 to 10); the section is named next to each.
//!
//! # Discovery
//!
//! v4's `PositionManager` has no ERC-721Enumerable, so an address's NFTs are
//! found from `Transfer` logs (facts section 7). `balanceOf` says how many
//! the address holds; the logs are then read **from the newest block
//! backwards**, in windows of [`SCAN_WINDOW`] blocks (the one cap the
//! providers agree on, facts section 9; [`Node::logs`] halves a window an
//! endpoint refuses), once for transfers *to* the address and once for
//! transfers *from* it. A token is held when its newest transfer touching the
//! address is the incoming one. The scan stops as soon as as many held tokens
//! are found as `balanceOf` promised, so a wallet whose positions are recent
//! costs a window or two.
//!
//! **The scan remembers** ([`ScanMemory`], handed back through
//! [`ProtocolRequest::memory`] and kept by the device): the newest transfer
//! of every token seen, the block the history was read down to and the block
//! it was read up to. A later pass reads only the blocks above that (a few
//! requests for a quiet wallet) and goes on below it only while `balanceOf`
//! still promises more than was found. The newest [`REORG_MARGIN`] blocks
//! are never remembered, so a reorg cannot leave a transfer that did not
//! happen. Memory that cannot be read, belongs to another wallet or
//! contradicts `balanceOf` is dropped and the scan starts over.
//!
//! **What this cannot do** (gaps, see the report of D2):
//!
//! * the PoolManager deployment block, the natural floor of the scan, is not
//!   in the facts document (its "Unverified" item 5), so there is none. One
//!   pass reads at most `windows_per_pass` windows ([`MAX_SCAN_WINDOWS`],
//!   3 million blocks); a position older than that is **not found on that
//!   pass, and the answer says so**: [`Discovery::complete`] is `false`, the
//!   positions that were found are returned, and the next pass goes on
//!   below the floor it remembers. An incomplete answer is never a review
//!   of what the address holds, so the caller closes nothing on it
//!   (ADR-02); the plain [`ProtocolAdapter::discover`], which has no way to
//!   say "partial", refuses with an error instead. A position the user
//!   enters by token id still reads: [`ProtocolAdapter::read_position`]
//!   needs only the id;
//! * a burned NFT is not held by anyone, so a closed v4 position is never
//!   discovered; [`ProtocolAdapter::read_position`] answers `None` for it.
//!
//! # Reading one position
//!
//! 1. `PositionManager.getPoolAndPositionInfo(tokenId)` (one call) gives the
//!    `PoolKey` and the packed `PositionInfo` (facts section 8): the ticks
//!    are bits 8-31 and 32-55 of it; its 25-byte pool id is **not** the core
//!    pool id and is never used;
//! 2. the real pool id is `keccak256(abi.encode(poolKey))` — the five words
//!    of the answer as they came; the position id is
//!    `keccak256(abi.encodePacked(positionManager, tickLower, tickUpper,
//!    bytes32(tokenId)))` (the manager is the position's owner inside the
//!    pool, the token id its salt). Keccak-256 is implemented in this file
//!    ([`keccak256`]); the crate carries no such dependency;
//! 3. one Multicall3 batch on `StateView`: `getSlot0`, `getPositionInfo`,
//!    `getFeeGrowthInside` and the decimals of every non-native currency.
//!
//! Principal is the liquidity's worth at the pool's price; fees are
//! `liquidity * (feeGrowthInside - feeGrowthInsideLast) / 2^128` with the
//! subtraction wrapping modulo 2^256 and the product on 512 bits. v4 has no
//! `tokensOwed`: fees are paid out whenever the position is modified, so
//! there is nothing banked beside the formula.
//!
//! # Approximate
//!
//! Any pool whose `hooks` address is not zero is returned with
//! `approximate = true`. The facts document (section 10) splits hooked pools
//! finer — exact fee growth when the hook has no delta permission and no
//! dynamic fee — but the brief of this task is to flag every hooked pool, and
//! a hook that can skim a withdrawal is invisible to `StateView`; the finer
//! split is left for whoever has the screen's wording in hand.
//!
//! # Events
//!
//! Always an empty list: the topic0 of `ModifyLiquidity` is not in the facts
//! document, and block timestamps and receipts (gas) are not something
//! [`Node`] reads. The engine shows a dash for a position with no stored
//! operations, so empty is safe and honest.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cryptotrack::ports::ChainSource;
use crate::cryptotrack::sources::evm::abi;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, TokenId};

use super::tick_math::principal;
use super::uniswap_v3::{SYMBOL_SELECTOR, symbol_of};
use super::{
    AdapterFuture, Answer, AssetAmount, Discovery, FeeReading, LegToken, Node, PositionKey,
    PositionKind, PositionReading, PositionStatus, ProtocolAdapter, ProtocolRequest, U256,
};

/// The protocol id a position of this protocol stores
/// ([`crate::model::cryptotrack::CryptoPosition::protocol`]).
pub const PROTOCOL: &str = "uniswap-v4";

/// One network's deployment (facts section 6, the official v4 deployments
/// page). Hard-coded per network: the page itself warns that an address does
/// not identify a chain. The `PoolManager` is in the table of the facts but
/// nothing here reads it: `StateView` is the reader.
struct Deployment {
    chain: Chain,
    manager: &'static str,
    state_view: &'static str,
}

const DEPLOYMENTS: [Deployment; 6] = [
    Deployment {
        chain: Chain::Ethereum,
        manager: "0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e",
        state_view: "0x7ffe42c4a5deea5b0fec41c94c136cf115597227",
    },
    Deployment {
        chain: Chain::Optimism,
        manager: "0x3c3ea4b57a46241e54610e5f022e5c45859a1017",
        state_view: "0xc18a3169788f4f75a170290584eca6395c75ecdb",
    },
    Deployment {
        chain: Chain::Bsc,
        manager: "0x7a4a5c919ae2541aed11041a1aeee68f1287f95b",
        state_view: "0xd13dd3d6e93f276fafc9db9e6bb47c1180aee0c4",
    },
    Deployment {
        chain: Chain::Polygon,
        manager: "0x1ec2ebf4f37e7363fdfe3551602425af0b3ceef9",
        state_view: "0x5ea1bd7974c8a611cbab0bdcafcb1d9cc9b3ba5a",
    },
    Deployment {
        chain: Chain::Arbitrum,
        manager: "0xd88f38f930b7952f2db2432cb002e7abbf3dd869",
        state_view: "0x76fd297e2d437cd7f76d50f01afe6160f86e9990",
    },
    Deployment {
        chain: Chain::Base,
        manager: "0x7c5f5a4bbd8fd63184577525326123b519429bdc",
        state_view: "0xa3c0c9b65bad0b08107aa264b0f3db444b867a71",
    },
];

fn deployment(chain: Chain) -> Option<&'static Deployment> {
    DEPLOYMENTS.iter().find(|row| row.chain == chain)
}

// Selectors, each with the full signature (facts sections 7 and 8; every one
// computed and cross-checked against 4byte.directory there, and recomputed
// by a test of this file with the Keccak-256 below).

/// `balanceOf(address)` — ERC-721.
const SEL_BALANCE_OF: &str = "70a08231";
/// `getPoolAndPositionInfo(uint256)` on the position manager.
const SEL_POOL_AND_POSITION_INFO: &str = "7ba03aad";
/// `getSlot0(bytes32)` on `StateView`.
const SEL_GET_SLOT0: &str = "c815641c";
/// `getPositionInfo(bytes32,bytes32)` on `StateView`.
const SEL_GET_POSITION_INFO: &str = "97fd7b42";
/// `getFeeGrowthInside(bytes32,int24,int24)` on `StateView`.
const SEL_GET_FEE_GROWTH_INSIDE: &str = "53e9c1fb";
/// ERC-20 `decimals()`: recomputed by the main session with its own
/// Keccak-256 (facts, addendum of 2026-11-16) and again by a test below.
const SEL_DECIMALS: &str = "313ce567";

/// `Transfer(address,address,uint256)`, the ERC-721 event: facts section 7.
/// All three parameters are indexed: `topics = [topic0, from, to, tokenId]`.
const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Words of `getPoolAndPositionInfo`: the five slots of `PoolKey` (facts
/// section 8), then the packed `PositionInfo`.
const INFO_ANSWER_WORDS: usize = 6;
const POOL_KEY_WORDS: usize = 5;

/// A native currency is the zero address in a `PoolKey`.
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
/// The native coin of every network of the table (ETH, BNB, POL) has 18.
const NATIVE_DECIMALS: u8 = 18;

/// The blocks one `eth_getLogs` window asks for: the one cap documented
/// consistently across providers (facts section 9).
const SCAN_WINDOW: u64 = 10_000;

/// How many windows one pass of discovery reads before it stops with a
/// partial answer. There is no deployment block to stop at (facts,
/// "Unverified" 5), and a window is two requests, so a pass is bounded: 300
/// windows is 3 million blocks. The next pass goes on from where this one
/// stopped.
const MAX_SCAN_WINDOWS: u32 = 300;

/// The newest blocks a remembered scan does not trust: a transfer that deep
/// may still be reorganised away, so the next pass reads them again.
const REORG_MARGIN: u64 = 128;

/// A memory larger than this is not kept (and not read): it is a bad
/// answer or somebody else's text, and a scan can always start over.
const MAX_MEMORY_BYTES: usize = 1 << 20;

/// The ticks the protocol allows (`TickMath.MIN_TICK`/`MAX_TICK`).
const MAX_TICK: i64 = 887_272;

/// More NFTs than this on one address is not a wallet, it is a bad answer.
const MAX_POSITIONS: usize = 10_000;

/// The Uniswap v4 adapter. Holds the node client of the pass and builds a
/// [`Node`] per request.
pub struct UniswapV4<C> {
    client: C,
    /// The log windows one pass may read before it stops with a partial
    /// answer ([`MAX_SCAN_WINDOWS`] outside the tests).
    windows_per_pass: u32,
}

/// The adapter for [`super::Registry::builtin`]:
/// `uniswap_v4::adapter(node.clone())`.
pub fn adapter<C: ChainSource + 'static>(node: C) -> Box<dyn ProtocolAdapter> {
    Box::new(UniswapV4 {
        client: node,
        windows_per_pass: MAX_SCAN_WINDOWS,
    })
}

/// The position NFT of this protocol on `chain` (see
/// [`super::receipt_tokens`]). Empty where the protocol is not deployed.
pub fn receipt_tokens(chain: Chain) -> Vec<CryptoAsset> {
    deployment(chain)
        .map(|row| vec![CryptoAsset::Token(TokenId::new(chain, row.manager))])
        .unwrap_or_default()
}

impl<C: ChainSource + 'static> ProtocolAdapter for UniswapV4<C> {
    fn protocol(&self) -> &'static str {
        PROTOCOL
    }

    fn supports(&self, chain: Chain) -> bool {
        deployment(chain).is_some()
    }

    fn discover<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
    ) -> AdapterFuture<'fut, Vec<PositionKey>> {
        Box::pin(async move { self.discover_keys(request).await })
    }

    fn discover_scan<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
    ) -> AdapterFuture<'fut, Discovery> {
        Box::pin(async move { self.scan(request).await })
    }

    fn read_position<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
        key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Option<PositionReading>> {
        Box::pin(async move { self.read(request, key).await })
    }

    /// Always empty (see the module doc): no topic0 for `ModifyLiquidity`,
    /// no block timestamps, no receipts.
    fn events<'fut>(
        &'fut self,
        _request: ProtocolRequest<'fut>,
        _key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Vec<CryptoTx>> {
        Box::pin(async move { Ok(Vec::new()) })
    }
}

/// The newest transfer of one token that touches the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Newest {
    block: u64,
    index: u64,
    /// The transfer was *to* the address, which makes the token held.
    incoming: bool,
}

impl Newest {
    /// Chain order, and for the same log (a transfer to oneself) the
    /// incoming reading.
    fn rank(self) -> (u64, u64, bool) {
        (self.block, self.index, self.incoming)
    }
}

/// One transfer of a token, as the memory keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Remembered {
    token: String,
    block: u64,
    index: u64,
    incoming: bool,
}

/// Where a scan of one wallet stands, kept by the device between passes.
///
/// The blocks `floor..=head` were read completely and `seen` is the newest
/// transfer of every token that touches the wallet in them.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScanMemory {
    version: u8,
    owner: String,
    manager: String,
    floor: u64,
    head: u64,
    seen: Vec<Remembered>,
}

const MEMORY_VERSION: u8 = 1;
const MAX_REMEMBERED: usize = 4 * MAX_POSITIONS;

impl ScanMemory {
    /// `None` for anything that is not a usable memory of *this* wallet on
    /// *this* network at a chain that has reached `head`.
    fn load(text: &str, owner: &str, manager: &str, latest: u64) -> Option<Self> {
        if text.len() > MAX_MEMORY_BYTES {
            return None;
        }
        let memory: Self = serde_json::from_str(text).ok()?;
        let usable = memory.version == MEMORY_VERSION
            && memory.owner == owner
            && memory.manager == manager
            && memory.head <= latest
            && memory.floor <= memory.head.saturating_add(1)
            && memory.seen.len() <= MAX_REMEMBERED
            && memory.seen.iter().all(|entry| {
                !entry.token.is_empty()
                    && entry.token.len() <= 78
                    && entry.token.bytes().all(|byte| byte.is_ascii_digit())
                    && entry.block >= memory.floor
                    && entry.block <= memory.head
            });
        usable.then_some(memory)
    }

    fn newest(&self) -> HashMap<String, Newest> {
        self.seen
            .iter()
            .map(|entry| {
                (
                    entry.token.clone(),
                    Newest {
                        block: entry.block,
                        index: entry.index,
                        incoming: entry.incoming,
                    },
                )
            })
            .collect()
    }
}

/// How a walk over the logs ended.
enum WalkEnd {
    /// As many held tokens as `balanceOf` promised.
    Whole,
    /// The pass ran out of windows before it found them all.
    Short,
    /// The logs and `balanceOf` cannot both be right.
    Disagrees,
}

struct Walk {
    newest: HashMap<String, Newest>,
    floor: u64,
    head: u64,
    end: WalkEnd,
}

impl Walk {
    /// The held tokens, newest transfer first.
    fn held(&self) -> Vec<(&String, &Newest)> {
        let mut held: Vec<(&String, &Newest)> = self
            .newest
            .iter()
            .filter(|(_, entry)| entry.incoming)
            .collect();
        held.sort_by_key(|(_, entry)| std::cmp::Reverse(entry.rank()));
        held
    }

    /// What the next pass starts from, without the blocks a reorg could
    /// still change. `None` when it would be too big to keep.
    fn memory(&self, owner: &str, manager: &str, latest: u64) -> Option<String> {
        let head = self.head.min(latest.saturating_sub(REORG_MARGIN));
        let seen: Vec<Remembered> = self
            .newest
            .iter()
            .filter(|(_, entry)| entry.block <= head)
            .map(|(token, entry)| Remembered {
                token: token.clone(),
                block: entry.block,
                index: entry.index,
                incoming: entry.incoming,
            })
            .collect();
        if seen.len() > MAX_REMEMBERED {
            return None;
        }
        let text = serde_json::to_string(&ScanMemory {
            version: MEMORY_VERSION,
            owner: owner.to_string(),
            manager: manager.to_string(),
            floor: self.floor.min(head.saturating_add(1)),
            head,
            seen,
        })
        .ok()?;
        (text.len() <= MAX_MEMORY_BYTES).then_some(text)
    }
}

/// The two log filters of a wallet.
struct Filters {
    incoming: Vec<Value>,
    outgoing: Vec<Value>,
}

impl<C: ChainSource> UniswapV4<C> {
    /// The positions alone, for a caller with no way to take a partial
    /// answer: a scan that did not reach the start is an error here, never a
    /// short list.
    async fn discover_keys(&self, request: ProtocolRequest<'_>) -> Result<Vec<PositionKey>> {
        let found = self.scan(request).await?;
        if !found.complete {
            return Err(Error::Network(
                "the log scan did not reach the start of the history".into(),
            ));
        }
        Ok(found.keys)
    }

    async fn scan(&self, request: ProtocolRequest<'_>) -> Result<Discovery> {
        let deployment = deployment(request.chain).ok_or(Error::Unsupported)?;
        let node = Node::new(&self.client, &request)?;
        let owner = abi::address_word(node.address())?;
        let balance = node
            .call(deployment.manager, SEL_BALANCE_OF, &[owner])
            .await?
            .u128(0)?;
        let count = usize::try_from(balance)
            .ok()
            .filter(|count| *count <= MAX_POSITIONS)
            .ok_or_else(|| Error::Network("implausible position count".into()))?;
        if count == 0 {
            // Nothing is held, so there is nothing to remember: the first
            // position of a later pass is a recent one.
            return Ok(Discovery::complete(Vec::new()));
        }

        let owner_topic = format!("0x{}", hex(&owner));
        let filters = Filters {
            incoming: vec![json!(TRANSFER_TOPIC), Value::Null, json!(owner_topic)],
            outgoing: vec![json!(TRANSFER_TOPIC), json!(owner_topic)],
        };
        let latest = node.block_number().await?;
        let remembered = request
            .memory
            .and_then(|text| ScanMemory::load(text, node.address(), deployment.manager, latest));
        let had_memory = remembered.is_some();
        let mut walk = self
            .walk(&node, deployment, &filters, count, latest, remembered)
            .await?;
        if had_memory && matches!(walk.end, WalkEnd::Disagrees) {
            // What was remembered contradicts the chain: read it all again.
            walk = self
                .walk(&node, deployment, &filters, count, latest, None)
                .await?;
        }
        let complete = match walk.end {
            WalkEnd::Whole => true,
            WalkEnd::Short => false,
            WalkEnd::Disagrees => {
                return Err(Error::Network(format!(
                    "balanceOf promised {count} positions, the logs showed {}",
                    walk.held().len()
                )));
            }
        };
        Ok(Discovery {
            keys: walk
                .held()
                .into_iter()
                .map(|(token, _)| PositionKey::new(PROTOCOL, request.chain, token))
                .collect(),
            complete,
            memory: walk.memory(node.address(), deployment.manager, latest),
        })
    }

    /// Read the logs: first the blocks above what is remembered, oldest
    /// first, then below it, newest first, while `balanceOf` promises more
    /// than was found. One pass reads at most `windows_per_pass` windows.
    async fn walk(
        &self,
        node: &Node<'_, C>,
        deployment: &Deployment,
        filters: &Filters,
        count: usize,
        latest: u64,
        start: Option<ScanMemory>,
    ) -> Result<Walk> {
        // Nothing read yet is the empty range just above the newest block.
        let (newest, floor, head) = match &start {
            Some(memory) => (memory.newest(), memory.floor, memory.head),
            None => (HashMap::new(), latest.saturating_add(1), latest),
        };
        let mut walk = Walk {
            newest,
            floor,
            head,
            end: WalkEnd::Short,
        };
        let mut windows = 0u32;
        while walk.head < latest {
            if windows >= self.windows_per_pass {
                return Ok(walk);
            }
            let to = walk.head.saturating_add(SCAN_WINDOW).min(latest);
            let from = walk.head.saturating_add(1);
            read_window(node, deployment, filters, (from, to), &mut walk.newest).await?;
            walk.head = to;
            windows += 1;
        }
        loop {
            let held = walk.newest.values().filter(|entry| entry.incoming).count();
            if held >= count || walk.floor == 0 {
                walk.end = if held == count {
                    WalkEnd::Whole
                } else {
                    WalkEnd::Disagrees
                };
                return Ok(walk);
            }
            if windows >= self.windows_per_pass {
                return Ok(walk);
            }
            let window_end = walk.floor - 1;
            let window_start = window_end.saturating_sub(SCAN_WINDOW - 1);
            let span = (window_start, window_end);
            read_window(node, deployment, filters, span, &mut walk.newest).await?;
            walk.floor = window_start;
            windows += 1;
        }
    }

    async fn read(
        &self,
        request: ProtocolRequest<'_>,
        key: &PositionKey,
    ) -> Result<Option<PositionReading>> {
        let deployment = deployment(request.chain).ok_or(Error::Unsupported)?;
        let node = Node::new(&self.client, &request)?;
        let token_id = U256::from_decimal(&key.id)
            .map_err(|_| Error::InvalidInput("a position id is a token id".into()))?;

        let stored = node
            .call_many(&[(
                deployment.manager.to_string(),
                abi::calldata(SEL_POOL_AND_POSITION_INFO, &[token_id.to_be_bytes()])?,
            )])
            .await?
            .into_iter()
            .next()
            .flatten();
        // A refusal, or the all-zero answer of a cleared `positionInfo`, is a
        // burned NFT: a position that is gone, not an error.
        let Some(stored) = stored else {
            return Ok(None);
        };
        let Some(stored) = StoredPosition::parse(&stored)? else {
            return Ok(None);
        };

        let pool_id = stored.pool_id();
        let position_id = position_id(
            deployment.manager,
            stored.tick_lower,
            stored.tick_upper,
            &token_id,
        )?;
        let state = self
            .read_state(&node, deployment, &stored, &pool_id, &position_id)
            .await?;
        Ok(Some(Self::assemble(request, key, &stored, &state)?))
    }

    /// `StateView` and the currencies' decimals in one Multicall3 batch: all
    /// of it must be answered or nothing is.
    async fn read_state(
        &self,
        node: &Node<'_, C>,
        deployment: &Deployment,
        stored: &StoredPosition,
        pool_id: &[u8; 32],
        position_id: &[u8; 32],
    ) -> Result<PoolState> {
        let at_view = |selector: &str, args: &[[u8; 32]]| -> Result<(String, String)> {
            Ok((
                deployment.state_view.to_string(),
                abi::calldata(selector, args)?,
            ))
        };
        let mut calls = vec![
            at_view(SEL_GET_SLOT0, &[*pool_id])?,
            at_view(SEL_GET_POSITION_INFO, &[*pool_id, *position_id])?,
            at_view(
                SEL_GET_FEE_GROWTH_INSIDE,
                &[
                    *pool_id,
                    abi::int_word(stored.tick_lower),
                    abi::int_word(stored.tick_upper),
                ],
            )?,
        ];
        for currency in &stored.currencies {
            if currency != ZERO_ADDRESS {
                calls.push((currency.clone(), abi::calldata(SEL_DECIMALS, &[])?));
            }
        }
        // The tickers ride the same batch, after everything that must
        // answer: a refusal here costs a symbol and not the reading.
        for currency in &stored.currencies {
            if currency != ZERO_ADDRESS {
                calls.push((currency.clone(), abi::calldata(SYMBOL_SELECTOR, &[])?));
            }
        }
        let mut answers = node.call_many(&calls).await?.into_iter();
        let mut next = |what: &str| {
            answers
                .next()
                .flatten()
                .ok_or_else(|| Error::Network(format!("the node refused {what}")))
        };
        let slot0 = next("getSlot0")?;
        let position = next("getPositionInfo")?;
        let inside = next("getFeeGrowthInside")?;
        expect_words(&slot0, 4, "getSlot0")?;
        expect_words(&position, 3, "getPositionInfo")?;
        expect_words(&inside, 2, "getFeeGrowthInside")?;

        let mut decimals = [NATIVE_DECIMALS; 2];
        for (side, currency) in stored.currencies.iter().enumerate() {
            if currency != ZERO_ADDRESS {
                let answer = next("decimals()")?;
                expect_words(&answer, 1, "decimals()")?;
                decimals[side] = u8::try_from(answer.u128(0)?)
                    .map_err(|_| Error::Network("decimals() is a uint8".into()))?;
            }
        }
        let mut symbols = [None, None];
        for (side, currency) in stored.currencies.iter().enumerate() {
            if currency != ZERO_ADDRESS {
                symbols[side] = symbol_of(answers.next().flatten().as_ref());
            }
        }
        let sqrt_price = slot0.u256(0)?;
        if sqrt_price.is_zero() {
            return Err(Error::Network("the pool is not initialised".into()));
        }
        Ok(PoolState {
            sqrt_price,
            liquidity: position.u128(0)?,
            growth_last: [position.u256(1)?, position.u256(2)?],
            growth_inside: [inside.u256(0)?, inside.u256(1)?],
            decimals,
            symbols,
        })
    }

    fn assemble(
        request: ProtocolRequest<'_>,
        key: &PositionKey,
        stored: &StoredPosition,
        state: &PoolState,
    ) -> Result<PositionReading> {
        let chain = request.chain;
        let assets = [
            currency_asset(chain, &stored.currencies[0]),
            currency_asset(chain, &stored.currencies[1]),
        ];

        let mut uncollected = Vec::with_capacity(2);
        for (side, asset) in assets.iter().enumerate() {
            let raw = uncollected_fee(
                state.liquidity,
                state.growth_inside[side],
                state.growth_last[side],
            )?;
            uncollected.push(AssetAmount {
                asset: asset.clone(),
                amount: raw.to_amount(state.decimals[side])?,
            });
        }

        let (raw0, raw1) = principal(
            state.sqrt_price,
            stored.tick_lower,
            stored.tick_upper,
            state.liquidity,
        )?;
        let raw = [raw0, raw1];
        let mut legs = Vec::with_capacity(2);
        for (side, asset) in assets.iter().enumerate() {
            legs.push(CryptoLeg {
                direction: CryptoDirection::In,
                asset: asset.clone(),
                amount: raw[side].to_amount(state.decimals[side])?,
                counterparty: None,
            });
        }

        // v4 pays fees out whenever the position is modified, so a position
        // with no liquidity holds nothing: it is closed.
        let status = if state.liquidity == 0 {
            PositionStatus::Closed
        } else {
            PositionStatus::Open
        };
        Ok(PositionReading {
            key: key.clone(),
            account_id: request.account_id.to_string(),
            name: format!("Uniswap v4 #{}", key.id),
            kind: PositionKind::Pool,
            status,
            legs,
            // `collected` needs the history (`events`), which is not read.
            fees: FeeReading {
                collected: None,
                uncollected: Some(uncollected),
            },
            receipts: receipt_tokens(chain),
            // Any hook can move value `StateView` does not see (facts
            // section 10); only a pool with none is exact.
            approximate: stored.hooks != ZERO_ADDRESS,
            opened_at: None,
            closed_at: None,
            // The native coin is not a token: only contracts are listed.
            tokens: stored
                .currencies
                .iter()
                .enumerate()
                .filter(|(_, currency)| *currency != ZERO_ADDRESS)
                .map(|(side, currency)| LegToken {
                    token: TokenId::new(chain, currency),
                    symbol: state.symbols[side].clone(),
                    name: None,
                    decimals: Some(state.decimals[side]),
                })
                .collect(),
            read_at: crate::model::vault::now_iso(),
        })
    }
}

/// The transfers to and from the wallet in one range, merged into `newest`:
/// a transfer replaces the one a token has only when it is later in the
/// chain, whatever order the ranges are read in.
async fn read_window<C: ChainSource>(
    node: &Node<'_, C>,
    deployment: &Deployment,
    filters: &Filters,
    (from, to): (u64, u64),
    newest: &mut HashMap<String, Newest>,
) -> Result<()> {
    for (is_incoming, topics) in [(true, &filters.incoming), (false, &filters.outgoing)] {
        let logs = node
            .logs(deployment.manager, topics.clone(), from, to)
            .await?;
        for log in &logs {
            let Some((token, seen)) = parse_transfer(log, is_incoming)? else {
                continue;
            };
            let slot = newest.entry(token).or_insert(seen);
            if seen.rank() > slot.rank() {
                *slot = seen;
            }
        }
    }
    Ok(())
}

fn currency_asset(chain: Chain, currency: &str) -> CryptoAsset {
    if currency == ZERO_ADDRESS {
        CryptoAsset::Native { chain }
    } else {
        CryptoAsset::Token(TokenId::new(chain, currency))
    }
}

fn expect_words(answer: &Answer, words: usize, what: &str) -> Result<()> {
    if answer.len() != words {
        return Err(Error::Network(format!(
            "{what} did not answer {words} words"
        )));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// One `Transfer` log as the node gave it: the token and where it stands in
/// the chain. `None` for a log the node marks as removed by a reorg.
fn parse_transfer(log: &Value, incoming: bool) -> Result<Option<(String, Newest)>> {
    let bad = || Error::Network("a Transfer log of the wrong shape".into());
    if log["removed"].as_bool() == Some(true) {
        return Ok(None);
    }
    let topics = log["topics"].as_array().ok_or_else(bad)?;
    if topics.len() != 4 {
        return Err(bad());
    }
    let token = U256::from_hex(topics[3].as_str().ok_or_else(bad)?)?;
    let number = |field: &str| -> Result<u64> {
        let text = log[field].as_str().ok_or_else(bad)?;
        let digits = text.strip_prefix("0x").ok_or_else(bad)?;
        u64::from_str_radix(digits, 16).map_err(|_| bad())
    };
    Ok(Some((
        token.to_decimal(),
        Newest {
            block: number("blockNumber")?,
            index: number("logIndex")?,
            incoming,
        },
    )))
}

/// What `getPoolAndPositionInfo` answered, narrowed to what is used.
#[derive(Debug, Clone, PartialEq)]
struct StoredPosition {
    /// `currency0`, `currency1`: lowercase addresses, zero for the native coin.
    currencies: [String; 2],
    hooks: String,
    tick_lower: i64,
    tick_upper: i64,
    /// The five words of the `PoolKey` exactly as the contract answered:
    /// `abi.encode(poolKey)`, the preimage of the pool id.
    key_words: [[u8; 32]; POOL_KEY_WORDS],
}

impl StoredPosition {
    /// `Ok(None)` when the answer is a cleared position (a burned NFT).
    fn parse(answer: &Answer) -> Result<Option<Self>> {
        expect_words(answer, INFO_ANSWER_WORDS, "getPoolAndPositionInfo")?;
        let info = answer.word(POOL_KEY_WORDS)?;
        if info.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }
        // PositionInfo: 200 bits poolId | 24 bits tickUpper | 24 bits
        // tickLower | 8 bits hasSubscriber (facts section 8): in the 32 big
        // endian bytes the flag is byte 31, tickLower bytes 28-30 and
        // tickUpper bytes 25-27, each a signed 24-bit value.
        let tick_lower = signed_24(&info[28..31]);
        let tick_upper = signed_24(&info[25..28]);
        if tick_lower.abs() > MAX_TICK || tick_upper.abs() > MAX_TICK {
            return Err(Error::Network("a tick outside the protocol's range".into()));
        }
        if tick_lower >= tick_upper {
            return Err(Error::Network("a range has lower below upper".into()));
        }
        // fee (uint24) and tickSpacing (int24) are checked for shape, they
        // enter the pool id as the contract wrote them.
        if answer.u128(2)? > 0xff_ffff {
            return Err(Error::Network("a fee is a uint24".into()));
        }
        if answer.int(3)? <= 0 {
            return Err(Error::Network("a tick spacing is positive".into()));
        }
        let mut key_words = [[0u8; 32]; POOL_KEY_WORDS];
        for (slot, word) in key_words.iter_mut().enumerate() {
            *word = *answer.word(slot)?;
        }
        Ok(Some(Self {
            currencies: [answer.address(0)?, answer.address(1)?],
            hooks: answer.address(4)?,
            tick_lower,
            tick_upper,
            key_words,
        }))
    }

    /// `PoolId.toId()`: `keccak256(abi.encode(poolKey))`, five words of 32
    /// bytes (facts section 8).
    fn pool_id(&self) -> [u8; 32] {
        keccak256(&self.key_words.concat())
    }
}

/// A big-endian two's-complement 24-bit value.
fn signed_24(bytes: &[u8]) -> i64 {
    let raw = (i64::from(bytes[0]) << 16) | (i64::from(bytes[1]) << 8) | i64::from(bytes[2]);
    if raw >= 1 << 23 { raw - (1 << 24) } else { raw }
}

/// `Position.calculatePositionKey` for a position the manager owns inside the
/// pool, the token id as salt: `keccak256(abi.encodePacked(manager, tickLower,
/// tickUpper, bytes32(tokenId)))`, 20 + 3 + 3 + 32 bytes (facts section 8).
/// Packed, not `abi.encode`: the classic v4 reader bug is mixing the two.
fn position_id(
    manager: &str,
    tick_lower: i64,
    tick_upper: i64,
    token_id: &U256,
) -> Result<[u8; 32]> {
    let manager = abi::address_word(manager)?;
    let mut packed = Vec::with_capacity(58);
    packed.extend_from_slice(&manager[12..]);
    packed.extend_from_slice(&tick_lower.to_be_bytes()[5..]);
    packed.extend_from_slice(&tick_upper.to_be_bytes()[5..]);
    packed.extend_from_slice(&token_id.to_be_bytes());
    Ok(keccak256(&packed))
}

/// What `StateView` and the currencies answered.
#[derive(Debug, Clone, PartialEq)]
struct PoolState {
    sqrt_price: U256,
    liquidity: u128,
    growth_last: [U256; 2],
    growth_inside: [U256; 2],
    decimals: [u8; 2],
    symbols: [Option<String>; 2],
}

/// `liquidity * (inside - last) / 2^128`: the difference wraps modulo 2^256
/// (the accumulators are defined to overflow), the product is taken on 512
/// bits ([`U256::mul_div_floor`]) so nothing is lost before the division.
fn uncollected_fee(liquidity: u128, inside: U256, last: U256) -> Result<U256> {
    let q128 = U256::power_of_two(128).ok_or_else(overflow)?;
    inside
        .wrapping_sub(last)
        .mul_div_floor(U256::from_u128(liquidity), q128)
        .ok_or_else(overflow)
}

fn overflow() -> Error {
    Error::Network("a figure does not fit a word".into())
}

/// Keccak-256 (the original padding byte `0x01`, not NIST SHA3-256's `0x06`).
///
/// Written here because the crate carries no such dependency and may not gain
/// one (ADR 01). The round constants and rotation offsets are *derived* from
/// their definitions in FIPS 202 (the LFSR of section 3.2.5 and the walk of
/// 3.2.2) rather than typed from memory, and the tests pin the result to the
/// published hashes of the empty string and of `abc` and to the pool id an
/// independent implementation computed.
fn keccak256(data: &[u8]) -> [u8; 32] {
    const RATE: usize = 136;
    let constants = keccak_round_constants();
    let rotations = keccak_rotations();
    let mut state = [0u64; 25];

    let mut padded = data.to_vec();
    padded.push(0x01);
    while !padded.len().is_multiple_of(RATE) {
        padded.push(0);
    }
    let last = padded.len() - 1;
    padded[last] |= 0x80;

    for block in padded.chunks(RATE) {
        for (lane, bytes) in state.iter_mut().zip(block.chunks(8)) {
            let mut word = [0u8; 8];
            word.copy_from_slice(bytes);
            *lane ^= u64::from_le_bytes(word);
        }
        keccak_f(&mut state, &constants, &rotations);
    }

    let mut out = [0u8; 32];
    for (bytes, lane) in out.chunks_mut(8).zip(state.iter()) {
        bytes.copy_from_slice(&lane.to_le_bytes());
    }
    out
}

/// The 24 round constants: bit `2^j - 1` of round `r`'s constant is the LFSR
/// output `rc(j + 7r)` (FIPS 202, algorithm 5).
fn keccak_round_constants() -> [u64; 24] {
    let lfsr_bit = |step: usize| -> bool {
        let mut register: u16 = 1;
        for _ in 0..step % 255 {
            register <<= 1;
            if register & 0x100 != 0 {
                // x^8 + x^6 + x^5 + x^4 + 1
                register ^= 0x171;
            }
        }
        register & 1 == 1
    };
    let mut constants = [0u64; 24];
    for (round, constant) in constants.iter_mut().enumerate() {
        for bit in 0..7 {
            if lfsr_bit(bit + 7 * round) {
                *constant |= 1u64 << ((1u32 << bit) - 1);
            }
        }
    }
    constants
}

/// Rotation offsets of the lanes, indexed `column + 5 * row` (FIPS 202,
/// algorithm 2): the walk `(x, y) -> (y, 2x + 3y)` from `(1, 0)`.
fn keccak_rotations() -> [u32; 25] {
    let mut offsets = [0u32; 25];
    let (mut column, mut row) = (1usize, 0usize);
    for step in 0..24u32 {
        offsets[column + 5 * row] = ((step + 1) * (step + 2) / 2) % 64;
        (column, row) = (row, (2 * column + 3 * row) % 5);
    }
    offsets
}

fn keccak_f(state: &mut [u64; 25], constants: &[u64; 24], rotations: &[u32; 25]) {
    for constant in constants {
        // theta
        let mut parity = [0u64; 5];
        for (column, slot) in parity.iter_mut().enumerate() {
            *slot = (0..5).fold(0, |total, row| total ^ state[column + 5 * row]);
        }
        for column in 0..5 {
            let mix = parity[(column + 4) % 5] ^ parity[(column + 1) % 5].rotate_left(1);
            for row in 0..5 {
                state[column + 5 * row] ^= mix;
            }
        }
        // rho and pi
        let mut moved = [0u64; 25];
        for column in 0..5 {
            for row in 0..5 {
                let lane = state[column + 5 * row].rotate_left(rotations[column + 5 * row]);
                moved[row + 5 * ((2 * column + 3 * row) % 5)] = lane;
            }
        }
        // chi
        for column in 0..5 {
            for row in 0..5 {
                state[column + 5 * row] = moved[column + 5 * row]
                    ^ (!moved[(column + 1) % 5 + 5 * row] & moved[(column + 2) % 5 + 5 * row]);
            }
        }
        // iota
        state[0] ^= constant;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use super::super::node::stub;
    use super::*;

    const OWNER: &str = "0x1111111111111111111111111111111111111111";
    const TOKEN1: &str = "0x00000000000000000000000000000000000a0002";
    const MANAGER: &str = "0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e";
    const STATE_VIEW: &str = "0x7ffe42c4a5deea5b0fec41c94c136cf115597227";

    /// Computed outside Rust by an independent Python Keccak-256 (itself
    /// checked against the published hashes of "" and "abc") from the words
    /// of `getPoolAndPositionInfo.hookless` / `.hooked`.
    const POOL_ID_HOOKLESS: &str =
        "1721998a98804d835c4728db2e8a7d74cf49977491bbdbd4dce51b462eaf5b03";
    const POOL_ID_HOOKED: &str = "c0705a6d3450e782860f37ebe8048d72552a3f9e023d40a69eeeb28bf4ac2f29";
    /// `keccak256(abi.encodePacked(MANAGER, int24 -201000, int24 -200000,
    /// bytes32(4242)))`, same independent computation.
    const POSITION_ID: &str = "32d7586976c18654ceafb30a0a17ac2f0092de77481cd19c5c47d46c9b80798d";

    // Fixtures: every file is `{"_source": …, "result": …}`. None is a live
    // node answer: they are hand-built from the layouts of the facts document
    // with synthetic addresses, because this layer had no network. Every
    // expected number below was computed outside Rust (Python integers and
    // 120-digit decimals) from the same words.
    macro_rules! fixture {
        ($name:literal) => {
            fixture_result(include_str!(concat!(
                "fixtures/uniswap-v4/",
                $name,
                ".json"
            )))
        };
    }

    fn fixture_value(text: &str) -> Value {
        let value: Value = serde_json::from_str(text).expect("a fixture is JSON");
        value["result"].clone()
    }

    fn fixture_result(text: &str) -> String {
        fixture_value(text).as_str().expect("a result").to_string()
    }

    fn word(value: u128) -> String {
        format!("{value:064x}")
    }

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
            .collect()
    }

    fn be_usize(bytes: &[u8]) -> usize {
        bytes
            .iter()
            .fold(0usize, |total, byte| total * 256 + usize::from(*byte))
    }

    /// The calls inside a Multicall3 `aggregate3` request: `(target,
    /// calldata)`.
    fn decode_aggregate(calldata: &str) -> Vec<(String, String)> {
        let bytes = unhex(&calldata[2 + 8..]);
        let array = be_usize(&bytes[..32]);
        let count = be_usize(&bytes[array..array + 32]);
        let heads = array + 32;
        (0..count)
            .map(|index| {
                let offset = be_usize(&bytes[heads + index * 32..heads + index * 32 + 32]);
                let tuple = heads + offset;
                let target = format!("0x{}", hex(&bytes[tuple + 12..tuple + 32]));
                let data_at = tuple + be_usize(&bytes[tuple + 64..tuple + 96]);
                let len = be_usize(&bytes[data_at..data_at + 32]);
                (target, hex(&bytes[data_at + 32..data_at + 32 + len]))
            })
            .collect()
    }

    /// An `aggregate3` answer: `(success, returnData)` per call.
    fn encode_results(results: &[Option<String>]) -> String {
        let mut elements = Vec::new();
        for result in results {
            let data = result.as_deref().map(|text| unhex(&text[2..]));
            let data = data.unwrap_or_default();
            let mut item = unhex(&word(u128::from(result.is_some())));
            item.extend(unhex(&word(64)));
            item.extend(unhex(&word(data.len() as u128)));
            item.extend(&data);
            item.resize(item.len() + (32 - data.len() % 32) % 32, 0);
            elements.push(item);
        }
        let mut out = unhex(&word(32));
        out.extend(unhex(&word(elements.len() as u128)));
        let mut offset = elements.len() * 32;
        for item in &elements {
            out.extend(unhex(&word(offset as u128)));
            offset += item.len();
        }
        for item in &elements {
            out.extend(item);
        }
        format!("0x{}", hex(&out))
    }

    /// What the stub node answers for a call: `Some(return data)` or `None`
    /// for a revert.
    type Handler = Box<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;

    /// A node that answers `eth_call`s through `handler`, directly or inside
    /// Multicall3.
    async fn node(handler: Handler) -> (String, stub::Seen) {
        stub::serve(move |method, params, _index| {
            assert_eq!(method, "eth_call");
            let to = params[0]["to"].as_str().expect("to").to_lowercase();
            let data = params[0]["data"].as_str().expect("data");
            if to == abi::MULTICALL3.to_lowercase() {
                let results: Vec<Option<String>> = decode_aggregate(data)
                    .iter()
                    .map(|(target, calldata)| handler(target, calldata))
                    .collect();
                return json!(encode_results(&results));
            }
            match handler(&to, &data[2..]) {
                Some(answer) => json!(answer),
                None => Value::Null,
            }
        })
        .await
    }

    /// The answers of one position scenario. The handler also checks the
    /// arguments `StateView` is asked with: a wrong pool id or position id is
    /// a refusal, which fails the read.
    struct Scenario {
        info: String,
        pool_id: &'static str,
        position: String,
        inside: String,
        /// `None`: the token refuses `symbol()`.
        symbol1: Option<String>,
    }

    impl Scenario {
        fn hookless() -> Self {
            Self {
                info: fixture!("getPoolAndPositionInfo.hookless"),
                pool_id: POOL_ID_HOOKLESS,
                position: fixture!("getPositionInfo.open"),
                inside: fixture!("getFeeGrowthInside.open"),
                symbol1: Some(fixture!("symbol.token1")),
            }
        }

        fn handler(self) -> Handler {
            Box::new(move |target, calldata| {
                let selector = &calldata[..8];
                let arguments = &calldata[8..];
                let lower = hex(&abi::int_word(-201_000));
                let upper = hex(&abi::int_word(-200_000));
                match (target, selector) {
                    (MANAGER, SEL_POOL_AND_POSITION_INFO) => {
                        (arguments == word(4242)).then(|| self.info.clone())
                    }
                    (STATE_VIEW, SEL_GET_SLOT0) => {
                        (arguments == self.pool_id).then(|| fixture!("getSlot0.in-range"))
                    }
                    (STATE_VIEW, SEL_GET_POSITION_INFO) => (arguments
                        == format!("{}{POSITION_ID}", self.pool_id))
                    .then(|| self.position.clone()),
                    (STATE_VIEW, SEL_GET_FEE_GROWTH_INSIDE) => (arguments
                        == format!("{}{lower}{upper}", self.pool_id))
                    .then(|| self.inside.clone()),
                    (TOKEN1, SEL_DECIMALS) => Some(fixture!("decimals.token1")),
                    (TOKEN1, SYMBOL_SELECTOR) => self.symbol1.clone(),
                    _ => None,
                }
            })
        }
    }

    fn adapter_for_tests() -> UniswapV4<stub::Loopback> {
        UniswapV4 {
            client: stub::client(),
            windows_per_pass: MAX_SCAN_WINDOWS,
        }
    }

    fn adapter_with_windows(windows_per_pass: u32) -> UniswapV4<stub::Loopback> {
        UniswapV4 {
            client: stub::client(),
            windows_per_pass,
        }
    }

    fn request_with<'req>(url: &'req str, memory: Option<&'req str>) -> ProtocolRequest<'req> {
        ProtocolRequest {
            memory,
            ..request(url)
        }
    }

    fn request(url: &str) -> ProtocolRequest<'_> {
        ProtocolRequest {
            chain: Chain::Ethereum,
            address: OWNER,
            account_id: "cacc-1",
            since: None,
            rpc: Some(url),
            memory: None,
        }
    }

    fn key() -> PositionKey {
        PositionKey::new(PROTOCOL, Chain::Ethereum, "4242")
    }

    async fn read(scenario: Scenario) -> Result<Option<PositionReading>> {
        let (url, _) = node(scenario.handler()).await;
        adapter_for_tests()
            .read_position(request(&url), &key())
            .await
    }

    // --- keccak and the ids ------------------------------------------------

    #[test]
    fn keccak256_matches_the_published_vectors() {
        // The two hashes the facts document validates its own implementation
        // with.
        assert_eq!(
            hex(&keccak256(b"")),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        assert_eq!(
            hex(&keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        // A message of exactly one rate (136 bytes) and one longer than it
        // pad into different block counts; the hash of the former and the
        // latter must differ and be stable.
        let exact = keccak256(&[0x61; 136]);
        let longer = keccak256(&[0x61; 137]);
        assert_ne!(exact, longer);
        // The first round constants of the specification, derived by the
        // LFSR.
        let constants = keccak_round_constants();
        assert_eq!(constants[0], 0x0000_0000_0000_0001);
        assert_eq!(constants[1], 0x0000_0000_0000_8082);
        assert_eq!(constants[23], 0x8000_0000_8000_8008);
    }

    #[test]
    fn the_selectors_and_the_topic_are_the_hashes_of_their_signatures() {
        // Recomputed here with the Keccak-256 above, which the test before
        // pins to the published vectors.
        let selector = |signature: &str| hex(&keccak256(signature.as_bytes())[..4]);
        assert_eq!(selector("balanceOf(address)"), SEL_BALANCE_OF);
        assert_eq!(
            selector("getPoolAndPositionInfo(uint256)"),
            SEL_POOL_AND_POSITION_INFO
        );
        assert_eq!(selector("getSlot0(bytes32)"), SEL_GET_SLOT0);
        assert_eq!(
            selector("getPositionInfo(bytes32,bytes32)"),
            SEL_GET_POSITION_INFO
        );
        assert_eq!(
            selector("getFeeGrowthInside(bytes32,int24,int24)"),
            SEL_GET_FEE_GROWTH_INSIDE
        );
        assert_eq!(selector("decimals()"), SEL_DECIMALS);
        assert_eq!(selector("symbol()"), SYMBOL_SELECTOR);
        assert_eq!(
            format!(
                "0x{}",
                hex(&keccak256(b"Transfer(address,address,uint256)"))
            ),
            TRANSFER_TOPIC
        );
    }

    #[test]
    fn the_pool_id_and_the_position_id_match_an_independent_computation() {
        let hookless = StoredPosition::parse(
            &Answer::parse(&fixture!("getPoolAndPositionInfo.hookless")).expect("an answer"),
        )
        .expect("parsed")
        .expect("a position");
        assert_eq!(hex(&hookless.pool_id()), POOL_ID_HOOKLESS);
        assert_eq!(
            (hookless.tick_lower, hookless.tick_upper),
            (-201_000, -200_000)
        );
        let hooked = StoredPosition::parse(
            &Answer::parse(&fixture!("getPoolAndPositionInfo.hooked")).expect("an answer"),
        )
        .expect("parsed")
        .expect("a position");
        assert_eq!(hex(&hooked.pool_id()), POOL_ID_HOOKED);
        assert_eq!(
            hex(&position_id(MANAGER, -201_000, -200_000, &U256::from_u128(4242)).expect("an id")),
            POSITION_ID
        );
    }

    // --- the tables --------------------------------------------------------

    #[test]
    fn the_manager_is_the_receipt_on_every_network_the_protocol_lives_on() {
        let expected = [
            (Chain::Ethereum, MANAGER),
            (
                Chain::Optimism,
                "0x3c3ea4b57a46241e54610e5f022e5c45859a1017",
            ),
            (Chain::Bsc, "0x7a4a5c919ae2541aed11041a1aeee68f1287f95b"),
            (Chain::Polygon, "0x1ec2ebf4f37e7363fdfe3551602425af0b3ceef9"),
            (
                Chain::Arbitrum,
                "0xd88f38f930b7952f2db2432cb002e7abbf3dd869",
            ),
            (Chain::Base, "0x7c5f5a4bbd8fd63184577525326123b519429bdc"),
        ];
        for (chain, manager) in expected {
            assert_eq!(
                receipt_tokens(chain),
                vec![CryptoAsset::Token(TokenId::new(chain, manager))],
                "{chain:?}"
            );
        }
        assert!(receipt_tokens(Chain::Solana).is_empty());
        assert!(receipt_tokens(Chain::Avalanche).is_empty());
    }

    #[test]
    fn the_adapter_lives_where_the_facts_say_and_nowhere_else() {
        let adapter = adapter_for_tests();
        assert_eq!(adapter.protocol(), PROTOCOL);
        for chain in [
            Chain::Ethereum,
            Chain::Optimism,
            Chain::Bsc,
            Chain::Polygon,
            Chain::Arbitrum,
            Chain::Base,
        ] {
            assert!(adapter.supports(chain), "{chain:?}");
        }
        assert!(!adapter.supports(Chain::Avalanche));
        assert!(!adapter.supports(Chain::Bitcoin));
    }

    // --- reading one position ----------------------------------------------

    #[tokio::test]
    async fn a_hookless_position_reads_its_principal_and_its_fees_and_is_exact() {
        let reading = read(Scenario::hookless())
            .await
            .expect("a read")
            .expect("the position is there");
        assert_eq!(reading.status, PositionStatus::Open);
        assert_eq!(reading.kind, PositionKind::Pool);
        assert_eq!(reading.name, "Uniswap v4 #4242");
        assert!(!reading.approximate, "no hook: exact");
        assert_eq!(reading.receipts, receipt_tokens(Chain::Ethereum));

        let native = CryptoAsset::Native {
            chain: Chain::Ethereum,
        };
        let token1 = CryptoAsset::Token(TokenId::new(Chain::Ethereum, TOKEN1));
        // fee0 = (5*2^128 - 2*2^128) * 12345678901234567 / 2^128 = 3 * L raw
        // = 37037036703703701 raw native (18 places); fee1 = (3*2^128 +
        // 12345678901234567890123456789) * L / 2^128 floored =
        // 37037036704151610 raw (6 places). Python integers.
        let uncollected = reading.fees.uncollected.expect("fees were read");
        assert_eq!(uncollected[0].asset, native);
        assert_eq!(uncollected[0].amount.to_string(), "0.037037036703703701");
        assert_eq!(uncollected[1].asset, token1);
        assert_eq!(uncollected[1].amount.to_string(), "37037036704.15161");
        assert!(
            reading.fees.collected.is_none(),
            "what was collected needs the history, which is not read: unknown, not zero"
        );

        // Principal at tick -200500 inside -201000..-200000, L =
        // 12345678901234567: 6880193878732039285 raw native and 13503043462
        // raw token1, from 120-digit decimals outside Rust. The tick math is
        // not bit-identical to the protocol's, so 12 digits are compared.
        assert_eq!(reading.legs.len(), 2);
        assert_eq!(reading.legs[0].asset, native, "currency0 zero is the coin");
        assert_eq!(reading.legs[0].direction, CryptoDirection::In);
        assert!(
            reading.legs[0]
                .amount
                .to_string()
                .starts_with("6.880193878732"),
            "{}",
            reading.legs[0].amount
        );
        assert_eq!(reading.legs[1].asset, token1);
        assert!(
            reading.legs[1].amount.to_string().starts_with("13503.0434"),
            "{}",
            reading.legs[1].amount
        );
    }

    #[tokio::test]
    async fn a_pool_behind_a_hook_is_approximate() {
        let mut scenario = Scenario::hookless();
        scenario.info = fixture!("getPoolAndPositionInfo.hooked");
        scenario.pool_id = POOL_ID_HOOKED;
        let reading = read(scenario)
            .await
            .expect("a read")
            .expect("the position is there");
        assert!(reading.approximate, "the flag travels with the numbers");
        assert_eq!(reading.status, PositionStatus::Open);
    }

    #[tokio::test]
    async fn a_position_without_liquidity_is_closed_and_its_fees_are_a_real_zero() {
        let mut scenario = Scenario::hookless();
        scenario.position = fixture!("getPositionInfo.empty");
        // Nothing earned since the snapshot either.
        scenario.inside = format!("0x{}{}", word(0), word(0));
        let reading = read(scenario)
            .await
            .expect("a read")
            .expect("a closed position is a position");
        assert_eq!(reading.status, PositionStatus::Closed);
        assert!(super::super::is_closed(&reading.legs));
        let uncollected = reading.fees.uncollected.expect("read");
        assert!(uncollected.iter().all(|fee| fee.amount.is_zero()));
    }

    #[tokio::test]
    async fn the_fee_growth_that_wrapped_is_read_through_the_whole_path() {
        // inside1 = 12345678900000000000000000000 against a snapshot of
        // 2^256 - 5e27: the wrapped delta is 17345678900000000000000000000
        // and the fee floor(delta * L / 2^128) = 629313 raw = 0.629313 in 6
        // places. A checked subtraction would fail, a saturating one say 0.
        let mut scenario = Scenario::hookless();
        scenario.position = fixture!("getPositionInfo.wrapped");
        scenario.inside = fixture!("getFeeGrowthInside.wrapped");
        let reading = read(scenario)
            .await
            .expect("a read")
            .expect("the position is there");
        let fee = &reading.fees.uncollected.expect("read")[1];
        assert_eq!(fee.amount.to_string(), "0.629313");
    }

    #[tokio::test]
    async fn a_burned_nft_is_gone_and_not_an_error() {
        // The manager clears the position: all-zero words, not a revert.
        let mut scenario = Scenario::hookless();
        scenario.info = fixture!("getPoolAndPositionInfo.burned");
        assert!(read(scenario).await.expect("an answer").is_none());
        // And a manager that refuses the call says the same.
        let (url, _) = node(Box::new(|_, _| None)).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a refusal of the manager is an answer");
        assert!(reading.is_none());
    }

    #[tokio::test]
    async fn an_answer_of_the_wrong_shape_is_refused_by_a_read() {
        let mut scenario = Scenario::hookless();
        scenario.info = format!("0x{}{}", word(1), word(2));
        assert!(read(scenario).await.is_err());

        // A tick outside the protocol's range is not a position: the packed
        // info word is rewritten with tickLower -900000 (0xf21f60).
        let mut scenario = Scenario::hookless();
        let mut info = scenario.info.clone();
        let end = info.len();
        info.replace_range(end - 8..end - 2, "f21f60");
        scenario.info = info;
        assert!(read(scenario).await.is_err());

        // StateView answering a different number of words than documented.
        let mut scenario = Scenario::hookless();
        scenario.inside = format!("0x{}", word(1));
        assert!(read(scenario).await.is_err());

        // A pool that was never initialised has no price.
        let handler = Scenario::hookless().handler();
        let zero_price: Handler = Box::new(move |target, calldata| {
            if &calldata[..8] == SEL_GET_SLOT0 {
                return Some(format!("0x{}{}{}{}", word(0), word(0), word(0), word(0)));
            }
            handler(target, calldata)
        });
        let (url, _) = node(zero_price).await;
        assert!(
            adapter_for_tests()
                .read_position(request(&url), &key())
                .await
                .is_err()
        );

        // A position id that is not a number.
        let (url, _) = node(Scenario::hookless().handler()).await;
        let bad = PositionKey::new(PROTOCOL, Chain::Ethereum, "not-a-token");
        assert!(matches!(
            adapter_for_tests().read_position(request(&url), &bad).await,
            Err(Error::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn a_state_view_that_refuses_a_call_is_an_error_and_not_a_zero() {
        let inner = Scenario::hookless().handler();
        let handler: Handler = Box::new(move |target, calldata| {
            if &calldata[..8] == SEL_GET_FEE_GROWTH_INSIDE {
                return None;
            }
            inner(target, calldata)
        });
        let (url, _) = node(handler).await;
        assert!(
            adapter_for_tests()
                .read_position(request(&url), &key())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_native_coin_asks_for_no_decimals() {
        // The coin has no contract: the batch holds three StateView calls and
        // one `decimals()` (token1), and the node is asked once.
        let (url, seen) = node(Scenario::hookless().handler()).await;
        adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position");
        let bodies = seen.lock().expect("the log").len();
        assert_eq!(bodies, 2, "positions, then one batch on StateView");
    }

    #[tokio::test]
    async fn the_native_coin_is_not_listed_and_the_token_is() {
        let reading = read(Scenario::hookless())
            .await
            .expect("a read")
            .expect("the position");
        assert_eq!(
            reading.tokens,
            vec![LegToken {
                token: TokenId::new(Chain::Ethereum, TOKEN1),
                symbol: Some("WETH".into()),
                name: None,
                decimals: Some(6),
            }],
            "one token: the currency with no contract is the coin"
        );
    }

    #[tokio::test]
    async fn a_token_that_refuses_symbol_or_answers_a_non_string_has_no_symbol() {
        let mut refusing = Scenario::hookless();
        refusing.symbol1 = None;
        let reading = read(refusing).await.expect("a read").expect("the position");
        assert_eq!(reading.tokens.len(), 1);
        assert_eq!(reading.tokens[0].symbol, None);
        assert_eq!(reading.tokens[0].decimals, Some(6));

        let mut bare = Scenario::hookless();
        bare.symbol1 = Some(format!("0x{}", word(7)));
        let reading = read(bare).await.expect("a read").expect("the position");
        assert_eq!(reading.tokens[0].symbol, None);
    }

    #[tokio::test]
    async fn an_oversized_or_non_utf8_symbol_is_passed_on_for_the_registration_to_clip() {
        let encode = |bytes: &[u8]| {
            let mut text = format!("0x{}{}", word(32), word(bytes.len() as u128));
            for chunk in bytes.chunks(32) {
                text.push_str(&hex(chunk));
                text.push_str(&"00".repeat(32 - chunk.len()));
            }
            text
        };
        let mut long = Scenario::hookless();
        long.symbol1 = Some(encode(&[b'A'; 300]));
        let reading = read(long).await.expect("a read").expect("the position");
        assert_eq!(
            reading.tokens[0].symbol.as_deref(),
            Some("A".repeat(300).as_str())
        );

        let mut broken = Scenario::hookless();
        broken.symbol1 = Some(encode(&[0xff, 0xfe, b'X']));
        let reading = read(broken).await.expect("a read").expect("the position");
        let symbol = reading.tokens[0].symbol.clone().expect("a lossy symbol");
        assert!(symbol.ends_with('X'), "{symbol}");
    }

    // --- discovery ---------------------------------------------------------

    type SpanLog = Arc<Mutex<Vec<(u64, u64)>>>;

    /// A node with the wallet's logs: `incoming` for transfers to it,
    /// `outgoing` from it, the latest block 25 000 and a balance of
    /// `balance`. Refuses any `eth_getLogs` wider than `widest` blocks, and
    /// records every range it was asked.
    fn log_node(
        balance: u128,
        widest: u64,
        incoming: Value,
        outgoing: Value,
    ) -> (SpanLog, impl Fn(&str, &Value, usize) -> Value) {
        log_node_at(
            Arc::new(AtomicU64::new(25_000)),
            balance,
            widest,
            incoming,
            outgoing,
        )
    }

    /// [`log_node`] whose latest block can move between passes.
    fn log_node_at(
        latest: Arc<AtomicU64>,
        balance: u128,
        widest: u64,
        incoming: Value,
        outgoing: Value,
    ) -> (SpanLog, impl Fn(&str, &Value, usize) -> Value) {
        let spans: SpanLog = Arc::default();
        let log = Arc::clone(&spans);
        let owner_topic = format!("0x{}", hex(&abi::address_word(OWNER).expect("an address")));
        let handler = move |method: &str, params: &Value, _index: usize| -> Value {
            match method {
                "eth_call" => json!(format!("0x{}", word(balance))),
                "eth_blockNumber" => json!(format!("0x{:x}", latest.load(Ordering::SeqCst))),
                "eth_getLogs" => {
                    let filter = &params[0];
                    let block = |field: &str| {
                        u64::from_str_radix(
                            filter[field]
                                .as_str()
                                .expect("block")
                                .trim_start_matches("0x"),
                            16,
                        )
                        .expect("a block")
                    };
                    let (from, to) = (block("fromBlock"), block("toBlock"));
                    log.lock().expect("the log").push((from, to));
                    if to - from + 1 > widest {
                        return Value::Null;
                    }
                    let topics = filter["topics"].as_array().expect("topics");
                    assert_eq!(topics[0], TRANSFER_TOPIC);
                    let is_incoming = topics.len() == 3;
                    if is_incoming {
                        assert_eq!(topics[2], owner_topic.as_str());
                    } else {
                        assert_eq!(topics[1], owner_topic.as_str());
                    }
                    let source = if is_incoming { &incoming } else { &outgoing };
                    let inside: Vec<Value> = source
                        .as_array()
                        .expect("logs")
                        .iter()
                        .filter(|entry| {
                            let number = u64::from_str_radix(
                                entry["blockNumber"]
                                    .as_str()
                                    .expect("block")
                                    .trim_start_matches("0x"),
                                16,
                            )
                            .expect("a block");
                            (from..=to).contains(&number)
                        })
                        .cloned()
                        .collect();
                    Value::Array(inside)
                }
                other => panic!("unexpected {other}"),
            }
        };
        (spans, handler)
    }

    #[tokio::test]
    async fn discovery_lists_the_nfts_the_wallet_still_holds() {
        // Tokens 11, 22 and 33 came in; 33 went out again at block 2500.
        // balanceOf says 2: the scan reads three windows (25 000-15 001,
        // 15 000-5 001, 5 000-0), and 33 is not listed.
        let (spans, handler) = log_node(
            2,
            u64::MAX,
            fixture_value(include_str!("fixtures/uniswap-v4/logs.incoming.json")),
            fixture_value(include_str!("fixtures/uniswap-v4/logs.outgoing.json")),
        );
        let (url, _) = stub::serve(handler).await;
        let found = adapter_for_tests()
            .discover(request(&url))
            .await
            .expect("a discovery");
        assert_eq!(
            found,
            vec![
                PositionKey::new(PROTOCOL, Chain::Ethereum, "11"),
                PositionKey::new(PROTOCOL, Chain::Ethereum, "22"),
            ],
            "newest first, the one that left is not there"
        );
        assert_eq!(
            spans.lock().expect("log").len(),
            6,
            "three windows, two filters"
        );
    }

    #[tokio::test]
    async fn discovery_stops_as_soon_as_the_promised_positions_are_found() {
        // Only token 11 (block 24 500) is wanted: the newest window has it
        // and the scan does not go back.
        let (spans, handler) = log_node(
            1,
            u64::MAX,
            fixture_value(include_str!("fixtures/uniswap-v4/logs.incoming.json")),
            json!([]),
        );
        let (url, _) = stub::serve(handler).await;
        let found = adapter_for_tests()
            .discover(request(&url))
            .await
            .expect("a discovery");
        assert_eq!(
            found,
            vec![PositionKey::new(PROTOCOL, Chain::Ethereum, "11")]
        );
        assert_eq!(
            spans.lock().expect("log").len(),
            2,
            "one window, two filters"
        );
    }

    #[tokio::test]
    async fn the_log_ranges_are_split_to_fit_what_an_endpoint_accepts() {
        // The node refuses any range wider than 2 500 blocks (the way a
        // public endpoint does; the documented ceiling is 10 000, facts
        // section 9). The reader asks windows of at most 10 000, halves a
        // refused one and still finds every token.
        let (spans, handler) = log_node(
            2,
            2_500,
            fixture_value(include_str!("fixtures/uniswap-v4/logs.incoming.json")),
            fixture_value(include_str!("fixtures/uniswap-v4/logs.outgoing.json")),
        );
        let (url, _) = stub::serve(handler).await;
        let found = adapter_for_tests()
            .discover(request(&url))
            .await
            .expect("a discovery");
        assert_eq!(found.len(), 2);
        let asked = spans.lock().expect("log").clone();
        assert!(
            asked.iter().all(|(from, to)| to - from < SCAN_WINDOW),
            "no request wider than the documented 10 000: {asked:?}"
        );
        assert!(
            asked.iter().any(|(from, to)| to - from + 1 > 2_500),
            "the first, too wide range was tried and refused"
        );
        assert!(
            asked.iter().any(|(from, to)| to - from < 2_500),
            "and narrowed"
        );
    }

    #[tokio::test]
    async fn a_wallet_with_no_nft_costs_one_call_and_finds_nothing() {
        let (url, seen) = node(Box::new(|_, _| Some(fixture!("balanceOf.zero")))).await;
        let found = adapter_for_tests()
            .discover(request(&url))
            .await
            .expect("a discovery");
        assert!(found.is_empty());
        assert_eq!(seen.lock().expect("the log").len(), 1);
        // The fixture of a balance of two is the one the scan tests use.
        assert_eq!(fixture!("balanceOf.two"), format!("0x{}", word(2)));
    }

    #[tokio::test]
    async fn a_scan_that_finds_fewer_than_balance_of_promised_is_an_error() {
        // balanceOf says 5, the logs show two held tokens: a short list would
        // be a position silently missing.
        let (_, handler) = log_node(
            5,
            u64::MAX,
            fixture_value(include_str!("fixtures/uniswap-v4/logs.incoming.json")),
            fixture_value(include_str!("fixtures/uniswap-v4/logs.outgoing.json")),
        );
        let (url, _) = stub::serve(handler).await;
        assert!(adapter_for_tests().discover(request(&url)).await.is_err());
    }

    #[tokio::test]
    async fn a_log_or_a_balance_of_the_wrong_shape_is_an_error_and_not_a_panic() {
        let (url, _) = node(Box::new(|_, _| Some("0x".to_string()))).await;
        assert!(adapter_for_tests().discover(request(&url)).await.is_err());
        let (url, _) = node(Box::new(|_, _| Some(format!("0x{}", "f".repeat(64))))).await;
        assert!(
            adapter_for_tests().discover(request(&url)).await.is_err(),
            "an absurd balance is refused, not allocated"
        );
        let (_, handler) = log_node(
            1,
            u64::MAX,
            json!([{"blockNumber": "0x5dc0", "logIndex": "0x0", "topics": ["0x01"]}]),
            json!([]),
        );
        let (url, _) = stub::serve(handler).await;
        assert!(adapter_for_tests().discover(request(&url)).await.is_err());
    }

    // --- a scan that remembers and may stop short ---------------------------

    fn wallet_logs() -> (Value, Value) {
        (
            fixture_value(include_str!("fixtures/uniswap-v4/logs.incoming.json")),
            fixture_value(include_str!("fixtures/uniswap-v4/logs.outgoing.json")),
        )
    }

    fn ids(found: &Discovery) -> Vec<&str> {
        found.keys.iter().map(|key| key.id.as_str()).collect()
    }

    #[tokio::test]
    async fn a_second_pass_reads_only_the_blocks_that_are_new() {
        let latest = Arc::new(AtomicU64::new(25_000));
        let (incoming, outgoing) = wallet_logs();
        let (spans, handler) = log_node_at(Arc::clone(&latest), 2, u64::MAX, incoming, outgoing);
        let (url, _) = stub::serve(handler).await;
        let adapter = adapter_for_tests();

        let first = adapter
            .discover_scan(request(&url))
            .await
            .expect("a first pass");
        assert!(first.complete);
        assert_eq!(ids(&first), vec!["11", "22"]);
        let memory = first.memory.clone().expect("the scan left a memory");
        assert_eq!(
            spans.lock().expect("log").len(),
            6,
            "three windows, two filters"
        );
        spans.lock().expect("log").clear();

        latest.store(25_500, Ordering::SeqCst);
        let second = adapter
            .discover_scan(request_with(&url, Some(&memory)))
            .await
            .expect("a second pass");
        assert!(second.complete);
        assert_eq!(ids(&second), vec!["11", "22"], "what was found is not lost");
        let asked = spans.lock().expect("log").clone();
        assert_eq!(asked.len(), 2, "one window, two filters: {asked:?}");
        assert!(
            asked
                .iter()
                .all(|(from, to)| *from > 24_000 && *to == 25_500),
            "only the newest blocks: {asked:?}"
        );
    }

    #[tokio::test]
    async fn a_position_older_than_the_window_is_a_partial_answer_and_not_an_error() {
        // balanceOf says 2; one window per pass reaches only token 11.
        let (incoming, outgoing) = wallet_logs();
        let (_, handler) = log_node(2, u64::MAX, incoming, outgoing);
        let (url, _) = stub::serve(handler).await;
        let adapter = adapter_with_windows(1);

        let found = adapter
            .discover_scan(request(&url))
            .await
            .expect("a partial answer is an answer");
        assert!(!found.complete, "the history was not all seen");
        assert_eq!(ids(&found), vec!["11"], "what was seen is still returned");
        assert!(found.memory.is_some(), "and the scan remembers where it is");
        // The plain `discover` has no way to say "partial": it must not
        // return a short list as if it were whole (ADR-02).
        assert!(adapter.discover(request(&url)).await.is_err());
    }

    #[tokio::test]
    async fn the_next_pass_goes_on_below_where_the_last_one_stopped() {
        let (incoming, outgoing) = wallet_logs();
        let (spans, handler) = log_node(2, u64::MAX, incoming, outgoing);
        let (url, _) = stub::serve(handler).await;
        let adapter = adapter_with_windows(2);

        // 25 000-15 001 and 15 000-5 001: token 22 (block 3000) is below.
        let first = adapter
            .discover_scan(request(&url))
            .await
            .expect("a first pass");
        assert!(!first.complete);
        assert_eq!(ids(&first), vec!["11"]);
        let memory = first.memory.expect("a memory");
        spans.lock().expect("log").clear();

        let second = adapter
            .discover_scan(request_with(&url, Some(&memory)))
            .await
            .expect("a second pass");
        assert!(second.complete, "the history now reaches the start");
        assert_eq!(ids(&second), vec!["11", "22"], "token 11 was not forgotten");
        let asked = spans.lock().expect("log").clone();
        assert!(
            asked
                .iter()
                .all(|(from, to)| *to <= 5_000 || *from > 24_000),
            "nothing between 5 001 and the new blocks is read again: {asked:?}"
        );
    }

    #[tokio::test]
    async fn a_memory_that_cannot_be_used_is_ignored() {
        let (incoming, outgoing) = wallet_logs();
        let (_, handler) = log_node(2, u64::MAX, incoming, outgoing);
        let (url, _) = stub::serve(handler).await;
        for memory in ["not json", "{\"v\":99}", ""] {
            let found = adapter_for_tests()
                .discover_scan(request_with(&url, Some(memory)))
                .await
                .expect("a full scan instead");
            assert!(found.complete);
            assert_eq!(ids(&found), vec!["11", "22"], "memory {memory:?}");
        }
    }

    #[tokio::test]
    async fn a_memory_that_disagrees_with_balance_of_is_dropped_and_the_scan_redone() {
        // The first pass saw tokens 11 and 22. Later the node says the
        // wallet holds one (22 was a reorged log): the memory is wrong, and
        // a fresh scan is the answer, not an error that would repeat.
        let latest = Arc::new(AtomicU64::new(25_000));
        let (incoming, outgoing) = wallet_logs();
        let (_, handler) = log_node_at(Arc::clone(&latest), 2, u64::MAX, incoming, outgoing);
        let (url, _) = stub::serve(handler).await;
        let memory = adapter_for_tests()
            .discover_scan(request(&url))
            .await
            .expect("a first pass")
            .memory
            .expect("a memory");

        let (incoming, outgoing) = wallet_logs();
        let incoming = Value::Array(
            incoming
                .as_array()
                .expect("logs")
                .iter()
                .filter(|log| log["blockNumber"] != "0xbb8")
                .cloned()
                .collect(),
        );
        let (_, handler) = log_node_at(latest, 1, u64::MAX, incoming, outgoing);
        let (url, _) = stub::serve(handler).await;
        let found = adapter_for_tests()
            .discover_scan(request_with(&url, Some(&memory)))
            .await
            .expect("a fresh scan");
        assert!(found.complete);
        assert_eq!(ids(&found), vec!["11"]);
    }

    #[tokio::test]
    async fn events_are_empty_until_the_history_can_be_read() {
        let (url, _) = node(Box::new(|_, _| None)).await;
        let events = adapter_for_tests()
            .events(request(&url), &key())
            .await
            .expect("events");
        assert!(events.is_empty());
    }
}
