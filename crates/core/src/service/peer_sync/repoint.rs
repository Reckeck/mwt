//! Catalog 96: what pointed to a removed duplicate of an operation points to
//! the copy that stays.
//!
//! A dedupe deletes the losing copy of a generated operation. Its attachments,
//! property link, enrichments, the payments of shared events and the
//! reconcile mark of an exchange sync must not be left dangling (an ownerless
//! attachment would be pruned on the next open, files included). Only the
//! fields listed on [`UnlockedVault::repoint_tx`] are touched, one by one: no
//! whole-document string replace, an id of another kind could be equal.

use std::collections::BTreeMap;

use super::*;
use crate::model::entity_ref::EntityKind;

impl UnlockedVault {
    /// Move every reference to the operation `old` onto the operation `new`.
    /// Returns whether anything changed. The references are:
    /// - `attachments[].owner` (kind `Tx`);
    /// - `tx_properties[].tx_id` (the link of `old` is dropped when `new`
    ///   already has one: an operation links to one property);
    /// - `enrichments[].tx_id`;
    /// - `shared_events[].items[].purchases[].tx_id`;
    /// - `shared_events[].settlements[].tx_id`;
    /// - `import_runs[].reconcile_tx_id`.
    pub(in crate::service) fn repoint_tx(&mut self, old: &str, new: &str) -> bool {
        if old == new {
            return false;
        }
        let mut changed = false;
        for item in &mut self.doc.attachments {
            if item.owner.kind == EntityKind::Tx && item.owner.id == old {
                item.owner.id = new.to_string();
                changed = true;
            }
        }
        let links = self.doc.tx_properties.clone();
        if self.doc.tx_properties.iter().any(|link| link.tx_id == new) {
            self.doc.tx_properties.retain(|link| link.tx_id != old);
        } else {
            for link in &mut self.doc.tx_properties {
                if link.tx_id == old {
                    link.tx_id = new.to_string();
                }
            }
        }
        changed |= self.doc.tx_properties != links;
        for record in &mut self.doc.enrichments {
            changed |= swap(&mut record.tx_id, old, new);
        }
        for event in &mut self.doc.shared_events {
            for item in &mut event.items {
                for purchase in &mut item.purchases {
                    changed |= swap_opt(&mut purchase.tx_id, old, new);
                }
            }
            for settlement in &mut event.settlements {
                changed |= swap_opt(&mut settlement.tx_id, old, new);
            }
        }
        for run in &mut self.doc.import_runs {
            changed |= swap_opt(&mut run.reconcile_tx_id, old, new);
        }
        changed
    }

    /// [`Self::repoint_tx`] for every pair, as one tracked edit of this
    /// device: the records it changed are stamped (so the other devices get
    /// them) and recorded in the batch for its undo.
    pub(super) fn repoint_removed_copies(
        &mut self,
        pairs: &[(String, String)],
        batch_id: &str,
    ) -> Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }
        let pre = view::records(&self.doc_value()?);
        let mut moved = false;
        for (old, new) in pairs {
            moved |= self.repoint_tx(old, new);
        }
        if !moved {
            return Ok(());
        }
        let post = view::records(&self.doc_value()?);
        let touched: Vec<String> = pre
            .keys()
            .chain(post.keys())
            .filter(|key| pre.get(*key) != post.get(*key))
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut before_states: BTreeMap<String, RecordState> = BTreeMap::new();
        for key in &touched {
            before_states.insert(key.clone(), self.state_of(&pre, key));
        }
        let changed = self.track_changes()?;
        let settled = view::records(&self.doc_value()?);
        let Some(position) = self
            .doc
            .sync
            .batches
            .iter()
            .position(|batch| batch.id == batch_id)
        else {
            return Ok(());
        };
        for key in changed {
            let Some(prior) = before_states.remove(&key) else {
                continue;
            };
            let entry = BatchEntry {
                after: self.state_of(&settled, &key).marks(),
                before: prior,
                key,
            };
            let kept = &mut self.doc.sync.batches[position].entries;
            match kept.iter_mut().find(|known| known.key == entry.key) {
                Some(known) => known.after = entry.after,
                None => kept.push(entry),
            }
        }
        Ok(())
    }
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
