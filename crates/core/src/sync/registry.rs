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

/// An entry of an array inside the item of a split array, see
/// [`Collection::inner_entry`].
#[derive(Debug, Clone, Copy)]
pub struct InnerEntry<'name> {
    pub head: &'static str,
    pub field: &'static str,
    /// Key path of the inner items.
    pub path: &'static str,
    /// Name of the item entry (`items.<id>`).
    pub parent: &'name str,
    /// Key of the inner item.
    pub key: &'name str,
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
    /// Array fields split into one entry per item, as `(field, key path)`:
    /// the path (dotted, like `key`) names the item inside the array. A path
    /// may hold several dotted paths joined by `+` (the key of an adjustment
    /// line is its whole asset); their texts are joined by `|`. Two
    /// devices adding or editing different items both keep their change, and a
    /// removed item travels as the stamp of its entry.
    pub flatten_by: &'static [(&'static str, &'static str)],
    /// Arrays inside the items of a split array, as `(array, field, key
    /// path)`: the `purchases` of an `items` entry. Each one becomes an entry
    /// of its own, `items.<item key>.purchases.<key>`, with its own stamp; the
    /// item entry keeps the other fields. An entry without a stamp takes the
    /// stamp of the item it sits in, so stamps written before the split still
    /// say when a purchase last changed.
    pub flatten_inner: &'static [(&'static str, &'static str, &'static str)],
    /// Fields computed from a split array, as `(field, array)`: `allocated`
    /// is the sum of `moves`. Between devices that know the array the field is
    /// never a change of its own: it is re-derived after a merge and sent with
    /// the stamp of the newest entry, so a recomputed value cannot be mistaken
    /// for an edit. A device that does not know the array just writes the
    /// field; the merge turns that into one correction entry (`derived.rs`).
    pub derived: &'static [(&'static str, &'static str)],
    /// Item fields of a split array that every device recomputes from its own
    /// data, as `(array, item field)`: an adjustment line's `before` is the
    /// ledger balance, so a device that receives a backdated operation
    /// rewrites it. The field stays in the stored and sent value (an older
    /// device requires it), but it is never a change: `prints`, the merge and
    /// the plan compare [`comparable`](super::view::comparable) forms, which
    /// leave it out.
    pub recomputed: &'static [(&'static str, &'static str)],
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

impl Collection {
    /// Key path of the items of the array field `name`, when it is split.
    pub fn item_key(&self, name: &str) -> Option<&'static str> {
        self.flatten_by
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, path)| *path)
    }

    /// The inner array of an entry `head.<key>.<field>.<inner key>`, when
    /// `name` is one: `(head, key, field, inner key path, inner key)`.
    pub fn inner_entry<'name>(&self, name: &'name str) -> Option<InnerEntry<'name>> {
        self.flatten_inner.iter().find_map(|(head, field, path)| {
            let rest = name.strip_prefix(head)?.strip_prefix('.')?;
            let marker = format!(".{field}.");
            let at = rest.find(&marker)?;
            Some(InnerEntry {
                head,
                field,
                path,
                parent: &name[..head.len() + 1 + at],
                key: &rest[at + marker.len()..],
            })
        })
    }

    /// Fields that sync as separate entries (`field.<entry>`).
    pub fn heads(&self) -> impl Iterator<Item = &'static str> {
        self.flatten
            .iter()
            .copied()
            .chain(self.flatten_by.iter().map(|(field, _)| *field))
    }
}

const fn records(name: &'static str, key: &'static [&'static str]) -> Collection {
    Collection {
        name,
        rule: Rule::Records,
        key,
        flatten: &[],
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
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
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
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
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
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
/// the sync bookkeeping itself, and exchange API keys: they live in the
/// device secrets file (`crypto::device_secrets`) and never leave the device;
/// the field is only an in-memory index of them, never written.
pub const LOCAL: &[&str] = &[
    "version",
    "id",
    "created_at",
    "holdings",
    "sync",
    "exchanges",
];

/// Collections that synced credentials before they became local: entries of
/// them in an old sync history are still masked.
const LEGACY_SECRET: &[&str] = &["exchanges"];

/// Settings that describe the device and must not travel. The first three
/// live in the app config since sync, the vault's old copies stay. The dashboard
/// layout, its period and the pins stay in the vault on purpose: they hold ids
/// of that vault and the text of saved filters, and the app config is plain JSON.
pub const DEVICE_SETTINGS: &[&str] = &[
    "display_decimals",
    "fx_refresh_interval_sec",
    "minimize_to_tray",
    "dashboard_widgets",
    "dashboard_period",
    "pinned",
    "inflation_adopted",
];

/// What the `settings` rule skips: `DEVICE_SETTINGS` and the device parts of
/// an object setting (the system notice switch and the locked-screen privacy
/// switch, which the lock-screen snapshot reads on this device only).
const SETTINGS_SKIP: &[&str] = &[
    "display_decimals",
    "fx_refresh_interval_sec",
    "minimize_to_tray",
    "dashboard_widgets",
    "dashboard_period",
    "pinned",
    "inflation_adopted",
    "notifications.system",
    "notifications.hide_details_when_locked",
];

/// Where a setting lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// In the vault and synced: every device of the group sees the same value.
    Group,
    /// Belongs to the device: never synced (listed in the `skip` of the
    /// `settings` record).
    Device,
}

/// The decision for every field of `Settings`, so a new field cannot travel
/// by accident: `every_setting_has_a_scope` is red until it has a line here.
/// A `Device` name must also be in the `skip` of the `settings` rule
/// (`device_fields_are_skipped`).
pub const SETTINGS_SCOPE: &[(&str, Scope)] = &[
    // Moved to the app config; the vault's old copies stay and do not travel.
    ("display_decimals", Scope::Device),
    ("fx_refresh_interval_sec", Scope::Device),
    ("minimize_to_tray", Scope::Device),
    // Each device's own, kept in its encrypted vault file.
    ("dashboard_widgets", Scope::Device),
    ("dashboard_period", Scope::Device),
    ("pinned", Scope::Device),
    // Plan parameters, notification rules, inflation and onboarding progress are the group's.
    ("savings_rate", Scope::Group),
    ("daily_budget_mode", Scope::Group),
    ("budget_currency", Scope::Group),
    // The object is split: the two switches below are the device's, the rest
    // (`default_time`, `categories`) is the group's.
    ("notifications", Scope::Group),
    ("notifications.system", Scope::Device),
    ("notifications.hide_details_when_locked", Scope::Device),
    ("notifications.default_time", Scope::Group),
    ("notifications.categories", Scope::Group),
    ("budget_configured", Scope::Group),
    ("onboarding_skipped", Scope::Group),
    ("inflation_mode", Scope::Group),
    ("inflation_source", Scope::Group),
    // The flat rates by currency: the group's, so every device computes with
    // one table.
    ("inflation_manual", Scope::Group),
    // This device has given its old table to the group; a peer's flag says
    // nothing about the rates kept here.
    ("inflation_adopted", Scope::Device),
    ("manual_income", Scope::Group),
    ("manual_expenses", Scope::Group),
    ("categories_rev", Scope::Group),
    ("freedom_rate", Scope::Group),
    ("carry_since", Scope::Group),
    ("onboarding_checklist_hidden", Scope::Group),
    ("crypto", Scope::Group),
    ("schedule_timezone", Scope::Group),
];

/// Most days of cached prices and rates the first sync sends.
pub const UNION_DAYS: u64 = 90;

/// Identity of an adjustment line: the whole asset reference.
const LINE_KEY: &str = "asset.symbol+asset.coingecko_id+asset.chain+asset.contract";

pub static COLLECTIONS: &[Collection] = &[
    // Per-currency maps merge per currency: two devices choosing the
    // inflation of different currencies both keep their choice.
    Collection {
        flatten: &[
            "inflation_mode",
            "inflation_source",
            "inflation_manual",
            "notifications",
        ],
        // Each category is an entry of its own: two devices switching
        // different categories keep both.
        flatten_by: &[("notifications.categories", "category")],
        ..single("settings", SETTINGS_SKIP)
    },
    // Exchange and property details merge per field; a car's papers and
    // mileage one level further. The terminal of an exchange (watchlist,
    // drawings, layout) belongs to the device and never travels.
    Collection {
        flatten: &["exchange", "property", "property.car"],
        skip: &["exchange.terminal"],
        ..records("stores", ID)
    },
    Collection {
        flatten: &["meta"],
        // A correction syncs per asset; `before` is the ledger's, not an edit.
        flatten_by: &[("lines", LINE_KEY)],
        recomputed: &[("lines", "before")],
        ..records("transactions", &["*.meta.id"])
    },
    records("categories", &["key"]),
    Collection {
        name: "price_cache",
        rule: Rule::Union { cap: None },
        key: &["asset_key", "ts"],
        flatten: &[],
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
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
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
        skip: &[],
        age: Some("ts"),
        secret: false,
        secret_fields: &[],
        whole: false,
        cache: true,
    },
    // The accounts of the fund are a set: `scope` splits one level, and its
    // `store_ids` splits into one entry per id (the item is its own key).
    Collection {
        flatten: &["scope"],
        flatten_by: &[("scope.store_ids", "")],
        ..single("emergency_fund", &[])
    },
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
    // The allocation is a journal of moves, so two top-ups both count.
    Collection {
        flatten_by: &[("moves", "id")],
        derived: &[("allocated", "moves")],
        ..records("envelopes", ID)
    },
    records("debt_terms", ID),
    // People, purchases and settlements added on different devices all stay.
    Collection {
        flatten_by: &[
            ("participants", "id"),
            ("items", "id"),
            ("settlements", "id"),
            ("adjustments", "participant_id"),
            ("removed_with_payment", "id"),
        ],
        flatten_inner: &[("items", "purchases", "id")],
        ..records("shared_events", ID)
    },
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
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
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
        flatten_by: &[],
        flatten_inner: &[],
        derived: &[],
        recomputed: &[],
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
    LEGACY_SECRET.contains(&name) || find(name).is_some_and(|coll| coll.secret)
}

/// Whether `field` of the collection holds a credential (see
/// [`Collection::secret_fields`]); a secret collection hides all of them.
pub fn is_secret_field(name: &str, field: &str) -> bool {
    LEGACY_SECRET.contains(&name)
        || find(name).is_some_and(|coll| coll.secret || coll.secret_fields.contains(&field))
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

    fn settings_keys() -> Vec<String> {
        let settings =
            serde_json::to_value(crate::model::vault::Settings::default()).expect("settings");
        settings
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn every_setting_has_a_scope() {
        let keys = settings_keys();
        for name in &keys {
            assert!(
                SETTINGS_SCOPE.iter().any(|(field, _)| field == name),
                "Settings.{name} has no scope: decide Group or Device in sync::registry::SETTINGS_SCOPE"
            );
        }
        let settings =
            serde_json::to_value(crate::model::vault::Settings::default()).expect("settings");
        for (name, _) in SETTINGS_SCOPE {
            assert!(
                crate::sync::view::lookup(&settings, name).is_some(),
                "SETTINGS_SCOPE names {name}, which is not a field of Settings"
            );
        }
    }

    /// An object setting that is split (`flatten`) decides each of its parts.
    #[test]
    fn every_part_of_a_split_setting_has_a_scope() {
        let settings =
            serde_json::to_value(crate::model::vault::Settings::default()).expect("settings");
        let rule = find("settings").expect("settings rule");
        for head in rule.flatten {
            let Some(inner) = settings.get(head).and_then(|part| part.as_object()) else {
                continue;
            };
            // The per-currency maps start empty; only `notifications` has parts.
            for part in inner.keys() {
                let path = format!("{head}.{part}");
                assert!(
                    SETTINGS_SCOPE.iter().any(|(field, _)| *field == path),
                    "Settings.{path} has no scope: decide Group or Device in sync::registry::SETTINGS_SCOPE"
                );
            }
        }
    }

    #[test]
    fn device_fields_are_skipped() {
        let skip = find("settings").expect("settings rule").skip;
        for (name, scope) in SETTINGS_SCOPE {
            match scope {
                Scope::Device => assert!(
                    skip.contains(name),
                    "Settings.{name} is a device setting but is not in the skip of the settings rule"
                ),
                Scope::Group => assert!(
                    !skip.contains(name),
                    "Settings.{name} is a group setting but the settings rule skips it"
                ),
            }
        }
        for name in skip {
            assert!(
                SETTINGS_SCOPE
                    .iter()
                    .any(|(field, scope)| field == name && *scope == Scope::Device),
                "the settings rule skips {name}, which SETTINGS_SCOPE does not mark Device"
            );
        }
    }

    type Schema = serde_json::Value;

    /// The schemas a node stands for: `$ref`s followed, `anyOf`/`oneOf`/`allOf`
    /// flattened (an `Option<T>` is `anyOf [T, null]`).
    fn variants<'doc>(root: &'doc Schema, node: &'doc Schema) -> Vec<&'doc Schema> {
        if let Some(target) = node.get("$ref").and_then(|text| text.as_str()) {
            let found = target
                .strip_prefix("#/")
                .and_then(|path| path.split('/').try_fold(root, |at, step| at.get(step)));
            return found.map_or_else(Vec::new, |inner| variants(root, inner));
        }
        let mut out = vec![node];
        for key in ["anyOf", "oneOf", "allOf"] {
            if let Some(list) = node.get(key).and_then(|list| list.as_array()) {
                for inner in list {
                    out.extend(variants(root, inner));
                }
            }
        }
        out
    }

    /// The nodes one step below `nodes`; `*` is any property (a variant of a
    /// tagged enum).
    fn step<'doc>(root: &'doc Schema, nodes: &[&'doc Schema], name: &str) -> Vec<&'doc Schema> {
        nodes
            .iter()
            .flat_map(|node| variants(root, node))
            .filter_map(|variant| variant.get("properties")?.as_object())
            .flat_map(|props| {
                props
                    .iter()
                    .filter(|(field, _)| name == "*" || field.as_str() == name)
                    .map(|(_, child)| child)
            })
            .collect()
    }

    fn walk<'doc>(root: &'doc Schema, nodes: &[&'doc Schema], path: &str) -> Vec<&'doc Schema> {
        path.split('.')
            .fold(nodes.to_vec(), |at, name| step(root, &at, name))
    }

    /// The schemas of the items of the arrays `nodes` stand for.
    fn items<'doc>(root: &'doc Schema, nodes: &[&'doc Schema]) -> Vec<&'doc Schema> {
        nodes
            .iter()
            .flat_map(|node| variants(root, node))
            .filter_map(|variant| variant.get("items"))
            .flat_map(|item| variants(root, item))
            .collect()
    }

    /// What the model declares for a rule's paths. Records come from the
    /// schema of the document (every field, whatever the fixture holds); the
    /// `full_doc` fixture is only used to see that a record has its key.
    #[test]
    fn registry_paths_exist_in_the_model() {
        use super::super::view::record_key;
        let schema = crate::model::vault::json_schema();
        let doc = serde_json::to_value(crate::model::fixtures::full_doc()).expect("doc");
        for coll in COLLECTIONS {
            let name = coll.name;
            let top = schema["properties"]
                .get(name)
                .unwrap_or_else(|| panic!("{name} is not in the schema"));
            let top = variants(&schema, top);
            let records = if matches!(coll.rule, Rule::Single) {
                top
            } else {
                items(&schema, &top)
            };
            let records_before_tag = records.clone();
            // A tagged enum keeps its body one level down (`{"topup": {...}}`).
            let tagged = coll.key.first().is_some_and(|path| path.starts_with("*."));
            let bodies = if tagged {
                step(&schema, &records, "*")
            } else {
                records
            };
            assert!(
                !bodies.is_empty(),
                "{name}: the schema has no record for it"
            );

            let present = |path: &str| !walk(&schema, &bodies, path).is_empty();
            let item_has = |item: &Schema, key: &str| {
                key.split('+')
                    .filter(|part| !part.is_empty())
                    .all(|part| !walk(&schema, &[item], part).is_empty())
            };
            for path in coll.flatten {
                assert!(present(path), "{name}: flatten {path} is not in the model");
            }
            for path in coll.skip {
                assert!(present(path), "{name}: skip {path} is not in the model");
            }
            for (field, key) in coll.flatten_by {
                let entries = items(&schema, &walk(&schema, &bodies, field));
                assert!(
                    !entries.is_empty(),
                    "{name}: flatten_by {field} is not an array in the model"
                );
                for item in entries {
                    assert!(
                        item_has(item, key),
                        "{name}: flatten_by {field} key {key:?} is not in an item"
                    );
                }
            }
            for (head, field, key) in coll.flatten_inner {
                let outer = items(&schema, &walk(&schema, &bodies, head));
                let inner = items(&schema, &walk(&schema, &outer, field));
                assert!(
                    !inner.is_empty(),
                    "{name}: flatten_inner {head}.{field} is not an array in the model"
                );
                for item in inner {
                    assert!(
                        item_has(item, key),
                        "{name}: flatten_inner {head}.{field} key {key:?} is not in an entry"
                    );
                }
            }
            for (field, array) in coll.derived {
                assert!(
                    present(field),
                    "{name}: derived {field} is not in the model"
                );
                assert!(
                    present(array),
                    "{name}: derived from {array}, which is not in the model"
                );
            }
            for (array, field) in coll.recomputed {
                let entries = items(&schema, &walk(&schema, &bodies, array));
                assert!(
                    !entries.is_empty(),
                    "{name}: recomputed {array} is not an array in the model"
                );
                assert!(
                    entries.iter().any(|item| item_has(item, field)),
                    "{name}: recomputed {array}.{field} is not in the model"
                );
            }

            // The key paths are fields of the record (`*` steps into a variant).
            for path in coll.key {
                assert!(
                    !walk(&schema, &records_before_tag, path).is_empty(),
                    "{name}: key {path} is not in the model"
                );
            }
            // A record of the fixture resolves its key (the fixture does not
            // fill every collection; the schema check above covers the rest).
            for record in doc[name].as_array().map_or(&[][..], Vec::as_slice) {
                assert!(
                    record_key(coll, record).is_some(),
                    "{name}: a record has no value at the key {:?}: {record}",
                    coll.key
                );
            }
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
