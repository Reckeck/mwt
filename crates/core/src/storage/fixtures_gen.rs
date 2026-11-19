//! The v4 vault fixtures: a real vault of the pre-reset build, committed as bytes.
//!
//! `fixtures/v4.json.wallet` and `fixtures/v4.pack.wallet` are the same document
//! sealed in the two payload formats, `fixtures/v4.expected.json` is that
//! document as JSON. Their password is [`V4_PASSWORD`] (`v4-fixture-password`);
//! it and the exchange keys inside are throwaway values, never real secrets.
//!
//! They were made by the unchanged code at the base commit of the vault reset
//! (payload version 4, container version 1) with
//!
//! ```text
//! cargo test -p trade-core --lib storage::fixtures_gen::generate -- --ignored
//! ```
//!
//! Regenerating after the reset would write a different format: the generator
//! is a record of how the bytes came to be, and goes away with the transition.

use std::path::Path;

use serde_json::{Value as Json, json};

use super::{SnapshotFormat, VaultFile};
use crate::model::fixtures::{every_transaction, full_doc};
use crate::model::store::AssetRef;
use crate::model::vault::{ExchangeKeys, Exchanges, VaultDoc};

/// The password of both fixtures.
pub(super) const V4_PASSWORD: &str = "v4-fixture-password";

/// The version the fixtures claim.
pub(super) const V4_VERSION: u32 = 4;

/// Collections `full_doc` leaves empty or thin, as the JSON the document has them in.
fn extras() -> Json {
    let queued = serde_json::to_value(&every_transaction()[..1]).expect("transactions");
    json!({
        "pending_imports": [
            {
                "exec_id": "exec-1", "store_id": "bybit", "category": "linear",
                "symbol": "BTCUSDT", "side": "buy", "asset_bought": "BTC",
                "qty_bought": "0.01", "asset_sold": "USDT", "qty_sold": "650",
                "price": "65000", "fee": "0.39", "fee_asset": "USDT",
                "occurred_at": "2026-10-01T12:00:00Z", "comment": "Bybit fill",
                "is_maker": false, "closed_size": null, "closed_pnl": null,
                "kind": "fill", "from_account": null, "to_account": null
            },
            {
                "exec_id": "exec-2", "store_id": "bybit", "category": "spot",
                "symbol": "ETHUSDT", "side": "sell", "asset_bought": "USDT",
                "qty_bought": "1800", "asset_sold": "ETH", "qty_sold": "0.5",
                "price": "3600", "fee": "0", "fee_asset": "",
                "occurred_at": "2026-10-02T12:00:00Z", "comment": "",
                "is_maker": true, "closed_size": "0.5", "closed_pnl": "12.5",
                "kind": "transfer", "from_account": "FUND", "to_account": "UNIFIED"
            }
        ],
        "envelopes": [{
            "id": "env-1", "store_id": "bank", "name": "Holiday",
            "asset": serde_json::to_value(AssetRef::fiat("USD", "US Dollar")).expect("asset"),
            "allocated": "300",
            "moves": [{"id": "open:env-1", "delta": "300", "at": "2026-02-01T00:00:00Z"}],
            "target": "1000", "goal_id": "goal", "created_at": "2026-02-01T00:00:00Z"
        }],
        "debt_terms": [{
            "id": "debt-1", "contact_id": "contact", "asset": "USD",
            "due_at": "2026-12-31", "note": "lent", "created_at": "2026-02-01T00:00:00Z"
        }],
        "key_rates": [{"date": "2026-09-12", "rate": "16.5"}],
        "calc_scenarios": [{
            "id": "calc-1", "name": "Deposit", "input": {"currency": "rub", "term_months": 12},
            "source": null, "prefilled_at": null,
            "created_at": "2026-03-01T00:00:00Z", "updated_at": "2026-03-02T00:00:00Z"
        }],
        "inflation_points": [{
            "currency": "rub", "month": "2026-09", "yoy_pct": "8.1", "source": "cbr"
        }],
        "import_runs": [{
            "id": "run-1", "store_id": "bybit", "at": "2026-10-01T12:05:00Z",
            "source": "manual", "fetched": 10, "staged": 2, "known": 6,
            "filtered": 2, "confirmed": 0, "trades": 1, "transfers": 1,
            "reconcile_tx_id": null, "error": null
        }],
        "discarded_imports": ["exec-old-1", "exec-old-2"],
        "workflow_vars": [{
            "scope": "global", "name": "counter", "value": "{\"Number\":\"3\"}",
            "updated_at": "2026-04-01T00:00:00Z"
        }],
        "workflow_queue": [{
            "id": "queue-1", "workflow_id": "auto-0", "run_id": "run-0",
            "created_at": "2026-04-01T00:00:00Z", "summary": "one payment",
            "transactions": queued
        }],
        "workflow_state": [{
            "workflow_id": "auto-0", "marks": {"t": "2026-04-01T09:00:00Z"},
            "tx_cursor": "tx-1", "tx_index": 4
        }],
        "data_sources": [{
            "id": "src-1", "name": "Key rate", "url": "https://example.invalid/rate",
            "method": {"kind": "json", "path": "rate"}, "output": "number",
            "interval": "day",
            "last": {"at": "2026-10-01T00:00:00Z", "number": "16.5", "text": null, "series": []},
            "history": [], "pending": null, "failures": 0, "status": "ok",
            "error": null, "checked_at": "2026-10-01T00:00:00Z",
            "created_at": "2026-09-01T00:00:00Z"
        }],
        "asset_price_sources": [{"asset_key": "asset:FLAT", "source_id": "src-1", "currency": "rub"}],
        "deposit_rate_sources": [{"store_id": "bank", "source_id": "src-1"}],
        "attachments": [{
            "id": "att-1", "owner": {"kind": "tx", "id": "tx-1"}, "kind": "file",
            "title": "receipt.pdf", "url": null, "mime": "application/pdf",
            "size": 2048, "sha256": "ab".repeat(32), "created_at": "2026-05-01T00:00:00Z",
            "removed_at": null
        }],
        "attachment_keys": [{"id": "att-1", "key": "cd".repeat(32)}],
        "shared_events": [{
            "id": "event-1", "title": "Trip", "created_at": "2026-06-01T00:00:00Z",
            "note": "", "archived": false, "starts_on": "2026-06-10", "ends_on": null,
            "currency": "usd", "rounding_step": "1", "category_id": null,
            "organizer": "me", "i_participate": true,
            "participants": [{"id": "guest", "name": "Guest", "contact_id": null, "details": null}],
            "items": [], "settlements": [], "adjustments": []
        }]
    })
}

/// The document of both fixtures: `full_doc` plus what a real v4 vault has on top.
pub(super) fn v4_doc() -> VaultDoc {
    let mut doc = full_doc();
    assert_eq!(doc.version, V4_VERSION, "the generator needs the v4 build");
    doc.exchanges = Exchanges {
        bybit: vec![ExchangeKeys {
            store_id: "bybit".into(),
            api_key: "TESTKEY0123456789".into(),
            api_secret: "test-secret-not-real".into(),
            spot_store_id: Some("bybit-spot".into()),
            futures_store_id: None,
        }],
    };
    doc.holdings = vec![crate::model::store::Holding {
        id: "holding-1".into(),
        store_id: doc.stores[0].id.clone(),
        asset: AssetRef::fiat("USD", "US Dollar"),
        quantity: crate::Amount::parse("1500").expect("amount"),
        avg_cost: None,
    }];
    doc.price_cache = vec![crate::model::vault::PricePoint {
        asset_key: "crypto:btc".into(),
        value_usd: crate::Amount::parse("65000").expect("amount"),
        ts: "2026-10-01T00:00:00Z".into(),
        source: "coingecko".into(),
        change_24h_pct: Some(crate::Amount::parse("1.5").expect("amount")),
    }];
    doc.fx_cache = vec![crate::model::vault::FxPoint {
        currency: "rub".into(),
        per_usd: crate::Amount::parse("92.5").expect("amount"),
        ts: "2026-10-01T00:00:00Z".into(),
        source: "cbr".into(),
    }];
    // A graph written by the build before graph version 3.
    doc.workflows[0].graph = old_graph(&doc.workflows[0].graph);

    let mut value = serde_json::to_value(&doc).expect("doc as JSON");
    let fields = value.as_object_mut().expect("a document is an object");
    let Json::Object(extras) = extras() else {
        unreachable!("extras is an object")
    };
    fields.extend(extras);
    let mut doc: VaultDoc = serde_json::from_value(value).expect("the extras fit the document");
    doc.workflow_waits = vec![wait(&doc.workflows[0].graph)];
    doc
}

/// `graph` with its `version` set to 2.
fn old_graph(graph: &str) -> String {
    let mut value: Json = serde_json::from_str(graph).expect("a graph is JSON");
    value["version"] = json!(2);
    value.to_string()
}

/// A run asleep at a "Wait for confirmation" node, carrying a graph of version 2.
fn wait(graph: &str) -> crate::model::workflow::WorkflowWait {
    crate::model::workflow::WorkflowWait {
        id: "wait-1".into(),
        workflow_id: "auto-0".into(),
        run_id: "run-0".into(),
        node_id: "t".into(),
        resume_at: "2026-04-02T09:00:00Z".into(),
        memo: "{}".into(),
        trigger: "schedule 2026-04-01T09:00:00Z".into(),
        kind: crate::model::workflow::WaitKind::Confirm,
        question: "Pay the rent?".into(),
        created_at: "2026-04-01T09:00:00Z".into(),
        graph: graph.into(),
    }
}

fn write_fixture(dir: &Path, name: &str, doc: &VaultDoc, fmt: SnapshotFormat) {
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    VaultFile::save(&path, V4_PASSWORD, doc, fmt).expect("write the fixture");
}

/// The files the generator writes.
const FIXTURE_FILES: [&str; 3] = ["v4.json.wallet", "v4.pack.wallet", "v4.expected.json"];

/// Refuses to run over fixtures that already exist: they are committed bytes.
fn ensure_nothing_to_overwrite(dir: &Path) -> Result<(), String> {
    match FIXTURE_FILES.iter().find(|name| dir.join(name).exists()) {
        Some(name) => Err(format!(
            "{name} already exists in {}; the committed fixtures are never overwritten",
            dir.display()
        )),
        None => Ok(()),
    }
}

/// Writes the three fixture files next to this module. Run by hand, see the module docs.
#[test]
#[ignore = "writes the committed fixtures; run by hand once, before the format changes"]
fn generate_v4_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/storage/fixtures");
    ensure_nothing_to_overwrite(&dir).unwrap_or_else(|reason| panic!("{reason}"));
    let doc = v4_doc();
    write_fixture(&dir, "v4.json.wallet", &doc, SnapshotFormat::Json);
    write_fixture(&dir, "v4.pack.wallet", &doc, SnapshotFormat::Pack);
    let expected = serde_json::to_string_pretty(&doc).expect("doc as JSON");
    std::fs::write(dir.join("v4.expected.json"), expected + "\n").expect("write expected");
}

#[test]
fn the_generator_refuses_to_overwrite_existing_fixtures() {
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/storage/fixtures");
    assert!(ensure_nothing_to_overwrite(&committed).is_err());
    let empty = std::env::temp_dir().join(format!("fixtures-gen-guard-{}", std::process::id()));
    std::fs::create_dir_all(&empty).expect("temp dir");
    assert!(ensure_nothing_to_overwrite(&empty).is_ok());
    std::fs::remove_dir_all(&empty).expect("clean up");
}
