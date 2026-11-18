//! Scenarios of the asset-classes area: each decides in `planning::allocation`,
//! then ends in one `vault.transact`.

use crate::error::Result;
use crate::model::planning::{AssetClass, AssetClassLink};
use crate::planning::allocation::{
    AssetClassTarget, AssetClassesUpdate, AssetClassesView, asset_classes,
    check_assign_asset_class, plan_set_asset_classes, preset_classes,
};
use crate::service::UnlockedVault;
use crate::service::sync_ids::asset_class_id;

/// Replace the baskets. Shares must add up to 100; a basket that is gone
/// takes its assignments with it. An empty list brings the preset back.
pub fn set_asset_classes(
    vault: &mut UnlockedVault,
    classes: Vec<AssetClass>,
) -> Result<AssetClassesView> {
    let named = classes
        .into_iter()
        .map(|mut class| {
            if class.id.trim().is_empty() {
                class.id = asset_class_id(class.name.trim());
            }
            class
        })
        .collect();
    let AssetClassesUpdate { classes, known } = plan_set_asset_classes(named)?;
    // Baskets leave by the saved list: an empty one clears them all (the
    // preset is back), while `known` only sweeps links and store assignments.
    let order: Vec<String> = classes.iter().map(|class| class.id.clone()).collect();
    vault.transact(|tx| {
        let doc = tx.doc_mut();
        doc.asset_class_of
            .retain(|link| known.contains(&link.class_id));
        for store in &mut doc.stores {
            if store
                .asset_class
                .as_ref()
                .is_some_and(|id| !known.contains(id))
            {
                store.asset_class = None;
            }
        }
        let stale: Vec<String> = tx
            .collection::<AssetClass>()
            .iter()
            .filter(|class| !order.contains(&class.id))
            .map(|class| class.id.clone())
            .collect();
        let mut stored = tx.collection::<AssetClass>();
        for id in &stale {
            stored.remove(id)?;
        }
        for class in classes {
            if stored.get(&class.id).is_some() {
                stored.update(&class.id.clone(), |slot| *slot = class)?;
            } else {
                stored.insert(class)?;
            }
        }
        tx.doc_mut()
            .asset_classes
            .sort_by_key(|class| order.iter().position(|id| *id == class.id));
        Ok(())
    })?;
    Ok(asset_classes(&vault.doc))
}

/// Put an asset or a store in a basket, or take it out with `None`.
/// Assigning to a preset basket saves the preset first.
pub fn assign_asset_class(
    vault: &mut UnlockedVault,
    target: AssetClassTarget,
    class_id: Option<String>,
) -> Result<AssetClassesView> {
    check_assign_asset_class(&vault.doc, &target, class_id.as_deref())?;
    vault.transact(|tx| {
        if tx.doc().asset_classes.is_empty() && class_id.is_some() {
            for class in preset_classes() {
                tx.collection::<AssetClass>().insert(class)?;
            }
        }
        let doc = tx.doc_mut();
        match target {
            AssetClassTarget::Asset { asset_key } => {
                doc.asset_class_of
                    .retain(|link| link.asset_key != asset_key);
                if let Some(class_id) = class_id {
                    doc.asset_class_of.push(AssetClassLink {
                        asset_key,
                        class_id,
                    });
                }
            }
            AssetClassTarget::Store { store_id } => {
                if let Some(store) = doc.stores.iter_mut().find(|store| store.id == store_id) {
                    store.asset_class = class_id;
                }
            }
        }
        Ok(())
    })?;
    Ok(asset_classes(&vault.doc))
}
