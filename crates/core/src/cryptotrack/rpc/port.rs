//! The shared rate-limited client is the production [`ChainSource`].

use serde_json::Value;

use super::RpcClient;
use crate::cryptotrack::ports::ChainSource;
use crate::error::Result;

impl ChainSource for RpcClient {
    async fn get_json(&self, base: &str, path_and_query: &str, subject: &str) -> Result<Value> {
        Self::get_json(self, base, path_and_query, subject).await
    }

    async fn call(&self, url: &str, subject: &str, method: &str, params: Value) -> Result<Value> {
        Self::call(self, url, subject, method, params).await
    }
}
