//! Catalog 96: what pointed to a removed duplicate of an operation points to
//! the copy that stays.
//!
//! A dedupe deletes the losing copy of a generated operation. Its attachments,
//! property link, enrichments, the payments of shared events and the
//! reconcile mark of an exchange sync must not be left dangling (an ownerless
//! attachment would be pruned on the next open, files included). Only the
//! fields listed on [`repoint_tx`] are touched, one by one: no
//! whole-document string replace, an id of another kind could be equal.

use crate::model::entity_ref::EntityKind;
use crate::model::vault::VaultDoc;

/// Move every reference to the operation `old` onto the operation `new`.
/// Returns whether anything changed. The references are:
/// - `attachments[].owner` (kind `Tx`);
/// - `tx_properties[].tx_id` (the link of `old` is dropped when `new`
///   already has one: an operation links to one property);
/// - `enrichments[].tx_id`;
/// - `shared_events[].items[].purchases[].tx_id`;
/// - `shared_events[].settlements[].tx_id`;
/// - `import_runs[].reconcile_tx_id`.
pub fn repoint_tx(doc: &mut VaultDoc, old: &str, new: &str) -> bool {
    if old == new {
        return false;
    }
    let mut changed = false;
    for item in &mut doc.attachments {
        if item.owner.kind == EntityKind::Tx && item.owner.id == old {
            item.owner.id = new.to_string();
            changed = true;
        }
    }
    let links = doc.tx_properties.clone();
    if doc.tx_properties.iter().any(|link| link.tx_id == new) {
        doc.tx_properties.retain(|link| link.tx_id != old);
    } else {
        for link in &mut doc.tx_properties {
            if link.tx_id == old {
                link.tx_id = new.to_string();
            }
        }
    }
    changed |= doc.tx_properties != links;
    for record in &mut doc.enrichments {
        changed |= swap(&mut record.tx_id, old, new);
    }
    for event in &mut doc.shared_events {
        for item in &mut event.items {
            for purchase in &mut item.purchases {
                changed |= swap_opt(&mut purchase.tx_id, old, new);
            }
        }
        for settlement in &mut event.settlements {
            changed |= swap_opt(&mut settlement.tx_id, old, new);
        }
    }
    for run in &mut doc.import_runs {
        changed |= swap_opt(&mut run.reconcile_tx_id, old, new);
    }
    changed
}

fn swap(field: &mut String, old: &str, new: &str) -> bool {
    if field == old {
        *field = new.to_string();
        return true;
    }
    false
}

fn swap_opt(field: &mut Option<String>, old: &str, new: &str) -> bool {
    field.as_mut().is_some_and(|value| swap(value, old, new))
}
