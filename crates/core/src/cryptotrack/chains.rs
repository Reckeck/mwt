//! The EVM networks the crypto module reads: chain id, native coin, preset
//! RPC endpoints, preset Blockscout instances and explorer link templates.
//!
//! Every value below was taken from a published source and not from memory
//! (stage 2, spec E1). Where a source could not be found the field is empty
//! rather than guessed.
//!
//! Sources (fetched 2026-11-11):
//! * chain id, native coin, public RPC and explorers: the ethereum-lists
//!   registry, `_data/chains/eip155-<id>.json`
//!   (<https://github.com/ethereum-lists/chains>), checked against
//!   <https://docs.arbitrum.io/for-devs/dev-tools-and-resources/chain-info>
//!   (Arbitrum One 42161), <https://docs.optimism.io/superchain/networks>
//!   (OP Mainnet 10), <https://docs.base.org/base-chain/quickstart/connecting-to-base>
//!   (Base 8453), <https://docs.polygon.technology/pos/reference/rpc-endpoints/>
//!   (Polygon 137, POL), <https://docs.bnbchain.org/bnb-smart-chain/developers/json_rpc/json-rpc-endpoint/>
//!   (BSC 56).
//! * Blockscout instances: the Chainscout registry
//!   (<https://github.com/blockscout/chainscout>, `data/chains.json`) and the
//!   Arbitrum page above. BNB Smart Chain has no entry there, so it has none.
//! * Explorer paths `/tx/<hash>` and `/address/<address>`: EIP-3091,
//!   <https://eips.ethereum.org/EIPS/eip-3091>.
//!
//! Public RPC endpoints are rate limited and carry no guarantees; they are a
//! starting point and the user can set their own (spec E17). Entries that need
//! an API key in the URL are left out.

use crate::model::chain::Chain;

/// The coin a network pays gas in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCoin {
    pub symbol: &'static str,
    pub name: &'static str,
    pub decimals: u8,
}

/// Static facts about one EVM network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvmChain {
    pub chain: Chain,
    /// EIP-155 chain id.
    pub chain_id: u64,
    pub native: NativeCoin,
    /// Preset JSON-RPC endpoints, https only, no key in the URL.
    pub rpc: &'static [&'static str],
    /// Preset Blockscout instances (base URLs; the API is `<base>/api`).
    pub blockscout: &'static [&'static str],
    /// Base URL of the preset explorer (EIP-3091 paths).
    pub explorer: &'static str,
}

const ETHER: NativeCoin = NativeCoin {
    symbol: "ETH",
    name: "Ether",
    decimals: 18,
};

static CHAINS: [EvmChain; 6] = [
    EvmChain {
        chain: Chain::Ethereum,
        chain_id: 1,
        native: ETHER,
        rpc: &[
            "https://ethereum-rpc.publicnode.com",
            "https://eth.drpc.org",
        ],
        blockscout: &["https://eth.blockscout.com"],
        explorer: "https://etherscan.io",
    },
    EvmChain {
        chain: Chain::Arbitrum,
        chain_id: 42161,
        native: ETHER,
        rpc: &[
            "https://arb1.arbitrum.io/rpc",
            "https://arbitrum-one-rpc.publicnode.com",
        ],
        blockscout: &["https://arbitrum.blockscout.com"],
        explorer: "https://arbiscan.io",
    },
    EvmChain {
        chain: Chain::Optimism,
        chain_id: 10,
        native: ETHER,
        rpc: &[
            "https://mainnet.optimism.io",
            "https://optimism-rpc.publicnode.com",
        ],
        blockscout: &["https://explorer.optimism.io"],
        explorer: "https://optimistic.etherscan.io",
    },
    EvmChain {
        chain: Chain::Base,
        chain_id: 8453,
        native: ETHER,
        rpc: &[
            "https://mainnet.base.org",
            "https://base-rpc.publicnode.com",
        ],
        blockscout: &["https://base.blockscout.com"],
        explorer: "https://basescan.org",
    },
    EvmChain {
        chain: Chain::Polygon,
        chain_id: 137,
        native: NativeCoin {
            symbol: "POL",
            name: "POL",
            decimals: 18,
        },
        rpc: &[
            "https://polygon.drpc.org",
            "https://polygon-bor-rpc.publicnode.com",
        ],
        blockscout: &["https://polygon.blockscout.com"],
        explorer: "https://polygonscan.com",
    },
    EvmChain {
        // `Chain::Bsc` is BNB Smart Chain. The public endpoints disable
        // `eth_getLogs` (BNB Chain docs), so a log-based source must use a
        // user-set endpoint here.
        chain: Chain::Bsc,
        chain_id: 56,
        native: NativeCoin {
            symbol: "BNB",
            name: "BNB Chain Native Token",
            decimals: 18,
        },
        rpc: &[
            "https://bsc-dataseed.bnbchain.org",
            "https://bsc-rpc.publicnode.com",
        ],
        blockscout: &[],
        explorer: "https://bscscan.com",
    },
];

/// Every EVM network of the table.
pub fn all() -> &'static [EvmChain] {
    &CHAINS
}

/// The table row of a network, `None` for networks the EVM engine does not read.
pub fn evm_chain(chain: Chain) -> Option<&'static EvmChain> {
    CHAINS.iter().find(|row| row.chain == chain)
}

/// The network with that EIP-155 chain id.
pub fn by_chain_id(chain_id: u64) -> Option<&'static EvmChain> {
    CHAINS.iter().find(|row| row.chain_id == chain_id)
}

impl EvmChain {
    /// Link template of a transaction page on the preset explorer.
    pub fn tx_url(&self, hash: &str) -> String {
        format!("{}/tx/{hash}", self.explorer)
    }

    /// Link template of an address page on the preset explorer.
    pub fn address_url(&self, address: &str) -> String {
        format!("{}/address/{address}", self.explorer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_the_six_networks_with_documented_ids() {
        let ids: Vec<(Chain, u64, &str)> = all()
            .iter()
            .map(|row| (row.chain, row.chain_id, row.native.symbol))
            .collect();
        assert_eq!(
            ids,
            vec![
                (Chain::Ethereum, 1, "ETH"),
                (Chain::Arbitrum, 42161, "ETH"),
                (Chain::Optimism, 10, "ETH"),
                (Chain::Base, 8453, "ETH"),
                (Chain::Polygon, 137, "POL"),
                (Chain::Bsc, 56, "BNB"),
            ]
        );
    }

    #[test]
    fn every_preset_is_https_without_a_key() {
        for row in all() {
            assert!(!row.rpc.is_empty(), "{:?} has an RPC preset", row.chain);
            for url in row.rpc.iter().chain(row.blockscout).chain(&[row.explorer]) {
                assert!(url.starts_with("https://"), "{url}");
                assert!(
                    !url.contains('$') && !url.contains('?') && !url.ends_with('/'),
                    "{url}"
                );
            }
        }
    }

    #[test]
    fn lookups_by_chain_and_by_id_agree() {
        for row in all() {
            assert_eq!(evm_chain(row.chain), Some(row));
            assert_eq!(by_chain_id(row.chain_id), Some(row));
        }
        assert_eq!(evm_chain(Chain::Bitcoin), None);
        assert_eq!(by_chain_id(999_999), None);
    }
}
