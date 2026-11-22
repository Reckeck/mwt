//! The wallet tree rules over a `VaultDoc`: where a wallet may sit, which
//! names are free, which extras fit which kind, and what each write of the
//! stores area would change. Pure: no vault, clock or I/O. Stores form a tree
//! of at most `MAX_STORE_DEPTH + 1` levels; every write that places a node
//! goes through [`validate_placement`], so the tree rules live in one spot
//! (reads stay lenient so a hand-edited vault still opens).

use crate::tree::{children_of, depth, find_store, subtree_ids};
use mwt_base::error::{Error, Result, StoreTreeViolation, WalletRule};
use mwt_model::Chain;
use mwt_model::VaultDoc;
use mwt_model::{
    AssetRef, Binding, BindingShape, LiabilityTerms, MAX_STORE_DEPTH, Store, StoreKind, SubType,
};

/// The fields of a wallet form, beyond its name, that the placement rules
/// look at. Built by the scenario from its own form type.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExtraClaims<'a> {
    pub liability: Option<&'a LiabilityTerms>,
    pub contact_id: Option<&'a str>,
    pub secured_by: Option<&'a str>,
    pub custody: bool,
    pub property: bool,
    pub provider: bool,
}

/// What a move of one wallet comes to.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreMove {
    /// It already hangs there: nothing is written.
    Stay(Box<Store>),
    Reparent,
}

/// Levels below `id`: 0 for a leaf.
pub fn subtree_height(doc: &VaultDoc, id: &str) -> usize {
    let mut seen = std::collections::HashSet::from([id.to_string()]);
    let mut level = vec![id.to_string()];
    let mut height = 0;
    loop {
        let next: Vec<String> = level
            .iter()
            .flat_map(|parent| children_of(doc, parent))
            .filter(|child| seen.insert(child.id.clone()))
            .map(|child| child.id.clone())
            .collect();
        if next.is_empty() {
            return height;
        }
        height += 1;
        level = next;
    }
}

/// R5: names are unique among siblings (roots among roots), ignoring case
/// and surrounding spaces. `node` is skipped so a rename to itself passes.
pub fn ensure_unique_name(
    doc: &VaultDoc,
    node: Option<&str>,
    parent: Option<&str>,
    name: &str,
) -> Result<()> {
    let wanted = name.trim().to_lowercase();
    let taken = doc.stores.iter().any(|store| {
        node != Some(store.id.as_str())
            && store.parent_id.as_deref() == parent
            && store.name.trim().to_lowercase() == wanted
    });
    if taken {
        return Err(Error::StoreTree(StoreTreeViolation::NameTaken));
    }
    Ok(())
}

/// A name free among the wallets (or among the children of `parent`).
pub fn free_name(doc: &VaultDoc, node: &str, parent: Option<&str>, wanted: &str) -> String {
    let free = |name: &str| ensure_unique_name(doc, Some(node), parent, name).is_ok();
    if free(wanted) {
        return wanted.to_string();
    }
    (2..)
        .map(|index| format!("{wanted} ({index})"))
        .find(|name| free(name))
        .unwrap_or_else(|| wanted.to_string())
}

/// Whether `node` (an existing store, or `None` for a new one) may sit
/// under `parent` with `name`. Rules R1–R6.
pub fn validate_placement(
    doc: &VaultDoc,
    node: Option<&str>,
    parent: Option<&str>,
    name: &str,
) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::InvalidInput("store name must not be empty".into()));
    }
    let current = node.and_then(|id| find_store(doc, id));
    if let Some(parent_id) = parent {
        let parent_store =
            find_store(doc, parent_id).ok_or_else(|| Error::Missing("parent store".into()))?;
        if let Some(node_id) = node {
            if current.is_some_and(|store| store.parent_id.is_none()) {
                return Err(Error::StoreTree(StoreTreeViolation::RootMove));
            }
            if current.is_some_and(|store| store.kind != parent_store.kind) {
                return Err(Error::WalletRule(WalletRule::SubTypeNotAllowed));
            }
            if parent_id == node_id || subtree_ids(doc, node_id).iter().any(|id| id == parent_id) {
                return Err(Error::StoreTree(StoreTreeViolation::Cycle));
            }
        }
        let height = node.map_or(0, |id| subtree_height(doc, id));
        if depth(doc, parent_id) + 1 + height > MAX_STORE_DEPTH {
            return Err(Error::StoreTree(StoreTreeViolation::TooDeep));
        }
        if parent_store.archived {
            return Err(Error::StoreTree(StoreTreeViolation::ParentArchived));
        }
    }
    // A typed sub-account cannot become a top-level wallet.
    if parent.is_none() && current.is_some_and(|store| store.parent_id.is_some()) {
        return Err(Error::WalletRule(WalletRule::NotApplicable));
    }
    ensure_unique_name(doc, node, parent, name)
}

/// Which extras a wallet of this kind and level may carry, shared by
/// create and update.
pub fn check_store_extras(
    doc: &VaultDoc,
    kind: StoreKind,
    sub_type: Option<SubType>,
    is_root: bool,
    claims: &ExtraClaims<'_>,
) -> Result<()> {
    let credit = sub_type.is_some_and(SubType::is_credit);
    if claims.liability.is_some() && kind != StoreKind::Liability && !credit {
        return Err(Error::WalletRule(WalletRule::NotApplicable));
    }
    if let Some(terms) = claims.liability
        && terms.due_day.is_some_and(|day| !(1..=31).contains(&day))
    {
        return Err(Error::InvalidInput("the payment day must be 1–31".into()));
    }
    let misplaced = (claims.custody && !(is_root && kind == StoreKind::Crypto))
        || (claims.property && !(is_root && kind == StoreKind::Property))
        || (claims.provider && !(is_root && kind == StoreKind::Exchange))
        || (claims.secured_by.is_some() && kind != StoreKind::Liability);
    if misplaced {
        return Err(Error::WalletRule(WalletRule::NotApplicable));
    }
    if let Some(target) = claims.secured_by
        && !find_store(doc, target)
            .is_some_and(|store| store.kind == StoreKind::Property && store.parent_id.is_none())
    {
        return Err(Error::Missing(format!("property {target}")));
    }
    if let Some(contact_id) = claims.contact_id {
        if !is_root {
            return Err(Error::InvalidInput(
                "a contact is set on the top-level wallet; sub-accounts share it".into(),
            ));
        }
        if !doc.contacts.iter().any(|contact| contact.id == contact_id) {
            return Err(Error::Missing(format!("contact {contact_id}")));
        }
    }
    Ok(())
}

/// A sub-account's type must be offered by its wallet kind and its
/// binding must have the shape the type expects.
pub fn check_sub_account(
    kind: StoreKind,
    sub_type: Option<SubType>,
    binding: Option<&Binding>,
) -> Result<(SubType, Binding)> {
    let sub_type = sub_type.ok_or(Error::WalletRule(WalletRule::BindingRequired))?;
    if !kind.admits_sub_type(sub_type) {
        return Err(Error::WalletRule(WalletRule::SubTypeNotAllowed));
    }
    let binding = match (sub_type.binding_shape(), binding) {
        (BindingShape::Market(market), None | Some(Binding::Any)) => Binding::Market(market),
        (_, None | Some(Binding::Any)) => {
            return Err(Error::WalletRule(WalletRule::BindingRequired));
        }
        (shape, Some(binding)) if binding.shape() == Some(shape) => binding.clone(),
        _ => return Err(Error::WalletRule(WalletRule::BindingMismatch)),
    };
    if let Binding::Pair { base, quote } = &binding
        && base.key() == quote.key()
    {
        return Err(Error::InvalidInput(
            "a pair needs two different assets".into(),
        ));
    }
    Ok((sub_type, binding))
}

/// A new top-level wallet must be of a kind the user may create, and the
/// kinds that allow few wallets must not exceed them.
pub fn check_root_creatable(doc: &VaultDoc, kind: StoreKind) -> Result<()> {
    if !kind.user_creatable() {
        return Err(Error::WalletRule(WalletRule::NotCreatable));
    }
    if let Some(max) = kind.max_roots()
        && doc
            .stores
            .iter()
            .filter(|store| store.kind == kind && store.parent_id.is_none())
            .count()
            >= max
    {
        return Err(Error::WalletRule(WalletRule::CashSingleton));
    }
    Ok(())
}

/// Sub-account defaults for a wallet of `kind`: its first template, bound to
/// a market, or to USD for currency accounts and BTC for tokens. Real callers
/// pass type and binding explicitly.
pub fn default_sub_account(kind: StoreKind) -> (Option<SubType>, Option<Binding>) {
    let sub_type = kind.sub_types().first().copied();
    let binding = sub_type.map(|sub_type| match sub_type.binding_shape() {
        BindingShape::Market(market) => Binding::Market(market),
        BindingShape::Pair => Binding::Pair {
            base: bitcoin(),
            quote: AssetRef::fiat("USD", "US Dollar"),
        },
        BindingShape::Asset if sub_type == SubType::Token => Binding::Asset(bitcoin()),
        BindingShape::Asset => Binding::Asset(AssetRef::fiat("USD", "US Dollar")),
    });
    (sub_type, binding)
}

fn bitcoin() -> AssetRef {
    AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin)
}

/// A move: a wallet that already hangs there stays as it is; any other place
/// must pass the tree rules.
pub fn plan_move(doc: &VaultDoc, id: &str, parent_id: Option<&str>) -> Result<StoreMove> {
    let store = find_store(doc, id).ok_or_else(|| Error::Missing("store".into()))?;
    if store.parent_id.as_deref() == parent_id {
        return Ok(StoreMove::Stay(Box::new(store.clone())));
    }
    validate_placement(doc, Some(id), parent_id, &store.name)?;
    Ok(StoreMove::Reparent)
}

/// The stores an archive (or a restore) of `id` reaches: it and its whole
/// subtree. A node cannot be restored while an ancestor is still archived
/// (R8).
pub fn plan_archive(doc: &VaultDoc, id: &str, archived: bool) -> Result<Vec<String>> {
    let store = find_store(doc, id).ok_or_else(|| Error::Missing("store".into()))?;
    if archived && store.kind == StoreKind::Cash && store.parent_id.is_none() {
        return Err(Error::WalletRule(WalletRule::CashLocked));
    }
    if archived && store.archived {
        return Err(Error::InvalidInput("already archived".into()));
    }
    if !archived
        && crate::tree::ancestors_of(doc, id)
            .iter()
            .any(|parent| parent.archived)
    {
        return Err(Error::StoreTree(StoreTreeViolation::RestoreParentFirst));
    }
    let mut affected = subtree_ids(doc, id);
    affected.push(id.to_string());
    Ok(affected)
}

/// A wallet can go only when nothing refers to it: not the cash wallet, no
/// operation, no sub-account.
pub fn check_deletable(doc: &VaultDoc, id: &str) -> Result<()> {
    let store = find_store(doc, id);
    if store.is_some_and(|store| store.kind == StoreKind::Cash && store.parent_id.is_none()) {
        return Err(Error::WalletRule(WalletRule::CashLocked));
    }
    if doc.transactions.iter().any(|tx| tx.touches(id)) {
        return Err(Error::InvalidInput(
            "store has transactions; archive instead".into(),
        ));
    }
    if !children_of(doc, id).is_empty() {
        return Err(Error::InvalidInput(
            "store has sub-accounts; move or delete them first".into(),
        ));
    }
    store
        .map(|_| ())
        .ok_or_else(|| Error::Missing("store".into()))
}

/// The binding an edit sets, if any: only a sub-account has one, and
/// everything already booked on it must still fit.
pub fn edited_binding(
    doc: &VaultDoc,
    id: &str,
    kind: StoreKind,
    sub_type: Option<SubType>,
    binding: Option<&Binding>,
) -> Result<Option<Binding>> {
    let Some(binding) = binding else {
        return Ok(None);
    };
    let is_sub_account = find_store(doc, id).is_some_and(|store| store.parent_id.is_some());
    if !is_sub_account {
        return Err(Error::WalletRule(WalletRule::NotApplicable));
    }
    let (_, binding) = check_sub_account(kind, sub_type, Some(binding))?;
    let fits = doc
        .transactions
        .iter()
        .flat_map(mwt_model::Transaction::legs)
        .filter(|leg| leg.store_id == id)
        .all(|leg| binding.admits(&leg.asset));
    if !fits {
        return Err(Error::WalletRule(WalletRule::BindingMismatch));
    }
    Ok(Some(binding))
}
