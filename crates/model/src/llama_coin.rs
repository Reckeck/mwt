//! DeFiLlama's coin identifiers: the pure mapping from a crypto asset to the name a price request
//! uses. It sits in the model so the crypto module can ask "can this asset be quoted at all"
//! without naming the pricing client.

use crate::{Chain, CryptoAsset};

/// DeFiLlama's name for a network; `None` for one it does not price by contract.
fn llama_chain(chain: Chain) -> Option<&'static str> {
    Some(match chain {
        Chain::Ethereum => "ethereum",
        Chain::Bsc => "bsc",
        Chain::Polygon => "polygon",
        Chain::Arbitrum => "arbitrum",
        Chain::Optimism => "optimism",
        Chain::Avalanche => "avax",
        Chain::Base => "base",
        Chain::Solana => "solana",
        Chain::Tron => "tron",
        _ => return None,
    })
}

/// The coin identifier of a request: `<chain>:<contract>` for a token,
/// `coingecko:<id>` for a network's own coin. `None` when the asset has no
/// identifier DeFiLlama understands.
pub fn llama_coin(asset: &CryptoAsset) -> Option<String> {
    match asset {
        CryptoAsset::Native { chain } => chain
            .info()
            .cg_coin_id
            .map(|coin_id| format!("coingecko:{coin_id}")),
        CryptoAsset::Token(token) => {
            let chain = llama_chain(token.chain)?;
            (!token.contract.is_empty()).then(|| format!("{chain}:{}", token.contract))
        }
    }
}
