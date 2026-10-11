//! The danger zone's «Сменить тип или перенести» (plan §4): change a
//! wallet's kind, move an account into another wallet (of any kind), make a
//! sub-account a wallet of its own, or merge one account into another.
//!
//! Every change is first worked out on the document in memory: the plan says
//! what moves along, what is split off and archived, and which operations
//! would no longer fit. Applying writes a `.bak` of the vault first and then
//! the whole change at once — or nothing.

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::model::store::{
    AssetRef, Binding, BindingShape, Custody, ExchangeProfile, MarketKind, Store, StoreKind,
    SubType,
};
use crate::model::transaction::{LegRole, Transaction, allowed_kinds};
use crate::model::vault::VaultDoc;
use crate::storage::VaultFile;

use super::UnlockedVault;

/// What to do with an account.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreChangeInput {
    pub store_id: String,
    pub change: StoreChange,
}

/// One account the change touches.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreChangeItem {
    pub store_id: String,
    pub name: String,
    pub kind_after: StoreKind,
    pub sub_type_before: Option<SubType>,
    pub sub_type_after: Option<SubType>,
}

/// An operation that would no longer fit the account it is on.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreChangeConflict {
    pub tx_id: String,
    pub timestamp: String,
    pub comment: String,
    pub message: String,
}

/// What a change would do, before it is applied.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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
    /// Why the change cannot be made at all.
    pub error: Option<String>,
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
fn retype(sub_type: Option<SubType>, binding: &Binding, kind: StoreKind) -> Option<SubType> {
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
fn replace_id(value: &mut serde_json::Value, from: &str, to: &str) {
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

impl UnlockedVault {
    /// What a change would do; nothing is written.
    pub fn plan_store_change(&mut self, input: &StoreChangeInput) -> StoreChangePlan {
        let backup = self.doc.clone();
        let plan = self.change_in_memory(input);
        self.doc = backup;
        plan
    }

    /// Make the change: refused while it has an error or a conflict. The vault
    /// file is copied to `.bak` first.
    pub fn apply_store_change(&mut self, input: &StoreChangeInput) -> Result<StoreChangePlan> {
        let backup = self.doc.clone();
        let plan = self.change_in_memory(input);
        let refusal = plan.error.clone().or_else(|| {
            (!plan.conflicts.is_empty()).then(|| {
                format!(
                    "{} operations would not fit the new account",
                    plan.conflicts.len()
                )
            })
        });
        if let Some(message) = refusal {
            self.doc = backup;
            return Err(Error::InvalidInput(message));
        }
        VaultFile::backup(&self.path)?;
        self.recompute_holdings();
        if let Err(err) = self.persist() {
            self.doc = backup;
            self.recompute_holdings();
            return Err(err);
        }
        Ok(plan)
    }

    /// Apply the change to `self.doc` and report on it. On an error the
    /// document may be half-changed: callers restore their copy.
    fn change_in_memory(&mut self, input: &StoreChangeInput) -> StoreChangePlan {
        let mut plan = StoreChangePlan {
            store_id: input.store_id.clone(),
            ..StoreChangePlan::default()
        };
        let result = match &input.change {
            StoreChange::ChangeKind { kind } => self.change_kind(&input.store_id, *kind, &mut plan),
            StoreChange::MoveTo { parent_id } => {
                self.move_to(&input.store_id, parent_id, &mut plan)
            }
            StoreChange::Detach => self.detach(&input.store_id, &mut plan),
            StoreChange::MergeInto { target_id } => {
                self.merge_into(&input.store_id, target_id, &mut plan)
            }
        };
        match result {
            Ok(affected) => plan.conflicts = self.conflicts_on(&affected),
            Err(err) => plan.error = Some(format!("{}: {err}", err.code())),
        }
        plan
    }

    fn store_mut(&mut self, id: &str) -> Result<&mut Store> {
        self.doc
            .stores
            .iter_mut()
            .find(|store| store.id == id)
            .ok_or_else(|| Error::Missing(format!("store {id}")))
    }

    /// A name free among the wallets (or among the children of `parent`).
    fn free_name(&self, node: &str, parent: Option<&str>, wanted: &str) -> String {
        if self.ensure_unique_name(Some(node), parent, wanted).is_ok() {
            return wanted.to_string();
        }
        (2..)
            .map(|index| format!("{wanted} ({index})"))
            .find(|name| self.ensure_unique_name(Some(node), parent, name).is_ok())
            .unwrap_or_else(|| wanted.to_string())
    }

    /// Give a wallet the fields of its new kind and drop the old kind's.
    fn set_root_kind(&mut self, id: &str, kind: StoreKind) -> Result<()> {
        let leaving_exchange = {
            let store = self.store_mut(id)?;
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
            self.doc.exchanges.bybit.retain(|keys| keys.store_id != id);
        }
        Ok(())
    }

    /// Exchange keys stop booking fills on an account that is no longer an exchange's.
    fn forget_import_target(&mut self, id: &str) {
        for keys in &mut self.doc.exchanges.bybit {
            if keys.spot_store_id.as_deref() == Some(id) {
                keys.spot_store_id = None;
            }
            if keys.futures_store_id.as_deref() == Some(id) {
                keys.futures_store_id = None;
            }
        }
    }

    fn change_kind(
        &mut self,
        id: &str,
        kind: StoreKind,
        plan: &mut StoreChangePlan,
    ) -> Result<HashSet<String>> {
        let root = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {id}")))?;
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
        self.set_root_kind(id, kind)?;
        let mut affected: HashSet<String> = HashSet::from([id.to_string()]);
        let children: Vec<Store> = self.children_of(id).into_iter().cloned().collect();
        let mut kept = vec![item(self.find_store(id).unwrap_or(&root), None)];
        for child in children {
            match retype(child.sub_type, &child.binding, kind) {
                Some(sub_type) => {
                    let store = self.store_mut(&child.id)?;
                    store.kind = kind;
                    store.sub_type = Some(sub_type);
                    if kind != StoreKind::Exchange {
                        self.forget_import_target(&child.id);
                    }
                    affected.insert(child.id.clone());
                    kept.push(item(
                        self.find_store(&child.id).unwrap_or(&child),
                        child.sub_type,
                    ));
                }
                None => {
                    // Split off as an archived wallet of the old kind: its
                    // operations stay valid where they are.
                    let name =
                        self.free_name(&child.id, None, &format!("{} · {}", root.name, child.name));
                    let store = self.store_mut(&child.id)?;
                    store.parent_id = None;
                    store.sub_type = None;
                    store.binding = Binding::Any;
                    store.archived = true;
                    store.name = name;
                    store.exchange =
                        (store.kind == StoreKind::Exchange).then(ExchangeProfile::default);
                    plan.archived.push(item(store, child.sub_type));
                }
            }
        }
        plan.kept = kept;
        Ok(affected)
    }

    fn move_to(
        &mut self,
        id: &str,
        parent_id: &str,
        plan: &mut StoreChangePlan,
    ) -> Result<HashSet<String>> {
        let store = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {id}")))?;
        let parent = self
            .find_store(parent_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {parent_id}")))?;
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
            let sub_type =
                retype(store.sub_type, &store.binding, parent.kind).ok_or_else(|| {
                    Error::InvalidInput(format!(
                        "a {} wallet has no account of this type",
                        parent.kind_label()
                    ))
                })?;
            (sub_type, store.binding.clone())
        } else {
            // A wallet moved in becomes an account of one asset.
            if !self.children_of(id).is_empty() {
                return Err(Error::InvalidInput(
                    "move or merge its sub-accounts first: an account holds no accounts".into(),
                ));
            }
            self.root_as_account(&store, parent.kind)?
        };
        let name = self.free_name(id, Some(parent_id), &store.name);
        let was_exchange_root = store.parent_id.is_none() && store.kind == StoreKind::Exchange;
        {
            let target = self.store_mut(id)?;
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
            self.doc.exchanges.bybit.retain(|keys| keys.store_id != id);
        }
        if parent.kind != StoreKind::Exchange {
            self.forget_import_target(id);
        }
        if let Some(moved) = self.find_store(id) {
            plan.kept.push(item(moved, store.sub_type));
        }
        Ok(HashSet::from([id.to_string()]))
    }

    /// The type and binding a wallet takes as an account of a `kind` wallet:
    /// the asset it holds, which has to be one.
    fn root_as_account(&self, store: &Store, kind: StoreKind) -> Result<(SubType, Binding)> {
        let held: Vec<&AssetRef> = self
            .doc
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

    fn detach(&mut self, id: &str, plan: &mut StoreChangePlan) -> Result<HashSet<String>> {
        let store = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {id}")))?;
        let Some(parent_id) = store.parent_id.clone() else {
            return Err(Error::InvalidInput("it is a wallet already".into()));
        };
        if !store.kind.user_creatable() || store.kind.max_roots().is_some() {
            return Err(Error::InvalidInput(
                "this kind has a single wallet; move the account instead".into(),
            ));
        }
        let parent_name = self
            .find_store(&parent_id)
            .map(|parent| parent.name.clone())
            .unwrap_or_default();
        let name = self.free_name(id, None, &store.name);
        let name = if name == store.name {
            name
        } else {
            self.free_name(id, None, &format!("{} ({parent_name})", store.name))
        };
        {
            let target = self.store_mut(id)?;
            target.parent_id = None;
            target.sub_type = None;
            target.binding = Binding::Any;
            target.name = name;
            target.custody = (target.kind == StoreKind::Crypto).then_some(Custody::Hot);
            target.exchange = (target.kind == StoreKind::Exchange).then(ExchangeProfile::default);
        }
        if let Some(moved) = self.find_store(id) {
            plan.kept.push(item(moved, store.sub_type));
        }
        Ok(HashSet::from([id.to_string()]))
    }

    fn merge_into(
        &mut self,
        id: &str,
        target_id: &str,
        plan: &mut StoreChangePlan,
    ) -> Result<HashSet<String>> {
        let source = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {id}")))?;
        let target = self
            .find_store(target_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {target_id}")))?;
        if source.id == target.id {
            return Err(Error::InvalidInput("merge into another account".into()));
        }
        if !self.children_of(id).is_empty() {
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
        plan.moved_transactions = self
            .doc
            .transactions
            .iter()
            .filter(|tx| tx.touches(id))
            .count() as u32;
        // Drop the account itself, then point every reference at the target.
        self.doc.stores.retain(|store| store.id != id);
        self.doc.exchanges.bybit.retain(|keys| keys.store_id != id);
        let mut value =
            serde_json::to_value(&self.doc).map_err(|err| Error::Serialize(err.to_string()))?;
        replace_id(&mut value, id, target_id);
        self.doc = serde_json::from_value::<VaultDoc>(value)
            .map_err(|err| Error::Deserialize(err.to_string()))?;
        plan.kept.push(item(&source, source.sub_type));
        Ok(HashSet::from([target_id.to_string()]))
    }

    /// The operations on `stores` that break the rules of the account they are
    /// on now: what the kind accepts, what the account may hold, both sides on
    /// one account.
    fn conflicts_on(&self, stores: &HashSet<String>) -> Vec<StoreChangeConflict> {
        self.doc
            .transactions
            .iter()
            .filter(|tx| stores.iter().any(|id| tx.touches(id)))
            .filter_map(|tx| {
                self.store_rule_violation(tx)
                    .map(|message| StoreChangeConflict {
                        tx_id: tx.meta().id.clone(),
                        timestamp: tx.meta().timestamp.clone(),
                        comment: tx.meta().comment.clone(),
                        message,
                    })
            })
            .collect()
    }

    fn store_rule_violation(&self, tx: &Transaction) -> Option<String> {
        for id in Self::store_sides(tx) {
            let store = self.find_store(id)?;
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
            let store = self.find_store(&leg.store_id)?;
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
}

impl Store {
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
    use super::super::test_support::*;
    use super::*;
    use crate::model::chain::Chain;
    use crate::service::StoreExtras;

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "RUB")
    }

    fn sub(
        vault: &mut UnlockedVault,
        parent: &str,
        name: &str,
        sub_type: SubType,
        binding: Binding,
    ) -> String {
        vault
            .create_store_with(
                name,
                StoreKind::Bank,
                "#000",
                "",
                Some(parent.into()),
                StoreExtras {
                    sub_type: Some(sub_type),
                    binding: Some(binding),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    fn change(store_id: &str, change: StoreChange) -> StoreChangeInput {
        StoreChangeInput {
            store_id: store_id.into(),
            change,
        }
    }

    #[test]
    fn a_bank_becomes_a_crypto_wallet_and_its_deposit_is_split_off() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = sub(
            &mut vault,
            &bank,
            "Card",
            SubType::Card,
            Binding::Asset(rub()),
        );
        let deposit = sub(
            &mut vault,
            &bank,
            "Deposit",
            SubType::Deposit,
            Binding::Asset(rub()),
        );
        vault
            .add_transaction(income_of(&card, rub(), "100", "0.01", "salary"))
            .unwrap();

        let input = change(
            &bank,
            StoreChange::ChangeKind {
                kind: StoreKind::Crypto,
            },
        );
        let plan = vault.plan_store_change(&input);
        assert_eq!(plan.error, None);
        assert!(plan.conflicts.is_empty(), "{:?}", plan.conflicts);
        assert_eq!(plan.kept.len(), 2);
        assert_eq!(plan.kept[1].sub_type_after, Some(SubType::Token));
        assert_eq!(plan.archived.len(), 1);
        assert_eq!(
            vault.find_store(&bank).unwrap().kind,
            StoreKind::Bank,
            "a plan changes nothing"
        );

        vault.apply_store_change(&input).unwrap();
        assert_eq!(vault.find_store(&bank).unwrap().kind, StoreKind::Crypto);
        let card_after = vault.find_store(&card).unwrap();
        assert_eq!(card_after.kind, StoreKind::Crypto);
        assert_eq!(card_after.sub_type, Some(SubType::Token));
        let split = vault.find_store(&deposit).unwrap();
        assert!(split.archived && split.parent_id.is_none());
        assert_eq!(split.kind, StoreKind::Bank);
        assert_eq!(holding_of(&vault, &card, &rub()), amount("100"));
        assert!(vault.path.with_file_name("test.wallet.bak").exists());
    }

    #[test]
    fn a_wallet_holding_tokens_cannot_become_a_bank() {
        let (_dir, mut vault) = setup();
        let cold = vault.doc.stores[2].id.clone();
        vault
            .add_transaction(income_of(&cold, btc(), "1", "30000", "bought"))
            .unwrap();
        let input = change(
            &cold,
            StoreChange::ChangeKind {
                kind: StoreKind::Bank,
            },
        );
        let plan = vault.plan_store_change(&input);
        assert_eq!(plan.conflicts.len(), 1);
        assert!(vault.apply_store_change(&input).is_err());
        assert_eq!(vault.find_store(&cold).unwrap().kind, StoreKind::Crypto);
    }

    #[test]
    fn a_card_moves_to_another_bank_and_a_token_account_to_a_crypto_wallet() {
        let (_dir, mut vault) = empty_vault();
        let first = store(&mut vault, "First", StoreKind::Bank, None);
        let second = store(&mut vault, "Second", StoreKind::Bank, None);
        let card = sub(
            &mut vault,
            &first,
            "Card",
            SubType::Card,
            Binding::Asset(rub()),
        );
        vault
            .apply_store_change(&change(
                &card,
                StoreChange::MoveTo {
                    parent_id: second.clone(),
                },
            ))
            .unwrap();
        assert_eq!(
            vault.find_store(&card).unwrap().parent_id.as_deref(),
            Some(second.as_str())
        );

        // A card bound to rubles under a crypto wallet becomes a token account.
        let wallet = store(&mut vault, "Wallet", StoreKind::Crypto, None);
        vault
            .apply_store_change(&change(
                &card,
                StoreChange::MoveTo {
                    parent_id: wallet.clone(),
                },
            ))
            .unwrap();
        let moved = vault.find_store(&card).unwrap();
        assert_eq!(
            (moved.kind, moved.sub_type),
            (StoreKind::Crypto, Some(SubType::Token))
        );
    }

    #[test]
    fn a_sub_account_becomes_a_wallet_and_a_one_asset_wallet_an_account() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = sub(
            &mut vault,
            &bank,
            "Card",
            SubType::Card,
            Binding::Asset(rub()),
        );
        vault
            .apply_store_change(&change(&card, StoreChange::Detach))
            .unwrap();
        let alone = vault.find_store(&card).unwrap();
        assert!(alone.parent_id.is_none() && alone.sub_type.is_none());

        vault
            .add_transaction(income_of(&card, rub(), "10", "0.01", "salary"))
            .unwrap();
        vault
            .apply_store_change(&change(
                &card,
                StoreChange::MoveTo {
                    parent_id: bank.clone(),
                },
            ))
            .unwrap();
        let back = vault.find_store(&card).unwrap();
        assert_eq!(back.sub_type, Some(SubType::Card));
        assert_eq!(back.binding, Binding::Asset(rub()));
    }

    #[test]
    fn merging_moves_every_operation_and_drops_the_account() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let first = sub(
            &mut vault,
            &bank,
            "Card 1",
            SubType::Card,
            Binding::Asset(rub()),
        );
        let second = sub(
            &mut vault,
            &bank,
            "Card 2",
            SubType::Card,
            Binding::Asset(rub()),
        );
        vault
            .add_transaction(income_of(&first, rub(), "100", "0.01", "salary"))
            .unwrap();
        vault
            .add_transaction(income_of(&second, rub(), "50", "0.01", "salary"))
            .unwrap();
        let plan = vault
            .apply_store_change(&change(
                &first,
                StoreChange::MergeInto {
                    target_id: second.clone(),
                },
            ))
            .unwrap();
        assert_eq!(plan.moved_transactions, 1);
        assert!(vault.find_store(&first).is_none());
        assert_eq!(holding_of(&vault, &second, &rub()), amount("150"));

        // A transfer between the two would land on one account: refused.
        let third = sub(
            &mut vault,
            &bank,
            "Card 3",
            SubType::Card,
            Binding::Asset(rub()),
        );
        vault
            .add_transaction(transfer_between(
                crate::model::transaction::AccountRef::Store(second.clone()),
                crate::model::transaction::AccountRef::Store(third.clone()),
                rub(),
                "10",
            ))
            .unwrap();
        let plan = vault.plan_store_change(&change(
            &third,
            StoreChange::MergeInto { target_id: second },
        ));
        assert_eq!(plan.conflicts.len(), 1);
    }

    #[test]
    fn retype_maps_one_asset_accounts_between_kinds() {
        let btc_binding = Binding::Asset(AssetRef::crypto("BTC", "Bitcoin", None, Chain::Bitcoin));
        assert_eq!(
            retype(Some(SubType::Token), &btc_binding, StoreKind::Bank),
            None,
            "a bank holds no bitcoin"
        );
        assert_eq!(
            retype(
                Some(SubType::Funding),
                &Binding::Market(MarketKind::Funding),
                StoreKind::Bank
            ),
            None
        );
        assert_eq!(
            retype(
                Some(SubType::Account),
                &Binding::Asset(rub()),
                StoreKind::Bank
            ),
            Some(SubType::Account),
            "an older account stays what it is"
        );
    }
}
