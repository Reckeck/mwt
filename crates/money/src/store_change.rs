//! Changing an account's kind or place, and merging one account into another.
//! The change is worked out on a copy of the document before anything is written.
//!
//! Merging: what happens to the corrections.
//!
//! A correction stores the real balance `after` of its own account. Once two
//! accounts become one, "really 80" must mean 80 *plus what the other account
//! held at that moment*, or the other account's money is lost. Every moved
//! correction keeps its delta `after - before`, and the merged balance per
//! asset is the sum of both accounts.
//!
//! Pure: no vault, no clock of its own, no I/O. The document must be rebased
//! (every mutation ends in `recompute_holdings`), so each correction's leg is
//! its real delta.

use std::collections::{HashMap, HashSet};

use crate::stores::free_name;
use crate::tree::{children_of, find_store};
use crate::validate::store_sides;
use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_base::time::secs_or_now;
use mwt_model::VaultDoc;
use mwt_model::{
    AssetRef, Binding, BindingShape, Custody, ExchangeProfile, MarketKind, Store, StoreKind,
    SubType,
};
use mwt_model::{LegRole, Transaction, allowed_kinds};

/// Creation order of a record, built by the caller from the sync stamp
/// (money must not depend on sync). Only its order matters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CreatedAt(pub (u64, u32, String));

pub struct MergeInput<'a> {
    pub transactions: &'a [Transaction],
    pub source_id: &'a str,
    pub target_id: &'a str,
    pub source_name: &'a str,
    pub target_name: &'a str,
    /// Transaction id -> creation order; missing means unstamped (newest).
    pub created: &'a HashMap<String, CreatedAt>,
}

/// One moved correction whose counted balance is raised by what the other
/// account held at that moment.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CorrectionShift {
    pub tx_id: String,
    pub timestamp: String,
    pub from_store_id: String,
    /// Whose correction it is.
    pub from_store_name: String,
    pub asset: AssetRef,
    pub after_old: Amount,
    pub after_new: Amount,
    /// `after_new - after_old`.
    pub other_balance: Amount,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MergePlan {
    pub moved_transactions: u32,
    pub shifts: Vec<CorrectionShift>,
}

/// Rank of a correction in the order `rebase_adjustments` uses: second,
/// unstamped last, creation stamp, list index.
struct Ranked<'a> {
    secs: i64,
    unstamped: bool,
    created: Option<&'a CreatedAt>,
    index: usize,
}

impl Ranked<'_> {
    fn key(&self) -> (i64, bool, Option<&CreatedAt>, usize) {
        (self.secs, self.unstamped, self.created, self.index)
    }
}

pub fn merge_plan(input: &MergeInput<'_>) -> MergePlan {
    let ranks: HashMap<usize, Ranked<'_>> = input
        .transactions
        .iter()
        .enumerate()
        .filter(|(_, tx)| matches!(tx, Transaction::Adjustment(_)))
        .map(|(index, tx)| {
            let created = input.created.get(&tx.meta().id);
            let rank = Ranked {
                secs: secs_or_now(&tx.meta().timestamp),
                unstamped: created.is_none(),
                created,
                index,
            };
            (index, rank)
        })
        .collect();

    let mut plan = MergePlan {
        moved_transactions: input
            .transactions
            .iter()
            .filter(|tx| tx.touches(input.source_id))
            .count() as u32,
        shifts: Vec::new(),
    };
    for (index, tx) in input.transactions.iter().enumerate() {
        let Transaction::Adjustment(inner) = tx else {
            continue;
        };
        let (own_name, other_id) = if inner.store_id == input.source_id {
            (input.source_name, input.target_id)
        } else if inner.store_id == input.target_id {
            (input.target_name, input.source_id)
        } else {
            continue;
        };
        let Some(rank) = ranks.get(&index) else {
            continue;
        };
        let held = other_balances(input.transactions, &ranks, other_id, rank);
        for line in &inner.lines {
            let shift = held.get(&line.asset.key()).copied().unwrap_or_default();
            if shift.is_zero() {
                continue;
            }
            plan.shifts.push(CorrectionShift {
                tx_id: tx.meta().id.clone(),
                timestamp: tx.meta().timestamp.clone(),
                from_store_id: inner.store_id.clone(),
                from_store_name: own_name.to_string(),
                asset: line.asset.clone(),
                after_old: line.after,
                after_new: line.after + shift,
                other_balance: shift,
            });
        }
    }
    plan
}

/// What `store_id` held, per asset key, at the correction `at`: its legs up to
/// that second, minus the corrections of the same second ranking at or after it.
fn other_balances(
    transactions: &[Transaction],
    ranks: &HashMap<usize, Ranked<'_>>,
    store_id: &str,
    at: &Ranked<'_>,
) -> HashMap<String, Amount> {
    let mut totals: HashMap<String, Amount> = HashMap::new();
    for (index, tx) in transactions.iter().enumerate() {
        if secs_or_now(&tx.meta().timestamp) > at.secs {
            continue;
        }
        if ranks
            .get(&index)
            .is_some_and(|other| other.key() >= at.key())
        {
            continue;
        }
        for leg in tx.legs() {
            if leg.store_id == store_id && !leg.asset.is_derivative() {
                *totals.entry(leg.asset.key()).or_default() += leg.amount;
            }
        }
    }
    totals
}

/// What to do with an account.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum StoreChange {
    /// A wallet becomes another kind; its sub-accounts follow or are split off.
    ChangeKind { kind: StoreKind },
    /// Hang the account under another wallet, of its kind or another one. A
    /// wallet without sub-accounts may be moved in as well.
    MoveTo { parent_id: String },
    /// A sub-account becomes a wallet of its own, of the same kind.
    Detach,
    /// Every operation of the account moves to `target_id`; the account goes.
    MergeInto { target_id: String },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreChangeInput {
    pub store_id: String,
    pub change: StoreChange,
}

/// One account the change touches.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreChangeItem {
    pub store_id: String,
    pub name: String,
    pub kind_after: StoreKind,
    pub sub_type_before: Option<SubType>,
    pub sub_type_after: Option<SubType>,
}

/// An operation that would no longer fit the account it is on.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreChangeConflict {
    pub tx_id: String,
    pub timestamp: String,
    pub comment: String,
    pub message: String,
}

/// What a change would do, before it is applied.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreChangePlan {
    pub store_id: String,
    /// The account itself and the sub-accounts that move along with it.
    pub kept: Vec<StoreChangeItem>,
    /// Sub-accounts the new kind cannot hold: split off as archived wallets
    /// of the old kind, with their operations intact.
    pub archived: Vec<StoreChangeItem>,
    /// Operations that would break the rules of their new account.
    pub conflicts: Vec<StoreChangeConflict>,
    /// Merge: how many operations move to the target.
    pub moved_transactions: u32,
    /// Merge: corrections whose counted balance grows by what the other
    /// account held at that moment.
    #[serde(default)]
    pub corrections: Vec<CorrectionShift>,
    /// Why the change cannot be made at all.
    pub error: Option<String>,
}

/// A change worked out on a copy of the document: what it would do, and the
/// document as it would then be.
pub struct ChangeOutcome {
    pub plan: StoreChangePlan,
    pub after: VaultDoc,
}

/// Work the change out on a copy of `doc`; `doc` is not touched. `created`
/// is the creation order of the transactions (merge only).
pub fn work_out(
    doc: &VaultDoc,
    input: &StoreChangeInput,
    created: &HashMap<String, CreatedAt>,
) -> ChangeOutcome {
    let mut after = doc.clone();
    let plan = change_in_memory(&mut after, input, created);
    ChangeOutcome { plan, after }
}

/// Why a plan cannot be applied: it has an error or a conflict.
pub fn refusal(plan: &StoreChangePlan) -> Option<String> {
    plan.error.clone().or_else(|| {
        (!plan.conflicts.is_empty()).then(|| {
            format!(
                "{} operations would not fit the new account",
                plan.conflicts.len()
            )
        })
    })
}

/// Kinds a wallet may be changed from and to: they hold any money and need
/// no data of their own. Cash, property, loyalty points and loans do.
fn convertible(kind: StoreKind) -> bool {
    matches!(
        kind,
        StoreKind::Bank | StoreKind::Crypto | StoreKind::Exchange
    )
}

/// The type a sub-account takes under a wallet of `kind`: its own when the
/// kind admits it, else the matching one-asset type, else none.
pub fn retype(sub_type: Option<SubType>, binding: &Binding, kind: StoreKind) -> Option<SubType> {
    let sub_type = sub_type?;
    if kind.admits_sub_type(sub_type) {
        return Some(sub_type);
    }
    let plain_asset = matches!(
        sub_type,
        SubType::Card | SubType::Account | SubType::Token | SubType::Pocket
    );
    let asset = match binding {
        Binding::Asset(asset) if plain_asset => asset,
        _ => return None,
    };
    match kind {
        StoreKind::Bank if asset.is_fiat() => Some(SubType::Card),
        StoreKind::Cash if asset.is_fiat() => Some(SubType::Pocket),
        StoreKind::Crypto => Some(SubType::Token),
        _ => None,
    }
}

fn item(store: &Store, sub_type_before: Option<SubType>) -> StoreChangeItem {
    StoreChangeItem {
        store_id: store.id.clone(),
        name: store.name.clone(),
        kind_after: store.kind,
        sub_type_before,
        sub_type_after: store.sub_type,
    }
}

/// Replace every string equal to `from` with `to`, anywhere in the document.
/// Ids are UUIDs, so a value equal to one is that id.
pub fn replace_id(value: &mut serde_json::Value, from: &str, to: &str) {
    match value {
        serde_json::Value::String(text) if text == from => *text = to.to_string(),
        serde_json::Value::Array(items) => {
            for entry in items {
                replace_id(entry, from, to);
            }
        }
        serde_json::Value::Object(map) => {
            for entry in map.values_mut() {
                replace_id(entry, from, to);
            }
        }
        _ => {}
    }
}

/// Apply the change to `doc` and report on it. On an error the document may
/// be half-changed: callers work on a copy.
pub fn change_in_memory(
    doc: &mut VaultDoc,
    input: &StoreChangeInput,
    created: &HashMap<String, CreatedAt>,
) -> StoreChangePlan {
    let mut plan = StoreChangePlan {
        store_id: input.store_id.clone(),
        ..StoreChangePlan::default()
    };
    let result = match &input.change {
        StoreChange::ChangeKind { kind } => change_kind(doc, &input.store_id, *kind, &mut plan),
        StoreChange::MoveTo { parent_id } => move_to(doc, &input.store_id, parent_id, &mut plan),
        StoreChange::Detach => detach(doc, &input.store_id, &mut plan),
        StoreChange::MergeInto { target_id } => {
            merge_into(doc, &input.store_id, target_id, created, &mut plan)
        }
    };
    match result {
        Ok(affected) => plan.conflicts = conflicts_on(doc, &affected),
        Err(err) => plan.error = Some(format!("{}: {err}", err.code())),
    }
    plan
}

fn store_mut<'doc>(doc: &'doc mut VaultDoc, id: &str) -> Result<&'doc mut Store> {
    doc.stores
        .iter_mut()
        .find(|store| store.id == id)
        .ok_or_else(|| Error::Missing(format!("store {id}")))
}

fn stored(doc: &VaultDoc, id: &str) -> Result<Store> {
    find_store(doc, id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("store {id}")))
}

/// Give a wallet the fields of its new kind and drop the old kind's.
fn set_root_kind(doc: &mut VaultDoc, id: &str, kind: StoreKind) -> Result<()> {
    let leaving_exchange = {
        let store = store_mut(doc, id)?;
        let old = store.kind;
        if store.purpose == old.default_purpose() {
            store.purpose = kind.default_purpose();
        }
        store.kind = kind;
        store.custody =
            (kind == StoreKind::Crypto).then_some(store.custody.unwrap_or(Custody::Hot));
        store.exchange = (kind == StoreKind::Exchange).then(|| {
            store
                .exchange
                .clone()
                .unwrap_or_else(ExchangeProfile::default)
        });
        old == StoreKind::Exchange && kind != StoreKind::Exchange
    };
    if leaving_exchange {
        doc.exchanges.bybit.retain(|keys| keys.store_id != id);
    }
    Ok(())
}

/// Exchange keys stop booking fills on an account that is no longer an exchange's.
fn forget_import_target(doc: &mut VaultDoc, id: &str) {
    for keys in &mut doc.exchanges.bybit {
        if keys.spot_store_id.as_deref() == Some(id) {
            keys.spot_store_id = None;
        }
        if keys.futures_store_id.as_deref() == Some(id) {
            keys.futures_store_id = None;
        }
    }
}

fn change_kind(
    doc: &mut VaultDoc,
    id: &str,
    kind: StoreKind,
    plan: &mut StoreChangePlan,
) -> Result<HashSet<String>> {
    let root = stored(doc, id)?;
    if root.parent_id.is_some() {
        return Err(Error::InvalidInput(
            "only a wallet changes its kind; move a sub-account instead".into(),
        ));
    }
    if root.kind == kind {
        return Err(Error::InvalidInput(
            "the wallet is already of this kind".into(),
        ));
    }
    if !convertible(root.kind) || !convertible(kind) {
        return Err(Error::InvalidInput(
            "only banks, crypto wallets and exchanges change their kind".into(),
        ));
    }
    set_root_kind(doc, id, kind)?;
    let mut affected: HashSet<String> = HashSet::from([id.to_string()]);
    let children: Vec<Store> = children_of(doc, id).into_iter().cloned().collect();
    let mut kept = vec![item(find_store(doc, id).unwrap_or(&root), None)];
    for child in children {
        match retype(child.sub_type, &child.binding, kind) {
            Some(sub_type) => {
                let store = store_mut(doc, &child.id)?;
                store.kind = kind;
                store.sub_type = Some(sub_type);
                if kind != StoreKind::Exchange {
                    forget_import_target(doc, &child.id);
                }
                affected.insert(child.id.clone());
                kept.push(item(
                    find_store(doc, &child.id).unwrap_or(&child),
                    child.sub_type,
                ));
            }
            None => {
                // Split off as an archived wallet of the old kind: its
                // operations stay valid where they are.
                let name = free_name(
                    doc,
                    &child.id,
                    None,
                    &format!("{} · {}", root.name, child.name),
                );
                let store = store_mut(doc, &child.id)?;
                store.parent_id = None;
                store.sub_type = None;
                store.binding = Binding::Any;
                store.archived = true;
                store.name = name;
                store.exchange = (store.kind == StoreKind::Exchange).then(ExchangeProfile::default);
                plan.archived.push(item(store, child.sub_type));
            }
        }
    }
    plan.kept = kept;
    Ok(affected)
}

fn move_to(
    doc: &mut VaultDoc,
    id: &str,
    parent_id: &str,
    plan: &mut StoreChangePlan,
) -> Result<HashSet<String>> {
    let store = stored(doc, id)?;
    let parent = stored(doc, parent_id)?;
    if parent.id == store.id || parent.parent_id.is_some() {
        return Err(Error::InvalidInput(
            "move into a wallet, not into a sub-account".into(),
        ));
    }
    if store.parent_id.as_deref() == Some(parent_id) {
        return Err(Error::InvalidInput("it is already there".into()));
    }
    if parent.archived {
        return Err(Error::Archived(parent.id));
    }
    if store.kind == StoreKind::Cash && store.parent_id.is_none() {
        return Err(Error::InvalidInput("the cash wallet stays a wallet".into()));
    }
    let (sub_type, binding) = if store.parent_id.is_some() {
        let sub_type = retype(store.sub_type, &store.binding, parent.kind).ok_or_else(|| {
            Error::InvalidInput(format!(
                "a {} wallet has no account of this type",
                parent.kind_label()
            ))
        })?;
        (sub_type, store.binding.clone())
    } else {
        // A wallet moved in becomes an account of one asset.
        if !children_of(doc, id).is_empty() {
            return Err(Error::InvalidInput(
                "move or merge its sub-accounts first: an account holds no accounts".into(),
            ));
        }
        root_as_account(doc, &store, parent.kind)?
    };
    let name = free_name(doc, id, Some(parent_id), &store.name);
    let was_exchange_root = store.parent_id.is_none() && store.kind == StoreKind::Exchange;
    {
        let target = store_mut(doc, id)?;
        target.parent_id = Some(parent_id.to_string());
        target.kind = parent.kind;
        target.sub_type = Some(sub_type);
        target.binding = binding;
        target.name = name;
        target.custody = None;
        target.exchange = None;
        target.property = None;
        target.contact_id = None;
        if !matches!(sub_type, SubType::Deposit | SubType::Savings | SubType::Pds) {
            target.deposit = None;
        }
    }
    if was_exchange_root {
        doc.exchanges.bybit.retain(|keys| keys.store_id != id);
    }
    if parent.kind != StoreKind::Exchange {
        forget_import_target(doc, id);
    }
    if let Some(moved) = find_store(doc, id) {
        plan.kept.push(item(moved, store.sub_type));
    }
    Ok(HashSet::from([id.to_string()]))
}

/// The type and binding a wallet takes as an account of a `kind` wallet:
/// the asset it holds, which has to be one.
fn root_as_account(doc: &VaultDoc, store: &Store, kind: StoreKind) -> Result<(SubType, Binding)> {
    let held: Vec<&AssetRef> = doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == store.id && !holding.quantity.is_zero())
        .map(|holding| &holding.asset)
        .collect();
    match kind {
        StoreKind::Exchange => Ok((SubType::Funding, Binding::Market(MarketKind::Funding))),
        _ => {
            let [asset] = held.as_slice() else {
                return Err(Error::InvalidInput(
                    "an account holds one asset: this wallet holds none or several".into(),
                ));
            };
            let binding = Binding::Asset((*asset).clone());
            let sub_type = match kind {
                StoreKind::Bank if asset.is_fiat() => SubType::Card,
                StoreKind::Cash if asset.is_fiat() => SubType::Pocket,
                StoreKind::Crypto => SubType::Token,
                _ => {
                    return Err(Error::InvalidInput(
                        "this kind of wallet cannot hold that asset".into(),
                    ));
                }
            };
            debug_assert_eq!(sub_type.binding_shape(), BindingShape::Asset);
            Ok((sub_type, binding))
        }
    }
}

fn detach(doc: &mut VaultDoc, id: &str, plan: &mut StoreChangePlan) -> Result<HashSet<String>> {
    let store = stored(doc, id)?;
    let Some(parent_id) = store.parent_id.clone() else {
        return Err(Error::InvalidInput("it is a wallet already".into()));
    };
    if !store.kind.user_creatable() || store.kind.max_roots().is_some() {
        return Err(Error::InvalidInput(
            "this kind has a single wallet; move the account instead".into(),
        ));
    }
    let parent_name = find_store(doc, &parent_id)
        .map(|parent| parent.name.clone())
        .unwrap_or_default();
    let name = free_name(doc, id, None, &store.name);
    let name = if name == store.name {
        name
    } else {
        free_name(doc, id, None, &format!("{} ({parent_name})", store.name))
    };
    {
        let target = store_mut(doc, id)?;
        target.parent_id = None;
        target.sub_type = None;
        target.binding = Binding::Any;
        target.name = name;
        target.custody = (target.kind == StoreKind::Crypto).then_some(Custody::Hot);
        target.exchange = (target.kind == StoreKind::Exchange).then(ExchangeProfile::default);
    }
    if let Some(moved) = find_store(doc, id) {
        plan.kept.push(item(moved, store.sub_type));
    }
    Ok(HashSet::from([id.to_string()]))
}

fn merge_into(
    doc: &mut VaultDoc,
    id: &str,
    target_id: &str,
    created: &HashMap<String, CreatedAt>,
    plan: &mut StoreChangePlan,
) -> Result<HashSet<String>> {
    let source = stored(doc, id)?;
    let target = stored(doc, target_id)?;
    if source.id == target.id {
        return Err(Error::InvalidInput("merge into another account".into()));
    }
    if !children_of(doc, id).is_empty() {
        return Err(Error::InvalidInput(
            "move or merge its sub-accounts first".into(),
        ));
    }
    if source.kind == StoreKind::Cash && source.parent_id.is_none() {
        return Err(Error::InvalidInput(
            "the cash wallet cannot be merged away".into(),
        ));
    }
    if target.archived {
        return Err(Error::Archived(target.id));
    }
    let merge = merge_plan(&MergeInput {
        transactions: &doc.transactions,
        source_id: id,
        target_id,
        source_name: &source.name,
        target_name: &target.name,
        created,
    });
    plan.moved_transactions = merge.moved_transactions;
    write_shifted_corrections(doc, &merge.shifts);
    plan.corrections = merge.shifts;
    // Drop the account itself, then point every reference at the target.
    doc.stores.retain(|store| store.id != id);
    doc.exchanges.bybit.retain(|keys| keys.store_id != id);
    let mut value = serde_json::to_value(&*doc).map_err(|err| Error::Serialize(err.to_string()))?;
    replace_id(&mut value, id, target_id);
    *doc = serde_json::from_value::<VaultDoc>(value)
        .map_err(|err| Error::Deserialize(err.to_string()))?;
    plan.kept.push(item(&source, source.sub_type));
    Ok(HashSet::from([target_id.to_string()]))
}

/// Set the counted balance of every shifted correction line.
fn write_shifted_corrections(doc: &mut VaultDoc, shifts: &[CorrectionShift]) {
    for shift in shifts {
        let line = doc
            .transactions
            .iter_mut()
            .filter(|tx| tx.meta().id == shift.tx_id)
            .find_map(|tx| match tx {
                Transaction::Adjustment(inner) => inner
                    .lines
                    .iter_mut()
                    .find(|line| line.asset.key() == shift.asset.key()),
                _ => None,
            });
        if let Some(line) = line {
            line.after = shift.after_new;
        }
    }
}

/// The operations on `stores` that break the rules of the account they are
/// on now: what the kind accepts, what the account may hold, both sides on
/// one account.
fn conflicts_on(doc: &VaultDoc, stores: &HashSet<String>) -> Vec<StoreChangeConflict> {
    doc.transactions
        .iter()
        .filter(|tx| stores.iter().any(|id| tx.touches(id)))
        .filter_map(|tx| {
            store_rule_violation(doc, tx).map(|message| StoreChangeConflict {
                tx_id: tx.meta().id.clone(),
                timestamp: tx.meta().timestamp.clone(),
                comment: tx.meta().comment.clone(),
                message,
            })
        })
        .collect()
}

fn store_rule_violation(doc: &VaultDoc, tx: &Transaction) -> Option<String> {
    for id in store_sides(tx) {
        let store = find_store(doc, id)?;
        if !allowed_kinds(store.kind).contains(&tx.kind()) {
            return Some(format!(
                "a {} is not possible on {}",
                tx.kind().tag(),
                store.name
            ));
        }
    }
    let mut seen: Vec<(String, String)> = Vec::new();
    for leg in tx
        .legs()
        .into_iter()
        .filter(|leg| matches!(leg.role, LegRole::Principal | LegRole::Adjustment))
    {
        let store = find_store(doc, &leg.store_id)?;
        if !store.binding.admits(&leg.asset) {
            return Some(format!("{} cannot hold {}", store.name, leg.asset.symbol));
        }
        if matches!(store.kind, StoreKind::Cash | StoreKind::Bank) && !leg.asset.is_fiat() {
            return Some(format!(
                "{} holds only fiat, not {}",
                store.name, leg.asset.symbol
            ));
        }
        let key = (leg.store_id.clone(), leg.asset.key());
        if seen.contains(&key) {
            return Some(format!("both sides land on {}", store.name));
        }
        seen.push(key);
    }
    None
}

trait StoreExt: Sized {
    fn kind_label(&self) -> &'static str;
}

impl StoreExt for Store {
    fn kind_label(&self) -> &'static str {
        match self.kind {
            StoreKind::Cash => "cash",
            StoreKind::Bank => "bank",
            StoreKind::Crypto => "crypto",
            StoreKind::Exchange => "exchange",
            StoreKind::Property => "property",
            StoreKind::Loyalty => "loyalty",
            StoreKind::Liability => "liability",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mwt_model::{AdjustmentLine, AdjustmentTx, KindDetails, PriceStamp, TopupTx, TxMeta};

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "RUB")
    }

    fn money(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn meta(id: &str, timestamp: &str) -> TxMeta {
        let mut meta = TxMeta::new("");
        meta.id = id.into();
        meta.timestamp = timestamp.into();
        meta
    }

    fn topup(id: &str, store: &str, raw: &str, timestamp: &str) -> Transaction {
        Transaction::Topup(TopupTx {
            parts: Vec::new(),
            distribution: None,
            meta: meta(id, timestamp),
            store_id: store.into(),
            asset: rub(),
            amount: money(raw),
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        })
    }

    fn correction(
        id: &str,
        store: &str,
        before: &str,
        after: &str,
        timestamp: &str,
    ) -> Transaction {
        Transaction::Adjustment(AdjustmentTx {
            meta: meta(id, timestamp),
            store_id: store.into(),
            lines: vec![AdjustmentLine {
                asset: rub(),
                before: money(before),
                after: money(after),
                price: PriceStamp::default(),
            }],
            fee: None,
        })
    }

    fn plan_of(transactions: &[Transaction], created: &HashMap<String, CreatedAt>) -> MergePlan {
        merge_plan(&MergeInput {
            transactions,
            source_id: "a",
            target_id: "b",
            source_name: "Card 1",
            target_name: "Card 2",
            created,
        })
    }

    /// Card 1: +300 (1 Jan), really 80 (1 Mar), +50 (10 Mar); Card 2: +1 000 (15 Jan).
    fn review() -> Vec<Transaction> {
        vec![
            topup("t1", "a", "300", "2026-01-01T00:00:00Z"),
            topup("t2", "b", "1000", "2026-01-15T00:00:00Z"),
            correction("c1", "a", "300", "80", "2026-03-01T00:00:00Z"),
            topup("t3", "a", "50", "2026-03-10T00:00:00Z"),
        ]
    }

    #[test]
    fn a_correction_of_the_source_is_shifted_by_the_targets_balance() {
        let plan = plan_of(&review(), &HashMap::new());
        assert_eq!(plan.moved_transactions, 3);
        assert_eq!(plan.shifts.len(), 1);
        let shift = &plan.shifts[0];
        assert_eq!(shift.from_store_name, "Card 1");
        assert_eq!(shift.after_old, money("80"));
        assert_eq!(shift.after_new, money("1080"));
        assert_eq!(shift.other_balance, money("1000"));
    }

    #[test]
    fn a_correction_of_the_target_is_shifted_by_the_sources_balance() {
        let mut transactions = review();
        transactions.push(correction(
            "c2",
            "b",
            "1000",
            "1200",
            "2026-04-01T00:00:00Z",
        ));
        let plan = plan_of(&transactions, &HashMap::new());
        assert_eq!(plan.shifts.len(), 2);
        let second = &plan.shifts[1];
        assert_eq!(second.tx_id, "c2");
        assert_eq!(second.after_old, money("1200"));
        assert_eq!(second.after_new, money("1330"));
        assert_eq!(second.other_balance, money("130"));
    }

    #[test]
    fn a_zero_shift_is_not_listed() {
        // The target holds nothing yet when the source is corrected.
        let transactions = vec![
            topup("t1", "a", "300", "2026-01-01T00:00:00Z"),
            correction("c1", "a", "300", "80", "2026-03-01T00:00:00Z"),
            topup("t2", "b", "1000", "2026-04-01T00:00:00Z"),
        ];
        assert!(plan_of(&transactions, &HashMap::new()).shifts.is_empty());
    }

    #[test]
    fn same_second_corrections_follow_the_rebase_order() {
        // Both correct at one second; "ca" was created first, so it counts
        // into the shift of "cb" and not the reverse.
        let transactions = vec![
            topup("t1", "a", "100", "2026-01-01T00:00:00Z"),
            topup("t2", "b", "200", "2026-01-01T00:00:00Z"),
            correction("cb", "b", "200", "250", "2026-02-01T00:00:00Z"),
            correction("ca", "a", "100", "70", "2026-02-01T00:00:00Z"),
        ];
        let created = HashMap::from([
            ("ca".to_string(), CreatedAt((1, 0, "d".into()))),
            ("cb".to_string(), CreatedAt((2, 0, "d".into()))),
        ]);
        let plan = plan_of(&transactions, &created);
        let of = |id: &str| plan.shifts.iter().find(|shift| shift.tx_id == id).unwrap();
        // ca sees only b's 200 (cb ranks after it); cb sees a's 70 after ca.
        assert_eq!(of("ca").other_balance, money("200"));
        assert_eq!(of("ca").after_new, money("270"));
        assert_eq!(of("cb").other_balance, money("70"));
        assert_eq!(of("cb").after_new, money("320"));
        // deltas preserved: merged 100 + 200 -> 270 (-30) -> 320 (+50).
    }
}
