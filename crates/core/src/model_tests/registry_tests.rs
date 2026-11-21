//! The sync registry against the model; it reads `sync::view`, a higher layer.

use crate::model::registry::*;

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

/// The value at a dotted path (`a.b`) of the serialized `Settings`, if any.
fn setting_at<'a>(settings: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    name.split('.')
        .try_fold(settings, |node, step| node.get(step))
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
            "Settings.{name} has no scope: decide Group or Device in model::registry::SETTINGS_SCOPE"
        );
    }
    let settings =
        serde_json::to_value(crate::model::vault::Settings::default()).expect("settings");
    for (name, _) in SETTINGS_SCOPE {
        assert!(
            setting_at(&settings, name).is_some(),
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
                "Settings.{path} has no scope: decide Group or Device in model::registry::SETTINGS_SCOPE"
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
    use crate::sync::view::record_key;
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
        assert!(
            setting_at(&settings, name).is_some(),
            "Settings.{name} is gone"
        );
    }
}
