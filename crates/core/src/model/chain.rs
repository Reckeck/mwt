use serde::{Deserialize, Serialize};

/// Supported blockchain networks + a pseudo `Fiat` variant.
///
/// Metadata (CoinGecko identifiers and explorer templates) lives in
/// `ChainInfo`. Adding a network = adding one variant here + one match arm.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum Chain {
    Bitcoin,
    Ethereum,
    Bsc,
    Solana,
    Polygon,
    Arbitrum,
    Optimism,
    Avalanche,
    Base,
    Tron,
    Litecoin,
    Dogecoin,
    Cardano,
    Xrp,
    Polkadot,
    Cosmos,
    Near,
    Ton,
    Algorand,
    #[default]
    Fiat,
}

/// Static metadata about a network.
#[derive(Debug, Clone)]
pub struct ChainInfo {
    pub chain: Chain,
    pub label: &'static str,
    /// CoinGecko `asset_platform_id` used for `token_price` lookups.
    pub cg_platform: Option<&'static str>,
    /// CoinGecko coin id for the native gas asset (if any).
    pub cg_coin_id: Option<&'static str>,
    /// Explorer URL template; `{tx}` is replaced with the tx hash.
    pub explorer: Option<&'static str>,
}

impl Chain {
    pub fn info(self) -> ChainInfo {
        ChainInfo {
            chain: self,
            label: match self {
                Chain::Bitcoin => "Bitcoin",
                Chain::Ethereum => "Ethereum",
                Chain::Bsc => "BNB Smart Chain",
                Chain::Solana => "Solana",
                Chain::Polygon => "Polygon",
                Chain::Arbitrum => "Arbitrum",
                Chain::Optimism => "Optimism",
                Chain::Avalanche => "Avalanche",
                Chain::Base => "Base",
                Chain::Tron => "Tron",
                Chain::Litecoin => "Litecoin",
                Chain::Dogecoin => "Dogecoin",
                Chain::Cardano => "Cardano",
                Chain::Xrp => "XRP Ledger",
                Chain::Polkadot => "Polkadot",
                Chain::Cosmos => "Cosmos",
                Chain::Near => "NEAR",
                Chain::Ton => "TON",
                Chain::Algorand => "Algorand",
                Chain::Fiat => "Fiat",
            },
            cg_platform: match self {
                Chain::Ethereum => Some("ethereum"),
                Chain::Bsc => Some("binance-smart-chain"),
                Chain::Solana => Some("solana"),
                Chain::Polygon => Some("polygon-pos"),
                Chain::Arbitrum => Some("arbitrum-one"),
                Chain::Optimism => Some("optimistic-ethereum"),
                Chain::Avalanche => Some("avalanche"),
                Chain::Base => Some("base"),
                Chain::Tron => Some("tron"),
                Chain::Algorand => Some("algorand"),
                _ => None,
            },
            // native gas assets that themselves have CoinGecko coin ids
            cg_coin_id: match self {
                Chain::Bitcoin => Some("bitcoin"),
                Chain::Ethereum => Some("ethereum"),
                Chain::Bsc => Some("binancecoin"),
                Chain::Solana => Some("solana"),
                Chain::Polygon => Some("matic-network"),
                Chain::Arbitrum => Some("arbitrum"),
                Chain::Optimism => Some("optimism"),
                Chain::Avalanche => Some("avalanche-2"),
                Chain::Base => Some("ethereum"), // ETH on Base
                Chain::Tron => Some("tron"),
                Chain::Litecoin => Some("litecoin"),
                Chain::Dogecoin => Some("dogecoin"),
                Chain::Cardano => Some("cardano"),
                Chain::Xrp => Some("ripple"),
                Chain::Polkadot => Some("polkadot"),
                Chain::Cosmos => Some("cosmos"),
                Chain::Near => Some("near"),
                Chain::Ton => Some("the-open-network"),
                Chain::Algorand => None,
                Chain::Fiat => None,
            },
            explorer: match self {
                Chain::Bitcoin => Some("https://mempool.space/tx/{tx}"),
                Chain::Ethereum => Some("https://etherscan.io/tx/{tx}"),
                Chain::Bsc => Some("https://bscscan.com/tx/{tx}"),
                Chain::Solana => Some("https://solscan.io/tx/{tx}"),
                Chain::Polygon => Some("https://polygonscan.com/tx/{tx}"),
                Chain::Arbitrum => Some("https://arbiscan.io/tx/{tx}"),
                Chain::Optimism => Some("https://optimistic.etherscan.io/tx/{tx}"),
                Chain::Avalanche => Some("https://snowtrace.io/tx/{tx}"),
                Chain::Base => Some("https://basescan.org/tx/{tx}"),
                Chain::Tron => Some("https://tronscan.org/#/transaction/{tx}"),
                Chain::Litecoin => Some("https://litecoinspace.org/tx/{tx}"),
                Chain::Dogecoin => Some("https://dogechain.info/tx/{tx}"),
                Chain::Cardano => Some("https://cexplorer.io/tx/{tx}"),
                Chain::Xrp => Some("https://bithomp.com/explorer/{tx}"),
                Chain::Polkadot => Some("https://polkadot.subscan.io/tx/{tx}"),
                Chain::Cosmos => Some("https://www.mintscan.io/transaction/{tx}"),
                Chain::Near => Some("https://nearblocks.io/txns/{tx}"),
                Chain::Ton => Some("https://tonscan.org/tx/{tx}"),
                Chain::Algorand => Some("https://algoexplorer.io/tx/{tx}"),
                Chain::Fiat => None,
            },
        }
    }

    pub fn label(self) -> &'static str {
        self.info().label
    }

    /// Explorer link for a tx hash, if supported.
    pub fn explorer_url(self, tx_hash: &str) -> Option<String> {
        self.info().explorer.map(|t| t.replace("{tx}", tx_hash))
    }
}

pub const ALL_CHAINS: &[Chain] = &[
    Chain::Bitcoin,
    Chain::Ethereum,
    Chain::Bsc,
    Chain::Solana,
    Chain::Polygon,
    Chain::Arbitrum,
    Chain::Optimism,
    Chain::Avalanche,
    Chain::Base,
    Chain::Tron,
    Chain::Litecoin,
    Chain::Dogecoin,
    Chain::Cardano,
    Chain::Xrp,
    Chain::Polkadot,
    Chain::Cosmos,
    Chain::Near,
    Chain::Ton,
    Chain::Algorand,
    Chain::Fiat,
];
