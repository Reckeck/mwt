//! The built-in list of known token contracts.
//!
//! A token is known by its network and contract, never by its symbol: a scam
//! token is free to call itself `USDC`. The list is deliberately short (the
//! stablecoins and wrapped majors people actually hold); anything else is
//! quarantined until the user confirms it, which is safe, merely one click
//! more.
//!
//! # Sources
//!
//! Every entry was checked on 2026-11-11 against a list fetched that day, by
//! chain id, address and symbol, and nothing is in the table that was not
//! found there:
//!
//! * EVM networks (Ethereum 1, Polygon 137, Arbitrum 42161, Optimism 10,
//!   Base 8453, BNB Chain 56, Avalanche 43114): the Uniswap default token
//!   list, <https://tokens.uniswap.org> (version 22.34.0, timestamp
//!   2026-11-06). The Arbitrum `0xfd08...cbb9` entry is Tether's token, which
//!   the list now names `USDT0` (the same contract, renamed by its issuer);
//!   that is why a symbol never identifies a token here.
//! * Solana: the `solana-labs/token-list` `solana.tokenlist.json`
//!   (<https://github.com/solana-labs/token-list>, archived but stable for
//!   these two mints), USDC and USDT.
//!
//! An address added later needs a source too: add it to this comment with it.

use crate::model::chain::Chain;
use crate::model::cryptotrack::TokenId;

/// `(chain, contract, symbol)`. EVM contracts are lowercase here, Solana
/// mints keep their case; comparison goes through [`TokenId::new`], which
/// applies the same normalization to the candidate.
const KNOWN: &[(Chain, &str, &str)] = &[
    (
        Chain::Ethereum,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "USDC",
    ),
    (
        Chain::Ethereum,
        "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "USDT",
    ),
    (
        Chain::Ethereum,
        "0x6b175474e89094c44da98b954eedeac495271d0f",
        "DAI",
    ),
    (
        Chain::Ethereum,
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        "WETH",
    ),
    (
        Chain::Ethereum,
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599",
        "WBTC",
    ),
    (
        Chain::Polygon,
        "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
        "USDC",
    ),
    (
        Chain::Polygon,
        "0xc2132d05d31c914a87c6611c10748aeb04b58e8f",
        "USDT",
    ),
    (
        Chain::Polygon,
        "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619",
        "WETH",
    ),
    (
        Chain::Arbitrum,
        "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
        "USDC",
    ),
    (
        Chain::Arbitrum,
        "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9",
        "USDT",
    ),
    (
        Chain::Arbitrum,
        "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",
        "WETH",
    ),
    (
        Chain::Optimism,
        "0x0b2c639c533813f4aa9d7837caf62653d097ff85",
        "USDC",
    ),
    (
        Chain::Optimism,
        "0x94b008aa00579c1307b0ef2c499ad98a8ce58e58",
        "USDT",
    ),
    (
        Chain::Optimism,
        "0x4200000000000000000000000000000000000006",
        "WETH",
    ),
    (
        Chain::Base,
        "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
        "USDC",
    ),
    (
        Chain::Base,
        "0x4200000000000000000000000000000000000006",
        "WETH",
    ),
    (
        Chain::Bsc,
        "0x55d398326f99059ff775485246999027b3197955",
        "USDT",
    ),
    (
        Chain::Bsc,
        "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d",
        "USDC",
    ),
    (
        Chain::Bsc,
        "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c",
        "WBNB",
    ),
    (
        Chain::Avalanche,
        "0xb97ef9ef8734c71904d8002f8b6bc66dd9c48a6e",
        "USDC",
    ),
    (
        Chain::Avalanche,
        "0x9702230a8ea53601f5cd2dc00fdbc13d4df4a8c7",
        "USDT",
    ),
    (
        Chain::Solana,
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "USDC",
    ),
    (
        Chain::Solana,
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "USDT",
    ),
];

/// Whether this exact contract on this network is on the built-in list.
pub fn is_known(token: &TokenId) -> bool {
    KNOWN
        .iter()
        .any(|(chain, contract, _)| TokenId::new(*chain, contract) == *token)
}

/// The built-in list's symbol for this exact contract, when it is listed.
pub fn symbol_of(token: &TokenId) -> Option<&'static str> {
    KNOWN
        .iter()
        .find(|(chain, contract, _)| TokenId::new(*chain, contract) == *token)
        .map(|(_, _, symbol)| *symbol)
}

/// Whether a symbol belongs to some known token (used only to warn that an
/// unknown contract imitates one; never to identify anything).
pub fn symbol_is_known(skeleton: &str) -> bool {
    KNOWN
        .iter()
        .any(|(_, _, symbol)| symbol.eq_ignore_ascii_case(skeleton))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_contract_is_normalized_the_way_lookups_are() {
        for (chain, contract, _) in KNOWN {
            assert!(is_known(&TokenId::new(*chain, contract)), "{contract}");
        }
    }

    #[test]
    fn an_evm_contract_in_other_case_is_the_same_known_token() {
        let upper = TokenId::new(
            Chain::Ethereum,
            "0xA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
        );
        assert!(is_known(&upper));
    }

    #[test]
    fn a_solana_mint_in_other_case_is_a_different_token() {
        let lowered = TokenId::new(
            Chain::Solana,
            "epjfwdd5aufqsseqm2qn1xzybapc8g4wegkgkzwytdt1v",
        );
        assert!(!is_known(&lowered));
    }

    #[test]
    fn the_same_contract_on_another_network_is_not_known() {
        let wrong = TokenId::new(Chain::Bsc, "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        assert!(!is_known(&wrong));
    }
}
