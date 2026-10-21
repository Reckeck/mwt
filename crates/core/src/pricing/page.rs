//! A user's data source page, fetched as is (plan I2). The pricing service's
//! client: same timeout and user agent as every other public endpoint.

use reqwest::header::{ACCEPT, USER_AGENT};

use super::PricingService;
use crate::error::{Error, Result};

/// Larger pages are refused: a value is never that far down, and the vault
/// keeps only the value.
pub const MAX_PAGE_BYTES: usize = 8 * 1024 * 1024;

impl PricingService {
    /// The body of `url`. Only `http(s)`; redirects are followed by reqwest.
    pub async fn fetch_page(&self, url: &str) -> Result<Vec<u8>> {
        let lower = url.trim().to_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            return Err(Error::InvalidInput(
                "url: the address starts with https://".into(),
            ));
        }
        let mut resp = self
            .client
            .get(url.trim())
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "text/html,application/json,text/csv,*/*;q=0.8")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Network(format!("http {}", resp.status().as_u16())));
        }
        let mut body = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|err| Error::Network(err.to_string()))?
        {
            if body.len() + chunk.len() > MAX_PAGE_BYTES {
                return Err(Error::Network("the page is larger than 8 MB".into()));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}
