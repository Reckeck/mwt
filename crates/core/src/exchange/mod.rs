//! Exchange integrations.
//!
//! Everything here is read-only by construction: the modules expose no
//! trading, funding or withdrawal endpoints, so a leaked key cannot move funds.

pub mod bybit;

pub use bybit::{
    BybitClient, Category, CoinBalance, EarnYield, Execution, InterTransfer, TradePrefill,
};

/// CoinGecko id and name of coins an exchange reports by ticker alone, so an
/// imported coin is priced and grouped like the one picked by hand.
const WELL_KNOWN_COINS: &[(&str, &str, &str)] = &[
    ("BTC", "bitcoin", "Bitcoin"),
    ("ETH", "ethereum", "Ethereum"),
    ("USDT", "tether", "Tether"),
    ("USDC", "usd-coin", "USDC"),
    ("BNB", "binancecoin", "BNB"),
    ("SOL", "solana", "Solana"),
    ("XRP", "ripple", "XRP"),
    ("TON", "the-open-network", "Toncoin"),
    ("DOGE", "dogecoin", "Dogecoin"),
    ("TRX", "tron", "TRON"),
    ("ADA", "cardano", "Cardano"),
    ("LTC", "litecoin", "Litecoin"),
    ("DOT", "polkadot", "Polkadot"),
    ("AVAX", "avalanche-2", "Avalanche"),
    ("LINK", "chainlink", "Chainlink"),
    ("MATIC", "matic-network", "Polygon"),
    ("POL", "polygon-ecosystem-token", "POL"),
    ("ATOM", "cosmos", "Cosmos"),
    ("NEAR", "near", "NEAR"),
    ("DAI", "dai", "Dai"),
    ("FDUSD", "first-digital-usd", "First Digital USD"),
    ("USDE", "ethena-usde", "Ethena USDe"),
    ("MNT", "mantle", "Mantle"),
    ("ARB", "arbitrum", "Arbitrum"),
    ("OP", "optimism", "Optimism"),
    ("SUI", "sui", "Sui"),
    ("APT", "aptos", "Aptos"),
    ("SHIB", "shiba-inu", "Shiba Inu"),
    ("PEPE", "pepe", "Pepe"),
    ("NOT", "notcoin", "Notcoin"),
];

/// `(coingecko_id, name)` of a well-known ticker.
pub fn well_known_coin(symbol: &str) -> Option<(&'static str, &'static str)> {
    WELL_KNOWN_COINS
        .iter()
        .find(|(ticker, _, _)| ticker.eq_ignore_ascii_case(symbol.trim()))
        .map(|(_, id, name)| (*id, *name))
}

const USD_STABLECOINS: &[&str] = &[
    "USDT",
    "USDC",
    "FDUSD",
    "DAI",
    "USDE",
    "BUSD",
    "TUSD",
    "PYUSD",
    "tether",
    "usd-coin",
    "first-digital-usd",
    "dai",
    "ethena-usde",
    "binance-usd",
    "true-usd",
    "paypal-usd",
];

/// Whether a price key (`asset:USDT`, `coin:tether`, …) is a dollar stablecoin.
pub fn is_usd_stablecoin(asset_key: &str) -> bool {
    let (kind, rest) = asset_key.split_once(':').unwrap_or(("", asset_key));
    let name = match kind {
        "coin" | "asset" => rest,
        // token:{chain}:{symbol}:{contract}
        "token" => rest.split(':').nth(1).unwrap_or_default(),
        _ => return false,
    };
    USD_STABLECOINS
        .iter()
        .any(|stable| stable.eq_ignore_ascii_case(name))
}
