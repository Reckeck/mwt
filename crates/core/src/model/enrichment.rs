//! What a block explorer told us about an on-chain transaction, waiting for
//! the user to apply or reject it. Typed — no `serde_json::Value` in the vault.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::chain::Chain;
use crate::model::transaction::Fee;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentSource {
    Explorer(Chain),
    TokenList,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct EnrichmentData {
    pub fee: Option<Fee>,
    pub confirmed: Option<bool>,
    pub block_time: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub token_name: Option<String>,
    pub token_decimals: Option<u8>,
    pub contract: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentStatus {
    Pending,
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct EnrichmentRecord {
    pub id: String,
    pub tx_id: String,
    pub source: EnrichmentSource,
    pub found: EnrichmentData,
    pub status: EnrichmentStatus,
    pub created_at: String,
}
