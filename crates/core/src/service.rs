//! Domain services: everything the app does on top of the [`VaultDoc`].
//! This module is pure Rust (no Tauri); the `app` crate only maps commands.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, Holding, Store, StoreKind};
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

    pub fn create_store(
        &mut self,
        name: &str,
        kind: StoreKind,
        color: &str,
        note: &str,
        parent_id: Option<String>,
    ) -> Result<Store> {
        if name.trim().is_empty() {
            return Err(Error::InvalidInput("store name must not be empty".into()));
        }
        if self
            .doc
            .stores
            .iter()
            .any(|store| store.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err(Error::InvalidInput("store name already exists".into()));
        }
        // Validate parent exists
        if let Some(ref pid) = parent_id
            && !self.doc.stores.iter().any(|store| store.id == *pid)
        {
            return Err(Error::Missing("parent store not found".into()));
        }
        let mut store = Store::new(name, kind, color);
        store.note = note.trim().to_string();
        store.parent_id = parent_id;
        self.doc.stores.push(store.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok(store)
    }

    pub fn update_store(
        &mut self,
        id: &str,
        name: &str,
        color: &str,
        note: &str,
        parent_id: Option<String>,
    ) -> Result<Store> {
        // Validate new parent if changed
        if let Some(ref pid) = parent_id
            && *pid != id
        {
            // can't be parent of itself
            if !self.doc.stores.iter().any(|store| store.id == *pid) {
                return Err(Error::Missing("parent store not found".into()));
            }
            // Check for cycles: walk up from proposed parent
            let mut ancestor = Some(pid.as_str());
            while let Some(current) = ancestor {
                if current == id {
                    return Err(Error::InvalidInput(
                        "cycle detected: cannot make store a descendant of itself".into(),
                    ));
                }
                ancestor = self
                    .doc
                    .stores
                    .iter()
                    .find(|store| store.id == current)
                    .and_then(|store| store.parent_id.as_deref());
            }
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
            store.parent_id = parent_id;
            store.clone()
        };
        self.persist()?;
        Ok(out)
    }

    pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> {
        let found = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            if archived && store.archived {
                return Err(Error::InvalidInput("already archived".into()));
            }
            store.archived = archived;
            true
        };
        if found {
            self.persist()?;
        }
        Ok(())
    }

    pub fn delete_store(&mut self, id: &str) -> Result<()> {
        if self.doc.transactions.iter().any(|tx| tx.touches(id)) {
            return Err(Error::InvalidInput(
                "store has transactions; archive instead".into(),
            ));
        }
        // Check for children
        if self
            .doc
            .stores
            .iter()
            .any(|store| store.parent_id.as_deref() == Some(id))
        {
            return Err(Error::InvalidInput(
                "store has children; archive them first or move them to another parent".into(),
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

    /// Transaction history, newest first; optionally filtered by store.
    pub fn transactions(&self, store_id: Option<&str>) -> Vec<Transaction> {
        let mut out: Vec<Transaction> = self
            .doc
            .transactions
            .iter()
            .filter(|tx| store_id.is_none_or(|filter| tx.touches(filter)))
            .cloned()
            .collect();
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

    /// Transactions that touch `store_id` (either leg of a transfer/trade).
    fn store_txs(&self, store_id: &str) -> Vec<Transaction> {
        self.doc
            .transactions
            .iter()
            .filter(|tx| tx.touches(store_id))
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

    /// Return direct children of a store.
    fn children_of(&self, parent_id: &str) -> Vec<&Store> {
        self.doc
            .stores
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(parent_id))
            .collect()
    }

    /// Return all descendant store IDs (BFS).
    fn subtree_ids(&self, root_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut queue = vec![root_id.to_string()];
        while let Some(id) = queue.pop() {
            for child in self.children_of(&id) {
                result.push(child.id.clone());
                queue.push(child.id.clone());
            }
        }
        result
    }

    /// Single-round-trip view of one store for the wallet page.
    pub fn store_overview(&self, store_id: &str) -> Result<StoreOverview> {
        let store = self
            .doc
            .stores
            .iter()
            .find(|store| store.id == store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let (base, fx) = self.base_and_fx();
        let positions = self.store_positions(store_id, fx);
        let total_usd = positions
            .iter()
            .fold(Amount::ZERO, |acc, point| acc + point.value_usd);

        // Children with their direct values
        let children = self
            .children_of(store_id)
            .into_iter()
            .map(|child| {
                let child_positions = self.store_positions(&child.id, fx);
                let child_value_usd = child_positions
                    .iter()
                    .fold(Amount::ZERO, |acc, point| acc + point.value_usd);
                StoreChildRow {
                    store: child.clone(),
                    value_usd: child_value_usd,
                    value_base: child_value_usd * fx,
                }
            })
            .collect::<Vec<_>>();

        // Subtree total = own value + sum of all descendants' values
        let subtree_ids = self.subtree_ids(store_id);
        let subtree_total_usd = self
            .doc
            .holdings
            .iter()
            .filter(|holding| {
                subtree_ids.contains(&holding.store_id) || holding.store_id == store_id
            })
            .fold(Amount::ZERO, |acc, holding| {
                let price = self.price_usd_for(&holding.asset.key());
                acc + holding.quantity * price
            });

        let txs = self.store_txs(store_id);
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

        Ok(StoreOverview {
            store,
            base_currency: base.clone(),
            fx_per_usd: fx,
            total_usd,
            total_base: total_usd * fx,
            subtree_total_base: subtree_total_usd * fx,
            realized_pnl_base: self.realized_pnl_filtered(Some(store_id)).total_base,
            positions,
            children,
            tx_count: txs.len() as u32,
            first_tx_at,
            last_tx_at,
        })
    }

    /// Portfolio value of one store over time.
    pub fn store_history(&self, store_id: &str, base_currency: &str) -> Vec<HistoryPoint> {
        self.history_series_filtered(Some(store_id), base_currency)
    }

    /// Realized PnL of one store.
    pub fn store_pnl(&self, store_id: &str) -> RealizedPnl {
        self.realized_pnl_filtered(Some(store_id))
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

    /// Shared history replay. `store_id` restricts both the event axis and the
    /// valuation to legs booked on that store (`None` covers the whole vault).
    fn history_series_filtered(
        &self,
        store_id: Option<&str>,
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

        // Flatten to legs first: a transfer contributes to one store's series
        // from its outgoing leg and to another's from its incoming one, and the
        // event axis is the set of leg timestamps.
        let mut movements: Vec<(String, String, Amount)> = Vec::new();
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                if store_id.is_some_and(|id| leg.store_id != id) {
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

        // Running quantity per asset. Valuing these (not the current
        // holdings) keeps an asset that was sold off later in the series, and
        // counts an asset held in two stores once.
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

    /// Shared PnL replay. `store_id` restricts it to the legs booked on that
    /// store (`None` replays the whole vault, as before).
    fn realized_pnl_filtered(&self, store_id: Option<&str>) -> RealizedPnl {
        let (base, _fx) = self.base_and_fx();
        let source: Vec<Transaction> = match store_id {
            Some(id) => self.store_txs(id),
            None => self.doc.transactions.clone(),
        };

        // Flatten to (timestamp, leg) and replay. Only legs booked on the
        // requested store count, which is what makes a per-store PnL differ
        // from the vault-wide one even though they share this code path.
        let mut movements: Vec<(String, Leg)> = Vec::new();
        for tx in &source {
            let ts = tx.meta().timestamp.clone();
            for leg in tx.legs() {
                if store_id.is_some_and(|id| leg.store_id != id) {
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

/// Everything the wallet page needs about one store, in a single round trip.
/// Computed on the fly — nothing here is persisted, so the vault format
/// version is unaffected.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreChildRow {
    pub store: Store,
    pub value_usd: Amount,
    pub value_base: Amount,
}

/// Everything the wallet page needs about one store, in a single round trip.
/// Computed on the fly — nothing here is persisted, so the vault format
/// version is unaffected.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreOverview {
    pub store: Store,
    pub base_currency: String,
    pub fx_per_usd: Amount,
    pub total_usd: Amount,
    pub total_base: Amount,
    /// Total value including all descendants (subtree).
    pub subtree_total_base: Amount,
    pub realized_pnl_base: Amount,
    /// Sorted by `value_base`, descending.
    pub positions: Vec<StorePosition>,
    /// Direct children with their own values (for tree rendering).
    pub children: Vec<StoreChildRow>,
    pub tx_count: u32,
    /// RFC3339; empty when the store has no transactions.
    pub first_tx_at: String,
    pub last_tx_at: String,
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
        assert_eq!(vault.transactions(Some(&from)).len(), 1);
        assert_eq!(vault.transactions(Some(&to)).len(), 1);
        assert_eq!(vault.transactions(None).len(), 1);
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

        let listed = vault.transactions(None);
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

    /// The same asset in two stores must be valued once per unit, not once
    /// per holding.
    #[test]
    fn history_counts_an_asset_held_twice_once() {
        let (_dir, vault) = two_stores_with_quotes();
        let last = vault.history_series("usd").pop().unwrap();
        assert_eq!(last.value_usd, amount("150"));
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
        let overview = vault.store_overview(&first).unwrap();
        assert_eq!(overview.total_usd.to_string(), "100");
        assert_eq!(overview.tx_count, 1);
    }

    /// A per-store series must not be valued against the other store's
    /// positions — the leg filter is what guarantees that.
    #[test]
    fn store_history_isolates_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[0].id.clone();
        let mine = vault.store_history(&first, "usd");
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
        let pnl = vault.store_pnl(&store);
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
