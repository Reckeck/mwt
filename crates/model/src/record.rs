//! Records: the kinds of `VaultDoc` that are a `Vec` of items with an `id`.
//! [`records!`] is the one list of them; it generates the [`Record`] impls and
//! the [`record_kinds`] table. It does not replace `sync::registry`, a test
//! ties the two. `Single`, `Union` and the kinds keyed by something other
//! than `id` stay with the sync registry only.

use super::vault::VaultDoc;

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

#[macro_export]
macro_rules! records {
    ($($item:ty => $field:ident, $level3_only:literal;)*) => {
        $(
            impl $crate::Record for $item {
                const COLLECTION: &'static str = stringify!($field);

                fn id(&self) -> &str {
                    &self.id
                }

                fn slot(doc: &mut $crate::VaultDoc) -> &mut Vec<Self> {
                    &mut doc.$field
                }

                fn slot_ref(doc: &$crate::VaultDoc) -> &Vec<Self> {
                    &doc.$field
                }
            }
        )*

        const RECORD_KINDS: &[$crate::RecordKind] = &[
            $($crate::RecordKind {
                collection: stringify!($field),
                level3_only: $level3_only,
            },)*
        ];
    };
}

records! {
    super::store::Store => stores, false;
    super::contact::Contact => contacts, false;
    super::planning::Budget => budgets, false;
    super::planning::Goal => goals, false;
    super::planning::Reminder => reminders, false;
    super::planning::PriceAlert => price_alerts, false;
    super::planning::CategorizationRule => categorization_rules, false;
    super::planning::UtilityItem => utility_items, false;
    super::planning::MeterReading => meter_readings, false;
    super::enrichment::EnrichmentRecord => enrichments, false;
    super::planning::Envelope => envelopes, false;
    super::contact::DebtTerms => debt_terms, false;
    super::workflow::Workflow => workflows, false;
    super::workflow::WorkflowRun => workflow_runs, false;
    super::workflow::QueuedActions => workflow_queue, false;
    super::workflow::WorkflowWait => workflow_waits, false;
    super::planning::CalcScenario => calc_scenarios, false;
    super::planning::AssetClass => asset_classes, false;
    super::vault::Valuation => valuations, false;
    super::datasource::DataSource => data_sources, false;
    super::attachment::Attachment => attachments, false;
    super::attachment::AttachmentKey => attachment_keys, false;
    super::shared_event::SharedEvent => shared_events, false;
    super::vault::ImportRun => import_runs, false;
    super::cryptotrack::CryptoAccount => crypto_accounts, true;
    super::cryptotrack::CryptoTx => crypto_events, true;
    super::cryptotrack::CryptoOverride => crypto_overrides, true;
    super::cryptotrack::CryptoPosition => crypto_positions, true;
    super::cryptotrack::CryptoSnapshot => crypto_snapshots, true;
    super::cryptotrack::CryptoWipe => crypto_wiped_at, true;
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
        assert_eq!(<crate::planning::Budget as Record>::COLLECTION, "budgets");
        assert!(<crate::planning::Budget as Record>::slot(&mut doc).is_empty());
        assert!(<crate::planning::Budget as Record>::slot_ref(&doc).is_empty());
    }
}
