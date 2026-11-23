//! Scenarios of the loyalty area: each decides in `household::loyalty`, then
//! ends in one `vault.transact` (or in the other area's write, called second).

use crate::Amount;
use crate::error::{Error, Result};
use crate::household::loyalty::{
    LoyaltyUpdate, plan_adjust_points, plan_create_loyalty_program, plan_set_loyalty_info,
};
use crate::kernel::tx::Tx;
use crate::model::ids::StoreId;
use crate::model::store::{LoyaltyInfo, Store, StoreKind};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::service::{Session, StoreExtras};

/// Set a programme's points and price of a point. The price becomes the
/// points' hand-set price, so the wallet counts in net worth.
pub fn set_loyalty_info(
    vault: &mut Session,
    store_id: &StoreId,
    info: LoyaltyInfo,
) -> Result<Store> {
    let update = plan_set_loyalty_info(&vault.doc, store_id, info, &now_iso())?;
    vault.transact(|tx| apply_loyalty_update(tx, update))
}

fn apply_loyalty_update(tx: &mut Tx<'_>, update: LoyaltyUpdate) -> Result<Store> {
    let LoyaltyUpdate {
        store_id,
        asset_key,
        info,
        price,
    } = update;
    let doc = tx.doc_mut();
    // Looked up first: a missing store must leave the document untouched.
    let position = doc
        .stores
        .iter()
        .position(|candidate| candidate.id == store_id.as_str())
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
    doc.manual_prices
        .retain(|known| known.asset_key != asset_key);
    doc.manual_prices.extend(price);
    let stored = &mut doc.stores[position];
    stored.loyalty = Some(info);
    Ok(stored.clone())
}

/// Create a bonus programme and set its points in one action, so a
/// programme never exists without them.
pub fn create_loyalty_program(
    vault: &mut Session,
    name: &str,
    color: &str,
    contact_id: Option<String>,
    info: LoyaltyInfo,
) -> Result<Store> {
    // Checked before the wallet exists: a refusal leaves nothing behind.
    let programme = plan_create_loyalty_program(&vault.doc, contact_id, info)?;
    let extras = StoreExtras {
        contact_id: programme.contact_id,
        ..StoreExtras::default()
    };
    let store = crate::usecases::money::stores::create_store_with(
        vault,
        name,
        StoreKind::Loyalty,
        color,
        "",
        None,
        extras,
    )?;
    set_loyalty_info(vault, &StoreId::from(store.id), programme.info)
}

/// Credit (`delta` > 0) or spend points of a programme, optionally tied
/// to the card whose purchase earned them.
pub fn adjust_points(
    vault: &mut Session,
    store_id: &StoreId,
    delta: Amount,
    comment: &str,
    source_store_id: Option<StoreId>,
    timestamp: Option<String>,
) -> Result<Transaction> {
    let tx = plan_adjust_points(
        &vault.doc,
        store_id,
        delta,
        comment,
        source_store_id,
        timestamp,
    )?;
    crate::usecases::money::transactions::add_transaction(vault, tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::vault::ManualPrice;
    use crate::service::test_support::{amount, empty_vault};

    #[test]
    fn a_missing_store_leaves_the_document_untouched() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.manual_prices.push(ManualPrice {
            asset_key: "points".into(),
            unit_price: amount("1"),
            currency: "rub".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        });
        let before = vault.doc.clone();
        let update = LoyaltyUpdate {
            store_id: StoreId::from("missing".to_string()),
            asset_key: "points".into(),
            info: LoyaltyInfo {
                points_symbol: "points".into(),
                point_value: amount("2"),
                currency: "rub".into(),
                expires_at: None,
                expiring_points: None,
                login: None,
                source_store_ids: Vec::new(),
                unit: Default::default(),
            },
            price: None,
        };
        let outcome = apply_loyalty_update(&mut Tx::new(&mut vault.doc), update);
        assert!(matches!(outcome, Err(Error::Missing(_))));
        assert_eq!(vault.doc, before);
    }
}
