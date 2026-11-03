//! How each part of [`crate::VaultDoc`] syncs. Every top-level field is
//! either a [`Collection`] here or listed in [`LOCAL`]; a test fails when a new
//! field has no rule, so nothing silently stays behind on one device.

/// How records of a collection merge between devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// Records by key. Each field: the later change wins. A deletion
    /// travels as a tombstone and beats every change made before it.
    Records,
    /// A capped history: merged like [`Rule::Records`], but trimming the
    /// oldest is not a deletion. After a merge the log is ordered by `order`
    /// and cut to `cap`.
    Log { cap: usize, order: &'static str },
    /// Points that never change once written (cached prices, rates,
    /// discarded ids): the union of both sides, no stamps. `cap` keeps the
    /// newest when set.
    Union { cap: Option<usize> },
    /// A single object (settings, exchange keys): its fields merge like a
    /// record's.
    Single,
}

#[derive(Debug, Clone, Copy)]
pub struct Collection {
    /// Field of `VaultDoc`.
    pub name: &'static str,
    pub rule: Rule,
    /// Dotted paths whose values, joined by `|`, identify a record. `*`
    /// steps into an externally tagged enum (`{"topup": {...}}`). Empty: the
    /// record itself is its key (a plain string).
    pub key: &'static [&'static str],
    /// Object fields split one level further, so editing a transaction's
    /// comment on one device and its categories on another both survive.
    pub flatten: &'static [&'static str],
    /// Fields that belong to the device and never travel.
    pub skip: &'static [&'static str],
    /// Field holding an RFC3339 time; the first sync sends only recent
    /// points of a [`Rule::Union`] collection.
    pub age: Option<&'static str>,
    /// Holds credentials (API keys, sealing keys): the window, the batches
    /// and the journal show no label or value of its records.
    pub secret: bool,
    /// Fields of an ordinary collection that hold a credential (a URL with a
    /// token, a stored value): `before`/`after` show a mask for them.
    pub secret_fields: &'static [&'static str],
    /// The record is one value with one stamp (a [`Rule::Single`] whose
    /// fields only make sense together, or that may be `null`): the later
    /// write wins as a whole, never field by field.
    pub whole: bool,
    /// Derived data each device refreshes on its own (cached prices and
    /// rates): it still unions in silently, but never reaches the question,
    /// the window or the journal.
    pub cache: bool,
}

const fn records(name: &'static str, key: &'static [&'static str]) -> Collection {
    Collection {
        name,
        rule: Rule::Records,
        key,
        flatten: &[],
        skip: &[],
        age: None,
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: false,
    }
}

const fn log(
    name: &'static str,
    key: &'static [&'static str],
    cap: usize,
    order: &'static str,
) -> Collection {
    Collection {
        name,
        rule: Rule::Log { cap, order },
        key,
        flatten: &[],
        skip: &[],
        age: None,
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: false,
    }
}

const fn single(name: &'static str, skip: &'static [&'static str]) -> Collection {
    Collection {
        name,
        rule: Rule::Single,
        key: &[],
        flatten: &[],
        skip,
        age: None,
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: false,
    }
}

const ID: &[&str] = &["id"];

/// Fields of `VaultDoc` that never sync: identity and schema (checked
/// instead), holdings (derived from transactions, re-derived after a merge)
/// and the sync bookkeeping itself.
pub const LOCAL: &[&str] = &["version", "id", "created_at", "holdings", "sync"];

/// Settings that describe the device, kept in the app config since sync;
/// the vault's old copies stay but must not travel.
pub const DEVICE_SETTINGS: &[&str] = &[
    "display_decimals",
    "fx_refresh_interval_sec",
    "minimize_to_tray",
];

/// Most days of cached prices and rates the first sync sends.
pub const UNION_DAYS: u64 = 90;

pub static COLLECTIONS: &[Collection] = &[
    single("settings", DEVICE_SETTINGS),
    records("stores", ID),
    Collection {
        flatten: &["meta"],
        ..records("transactions", &["*.meta.id"])
    },
    records("categories", &["key"]),
    Collection {
        name: "price_cache",
        rule: Rule::Union { cap: None },
        key: &["asset_key", "ts"],
        flatten: &[],
        skip: &[],
        age: Some("ts"),
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: true,
    },
    Collection {
        name: "fx_cache",
        rule: Rule::Union { cap: None },
        key: &["currency", "ts"],
        flatten: &[],
        skip: &[],
        age: Some("ts"),
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: true,
    },
    Collection {
        secret: true,
        ..single("exchanges", &[])
    },
    single("emergency_fund", &[]),
    Collection {
        whole: true,
        ..single("health_last", &[])
    },
    records("pending_imports", &["exec_id"]),
    records("contacts", ID),
    records("budgets", ID),
    records("goals", ID),
    records("reminders", ID),
    records("price_alerts", ID),
    records("categorization_rules", ID),
    records("utility_items", ID),
    records("meter_readings", ID),
    records("enrichments", ID),
    records("manual_prices", &["asset_key"]),
    records("envelopes", ID),
    records("debt_terms", ID),
    records("shared_events", ID),
    records("key_rates", &["date"]),
    log(
        "notification_log",
        &["key"],
        crate::model::notify::MAX_NOTICE_LOG,
        "delivered_at",
    ),
    records("workflows", ID),
    log(
        "workflow_runs",
        ID,
        crate::workflow::model::MAX_WORKFLOW_RUNS,
        "started_at",
    ),
    Collection {
        secret_fields: &["value"],
        ..records("workflow_vars", &["scope", "name"])
    },
    // The queue, waits and trigger marks sync so a new main device picks
    // them up; only the main device runs them.
    records("workflow_queue", ID),
    // Marks merge per trigger (an older device stamps the whole map: see
    // `merge::legacy_head`); the cursor belongs to the device.
    Collection {
        flatten: &["marks"],
        skip: &["tx_cursor", "tx_index"],
        ..records("workflow_state", &["workflow_id"])
    },
    records("workflow_waits", ID),
    records("calc_scenarios", ID),
    // A revised month replaces the old reading instead of piling up.
    records("inflation_points", &["currency", "month"]),
    records("asset_classes", ID),
    records("asset_class_of", &["asset_key"]),
    records("valuations", ID),
    // One link per operation: a new property replaces the old one.
    records("tx_properties", &["tx_id"]),
    Collection {
        secret_fields: &["url"],
        ..records("data_sources", ID)
    },
    records("asset_price_sources", &["asset_key"]),
    records("deposit_rate_sources", &["store_id"]),
    // The records and their keys; the sealed bytes stay on each device.
    records("attachments", ID),
    Collection {
        secret: true,
        ..records("attachment_keys", ID)
    },
    log(
        "import_runs",
        ID,
        crate::model::vault::MAX_IMPORT_RUNS,
        "at",
    ),
    Collection {
        name: "discarded_imports",
        rule: Rule::Union {
            cap: Some(crate::model::vault::MAX_DISCARDED_IMPORTS),
        },
        key: &[],
        flatten: &[],
        skip: &[],
        age: None,
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: false,
    },
];

/// Whether the collection holds credentials (see [`Collection::secret`]).
pub fn is_secret(name: &str) -> bool {
    find(name).is_some_and(|coll| coll.secret)
}

/// Whether `field` of the collection holds a credential (see
/// [`Collection::secret_fields`]); a secret collection hides all of them.
pub fn is_secret_field(name: &str, field: &str) -> bool {
    find(name).is_some_and(|coll| coll.secret || coll.secret_fields.contains(&field))
}

pub fn find(name: &str) -> Option<&'static Collection> {
    COLLECTIONS.iter().find(|coll| coll.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vault_field_has_a_sync_rule() {
        let doc = serde_json::to_value(crate::VaultDoc::default()).expect("doc");
        let fields = doc.as_object().expect("object");
        for name in fields.keys() {
            assert!(
                find(name).is_some() || LOCAL.contains(&name.as_str()),
                "VaultDoc.{name} has no sync rule: add it to sync::registry"
            );
        }
        for coll in COLLECTIONS {
            assert!(
                fields.contains_key(coll.name),
                "{} is not a field",
                coll.name
            );
        }
    }

    #[test]
    fn device_settings_exist() {
        let settings =
            serde_json::to_value(crate::model::vault::Settings::default()).expect("settings");
        for name in DEVICE_SETTINGS {
            assert!(settings.get(name).is_some(), "Settings.{name} is gone");
        }
    }
}
