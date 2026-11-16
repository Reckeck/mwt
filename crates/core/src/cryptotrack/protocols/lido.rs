//! Lido: stETH and wstETH (spec D4).
//!
//! At most two positions per address and network: the rebasing stETH and the
//! wrapped wstETH. Discovery is a balance check, the read is the balance and,
//! for wstETH on Ethereum, `stEthPerToken()`.
//!
//! Every address and selector below is from
//! `trade-app.crypto.D-protocol-facts.md` (Lido part, sections 8-11) and from
//! nowhere else.
//!
//! # A rebase is not an incoming transfer
//!
//! stETH grows by changing the global share rate, not by moving coins: the
//! contract emits **no** `Transfer` for a holder when it rebases (facts,
//! section 10, `StETH.sol`). So [`ProtocolAdapter::events`] has nothing to
//! report for growth, and the growth reaches the module as the difference of
//! two readings. [`split_period`] is the shares-based decomposition the facts
//! document gives: the yield is `shares_start * (rate_end - rate_start)` and
//! the real flow is `(shares_end - shares_start) * rate_end`, so a balance
//! that only grew by a rebase has a zero flow and can never look like a
//! transfer. Nobody derives an incoming amount from a balance delta.
//!
//! # What each network answers
//!
//! | network | positions | leg |
//! |---|---|---|
//! | Ethereum | stETH, wstETH | stETH balance; wstETH balance converted to stETH through `stEthPerToken()` |
//! | Optimism | rebasing stETH (official bridged), bridged wstETH | stETH balance; wstETH balance **as wstETH** |
//! | Arbitrum, Base, BNB | bridged wstETH | wstETH balance **as wstETH** |
//! | Polygon | none ([`Lido::supports`] is `false`) | - |
//!
//! The bridged wstETH is a plain ERC-20: `stEthPerToken()` and `sharesOf()`
//! revert there (facts, section 8), so the rate cannot be read locally. The
//! decision: the adapter never asks, never guesses a rate and never converts.
//! The leg stays in wstETH, the quantity the wallet really holds, and the
//! price of wstETH is the price lookup's business. Skipping the call (rather
//! than making it and handling the revert) also saves a request per read.

use crate::Amount;
use crate::cryptotrack::rpc::RpcClient;
use crate::cryptotrack::sources::evm::abi;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, TokenId};
use crate::model::vault::now_iso;

use super::{
    AdapterFuture, FeeReading, Node, PositionKey, PositionKind, PositionReading, PositionStatus,
    ProtocolAdapter, ProtocolRequest, U256,
};

/// The protocol id a position of this protocol stores.
pub const PROTOCOL: &str = "lido";

/// `balanceOf(address)`: ERC-20; stETH source `0.4.24/StETH.sol`, facts
/// section 9.
const BALANCE_OF: &str = "70a08231";
/// `stEthPerToken()` of wstETH, `0.6.12/WstETH.sol`, facts section 9. 1e18-scaled.
const ST_ETH_PER_TOKEN: &str = "035faf82";

/// Both tokens have 18 decimals (facts, section 11).
const DECIMALS: u8 = 18;

/// What a token is, which decides how it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// Rebasing stETH: the balance already includes the yield.
    Steth,
    /// wstETH on Ethereum: `stEthPerToken()` can be read from the token.
    WstethWithRate,
    /// Bridged wstETH: a plain ERC-20 with no local rate.
    WstethNoRate,
}

struct Token {
    chain: Chain,
    contract: &'static str,
    form: Form,
}

/// Facts section 8. Polygon is deliberately absent: its wstETH address is
/// listed under "Unverified" (not on Lido's own page), so it is not
/// hardcoded and the adapter does not support Polygon.
const TOKENS: &[Token] = &[
    Token {
        chain: Chain::Ethereum,
        contract: "0xae7ab96520de3a18e5e111b5eaab095312d7fe84",
        form: Form::Steth,
    },
    Token {
        chain: Chain::Ethereum,
        contract: "0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0",
        form: Form::WstethWithRate,
    },
    // Optimism: the official rebasing stETH (`ERC20RebasableBridgedPermit`)
    // and the bridged wstETH.
    Token {
        chain: Chain::Optimism,
        contract: "0x76a50b8c7349ccddb7578c6627e79b5d99d24138",
        form: Form::Steth,
    },
    Token {
        chain: Chain::Optimism,
        contract: "0x1f32b1c2345538c0c6f582fcb022739c4a194ebb",
        form: Form::WstethNoRate,
    },
    Token {
        chain: Chain::Arbitrum,
        contract: "0x5979d7b546e38e414f7e9822514be443a4800529",
        form: Form::WstethNoRate,
    },
    Token {
        chain: Chain::Base,
        contract: "0xc1cba3fcea344f92d9239c08c0568f6f2f0ee452",
        form: Form::WstethNoRate,
    },
    Token {
        chain: Chain::Bsc,
        contract: "0x26c5e01524d2e6280a48f2c50ff6de7e52e9611c",
        form: Form::WstethNoRate,
    },
];

fn tokens_on(chain: Chain) -> impl Iterator<Item = &'static Token> {
    TOKENS.iter().filter(move |token| token.chain == chain)
}

fn asset_of(token: &Token) -> CryptoAsset {
    CryptoAsset::Token(TokenId::new(token.chain, token.contract))
}

/// stETH and wstETH on `chain` (see [`super::receipt_tokens`]). Both sit in
/// the wallet and a node reports them; naming them keeps the stake from being
/// counted twice. Empty on a network Lido is not read on, Polygon included.
pub fn receipt_tokens(chain: Chain) -> Vec<CryptoAsset> {
    tokens_on(chain).map(asset_of).collect()
}

fn one_ether() -> U256 {
    U256::from_u128(1_000_000_000_000_000_000)
}

/// `balance * rate / 1e18` rounded down, the forward form of the facts
/// (section 11): stETH worth of a wstETH balance, or of any share amount.
pub fn scale_by_rate(balance: U256, rate: U256) -> Result<U256> {
    balance
        .mul_div_floor(rate, one_ether())
        .ok_or_else(|| Error::Network("stETH value does not fit a word".into()))
}

/// `factor * (end - start) / 1e18` as a signed [`Amount`] in whole units (18
/// places), rounded toward zero. Negative when `end < start`: the share rate
/// falls on a negative rebase, so a period's yield may be a loss.
fn signed_scaled_delta(factor: U256, end: U256, start: U256) -> Result<Amount> {
    let (magnitude, negative) = if end >= start {
        (end.wrapping_sub(start), false)
    } else {
        (start.wrapping_sub(end), true)
    };
    let scaled = scale_by_rate(factor, magnitude)?.to_amount(DECIMALS)?;
    Ok(if negative { -scaled } else { scaled })
}

/// What changed over a period, split the way the facts document says
/// (section 10 for stETH, section 11 for wstETH): the rebase and the flows.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodSplit {
    /// Growth from the rate alone, in stETH. Negative after a negative
    /// rebase.
    pub yield_amount: Amount,
    /// Real transfers in (positive) or out (negative), in stETH at the end
    /// rate. Zero for a balance that only grew by a rebase.
    pub principal_flow: Amount,
}

/// The shares-based decomposition: `shares_*` are `sharesOf` for stETH (or the
/// wstETH balance, one wstETH being one share), `rate_*` is the 1e18-scaled
/// stETH per share (`stEthPerToken()`, equal to `getPooledEthByShares(1e18)`).
///
/// ```text
/// yield          = shares_start * (rate_end - rate_start)
/// principal_flow = (shares_end - shares_start) * rate_end
/// ```
///
/// Only shares count as flows: the balance of stETH is never compared, which
/// is why a rebase cannot become a transfer.
pub fn split_period(
    shares_start: U256,
    shares_end: U256,
    rate_start: U256,
    rate_end: U256,
) -> Result<PeriodSplit> {
    Ok(PeriodSplit {
        yield_amount: signed_scaled_delta(shares_start, rate_end, rate_start)?,
        principal_flow: signed_scaled_delta(rate_end, shares_end, shares_start)?,
    })
}

/// The Lido adapter. Keeps the shared client and builds a [`Node`] per
/// request.
pub struct Lido {
    client: RpcClient,
}

/// The adapter for [`super::Registry::builtin`]:
/// `lido::adapter(node)`.
pub fn adapter(node: RpcClient) -> Box<dyn ProtocolAdapter> {
    Box::new(Lido { client: node })
}

impl Lido {
    async fn balance(node: &Node<'_>, token: &Token) -> Result<U256> {
        let owner = abi::address_word(node.address())?;
        node.call(token.contract, BALANCE_OF, &[owner])
            .await?
            .u256(0)
    }

    async fn rate(node: &Node<'_>, token: &Token) -> Result<U256> {
        let rate = node
            .call(token.contract, ST_ETH_PER_TOKEN, &[])
            .await?
            .u256(0)?;
        if rate.is_zero() {
            return Err(Error::Network("stEthPerToken is zero".into()));
        }
        Ok(rate)
    }
}

impl ProtocolAdapter for Lido {
    fn protocol(&self) -> &'static str {
        PROTOCOL
    }

    /// Facts section 8: Ethereum, Optimism, Arbitrum, Base, BNB. Not Polygon
    /// (its wstETH address is unverified).
    fn supports(&self, chain: Chain) -> bool {
        tokens_on(chain).next().is_some()
    }

    fn discover<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
    ) -> AdapterFuture<'fut, Vec<PositionKey>> {
        Box::pin(async move {
            let node = Node::new(&self.client, &request)?;
            let mut found = Vec::new();
            for token in tokens_on(request.chain) {
                if !Self::balance(&node, token).await?.is_zero() {
                    found.push(PositionKey::new(PROTOCOL, request.chain, token.contract));
                }
            }
            Ok(found)
        })
    }

    fn read_position<'fut>(
        &'fut self,
        request: ProtocolRequest<'fut>,
        key: &'fut PositionKey,
    ) -> AdapterFuture<'fut, Option<PositionReading>> {
        Box::pin(async move {
            let Some(token) = tokens_on(request.chain).find(|token| token.contract == key.id)
            else {
                return Ok(None);
            };
            let node = Node::new(&self.client, &request)?;
            let balance = Self::balance(&node, token).await?;
            if balance.is_zero() {
                return Ok(None);
            }
            let (name, asset, amount) = match token.form {
                Form::Steth => ("Lido stETH", asset_of(token), balance.to_amount(DECIMALS)?),
                Form::WstethWithRate => {
                    let rate = Self::rate(&node, token).await?;
                    // The leg is what the wstETH is worth in stETH, so the
                    // stake is priced as stETH on the one network that can
                    // say the rate. Floor, as the contract does.
                    let steth = tokens_on(token.chain)
                        .find(|other| other.form == Form::Steth)
                        .ok_or(Error::Unsupported)?;
                    (
                        "Lido wstETH",
                        asset_of(steth),
                        scale_by_rate(balance, rate)?.to_amount(DECIMALS)?,
                    )
                }
                // No rate is readable here: the leg stays in wstETH, never
                // converted with a guessed rate (see the module doc).
                Form::WstethNoRate => {
                    ("Lido wstETH", asset_of(token), balance.to_amount(DECIMALS)?)
                }
            };
            Ok(Some(PositionReading {
                key: key.clone(),
                account_id: request.account_id.to_string(),
                name: name.to_string(),
                kind: PositionKind::Staking,
                status: PositionStatus::Open,
                legs: vec![CryptoLeg {
                    direction: CryptoDirection::In,
                    asset,
                    amount,
                    counterparty: None,
                }],
                // Staking yield accrues inside the balance (stETH) or the
                // rate (wstETH): nothing is held apart from the principal, so
                // the uncollected fees are a real zero, not a gap.
                fees: FeeReading::none_earned(),
                receipts: receipt_tokens(token.chain),
                approximate: false,
                opened_at: None,
                closed_at: None,
                read_at: now_iso(),
            }))
        })
    }

    /// Always empty here, on purpose. A rebase emits no `Transfer`, so it is
    /// not an event. The real operations (submit, wrap, unwrap, withdrawal)
    /// would come from the address's history, but this layer has only
    /// [`Node::logs`]: a log carries no block time and no gas, and the node
    /// layer has no block or receipt call to add them. Inventing a path would
    /// mean guessing a time and a fee, so the history of a Lido position is
    /// left to the address history the module already fetches.
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

    const OWNER: &str = "0x1111111111111111111111111111111111111111";
    const STETH: &str = "0xae7ab96520de3a18e5e111b5eaab095312d7fe84";
    const WSTETH: &str = "0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0";
    const OPTIMISM_STETH: &str = "0x76a50b8c7349ccddb7578c6627e79b5d99d24138";
    const ARBITRUM_WSTETH: &str = "0x5979d7b546e38e414f7e9822514be443a4800529";
    /// `stEthPerToken()` of the fixture: 1.249306133740130361 stETH per wstETH.
    const RATE: &str = "1249306133740130361";

    // Fixtures: the rate is the live value documented in the facts file
    // (section 9), rebuilt as the one-word answer; the balances are
    // hand-built from the documented layout. No live capture was available.
    fn fixture(text: &str) -> String {
        let value: Value = serde_json::from_str(text).expect("a fixture");
        value["result"].as_str().expect("a result").to_string()
    }
    fn rate_word() -> String {
        fixture(include_str!("fixtures/lido/stEthPerToken.ethereum.json"))
    }
    fn two_wsteth() -> String {
        fixture(include_str!("fixtures/lido/balanceOf.wsteth.two.json"))
    }
    fn one_and_a_half_steth() -> String {
        fixture(include_str!(
            "fixtures/lido/balanceOf.steth.one-and-a-half.json"
        ))
    }
    fn zero() -> String {
        fixture(include_str!("fixtures/lido/balanceOf.zero.json"))
    }

    /// A node that answers `balanceOf` per token contract (zero when not
    /// listed) and `stEthPerToken` with the fixture, and records which
    /// selectors it was asked.
    async fn node(balances: Vec<(&'static str, String)>) -> (String, stub::Seen) {
        let rate = rate_word();
        stub::serve(move |method, params, _| {
            assert_eq!(method, "eth_call");
            let target = params[0]["to"].as_str().unwrap_or_default().to_lowercase();
            let data = params[0]["data"].as_str().unwrap_or_default();
            if data.starts_with("0x70a08231") {
                let found = balances.iter().find(|(contract, _)| *contract == target);
                return json!(found.map_or_else(zero, |(_, word)| word.clone()));
            }
            if data.starts_with("0x035faf82") && target == WSTETH {
                return json!(rate);
            }
            Value::Null
        })
        .await
    }

    fn request<'req>(chain: Chain, url: &'req str) -> ProtocolRequest<'req> {
        ProtocolRequest {
            chain,
            address: OWNER,
            account_id: "cacc-1",
            since: None,
            rpc: Some(url),
            memory: None,
        }
    }

    fn adapter_under_test() -> Lido {
        Lido {
            client: stub::client(),
        }
    }

    fn wei(text: &str) -> U256 {
        U256::from_decimal(text).expect("a number")
    }

    #[test]
    fn the_adapter_reads_the_five_networks_and_not_polygon() {
        let lido = adapter_under_test();
        for chain in [
            Chain::Ethereum,
            Chain::Optimism,
            Chain::Arbitrum,
            Chain::Base,
            Chain::Bsc,
        ] {
            assert!(lido.supports(chain), "{chain:?}");
            assert!(!receipt_tokens(chain).is_empty(), "{chain:?}");
        }
        assert!(!lido.supports(Chain::Polygon));
        assert!(receipt_tokens(Chain::Polygon).is_empty());
        assert_eq!(receipt_tokens(Chain::Ethereum).len(), 2);
        assert_eq!(receipt_tokens(Chain::Arbitrum).len(), 1);
        assert_eq!(receipt_tokens(Chain::Optimism).len(), 2);
    }

    #[tokio::test]
    async fn discovery_lists_only_the_tokens_the_address_holds() {
        let (url, _) = node(vec![(WSTETH, two_wsteth())]).await;
        let keys = adapter_under_test()
            .discover(request(Chain::Ethereum, &url))
            .await
            .expect("a discovery");
        assert_eq!(
            keys,
            vec![PositionKey::new(PROTOCOL, Chain::Ethereum, WSTETH)]
        );

        let (url, _) = node(vec![
            (WSTETH, two_wsteth()),
            (STETH, one_and_a_half_steth()),
        ])
        .await;
        let keys = adapter_under_test()
            .discover(request(Chain::Ethereum, &url))
            .await
            .expect("a discovery");
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn a_steth_position_is_its_balance_with_no_fees_apart() {
        let (url, _) = node(vec![(STETH, one_and_a_half_steth())]).await;
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, STETH);
        let reading = adapter_under_test()
            .read_position(request(Chain::Ethereum, &url), &key)
            .await
            .expect("a reading")
            .expect("the position is there");
        assert_eq!(reading.kind, PositionKind::Staking);
        assert_eq!(reading.legs.len(), 1);
        assert_eq!(reading.legs[0].direction, CryptoDirection::In);
        assert_eq!(
            reading.legs[0].amount,
            Amount::parse("1.5").expect("amount")
        );
        assert_eq!(reading.fees, FeeReading::none_earned());
        assert!(!reading.approximate);
        assert_eq!(reading.receipts, receipt_tokens(Chain::Ethereum));
    }

    /// Hand calculation: 2 wstETH * 1.249306133740130361 = 2.498612267480260722 stETH.
    #[tokio::test]
    async fn a_wsteth_position_is_valued_in_steth_through_the_rate() {
        let (url, seen) = node(vec![(WSTETH, two_wsteth())]).await;
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WSTETH);
        let reading = adapter_under_test()
            .read_position(request(Chain::Ethereum, &url), &key)
            .await
            .expect("a reading")
            .expect("the position is there");
        assert_eq!(
            reading.legs[0].amount,
            Amount::parse("2.498612267480260722").expect("amount")
        );
        assert_eq!(
            reading.legs[0].asset,
            CryptoAsset::Token(TokenId::new(Chain::Ethereum, STETH))
        );
        let asked = seen.lock().expect("log").len();
        assert_eq!(asked, 2, "one balance and one rate");
    }

    #[tokio::test]
    async fn a_bridged_wsteth_has_no_rate_and_none_is_asked_or_guessed() {
        let (url, seen) = node(vec![(ARBITRUM_WSTETH, two_wsteth())]).await;
        let key = PositionKey::new(PROTOCOL, Chain::Arbitrum, ARBITRUM_WSTETH);
        let reading = adapter_under_test()
            .read_position(request(Chain::Arbitrum, &url), &key)
            .await
            .expect("a reading")
            .expect("the position is there");
        // The wstETH the wallet holds, unconverted.
        assert_eq!(reading.legs[0].amount, Amount::parse("2").expect("amount"));
        assert_eq!(
            reading.legs[0].asset,
            CryptoAsset::Token(TokenId::new(Chain::Arbitrum, ARBITRUM_WSTETH))
        );
        let bodies = seen.lock().expect("log").clone();
        assert!(
            !bodies
                .iter()
                .any(|body| body.to_string().contains("035faf82")),
            "stEthPerToken reverts there and must not be called"
        );
    }

    #[tokio::test]
    async fn optimism_reads_the_rebasing_steth_by_its_balance() {
        let (url, _) = node(vec![(OPTIMISM_STETH, one_and_a_half_steth())]).await;
        let key = PositionKey::new(PROTOCOL, Chain::Optimism, OPTIMISM_STETH);
        let reading = adapter_under_test()
            .read_position(request(Chain::Optimism, &url), &key)
            .await
            .expect("a reading")
            .expect("the position is there");
        assert_eq!(
            reading.legs[0].amount,
            Amount::parse("1.5").expect("amount")
        );
    }

    #[tokio::test]
    async fn a_position_with_no_balance_is_gone_and_a_foreign_key_is_none() {
        let (url, _) = node(vec![]).await;
        let lido = adapter_under_test();
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, WSTETH);
        assert!(
            lido.read_position(request(Chain::Ethereum, &url), &key)
                .await
                .expect("a reading")
                .is_none()
        );
        let foreign = PositionKey::new(PROTOCOL, Chain::Ethereum, "0xdead");
        assert!(
            lido.read_position(request(Chain::Ethereum, &url), &foreign)
                .await
                .expect("a reading")
                .is_none()
        );
    }

    #[tokio::test]
    async fn an_answer_of_the_wrong_shape_is_an_error_not_a_panic() {
        let (url, _) = stub::serve(|_, _, _| json!("0x")).await;
        let result = adapter_under_test()
            .discover(request(Chain::Ethereum, &url))
            .await;
        assert!(result.is_err());
    }

    /// The acceptance criterion: growth of the stETH balance by a rebase is
    /// not a flow. Shares are unchanged, only the rate moved.
    #[test]
    fn a_steth_rebase_is_yield_and_never_a_transfer() {
        let shares = wei("2000000000000000000");
        let split = split_period(
            shares,
            shares,
            wei(RATE),
            wei("1249506133740130361"), // +0.0002 stETH per share
        )
        .expect("a split");
        assert_eq!(split.principal_flow, Amount::parse("0").expect("zero"));
        // Hand: 2e18 * 2e14 / 1e18 = 4e14 wei = 0.0004 stETH.
        assert_eq!(split.yield_amount, Amount::parse("0.0004").expect("amount"));
    }

    #[tokio::test]
    async fn a_rebase_creates_no_event() {
        let (url, _) = node(vec![(STETH, one_and_a_half_steth())]).await;
        let key = PositionKey::new(PROTOCOL, Chain::Ethereum, STETH);
        let events = adapter_under_test()
            .events(request(Chain::Ethereum, &url), &key)
            .await
            .expect("events");
        assert!(events.is_empty());
    }

    /// The rate can fall on a negative rebase: the yield is negative.
    #[test]
    fn the_yield_of_a_period_may_be_negative() {
        let split = split_period(
            wei("1000000000000000000"),
            wei("1000000000000000000"),
            wei(RATE),
            wei("1249206133740130361"), // -0.0001 per share... 1.249206 vs 1.249306
        )
        .expect("a split");
        assert!(split.yield_amount.is_negative());
        // Hand: 1e18 * 1e14 / 1e18 = 1e14 wei lost = 0.0001 stETH.
        assert_eq!(
            split.yield_amount,
            Amount::parse("-0.0001").expect("amount")
        );
        assert_eq!(split.principal_flow, Amount::parse("0").expect("zero"));
    }

    /// Flows and yield together, checked by hand: 1 wstETH grows to 3 while
    /// the rate rises 1.249306133740130361 -> 1.249506133740130361.
    /// yield = 1e18 * 2e14 / 1e18 = 0.0002; flow = 2e18 * 1249506133740130361 / 1e18
    /// = 2.499012267480260722; the two add up to the change of the stETH
    /// value: 3 * 1.249506133740130361 - 1 * 1.249306133740130361
    /// = 3.748518401220391083 - 1.249306133740130361 = 2.499212267480260722.
    #[test]
    fn a_period_with_a_deposit_splits_into_yield_and_flow() {
        let split = split_period(
            wei("1000000000000000000"),
            wei("3000000000000000000"),
            wei(RATE),
            wei("1249506133740130361"),
        )
        .expect("a split");
        assert_eq!(split.yield_amount, Amount::parse("0.0002").expect("amount"));
        assert_eq!(
            split.principal_flow,
            Amount::parse("2.499012267480260722").expect("amount")
        );
        assert_eq!(
            split.yield_amount + split.principal_flow,
            Amount::parse("2.499212267480260722").expect("amount")
        );
    }

    #[test]
    fn a_withdrawal_is_a_negative_flow() {
        let split = split_period(
            wei("3000000000000000000"),
            wei("1000000000000000000"),
            wei(RATE),
            wei(RATE),
        )
        .expect("a split");
        assert_eq!(split.yield_amount, Amount::parse("0").expect("zero"));
        assert!(split.principal_flow.is_negative());
    }

    #[test]
    fn a_value_that_overflows_a_word_is_an_error() {
        assert!(scale_by_rate(U256::MAX, U256::MAX).is_err());
    }
}
