//! Reads of the store tree over a `VaultDoc`: no vault, clock or I/O. Every
//! walk stops at a missing parent or at a cycle left by a hand-edited vault.

use std::collections::HashSet;

use mwt_model::VaultDoc;
use mwt_model::{Store, StoreScope};

/// The store with this id.
pub fn find_store<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc Store> {
    doc.stores.iter().find(|store| store.id == id)
}

/// Parent chain from the root down to `id`'s parent. Stops at a missing
/// parent or at a cycle left behind by a hand-edited vault.
pub fn ancestors_of<'doc>(doc: &'doc VaultDoc, id: &str) -> Vec<&'doc Store> {
    let mut chain = Vec::new();
    let mut seen = HashSet::from([id.to_string()]);
    let mut parent = find_store(doc, id).and_then(|store| store.parent_id.as_deref());
    while let Some(parent_id) = parent {
        if !seen.insert(parent_id.to_string()) {
            break;
        }
        let Some(store) = find_store(doc, parent_id) else {
            break;
        };
        chain.push(store);
        parent = store.parent_id.as_deref();
    }
    chain.reverse();
    chain
}

/// 0 for a top-level wallet.
pub fn depth(doc: &VaultDoc, id: &str) -> usize {
    ancestors_of(doc, id).len()
}

/// Top-level wallet that `id` belongs to (itself for a root).
pub fn root_of(doc: &VaultDoc, id: &str) -> String {
    ancestors_of(doc, id)
        .first()
        .map(|store| store.id.clone())
        .unwrap_or_else(|| id.to_string())
}

/// Direct children of a store.
pub fn children_of<'doc>(doc: &'doc VaultDoc, parent_id: &str) -> Vec<&'doc Store> {
    doc.stores
        .iter()
        .filter(|child| child.parent_id.as_deref() == Some(parent_id))
        .collect()
}

/// Every descendant id of `root_id`, without `root_id` itself.
pub fn subtree_ids(doc: &VaultDoc, root_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::from([root_id.to_string()]);
    let mut queue = vec![root_id.to_string()];
    while let Some(id) = queue.pop() {
        for child in children_of(doc, &id) {
            if seen.insert(child.id.clone()) {
                result.push(child.id.clone());
                queue.push(child.id.clone());
            }
        }
    }
    result
}

/// Every store id a scope covers. An empty id list means the whole vault.
pub fn resolve_scope(doc: &VaultDoc, scope: &StoreScope) -> HashSet<String> {
    if scope.store_ids.is_empty() {
        return doc.stores.iter().map(|store| store.id.clone()).collect();
    }
    let mut out = HashSet::new();
    for id in &scope.store_ids {
        out.insert(id.clone());
        if scope.include_children {
            out.extend(subtree_ids(doc, id));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(id: &str, parent: Option<&str>) -> Store {
        Store {
            id: id.to_string(),
            parent_id: parent.map(str::to_string),
            ..Store::default()
        }
    }

    /// root -> mid -> leaf, plus a second root.
    fn doc() -> VaultDoc {
        VaultDoc {
            stores: vec![
                store("root", None),
                store("mid", Some("root")),
                store("leaf", Some("mid")),
                store("other", None),
            ],
            ..VaultDoc::default()
        }
    }

    /// `a` and `b` name each other as parent.
    fn cyclic_doc() -> VaultDoc {
        VaultDoc {
            stores: vec![store("a", Some("b")), store("b", Some("a"))],
            ..VaultDoc::default()
        }
    }

    fn ids(stores: Vec<&Store>) -> Vec<&str> {
        stores.into_iter().map(|node| node.id.as_str()).collect()
    }

    #[test]
    fn find_store_by_id() {
        let doc = doc();
        assert_eq!(
            find_store(&doc, "mid").map(|node| node.id.as_str()),
            Some("mid")
        );
        assert!(find_store(&doc, "nope").is_none());
    }

    #[test]
    fn ancestors_run_from_the_root_down() {
        let doc = doc();
        assert_eq!(ids(ancestors_of(&doc, "leaf")), ["root", "mid"]);
        assert!(ancestors_of(&doc, "root").is_empty());
        assert!(ancestors_of(&doc, "nope").is_empty());
    }

    #[test]
    fn ancestors_stop_at_a_cycle() {
        let doc = cyclic_doc();
        assert_eq!(ids(ancestors_of(&doc, "a")), ["b"]);
    }

    #[test]
    fn depth_counts_levels() {
        let doc = doc();
        assert_eq!(depth(&doc, "root"), 0);
        assert_eq!(depth(&doc, "leaf"), 2);
        assert_eq!(depth(&cyclic_doc(), "a"), 1);
    }

    #[test]
    fn root_of_names_the_top_wallet() {
        let doc = doc();
        assert_eq!(root_of(&doc, "leaf"), "root");
        assert_eq!(root_of(&doc, "root"), "root");
        assert_eq!(root_of(&doc, "nope"), "nope");
    }

    #[test]
    fn children_are_direct_only() {
        let doc = doc();
        assert_eq!(ids(children_of(&doc, "root")), ["mid"]);
        assert!(children_of(&doc, "leaf").is_empty());
    }

    #[test]
    fn subtree_excludes_the_root_and_stops_at_a_cycle() {
        let doc = doc();
        let mut found = subtree_ids(&doc, "root");
        found.sort();
        assert_eq!(found, ["leaf", "mid"]);
        let mut cyclic = subtree_ids(&cyclic_doc(), "a");
        cyclic.sort();
        assert_eq!(cyclic, ["b"]);
    }

    #[test]
    fn scope_resolves_ids_with_or_without_children() {
        let doc = doc();
        let whole = resolve_scope(&doc, &StoreScope::default());
        assert_eq!(whole.len(), 4);
        let with_children = resolve_scope(&doc, &StoreScope::store("mid", true));
        assert_eq!(
            with_children,
            HashSet::from(["mid".to_string(), "leaf".to_string()])
        );
        let alone = resolve_scope(&doc, &StoreScope::store("mid", false));
        assert_eq!(alone, HashSet::from(["mid".to_string()]));
    }
}
