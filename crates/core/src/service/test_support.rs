//! Builders shared by the service tests: each takes the minimum a user would
//! supply, so a test reads as the transaction it describes.

use tempfile::tempdir;

use super::UnlockedVault;
use crate::Amount;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::*;

pub fn amount(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

pub fn btc() -> AssetRef {
    AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin)
}

pub fn usd() -> AssetRef {
    AssetRef::fiat("USD", "US Dollar")
}

pub fn usd_price(raw: &str) -> PriceStamp {
    PriceStamp {
        price_usd: Some(amount(raw)),
        ..PriceStamp::default()
    }
}

pub fn empty_vault() -> (tempfile::TempDir, UnlockedVault) {
    let dir = tempdir().unwrap();
    let vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    (dir, vault)
}

/// A vault with an exchange (`Binance`) and a cold wallet (`Cold`).
pub fn setup() -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = empty_vault();
    vault
        .create_store_with(
            "Binance",
            StoreKind::Exchange,
            "#f0b90b",
            "",
            None,
            crate::service::StoreExtras {
                provider: Some(crate::model::store::ExchangeProvider::Bybit),
                ..Default::default()
            },
        )
        .unwrap();
    vault
        .create_store("Cold", StoreKind::Crypto, "#555", "", None)
        .unwrap();
    (dir, vault)
}

pub fn store(
    vault: &mut UnlockedVault,
    name: &str,
    kind: StoreKind,
    parent: Option<&str>,
) -> String {
    // The vault comes with its one cash wallet: reuse it under the asked name.
    if kind == StoreKind::Cash && parent.is_none() {
        let cash = vault
            .doc
            .stores
            .iter_mut()
            .find(|store| store.kind == StoreKind::Cash && store.parent_id.is_none())
            .expect("the vault has a cash wallet");
        cash.name = name.to_string();
        return cash.id.clone();
    }
    vault
        .create_store(name, kind, "#000", "", parent.map(str::to_string))
        .unwrap()
        .id
}

pub fn meta(comment: &str, categories: &[&str]) -> TxMeta {
    let mut meta = TxMeta::new(comment);
    meta.categories = categories.iter().map(|key| key.to_string()).collect();
    meta
}

pub fn fee_usd(raw: &str) -> Option<Fee> {
    Some(Fee {
        asset: usd(),
        amount: amount(raw),
        price: usd_price("1"),
    })
}

/// «Пополнить»: USD salary at a price of 1.
pub fn income(store: &str, raw: &str, comment: &str) -> Transaction {
    income_of(store, usd(), raw, "1", comment)
}

/// «Пополнить» in a chosen asset at a chosen USD price.
pub fn income_of(
    store: &str,
    asset: AssetRef,
    raw: &str,
    price: &str,
    comment: &str,
) -> Transaction {
    Transaction::Topup(TopupTx {
        meta: meta(comment, &["salary"]),
        store_id: store.into(),
        asset,
        amount: amount(raw),
        fee: None,
        from: None,
        details: KindDetails::default(),
        price: usd_price(price),
    })
}

/// «Оплатить» in USD, category groceries.
pub fn payment(store: &str, raw: &str, comment: &str) -> Transaction {
    Transaction::Payment(PaymentTx {
        meta: meta(comment, &["groceries"]),
        store_id: store.into(),
        asset: usd(),
        amount: amount(raw),
        fee: None,
        to: None,
        details: KindDetails::default(),
        price: usd_price("1"),
    })
}

/// «Перевести» between two sides.
pub fn transfer_between(
    from: AccountRef,
    to: AccountRef,
    asset: AssetRef,
    raw: &str,
) -> Transaction {
    Transaction::Transfer(TransferTx {
        meta: meta("move", &[]),
        from,
        to,
        asset,
        amount: amount(raw),
        fee: None,
        network: None,
        tx_hash: None,
        debt: None,
        price: PriceStamp::default(),
    })
}

/// BTC from one store to another.
pub fn transfer(from: &str, to: &str, raw: &str) -> Transaction {
    transfer_between(
        AccountRef::Store(from.into()),
        AccountRef::Store(to.into()),
        btc(),
        raw,
    )
}

/// Spot trade: USD sold, BTC bought, both unpriced.
pub fn trade(store: &str, sell: &str, buy: &str, comment: &str) -> Transaction {
    Transaction::TradeSpot(TradeSpotTx {
        meta: meta(comment, &[]),
        store_id: store.into(),
        sold: TradeSide {
            asset: usd(),
            qty: amount(sell),
            price: PriceStamp::default(),
        },
        bought: TradeSide {
            asset: btc(),
            qty: amount(buy),
            price: PriceStamp::default(),
        },
        fee: None,
        liquidity: Some(Liquidity::Taker),
        order_id: None,
    })
}

/// Spot trade: `btc_qty` BTC sold at `btc_price` USD for `usd_qty` USD.
pub fn sell_btc(store: &str, btc_qty: &str, usd_qty: &str, btc_price: &str) -> Transaction {
    Transaction::TradeSpot(TradeSpotTx {
        meta: meta("sell", &[]),
        store_id: store.into(),
        sold: TradeSide {
            asset: btc(),
            qty: amount(btc_qty),
            price: usd_price(btc_price),
        },
        bought: TradeSide {
            asset: usd(),
            qty: amount(usd_qty),
            price: usd_price("1"),
        },
        fee: None,
        liquidity: None,
        order_id: None,
    })
}

pub fn at(tx: &mut Transaction, stamp: &str) {
    tx.meta_mut().timestamp = stamp.into();
}

pub fn holding_of(vault: &UnlockedVault, store: &str, asset: &AssetRef) -> Amount {
    vault
        .doc
        .holdings
        .iter()
        .find(|holding| holding.store_id == store && holding.asset.key() == asset.key())
        .map(|holding| holding.quantity)
        .unwrap_or(Amount::ZERO)
}

/// A USD quote in the price cache, so holdings have a value.
pub fn quote_usd(vault: &mut UnlockedVault) {
    vault.doc.price_cache.push(crate::model::vault::PricePoint {
        asset_key: usd().key(),
        value_usd: amount("1"),
        ts: "2000-01-01T00:00:00Z".into(),
        source: "test".into(),
    });
}
