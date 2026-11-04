//! A price alert fired on an old threshold does not silence a newer one (124).

use super::tests::{settle, two_devices};
use super::*;
use crate::model::planning::AlertCondition;
use crate::model::vault::PricePoint;
use crate::pricing::history::unix_secs;
use crate::service::PriceAlertDraft;
use crate::service::test_support::{amount, btc};

fn price(vault: &mut UnlockedVault, value: &str, ts: &str) {
    vault.doc.price_cache.push(PricePoint {
        asset_key: btc().key(),
        value_usd: amount(value),
        ts: ts.into(),
        source: "test".into(),
        change_24h_pct: None,
    });
}

fn draft(threshold: &str) -> PriceAlertDraft {
    PriceAlertDraft {
        asset: btc(),
        condition: AlertCondition::Above,
        price: amount(threshold),
        currency: "usd".into(),
        active: true,
        store_id: None,
        repeats: false,
    }
}

#[test]
fn a_firing_on_the_old_threshold_does_not_silence_the_raised_one() {
    let (_dir, mut left, mut right) = two_devices();
    let id = left.create_price_alert(draft("95000")).unwrap().id;
    settle(&mut left, &mut right);

    // Right fires on the old threshold while left raises it.
    price(&mut right, "100000", "2026-03-01T00:00:00Z");
    let at = unix_secs("2026-03-01T01:00:00Z").unwrap();
    assert_eq!(right.check_alerts(at).unwrap().len(), 1);
    left.update_price_alert(&id, draft("120000")).unwrap();
    settle(&mut left, &mut right);

    for device in [&left, &right] {
        let merged = &device.doc.price_alerts[0];
        assert_eq!(merged.price, amount("120000"));
        assert!(merged.triggered_at.is_some(), "the stale firing is kept");
        assert!(!merged.is_triggered(), "but it no longer counts as fired");
    }

    // The new threshold is reached: it fires.
    price(&mut left, "130000", "2026-03-02T00:00:00Z");
    let later = unix_secs("2026-03-02T01:00:00Z").unwrap();
    assert_eq!(left.check_alerts(later).unwrap().len(), 1);
    assert!(left.doc.price_alerts[0].is_triggered());
}
