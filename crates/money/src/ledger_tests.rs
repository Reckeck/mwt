//! The period term of the operations filter (spec op-period I3, Q17, R3).

use std::collections::{HashMap, HashSet};

use mwt_base::Amount;
use mwt_base::error::Result;
use mwt_model::{
    AssetRef, CryptoHistory, DeviceSecrets, KindDetails, OpPeriod, PeriodKind, PriceStamp, Store,
    StoreKind, TopupTx, Transaction, TxMeta, VaultDoc,
};

use super::*;
use crate::books::BooksSource;
use crate::valuation::ValuationCurve;

struct Source(VaultDoc);

impl BooksSource for Source {
    fn doc(&self) -> &VaultDoc {
        &self.0
    }
    fn base_and_fx(&self) -> (String, Amount) {
        ("RUB".into(), Amount::from(1))
    }
    fn price_usd_for(&self, _asset_key: &str) -> Amount {
        Amount::from(1)
    }
    fn price_base_for(&self, _asset_key: &str, _fx: Amount) -> Amount {
        Amount::from(1)
    }
    fn valuation_curves(&self) -> HashMap<String, ValuationCurve> {
        HashMap::new()
    }
    fn prepare_transaction(&self, _tx: &mut Transaction) -> Result<()> {
        Ok(())
    }
    fn earmarked(&self, _store_id: &str, _asset_key: &str) -> Amount {
        Amount::ZERO
    }
    fn keys_on_device(&self) -> bool {
        false
    }
    fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        Ok(DeviceSecrets::default())
    }
    fn crypto_history(&self, _cutoffs: &[i64], _in_scope: &dyn Fn(&str) -> bool) -> CryptoHistory {
        CryptoHistory::default()
    }
}

struct NoSources;

impl FilterSources for NoSources {
    fn realized_tx_ids(&self, _scope: Option<&HashSet<String>>) -> HashSet<String> {
        HashSet::new()
    }
    fn tx_ids_with_attachments(&self) -> Vec<String> {
        Vec::new()
    }
    fn unpriced_tx_ids(&self) -> HashSet<String> {
        HashSet::new()
    }
}

fn income(id: &str, timestamp: &str, period: Option<(&str, &str)>) -> Transaction {
    let mut meta = TxMeta::new("income");
    meta.id = id.into();
    meta.timestamp = timestamp.into();
    meta.period = period.map(|(start, end)| OpPeriod {
        kind: PeriodKind::Range,
        start: start.into(),
        end: end.into(),
    });
    Transaction::Topup(TopupTx {
        meta,
        store_id: "card".into(),
        asset: AssetRef::fiat("RUB", "Rouble"),
        amount: Amount::parse("100").unwrap(),
        fee: None,
        from: None,
        details: KindDetails::default(),
        price: PriceStamp::default(),
        parts: Vec::new(),
        distribution: None,
    })
}

fn source() -> Source {
    let mut store = Store::new("card", StoreKind::Bank, "#000");
    store.id = "card".into();
    Source(VaultDoc {
        stores: vec![store],
        transactions: vec![
            income(
                "late",
                "2026-11-05T09:00:00Z",
                Some(("2026-10-16", "2026-10-31")),
            ),
            income("plain", "2026-11-20T09:00:00Z", None),
            income("old", "2026-09-01T09:00:00Z", None),
        ],
        ..VaultDoc::default()
    })
}

fn ids(filter: &TxFilter) -> Vec<String> {
    let source = source();
    let books = Books::new(&source);
    list_transactions(&books, &NoSources, filter, "UTC")
        .unwrap()
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect()
}

fn span(from: &str, to: &str) -> TxFilter {
    TxFilter {
        period: Some(PeriodFilter {
            from: from.into(),
            to: to.into(),
        }),
        ..TxFilter::default()
    }
}

#[test]
fn an_operation_for_october_paid_in_november_matches_october_only() {
    assert_eq!(ids(&span("2026-10-01", "2026-10-31")), ["late"]);
    assert!(!ids(&span("2026-11-01", "2026-11-15")).contains(&"late".to_string()));
}

#[test]
fn without_a_period_the_month_of_the_date_counts() {
    assert_eq!(ids(&span("2026-11-01", "2026-11-02")), ["plain"]);
    assert_eq!(ids(&span("2026-09-30", "2026-09-30")), ["old"]);
}

#[test]
fn the_period_and_the_date_filter_combine_by_and() {
    let filter = TxFilter {
        from: Some("2026-11-10".into()),
        ..span("2026-10-01", "2026-11-30")
    };
    assert_eq!(ids(&filter), ["plain"]);
}

#[test]
fn the_list_stays_in_money_date_order() {
    assert_eq!(
        ids(&span("2026-09-01", "2026-11-30")),
        ["plain", "late", "old"]
    );
}
