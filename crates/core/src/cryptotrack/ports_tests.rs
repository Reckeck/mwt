//! The ports against fakes: no network, no key, no clock.
//!
//! The first test is the engine test converted to a fake [`ChainSource`]: the
//! explorer page of `evm::tests::a_full_read_walks_all_lists_and_builds_deterministic_ids`
//! now comes from a fake chain source and runs through the engine's
//! `normalize` (the rest of that test still uses its own fake explorer).

use std::sync::Mutex;

use chrono::NaiveDate;
use serde_json::{Value, json};

use super::{ChainSource, ExchangeClient, PriceSource};
use crate::cryptotrack::engine::{Followed, normalize};
use crate::cryptotrack::sources::bybit::{FillsPage, PageRequest};
use crate::cryptotrack::sources::evm::{ExplorerApi, HistoryRequest, fetch_page};
use crate::error::Result;
use crate::exchange::{EarnYield, InterTransfer};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoAsset, CryptoPricePoint, TokenId};

const ADDRESS: &str = "0xde0B295669a9FD93d5F28D9Ec85E40f4cb697BAe";

/// Serves one recorded explorer page and remembers whom it was asked for.
struct FakeChain {
    page: Value,
    subjects: Mutex<Vec<String>>,
}

impl ChainSource for FakeChain {
    async fn get_json(&self, _base: &str, _path: &str, subject: &str) -> Result<Value> {
        self.subjects.lock().unwrap().push(subject.to_owned());
        Ok(self.page.clone())
    }

    async fn call(
        &self,
        _url: &str,
        subject: &str,
        _method: &str,
        _params: Value,
    ) -> Result<Value> {
        self.subjects.lock().unwrap().push(subject.to_owned());
        Ok(json!("0x0"))
    }
}

/// Lets an explorer reader run on any chain source.
struct Explorer<'src, C: ChainSource>(&'src C);

impl<C: ChainSource> ExplorerApi for Explorer<'_, C> {
    async fn get_json(&self, base: &str, path_and_query: &str, subject: &str) -> Result<Value> {
        self.0.get_json(base, path_and_query, subject).await
    }
}

fn recorded_page() -> Value {
    let path = format!(
        "{}/src/cryptotrack/sources/evm/fixtures/blockscout.transactions.1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn the_engine_normalizes_a_page_read_through_a_fake_chain_source() {
    let chain = FakeChain {
        page: recorded_page(),
        subjects: Mutex::default(),
    };
    let request = HistoryRequest {
        chain: Chain::Ethereum,
        address: ADDRESS,
        cursor: None,
        since: None,
        explorer: None,
    };
    let batch = fetch_page(&Explorer(&chain), &request).await.unwrap();
    let address = ADDRESS.to_lowercase();
    let done = normalize(
        &batch.raw_events,
        Followed {
            account_id: "acc",
            chain: Chain::Ethereum,
            address: &address,
        },
    );
    assert!(!done.events.is_empty());
    assert_eq!(*chain.subjects.lock().unwrap(), vec![address]);
}

/// Records what it was asked and honours the policy like the real adapter.
struct FakePrices {
    consent: bool,
    asked: Mutex<Vec<String>>,
}

impl PriceSource for FakePrices {
    fn may_lookup_contract(&self) -> bool {
        self.consent
    }

    async fn historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> Result<Vec<CryptoPricePoint>> {
        for (asset, _) in wanted {
            if self.consent || matches!(asset, CryptoAsset::Native { .. }) {
                self.asked.lock().unwrap().push(asset.key());
            }
        }
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn a_price_source_reports_its_consent() {
    let source = FakePrices {
        consent: false,
        asked: Mutex::default(),
    };
    let day = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let token = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xabc"));
    source.historic_prices(&[(token, day)]).await.unwrap();
    assert!(!source.may_lookup_contract());
    assert!(source.asked.lock().unwrap().is_empty());
}

/// An exchange that answers nothing, to check the port is implementable.
struct FakeExchange;

impl ExchangeClient for FakeExchange {
    async fn fills_page(&self, _request: PageRequest<'_>) -> Result<FillsPage> {
        Ok(FillsPage::default())
    }

    async fn transfers(&self, _limit: usize) -> Result<Vec<InterTransfer>> {
        Ok(Vec::new())
    }

    async fn payouts(&self, _limit: usize) -> Result<Vec<EarnYield>> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn an_exchange_client_is_a_fake_away() {
    let exchange = FakeExchange;
    assert!(exchange.transfers(10).await.unwrap().is_empty());
    assert!(exchange.payouts(10).await.unwrap().is_empty());
}
