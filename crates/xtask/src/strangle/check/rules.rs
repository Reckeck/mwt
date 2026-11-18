//! The four rules of `strangle check`, each a function of a `Subject` that returns its findings.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use syn::{Block, Expr, Stmt};

use super::super::baseline::{self, production_methods};
use super::super::forward::forwarded_entries;
use super::super::home::{Home, USECASES, area_files, homes};
use super::Subject;

/// The token that marks a binding `mut`, with its trailing space.
const BINDING_MUT: &str = "mut ";

/// The signature without `mut` on parameter bindings (`mut name: T`): the delegate leaves the
/// binding out, so both sides are compared without it. `&mut T` and `&mut self` stay.
pub(super) fn without_binding_mut(signature: &str) -> String {
    let mut text = String::with_capacity(signature.len());
    let mut rest = signature;
    while let Some(at) = rest.find(BINDING_MUT) {
        let (before, after) = rest.split_at(at);
        text.push_str(before);
        let name_len = after[BINDING_MUT.len()..]
            .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .unwrap_or(0);
        let starts_param = text.ends_with('(') || text.ends_with(", ");
        let is_binding =
            starts_param && name_len > 0 && after[BINDING_MUT.len() + name_len..].starts_with(':');
        if !is_binding {
            text.push_str(BINDING_MUT);
        }
        rest = &after[BINDING_MUT.len()..];
    }
    text.push_str(rest);
    text
}

/// Rule 1: the baseline methods and the forward entries are the same set, with the same signatures.
pub(super) fn parity(subject: &Subject) -> Result<Vec<String>> {
    let mut forwarded = BTreeMap::new();
    for home in homes(&subject.workspace.map, subject.area) {
        let path = subject.root.join(home.shim());
        if let Ok(text) = std::fs::read_to_string(&path) {
            for entry in forwarded_entries(&text).unwrap_or_default() {
                forwarded.insert(entry.name, entry.signature);
            }
        }
    }
    let wanted: BTreeMap<String, String> = subject
        .baseline
        .methods
        .iter()
        .map(|method| (method.name.clone(), method.signature.clone()))
        .collect();
    let (wanted_names, forwarded_names) = (names(&wanted), names(&forwarded));
    let differing = wanted.iter().filter_map(|(name, old)| {
        let new = forwarded.get(name)?;
        (without_binding_mut(old) != without_binding_mut(new))
            .then(|| format!("signature differs: {name}: {old} vs {new}"))
    });
    Ok(difference("not forwarded", &wanted_names, &forwarded_names)
        .into_iter()
        .chain(difference(
            "forwarded but not in the baseline",
            &forwarded_names,
            &wanted_names,
        ))
        .chain(differing)
        .collect())
}

fn names(entries: &BTreeMap<String, String>) -> BTreeSet<String> {
    entries.keys().cloned().collect()
}

/// Rule 2: no `impl UnlockedVault` method in the group or usecases files; the shim keeps only
/// passthrough calls into `crate::<group>` or `crate::usecases`.
pub(super) fn no_logic(subject: &Subject) -> Result<Vec<String>> {
    let homes = homes(&subject.workspace.map, subject.area);
    let shims: BTreeSet<String> = homes.iter().map(Home::shim).collect();
    let targets: BTreeSet<&str> = homes
        .iter()
        .map(|home| home.group.as_str())
        .chain([USECASES])
        .collect();
    let mut found = Vec::new();
    for path in area_files(subject.workspace, subject.area) {
        let Ok(source) = std::fs::read_to_string(subject.root.join(&path)) else {
            continue;
        };
        let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
        let is_shim = shims.contains(&path);
        found.extend(production_methods(&file).into_iter().filter_map(|method| {
            if !is_shim {
                Some("is an `impl UnlockedVault` method outside the shim")
            } else if !is_passthrough(&method.block, &targets) {
                Some("is not a passthrough call into the group or usecases")
            } else {
                None
            }
            .map(|why| format!("{path}: `{}` {why}", method.sig.ident))
        }));
    }
    Ok(found)
}

/// Rule 3: no `persist()` call outside tests.
pub(super) fn no_persist(subject: &Subject) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for path in area_files(subject.workspace, subject.area) {
        let Some(scanned) = subject.workspace.corpus.get(&path) else {
            continue;
        };
        if scanned.is_test {
            continue;
        }
        found.extend(
            scanned
                .facts
                .calls
                .iter()
                .filter(|call| call.name == "persist" && !call.in_test)
                .map(|call| format!("{path}:{}: persist()", call.line)),
        );
    }
    Ok(found)
}

/// Rule 4: the test names are the baseline's.
pub(super) fn test_names(subject: &Subject) -> Result<Vec<String>> {
    let now = baseline::test_names(subject.workspace, subject.area);
    let wanted: BTreeSet<String> = subject.baseline.tests.iter().cloned().collect();
    Ok(difference("test missing", &wanted, &now))
}

/// Rule 5: every changed file is one of the allowed set.
pub(super) fn outside_allowed(
    label: &str,
    changed: &[String],
    allowed: &BTreeSet<String>,
) -> Vec<String> {
    changed
        .iter()
        .filter(|path| !allowed.contains(*path))
        .map(|path| format!("{label}: files: {path} is outside the allowed set"))
        .collect()
}

/// `<what>: <name>` for every name of `left` that `right` lacks.
fn difference(what: &str, left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right)
        .map(|name| format!("{what}: {name}"))
        .collect()
}

/// A body is a passthrough when it is one call of `crate::<target>::...` whose arguments are plain
/// names, optionally borrowed.
fn is_passthrough(block: &Block, targets: &BTreeSet<&str>) -> bool {
    let [Stmt::Expr(Expr::Call(call), _)] = block.stmts.as_slice() else {
        return false;
    };
    let Expr::Path(callee) = call.func.as_ref() else {
        return false;
    };
    let segments: Vec<String> = callee
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    matches!(segments.as_slice(), [root, target, _, ..] if root == "crate" && targets.contains(target.as_str()))
        && call.args.iter().all(is_plain_argument)
}

/// A name (`self`, `id`), or a borrow of one.
fn is_plain_argument(arg: &Expr) -> bool {
    match arg {
        Expr::Path(_) => true,
        Expr::Reference(borrow) => is_plain_argument(&borrow.expr),
        _ => false,
    }
}
