//! Tests of the price_info area that need the vault (`UnlockedVault`): moved from
//! `money/price_info_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::store::StoreKind;
use crate::model::vault::PricePoint;
use crate::money::price_info::*;
use crate::service::test_support::*;

fn point(key: &str, usd: &str, ts: &str, source: &str, change: Option<&str>) -> PricePoint {
    PricePoint {
        asset_key: key.into(),
        value_usd: amount(usd),
        ts: ts.into(),
        source: source.into(),
        change_24h_pct: change.map(amount),
    }
}

#[test]
fn a_cached_quote_reports_its_provider() {
    let (_dir, mut vault) = empty_vault();
    crate::usecases::money::rates::apply_prices(
        &mut vault,
        &[point(
            "coin:bitcoin",
            "50000",
            "2026-10-19T10:00:00Z",
            "coingecko",
            Some("2.5"),
        )],
    );
    let quote = vault.price_quote_for("coin:bitcoin");
    assert_eq!(quote.info.origin, PriceOrigin::Market);
    assert_eq!(quote.info.source.as_deref(), Some("coingecko"));
    assert_eq!(quote.change_24h_pct, Some(amount("2.5")));
}

#[test]
fn the_24h_change_falls_back_to_history() {
    let (_dir, mut vault) = empty_vault();
    crate::usecases::money::rates::apply_prices(
        &mut vault,
        &[point(
            "coin:bitcoin",
            "40000",
            "2026-10-17T10:00:00Z",
            "cryptoprice",
            None,
        )],
    );
    crate::usecases::money::rates::apply_prices(
        &mut vault,
        &[point(
            "coin:bitcoin",
            "50000",
            "2026-10-19T10:00:00Z",
            "cryptoprice",
            None,
        )],
    );
    let quote = vault.price_quote_for("coin:bitcoin");
    assert_eq!(quote.change_24h_pct, Some(amount("25")));
    // The old generic label names no provider.
    assert_eq!(quote.info.source, None);
}

#[test]
fn an_unquoted_stablecoin_is_peg_valued() {
    let (_dir, vault) = empty_vault();
    let quote = vault.price_quote_for("coin:tether");
    assert_eq!(quote.info.origin, PriceOrigin::Peg);
    assert_eq!(quote.usd, amount("1"));
}

#[test]
fn only_a_stamp_reports_stamp_origin() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&wallet, btc(), "1", "30000", "gift"),
    )
    .unwrap();
    let quote = vault.price_quote_for(&btc().key());
    assert_eq!(quote.info.origin, PriceOrigin::Stamp);
    assert_eq!(quote.usd, amount("30000"));
}

#[test]
fn a_manual_price_reports_manual_origin() {
    let (_dir, mut vault) = empty_vault();
    assert_eq!(
        vault.price_quote_for("asset:FLAT").info.origin,
        PriceOrigin::None
    );
    vault
        .doc
        .manual_prices
        .push(crate::model::vault::ManualPrice {
            asset_key: "asset:FLAT".into(),
            unit_price: amount("100000"),
            currency: "usd".into(),
            updated_at: "2026-10-01T00:00:00Z".into(),
        });
    let quote = vault.price_quote_for("asset:FLAT");
    assert_eq!(quote.info.origin, PriceOrigin::Manual);
    assert_eq!(quote.usd, amount("100000"));
}
