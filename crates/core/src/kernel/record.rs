//! Records: the kinds of `VaultDoc` that are a `Vec` of items with an `id`.
//! [`records!`] is the one list of them; it generates the [`Record`] impls and
//! the [`record_kinds`] table. It does not replace `sync::registry`, a test
//! ties the two. `Single`, `Union` and the kinds keyed by something other
//! than `id` stay with the sync registry only.

use crate::VaultDoc;

/// An item of a `Vec` field of [`VaultDoc`] that is found by its `id`.
pub trait Record: Sized {
    /// The `VaultDoc` field, and the name in the sync registry.
    const COLLECTION: &'static str;

    fn id(&self) -> &str;

    fn slot(doc: &mut VaultDoc) -> &mut Vec<Self>;

    fn slot_ref(doc: &VaultDoc) -> &Vec<Self>;
}

/// One line of the registry: the collection and whether only level 3 has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordKind {
    pub collection: &'static str,
    pub level3_only: bool,
}

macro_rules! records {
    ($($item:ty => $field:ident, $level3_only:literal;)*) => {
        $(
            impl Record for $item {
                const COLLECTION: &'static str = stringify!($field);

                fn id(&self) -> &str {
                    &self.id
                }

                fn slot(doc: &mut VaultDoc) -> &mut Vec<Self> {
                    &mut doc.$field
                }

                fn slot_ref(doc: &VaultDoc) -> &Vec<Self> {
                    &doc.$field
                }
            }
        )*

        const RECORD_KINDS: &[RecordKind] = &[
            $(RecordKind {
                collection: stringify!($field),
                level3_only: $level3_only,
            },)*
        ];
    };
}

records! {
    crate::model::store::Store => stores, false;
    crate::model::contact::Contact => contacts, false;
    crate::model::planning::Budget => budgets, false;
    crate::model::planning::Goal => goals, false;
    crate::model::planning::Reminder => reminders, false;
    crate::model::planning::PriceAlert => price_alerts, false;
    crate::model::planning::CategorizationRule => categorization_rules, false;
    crate::model::planning::UtilityItem => utility_items, false;
    crate::model::planning::MeterReading => meter_readings, false;
    crate::model::enrichment::EnrichmentRecord => enrichments, false;
    crate::model::planning::Envelope => envelopes, false;
    crate::model::contact::DebtTerms => debt_terms, false;
    crate::workflow::Workflow => workflows, false;
    crate::workflow::WorkflowRun => workflow_runs, false;
    crate::workflow::QueuedActions => workflow_queue, false;
    crate::workflow::model::WorkflowWait => workflow_waits, false;
    crate::model::planning::CalcScenario => calc_scenarios, false;
    crate::model::planning::AssetClass => asset_classes, false;
    crate::model::vault::Valuation => valuations, false;
    crate::model::datasource::DataSource => data_sources, false;
    crate::model::attachment::Attachment => attachments, false;
    crate::model::attachment::AttachmentKey => attachment_keys, false;
    crate::model::shared_event::SharedEvent => shared_events, false;
    crate::model::vault::ImportRun => import_runs, false;
    crate::model::cryptotrack::CryptoAccount => crypto_accounts, true;
    crate::model::cryptotrack::CryptoTx => crypto_events, true;
    crate::model::cryptotrack::CryptoOverride => crypto_overrides, true;
    crate::model::cryptotrack::CryptoPosition => crypto_positions, true;
    crate::model::cryptotrack::CryptoSnapshot => crypto_snapshots, true;
    crate::model::cryptotrack::CryptoWipe => crypto_wiped_at, true;
}

/// Every record kind with its level-3 flag.
pub fn record_kinds() -> &'static [RecordKind] {
    RECORD_KINDS
}

/// The crypto fields of `VaultDoc` that are not records: a token is keyed by
/// chain and contract, the prices are a union, the cursors stay on the device.
const LEVEL3_NON_RECORDS: &[&str] = &["crypto_tokens", "crypto_prices", "crypto_cursors"];

/// Every `VaultDoc` field of the crypto module: the level-3 records and the
/// crypto fields that are not records. One home for "what is level 3".
pub fn level3_collections() -> impl Iterator<Item = &'static str> {
    record_kinds()
        .iter()
        .filter(|kind| kind.level3_only)
        .map(|kind| kind.collection)
        .chain(LEVEL3_NON_RECORDS.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn level3_only_matches_the_crypto_prefix() {
        for kind in record_kinds() {
            assert_eq!(
                kind.level3_only,
                kind.collection.starts_with("crypto_"),
                "{}",
                kind.collection
            );
        }
    }

    #[test]
    fn level3_collections_are_the_crypto_fields_of_the_document() {
        let doc = serde_json::to_value(VaultDoc::default()).expect("doc");
        let mut from_doc: Vec<&str> = doc
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .filter(|name| name.starts_with("crypto_"))
            .collect();
        let mut listed: Vec<&str> = level3_collections().collect();
        from_doc.sort_unstable();
        listed.sort_unstable();
        assert_eq!(listed, from_doc);
    }

    #[test]
    fn slots_point_at_the_named_field() {
        let mut doc = VaultDoc::default();
        let value = serde_json::to_value(&doc).expect("doc");
        for kind in record_kinds() {
            assert!(value.get(kind.collection).is_some(), "{}", kind.collection);
        }
        assert_eq!(
            <crate::model::planning::Budget as Record>::COLLECTION,
            "budgets"
        );
        assert!(<crate::model::planning::Budget as Record>::slot(&mut doc).is_empty());
        assert!(<crate::model::planning::Budget as Record>::slot_ref(&doc).is_empty());
    }
}
