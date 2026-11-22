//! Uniswap v3 concentrated liquidity (spec D1).
//!
//! A position is an NFT of the network's position manager. This adapter
//! finds an address's NFTs through the manager's `balanceOf` and
//! `tokenOfOwnerByIndex`, and reads one NFT's principal and fees from the
//! manager and the pool it lives in. Every address, selector and return
//! layout below is from `D-protocol-facts.md` (the Uniswap v3 section); the
//! source of each is named next to it.
//!
//! # What a read costs
//!
//! * `NFPM.positions(tokenId)` — the per-NFT state (one call);
//! * `Factory.getPool(token0, token1, fee)` — where the NFT's pool is;
//! * one Multicall3 batch about that pool: `slot0`, both global fee growths
//!   and `ticks` of both range ends, plus the tokens' decimals.
//!
//! # Fees
//!
//! `uncollected = tokensOwed + liquidity * (feeGrowthInside_now -
//! feeGrowthInside_last) / 2^128`, where `feeGrowthInside_now` is derived
//! from the pool's global growth and the two ticks' outside growth by the
//! branches of `Tick.getFeeGrowthInside`. Every subtraction wraps modulo
//! 2^256 ([`U256::wrapping_sub`]) — the accumulators are *defined* to
//! overflow — and the product goes through [`U256::mul_div_floor`], whose
//! 512-bit middle loses nothing. The `pool.positions(key)` getter is never
//! used: it is the aggregate of every NFT sharing the range, not this one.
//!
//! `tokensOwed` is "claimable", not "fees earned": after a
//! `decreaseLiquidity` it also holds the withdrawn principal. A position
//! drained but not collected therefore stays open and shows its owed
//! amounts as fees to collect.
//!
//! # What this file does not know (gaps, see the report of D1)
//!
//! * **ERC-20 `decimals()`** — closed on 2026-11-16:
//!   [`DECIMALS_SELECTOR`] holds the selector the main session recomputed
//!   itself, so a read no longer refuses for the want of it.
//! * **`events`**: the topic0 of `IncreaseLiquidity`, `DecreaseLiquidity`
//!   and `Collect` are not in the facts document, and block timestamps and
//!   transaction receipts (gas) are not something [`Node`] reads. `events`
//!   answers an honest empty list rather than an invented path.

use crate::ports::ChainSource;
use crate::sources::evm::abi;
use mwt_base::error::{Error, Result};
use mwt_model::Chain;
use mwt_model::{CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, TokenId};

use super::tick_math::principal;
use super::{
    AdapterFuture, Answer, AssetAmount, FeeReading, LegToken, Node, PositionKey, PositionKind,
    PositionReading, PositionStatus, ProtocolAdapter, ProtocolRequest, U256,
};

/// The protocol id a position of this protocol stores
/// ([`mwt_model::CryptoPosition::protocol`]).
pub const PROTOCOL: &str = "uniswap-v3";

/// One network's deployment (facts, "NonfungiblePositionManager and
/// UniswapV3Factory, per network": the official Uniswap deployment pages).
/// Hard-coded per network: the pages warn against assuming the mainnet
/// addresses elsewhere, and BNB and Base really differ.
struct Deployment {
    chain: Chain,
    manager: &'static str,
    factory: &'static str,
}

const DEPLOYMENTS: [Deployment; 6] = [
    Deployment {
        chain: Chain::Ethereum,
        manager: "0xc36442b4a4522e871399cd717abdd847ab11fe88",
        factory: "0x1f98431c8ad98523631ae4a59f267346ea31f984",
    },
    Deployment {
        chain: Chain::Optimism,
        manager: "0xc36442b4a4522e871399cd717abdd847ab11fe88",
        factory: "0x1f98431c8ad98523631ae4a59f267346ea31f984",
    },
    Deployment {
        chain: Chain::Bsc,
        manager: "0x7b8a01b39d58278b5de7e48c8449c9f4f5170613",
        factory: "0xdb1d10011ad0ff90774d0c6bb92e5c5c8b4461f7",
    },
    Deployment {
        chain: Chain::Polygon,
        manager: "0xc36442b4a4522e871399cd717abdd847ab11fe88",
        factory: "0x1f98431c8ad98523631ae4a59f267346ea31f984",
    },
    Deployment {
        chain: Chain::Arbitrum,
        manager: "0xc36442b4a4522e871399cd717abdd847ab11fe88",
        factory: "0x1f98431c8ad98523631ae4a59f267346ea31f984",
    },
    Deployment {
        chain: Chain::Base,
        manager: "0x03a520b32c04bf3beef7beb72e919cf822ed34f1",
        factory: "0x33128a8fc17869897dce68ed026d694621f6fdfd",
    },
];

fn deployment(chain: Chain) -> Option<&'static Deployment> {
    DEPLOYMENTS.iter().find(|row| row.chain == chain)
}

// Selectors, each with the full signature (facts, "Selectors and signatures",
// every one computed and cross-checked against 4byte.directory there).

/// `balanceOf(address)` — ERC-721.
const SEL_BALANCE_OF: &str = "70a08231";
/// `tokenOfOwnerByIndex(address,uint256)` — ERC-721Enumerable.
const SEL_TOKEN_OF_OWNER_BY_INDEX: &str = "2f745c59";
/// `positions(uint256)` on the position manager.
const SEL_POSITIONS: &str = "99fbab88";
/// `getPool(address,address,uint24)` on the factory.
const SEL_GET_POOL: &str = "1698ee82";
/// `slot0()` on the pool.
const SEL_SLOT0: &str = "3850c7bd";
/// `feeGrowthGlobal0X128()` on the pool.
const SEL_FEE_GROWTH_GLOBAL0: &str = "f3058399";
/// `feeGrowthGlobal1X128()` on the pool.
const SEL_FEE_GROWTH_GLOBAL1: &str = "46141319";
/// `ticks(int24)` on the pool.
const SEL_TICKS: &str = "f30dba93";

/// ERC-20 `decimals()`, verified by the main session on 2026-11-16 with its
/// own Keccak-256 (self-tested against the published hash of the empty
/// string): `keccak256("decimals()")[0..4] = 313ce567`. The same run
/// reproduced every selector of the facts document, so this one stands on the
/// same evidence as the rest.
const DECIMALS_SELECTOR: Option<&str> = Some("313ce567");

/// ERC-20 `symbol()`: `keccak256("symbol()")[0..4] = 95d89b41`, recomputed
/// the same way as [`DECIMALS_SELECTOR`] (the v4 adapter's test hashes the
/// signature again). Shared with the v4 adapter: one home for the rule.
pub(super) const SYMBOL_SELECTOR: &str = "95d89b41";

/// The ticker a token answered to `symbol()`, if it answered a string.
///
/// A node's answer is untrusted: a refusal, a `bytes32` symbol (old tokens)
/// or any other shape is `None` and never an error, so one odd token does
/// not fail the reading. Non-UTF-8 bytes are replaced; clipping to a
/// storable size happens at registration (`service::cryptotrack::tokens`).
pub(super) fn symbol_of(answer: Option<&Answer>) -> Option<String> {
    let answer = answer?;
    let offset = usize::try_from(answer.u128(0).ok()?).ok()?;
    if offset % 32 != 0 {
        return None;
    }
    let start = offset / 32;
    let length = usize::try_from(answer.u128(start).ok()?).ok()?;
    if length > answer.len().saturating_mul(32) {
        return None;
    }
    let mut bytes = Vec::with_capacity(length);
    for chunk in 0..length.div_ceil(32) {
        bytes.extend_from_slice(
            answer
                .word(start.checked_add(1)?.checked_add(chunk)?)
                .ok()?,
        );
    }
    bytes.truncate(length);
    let symbol = String::from_utf8_lossy(&bytes).into_owned();
    (!symbol.is_empty()).then_some(symbol)
}

/// Words of `NFPM.positions(tokenId)` (facts, return layout, offsets / 32).
mod stored_word {
    pub(crate) const TOKEN0: usize = 2;
    pub(crate) const TOKEN1: usize = 3;
    pub(crate) const FEE: usize = 4;
    pub(crate) const TICK_LOWER: usize = 5;
    pub(crate) const TICK_UPPER: usize = 6;
    pub(crate) const LIQUIDITY: usize = 7;
    pub(crate) const GROWTH0_LAST: usize = 8;
    pub(crate) const GROWTH1_LAST: usize = 9;
    pub(crate) const OWED0: usize = 10;
    pub(crate) const OWED1: usize = 11;
    pub(crate) const COUNT: usize = 12;
}

/// The ticks the protocol allows (`TickMath.MIN_TICK`/`MAX_TICK`): a word
/// outside is not a tick and the answer is refused.
const MAX_TICK: i64 = 887_272;

/// More NFTs than this on one address is not a wallet, it is a bad answer.
const MAX_POSITIONS: usize = 10_000;

/// The Uniswap v3 adapter. Holds the node client of the pass and builds a
/// [`Node`] per request.
pub struct UniswapV3<C> {
    client: C,
    decimals_selector: Option<&'static str>,
}

/// The adapter for [`super::Registry::builtin`]:
/// `uniswap_v3::adapter(node.clone())`.
pub fn adapter<C: ChainSource + 'static>(node: C) -> Box<dyn ProtocolAdapter> {
    Box::new(UniswapV3 {
        client: node,
        decimals_selector: DECIMALS_SELECTOR,
    })
}

/// The position NFT of this protocol on `chain`: what a wallet shows for a
/// range, and so what must stay out of the capital once the position counts
/// (see [`super::receipt_tokens`]). Empty where the protocol is not
/// deployed.
pub fn receipt_tokens(chain: Chain) -> Vec<CryptoAsset> {
    deployment(chain)
        .map(|row| vec![CryptoAsset::Token(TokenId::new(chain, row.manager))])
        .unwrap_or_default()
}

impl<C: ChainSource + 'static> ProtocolAdapter for UniswapV3<C> {
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

    fn read_position<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
        key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Option<PositionReading>> {
        Box::pin(async move { self.read(request, key).await })
    }

    /// Always empty for now. What a position's history needs is not in the
    /// facts document or not readable through [`Node`]: the topic0 of
    /// `IncreaseLiquidity`/`DecreaseLiquidity`/`Collect` (the amounts live in
    /// those events), the blocks' timestamps, and the receipts for gas. The
    /// ERC-721 `Transfer` topic0 is documented, but a mint's transfer says
    /// only that the token appeared, not what was put in.
    fn events<'fut>(
        &'fut self,
        _request: ProtocolRequest<'fut>,
        _key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Vec<CryptoTx>> {
        Box::pin(async move { Ok(Vec::new()) })
    }
}

impl<C: ChainSource> UniswapV3<C> {
    /// Every NFT the address holds in the manager.
    ///
    /// **A position with no liquidity and nothing owed is still returned.**
    /// It may be a range the user closed — the module keeps closed positions
    /// in the result of the period they were open in — and discovery is the
    /// cheap step: it cannot tell closed from open without the expensive
    /// read, and a burned NFT is not enumerated at all, so what is listed
    /// here exists. [`Self::read`] decides the status.
    async fn discover_keys(&self, request: ProtocolRequest<'_>) -> Result<Vec<PositionKey>> {
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
            return Ok(Vec::new());
        }
        let mut calls = Vec::with_capacity(count);
        for index in 0..count {
            let index = U256::from_u128(index as u128).to_be_bytes();
            calls.push((
                deployment.manager.to_string(),
                abi::calldata(SEL_TOKEN_OF_OWNER_BY_INDEX, &[owner, index])?,
            ));
        }
        let mut keys = Vec::with_capacity(count);
        // The balance said the index exists; a refusal now means the NFT
        // moved between the two calls — it is simply not listed.
        for answer in node.call_many(&calls).await?.into_iter().flatten() {
            keys.push(PositionKey::new(
                PROTOCOL,
                request.chain,
                &answer.u256(0)?.to_decimal(),
            ));
        }
        Ok(keys)
    }

    async fn read(
        &self,
        request: ProtocolRequest<'_>,
        key: &PositionKey,
    ) -> Result<Option<PositionReading>> {
        let deployment = deployment(request.chain).ok_or(Error::Unsupported)?;
        let decimals_selector = self.decimals_selector.ok_or(Error::Unsupported)?;
        let node = Node::new(&self.client, &request)?;
        let token_id = U256::from_decimal(&key.id)
            .map_err(|_| Error::InvalidInput("a position id is a token id".into()))?;

        // The manager refuses `positions` for a token that was burned: that
        // is a position that is gone, not an error.
        let stored = node
            .call_many(&[(
                deployment.manager.to_string(),
                abi::calldata(SEL_POSITIONS, &[token_id.to_be_bytes()])?,
            )])
            .await?
            .into_iter()
            .next()
            .flatten();
        let Some(stored) = stored else {
            return Ok(None);
        };
        let stored = StoredPosition::parse(&stored)?;

        let pool = node
            .call(
                deployment.factory,
                SEL_GET_POOL,
                &[
                    abi::address_word(&stored.token0)?,
                    abi::address_word(&stored.token1)?,
                    U256::from_u128(u128::from(stored.fee)).to_be_bytes(),
                ],
            )
            .await?
            .address(0)?;
        if pool == ZERO_ADDRESS {
            return Err(Error::Network(
                "the factory knows no pool for the NFT".into(),
            ));
        }

        let state = self
            .read_pool(&node, &pool, &stored, decimals_selector)
            .await?;
        Ok(Some(self.assemble(request, key, &stored, &state)?))
    }

    /// The pool's side of the reading and the tokens' decimals, in one
    /// Multicall3 batch: all of it must be answered or nothing is.
    async fn read_pool(
        &self,
        node: &Node<'_, C>,
        pool: &str,
        stored: &StoredPosition,
        decimals_selector: &str,
    ) -> Result<PoolState> {
        let at_pool = |selector: &str, args: &[[u8; 32]]| -> Result<(String, String)> {
            Ok((pool.to_string(), abi::calldata(selector, args)?))
        };
        let at_token = |token: &str| -> Result<(String, String)> {
            Ok((token.to_string(), abi::calldata(decimals_selector, &[])?))
        };
        let at_symbol = |token: &str| -> Result<(String, String)> {
            Ok((token.to_string(), abi::calldata(SYMBOL_SELECTOR, &[])?))
        };
        let calls = [
            at_pool(SEL_SLOT0, &[])?,
            at_pool(SEL_FEE_GROWTH_GLOBAL0, &[])?,
            at_pool(SEL_FEE_GROWTH_GLOBAL1, &[])?,
            at_pool(SEL_TICKS, &[abi::int_word(stored.tick_lower)])?,
            at_pool(SEL_TICKS, &[abi::int_word(stored.tick_upper)])?,
            at_token(&stored.token0)?,
            at_token(&stored.token1)?,
            at_symbol(&stored.token0)?,
            at_symbol(&stored.token1)?,
        ];
        let answers = node.call_many(&calls).await?;
        let mut answers = answers.into_iter();
        let mut next = |what: &str| {
            answers
                .next()
                .flatten()
                .ok_or_else(|| Error::Network(format!("the pool refused {what}")))
        };
        let slot0 = next("slot0")?;
        let global0 = next("feeGrowthGlobal0X128")?;
        let global1 = next("feeGrowthGlobal1X128")?;
        let lower = next("ticks(lower)")?;
        let upper = next("ticks(upper)")?;
        let decimals0 = next("decimals() of token0")?;
        let decimals1 = next("decimals() of token1")?;
        // A symbol is a courtesy: a refusal is no reason to fail the read.
        let symbols = [
            symbol_of(answers.next().flatten().as_ref()),
            symbol_of(answers.next().flatten().as_ref()),
        ];
        Ok(PoolState {
            sqrt_price: slot0.u256(0)?,
            tick: checked_tick(slot0.int(1)?)?,
            global: [global0.u256(0)?, global1.u256(0)?],
            // `ticks`: word 2 and 3 are `feeGrowthOutside0/1X128`.
            lower_outside: [lower.u256(2)?, lower.u256(3)?],
            upper_outside: [upper.u256(2)?, upper.u256(3)?],
            decimals: [decimals_of(&decimals0)?, decimals_of(&decimals1)?],
            symbols,
        })
    }

    fn assemble(
        &self,
        request: ProtocolRequest<'_>,
        key: &PositionKey,
        stored: &StoredPosition,
        state: &PoolState,
    ) -> Result<PositionReading> {
        let chain = request.chain;
        let assets = [
            CryptoAsset::Token(TokenId::new(chain, &stored.token0)),
            CryptoAsset::Token(TokenId::new(chain, &stored.token1)),
        ];

        // Fees: tokensOwed plus what the range earned since the NFT last
        // touched the pool.
        let mut uncollected = Vec::with_capacity(2);
        let owed = [stored.owed0, stored.owed1];
        for side in 0..2 {
            let inside = fee_growth_inside(
                state.tick,
                stored.tick_lower,
                stored.tick_upper,
                state.global[side],
                state.lower_outside[side],
                state.upper_outside[side],
            );
            let raw = uncollected_fees(
                owed[side],
                stored.liquidity,
                inside,
                stored.growth_last[side],
            )?;
            uncollected.push(AssetAmount {
                asset: assets[side].clone(),
                amount: raw.to_amount(state.decimals[side])?,
            });
        }

        // Principal: what the liquidity is worth in the two tokens at the
        // pool's current price.
        let (raw0, raw1) = principal(
            state.sqrt_price,
            stored.tick_lower,
            stored.tick_upper,
            stored.liquidity,
        )?;
        let raw = [raw0, raw1];
        let mut legs = Vec::with_capacity(2);
        for side in 0..2 {
            legs.push(CryptoLeg {
                direction: CryptoDirection::In,
                asset: assets[side].clone(),
                amount: raw[side].to_amount(state.decimals[side])?,
                counterparty: None,
            });
        }

        // Closed means nothing is left to take out: no liquidity and
        // nothing owed. A range drained but not yet collected still holds
        // the owed amounts (principal included), so it stays open.
        let closed = stored.liquidity == 0 && stored.owed0 == 0 && stored.owed1 == 0;
        Ok(PositionReading {
            key: key.clone(),
            account_id: request.account_id.to_string(),
            name: format!("Uniswap v3 #{}", key.id),
            kind: PositionKind::Pool,
            status: if closed {
                PositionStatus::Closed
            } else {
                PositionStatus::Open
            },
            legs,
            // `collected` needs the history (`events`), which is not read.
            fees: FeeReading {
                collected: None,
                uncollected: Some(uncollected),
            },
            receipts: receipt_tokens(chain),
            // Everything is read from the chain exactly; the tick math is
            // accurate far below a unit of any token (see `sqrt_ratio_at_tick`).
            approximate: false,
            opened_at: None,
            closed_at: None,
            tokens: (0..2)
                .map(|side| LegToken {
                    token: TokenId::new(chain, [&stored.token0, &stored.token1][side]),
                    symbol: state.symbols[side].clone(),
                    name: None,
                    decimals: Some(state.decimals[side]),
                })
                .collect(),
            read_at: mwt_model::now_iso(),
        })
    }
}

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// What `NFPM.positions(tokenId)` answered, narrowed to what is used.
#[derive(Debug, Clone, PartialEq)]
struct StoredPosition {
    token0: String,
    token1: String,
    fee: u32,
    tick_lower: i64,
    tick_upper: i64,
    liquidity: u128,
    growth_last: [U256; 2],
    owed0: u128,
    owed1: u128,
}

impl StoredPosition {
    fn parse(answer: &super::Answer) -> Result<Self> {
        if answer.len() != stored_word::COUNT {
            return Err(Error::Network(
                "positions() did not answer twelve words".into(),
            ));
        }
        let fee = u32::try_from(answer.u128(stored_word::FEE)?)
            .ok()
            .filter(|fee| *fee <= 0xff_ffff)
            .ok_or_else(|| Error::Network("a fee tier is a uint24".into()))?;
        let tick_lower = checked_tick(answer.int(stored_word::TICK_LOWER)?)?;
        let tick_upper = checked_tick(answer.int(stored_word::TICK_UPPER)?)?;
        if tick_lower >= tick_upper {
            return Err(Error::Network("a range has lower below upper".into()));
        }
        Ok(Self {
            token0: answer.address(stored_word::TOKEN0)?,
            token1: answer.address(stored_word::TOKEN1)?,
            fee,
            tick_lower,
            tick_upper,
            liquidity: answer.u128(stored_word::LIQUIDITY)?,
            growth_last: [
                answer.u256(stored_word::GROWTH0_LAST)?,
                answer.u256(stored_word::GROWTH1_LAST)?,
            ],
            owed0: answer.u128(stored_word::OWED0)?,
            owed1: answer.u128(stored_word::OWED1)?,
        })
    }
}

/// What the pool and the tokens answered.
#[derive(Debug, Clone, PartialEq)]
struct PoolState {
    sqrt_price: U256,
    tick: i64,
    global: [U256; 2],
    lower_outside: [U256; 2],
    upper_outside: [U256; 2],
    decimals: [u8; 2],
    symbols: [Option<String>; 2],
}

fn checked_tick(tick: i64) -> Result<i64> {
    if tick.abs() > MAX_TICK {
        return Err(Error::Network("a tick outside the protocol's range".into()));
    }
    Ok(tick)
}

fn decimals_of(answer: &super::Answer) -> Result<u8> {
    u8::try_from(answer.u128(0)?).map_err(|_| Error::Network("decimals() is a uint8".into()))
}

/// `Tick.getFeeGrowthInside` for one token: the fee growth accumulated
/// inside `[lower, upper)` per unit of liquidity, from the pool's global
/// growth and the growth "outside" each end. Branches verbatim from the
/// facts document, every subtraction wrapping modulo 2^256.
fn fee_growth_inside(
    current: i64,
    lower: i64,
    upper: i64,
    global: U256,
    lower_outside: U256,
    upper_outside: U256,
) -> U256 {
    let below = if current >= lower {
        lower_outside
    } else {
        global.wrapping_sub(lower_outside)
    };
    let above = if current < upper {
        upper_outside
    } else {
        global.wrapping_sub(upper_outside)
    };
    global.wrapping_sub(below).wrapping_sub(above)
}

/// `Position.update`: `tokensOwed + liquidity * (inside - last) / 2^128`,
/// the difference wrapping modulo 2^256 and the product on 512 bits. The
/// protocol truncates the product to `uint128`; a display wants the
/// untruncated value, so it is not.
fn uncollected_fees(owed: u128, liquidity: u128, inside: U256, last: U256) -> Result<U256> {
    let q128 = U256::power_of_two(128).ok_or_else(overflow)?;
    let earned = inside
        .wrapping_sub(last)
        .mul_div_floor(U256::from_u128(liquidity), q128)
        .ok_or_else(overflow)?;
    U256::from_u128(owed)
        .checked_add(earned)
        .ok_or_else(overflow)
}

fn overflow() -> Error {
    Error::Network("a fee does not fit a word".into())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::node::stub;
    use super::*;

    const OWNER: &str = "0x1111111111111111111111111111111111111111";
    const POOL: &str = "0x000000000000000000000000000000000000b001";
    const TOKEN0: &str = "0x00000000000000000000000000000000000a0001";
    const TOKEN1: &str = "0x00000000000000000000000000000000000a0002";
    const MANAGER: &str = "0xc36442b4a4522e871399cd717abdd847ab11fe88";
    const FACTORY: &str = "0x1f98431c8ad98523631ae4a59f267346ea31f984";
    /// The selector the tests supply for ERC-20 `decimals()`; the shipped
    /// adapter has none (see [`DECIMALS_SELECTOR`]).
    const TEST_DECIMALS: &str = "313ce567";

    // Fixtures: every file is `{"_source": …, "result": "0x…"}`. None is a
    // live node answer: they are hand-built from the layouts of the facts
    // document (`positions`, `slot0`, `ticks`), with synthetic token and pool
    // addresses, because this layer had no network. The expected numbers in
    // the tests below were computed outside Rust with arbitrary-precision
    // integers from the same words.
    macro_rules! fixture {
        ($name:literal) => {
            fixture_result(include_str!(concat!(
                "fixtures/uniswap-v3/",
                $name,
                ".json"
            )))
        };
    }

    fn fixture_result(text: &str) -> String {
        let value: Value = serde_json::from_str(text).expect("a fixture is JSON");
        value["result"].as_str().expect("a result").to_string()
    }

    fn words_of(hex: &str) -> Vec<String> {
        hex.as_bytes()[2..]
            .chunks(64)
            .map(|chunk| String::from_utf8_lossy(chunk).to_string())
            .collect()
    }

    fn hex_of(words: &[String]) -> String {
        format!("0x{}", words.concat())
    }

    fn word(value: u128) -> String {
        format!("{value:064x}")
    }

    /// What the stub node answers for a call: `Some(return data)` or `None`
    /// for a revert.
    type Handler = Box<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
            .collect()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

    /// A node that answers calls through `handler`, directly or inside
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

    /// The nine answers of the open scenario, keyed by selector.
    struct Scenario {
        positions: String,
        slot0: String,
        global0: String,
        global1: String,
        lower: String,
        upper: String,
        decimals0: String,
        decimals1: String,
        /// `None`: the token refuses `symbol()`.
        symbol0: Option<String>,
        symbol1: Option<String>,
    }

    impl Scenario {
        fn open() -> Self {
            Self {
                positions: fixture!("positions.open"),
                slot0: fixture!("slot0"),
                global0: fixture!("feeGrowthGlobal0X128"),
                global1: fixture!("feeGrowthGlobal1X128"),
                lower: fixture!("ticks.lower"),
                upper: fixture!("ticks.upper"),
                decimals0: fixture!("decimals.token0"),
                decimals1: fixture!("decimals.token1"),
                symbol0: Some(fixture!("symbol.token0")),
                symbol1: Some(fixture!("symbol.token1")),
            }
        }

        /// A token that refuses `symbol()` (token0) and one that answers
        /// something that is not a string (token1: a bare word).
        fn with_odd_symbols(mut self) -> Self {
            self.symbol0 = None;
            self.symbol1 = Some(format!("0x{}", word(7)));
            self
        }

        fn with_symbols(mut self, symbol0: &str, symbol1: &str) -> Self {
            self.symbol0 = Some(symbol0.to_string());
            self.symbol1 = Some(symbol1.to_string());
            self
        }

        /// A token that answers nothing to `decimals()`: the scale of the
        /// money is then unknown, and no default stands in for it.
        fn without_decimals(mut self) -> Self {
            self.decimals0 = String::new();
            self
        }

        fn handler(self) -> Handler {
            Box::new(move |target, calldata| {
                let selector = &calldata[..8];
                match (target, selector) {
                    (MANAGER, SEL_POSITIONS) => Some(self.positions.clone()),
                    (FACTORY, SEL_GET_POOL) => Some(fixture!("getPool")),
                    (POOL, SEL_SLOT0) => Some(self.slot0.clone()),
                    (POOL, SEL_FEE_GROWTH_GLOBAL0) => Some(self.global0.clone()),
                    (POOL, SEL_FEE_GROWTH_GLOBAL1) => Some(self.global1.clone()),
                    (POOL, SEL_TICKS) => {
                        // The tick is the argument, a negative int24 in
                        // two's complement: the lower end is -201000.
                        let argument = &calldata[8..];
                        if argument == hex(&abi::int_word(-201_000)) {
                            Some(self.lower.clone())
                        } else if argument == hex(&abi::int_word(-200_000)) {
                            Some(self.upper.clone())
                        } else {
                            None
                        }
                    }
                    (TOKEN0, TEST_DECIMALS) => {
                        (!self.decimals0.is_empty()).then(|| self.decimals0.clone())
                    }
                    (TOKEN1, TEST_DECIMALS) => Some(self.decimals1.clone()),
                    (TOKEN0, SYMBOL_SELECTOR) => self.symbol0.clone(),
                    (TOKEN1, SYMBOL_SELECTOR) => self.symbol1.clone(),
                    _ => None,
                }
            })
        }
    }

    fn adapter_for_tests() -> UniswapV3<stub::Loopback> {
        UniswapV3 {
            client: stub::client(),
            decimals_selector: Some(TEST_DECIMALS),
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
        PositionKey::new(PROTOCOL, Chain::Ethereum, "123456")
    }

    fn raw(decimal: &str) -> U256 {
        U256::from_decimal(decimal).expect("a decimal")
    }

    // --- the fee arithmetic --------------------------------------------

    #[test]
    fn fee_growth_inside_follows_the_branches_of_the_protocol() {
        let global = raw("1000");
        let (lower_out, upper_out) = (raw("300"), raw("200"));
        // Inside the range: below is the lower tick's outside, above the
        // upper tick's.
        assert_eq!(
            fee_growth_inside(5, 0, 10, global, lower_out, upper_out),
            raw("500")
        );
        // Below the range (current < lower): below = global - lower.outside;
        // above = upper.outside.
        assert_eq!(
            fee_growth_inside(-5, 0, 10, global, lower_out, upper_out),
            raw("1000")
                .wrapping_sub(raw("700"))
                .wrapping_sub(raw("200"))
        );
        // At the lower tick the position is inside (>=); at the upper it is
        // outside (strict <): the two boundaries are not symmetric.
        assert_eq!(
            fee_growth_inside(0, 0, 10, global, lower_out, upper_out),
            raw("500")
        );
        assert_eq!(
            fee_growth_inside(10, 0, 10, global, lower_out, upper_out),
            raw("1000")
                .wrapping_sub(raw("300"))
                .wrapping_sub(raw("800"))
        );
    }

    #[test]
    fn a_fee_growth_that_wrapped_past_two_to_the_256_still_gives_the_right_delta() {
        // The accumulator was 100 below the top of the word when the NFT last
        // touched the pool and has since wrapped to 50: the real growth is
        // 150, and a checked or saturating subtraction would say 0 or fail.
        let last = U256::MAX.wrapping_sub(raw("99"));
        let inside = raw("50");
        // liquidity 2^127 makes the answer 150 / 2.
        let earned = uncollected_fees(0, 1 << 127, inside, last).expect("a fee");
        assert_eq!(earned, raw("75"));
        // Owed fees come on top.
        let with_owed = uncollected_fees(1_000, 1 << 127, inside, last).expect("a fee");
        assert_eq!(with_owed, raw("1075"));
    }

    #[test]
    fn the_inside_growth_itself_wraps_when_the_outside_values_exceed_the_global_one() {
        // Outside values are allowed to be "ahead" of the global one after a
        // wrap: below = global - lower.outside wraps to a huge number and
        // the inside difference still comes out small.
        let global = raw("10");
        let lower_out = raw("30");
        let upper_out = raw("5");
        // current < lower: below = 10 - 30 (wrapped), above = 5.
        let inside = fee_growth_inside(-1, 0, 10, global, lower_out, upper_out);
        // global - below - above = 10 - (10 - 30) - 5 = 25.
        assert_eq!(inside, raw("25"));
    }

    #[test]
    fn the_product_is_taken_on_512_bits_and_not_before_the_division() {
        // delta = 2^255, liquidity = 2^128 - 1: the product is 383 bits,
        // the quotient (2^255 - 2^127) fits a word, and any 256-bit
        // intermediate would have lost it.
        let delta = U256::power_of_two(255).expect("a power");
        let earned = uncollected_fees(0, u128::MAX, delta, U256::ZERO).expect("a fee");
        // 2^255 * (2^128 - 1) / 2^128 = 2^255 - 2^127
        let expected = U256::power_of_two(255)
            .expect("a power")
            .wrapping_sub(U256::power_of_two(127).expect("a power"));
        assert_eq!(earned, expected);
    }

    #[test]
    fn the_largest_possible_fee_still_fits_a_word() {
        // The widest inputs the layout allows: a delta of 2^256 - 1, the
        // largest liquidity and the largest owed amount. The sum stays below
        // 2^256, so the error branches of `uncollected_fees` are guards
        // against a wrong layout, not a reachable overflow.
        assert!(uncollected_fees(u128::MAX, u128::MAX, U256::MAX, U256::ZERO).is_ok());
    }

    // --- the static tables -------------------------------------------------

    #[test]
    fn the_manager_is_the_receipt_on_every_network_the_protocol_lives_on() {
        let expected = [
            (Chain::Ethereum, MANAGER),
            (Chain::Optimism, MANAGER),
            (Chain::Polygon, MANAGER),
            (Chain::Arbitrum, MANAGER),
            (Chain::Bsc, "0x7b8a01b39d58278b5de7e48c8449c9f4f5170613"),
            (Chain::Base, "0x03a520b32c04bf3beef7beb72e919cf822ed34f1"),
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

    // --- discovery -------------------------------------------------------

    #[tokio::test]
    async fn discovery_lists_every_nft_the_address_holds() {
        let handler: Handler = Box::new(|target, calldata| {
            assert_eq!(target, MANAGER);
            match &calldata[..8] {
                SEL_BALANCE_OF => Some(fixture!("balanceOf")),
                SEL_TOKEN_OF_OWNER_BY_INDEX => {
                    if calldata.ends_with(&word(0)) {
                        Some(fixture!("tokenOfOwnerByIndex.0"))
                    } else {
                        Some(fixture!("tokenOfOwnerByIndex.1"))
                    }
                }
                _ => None,
            }
        });
        let (url, _) = node(handler).await;
        let found = adapter_for_tests()
            .discover(request(&url))
            .await
            .expect("a discovery");
        assert_eq!(
            found,
            vec![
                PositionKey::new(PROTOCOL, Chain::Ethereum, "123456"),
                PositionKey::new(PROTOCOL, Chain::Ethereum, "789012"),
            ]
        );
    }

    #[tokio::test]
    async fn an_address_with_no_nft_costs_one_call_and_finds_nothing() {
        let (url, seen) = node(Box::new(|_, _| Some(format!("0x{}", word(0))))).await;
        let found = adapter_for_tests()
            .discover(request(&url))
            .await
            .expect("a discovery");
        assert!(found.is_empty());
        assert_eq!(seen.lock().expect("the log").len(), 1);
    }

    #[tokio::test]
    async fn an_answer_of_the_wrong_shape_is_an_error_and_not_a_panic() {
        let (url, _) = node(Box::new(|_, _| Some("0x".to_string()))).await;
        assert!(adapter_for_tests().discover(request(&url)).await.is_err());
        let (url, _) = node(Box::new(|_, _| Some(format!("0x{}", "f".repeat(64))))).await;
        assert!(
            adapter_for_tests().discover(request(&url)).await.is_err(),
            "an absurd balance is refused, not allocated"
        );
    }

    // --- reading one position ----------------------------------------------

    /// Fees of the open scenario: `fee0 = 250000 + (inside - last) * L /
    /// 2^128` with L = 12345678901234567 and the words of the fixtures,
    /// computed outside Rust.
    #[tokio::test]
    async fn an_open_position_reads_its_fees_and_its_principal() {
        let (url, _) = node(Scenario::open().handler()).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position is there");

        assert_eq!(reading.status, PositionStatus::Open);
        assert_eq!(reading.kind, PositionKind::Pool);
        assert_eq!(reading.name, "Uniswap v3 #123456");
        assert!(!reading.approximate);
        assert_eq!(reading.receipts, receipt_tokens(Chain::Ethereum));

        let token0 = CryptoAsset::Token(TokenId::new(Chain::Ethereum, TOKEN0));
        let token1 = CryptoAsset::Token(TokenId::new(Chain::Ethereum, TOKEN1));
        let uncollected = reading.fees.uncollected.expect("fees were read");
        assert_eq!(uncollected[0].asset, token0);
        assert_eq!(uncollected[0].amount.to_string(), "1.265859");
        assert_eq!(uncollected[1].asset, token1);
        assert_eq!(uncollected[1].amount.to_string(), "0.000371129722963443");
        assert!(
            reading.fees.collected.is_none(),
            "what was collected needs the history, which is not read: unknown, not zero"
        );

        // Principal at the pool's tick -200500 (inside -201000..-200000):
        // 6880193878732039285 raw token0 (18 decimals would be wrong: this
        // token has 6) and 13503043462 raw token1, from an independent
        // computation. The tick math is not bit-identical to the
        // protocol's, so the amounts are compared to 9 significant digits.
        assert_eq!(reading.legs.len(), 2);
        assert_eq!(reading.legs[0].asset, token0);
        assert_eq!(reading.legs[0].direction, CryptoDirection::In);
        assert!(
            reading.legs[0]
                .amount
                .to_string()
                .starts_with("6880193878732.0"),
            "{}",
            reading.legs[0].amount
        );
        assert_eq!(reading.legs[1].asset, token1);
        assert!(
            reading.legs[1]
                .amount
                .to_string()
                .starts_with("0.00000001350304"),
            "{}",
            reading.legs[1].amount
        );
    }

    #[tokio::test]
    async fn a_position_with_nothing_left_is_closed_and_still_read() {
        let mut scenario = Scenario::open();
        scenario.positions = fixture!("positions.closed");
        let (url, _) = node(scenario.handler()).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("a closed position is a position");
        assert_eq!(reading.status, PositionStatus::Closed);
        assert!(super::super::is_closed(&reading.legs));
        let uncollected = reading.fees.uncollected.expect("read");
        assert!(uncollected.iter().all(|fee| fee.amount.is_zero()));
    }

    #[tokio::test]
    async fn a_drained_range_with_amounts_owed_stays_open_and_shows_them() {
        let mut scenario = Scenario::open();
        scenario.positions = fixture!("positions.drained");
        let (url, _) = node(scenario.handler()).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position is there");
        assert_eq!(
            reading.status,
            PositionStatus::Open,
            "there is still something to collect"
        );
        let uncollected = reading.fees.uncollected.expect("read");
        assert_eq!(uncollected[0].amount.to_string(), "5");
        assert_eq!(uncollected[1].amount.to_string(), "0.000000000000000123");
    }

    #[tokio::test]
    async fn the_fee_growth_that_wrapped_is_read_through_the_whole_path() {
        // The position's last inside growth is 2^256 - 10^28 + something
        // from before the pool's accumulators wrapped: patching the stored
        // word to a value just under 2^256 makes the real delta
        // `inside - last` wrap, and the answer must be the same kind of
        // small number a wrap-free read gives, not an error or zero.
        let mut scenario = Scenario::open();
        let mut words = words_of(&scenario.positions);
        // last0 = 2^256 - 5e27: with inside0 = global0 - lower - upper the
        // delta is inside0 + 5e27 (mod 2^256).
        words[8] = hex_word(U256::ZERO.wrapping_sub(raw("5000000000000000000000000000")));
        scenario.positions = hex_of(&words);
        let (url, _) = node(scenario.handler()).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position is there");
        let fee = &reading.fees.uncollected.expect("read")[0];
        // inside0 is ~ 4.6e38, so the delta is ~ 4.6e38 and the fee is
        // 4.6e38 * 1.23e16 / 3.4e38 ~ 1.7e16 raw = 1.7e10 whole tokens.
        assert!(fee.amount.to_string().starts_with("1"), "{}", fee.amount);
        assert!(fee.amount.to_string().len() > 8, "{}", fee.amount);
    }

    fn hex_word(value: U256) -> String {
        hex(&value.to_be_bytes())
    }

    #[tokio::test]
    async fn a_position_the_manager_refuses_is_gone_and_not_an_error() {
        // `positions` reverts with "Invalid token ID" for a burned NFT.
        let handler: Handler = Box::new(|_, _| None);
        let (url, _) = node(handler).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a refusal of the manager is an answer");
        assert!(reading.is_none());
    }

    #[tokio::test]
    async fn an_answer_of_the_wrong_shape_is_refused_by_a_read() {
        let mut scenario = Scenario::open();
        scenario.positions = format!("0x{}{}", word(1), word(2));
        let (url, _) = node(scenario.handler()).await;
        assert!(
            adapter_for_tests()
                .read_position(request(&url), &key())
                .await
                .is_err()
        );

        // A tick outside the protocol's range is not a position.
        let mut scenario = Scenario::open();
        let mut words = words_of(&scenario.positions);
        words[5] = hex_word(U256::ZERO.wrapping_sub(raw("900000")));
        scenario.positions = hex_of(&words);
        let (url, _) = node(scenario.handler()).await;
        assert!(
            adapter_for_tests()
                .read_position(request(&url), &key())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_pool_that_refuses_a_call_is_an_error_and_not_a_zero() {
        let scenario = Scenario::open();
        let inner = scenario.handler();
        let handler: Handler = Box::new(move |target, calldata| {
            if &calldata[..8] == SEL_SLOT0 {
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

    #[test]
    fn the_shipped_adapter_carries_the_verified_decimals_selector() {
        // The scale of the money depends on this one: a wrong selector reads
        // some other function's word and misplaces every decimal point. It
        // is `keccak256("decimals()")[0..4]`, recomputed by the main session
        // on 2026-11-16 (see the constant), and it is the same four bytes the
        // tests of this file exercise everything else with.
        assert_eq!(DECIMALS_SELECTOR, Some("313ce567"));
        assert_eq!(
            DECIMALS_SELECTOR,
            Some(TEST_DECIMALS),
            "the shipped adapter reads what the tests read"
        );
    }

    /// An ABI `string` answer: offset, length, the bytes padded to words.
    fn string_answer(bytes: &[u8]) -> String {
        let mut text = format!("0x{}{}", word(32), word(bytes.len() as u128));
        for chunk in bytes.chunks(32) {
            text.push_str(&hex(chunk));
            text.push_str(&"00".repeat(32 - chunk.len()));
        }
        text
    }

    #[tokio::test]
    async fn both_tokens_are_listed_with_their_symbol_and_decimals() {
        let (url, _) = node(Scenario::open().handler()).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position");
        assert_eq!(
            reading.tokens,
            vec![
                LegToken {
                    token: TokenId::new(Chain::Ethereum, TOKEN0),
                    symbol: Some("USDC".into()),
                    name: None,
                    decimals: Some(6),
                },
                LegToken {
                    token: TokenId::new(Chain::Ethereum, TOKEN1),
                    symbol: Some("WETH".into()),
                    name: None,
                    decimals: Some(18),
                },
            ]
        );
    }

    #[tokio::test]
    async fn the_symbols_ride_the_one_multicall_batch() {
        let (url, seen) = node(Scenario::open().handler()).await;
        adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position");
        assert_eq!(
            seen.lock().expect("the log").len(),
            3,
            "positions, getPool, then one batch with the symbols inside"
        );
    }

    #[tokio::test]
    async fn a_token_that_refuses_symbol_or_answers_a_non_string_has_no_symbol() {
        let (url, _) = node(Scenario::open().with_odd_symbols().handler()).await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("the read still succeeds")
            .expect("the position");
        assert_eq!(reading.tokens.len(), 2);
        assert_eq!(reading.tokens[0].symbol, None);
        assert_eq!(reading.tokens[1].symbol, None);
        assert_eq!(reading.tokens[0].decimals, Some(6));
    }

    #[tokio::test]
    async fn an_oversized_or_non_utf8_symbol_is_passed_on_for_the_registration_to_clip() {
        let long = "A".repeat(300);
        let (url, _) = node(
            Scenario::open()
                .with_symbols(
                    &string_answer(long.as_bytes()),
                    &string_answer(&[0xff, 0xfe, b'X']),
                )
                .handler(),
        )
        .await;
        let reading = adapter_for_tests()
            .read_position(request(&url), &key())
            .await
            .expect("a read")
            .expect("the position");
        assert_eq!(reading.tokens[0].symbol.as_deref(), Some(long.as_str()));
        let second = reading.tokens[1].symbol.clone().expect("a lossy symbol");
        assert!(second.ends_with('X'), "{second}");
    }

    #[test]
    fn the_symbol_selector_is_the_one_of_erc20_symbol() {
        assert_eq!(SYMBOL_SELECTOR, "95d89b41");
    }

    #[tokio::test]
    async fn a_token_that_refuses_decimals_is_an_error_and_not_a_guessed_scale() {
        // A contract that answers nothing to `decimals()` leaves the scale of
        // the money unknown, and 18 is not a safe default: on USDC it would
        // report a millionth of the real figure.
        let (url, _) = node(Scenario::open().without_decimals().handler()).await;
        let outcome = adapter_for_tests()
            .read_position(request(&url), &key())
            .await;
        assert!(
            outcome.is_err(),
            "an unknown scale refuses the read instead of inventing one"
        );
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
