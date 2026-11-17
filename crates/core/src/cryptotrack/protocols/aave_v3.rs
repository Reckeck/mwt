//! Aave v3 supplies and borrows (spec D3).
//!
//! A position is **one reserve the user has touched**: the key is the
//! reserve's underlying token, [`discover`](ProtocolAdapter::discover) walks
//! the market's reserve list and keeps the reserves where the address has a
//! supply or a debt, and [`read_position`](ProtocolAdapter::read_position) is
//! `getUserReserveData` for that one reserve. A reserve with a zero supply and
//! a zero debt is not a position.
//!
//! # What a reading says
//!
//! * [`PositionKind::Lending`]; the supplied amount is a leg
//!   [`CryptoDirection::In`], the borrowed amount a leg
//!   [`CryptoDirection::Out`], both in the **underlying** token (never the
//!   aToken, which is a receipt and goes to
//!   [`PositionReading::receipts`] so the same money is not counted twice).
//!   The value is collateral minus debt; the engine subtracts, the adapter
//!   only reports the legs.
//! * Interest accrues **inside** the aToken balance (and the debt grows inside
//!   the debt balance); the protocol keeps nothing apart. So
//!   [`PositionReading::fees`] is [`FeeReading::none_earned`] and not
//!   [`FeeReading::unknown`]: that is what makes the uncollected fees of a
//!   lending position a real zero rather than a gap.
//! * Stable debt is deprecated (facts, Aave section 7): its four fields read
//!   `0` unconditionally. A zero there is a fact, not missing data, and it is
//!   never read as a leg. If one is ever not zero the reading is marked
//!   approximate — the layout is not the one documented — rather than the debt
//!   being dropped silently.
//!
//! # Rounding, and the index
//!
//! `getUserReserveData` returns the balances the contract itself computes.
//! They are recomputed here from the scaled balance and the **current**
//! normalized index (`getReserveNormalizedIncome` /
//! `getReserveNormalizedVariableDebt`, not the stored `liquidityIndex`, which
//! lags) with the protocol's rounding — the aToken **floor**
//! ([`a_token_balance`]), the debt **ceil** ([`variable_debt`]) — and the two
//! are compared. They agree to the wei on the documented deployments; a
//! difference of more than one wei (older markets may round half up, facts
//! "Unverified" 2) marks the reading approximate. The contract's own number is
//! what is reported.
//!
//! # Health factor
//!
//! The health factor is **an account-wide figure of a market**
//! (`Pool.getUserAccountData`), not a property of one reserve, and it is not
//! money. [`PositionReading`] has no field for it and this adapter invents
//! none; [`AaveV3::account_health`] reads it for a screen that wants to show
//! it. Nothing in this file lets it into a quantity or a total.
//!
//! # Operations (`events`)
//!
//! Supply is `Stake`, withdraw `Unstake`. Borrow is `Unstake` and repay is
//! `Stake`: the "invested at the price of the day" the service derives is the
//! user's own money put into the position, and a borrowed token lands in the
//! wallet (money taken out of the position) while a repayment goes back in, so
//! `stake - unstake` stays equal to supply minus withdraw minus borrow plus
//! repay, the same sign as the collateral-minus-debt value. **None of this is
//! implemented**: see [`ProtocolAdapter::events`] below for why.
//!
//! # Markets
//!
//! Only the **Core** market of each network is read. Ethereum has two more
//! (Prime, formerly "Lido", and EtherFi; facts, Aave section 1) and they are
//! left out; another network has one market.

use crate::Amount;
use crate::cryptotrack::rpc::RpcClient;
use crate::cryptotrack::sources::evm::abi;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, TokenId, normalize_address,
};

use super::{
    AdapterFuture, Answer, FeeReading, Node, PositionKey, PositionKind, PositionReading,
    PositionStatus, ProtocolAdapter, ProtocolRequest, U256,
};

/// The protocol id a position of this protocol stores.
pub const PROTOCOL: &str = "aave-v3";

/// `getUserReserveData(address,address)` on the data provider (facts, Aave 2).
const SEL_USER_RESERVE_DATA: &str = "28dd2d01";
/// `getAllReservesTokens()` on the data provider (facts, Aave 2).
const SEL_ALL_RESERVES: &str = "b316ff89";
/// `getReserveTokensAddresses(address)` on the data provider (facts, Aave 2).
const SEL_RESERVE_TOKENS: &str = "d2493b6c";
/// `getReserveNormalizedIncome(address)` on the Pool (facts, Aave 2, helpers).
const SEL_NORMALIZED_INCOME: &str = "d15e0053";
/// `getReserveNormalizedVariableDebt(address)` on the Pool (facts, Aave 2,
/// helpers).
const SEL_NORMALIZED_DEBT: &str = "386497fd";
/// `scaledBalanceOf(address)` on an aToken (facts, Aave 2, helpers).
const SEL_SCALED_BALANCE: &str = "1da24f3e";
/// `getUserAccountData(address)` on the Pool (facts, Aave 2).
const SEL_ACCOUNT_DATA: &str = "bf92857c";
/// `decimals()` of the underlying token.
///
/// **Not in the facts document**, which quotes the decimals read from chain
/// but never the selector. It is the keccak-256 of the standard ERC-20
/// signature `decimals()`, computed locally with the same method the facts
/// document describes (the implementation was first checked against the
/// document's own selectors: `getUserReserveData`, `getAllReservesTokens`,
/// `getReserveTokensAddresses`, the two normalized indices, `scaledBalanceOf`,
/// `getUserAccountData`, all of which it reproduced). Reported as a gap for
/// the main session to accept.
const SEL_DECIMALS: &str = "313ce567";

/// The words of `getUserReserveData` (facts, Aave 3).
const USER_DATA_WORDS: usize = 9;
const WORD_ATOKEN_BALANCE: usize = 0;
const WORD_STABLE_DEBT: usize = 1;
const WORD_VARIABLE_DEBT: usize = 2;
const WORD_PRINCIPAL_STABLE_DEBT: usize = 3;
const WORD_SCALED_VARIABLE_DEBT: usize = 4;

/// `getUserAccountData` carries the health factor in its sixth word, 1e18
/// scaled (facts, Aave 4).
const WORD_HEALTH_FACTOR: usize = 5;
const ACCOUNT_DATA_WORDS: usize = 6;
const HEALTH_FACTOR_DECIMALS: u8 = 18;

/// Longest symbol kept from the reserve list: a node's answer is untrusted.
const SYMBOL_MAX_BYTES: u128 = 64;

/// One Aave v3 market: the Pool and the data provider that reads it (facts,
/// Aave 1, Core market).
struct Market {
    chain: Chain,
    pool: &'static str,
    data_provider: &'static str,
}

/// The Core market of each network, from the Aave Address Book as the facts
/// document quotes it; Pool and data provider are wired to each other on
/// every row (`POOL()` read back, facts Aave 1).
const MARKETS: [Market; 6] = [
    Market {
        chain: Chain::Ethereum,
        pool: "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2",
        data_provider: "0x0a16f2fcc0d44fae41cc54e079281d84a363becd",
    },
    Market {
        chain: Chain::Arbitrum,
        pool: "0x794a61358d6845594f94dc1db02a252b5b4814ad",
        data_provider: "0x243aa95cac2a25651eda86e80bee66114413c43b",
    },
    Market {
        chain: Chain::Optimism,
        pool: "0x794a61358d6845594f94dc1db02a252b5b4814ad",
        data_provider: "0x243aa95cac2a25651eda86e80bee66114413c43b",
    },
    Market {
        chain: Chain::Base,
        pool: "0xa238dd80c259a72e81d7e4664a9801593f98d1c5",
        data_provider: "0x0f43731eb8d45a581f4a36dd74f5f358bc90c73a",
    },
    Market {
        chain: Chain::Polygon,
        pool: "0x794a61358d6845594f94dc1db02a252b5b4814ad",
        data_provider: "0x243aa95cac2a25651eda86e80bee66114413c43b",
    },
    Market {
        chain: Chain::Bsc,
        pool: "0x6807dc923806fe8fd134338eabca509979a7e0cb",
        data_provider: "0xc90df74a7c16245c5f5c5870327ceb38fe5d5328",
    },
];

fn market(chain: Chain) -> Option<&'static Market> {
    MARKETS.iter().find(|market| market.chain == chain)
}

/// The tokens this protocol hands over on `chain` that are known without a
/// call (see [`super::receipt_tokens`]).
///
/// **This table can only be partial, and that is deliberate.** The aToken of
/// a reserve is not derivable: its name, symbol and address are chosen by
/// governance when the reserve is listed, and the only source is
/// `getReserveTokensAddresses` (facts, Aave 6: "a reader must never compute an
/// address or a symbol from the underlying symbol"). The facts document
/// carries exactly one aToken and one debt token as verified, on Ethereum Core
/// WETH, and those are the only entries: an address not in the document is not
/// guessed. Everything else is covered by the live path —
/// [`PositionReading::receipts`] carries the aToken and the variable debt
/// token of every reserve actually read — and, on a device that has not read
/// the chain, by the stored operations. The consequence to know: on a device
/// that only syncs, the aToken of any other reserve is still counted as a
/// wallet token until a device that read the chain has seen the position.
///
/// The variable debt token is listed too. It is not transferable, but a node
/// reports its balance as a holding, and counting a debt as an asset would
/// inflate the capital.
pub fn receipt_tokens(chain: Chain) -> Vec<CryptoAsset> {
    match chain {
        Chain::Ethereum => [
            // aEthWETH (facts, Aave 6).
            "0x4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8",
            // variableDebtEthWETH (facts, Aave 6).
            "0xea51d7853eefb32b6ee06b1c12e6dcca88be0ffe",
        ]
        .iter()
        .map(|contract| CryptoAsset::Token(TokenId::new(chain, contract)))
        .collect(),
        _ => Vec::new(),
    }
}

/// `10^27`: the unit of Aave's indices and rates.
fn ray() -> U256 {
    U256::from_u128(10u128.pow(27))
}

/// The aToken balance of a scaled balance: `floor(scaled * income / 1e27)`.
/// The aToken rounds **down** (`TokenMath.getATokenBalance`, facts Aave 5),
/// confirmed to the wei on live Ethereum Core WETH. `None` when the result
/// does not fit a word — a wrong reading, never a guess.
pub fn a_token_balance(scaled: U256, normalized_income: U256) -> Option<U256> {
    scaled.mul_div_floor(normalized_income, ray())
}

/// The variable debt of a scaled debt: `ceil(scaled * index / 1e27)`. The debt
/// rounds **up** (`TokenMath.getVTokenBalance`, facts Aave 5), so a debt is
/// never shown smaller than the protocol will ask for.
pub fn variable_debt(scaled_debt: U256, normalized_debt: U256) -> Option<U256> {
    mul_div_ceil(scaled_debt, normalized_debt, ray())
}

/// `ceil(value * factor / divisor)` as `floor((value * factor + divisor - 1) /
/// divisor)`: the explicit `+ divisor - 1` form, since [`U256`] has the floor
/// division only. The product is taken in one word first (`None` when it does
/// not fit: a scaled balance times an index is nowhere near 2^256 unless the
/// answer is garbage), and the floor division then runs on the 512-bit path.
fn mul_div_ceil(value: U256, factor: U256, divisor: U256) -> Option<U256> {
    if divisor.is_zero() {
        return None;
    }
    let one = U256::from_u128(1);
    value
        .checked_mul(factor)?
        .checked_add(divisor.wrapping_sub(one))?
        .mul_div_floor(one, divisor)
}

/// Whether two quantities differ by at most one wei.
fn within_one_wei(left: U256, right: U256) -> bool {
    let difference = if left >= right {
        left.wrapping_sub(right)
    } else {
        right.wrapping_sub(left)
    };
    difference <= U256::from_u128(1)
}

/// One reserve of the market's list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reserve {
    symbol: String,
    address: String,
}

/// A word that holds a byte offset, as an index of the 32-byte word it
/// points to. Refused when it is not a whole number of words or is absurd.
fn word_index(answer: &Answer, at: usize) -> Result<usize> {
    let offset = usize::try_from(answer.u128(at)?)
        .map_err(|_| Error::Network("an offset does not fit".into()))?;
    if !offset.is_multiple_of(32) {
        return Err(Error::Network("an offset is not a whole word".into()));
    }
    Ok(offset / 32)
}

/// `getAllReservesTokens()`: `TokenData[] { string symbol; address
/// tokenAddress }`, a dynamic array of dynamic structs (facts, Aave 6), so
/// offset-encoded and not flat. Every offset is checked against the answer.
fn decode_reserves(answer: &Answer) -> Result<Vec<Reserve>> {
    let array = word_index(answer, 0)?;
    let count = usize::try_from(answer.u128(array)?)
        .map_err(|_| Error::Network("a reserve count does not fit".into()))?;
    if count > answer.len() {
        return Err(Error::Network(
            "more reserves than the answer has words".into(),
        ));
    }
    let elements = array.saturating_add(1);
    let mut reserves = Vec::with_capacity(count);
    for position in 0..count {
        let element =
            elements.saturating_add(word_index(answer, elements.saturating_add(position))?);
        let text = element.saturating_add(word_index(answer, element)?);
        let address = answer.address(element.saturating_add(1))?;
        let length = answer.u128(text)?;
        if length > SYMBOL_MAX_BYTES {
            return Err(Error::Network("a reserve symbol is too long".into()));
        }
        let length = usize::try_from(length)
            .map_err(|_| Error::Network("a symbol length does not fit".into()))?;
        let mut bytes = Vec::with_capacity(length);
        for chunk in 0..length.div_ceil(32) {
            bytes.extend_from_slice(answer.word(text.saturating_add(1).saturating_add(chunk))?);
        }
        bytes.truncate(length);
        let symbol: String = String::from_utf8_lossy(&bytes)
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        reserves.push(Reserve { symbol, address });
    }
    Ok(reserves)
}

/// What `getUserReserveData` says about the user in one reserve.
struct UserReserve {
    supply: U256,
    variable_debt: U256,
    scaled_variable_debt: U256,
    /// Whether either stable-debt balance is not zero, which the deployed
    /// contract never does (facts, Aave 7).
    stable_debt_present: bool,
}

impl UserReserve {
    fn parse(answer: &Answer) -> Result<Self> {
        if answer.len() < USER_DATA_WORDS {
            return Err(Error::Network(
                "getUserReserveData answered with fewer than nine words".into(),
            ));
        }
        Ok(Self {
            supply: answer.u256(WORD_ATOKEN_BALANCE)?,
            variable_debt: answer.u256(WORD_VARIABLE_DEBT)?,
            scaled_variable_debt: answer.u256(WORD_SCALED_VARIABLE_DEBT)?,
            stable_debt_present: !answer.u256(WORD_STABLE_DEBT)?.is_zero()
                || !answer.u256(WORD_PRINCIPAL_STABLE_DEBT)?.is_zero(),
        })
    }

    /// A position is a reserve with a supply or a debt; the stable fields do
    /// not count, they are dropped by contract (facts, Aave 7).
    fn is_position(&self) -> bool {
        !self.supply.is_zero() || !self.variable_debt.is_zero()
    }
}

/// `(contract, calldata)` of one call.
fn call_of(to: &str, selector: &str, args: &[[u8; 32]]) -> Result<(String, String)> {
    Ok((to.to_string(), abi::calldata(selector, args)?))
}

/// The health factor of an account in a market, from `getUserAccountData`:
/// 1e18 scaled, `type(uint256).max` when there is no debt (facts, Aave 4).
/// `None` for no debt, because "infinite" is not a number. Display only.
fn health_factor_of(answer: &Answer) -> Result<Option<Amount>> {
    if answer.len() < ACCOUNT_DATA_WORDS {
        return Err(Error::Network(
            "getUserAccountData answered with fewer than six words".into(),
        ));
    }
    let health = answer.u256(WORD_HEALTH_FACTOR)?;
    if health == U256::MAX {
        return Ok(None);
    }
    health.to_amount(HEALTH_FACTOR_DECIMALS).map(Some)
}

/// The Aave v3 adapter: holds the node client and builds a [`Node`] per
/// request.
pub struct AaveV3 {
    client: RpcClient,
}

/// The adapter of this protocol, for [`super::Registry::builtin`].
pub fn adapter(node: RpcClient) -> Box<dyn ProtocolAdapter> {
    Box::new(AaveV3 { client: node })
}

impl AaveV3 {
    /// The health factor of the address in the Core market of the network:
    /// **for display only**, account-wide and not money. `None` when the
    /// address has no debt (no finite factor).
    pub async fn account_health(&self, request: ProtocolRequest<'_>) -> Result<Option<Amount>> {
        let market = market(request.chain).ok_or(Error::Unsupported)?;
        let node = Node::new(&self.client, &request)?;
        let user = abi::address_word(node.address())?;
        let answer = node.call(market.pool, SEL_ACCOUNT_DATA, &[user]).await?;
        health_factor_of(&answer)
    }

    async fn find_positions(&self, request: ProtocolRequest<'_>) -> Result<Vec<PositionKey>> {
        let market = market(request.chain).ok_or(Error::Unsupported)?;
        let node = Node::new(&self.client, &request)?;
        let reserves = decode_reserves(
            &node
                .call(market.data_provider, SEL_ALL_RESERVES, &[])
                .await?,
        )?;
        let user = abi::address_word(node.address())?;
        let mut calls = Vec::with_capacity(reserves.len());
        for reserve in &reserves {
            calls.push(call_of(
                market.data_provider,
                SEL_USER_RESERVE_DATA,
                &[abi::address_word(&reserve.address)?, user],
            )?);
        }
        let answers = node.call_many(&calls).await?;
        let mut keys = Vec::new();
        for (reserve, answer) in reserves.iter().zip(answers) {
            // A refused call is a reserve that cannot be read, not a position.
            let Some(answer) = answer else { continue };
            if UserReserve::parse(&answer)?.is_position() {
                keys.push(PositionKey::new(PROTOCOL, request.chain, &reserve.address));
            }
        }
        Ok(keys)
    }

    async fn read_reserve(
        &self,
        request: ProtocolRequest<'_>,
        key: &PositionKey,
    ) -> Result<Option<PositionReading>> {
        if key.protocol != PROTOCOL || key.chain != request.chain {
            return Err(Error::InvalidInput(
                "not an Aave v3 key of this network".into(),
            ));
        }
        let market = market(request.chain).ok_or(Error::Unsupported)?;
        let node = Node::new(&self.client, &request)?;
        let asset_address = normalize_address(request.chain, &key.id);
        let asset = abi::address_word(&asset_address)?;
        let user = abi::address_word(node.address())?;

        // One block for all of it: the balances, both indices, the token
        // addresses and the decimals are read together, so the recomputation
        // below compares numbers of the same moment.
        let calls = [
            call_of(market.data_provider, SEL_USER_RESERVE_DATA, &[asset, user])?,
            call_of(market.data_provider, SEL_RESERVE_TOKENS, &[asset])?,
            call_of(market.pool, SEL_NORMALIZED_INCOME, &[asset])?,
            call_of(market.pool, SEL_NORMALIZED_DEBT, &[asset])?,
            call_of(&asset_address, SEL_DECIMALS, &[])?,
            call_of(market.data_provider, SEL_ALL_RESERVES, &[])?,
        ];
        let answers = node.call_many(&calls).await?;
        if answers.len() != calls.len() {
            return Err(Error::Network("a call went unanswered".into()));
        }
        let Some(user_data) = &answers[0] else {
            return Ok(None);
        };
        let required = |at: usize| {
            answers[at]
                .as_ref()
                .ok_or_else(|| Error::Network(format!("call {at} of the reserve was refused")))
        };
        let reserve = UserReserve::parse(user_data)?;
        if !reserve.is_position() {
            return Ok(None);
        }
        let tokens = required(1)?;
        let income = required(2)?.u256(0)?;
        let debt_index = required(3)?.u256(0)?;
        let decimals = u8::try_from(required(4)?.u128(0)?)
            .map_err(|_| Error::Network("decimals do not fit a byte".into()))?;
        let a_token = tokens.address(0)?;
        // Word 1 is the stable debt token, always address(0) (facts, Aave 6).
        let debt_token = tokens.address(2)?;

        // The check of the contract's balances against the index math.
        let mut exact = !reserve.stable_debt_present;
        if !reserve.supply.is_zero() {
            let scaled = node
                .call(&a_token, SEL_SCALED_BALANCE, &[user])
                .await?
                .u256(0)?;
            exact &= a_token_balance(scaled, income)
                .is_some_and(|expected| within_one_wei(expected, reserve.supply));
        }
        if !reserve.variable_debt.is_zero() {
            exact &= variable_debt(reserve.scaled_variable_debt, debt_index)
                .is_some_and(|expected| within_one_wei(expected, reserve.variable_debt));
        }

        let asset_id = CryptoAsset::Token(TokenId::new(request.chain, &asset_address));
        let mut legs = Vec::new();
        for (direction, quantity) in [
            (CryptoDirection::In, reserve.supply),
            (CryptoDirection::Out, reserve.variable_debt),
        ] {
            if !quantity.is_zero() {
                legs.push(CryptoLeg {
                    direction,
                    asset: asset_id.clone(),
                    amount: quantity.to_amount(decimals)?,
                    counterparty: None,
                });
            }
        }
        let symbol = answers[5]
            .as_ref()
            .and_then(|list| decode_reserves(list).ok())
            .and_then(|list| {
                list.into_iter()
                    .find(|listed| listed.address == asset_address)
            })
            .map(|listed| listed.symbol)
            .filter(|symbol| !symbol.is_empty());
        let zero = format!("0x{}", "0".repeat(40));
        let receipts = [a_token, debt_token]
            .into_iter()
            .filter(|token| *token != zero)
            .map(|token| CryptoAsset::Token(TokenId::new(request.chain, &token)))
            .collect();
        Ok(Some(PositionReading {
            key: key.clone(),
            account_id: request.account_id.to_string(),
            name: format!("Aave v3 {}", symbol.as_deref().unwrap_or(&asset_address)),
            kind: PositionKind::Lending,
            status: PositionStatus::Open,
            legs,
            // Interest is inside the aToken balance and inside the debt, the
            // protocol keeps nothing apart: so there is nothing uncollected
            // and nothing collected, a real zero and not a gap. This is what
            // makes a lending position's uncollected fees `0` rather than `-`.
            fees: FeeReading::none_earned(),
            receipts,
            approximate: !exact,
            opened_at: None,
            closed_at: None,
            tokens: Vec::new(),
            read_at: crate::model::vault::now_iso(),
        }))
    }
}

impl ProtocolAdapter for AaveV3 {
    fn protocol(&self) -> &'static str {
        PROTOCOL
    }

    fn supports(&self, chain: Chain) -> bool {
        market(chain).is_some()
    }

    fn discover<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
    ) -> AdapterFuture<'fut, Vec<PositionKey>> {
        Box::pin(self.find_positions(request))
    }

    fn read_position<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
        key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Option<PositionReading>> {
        Box::pin(self.read_reserve(request, key))
    }

    /// Always empty, and not by oversight: the operations of a reserve are the
    /// Pool's `Supply`, `Withdraw`, `Borrow` and `Repay` logs filtered by the
    /// user, and this layer cannot read them. The facts document gives none
    /// of the four event signatures (so no topic to filter on), no deployment
    /// block of any Pool (so no floor for the scan), and the gas of an
    /// operation needs the transaction receipt, which `node` does not offer.
    /// An invented topic would read someone else's events, so the honest answer
    /// is nothing. The mapping to use once they exist is in the module
    /// documentation. Until then a lending position has no "invested at the
    /// price of the day" and no gas; the screen shows a dash for them.
    fn events<'fut>(
        &'fut self,
        _request: ProtocolRequest<'fut>,
        _key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Vec<CryptoTx>> {
        Box::pin(async move { Ok(Vec::new()) })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::cryptotrack::protocols::node::stub;

    const USER: &str = "0x1111111111111111111111111111111111111111";
    const USDC: &str = "0x00000000000000000000000000000000000000a1";
    const WETH: &str = "0x00000000000000000000000000000000000000a2";
    const A_TOKEN: &str = "0x00000000000000000000000000000000000000b2";
    const DEBT_TOKEN: &str = "0x00000000000000000000000000000000000000c2";

    // Every fixture is hand-built from the documented layout (the facts
    // document's Aave section), with synthetic reserve and token addresses;
    // the one real number is the WETH normalized income. Each file says so in
    // its `note`. No live node answer was available to this executor.
    const RESERVES: &str = include_str!("fixtures/aave-v3/getAllReservesTokens.two-reserves.json");
    const USER_DATA: &str =
        include_str!("fixtures/aave-v3/getUserReserveData.supply-and-debt.json");
    const UNTOUCHED: &str = include_str!("fixtures/aave-v3/getUserReserveData.untouched.json");
    const STABLE: &str = include_str!("fixtures/aave-v3/getUserReserveData.stable-debt.json");
    const TOKENS: &str = include_str!("fixtures/aave-v3/getReserveTokensAddresses.weth.json");
    const INCOME: &str = include_str!("fixtures/aave-v3/getReserveNormalizedIncome.weth.json");
    const DEBT_INDEX: &str =
        include_str!("fixtures/aave-v3/getReserveNormalizedVariableDebt.weth.json");
    const DECIMALS: &str = include_str!("fixtures/aave-v3/decimals.18.json");
    const SCALED: &str = include_str!("fixtures/aave-v3/scaledBalanceOf.weth.json");
    const ACCOUNT: &str = include_str!("fixtures/aave-v3/getUserAccountData.indebted.json");
    const ACCOUNT_NO_DEBT: &str = include_str!("fixtures/aave-v3/getUserAccountData.no-debt.json");

    fn bytes(fixture: &str) -> Vec<u8> {
        let value: Value = serde_json::from_str(fixture).expect("a fixture is JSON");
        let text = value["result"].as_str().expect("a result");
        (0..text.len() / 2 - 1)
            .map(|index| u8::from_str_radix(&text[2 + 2 * index..4 + 2 * index], 16).expect("hex"))
            .collect()
    }

    fn hex(data: &[u8]) -> String {
        format!(
            "0x{}",
            data.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }

    fn word(value: u128) -> Vec<u8> {
        let mut word = vec![0u8; 16];
        word.extend_from_slice(&value.to_be_bytes());
        word
    }

    /// The calls inside an incoming `aggregate3`: `(target, calldata)`.
    fn aggregate_calls(calldata: &str) -> Vec<(String, Vec<u8>)> {
        let data: Vec<u8> = (2..calldata.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&calldata[at..at + 2], 16).expect("hex"))
            .collect();
        let body = &data[4..];
        let read = |at: usize| usize::from_be_bytes(body[at + 24..at + 32].try_into().expect("8"));
        let count = read(32);
        (0..count)
            .map(|index| {
                let element = 64 + read(64 + 32 * index);
                let target = hex(&body[element + 12..element + 32]);
                let start = element + read(element + 64);
                let length =
                    usize::from_be_bytes(body[start + 24..start + 32].try_into().expect("8"));
                (target, body[start + 32..start + 32 + length].to_vec())
            })
            .collect()
    }

    /// An `aggregate3` answer: `(bool success, bytes returnData)[]`.
    fn aggregate_answer(results: &[Option<Vec<u8>>]) -> String {
        let mut elements = Vec::new();
        for result in results {
            let data = result.clone().unwrap_or_default();
            let mut element = word(u128::from(result.is_some()));
            element.extend(word(0x40));
            element.extend(word(data.len() as u128));
            element.extend(&data);
            element.resize(element.len() + (32 - data.len() % 32) % 32, 0);
            elements.push(element);
        }
        let mut out = word(0x20);
        out.extend(word(results.len() as u128));
        let mut offset = 32 * results.len();
        for element in &elements {
            out.extend(word(offset as u128));
            offset += element.len();
        }
        for element in &elements {
            out.extend(element);
        }
        hex(&out)
    }

    /// A node that answers by `(contract, selector, calldata)`; `None` is a
    /// call the contract refuses. Direct calls and Multicall3 are both served.
    async fn node(
        responder: impl Fn(&str, &str, &[u8]) -> Option<Vec<u8>> + Send + Sync + 'static,
    ) -> (String, stub::Seen) {
        stub::serve(move |method, params, _index| {
            assert_eq!(method, "eth_call");
            let to = params[0]["to"].as_str().expect("to").to_lowercase();
            let data = params[0]["data"].as_str().expect("data");
            if to == abi::MULTICALL3.to_lowercase() {
                let answers: Vec<Option<Vec<u8>>> = aggregate_calls(data)
                    .iter()
                    .map(|(target, calldata)| {
                        responder(target, &hex(&calldata[..4])[2..], calldata)
                    })
                    .collect();
                return json!(aggregate_answer(&answers));
            }
            let calldata: Vec<u8> = (2..data.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&data[at..at + 2], 16).expect("hex"))
                .collect();
            match responder(&to, &data[2..10], &calldata) {
                Some(answer) => json!(hex(&answer)),
                None => Value::Null,
            }
        })
        .await
    }

    fn request(url: &str) -> ProtocolRequest<'_> {
        ProtocolRequest {
            chain: Chain::Ethereum,
            address: USER,
            account_id: "cacc-1",
            since: None,
            rpc: Some(url),
            memory: None,
        }
    }

    /// The reserve `WETH` of the fixtures as one node: `user_data` is what
    /// `getUserReserveData` answers for it, `scaled` the aToken's scaled
    /// balance, USDC is never touched.
    fn weth_node(
        user_data: &'static str,
        scaled: &'static str,
    ) -> impl Fn(&str, &str, &[u8]) -> Option<Vec<u8>> + Send + Sync + 'static {
        let market = market(Chain::Ethereum).expect("a market");
        move |to, selector, calldata| {
            let asked_weth = calldata
                .windows(20)
                .any(|window| window == [&[0u8; 19][..], &[0xa2]].concat().as_slice());
            match (to, selector) {
                (to, SEL_ALL_RESERVES) if to == market.data_provider => Some(bytes(RESERVES)),
                (to, SEL_USER_RESERVE_DATA) if to == market.data_provider => Some(if asked_weth {
                    bytes(user_data)
                } else {
                    bytes(UNTOUCHED)
                }),
                (to, SEL_RESERVE_TOKENS) if to == market.data_provider => Some(bytes(TOKENS)),
                (to, SEL_NORMALIZED_INCOME) if to == market.pool => Some(bytes(INCOME)),
                (to, SEL_NORMALIZED_DEBT) if to == market.pool => Some(bytes(DEBT_INDEX)),
                (to, SEL_DECIMALS) if to == WETH => Some(bytes(DECIMALS)),
                (to, SEL_SCALED_BALANCE) if to == A_TOKEN => Some(bytes(scaled)),
                _ => None,
            }
        }
    }

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount")
    }

    fn u256(text: &str) -> U256 {
        U256::from_decimal(text).expect("a decimal")
    }

    #[test]
    fn the_atoken_rounds_down_and_the_debt_rounds_up_by_one_wei() {
        // One unit of scaled balance at an index of 1 ray and one unit: the
        // exact product is 1.000…001, so floor and ceil are a wei apart.
        let index = u256("1000000000000000000000000001");
        let one = u256("1");
        assert_eq!(a_token_balance(one, index), Some(u256("1")));
        assert_eq!(variable_debt(one, index), Some(u256("2")));
        // The fixtures' own pair: supply floor, debt ceil.
        let income = u256("1073258999975285341551629473");
        assert_eq!(
            a_token_balance(u256("2000000000000000000"), income),
            Some(u256("2146517999950570683")),
            "2e18 * 1.0732589999752853415… = …683.10 floors to …683"
        );
        let debt_index = u256("1050000000000000000000000001");
        assert_eq!(
            variable_debt(u256("500000000000000000"), debt_index),
            Some(u256("525000000000000001")),
            "…000.0000000005 ceils to …001"
        );
        assert_eq!(
            u256("500000000000000000").mul_div_floor(debt_index, ray()),
            Some(u256("525000000000000000")),
            "the floor of the same product is one wei less"
        );
    }

    #[test]
    fn an_exact_product_is_not_rounded_up_and_nonsense_is_refused() {
        let index = ray();
        assert_eq!(variable_debt(u256("7"), index), Some(u256("7")));
        assert_eq!(mul_div_ceil(u256("1"), u256("1"), U256::ZERO), None);
        assert_eq!(
            variable_debt(U256::MAX, u256("2")),
            None,
            "a product wider than a word is a wrong reading"
        );
    }

    #[test]
    fn the_markets_are_the_core_markets_of_the_facts_document() {
        let adapter = AaveV3 {
            client: stub::client(),
        };
        for chain in [
            Chain::Ethereum,
            Chain::Arbitrum,
            Chain::Optimism,
            Chain::Base,
            Chain::Polygon,
            Chain::Bsc,
        ] {
            assert!(adapter.supports(chain), "{chain:?}");
        }
        assert!(!adapter.supports(Chain::Avalanche));
        assert!(!adapter.supports(Chain::Solana));
        let ethereum = market(Chain::Ethereum).expect("a market");
        assert_eq!(ethereum.pool, "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2");
        assert_eq!(
            ethereum.data_provider,
            "0x0a16f2fcc0d44fae41cc54e079281d84a363becd"
        );
    }

    #[test]
    fn the_receipt_table_holds_only_what_the_facts_document_verified() {
        let known = receipt_tokens(Chain::Ethereum);
        assert_eq!(
            known,
            vec![
                CryptoAsset::Token(TokenId::new(
                    Chain::Ethereum,
                    "0x4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8"
                )),
                CryptoAsset::Token(TokenId::new(
                    Chain::Ethereum,
                    "0xea51d7853eefb32b6ee06b1c12e6dcca88be0ffe"
                )),
            ]
        );
        assert_eq!(
            crate::cryptotrack::protocols::receipt_tokens(PROTOCOL, Chain::Ethereum),
            known
        );
        assert!(receipt_tokens(Chain::Base).is_empty());
    }

    #[test]
    fn the_reserve_list_is_decoded_from_its_offsets() {
        let answer = Answer::of(
            crate::cryptotrack::sources::evm::abi::split_words(&bytes(RESERVES)).expect("words"),
        );
        assert_eq!(
            decode_reserves(&answer).expect("two reserves"),
            vec![
                Reserve {
                    symbol: "USDC".into(),
                    address: USDC.into()
                },
                Reserve {
                    symbol: "WETH".into(),
                    address: WETH.into()
                },
            ]
        );
    }

    #[test]
    fn a_reserve_list_of_the_wrong_shape_is_an_error_not_a_panic() {
        let words = abi::split_words(&bytes(RESERVES)).expect("words");
        // Cut anywhere: every prefix is refused or decodes to fewer than all.
        for keep in 0..words.len() {
            let _ = decode_reserves(&Answer::of(words[..keep].to_vec()));
        }
        assert!(decode_reserves(&Answer::of(words[..3].to_vec())).is_err());
        // A count that claims more than the answer holds.
        let mut lying = words.clone();
        lying[1] = {
            let mut huge = [0u8; 32];
            huge[24..].copy_from_slice(&u64::MAX.to_be_bytes());
            huge
        };
        assert!(decode_reserves(&Answer::of(lying)).is_err());
        // An offset that is not a whole word.
        let mut crooked = words;
        crooked[0][31] = 0x21;
        assert!(decode_reserves(&Answer::of(crooked)).is_err());
    }

    #[tokio::test]
    async fn discovery_keeps_the_reserves_the_user_has_touched() {
        let (url, seen) = node(weth_node(USER_DATA, SCALED)).await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let keys = adapter.discover(request(&url)).await.expect("a discovery");
        assert_eq!(
            keys,
            vec![PositionKey::new(PROTOCOL, Chain::Ethereum, WETH)]
        );
        assert_eq!(
            seen.lock().expect("log").len(),
            2,
            "the list, then every reserve in one Multicall3 request"
        );
    }

    #[tokio::test]
    async fn a_supply_and_a_debt_are_two_legs_and_the_value_is_their_difference() {
        let (url, _) = node(weth_node(USER_DATA, SCALED)).await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        let reading = adapter
            .read_position(request(&url), &key)
            .await
            .expect("a reading")
            .expect("the position is there");
        assert_eq!(reading.kind, PositionKind::Lending);
        assert_eq!(reading.status, PositionStatus::Open);
        assert_eq!(reading.name, "Aave v3 WETH");
        assert_eq!(reading.account_id, "cacc-1");
        let underlying = CryptoAsset::Token(TokenId::new(Chain::Ethereum, WETH));
        assert_eq!(
            reading.legs,
            vec![
                CryptoLeg {
                    direction: CryptoDirection::In,
                    asset: underlying.clone(),
                    amount: amount("2.146517999950570683"),
                    counterparty: None,
                },
                CryptoLeg {
                    direction: CryptoDirection::Out,
                    asset: underlying,
                    amount: amount("0.525000000000000001"),
                    counterparty: None,
                },
            ]
        );
        assert!(
            reading
                .legs
                .iter()
                .all(|leg| leg.asset != CryptoAsset::Token(TokenId::new(Chain::Ethereum, A_TOKEN))),
            "the aToken is a receipt, not a leg: it would count the supply twice"
        );
        assert_eq!(
            reading.receipts,
            vec![
                CryptoAsset::Token(TokenId::new(Chain::Ethereum, A_TOKEN)),
                CryptoAsset::Token(TokenId::new(Chain::Ethereum, DEBT_TOKEN)),
            ],
            "the stable debt token, address(0), is not a receipt"
        );
        assert_eq!(
            reading.fees,
            FeeReading::none_earned(),
            "interest is inside the balance: a real zero, not a gap"
        );
        assert!(
            !reading.approximate,
            "the index math reproduces the contract's balances"
        );
    }

    #[tokio::test]
    async fn a_reserve_with_nothing_in_it_is_not_a_position() {
        let (url, _) = node(weth_node(UNTOUCHED, SCALED)).await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        assert!(
            adapter
                .read_position(request(&url), &key)
                .await
                .expect("a reading")
                .is_none()
        );
        // And discovery of the same node finds nothing.
        assert!(
            adapter
                .discover(request(&url))
                .await
                .expect("a discovery")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_reserve_the_contract_refuses_is_a_position_that_is_gone() {
        let (url, _) = node(|_to, _selector, _data| None).await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        assert!(
            adapter
                .read_position(request(&url), &key)
                .await
                .expect("a refusal is not an error")
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_balance_the_index_does_not_reproduce_is_marked_approximate() {
        // The aToken claims a scaled balance ten wei larger than the one
        // that gives the balance the contract reported.
        let honest = weth_node(USER_DATA, SCALED);
        let (url, _) = node(move |to, selector, calldata| {
            if selector == SEL_SCALED_BALANCE && to == A_TOKEN {
                return Some(word(2_000_000_000_000_000_010));
            }
            honest(to, selector, calldata)
        })
        .await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        let reading = adapter
            .read_position(request(&url), &key)
            .await
            .expect("a reading")
            .expect("a position");
        assert!(reading.approximate);
        assert_eq!(
            reading.legs[0].amount,
            amount("2.146517999950570683"),
            "the contract's own balance is what is reported"
        );
    }

    #[tokio::test]
    async fn a_stable_debt_is_neither_a_leg_nor_silently_dropped() {
        let (url, _) = node(weth_node(STABLE, SCALED)).await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        let reading = adapter
            .read_position(request(&url), &key)
            .await
            .expect("a reading")
            .expect("a position");
        assert_eq!(reading.legs.len(), 1, "no leg for stable debt");
        assert_eq!(reading.legs[0].direction, CryptoDirection::In);
        assert!(
            reading.approximate,
            "a non-zero stable debt is not the documented layout"
        );
    }

    #[tokio::test]
    async fn an_answer_of_the_wrong_shape_is_an_error_not_a_panic() {
        let (url, _) = node(|to, selector, _data| {
            let market = market(Chain::Ethereum).expect("a market");
            (to == market.data_provider && selector == SEL_USER_RESERVE_DATA).then(|| word(1))
        })
        .await;
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        assert!(adapter.read_position(request(&url), &key).await.is_err());
        let wrong_network = PositionKey::new(PROTOCOL, Chain::Base, WETH);
        assert!(
            adapter
                .read_position(request(&url), &wrong_network)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_health_factor_is_read_for_display_and_is_absent_without_debt() {
        for (fixture, expected) in [(ACCOUNT, Some(amount("1.245"))), (ACCOUNT_NO_DEBT, None)] {
            let market = market(Chain::Ethereum).expect("a market");
            let (url, _) = node(move |to, selector, _data| {
                (to == market.pool && selector == SEL_ACCOUNT_DATA).then(|| bytes(fixture))
            })
            .await;
            let adapter = AaveV3 {
                client: stub::client(),
            };
            assert_eq!(
                adapter.account_health(request(&url)).await.expect("health"),
                expected
            );
        }
    }

    #[tokio::test]
    async fn the_operations_are_not_readable_here_and_the_answer_is_empty() {
        let adapter = AaveV3 {
            client: stub::client(),
        };
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WETH);
        assert!(
            adapter
                .events(request("http://127.0.0.1:1"), &key)
                .await
                .expect("events")
                .is_empty()
        );
    }
}
