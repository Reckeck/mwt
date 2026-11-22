//! Balances from a node: `eth_getBalance` and `balanceOf`.
//!
//! One `eth_call` to Multicall3 `aggregate3` reads the native balance and up
//! to [`TOKENS_PER_CALL`] tokens at once, all at the same block. A node that
//! refuses it (no Multicall3, `eth_call` limits) is answered by a JSON-RPC
//! batch of plain calls. Either way every request is for ONE address, through
//! the shared client, so the rule "one subject per batch" holds.

use serde_json::{Value, json};

use super::abi::{self, Call3, MULTICALL3};
use super::blockscout::is_address;
use super::model::{BalanceSource, RawAsset, RawBalance, Rejected, SourceBatch};
use crate::chains;
use crate::ports::{ChainCall, ChainSource};
use mwt_base::error::{Error, Result};
use mwt_model::Chain;
use mwt_model::normalize_address;

/// Calls of one `aggregate3`; keeps the answer far below node limits.
pub const TOKENS_PER_CALL: usize = 100;

#[derive(Debug, Clone, Copy)]
pub struct BalanceRequest<'req> {
    pub chain: Chain,
    pub address: &'req str,
    /// Tokens to read (`RawAsset::Token`); anything else is ignored. The
    /// native balance is always read.
    pub tokens: &'req [RawAsset],
    /// A JSON-RPC endpoint instead of the presets of the network.
    pub rpc: Option<&'req str>,
}

/// Read the native and token balances of an address from a node.
pub async fn read_balances(
    rpc: &impl ChainSource,
    request: &BalanceRequest<'_>,
) -> Result<SourceBatch> {
    let row = chains::evm_chain(request.chain).ok_or(Error::Unsupported)?;
    let address = normalize_address(request.chain, request.address);
    if !is_address(&address) {
        return Err(Error::InvalidInput("not an EVM address".into()));
    }
    let tokens: Vec<&RawAsset> = request
        .tokens
        .iter()
        .filter(|asset| matches!(asset, RawAsset::Token { .. }))
        .collect();
    let native = RawAsset::Native {
        decimals: row.native.decimals,
    };

    let urls: Vec<&str> = match request.rpc {
        Some(url) => vec![url],
        None => row.rpc.to_vec(),
    };
    let mut last = Error::Network("no RPC endpoint".into());
    for url in urls {
        let attempt = match multicall(rpc, url, &address, &native, &tokens).await {
            Ok(batch) => Ok(batch),
            // The node may simply not have Multicall3: plain calls next.
            Err(_) => plain_batch(rpc, url, &address, &native, &tokens).await,
        };
        match attempt {
            Ok(batch) => return Ok(batch),
            Err(err) => last = err,
        }
    }
    Err(last)
}

async fn multicall(
    rpc: &impl ChainSource,
    url: &str,
    address: &str,
    native: &RawAsset,
    tokens: &[&RawAsset],
) -> Result<SourceBatch> {
    let mut batch = SourceBatch::default();
    let mut native_pending = true;
    let chunks: Vec<&[&RawAsset]> = if tokens.is_empty() {
        vec![&[]]
    } else {
        tokens.chunks(TOKENS_PER_CALL).collect()
    };
    for chunk in chunks {
        let mut calls = Vec::with_capacity(chunk.len() + 1);
        if native_pending {
            calls.push(Call3 {
                target: MULTICALL3.to_lowercase(),
                data: abi::get_eth_balance_data(address)?,
            });
        }
        for asset in chunk {
            calls.push(Call3 {
                target: contract_of(asset)?.to_owned(),
                data: abi::balance_of_data(address)?,
            });
        }
        let calldata = abi::aggregate3_calldata(&calls)?;
        let answer = rpc
            .call(
                url,
                address,
                "eth_call",
                json!([{"to": MULTICALL3, "data": calldata}, "latest"]),
            )
            .await?;
        let text = answer
            .as_str()
            .ok_or_else(|| Error::Network("eth_call answer is not text".into()))?;
        let mut results = abi::decode_aggregate3(text, calls.len())?.into_iter();
        if native_pending {
            let data = results
                .next()
                .flatten()
                .ok_or_else(|| Error::Network("native balance call failed".into()))?;
            batch
                .balances
                .push(node_balance(native, abi::decode_uint_word(&data)?));
            native_pending = false;
        }
        for (asset, data) in chunk.iter().zip(results) {
            match data.map(|data| abi::decode_uint_word(&data)) {
                Some(Ok(value)) => batch.balances.push(node_balance(asset, value)),
                _ => batch.rejected.push(unreadable(asset)),
            }
        }
    }
    Ok(batch)
}

async fn plain_batch(
    rpc: &impl ChainSource,
    url: &str,
    address: &str,
    native: &RawAsset,
    tokens: &[&RawAsset],
) -> Result<SourceBatch> {
    let mut calls = vec![ChainCall::new("eth_getBalance", json!([address, "latest"]))];
    for asset in tokens {
        let data = abi::balance_of_calldata(address)?;
        calls.push(ChainCall::new(
            "eth_call",
            json!([{"to": contract_of(asset)?, "data": data}, "latest"]),
        ));
    }
    let answers = rpc.batch(url, address, &calls).await?;
    let mut answers = answers.into_iter();
    let first = answers
        .next()
        .ok_or_else(|| Error::Network("empty batch answer".into()))??;
    let mut batch = SourceBatch::default();
    batch
        .balances
        .push(node_balance(native, hex_quantity(&first)?));
    for (asset, answer) in tokens.iter().zip(answers) {
        match answer.and_then(|value| hex_word(&value)) {
            Ok(value) => batch.balances.push(node_balance(asset, value)),
            Err(_) => batch.rejected.push(unreadable(asset)),
        }
    }
    Ok(batch)
}

fn hex_quantity(value: &Value) -> Result<super::uint::Uint256> {
    super::uint::Uint256::from_hex(
        value
            .as_str()
            .ok_or_else(|| Error::Network("quantity is not text".into()))?,
    )
}

/// A `balanceOf` answer: exactly one 32-byte word.
fn hex_word(value: &Value) -> Result<super::uint::Uint256> {
    let text = value
        .as_str()
        .ok_or_else(|| Error::Network("answer is not text".into()))?;
    if text.len() != 66 {
        return Err(Error::Network("balanceOf did not return one word".into()));
    }
    super::uint::Uint256::from_hex(text)
}

fn contract_of(asset: &RawAsset) -> Result<&str> {
    match asset {
        RawAsset::Token { contract, .. } if is_address(contract) => Ok(contract),
        _ => Err(Error::InvalidInput("token has no valid contract".into())),
    }
}

fn node_balance(asset: &RawAsset, value: super::uint::Uint256) -> RawBalance {
    RawBalance {
        asset: asset.clone(),
        value,
        source: BalanceSource::Node,
    }
}

fn unreadable(asset: &RawAsset) -> Rejected {
    Rejected {
        subject: match asset {
            RawAsset::Token { contract, .. } => Some(contract.clone()),
            RawAsset::Native { .. } => None,
        },
        reason: "balance could not be read".into(),
    }
}
