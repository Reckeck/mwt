//! IPC types of the tokens the crypto module has met: what is known, what
//! sits in quarantine, and the user's verdict on it.
//!
//! Owner: spec E2 (its browser mock comes with E9). A token's identity is its
//! network and contract, never its symbol
//! (`crate::model::cryptotrack::TokenId`), and a name that came from a chain
//! is untrusted text.
//!
//! Commands: `crates/app/src/commands/cryptotrack/tokens.rs`.

use serde::{Deserialize, Serialize};

use crate::Amount;
pub use crate::cryptotrack::tokens::TokenStatus;
use crate::model::chain::Chain;

/// Why a token's text deserves a second look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum TokenWarning {
    /// Mixed scripts or fullwidth letters: it may not read the way it looks.
    LookAlike,
    /// Its symbol reads like a known token's but its contract is not.
    ImitatesKnown,
}

/// One token as the list shows it. `symbol` and `name` are already cleaned
/// (length, control characters, links) and are plain text, never markup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct TokenView {
    pub chain: Chain,
    pub contract: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub status: TokenStatus,
    /// What the old purchases cost, USD per unit, when the user said.
    pub basis_usd: Option<Amount>,
    pub warnings: Vec<TokenWarning>,
    /// When the user confirmed or rejected it (RFC3339 UTC); `None` with no
    /// verdict. The hidden-token list reads it for "hidden on <date>".
    #[ts(optional)]
    pub decided_at: Option<String>,
}

/// Which token a command is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TokenRefInput {
    pub chain: Chain,
    pub contract: String,
}

/// Set (`Some`) or clear (`None`) the cost basis of a token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SetTokenBasisInput {
    pub chain: Chain,
    pub contract: String,
    pub basis_usd: Option<Amount>,
}
