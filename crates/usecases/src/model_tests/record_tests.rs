//! Record kinds against the sync registry: they need `sync`, a higher layer than `model`.

use crate::model::record::record_kinds;
use crate::sync::registry::{self, Rule};

#[test]
fn every_record_kind_is_in_the_sync_registry() {
    for kind in record_kinds() {
        let coll = registry::find(kind.collection)
            .unwrap_or_else(|| panic!("{} is not in the sync registry", kind.collection));
        assert!(
            matches!(coll.rule, Rule::Records | Rule::Log { .. }),
            "{} is not a records or log collection",
            kind.collection
        );
        assert_eq!(coll.key, ["id"], "{} is not keyed by id", kind.collection);
    }
}

#[test]
fn every_id_keyed_registry_collection_is_a_record_kind() {
    for coll in registry::COLLECTIONS {
        let is_record = matches!(coll.rule, Rule::Records | Rule::Log { .. });
        if is_record && coll.key == ["id"] {
            assert!(
                record_kinds()
                    .iter()
                    .any(|kind| kind.collection == coll.name),
                "{} is keyed by id in the sync registry but has no records! line",
                coll.name
            );
        }
    }
}
