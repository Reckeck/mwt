//! Domain services: everything the app does on top of the [`VaultDoc`].
//! This module is pure Rust (no Tauri); the `app` crate only maps commands.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::Amount;
use crate::error::{Error, Result, StoreTreeViolation};
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, Holding, MAX_STORE_DEPTH, Store, StoreKind, StoreScope};
use crate::model::transaction::{ExchangeTradeTx, Leg, PriceStamp, TradeSide, Transaction, TxMeta};
use crate::model::vault::{PendingImport, VaultDoc, iso_to_millis, now_iso};
use crate::storage::{SnapshotFormat, VaultFile};
use crate::txinfo::{HashShape, hash_shape};

/// Upper bound on staged imports held in the vault. A background poll must not
/// be able to grow the encrypted file without limit.
pub const MAX_PENDING_IMPORTS: usize = 10_000;

/// An unlocked vault held in memory for the duration of a session.
/// The master password lives only here (zeroized on drop) — it is never
/// written to disk and is required again on the next unlock.
pub struct UnlockedVault {
    pub path: PathBuf,
    pub fmt: SnapshotFormat,
    pub doc: VaultDoc,
    /// Portfolio base currency for this session.
    ///
    /// Deliberately *not* part of `doc`: the base currency is an app-level
    /// preference (see [`crate::appconfig::AppSettings`]), so opening a
    /// different vault must not change it. The app layer pushes the current
    /// value in right after the vault is opened.
    base_currency: String,
    password: Zeroizing<String>,
}

impl UnlockedVault {
    pub fn create(path: PathBuf, password: &str) -> Result<Self> {
        VaultFile::create(&path, password)?;
        let doc = VaultDoc::default();
        let fmt = SnapshotFormat::for_profile();
        Ok(Self {
            path,
            fmt,
            doc,
            base_currency: crate::appconfig::DEFAULT_BASE_CURRENCY.to_string(),
            password: Zeroizing::new(password.to_string()),
        })
    }

    pub fn open(path: PathBuf, password: &str) -> Result<Self> {
        let (doc, fmt) = VaultFile::open(&path, password)?;
        Ok(Self {
            path,
            fmt,
            doc,
            base_currency: crate::appconfig::DEFAULT_BASE_CURRENCY.to_string(),
            password: Zeroizing::new(password.to_string()),
        })
    }

    /// Point the vault at the app-level base currency.
    ///
    /// Holdings, average costs and every "in base" figure are re-derived, so an
    /// existing vault keeps reporting the right numbers after the switch. The
    /// document itself is not rewritten — only the document's *history* values
    /// (`price_base` stamps) stay as they were written.
    pub fn set_base_currency(&mut self, base: &str) {
        let base = base.trim().to_lowercase();
        if base.is_empty() || base == self.base_currency {
            return;
        }
        self.base_currency = base;
        self.recompute_holdings();
    }

    /// Current base currency, never empty.
    pub fn base_currency(&self) -> &str {
        &self.base_currency
    }

    /// Persist the current document (encrypt + atomic write).
    pub fn persist(&self) -> Result<()> {
        VaultFile::save(&self.path, &self.password, &self.doc, self.fmt)
    }

    /// Replace the whole document with an externally supplied one and persist
    /// it (raw JSON editor).
    ///
    /// Holdings are re-derived from the transactions rather than trusted from
    /// the payload, so an edited `holdings` array can never disagree with the
    /// transactions it summarizes. `id` and `created_at` are pinned to the
    /// current document: a replace edits *content*, it does not re-create the
    /// vault, and a changed id would orphan every store in the recents list.
    pub fn replace_doc(&mut self, mut doc: VaultDoc) -> Result<()> {
        doc.id = self.doc.id.clone();
        doc.created_at = self.doc.created_at.clone();
        doc.version = crate::VAULT_FORMAT_VERSION;
        self.doc = doc;
        self.recompute_holdings();
        self.persist()
    }

    /// Try opening with a possibly-wrong password (used by unlock screen).
    pub fn try_open(path: PathBuf, password: &str) -> Result<Self> {
        Self::open(path, password)
    }

    pub fn change_password(&mut self, new_password: &str) -> Result<()> {
        self.password = Zeroizing::new(new_password.to_string());
        self.persist()
    }

    /// Re-encode on disk (dev JSON <-> prod binary) — mostly used by tests.
    pub fn switch_format(&mut self, fmt: SnapshotFormat) -> Result<()> {
        self.fmt = fmt;
        self.persist()
    }

    // -- stores -------------------------------------------------------------
    //
    // Stores form a tree of at most `MAX_STORE_DEPTH + 1` levels. Every write
    // that places a node goes through `validate_placement`, so the tree rules
    // live in one spot; reads stay lenient so a hand-edited vault still opens.

    fn find_store(&self, id: &str) -> Option<&Store> {
        self.doc.stores.iter().find(|store| store.id == id)
    }

    /// Parent chain from the root down to `id`'s parent. Stops at a missing
    /// parent or at a cycle left behind by a hand-edited vault.
    fn ancestors_of(&self, id: &str) -> Vec<&Store> {
        let mut chain = Vec::new();
        let mut seen = HashSet::from([id.to_string()]);
        let mut parent = self
            .find_store(id)
            .and_then(|store| store.parent_id.as_deref());
        while let Some(parent_id) = parent {
            if !seen.insert(parent_id.to_string()) {
                break;
            }
            let Some(store) = self.find_store(parent_id) else {
                break;
            };
            chain.push(store);
            parent = store.parent_id.as_deref();
        }
        chain.reverse();
        chain
    }

    /// 0 for a top-level wallet.
    pub fn depth(&self, id: &str) -> usize {
        self.ancestors_of(id).len()
    }

    /// Top-level wallet that `id` belongs to (itself for a root).
    pub fn root_of(&self, id: &str) -> String {
        self.ancestors_of(id)
            .first()
            .map(|store| store.id.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// Levels below `id`: 0 for a leaf.
    fn subtree_height(&self, id: &str) -> usize {
        let mut seen = HashSet::from([id.to_string()]);
        let mut level = vec![id.to_string()];
        let mut height = 0;
        loop {
            let next: Vec<String> = level
                .iter()
                .flat_map(|parent| self.children_of(parent))
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
    fn ensure_unique_name(
        &self,
        node: Option<&str>,
        parent: Option<&str>,
        name: &str,
    ) -> Result<()> {
        let wanted = name.trim().to_lowercase();
        let taken = self.doc.stores.iter().any(|store| {
            node != Some(store.id.as_str())
                && store.parent_id.as_deref() == parent
                && store.name.trim().to_lowercase() == wanted
        });
        if taken {
            return Err(Error::StoreTree(StoreTreeViolation::NameTaken));
        }
        Ok(())
    }

    /// Whether `node` (an existing store, or `None` for a new one) may sit
    /// under `parent` with `name`. Rules R1–R6.
    fn validate_placement(
        &self,
        node: Option<&str>,
        parent: Option<&str>,
        name: &str,
    ) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::InvalidInput("store name must not be empty".into()));
        }
        if let Some(parent_id) = parent {
            let parent_store = self
                .find_store(parent_id)
                .ok_or_else(|| Error::Missing("parent store".into()))?;
            if let Some(node_id) = node {
                if self
                    .find_store(node_id)
                    .is_some_and(|store| store.parent_id.is_none())
                {
                    return Err(Error::StoreTree(StoreTreeViolation::RootMove));
                }
                if parent_id == node_id
                    || self.subtree_ids(node_id).iter().any(|id| id == parent_id)
                {
                    return Err(Error::StoreTree(StoreTreeViolation::Cycle));
                }
            }
            let height = node.map_or(0, |id| self.subtree_height(id));
            if self.depth(parent_id) + 1 + height > MAX_STORE_DEPTH {
                return Err(Error::StoreTree(StoreTreeViolation::TooDeep));
            }
            if parent_store.archived {
                return Err(Error::StoreTree(StoreTreeViolation::ParentArchived));
            }
        }
        self.ensure_unique_name(node, parent, name)
    }

    pub fn create_store(
        &mut self,
        name: &str,
        kind: StoreKind,
        color: &str,
        note: &str,
        parent_id: Option<String>,
    ) -> Result<Store> {
        self.validate_placement(None, parent_id.as_deref(), name)?;
        let mut store = Store::new(name, kind, color);
        store.note = note.trim().to_string();
        store.parent_id = parent_id;
        self.doc.stores.push(store.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok(store)
    }

    /// Edit a store's own fields. Its place in the tree changes only through
    /// [`Self::move_store`].
    pub fn update_store(&mut self, id: &str, name: &str, color: &str, note: &str) -> Result<Store> {
        let parent = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?
            .parent_id
            .clone();
        if !name.trim().is_empty() {
            self.ensure_unique_name(Some(id), parent.as_deref(), name)?;
        }
        let out = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            if !name.trim().is_empty() {
                store.name = name.trim().to_string();
            }
            store.color = color.trim().to_string();
            store.note = note.trim().to_string();
            store.clone()
        };
        self.persist()?;
        Ok(out)
    }

    /// Re-hang a sub-account under another node, or make it a top-level
    /// wallet with `parent_id = None`. A top-level wallet never moves (R6).
    pub fn move_store(&mut self, id: &str, parent_id: Option<&str>) -> Result<Store> {
        let store = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing("store".into()))?;
        if store.parent_id.as_deref() == parent_id {
            return Ok(store);
        }
        self.validate_placement(Some(id), parent_id, &store.name)?;
        let out = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            store.parent_id = parent_id.map(str::to_string);
            store.clone()
        };
        self.persist()?;
        Ok(out)
    }

    /// Where `id` may be moved: every allowed parent (flat, without the
    /// current one) and whether it may become a top-level wallet.
    pub fn store_move_targets(&self, id: &str) -> Result<StoreMoveTargets> {
        let store = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        if store.parent_id.is_none() {
            return Ok(StoreMoveTargets::default());
        }
        let parents = flatten_tree(self.store_tree())
            .into_iter()
            .filter(|node| store.parent_id.as_deref() != Some(node.store.id.as_str()))
            .filter(|node| {
                self.validate_placement(Some(id), Some(&node.store.id), &store.name)
                    .is_ok()
            })
            .collect();
        Ok(StoreMoveTargets {
            parents,
            can_detach: self.ensure_unique_name(Some(id), None, &store.name).is_ok(),
        })
    }

    /// Archive (or restore) a store together with its whole subtree. A node
    /// cannot be restored while an ancestor is still archived (R8).
    pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> {
        let store = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        if archived && store.archived {
            return Err(Error::InvalidInput("already archived".into()));
        }
        if !archived && self.ancestors_of(id).iter().any(|parent| parent.archived) {
            return Err(Error::StoreTree(StoreTreeViolation::RestoreParentFirst));
        }
        let mut affected: HashSet<String> = self.subtree_ids(id).into_iter().collect();
        affected.insert(id.to_string());
        for store in &mut self.doc.stores {
            if affected.contains(&store.id) {
                store.archived = archived;
            }
        }
        self.persist()
    }

    pub fn delete_store(&mut self, id: &str) -> Result<()> {
        if self.doc.transactions.iter().any(|tx| tx.touches(id)) {
            return Err(Error::InvalidInput(
                "store has transactions; archive instead".into(),
            ));
        }
        if !self.children_of(id).is_empty() {
            return Err(Error::InvalidInput(
                "store has sub-accounts; move or delete them first".into(),
            ));
        }
        let before = self.doc.stores.len();
        self.doc.stores.retain(|store| store.id != id);
        self.doc.holdings.retain(|holding| holding.store_id != id);
        if self.doc.stores.len() == before {
            return Err(Error::Missing("store".into()));
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(())
    }

    // -- transactions -------------------------------------------------------
    //
    // Holdings are DERIVED from transactions (single source of truth):
    // `recompute_holdings()` re-derives every position and average cost basis
    // from the transaction stream, guaranteeing consistency.
    //
    // `Transaction` is a closed enum whose variants carry their own fields, so
    // there is no kind tag to dispatch on here: one entry point validates
    // whichever variant it is handed. Adding a variant does not mean adding a
    // branch here, only a `legs()` impl on the variant itself.

    /// Transaction history, newest first, limited to a scope.
    pub fn transactions(&self, scope: &StoreScope) -> Vec<Transaction> {
        let ids = (!scope.store_ids.is_empty()).then(|| self.resolve_scope(scope));
        let mut out: Vec<Transaction> = match ids {
            Some(ids) => self.scoped_txs(&ids),
            None => self.doc.transactions.clone(),
        };
        out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
        out
    }

    /// Validate and append a transaction of any variant.
    ///
    /// Everything kind-specific is already guaranteed by the type system; what
    /// is left is the cross-cutting set of rules — the stores exist, amounts
    /// carry a real magnitude, and any on-chain hash is well-formed.
    pub fn add_transaction(&mut self, mut tx: Transaction) -> Result<Transaction> {
        let legs = tx.legs();
        if legs.is_empty() {
            return Err(Error::InvalidInput("transaction has no legs".into()));
        }
        for leg in &legs {
            self.ensure_store(&leg.store_id)?;
            if leg.amount.is_zero() {
                return Err(Error::InvalidInput("amount must not be zero".into()));
            }
        }
        self.check_on_chain_refs(&tx)?;
        // Two legs must never book the same asset into the same store: they
        // would net to nothing while the history still showed both rows. A
        // trade legitimately puts two legs on one store (different assets), so
        // the pair — not the store alone — is what has to be distinct.
        for (index, leg) in legs.iter().enumerate() {
            if legs[..index].iter().any(|earlier| {
                earlier.store_id == leg.store_id && earlier.asset.key() == leg.asset.key()
            }) {
                return Err(Error::InvalidInput(
                    "legs of one transaction must not net against each other".into(),
                ));
            }
        }
        self.adopt_categories(&mut tx.meta_mut().categories);
        self.doc.transactions.push(tx.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok(tx)
    }

    /// Normalize `categories` in place and add the ones the user has not seen
    /// before to the vault-wide vocabulary, so the picker always offers every
    /// tag that is actually in use.
    ///
    /// Canonical form: trimmed, lowercased, empty entries dropped, duplicates
    /// removed while keeping the input order. Two entries differing only in
    /// case or whitespace are the same tag — otherwise the category analytics
    /// would split one bucket in two.
    fn adopt_categories(&mut self, categories: &mut Vec<String>) {
        let mut seen = HashSet::new();
        categories.retain(|raw| {
            let normalized = raw.trim().to_lowercase();
            !normalized.is_empty() && seen.insert(normalized)
        });
        for raw in categories.iter_mut() {
            *raw = raw.trim().to_lowercase();
        }
        for category in categories.iter() {
            if !self.doc.categories.iter().any(|known| known == category) {
                self.doc.categories.push(category.clone());
            }
        }
    }

    pub fn delete_transaction(&mut self, id: &str) -> Result<()> {
        let before = self.doc.transactions.len();
        self.doc.transactions.retain(|tx| tx.meta().id != id);
        if self.doc.transactions.len() == before {
            return Err(Error::Missing("transaction".into()));
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(())
    }

    pub fn update_comment(&mut self, id: &str, comment: &str) -> Result<Transaction> {
        let tx = self
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        tx.meta_mut().comment = comment.trim().to_string();
        let out = tx.clone();
        self.persist()?;
        Ok(out)
    }

    /// Stamp a manual price / FX override on a transaction.
    ///
    /// Delegated to [`Transaction::stamp_prices`], which knows which legs of
    /// which variant a price belongs on.
    pub fn set_prices(
        &mut self,
        tx_id: &str,
        price_usd: Option<Amount>,
        fx_override: Option<Amount>,
    ) -> Result<()> {
        let tx = self
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().id == tx_id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        tx.stamp_prices(price_usd, fx_override);
        self.recompute_holdings();
        self.persist()?;
        Ok(())
    }

    // -- categories ---------------------------------------------------------

    pub fn add_category(&mut self, category: &str) -> Result<String> {
        let normalized = category.trim().to_lowercase();
        if normalized.is_empty() {
            return Err(Error::InvalidInput("empty category".into()));
        }
        if !self
            .doc
            .categories
            .iter()
            .any(|existing| existing == &normalized)
        {
            self.doc.categories.push(normalized.clone());
            self.persist()?;
        }
        Ok(normalized)
    }

    // -- holdings recomputation ---------------------------------------------

    fn recompute_holdings(&mut self) {
        // key: (store_id, asset_key) -> (quantity, avg_cost)
        let mut qty: HashMap<(String, String), Amount> = HashMap::new();
        let mut cost: HashMap<(String, String), Amount> = HashMap::new();
        let base = self.base_currency.to_ascii_lowercase();
        let base = if base.is_empty() {
            crate::appconfig::DEFAULT_BASE_CURRENCY.to_string()
        } else {
            base
        };

        // Replay the whole stream leg by leg in timestamp order. Sorting on the
        // transaction is enough because every leg of a transaction shares its
        // timestamp, and within one timestamp the legs are independent
        // per (store, asset) key.
        let mut txs: Vec<Transaction> = self.doc.transactions.clone();
        txs.sort_by(|left, right| left.meta().timestamp.cmp(&right.meta().timestamp));

        for tx in &txs {
            for leg in tx.legs() {
                let key = (leg.store_id.clone(), leg.asset.key());
                let delta = leg.amount;
                if delta.is_zero() {
                    continue;
                }
                let price = leg_price_in_base(&leg, &base);
                let cur = qty.get(&key).copied().unwrap_or(Amount::ZERO);
                let new_qty = cur + delta;

                if delta.is_positive() && !price.is_zero() {
                    // weighted average cost
                    let new_cost = {
                        let old = cost.get(&key).copied().unwrap_or(Amount::ZERO);
                        let total_cost = old * cur + price * delta;
                        if new_qty == Amount::ZERO {
                            Amount::ZERO
                        } else {
                            total_cost / new_qty
                        }
                    };
                    cost.insert(key.clone(), new_cost);
                }
                // sells keep avg_cost unchanged (realized PnL measured against
                // it is computed on demand in analytics)
                qty.insert(key, new_qty);
            }
        }

        // Recover the full AssetRef for each surviving position from the legs
        // that produced it, rather than reconstructing an identity from the
        // cache key (which would lose the name, quote and chain).
        let mut identity: HashMap<(String, String), AssetRef> = HashMap::new();
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                identity
                    .entry((leg.store_id.clone(), leg.asset.key()))
                    .or_insert_with(|| leg.asset.clone());
            }
        }

        self.doc.holdings = qty
            .into_iter()
            .filter(|(_, quantity)| !quantity.is_zero())
            .map(|((store_id, asset_key), quantity)| {
                let asset = identity
                    .remove(&(store_id.clone(), asset_key.clone()))
                    .unwrap_or_else(|| AssetRef::fiat("UNKNOWN", "Unknown"));
                let avg_cost = cost.get(&(store_id.clone(), asset_key.clone())).copied();
                Holding {
                    id: uuid::Uuid::new_v4().to_string(),
                    store_id,
                    asset,
                    quantity,
                    avg_cost,
                }
            })
            .collect();
    }

    // -- analytics ----------------------------------------------------------

    /// Base currency code (never empty) and its cached FX rate.
    fn base_and_fx(&self) -> (String, Amount) {
        let base = self.base_currency.to_ascii_lowercase();
        let base = if base.is_empty() {
            crate::appconfig::DEFAULT_BASE_CURRENCY.to_string()
        } else {
            base
        };
        let fx = self
            .doc
            .fx_cache
            .iter()
            .find(|point| point.currency == base)
            .map(|point| point.per_usd)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        (base, fx)
    }

    /// Newest cached USD price for an asset key, or zero.
    fn price_usd_for(&self, asset_key: &str) -> Amount {
        self.doc
            .price_cache
            .iter()
            .filter(|point| point.asset_key == asset_key)
            .max_by(|left, right| left.ts.cmp(&right.ts))
            .map(|point| point.value_usd)
            .unwrap_or(Amount::ZERO)
    }

    /// Every store id a scope covers. An empty id list means the whole vault.
    pub fn resolve_scope(&self, scope: &StoreScope) -> HashSet<String> {
        if scope.store_ids.is_empty() {
            return self
                .doc
                .stores
                .iter()
                .map(|store| store.id.clone())
                .collect();
        }
        let mut out = HashSet::new();
        for id in &scope.store_ids {
            out.insert(id.clone());
            if scope.include_children {
                out.extend(self.subtree_ids(id));
            }
        }
        out
    }

    /// Transactions with at least one leg on a store in `ids`.
    fn scoped_txs(&self, ids: &HashSet<String>) -> Vec<Transaction> {
        self.doc
            .transactions
            .iter()
            .filter(|tx| ids.iter().any(|id| tx.touches(id)))
            .cloned()
            .collect()
    }

    /// Aggregate value of a store's holdings, sorted by value descending.
    fn store_positions(&self, store_id: &str, fx: Amount) -> Vec<StorePosition> {
        let mut out: Vec<StorePosition> = self
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == store_id)
            .map(|holding| {
                let price_usd = self.price_usd_for(&holding.asset.key());
                let value_usd = holding.quantity * price_usd;
                let price_base = price_usd * fx;
                let unrealized_base = match holding.avg_cost {
                    Some(cost) => (price_base - cost) * holding.quantity.abs(),
                    None => Amount::ZERO,
                };
                StorePosition {
                    asset: holding.asset.clone(),
                    quantity: holding.quantity,
                    avg_cost: holding.avg_cost,
                    price_usd,
                    value_usd,
                    value_base: value_usd * fx,
                    unrealized_base,
                }
            })
            .collect();
        out.sort_by_key(|position| std::cmp::Reverse(position.value_base));
        out
    }

    /// One row per asset across several stores. The average cost is weighted
    /// by quantity and only kept when every store knows its own.
    fn merged_positions(&self, ids: &[String], fx: Amount) -> Vec<StorePosition> {
        let mut merged: Vec<StorePosition> = Vec::new();
        let mut cost_known: HashMap<String, bool> = HashMap::new();
        for id in ids {
            for position in self.store_positions(id, fx) {
                let key = position.asset.key();
                let known = cost_known.entry(key.clone()).or_insert(true);
                *known &= position.avg_cost.is_some();
                match merged.iter_mut().find(|row| row.asset.key() == key) {
                    Some(row) => {
                        let quantity = row.quantity + position.quantity;
                        row.avg_cost = match (row.avg_cost, position.avg_cost) {
                            (Some(left), Some(right)) if !quantity.is_zero() => {
                                Some((left * row.quantity + right * position.quantity) / quantity)
                            }
                            _ => None,
                        };
                        row.quantity = quantity;
                        row.value_usd += position.value_usd;
                        row.value_base += position.value_base;
                        row.unrealized_base += position.unrealized_base;
                    }
                    None => merged.push(position),
                }
            }
        }
        for row in &mut merged {
            if !cost_known.get(&row.asset.key()).copied().unwrap_or(false) {
                row.avg_cost = None;
            }
        }
        merged.sort_by_key(|position| std::cmp::Reverse(position.value_base));
        merged
    }

    /// Direct children of a store.
    fn children_of(&self, parent_id: &str) -> Vec<&Store> {
        self.doc
            .stores
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(parent_id))
            .collect()
    }

    /// Every descendant id of `root_id`, without `root_id` itself.
    fn subtree_ids(&self, root_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut seen = HashSet::from([root_id.to_string()]);
        let mut queue = vec![root_id.to_string()];
        while let Some(id) = queue.pop() {
            for child in self.children_of(&id) {
                if seen.insert(child.id.clone()) {
                    result.push(child.id.clone());
                    queue.push(child.id.clone());
                }
            }
        }
        result
    }

    /// The whole wallet tree with own and rolled-up values. Roots (and
    /// orphans whose parent is gone) come first; live before archived,
    /// alphabetical inside each group.
    pub fn store_tree(&self) -> Vec<StoreTreeNode> {
        let (_base, fx) = self.base_and_fx();
        let mut own_usd: HashMap<&str, Amount> = HashMap::new();
        for holding in &self.doc.holdings {
            let value = holding.quantity * self.price_usd_for(&holding.asset.key());
            *own_usd.entry(holding.store_id.as_str()).or_default() += value;
        }
        let mut tx_count: HashMap<String, u32> = HashMap::new();
        for tx in &self.doc.transactions {
            let mut touched: HashSet<String> = HashSet::new();
            for leg in tx.legs() {
                touched.insert(leg.store_id.clone());
                if let Some(to) = leg.to_store_id {
                    touched.insert(to);
                }
            }
            for id in touched {
                *tx_count.entry(id).or_default() += 1;
            }
        }
        let ctx = TreeCtx {
            fx,
            own_usd,
            tx_count,
        };
        let mut seen = HashSet::new();
        let roots: Vec<&Store> = self
            .doc
            .stores
            .iter()
            .filter(|store| {
                store
                    .parent_id
                    .as_deref()
                    .is_none_or(|parent| self.find_store(parent).is_none())
            })
            .collect();
        let mut out: Vec<StoreTreeNode> = roots
            .into_iter()
            .map(|store| self.tree_node(store, 0, &[], &ctx, &mut seen))
            .collect();
        sort_tree(&mut out);
        out
    }

    fn tree_node(
        &self,
        store: &Store,
        depth: usize,
        parent_path: &[String],
        ctx: &TreeCtx,
        seen: &mut HashSet<String>,
    ) -> StoreTreeNode {
        seen.insert(store.id.clone());
        let mut path = parent_path.to_vec();
        path.push(store.name.clone());
        let mut children: Vec<StoreTreeNode> = self
            .children_of(&store.id)
            .into_iter()
            .filter(|child| !seen.contains(&child.id))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|child| self.tree_node(child, depth + 1, &path, ctx, seen))
            .collect();
        sort_tree(&mut children);
        let own_usd = ctx
            .own_usd
            .get(store.id.as_str())
            .copied()
            .unwrap_or(Amount::ZERO);
        let subtree_usd = children
            .iter()
            .fold(own_usd, |acc, child| acc + child.subtree_usd);
        StoreTreeNode {
            store: store.clone(),
            depth: depth as u32,
            path,
            own_usd,
            own_base: own_usd * ctx.fx,
            subtree_usd,
            subtree_base: subtree_usd * ctx.fx,
            own_tx_count: ctx.tx_count.get(&store.id).copied().unwrap_or(0),
            can_add_child: depth < MAX_STORE_DEPTH && !store.archived,
            children,
        }
    }

    /// Single-round-trip view of one store for the wallet page.
    /// `include_children` widens the totals, PnL and transaction stats to the
    /// whole subtree; positions are reported both ways regardless.
    pub fn store_overview(&self, store_id: &str, include_children: bool) -> Result<StoreOverview> {
        let store = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let (base, fx) = self.base_and_fx();
        let node = find_tree_node(self.store_tree(), store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;

        let mut subtree: Vec<String> = vec![store_id.to_string()];
        subtree.extend(self.subtree_ids(store_id));
        let scope = self.resolve_scope(&StoreScope::store(store_id, include_children));
        let txs = self.scoped_txs(&scope);
        let first_tx_at = txs
            .iter()
            .map(|tx| tx.meta().timestamp.as_str())
            .min()
            .unwrap_or("")
            .to_string();
        let last_tx_at = txs
            .iter()
            .map(|tx| tx.meta().timestamp.as_str())
            .max()
            .unwrap_or("")
            .to_string();

        let children = node
            .children
            .iter()
            .map(|child| StoreChildRow {
                store: child.store.clone(),
                value_usd: child.own_usd,
                value_base: child.own_base,
                subtree_base: child.subtree_base,
                share: if node.subtree_usd.is_zero() {
                    0.0
                } else {
                    (child.subtree_usd / node.subtree_usd).to_f64()
                },
            })
            .collect();

        Ok(StoreOverview {
            ancestors: self
                .ancestors_of(store_id)
                .into_iter()
                .map(|parent| StoreCrumb {
                    id: parent.id.clone(),
                    name: parent.name.clone(),
                    color: parent.color.clone(),
                })
                .collect(),
            store,
            base_currency: base,
            fx_per_usd: fx,
            include_children,
            own_usd: node.own_usd,
            own_base: node.own_base,
            subtree_usd: node.subtree_usd,
            subtree_base: node.subtree_base,
            realized_pnl_base: self.realized_pnl_filtered(Some(&scope)).total_base,
            positions: self.store_positions(store_id, fx),
            subtree_positions: self.merged_positions(&subtree, fx),
            children,
            can_add_child: node.can_add_child,
            tx_count: txs.len() as u32,
            first_tx_at,
            last_tx_at,
        })
    }

    /// Portfolio value of one store (optionally with its subtree) over time.
    pub fn store_history(
        &self,
        store_id: &str,
        include_children: bool,
        base_currency: &str,
    ) -> Vec<HistoryPoint> {
        let scope = self.resolve_scope(&StoreScope::store(store_id, include_children));
        self.history_series_filtered(Some(&scope), base_currency)
    }

    /// Realized PnL of one store (optionally with its subtree).
    pub fn store_pnl(&self, store_id: &str, include_children: bool) -> RealizedPnl {
        let scope = self.resolve_scope(&StoreScope::store(store_id, include_children));
        self.realized_pnl_filtered(Some(&scope))
    }

    pub fn portfolio_summary(&self) -> PortfolioSummary {
        let (base, fx) = self.base_and_fx();
        let mut total_usd = Amount::ZERO;
        let mut by_asset: HashMap<String, AssetStat> = HashMap::new();
        let mut by_chain: HashMap<String, Amount> = HashMap::new();
        let mut by_store: HashMap<String, Amount> = HashMap::new();

        for holding in &self.doc.holdings {
            let price = self.price_usd_for(&holding.asset.key());
            let value_usd = holding.quantity * price;
            let asset_key = holding.asset.key();

            let st = by_asset
                .entry(asset_key.clone())
                .or_insert_with(|| AssetStat {
                    asset: holding.asset.clone(),
                    quantity: Amount::ZERO,
                    price_usd: price,
                    value_usd: Amount::ZERO,
                });
            st.quantity += holding.quantity;
            st.price_usd = price;
            st.value_usd += value_usd;

            let chain_key = holding
                .asset
                .chain
                .map(|chain| chain.label().to_string())
                .unwrap_or_else(|| "Other".into());
            *by_chain.entry(chain_key).or_default() += value_usd;
            *by_store.entry(holding.store_id.clone()).or_default() += value_usd;
            total_usd += value_usd;
        }

        let total_base = total_usd * fx;
        let mut stats: Vec<AssetStat> = by_asset.into_values().collect();
        stats.sort_by_key(|stat| std::cmp::Reverse(stat.value_usd));
        let pct = |value: Amount| {
            if total_usd == Amount::ZERO {
                0.0
            } else {
                (value / total_usd).to_f64() * 100.0
            }
        };

        PortfolioSummary {
            base_currency: base,
            fx_per_usd: fx,
            total_usd,
            total_base,
            assets: stats,
            by_chain: by_chain
                .into_iter()
                .map(|(key, value)| NamedValue {
                    label: key,
                    value_usd: value,
                    value_base: value * fx,
                    pct: pct(value),
                })
                .collect(),
            by_store: by_store
                .into_iter()
                .map(|(key, value)| {
                    let name = self
                        .doc
                        .stores
                        .iter()
                        .find(|stat| stat.id == key)
                        .map(|stat| stat.name.clone())
                        .unwrap_or(key.clone());
                    StoreValue {
                        store_id: key,
                        name,
                        value_usd: value,
                        value_base: value * fx,
                        pct: pct(value),
                    }
                })
                .collect(),
        }
    }

    /// Portfolio value over time using transaction timestamps as event
    /// boundaries and the nearest cached price (≤ event ts) per asset.
    pub fn history_series(&self, base_currency: &str) -> Vec<HistoryPoint> {
        self.history_series_filtered(None, base_currency)
    }

    /// Shared history replay. `scope` restricts both the event axis and the
    /// valuation to legs booked on those stores (`None` covers the whole
    /// vault).
    fn history_series_filtered(
        &self,
        scope: Option<&HashSet<String>>,
        base_currency: &str,
    ) -> Vec<HistoryPoint> {
        let base = base_currency.to_lowercase();
        let fx = self
            .doc
            .fx_cache
            .iter()
            .find(|point| point.currency == base)
            .map(|point| point.per_usd)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        let in_scope = |store_id: &str| scope.is_none_or(|ids| ids.contains(store_id));

        // Flatten to legs first: a transfer contributes to one store's series
        // from its outgoing leg and to another's from its incoming one, and the
        // event axis is the set of leg timestamps.
        let mut movements: Vec<(String, String, Amount)> = Vec::new();
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                if !in_scope(&leg.store_id) {
                    continue;
                }
                movements.push((leg.asset.key(), tx.meta().timestamp.clone(), leg.amount));
            }
        }
        let mut stamps: Vec<String> = movements.iter().map(|(_, ts, _)| ts.clone()).collect();
        stamps.push(now_iso());
        stamps.sort();
        stamps.dedup();
        movements.sort_by(|left, right| left.1.cmp(&right.1));

        // Running quantity per asset across the whole scope. Valuing these
        // (not the current holdings) keeps an asset that was sold off later in
        // the series, and counts an asset held in two scoped stores once.
        let mut qty: HashMap<String, Amount> = HashMap::new();
        let mut out = Vec::with_capacity(stamps.len());
        let mut idx = 0usize;

        for ev in &stamps {
            while idx < movements.len() && movements[idx].1 <= *ev {
                let (asset, _, amount) = &movements[idx];
                *qty.entry(asset.clone()).or_default() += *amount;
                idx += 1;
            }
            let mut value_usd = Amount::ZERO;
            for (akey, quantity) in &qty {
                if quantity.is_zero() {
                    continue;
                }
                if let Some(price) = self.price_usd_at(akey, ev) {
                    value_usd += *quantity * price;
                }
            }
            out.push(HistoryPoint {
                ts: ev.clone(),
                ts_ms: iso_to_millis(ev),
                value_usd,
                value_base: value_usd * fx,
            });
        }
        out
    }

    /// Newest cached USD price of an asset at or before `ts`; without a quote
    /// yet, the latest price a transaction recorded for it (e.g. the last buy).
    fn price_usd_at(&self, asset_key: &str, ts: &str) -> Option<Amount> {
        self.doc
            .price_cache
            .iter()
            .filter(|point| point.asset_key == asset_key && point.ts.as_str() <= ts)
            .max_by(|left, right| left.ts.cmp(&right.ts))
            .map(|point| point.value_usd)
            .or_else(|| {
                self.doc
                    .transactions
                    .iter()
                    .filter(|tx| tx.meta().timestamp.as_str() <= ts)
                    .filter_map(|tx| {
                        tx.legs()
                            .into_iter()
                            .find(|leg| leg.asset.key() == asset_key)
                            .and_then(|leg| leg.price_usd)
                            .map(|price| (tx.meta().timestamp.clone(), price))
                    })
                    .max_by(|left, right| left.0.cmp(&right.0))
                    .map(|(_, price)| price)
            })
    }

    pub fn realized_pnl(&self) -> RealizedPnl {
        self.realized_pnl_filtered(None)
    }

    /// Shared PnL replay. `scope` restricts it to the legs booked on those
    /// stores (`None` replays the whole vault).
    fn realized_pnl_filtered(&self, scope: Option<&HashSet<String>>) -> RealizedPnl {
        let (base, _fx) = self.base_and_fx();

        // Flatten to (timestamp, leg) and replay. Only legs booked in scope
        // count, which is what makes a per-store PnL differ from the
        // vault-wide one even though they share this code path.
        let mut movements: Vec<(String, Leg)> = Vec::new();
        for tx in &self.doc.transactions {
            let ts = tx.meta().timestamp.clone();
            for leg in tx.legs() {
                if scope.is_some_and(|ids| !ids.contains(&leg.store_id)) {
                    continue;
                }
                movements.push((ts.clone(), leg));
            }
        }
        movements.sort_by(|left, right| left.0.cmp(&right.0));

        // avg cost tracking per (store, asset)
        let mut cost: HashMap<(String, String), Amount> = HashMap::new();
        let mut qty: HashMap<(String, String), Amount> = HashMap::new();
        let mut total = Amount::ZERO;
        let mut legs = Vec::new();

        for (ts, leg) in &movements {
            let delta = leg.amount;
            if delta.is_zero() {
                continue;
            }
            let key = (leg.store_id.clone(), leg.asset.key());
            let price = leg_price_in_base(leg, &base);
            let cur = qty.get(&key).copied().unwrap_or(Amount::ZERO);
            if delta.is_negative() {
                let avg = cost.get(&key).copied().unwrap_or(Amount::ZERO);
                let pnl = (price - avg) * delta.abs();
                total += pnl;
                legs.push(PnlLeg {
                    ts: ts.clone(),
                    store_id: leg.store_id.clone(),
                    asset_key: leg.asset.key(),
                    quantity: delta.abs(),
                    pnl_base: pnl,
                    side: "sell".into(),
                });
            } else {
                let new_qty = cur + delta;
                let new_cost = if new_qty == Amount::ZERO {
                    Amount::ZERO
                } else {
                    (cost.get(&key).copied().unwrap_or(Amount::ZERO) * cur + price * delta)
                        / new_qty
                };
                cost.insert(key.clone(), new_cost);
            }
            qty.insert(key, cur + delta);
        }
        RealizedPnl {
            total_base: total,
            legs,
        }
    }

    // -- bybit import staging (D2.4) ---------------------------------------

    /// Stage fills for user confirmation.
    ///
    /// Fills are keyed by Bybit's `execId`, so re-importing the same history is
    /// idempotent. Nothing here touches `transactions` — the user has to confirm
    /// first, which is what keeps a background import from rewriting history.
    /// Returns how many fills were newly staged.
    pub fn stage_imports(&mut self, imports: Vec<PendingImport>) -> Result<u32> {
        let before = self.doc.pending_imports.len();
        let mut known: HashSet<String> = self
            .doc
            .pending_imports
            .iter()
            .map(|pending| pending.exec_id.clone())
            .collect();

        for imp in imports {
            if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
                continue;
            }
            if self.ensure_store(&imp.store_id).is_err() {
                continue;
            }
            known.insert(imp.exec_id.clone());
            self.doc.pending_imports.push(imp);
            if self.doc.pending_imports.len() >= MAX_PENDING_IMPORTS {
                break;
            }
        }

        let staged = (self.doc.pending_imports.len() - before) as u32;
        if staged > 0 {
            self.persist()?;
        }
        Ok(staged)
    }

    /// Every staged fill, newest first.
    pub fn pending_imports(&self) -> Vec<PendingImport> {
        let mut out = self.doc.pending_imports.clone();
        out.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        out
    }

    /// Turn staged fills into real trades (two legs each, like `add_trade`).
    ///
    /// An empty `exec_ids` confirms everything staged. Confirmed entries leave
    /// the staging area so they cannot be imported twice.
    pub fn confirm_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32> {
        let all = exec_ids.is_empty();
        let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
        let mut confirmed = 0u32;
        let mut kept = Vec::new();

        for imp in std::mem::take(&mut self.doc.pending_imports) {
            if !(all || wanted.contains(imp.exec_id.as_str())) {
                kept.push(imp);
                continue;
            }
            if self.ensure_store(&imp.store_id).is_err() {
                continue; // store vanished: drop the staged fill, don't confirm it
            }
            if self.ensure_positive(&imp.qty_bought, "bought qty").is_err()
                || self.ensure_positive(&imp.qty_sold, "sold qty").is_err()
            {
                continue;
            }
            let mut meta = TxMeta::new(&imp.comment);
            meta.categories = vec!["trading".to_string()];
            if !imp.occurred_at.is_empty() {
                meta.timestamp = imp.occurred_at.clone();
            }

            // One record, two legs: a fill is a single event on the exchange,
            // and writing it as two transactions meant a delete could take only
            // one side with it.
            let trade = ExchangeTradeTx {
                meta,
                store_id: imp.store_id.clone(),
                sold: TradeSide {
                    asset: self.asset_for_import(&imp.asset_sold),
                    qty: imp.qty_sold,
                },
                bought: TradeSide {
                    asset: self.asset_for_import(&imp.asset_bought),
                    qty: imp.qty_bought,
                },
                fee: (!imp.fee.is_zero()).then(|| imp.fee.abs()),
                fee_asset: (!imp.fee_asset.is_empty()).then(|| imp.fee_asset.clone()),
                order_id: Some(imp.exec_id.clone()),
                market: Some(imp.category.clone()),
                liquidity: None,
                price: PriceStamp {
                    price_usd: Some(imp.price),
                    ..Default::default()
                },
            };

            let mut tx = Transaction::ExchangeTrade(trade);
            self.adopt_categories(&mut tx.meta_mut().categories);
            self.doc.transactions.push(tx);
            confirmed += 1;
        }

        self.doc.pending_imports = kept;
        if confirmed > 0 {
            self.recompute_holdings();
            self.persist()?;
        }
        Ok(confirmed)
    }

    /// Drop staged fills without creating transactions.
    /// An empty `exec_ids` discards everything.
    pub fn discard_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32> {
        let all = exec_ids.is_empty();
        let before = self.doc.pending_imports.len();
        let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
        self.doc
            .pending_imports
            .retain(|pending| !(all || wanted.contains(pending.exec_id.as_str())));
        let discarded = (before - self.doc.pending_imports.len()) as u32;
        if discarded > 0 {
            self.persist()?;
        }
        Ok(discarded)
    }

    /// Build an [`AssetRef`] for an imported symbol.
    ///
    /// The chain is inherited from an asset the user already tracks, so an
    /// imported fill lands in the same bucket as manual entries. Unknown
    /// symbols stay chain-less (`None`) rather than being guessed at — the
    /// user can bind a chain later.
    fn asset_for_import(&self, symbol: &str) -> AssetRef {
        let upper = symbol.trim().to_ascii_uppercase();
        match self
            .doc
            .holdings
            .iter()
            .find(|holding| holding.asset.symbol.eq_ignore_ascii_case(symbol))
            .map(|holding| holding.asset.clone())
        {
            // Reuse the identity the user already tracks (name, coingecko id,
            // chain) so the fill lands in the same bucket as manual entries.
            Some(mut asset) => {
                asset.symbol = upper;
                asset
            }
            // Unseen symbol: leave the chain unresolved so the user can bind
            // it later, rather than attributing the asset to asset wrong network.
            None => {
                let mut asset = AssetRef::crypto(&upper, &upper, None, Chain::Fiat);
                asset.chain = None;
                asset
            }
        }
    }

    // -- helpers ------------------------------------------------------------

    fn ensure_store(&self, id: &str) -> Result<()> {
        match self.doc.stores.iter().find(|store| store.id == id) {
            Some(store) if store.archived => Err(Error::Archived(id.into())),
            Some(_) => Ok(()),
            None => Err(Error::Missing(format!("store {id}"))),
        }
    }

    /// Shape-check every on-chain hash a transaction claims.
    ///
    /// A wrong network is as bad as a malformed hash: the explorer link would
    /// open a real page about a transaction that is not this one, so both are
    /// refused. A hash is matched against the chains of the assets it moved,
    /// which is why a trade with a BTC leg and a USDT leg cannot carry an EVM
    /// hash.
    fn check_on_chain_refs(&self, tx: &Transaction) -> Result<()> {
        let chains = tx.asset_chains();
        for (network, hash) in tx.on_chain_refs() {
            let Some(hash) = hash.filter(|raw| !raw.is_empty()) else {
                continue;
            };
            if hash_shape(hash) == HashShape::Malformed {
                return Err(Error::TxHashMalformed(hash.to_string()));
            }
            if let Some(network) = network
                && chains.iter().flatten().any(|chain| chain != &network)
            {
                return Err(Error::TxHashNetworkMismatch(format!(
                    "network {network:?} does not match the assets moved"
                )));
            }
        }
        Ok(())
    }

    fn ensure_positive(&self, amount: &Amount, what: &str) -> Result<()> {
        if amount.is_negative() || amount.is_zero() {
            return Err(Error::InvalidInput(format!(
                "{what} amount must be positive"
            )));
        }
        Ok(())
    }
}

/// Price of one unit of a leg's asset in `base`, as known at the time of the
/// movement.
///
/// Prefers an explicitly recorded base price, then an FX override paired with
/// a USD price, then the USD price itself when the base *is* USD. A USD price
/// with no known rate still falls through as the USD value rather than zero:
/// a wrong magnitude is easier to notice than a silent zero, and the rate
/// arrives on the next refresh.
fn leg_price_in_base(leg: &Leg, base: &str) -> Amount {
    if let Some(price_base) = leg.price_base {
        return price_base;
    }
    match (leg.price_usd, leg.fx_override) {
        (Some(usd), Some(fx)) => usd * fx,
        (Some(usd), None) if base == "usd" => usd,
        (Some(usd), None) => usd,
        _ => Amount::ZERO,
    }
}

// -- reporting DTOs ----------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetStat {
    pub asset: AssetRef,
    pub quantity: Amount,
    pub price_usd: Amount,
    pub value_usd: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct NamedValue {
    pub label: String,
    pub value_usd: Amount,
    pub value_base: Amount,
    pub pct: f64,
}

/// One store's slice of the portfolio. Unlike [`NamedValue`] it carries the
/// store id so the sidebar can link straight to the wallet page.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreValue {
    pub store_id: String,
    pub name: String,
    pub value_usd: Amount,
    pub value_base: Amount,
    pub pct: f64,
}

/// A single open position inside one store.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StorePosition {
    pub asset: AssetRef,
    pub quantity: Amount,
    /// Weighted average cost per unit in the base currency, if known.
    pub avg_cost: Option<Amount>,
    pub price_usd: Amount,
    pub value_usd: Amount,
    pub value_base: Amount,
    /// `(price_base − avg_cost) × |quantity|`; zero when `avg_cost` is unknown.
    pub unrealized_base: Amount,
}

/// A direct sub-account row on the wallet page.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreChildRow {
    pub store: Store,
    /// Own value (without the child's sub-accounts).
    pub value_usd: Amount,
    pub value_base: Amount,
    /// Value including the child's own sub-accounts.
    pub subtree_base: Amount,
    /// Share of the parent's subtree value, 0..1.
    pub share: f64,
}

/// One ancestor in the wallet page breadcrumbs.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreCrumb {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// Everything the wallet page needs about one store, in a single round trip.
/// Computed on the fly — nothing here is persisted, so the vault format
/// version is unaffected.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreOverview {
    pub store: Store,
    /// From the top-level wallet down to the direct parent.
    pub ancestors: Vec<StoreCrumb>,
    pub base_currency: String,
    pub fx_per_usd: Amount,
    /// Whether PnL and transaction stats below cover the subtree.
    pub include_children: bool,
    /// Value held on the store itself.
    pub own_usd: Amount,
    pub own_base: Amount,
    /// Own value plus every sub-account at any depth.
    pub subtree_usd: Amount,
    pub subtree_base: Amount,
    pub realized_pnl_base: Amount,
    /// Own positions, sorted by `value_base`, descending.
    pub positions: Vec<StorePosition>,
    /// Positions merged per asset across the whole subtree.
    pub subtree_positions: Vec<StorePosition>,
    /// Direct sub-accounts.
    pub children: Vec<StoreChildRow>,
    /// Whether a sub-account may be created here (depth and archive rules).
    pub can_add_child: bool,
    pub tx_count: u32,
    /// RFC3339; empty when there are no transactions.
    pub first_tx_at: String,
    pub last_tx_at: String,
}

/// One node of the wallet tree (sidebar, wallets page, pickers).
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreTreeNode {
    pub store: Store,
    /// 0 for a top-level wallet.
    pub depth: u32,
    /// Names from the top-level wallet down to this node, inclusive.
    pub path: Vec<String>,
    pub own_usd: Amount,
    pub own_base: Amount,
    pub subtree_usd: Amount,
    pub subtree_base: Amount,
    /// Transactions with a leg on this very store.
    pub own_tx_count: u32,
    /// `depth < MAX_STORE_DEPTH` and not archived.
    pub can_add_child: bool,
    pub children: Vec<StoreTreeNode>,
}

/// Allowed destinations for moving a sub-account.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreMoveTargets {
    /// Flat (no `children`), in tree order; the current parent is left out.
    pub parents: Vec<StoreTreeNode>,
    /// Whether it may become a top-level wallet (no root has its name).
    pub can_detach: bool,
}

/// Lookups shared by every node while building the tree.
struct TreeCtx<'doc> {
    fx: Amount,
    own_usd: HashMap<&'doc str, Amount>,
    tx_count: HashMap<String, u32>,
}

/// Live before archived, then by name.
fn sort_tree(nodes: &mut [StoreTreeNode]) {
    nodes.sort_by(|left, right| {
        left.store
            .archived
            .cmp(&right.store.archived)
            .then_with(|| {
                left.store
                    .name
                    .to_lowercase()
                    .cmp(&right.store.name.to_lowercase())
            })
    });
}

/// The node with `id`, children intact.
fn find_tree_node(nodes: Vec<StoreTreeNode>, id: &str) -> Option<StoreTreeNode> {
    for node in nodes {
        if node.store.id == id {
            return Some(node);
        }
        if let Some(found) = find_tree_node(node.children, id) {
            return Some(found);
        }
    }
    None
}

/// Depth-first flat order without `children`: parent, then its subtree.
fn flatten_tree(nodes: Vec<StoreTreeNode>) -> Vec<StoreTreeNode> {
    let mut out = Vec::new();
    for mut node in nodes {
        let children = std::mem::take(&mut node.children);
        out.push(node);
        out.extend(flatten_tree(children));
    }
    out
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PortfolioSummary {
    pub base_currency: String,
    pub fx_per_usd: Amount,
    pub total_usd: Amount,
    pub total_base: Amount,
    pub assets: Vec<AssetStat>,
    pub by_chain: Vec<NamedValue>,
    pub by_store: Vec<StoreValue>,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct HistoryPoint {
    pub ts: String,
    pub ts_ms: i64,
    pub value_usd: Amount,
    pub value_base: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PnlLeg {
    pub ts: String,
    pub store_id: String,
    pub asset_key: String,
    /// Absolute size of the closed-out quantity.
    pub quantity: Amount,
    pub pnl_base: Amount,
    pub side: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct RealizedPnl {
    pub total_base: Amount,
    pub legs: Vec<PnlLeg>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::transaction::{
        BankRail, BankTransferTx, Flow, IncomeSource, IncomeTx, PaymentTx, TransferTx,
        WalletTransferTx,
    };
    use crate::storage::SnapshotFormat;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = UnlockedVault::create(path, "pw").unwrap();
        vault
            .create_store("Binance", StoreKind::Exchange, "#f0b90b", "", None)
            .unwrap();
        vault
            .create_store("Cold", StoreKind::ColdWallet, "#555", "", None)
            .unwrap();
        (dir, vault)
    }

    fn btc() -> AssetRef {
        AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin)
    }

    fn usd() -> AssetRef {
        AssetRef::fiat("USD", "US Dollar")
    }

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    /// Builders for the variants the tests exercise. Each takes the minimum a
    /// user would supply, so a test reads as the transaction it describes.
    fn income(store: &str, raw: &str, comment: &str) -> Transaction {
        income_of(store, usd(), raw, "1", comment)
    }

    /// Income in a chosen asset and at a chosen USD price: the analytics
    /// replays value holdings, so a position with no price contributes
    /// nothing to a total.
    fn income_of(
        store: &str,
        asset: AssetRef,
        raw: &str,
        price: &str,
        comment: &str,
    ) -> Transaction {
        Transaction::Income(IncomeTx {
            meta: TxMeta::new(comment),
            store_id: store.into(),
            asset,
            amount: amount(raw),
            source: IncomeSource::Salary,
            price: PriceStamp {
                price_usd: Some(amount(price)),
                ..Default::default()
            },
        })
    }

    fn payment(store: &str, raw: &str, comment: &str) -> Transaction {
        Transaction::Payment(PaymentTx {
            meta: TxMeta::new(comment),
            store_id: store.into(),
            asset: usd(),
            amount: amount(raw),
            payee: "Shop".into(),
            receipt_ref: None,
            price: PriceStamp::default(),
        })
    }

    fn bank_transfer(store: &str, flow: Flow, raw: &str, comment: &str) -> Transaction {
        Transaction::BankTransfer(BankTransferTx {
            meta: TxMeta::new(comment),
            store_id: store.into(),
            asset: usd(),
            amount: amount(raw),
            flow,
            rail: BankRail::Sepa,
            counterparty_name: "Someone".into(),
            counterparty_account: Some("DE00".into()),
            reference: None,
            fee: None,
            fee_asset: None,
            price: PriceStamp::default(),
        })
    }

    fn transfer(from: &str, to: &str, raw: &str, comment: &str) -> Transaction {
        Transaction::Transfer(TransferTx {
            meta: TxMeta::new(comment),
            from_store_id: from.into(),
            to_store_id: to.into(),
            asset: btc(),
            amount: amount(raw),
            fee: None,
            fee_asset: None,
            network: None,
            tx_hash: None,
            price: PriceStamp::default(),
        })
    }

    fn trade(store: &str, sell: &str, buy: &str, comment: &str) -> Transaction {
        Transaction::ExchangeTrade(ExchangeTradeTx {
            meta: TxMeta::new(comment),
            store_id: store.into(),
            sold: TradeSide {
                asset: usd(),
                qty: amount(sell),
            },
            bought: TradeSide {
                asset: btc(),
                qty: amount(buy),
            },
            fee: None,
            fee_asset: None,
            order_id: None,
            market: Some("spot".into()),
            liquidity: Some("taker".into()),
            price: PriceStamp::default(),
        })
    }

    fn wallet_transfer(
        store: &str,
        flow: Flow,
        raw: &str,
        network: Option<Chain>,
        hash: Option<&str>,
    ) -> Transaction {
        // The asset has to live on the chain the hash claims, otherwise the
        // mismatch check fires for a reason that has nothing to do with the
        // hash under test.
        let asset = match network {
            Some(Chain::Ethereum) => {
                AssetRef::crypto("ETH", "Ethereum", Some("ethereum"), Chain::Ethereum)
            }
            _ => btc(),
        };
        Transaction::WalletTransfer(WalletTransferTx {
            meta: TxMeta::new("on-chain"),
            store_id: store.into(),
            asset,
            amount: amount(raw),
            flow,
            network,
            tx_hash: hash.map(str::to_string),
            from_address: None,
            to_address: None,
            memo: None,
            fee: None,
            fee_asset: None,
            price: PriceStamp::default(),
        })
    }

    fn at(tx: &mut Transaction, stamp: &str) {
        tx.meta_mut().timestamp = stamp.into();
    }

    fn holding_of(vault: &UnlockedVault, store: &str, asset: &AssetRef) -> Amount {
        vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == store && holding.asset.key() == asset.key())
            .map(|holding| holding.quantity)
            .unwrap_or(Amount::ZERO)
    }

    #[test]
    fn payment_reduces_the_balance() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();

        let mut top_up = income(&store, "100", "top up");
        at(&mut top_up, "2026-01-01T00:00:00Z");
        vault.add_transaction(top_up).unwrap();

        let mut groceries = payment(&store, "30", "groceries");
        at(&mut groceries, "2026-01-02T00:00:00Z");
        vault.add_transaction(groceries).unwrap();

        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "70");
    }

    /// The sign comes from the variant, not from a stored negative, so a
    /// negative magnitude on the wire is normalized rather than obeyed.
    #[test]
    fn a_negative_magnitude_is_normalized_not_trusted() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let tx = payment(&store, "-10", "groceries");
        let stored = vault.add_transaction(tx).unwrap();
        assert_eq!(stored.legs()[0].amount.to_string(), "-10");
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "-10");
    }

    #[test]
    fn a_zero_amount_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        assert!(matches!(
            vault.add_transaction(payment(&store, "0", "nothing")),
            Err(Error::InvalidInput(_))
        ));
    }

    /// The bank transfer is the reason for the enum: a payment and a bank
    /// transfer both move money out, and only one of them can carry an IBAN.
    #[test]
    fn a_bank_transfer_out_is_just_the_pilot_variant() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();

        let mut salary = income(&store, "500", "salary");
        at(&mut salary, "2026-01-01T00:00:00Z");
        vault.add_transaction(salary).unwrap();

        let mut rent = bank_transfer(&store, Flow::Out, "120", "rent");
        at(&mut rent, "2026-01-02T00:00:00Z");
        let stored = vault.add_transaction(rent).unwrap();

        let Transaction::BankTransfer(body) = &stored else {
            panic!("expected a bank transfer, got {}", stored.tag());
        };
        assert_eq!(body.rail, BankRail::Sepa);
        assert_eq!(body.counterparty_account.as_deref(), Some("DE00"));
        assert_eq!(body.counterparty_name, "Someone");
        // and it behaves like any other outgoing movement
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "380");
    }

    /// Direction is data, so an incoming bank transfer is a different record
    /// from an outgoing one even when everything else matches.
    #[test]
    fn bank_transfer_direction_is_not_guessed() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        vault
            .add_transaction(bank_transfer(&store, Flow::Out, "100", "pay"))
            .unwrap();
        vault
            .add_transaction(bank_transfer(&store, Flow::In, "30", "refund"))
            .unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "-70");
    }

    #[test]
    fn transaction_takes_any_number_of_categories() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let mut tx = payment(&store, "30", "groceries");
        tx.meta_mut().categories = vec![
            "  Food ".to_string(),
            "Weekend".to_string(),
            "food".to_string(),
            "   ".to_string(),
        ];
        let stored = vault.add_transaction(tx).unwrap();

        // Normalized, de-duplicated, empties dropped — order preserved.
        assert_eq!(stored.meta().categories, vec!["food", "weekend"]);
        // Every tag in use joins the vault-wide vocabulary, so the picker
        // offers it next time.
        for category in ["food", "weekend"] {
            assert!(vault.doc.categories.iter().any(|known| known == category));
        }
    }

    /// A transfer is one record, so its tags live in one place — there is no
    /// second leg that could be left un-normalized.
    #[test]
    fn a_transfer_carries_its_tags_once() {
        let (_dir, mut vault) = setup();
        let from = vault.doc.stores[0].id.clone();
        let to = vault.doc.stores[1].id.clone();
        let mut tx = transfer(&from, &to, "0.3", "to cold");
        tx.meta_mut().categories = vec!["Rebalance".to_string(), "rebalance".to_string()];
        let stored = vault.add_transaction(tx).unwrap();
        assert_eq!(stored.meta().categories, vec!["rebalance"]);
    }

    #[test]
    fn holdings_are_derived_from_transactions() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();

        let mut buy = income_of(&store, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();

        let mut spend = payment(&store, "0.25", "spend");
        at(&mut spend, "2026-01-02T00:00:00Z");
        vault.add_transaction(spend).unwrap();

        assert_eq!(holding_of(&vault, &store, &btc()).to_string(), "1");
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "-0.25");
    }

    /// A trade books both sides of one event on one store, and the resulting
    /// position is exactly what the two legs say.
    #[test]
    fn a_trade_moves_both_sides_of_one_record() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();

        let mut funding = income(&store, "1000", "top up");
        at(&mut funding, "2026-01-01T00:00:00Z");
        vault.add_transaction(funding).unwrap();

        let mut swap = trade(&store, "1000", "0.02", "btc dip");
        at(&mut swap, "2026-01-02T00:00:00Z");
        let stored = vault.add_transaction(swap).unwrap();

        assert_eq!(stored.tag(), "exchange_trade");
        assert_eq!(stored.legs().len(), 2);
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "0");
        assert_eq!(holding_of(&vault, &store, &btc()).to_string(), "0.02");
    }

    /// Two legs booking the same asset into the same store would net to
    /// nothing while the history still showed both — refuse it.
    #[test]
    fn a_trade_of_an_asset_for_itself_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let swap = Transaction::ExchangeTrade(ExchangeTradeTx {
            meta: TxMeta::new("silly"),
            store_id: store.clone(),
            sold: TradeSide {
                asset: btc(),
                qty: amount("1"),
            },
            bought: TradeSide {
                asset: btc(),
                qty: amount("1"),
            },
            fee: None,
            fee_asset: None,
            order_id: None,
            market: None,
            liquidity: None,
            price: PriceStamp::default(),
        });
        assert!(matches!(
            vault.add_transaction(swap),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn a_transfer_moves_quantities_between_stores() {
        let (_dir, mut vault) = setup();
        let from = vault.doc.stores[0].id.clone();
        let to = vault.doc.stores[1].id.clone();

        let mut deposit = income_of(&from, btc(), "1", "50000", "deposit");
        at(&mut deposit, "2026-01-01T00:00:00Z");
        vault.add_transaction(deposit).unwrap();

        let mut move_it = transfer(&from, &to, "0.3", "to cold");
        at(&mut move_it, "2026-01-02T00:00:00Z");
        vault.add_transaction(move_it).unwrap();

        assert_eq!(holding_of(&vault, &from, &btc()).to_string(), "0.7");
        assert_eq!(holding_of(&vault, &to, &btc()).to_string(), "0.3");
        // The vault total is unchanged: it is a relocation, not new money.
        let total: Amount = vault
            .doc
            .holdings
            .iter()
            .fold(Amount::ZERO, |acc, holding| acc + holding.quantity);
        assert_eq!(total, amount("1"));
    }

    #[test]
    fn a_transfer_to_the_same_store_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        assert!(matches!(
            vault.add_transaction(transfer(&store, &store, "1", "nowhere")),
            Err(Error::InvalidInput(_))
        ));
    }

    /// Deleting a transaction used to remove one leg of a pair and orphan the
    /// other. One record, one delete.
    #[test]
    fn deleting_a_trade_removes_both_sides() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let stored = vault
            .add_transaction(trade(&store, "100", "0.01", "swap"))
            .unwrap();
        assert_eq!(vault.doc.transactions.len(), 1);
        vault.delete_transaction(&stored.meta().id).unwrap();
        assert!(vault.doc.transactions.is_empty());
        assert!(vault.doc.holdings.is_empty());
    }

    #[test]
    fn a_transaction_for_an_unknown_store_is_rejected() {
        let (_dir, mut vault) = setup();
        assert!(matches!(
            vault.add_transaction(income("nope", "10", "ghost")),
            Err(Error::Missing(_))
        ));
    }

    #[test]
    fn a_transaction_for_an_archived_store_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        vault.archive_store(&store, true).unwrap();
        assert!(matches!(
            vault.add_transaction(income(&store, "10", "late")),
            Err(Error::Archived(_))
        ));
    }

    #[test]
    fn listing_filters_by_either_side_of_a_transfer() {
        let (_dir, mut vault) = setup();
        let from = vault.doc.stores[0].id.clone();
        let to = vault.doc.stores[1].id.clone();
        vault
            .add_transaction(transfer(&from, &to, "1", "move"))
            .unwrap();
        assert_eq!(
            vault.transactions(&StoreScope::store(&from, false)).len(),
            1
        );
        assert_eq!(vault.transactions(&StoreScope::store(&to, false)).len(), 1);
        assert_eq!(vault.transactions(&StoreScope::default()).len(), 1);
    }

    /// A manual price must fill in what is missing without clobbering a price
    /// the history already carries.
    #[test]
    fn set_prices_never_overwrites_an_existing_stamp() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let stored = vault
            .add_transaction(income_of(&store, btc(), "1", "50000", "priced"))
            .unwrap();

        vault
            .set_prices(&stored.meta().id, Some(amount("1")), None)
            .unwrap();
        let tx = &vault.doc.transactions[0];
        assert_eq!(tx.legs()[0].price_usd.unwrap().to_string(), "50000");
    }

    #[test]
    fn history_is_newest_first() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let mut older = income(&store, "1", "older");
        at(&mut older, "2026-01-01T00:00:00Z");
        vault.add_transaction(older).unwrap();
        let mut newer = income(&store, "1", "newer");
        at(&mut newer, "2026-06-01T00:00:00Z");
        vault.add_transaction(newer).unwrap();

        let listed = vault.transactions(&StoreScope::default());
        assert_eq!(listed[0].meta().comment, "newer");
        assert_eq!(listed[1].meta().comment, "older");
    }

    // -- on-chain validation -------------------------------------------------

    #[test]
    fn tx_hash_validation_rejects_malformed() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let tx = wallet_transfer(
            &store,
            Flow::Out,
            "1",
            Some(Chain::Bitcoin),
            Some("not-a-hash-at-all"),
        );
        assert!(matches!(
            vault.add_transaction(tx),
            Err(Error::TxHashMalformed(_))
        ));
    }

    #[test]
    fn tx_hash_validation_accepts_valid_bitcoin_hash() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let hash = "a".repeat(64);
        let tx = wallet_transfer(&store, Flow::Out, "1", Some(Chain::Bitcoin), Some(&hash));
        assert!(vault.add_transaction(tx).is_ok());
    }

    #[test]
    fn tx_hash_validation_accepts_valid_evm_hash() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let hash = format!("0x{}", "b".repeat(64));
        let tx = wallet_transfer(&store, Flow::In, "1", Some(Chain::Ethereum), Some(&hash));
        assert!(vault.add_transaction(tx).is_ok());
    }

    /// A hash on the wrong network would still open a real explorer page about
    /// a transaction that is not this one, so it is refused.
    #[test]
    fn tx_hash_network_mismatch_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let hash = format!("0x{}", "b".repeat(64));
        // A BTC position claiming an Ethereum hash: the explorer link would
        // open a real page about a transaction that is not this one.
        let tx = Transaction::WalletTransfer(WalletTransferTx {
            meta: TxMeta::new("mismatch"),
            store_id: store,
            asset: btc(),
            amount: amount("1"),
            flow: Flow::Out,
            network: Some(Chain::Ethereum),
            tx_hash: Some(hash),
            from_address: None,
            to_address: None,
            memo: None,
            fee: None,
            fee_asset: None,
            price: PriceStamp::default(),
        });
        assert!(matches!(
            vault.add_transaction(tx),
            Err(Error::TxHashNetworkMismatch(_))
        ));
    }

    #[test]
    fn tx_hash_network_match_accepted() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let hash = "a".repeat(64);
        let tx = wallet_transfer(&store, Flow::Out, "1", Some(Chain::Bitcoin), Some(&hash));
        assert!(vault.add_transaction(tx).is_ok());
    }

    #[test]
    fn an_empty_hash_is_accepted() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let tx = wallet_transfer(&store, Flow::Out, "1", Some(Chain::Bitcoin), Some(""));
        assert!(vault.add_transaction(tx).is_ok());
    }

    // -- wallet tree -----------------------------------------------------------

    fn node(vault: &mut UnlockedVault, name: &str, parent: Option<&str>) -> String {
        vault
            .create_store(
                name,
                StoreKind::Exchange,
                "#000",
                "",
                parent.map(str::to_string),
            )
            .unwrap()
            .id
    }

    fn violation(result: Result<impl std::fmt::Debug>) -> StoreTreeViolation {
        match result {
            Err(Error::StoreTree(violation)) => violation,
            other => panic!("expected a tree violation, got {other:?}"),
        }
    }

    /// Bybit › UTA › Spot, plus an empty top-level Ledger.
    fn three_levels() -> (tempfile::TempDir, UnlockedVault, [String; 4]) {
        let dir = tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("tree.wallet"), "pw").unwrap();
        let bybit = node(&mut vault, "Bybit", None);
        let uta = node(&mut vault, "UTA", Some(&bybit));
        let spot = node(&mut vault, "Spot", Some(&uta));
        let ledger = node(&mut vault, "Ledger", None);
        (dir, vault, [bybit, uta, spot, ledger])
    }

    #[test]
    fn a_store_cannot_become_its_own_parent() {
        let (_dir, mut vault, [_bybit, uta, ..]) = three_levels();
        assert_eq!(
            violation(vault.move_store(&uta, Some(&uta))),
            StoreTreeViolation::Cycle
        );
    }

    #[test]
    fn a_store_cannot_move_under_its_own_grandchild() {
        let dir = tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("tree.wallet"), "pw").unwrap();
        let root = node(&mut vault, "Root", None);
        let mid = node(&mut vault, "Mid", Some(&root));
        let leaf = node(&mut vault, "Leaf", Some(&mid));
        let other = node(&mut vault, "Other", Some(&root));
        assert_eq!(
            violation(vault.move_store(&mid, Some(&leaf))),
            StoreTreeViolation::Cycle
        );
        // a legal move still works next to it
        vault.move_store(&leaf, Some(&other)).unwrap();
    }

    #[test]
    fn a_fourth_level_is_rejected() {
        let (_dir, mut vault, [_bybit, _uta, spot, _ledger]) = three_levels();
        let created = vault.create_store("Deep", StoreKind::Exchange, "", "", Some(spot));
        assert_eq!(violation(created), StoreTreeViolation::TooDeep);
    }

    #[test]
    fn moving_a_subtree_counts_its_height() {
        let (_dir, mut vault, [bybit, uta, _spot, ledger]) = three_levels();
        let cold = node(&mut vault, "Cold", Some(&ledger));
        // UTA carries Spot below it: under Cold it would reach level four
        assert_eq!(
            violation(vault.move_store(&uta, Some(&cold))),
            StoreTreeViolation::TooDeep
        );
        // under a root it fits
        vault.move_store(&uta, Some(&ledger)).unwrap();
        assert_eq!(vault.depth(&uta), 1);
        assert_eq!(vault.root_of(&uta), ledger);
        assert_ne!(vault.root_of(&uta), bybit);
    }

    #[test]
    fn names_are_unique_among_siblings_only() {
        let (_dir, mut vault, [bybit, uta, _spot, ledger]) = three_levels();
        // a second "Spot" elsewhere is fine
        node(&mut vault, "Spot", Some(&ledger));
        // a case-different twin under the same parent is not, in any script
        let twin = vault.create_store(" spot ", StoreKind::Exchange, "", "", Some(uta.clone()));
        assert_eq!(violation(twin), StoreTreeViolation::NameTaken);
        node(&mut vault, "Наличные", Some(&bybit));
        let cyrillic = vault.create_store("НАЛИЧНЫЕ", StoreKind::Cash, "", "", Some(bybit));
        assert_eq!(violation(cyrillic), StoreTreeViolation::NameTaken);
        let root = vault.create_store("bybit", StoreKind::Exchange, "", "", None);
        assert_eq!(violation(root), StoreTreeViolation::NameTaken);
    }

    #[test]
    fn renaming_into_a_sibling_name_is_rejected() {
        let (_dir, mut vault, [_bybit, uta, spot, _ledger]) = three_levels();
        let futures = node(&mut vault, "Futures", Some(&uta));
        assert_eq!(
            violation(vault.update_store(&futures, "SPOT", "", "")),
            StoreTreeViolation::NameTaken
        );
        // keeping its own name (or changing only the case) is fine
        vault.update_store(&spot, "SPOT", "#fff", "note").unwrap();
    }

    #[test]
    fn a_top_level_wallet_never_becomes_a_sub_account() {
        let (_dir, mut vault, [bybit, _uta, _spot, ledger]) = three_levels();
        assert_eq!(
            violation(vault.move_store(&ledger, Some(&bybit))),
            StoreTreeViolation::RootMove
        );
        assert_eq!(
            vault.store_move_targets(&ledger).unwrap(),
            StoreMoveTargets::default()
        );
    }

    #[test]
    fn detaching_checks_top_level_names() {
        let (_dir, mut vault, [_bybit, uta, spot, ledger]) = three_levels();
        let clash = node(&mut vault, "Ledger", Some(&uta));
        assert!(!vault.store_move_targets(&clash).unwrap().can_detach);
        assert_eq!(
            violation(vault.move_store(&clash, None)),
            StoreTreeViolation::NameTaken
        );

        assert!(vault.store_move_targets(&spot).unwrap().can_detach);
        let moved = vault.move_store(&spot, None).unwrap();
        assert_eq!(moved.parent_id, None);
        assert_ne!(moved.id, ledger);
    }

    #[test]
    fn move_targets_skip_the_subtree_the_parent_and_deep_nodes() {
        let (_dir, vault, [bybit, uta, spot, ledger]) = three_levels();
        let targets = vault.store_move_targets(&uta).unwrap();
        let ids: Vec<&str> = targets
            .parents
            .iter()
            .map(|node| node.store.id.as_str())
            .collect();
        // UTA has a child, so it may only hang under a root; Bybit is its parent
        assert_eq!(ids, vec![ledger.as_str()]);
        assert!(targets.parents.iter().all(|node| node.children.is_empty()));

        let targets = vault.store_move_targets(&spot).unwrap();
        let ids: Vec<&str> = targets
            .parents
            .iter()
            .map(|node| node.store.id.as_str())
            .collect();
        assert!(ids.contains(&bybit.as_str()));
        assert!(ids.contains(&ledger.as_str()));
        assert!(!ids.contains(&uta.as_str()));
        assert!(!ids.contains(&spot.as_str()));
        assert_eq!(targets.parents[0].path, vec!["Bybit".to_string()]);
    }

    #[test]
    fn archiving_cascades_and_restoring_needs_the_parent() {
        let (_dir, mut vault, [bybit, uta, spot, _ledger]) = three_levels();
        vault.archive_store(&uta, true).unwrap();
        assert!(vault.find_store(&spot).unwrap().archived);
        assert!(!vault.find_store(&bybit).unwrap().archived);

        let created = vault.create_store("New", StoreKind::Exchange, "", "", Some(uta.clone()));
        assert_eq!(violation(created), StoreTreeViolation::ParentArchived);
        assert_eq!(
            violation(vault.archive_store(&spot, false).map(|()| "ok")),
            StoreTreeViolation::RestoreParentFirst
        );

        vault.archive_store(&uta, false).unwrap();
        assert!(!vault.find_store(&spot).unwrap().archived);
    }

    #[test]
    fn tree_totals_roll_up_through_every_level() {
        let (_dir, mut vault, [bybit, uta, spot, _ledger]) = three_levels();
        vault.add_transaction(income(&bybit, "1", "own")).unwrap();
        vault.add_transaction(income(&uta, "10", "mid")).unwrap();
        vault.add_transaction(income(&spot, "100", "leaf")).unwrap();
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: usd().key(),
            value_usd: amount("1"),
            ts: "2000-01-01T00:00:00Z".into(),
            source: "test".into(),
        });

        let tree = vault.store_tree();
        let root = tree.iter().find(|node| node.store.id == bybit).unwrap();
        assert_eq!(root.own_usd, amount("1"));
        assert_eq!(root.subtree_usd, amount("111"));
        assert_eq!(
            root.children[0].children[0].path,
            vec!["Bybit", "UTA", "Spot"]
        );
        assert!(root.children[0].can_add_child);
        assert!(!root.children[0].children[0].can_add_child);

        let overview = vault.store_overview(&bybit, true).unwrap();
        assert_eq!(overview.own_usd, amount("1"));
        assert_eq!(overview.subtree_usd, amount("111"));
        // the child row counts its own child (B6)
        assert_eq!(overview.children[0].value_usd, amount("10"));
        assert_eq!(overview.children[0].subtree_base, amount("110"));
        assert!((overview.children[0].share - 110.0 / 111.0).abs() < 1e-9);
        assert_eq!(overview.subtree_positions[0].quantity, amount("111"));
        assert_eq!(overview.tx_count, 3);
        assert_eq!(vault.store_overview(&bybit, false).unwrap().tx_count, 1);

        let leaf = vault.store_overview(&spot, true).unwrap();
        let crumbs: Vec<&str> = leaf
            .ancestors
            .iter()
            .map(|crumb| crumb.name.as_str())
            .collect();
        assert_eq!(crumbs, vec!["Bybit", "UTA"]);
        assert!(!leaf.can_add_child);
    }

    #[test]
    fn history_and_pnl_follow_the_scope() {
        let (_dir, mut vault, [bybit, uta, _spot, _ledger]) = three_levels();
        let mut buy = income_of(&uta, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();
        let mut sell = Transaction::ExchangeTrade(ExchangeTradeTx {
            meta: TxMeta::new("sell"),
            store_id: uta.clone(),
            sold: TradeSide {
                asset: btc(),
                qty: amount("0.5"),
            },
            bought: TradeSide {
                asset: usd(),
                qty: amount("30000"),
            },
            fee: None,
            fee_asset: None,
            order_id: None,
            market: None,
            liquidity: None,
            price: PriceStamp {
                price_usd: Some(amount("60000")),
                ..Default::default()
            },
        });
        at(&mut sell, "2026-01-02T00:00:00Z");
        vault.add_transaction(sell).unwrap();

        assert!(vault.store_pnl(&bybit, false).legs.is_empty());
        assert_eq!(vault.store_pnl(&bybit, true).legs.len(), 1);
        assert_eq!(
            vault.store_pnl(&bybit, true).total_base,
            vault.store_pnl(&uta, false).total_base
        );

        let own = vault.store_history(&bybit, false, "usd");
        assert!(own.iter().all(|point| point.value_usd.is_zero()));
        let rolled = vault.store_history(&bybit, true, "usd");
        assert!(rolled.iter().any(|point| !point.value_usd.is_zero()));
        assert_eq!(
            vault.transactions(&StoreScope::store(&bybit, true)).len(),
            2
        );
        assert!(
            vault
                .transactions(&StoreScope::store(&bybit, false))
                .is_empty()
        );
    }

    /// The same asset in two stores must be valued once per unit, not once
    /// per holding.
    #[test]
    fn history_counts_an_asset_held_twice_once() {
        let (_dir, vault) = two_stores_with_quotes();
        let last = vault.history_series("usd").pop().unwrap();
        assert_eq!(last.value_usd, amount("150"));
    }

    // -- analytics -----------------------------------------------------------

    /// Fixture: two bank stores, each holding a known USD balance.
    fn two_stores_with_quotes() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = UnlockedVault::create(path, "pw").unwrap();
        let bank_a = vault
            .create_store("A", StoreKind::Bank, "#111", "", None)
            .unwrap();
        let bank_b = vault
            .create_store("B", StoreKind::Bank, "#222", "", None)
            .unwrap();
        let mut left = income(&bank_a.id, "100", "in");
        at(&mut left, "2026-01-01T00:00:00Z");
        vault.add_transaction(left).unwrap();
        let mut right = income(&bank_b.id, "50", "in");
        at(&mut right, "2026-01-01T00:00:00Z");
        vault.add_transaction(right).unwrap();

        // Valuing a store means looking its assets up in the price cache, not
        // reading the price off the transaction, so seed it.
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: crate::model::store::AssetRef::fiat("USD", "US Dollar").key(),
            value_usd: amount("1"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
        });
        (dir, vault)
    }

    #[test]
    fn portfolio_summary_splits_by_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let summary = vault.portfolio_summary();
        assert_eq!(summary.total_usd.to_string(), "150");
        assert_eq!(summary.by_store.len(), 2);
    }

    #[test]
    fn store_overview_matches_global_slice() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[0].id.clone();
        let overview = vault.store_overview(&first, true).unwrap();
        assert_eq!(overview.own_usd.to_string(), "100");
        assert_eq!(overview.tx_count, 1);
    }

    /// A per-store series must not be valued against the other store's
    /// positions — the leg filter is what guarantees that.
    #[test]
    fn store_history_isolates_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[0].id.clone();
        let mine = vault.store_history(&first, true, "usd");
        let everyone = vault.history_series("usd");
        assert!(mine.len() <= everyone.len());
        for point in &mine {
            assert!(point.value_usd <= everyone.last().unwrap().value_usd);
        }
    }

    /// Realized PnL is measured against the weighted average cost that
    /// `recompute_holdings` also derives, so the two cannot disagree.
    #[test]
    fn store_pnl_filters_by_store() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();

        let mut buy = income_of(&store, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();

        let mut sell = Transaction::ExchangeTrade(ExchangeTradeTx {
            meta: TxMeta::new("sell"),
            store_id: store.clone(),
            sold: TradeSide {
                asset: btc(),
                qty: amount("0.5"),
            },
            bought: TradeSide {
                asset: usd(),
                qty: amount("30000"),
            },
            fee: None,
            fee_asset: None,
            order_id: None,
            market: None,
            liquidity: None,
            price: PriceStamp {
                price_usd: Some(amount("60000")),
                ..Default::default()
            },
        });
        at(&mut sell, "2026-01-02T00:00:00Z");
        vault.add_transaction(sell).unwrap();

        // bought at 50k, sold at 60k, half the position
        let pnl = vault.store_pnl(&store, true);
        assert_eq!(pnl.total_base, amount("5000"));
        assert_eq!(pnl.legs.len(), 1);
        assert_eq!(pnl.legs[0].side, "sell");
    }

    /// The average cost a holding carries must equal the one PnL measures
    /// against, since both come from the same leg replay.
    #[test]
    fn average_cost_is_consistent_with_pnl() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let mut buy = income_of(&store, btc(), "1", "40000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();

        let holding = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == store && holding.asset.key() == btc().key())
            .unwrap();
        assert_eq!(holding.avg_cost.unwrap().to_string(), "40000");
    }

    #[test]
    fn replace_doc_rederives_holdings_and_pins_identity() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        vault
            .add_transaction(income(&store, "100", "top up"))
            .unwrap();

        let mut edited = vault.doc.clone();
        // A hand-edited holdings array must not be able to disagree with the
        // transactions it summarizes.
        edited.holdings = vec![Holding {
            id: "forged".into(),
            store_id: store.clone(),
            asset: usd(),
            quantity: amount("999999"),
            avg_cost: None,
        }];
        edited.version = crate::VAULT_FORMAT_VERSION + 7;
        let id = vault.doc.id.clone();

        vault.replace_doc(edited).unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "100");
        assert_eq!(vault.doc.version, crate::VAULT_FORMAT_VERSION);
        assert_eq!(vault.doc.id, id);
    }

    #[test]
    fn dev_format_is_json() {
        let (dir, vault) = setup();
        let (_dir2, _vault) = (dir, vault);
        assert_eq!(SnapshotFormat::for_profile(), SnapshotFormat::Json);
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::exchange::Execution;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = UnlockedVault::create(path, "pw").unwrap();
        vault
            .create_store("Bybit", StoreKind::Exchange, "#f0b90b", "", None)
            .unwrap();
        (dir, vault)
    }

    fn fill(exec_id: &str, side: &str, qty: &str, price: &str) -> Execution {
        Execution {
            exec_id: exec_id.into(),
            symbol: "BTCUSDT".into(),
            side: side.into(),
            order_type: "Market".into(),
            exec_price: price.into(),
            exec_qty: qty.into(),
            // Quote leg; callers that care about leg symmetry pass sane values,
            // and the "garbled input" test deliberately passes junk.
            exec_value: "32500".into(),
            exec_fee: "0.5".into(),
            exec_time: "1700000000000".into(),
            is_maker: false,
            closed_size: None,
        }
    }

    fn staged(store: &str, exec_id: &str, side: &str) -> PendingImport {
        PendingImport::from_execution(store, "spot", &fill(exec_id, side, "0.5", "65000")).unwrap()
    }

    /// Staging must never create transactions on its own — that is the whole
    /// point of the confirmation step.
    #[test]
    fn staging_creates_no_transactions() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        let count = vault
            .stage_imports(vec![staged(&store, "e1", "buy")])
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(vault.doc.transactions.len(), 0);
        assert_eq!(vault.pending_imports().len(), 1);
    }

    /// Re-running an import must not duplicate fills: execId is the key.
    #[test]
    fn staging_is_idempotent_per_exec_id() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        assert_eq!(
            vault
                .stage_imports(vec![staged(&store, "e1", "buy")])
                .unwrap(),
            1
        );
        assert_eq!(
            vault
                .stage_imports(vec![staged(&store, "e1", "buy")])
                .unwrap(),
            0
        );
        // A batch that repeats asset known id alongside asset new one adds only the new one.
        assert_eq!(
            vault
                .stage_imports(vec![
                    staged(&store, "e1", "buy"),
                    staged(&store, "e2", "sell"),
                ])
                .unwrap(),
            1
        );
        assert_eq!(vault.pending_imports().len(), 2);
    }

    /// A fill that belongs to a store which no longer exists must be dropped,
    /// never staged against a dangling id.
    #[test]
    fn staging_skips_fills_for_unknown_stores() {
        let (_dir, mut vault) = setup();
        let count = vault
            .stage_imports(vec![staged("ghost-store", "e1", "buy")])
            .unwrap();
        assert_eq!(count, 0);
        assert!(vault.pending_imports().is_empty());
    }

    /// A buy becomes a two-leg trade: quote out (negative), base in (positive).
    #[test]
    fn confirming_a_buy_creates_a_balanced_trade() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        vault
            .stage_imports(vec![staged(&store, "e1", "buy")])
            .unwrap();

        assert_eq!(vault.confirm_pending_imports(&[]).unwrap(), 1);
        // One fill is one trade: a single record holding both sides, not two
        // transactions that had to be paired back up by a shared group id.
        assert_eq!(vault.doc.transactions.len(), 1);
        let trade = &vault.doc.transactions[0];
        assert_eq!(trade.tag(), "exchange_trade");
        assert_eq!(trade.legs().len(), 2);
        assert_eq!(trade.legs()[0].amount.to_string(), "-32500"); // USDT spent
        assert_eq!(trade.legs()[1].amount.to_string(), "0.5"); // BTC received
        // Staging area is drained so the same fill cannot be confirmed twice.
        assert!(vault.pending_imports().is_empty());
        // Holdings reflect the bought asset.
        assert!(
            vault
                .doc
                .holdings
                .iter()
                .any(|holding| holding.asset.symbol == "BTC")
        );
    }

    #[test]
    fn confirming_selectively_leaves_the_rest_staged() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        vault
            .stage_imports(vec![
                staged(&store, "e1", "buy"),
                staged(&store, "e2", "sell"),
            ])
            .unwrap();

        let confirmed = vault.confirm_pending_imports(&["e1".to_string()]).unwrap();
        assert_eq!(confirmed, 1);
        assert_eq!(vault.doc.transactions.len(), 1);
        let left: Vec<String> = vault
            .pending_imports()
            .into_iter()
            .map(|pending| pending.exec_id)
            .collect();
        assert_eq!(left, vec!["e2".to_string()]);
    }

    #[test]
    fn discarding_creates_nothing_and_drops_the_fill() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[0].id.clone();
        vault
            .stage_imports(vec![staged(&store, "e1", "buy")])
            .unwrap();
        assert_eq!(vault.discard_pending_imports(&[]).unwrap(), 1);
        assert!(vault.pending_imports().is_empty());
        assert_eq!(vault.doc.transactions.len(), 0);
    }

    #[test]
    fn confirmed_fills_reject_a_missing_exec_id() {
        let (_dir, mut vault) = setup();
        assert_eq!(
            vault
                .confirm_pending_imports(&["nope".to_string()])
                .unwrap(),
            0
        );
        assert_eq!(vault.doc.transactions.len(), 0);
    }

    /// Fills without an id, or with an unparsable quantity, must never reach
    /// staging — a garbled fill cannot become a transaction.
    #[test]
    fn unusable_fills_are_rejected_at_the_boundary() {
        assert!(PendingImport::from_execution("s", "spot", &fill("", "buy", "1", "100")).is_none());
        assert!(
            PendingImport::from_execution("s", "spot", &fill("e1", "buy", "not-a-number", "100"))
                .is_none()
        );
        assert!(
            PendingImport::from_execution("s", "spot", &fill("e1", "buy", "0", "100")).is_none()
        );
    }

    #[test]
    fn imported_fill_maps_legs_and_metadata() {
        let imp = PendingImport::from_execution(
            "store-1",
            "linear",
            &fill("exec-9", "Buy", "0.5", "65000"),
        )
        .unwrap();
        assert_eq!(imp.exec_id, "exec-9");
        assert_eq!(imp.store_id, "store-1");
        assert_eq!(imp.category, "linear");
        assert_eq!(imp.side, "buy");
        assert_eq!(imp.asset_bought, "BTC");
        assert_eq!(imp.qty_bought.to_string(), "0.5");
        assert_eq!(imp.asset_sold, "USDT");
        assert_eq!(imp.occurred_at, "2023-11-14T22:13:20Z");
        assert!(imp.comment.contains("bybit"));
    }

    /// Perpetual symbols are tagged as derivatives even on a swept import.
    #[test]
    fn category_hint_separates_perps_from_spot() {
        let mut execution = fill("e1", "buy", "1", "100");
        assert_eq!(execution.category_hint(), "spot");
        execution.symbol = "BTCUSDT_PERP".into();
        assert_eq!(execution.category_hint(), "linear");
    }
}

#[cfg(test)]
mod persist_tests {
    use super::*;
    use crate::model::vault::Settings;
    use tempfile::tempdir;

    /// Every settings field the UI can edit must survive a save and a reopen.
    ///
    /// This is the "does it come back after a restart" guarantee: a field that
    /// is not round-tripped here is a setting that silently reverts.
    #[test]
    fn every_settings_field_survives_a_restart() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "pw").unwrap();

        let edited = Settings {
            display_decimals: 6,
            dashboard_widgets: vec![crate::model::vault::DashboardWidgetCfg {
                id: "assets".into(),
                visible: false,
                order: 3,
            }],
            fx_refresh_interval_sec: 900,
            minimize_to_tray: false,
            bybit_import_interval_sec: 0,
        };

        // Applied the way `update_settings` does it: mutate, then persist.
        vault.doc.settings = edited.clone();
        vault.persist().unwrap();

        let (reopened, _) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(
            reopened.settings, edited,
            "settings reverted across a restart"
        );
    }

    /// The same, but through `replace_doc` — the path the raw JSON editor uses.
    /// Editing one field in the editor must not drop the others.
    #[test]
    fn settings_survive_a_json_editor_replace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("editor.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "pw").unwrap();
        vault.doc.settings.display_decimals = 4;
        vault.doc.settings.fx_refresh_interval_sec = 1800;
        vault.persist().unwrap();

        // What the editor sends back: the document it read, decimals changed.
        let (mut draft, _) = VaultFile::open(&path, "pw").unwrap();
        draft.settings.display_decimals = 2;
        vault.replace_doc(draft).unwrap();

        let (reopened, _) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(reopened.settings.display_decimals, 2);
        assert_eq!(
            reopened.settings.fx_refresh_interval_sec, 1800,
            "an unrelated setting was lost by the editor write"
        );
    }
}
