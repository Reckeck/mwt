//! Domain services: everything the app does on top of the [`VaultDoc`].
//! This module is pure Rust (no Tauri); the `app` crate only maps commands.

use std::collections::HashMap;
use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, Holding, Store, StoreKind};
use crate::model::transaction::{Transaction, TxKind};
use crate::model::vault::{VaultDoc, iso_to_millis, now_iso};
use crate::storage::{SnapshotFormat, VaultFile};

/// An unlocked vault held in memory for the duration of a session.
/// The master password lives only here (zeroized on drop) — it is never
/// written to disk and is required again on the next unlock.
pub struct UnlockedVault {
    pub path: PathBuf,
    pub fmt: SnapshotFormat,
    pub doc: VaultDoc,
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
            password: Zeroizing::new(password.to_string()),
        })
    }

    pub fn open(path: PathBuf, password: &str) -> Result<Self> {
        let (doc, fmt) = VaultFile::open(&path, password)?;
        Ok(Self {
            path,
            fmt,
            doc,
            password: Zeroizing::new(password.to_string()),
        })
    }

    /// Persist the current document (encrypt + atomic write).
    pub fn persist(&self) -> Result<()> {
        VaultFile::save(&self.path, &self.password, &self.doc, self.fmt)
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
    ) -> Result<Store> {
        if name.trim().is_empty() {
            return Err(Error::InvalidInput("store name must not be empty".into()));
        }
        if self
            .doc
            .stores
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err(Error::InvalidInput("store name already exists".into()));
        }
        let mut store = Store::new(name, kind, color);
        store.note = note.trim().to_string();
        self.doc.stores.push(store.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok(store)
    }

    pub fn update_store(&mut self, id: &str, name: &str, color: &str, note: &str) -> Result<Store> {
        let out = {
            let s = self
                .doc
                .stores
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            if !name.trim().is_empty() {
                s.name = name.trim().to_string();
            }
            s.color = color.trim().to_string();
            s.note = note.trim().to_string();
            s.clone()
        };
        self.persist()?;
        Ok(out)
    }

    pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> {
        let found = {
            let s = self
                .doc
                .stores
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            if archived && s.archived {
                return Err(Error::InvalidInput("already archived".into()));
            }
            s.archived = archived;
            true
        };
        if found {
            self.persist()?;
        }
        Ok(())
    }

    pub fn delete_store(&mut self, id: &str) -> Result<()> {
        if self
            .doc
            .transactions
            .iter()
            .any(|t| t.store_id == id || t.to_store_id.as_deref() == Some(id))
        {
            return Err(Error::InvalidInput(
                "store has transactions; archive instead".into(),
            ));
        }
        let before = self.doc.stores.len();
        self.doc.stores.retain(|s| s.id != id);
        self.doc.holdings.retain(|h| h.store_id != id);
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

    /// Add a single-leg transaction; updates holdings accordingly.
    /// Transaction history, newest first; optionally filtered by store.
    pub fn transactions(&self, store_id: Option<&str>) -> Vec<Transaction> {
        let mut out: Vec<Transaction> = self
            .doc
            .transactions
            .iter()
            .filter(|t| {
                store_id.is_none_or(|s| t.store_id == s || t.to_store_id.as_deref() == Some(s))
            })
            .cloned()
            .collect();
        out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        out
    }

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<Transaction> {
        self.ensure_store(&tx.store_id)?;
        match tx.kind {
            TxKind::Income => self.ensure_positive(&tx.amount, "income")?,
            TxKind::Expense => self.ensure_positive(&tx.amount, "expense")?,
            TxKind::Fee => self.ensure_positive(&tx.amount, "fee")?,
            TxKind::Transfer => {
                if tx.to_store_id.as_deref() == Some(tx.store_id.as_str()) {
                    return Err(Error::InvalidInput("transfer origin == destination".into()));
                }
                match &tx.to_store_id {
                    Some(to) => self.ensure_store(to)?,
                    None => return Err(Error::InvalidInput("transfer needs to_store_id".into())),
                }
            }
            TxKind::Trade => {
                return Err(Error::InvalidInput("use add_trade for trades".into()));
            }
            TxKind::PaperTrade => {}
        }
        self.doc.transactions.push(tx.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok(tx)
    }

    /// Move `amount` of `asset` from `from` to `to` (two legs, shared group).
    #[allow(clippy::too_many_arguments)]
    pub fn transfer(
        &mut self,
        from: &str,
        to: &str,
        asset: AssetRef,
        amount: Amount,
        fee: Option<Amount>,
        network: Option<Chain>,
        tx_hash: Option<String>,
        comment: &str,
        category: Option<String>,
    ) -> Result<(Transaction, Transaction)> {
        self.ensure_store(from)?;
        self.ensure_store(to)?;
        if from == to {
            return Err(Error::InvalidInput("transfer origin == destination".into()));
        }
        self.ensure_positive(&amount, "transfer")?;
        let group = uuid::Uuid::new_v4().to_string();
        let ts = now_iso();

        let mut out = Transaction::new(TxKind::Transfer, asset.clone(), -amount, from, comment);
        out.to_store_id = Some(to.to_string());
        out.network = network;
        out.tx_hash = tx_hash.clone();
        out.category = category.clone();
        out.fee = fee.map(|f| f.abs());
        out.group_id = Some(group.clone());
        out.timestamp = ts.clone();

        let mut inn = Transaction::new(TxKind::Transfer, asset.clone(), amount, to, comment);
        inn.to_store_id = Some(from.to_string());
        inn.network = network;
        inn.tx_hash = tx_hash;
        inn.category = category;
        inn.group_id = Some(group);
        inn.timestamp = ts;

        self.doc.transactions.push(out.clone());
        self.doc.transactions.push(inn.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok((out, inn))
    }

    /// Two-leg trade within stores: `sell_asset` (out, negative) → `buy_asset`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_trade(
        &mut self,
        store_id: &str,
        sell: AssetRef,
        sell_qty: Amount,
        buy: AssetRef,
        buy_qty: Amount,
        fee: Option<Amount>,
        comment: &str,
        category: Option<String>,
    ) -> Result<(Transaction, Transaction)> {
        self.ensure_store(store_id)?;
        self.ensure_positive(&sell_qty, "sell qty")?;
        self.ensure_positive(&buy_qty, "buy qty")?;
        let group = uuid::Uuid::new_v4().to_string();
        let ts = now_iso();

        let mut out = Transaction::new(TxKind::Trade, sell, -sell_qty, store_id, comment);
        out.category = category.clone();
        out.fee = fee.map(|f| f.abs());
        out.group_id = Some(group.clone());
        out.timestamp = ts.clone();

        let mut inn = Transaction::new(TxKind::Trade, buy, buy_qty, store_id, comment);
        inn.category = category;
        inn.group_id = Some(group);
        inn.timestamp = ts;

        self.doc.transactions.push(out.clone());
        self.doc.transactions.push(inn.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok((out, inn))
    }

    pub fn delete_transaction(&mut self, id: &str) -> Result<()> {
        let before = self.doc.transactions.len();
        self.doc
            .transactions
            .retain(|t| t.id != id && t.group_id.as_deref() != Some(id));
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
            .find(|t| t.id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        tx.comment = comment.trim().to_string();
        let out = tx.clone();
        self.persist()?;
        Ok(out)
    }

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
            .find(|t| t.id == tx_id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        tx.price_usd = price_usd;
        tx.fx_override = fx_override;
        self.persist()?;
        Ok(())
    }

    // -- categories ---------------------------------------------------------

    pub fn add_category(&mut self, category: &str) -> Result<String> {
        let c = category.trim().to_lowercase();
        if c.is_empty() {
            return Err(Error::InvalidInput("empty category".into()));
        }
        if !self.doc.categories.iter().any(|x| x == &c) {
            self.doc.categories.push(c.clone());
            self.persist()?;
        }
        Ok(c)
    }

    // -- holdings recomputation ---------------------------------------------

    fn recompute_holdings(&mut self) {
        // key: (store_id, asset_key) -> (quantity, avg_cost)
        let mut qty: HashMap<(String, String), Amount> = HashMap::new();
        let mut cost: HashMap<(String, String), Amount> = HashMap::new();
        let mut base = self.doc.settings.base_currency.to_ascii_lowercase();
        if base.is_empty() {
            base = "usd".into();
        }

        let mut txs: Vec<Transaction> = self.doc.transactions.clone();
        txs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        for tx in &txs {
            let key = (tx.store_id.clone(), tx.asset.key());
            let delta = signed_delta(tx);
            if delta.is_zero() {
                continue;
            }
            let price = tx_price_in_base(tx, &base);
            let cur = qty.get(&key).copied().unwrap_or(Amount::ZERO);
            let new_qty = cur + delta;

            if delta > Amount::ZERO && !price.is_zero() {
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
            // sells keep avg_cost unchanged (realized PnL measured against it
            // is computed on demand in analytics)
            qty.insert(key, new_qty);
        }

        self.doc.holdings = qty
            .into_iter()
            .filter(|(_, q)| !q.is_zero())
            .map(|((store_id, asset_key), quantity)| {
                let asset = self
                    .doc
                    .transactions
                    .iter()
                    .find(|t| t.store_id == store_id && t.asset.key() == asset_key)
                    .map(|t| t.asset.clone())
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
        let base = self.doc.settings.base_currency.to_ascii_lowercase();
        let base = if base.is_empty() {
            "usd".to_string()
        } else {
            base
        };
        let fx = self
            .doc
            .fx_cache
            .iter()
            .find(|p| p.currency == base)
            .map(|p| p.per_usd)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        (base, fx)
    }

    /// Newest cached USD price for an asset key, or zero.
    fn price_usd_for(&self, asset_key: &str) -> Amount {
        self.doc
            .price_cache
            .iter()
            .filter(|p| p.asset_key == asset_key)
            .max_by(|a, b| a.ts.cmp(&b.ts))
            .map(|p| p.value_usd)
            .unwrap_or(Amount::ZERO)
    }

    /// Transactions that touch `store_id` (either leg of a transfer/trade).
    fn store_txs(&self, store_id: &str) -> Vec<Transaction> {
        self.doc
            .transactions
            .iter()
            .filter(|t| t.store_id == store_id || t.to_store_id.as_deref() == Some(store_id))
            .cloned()
            .collect()
    }

    /// Aggregate value of a store's holdings, sorted by value descending.
    fn store_positions(&self, store_id: &str, fx: Amount) -> Vec<StorePosition> {
        let mut out: Vec<StorePosition> = self
            .doc
            .holdings
            .iter()
            .filter(|h| h.store_id == store_id)
            .map(|h| {
                let price_usd = self.price_usd_for(&h.asset.key());
                let value_usd = h.quantity * price_usd;
                let price_base = price_usd * fx;
                let unrealized_base = match h.avg_cost {
                    Some(cost) => (price_base - cost) * h.quantity.abs(),
                    None => Amount::ZERO,
                };
                StorePosition {
                    asset: h.asset.clone(),
                    quantity: h.quantity,
                    avg_cost: h.avg_cost,
                    price_usd,
                    value_usd,
                    value_base: value_usd * fx,
                    unrealized_base,
                }
            })
            .collect();
        out.sort_by_key(|t| std::cmp::Reverse(t.value_base));
        out
    }

    /// Single-round-trip view of one store for the wallet page.
    pub fn store_overview(&self, store_id: &str) -> Result<StoreOverview> {
        let store = self
            .doc
            .stores
            .iter()
            .find(|s| s.id == store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let (base, fx) = self.base_and_fx();
        let positions = self.store_positions(store_id, fx);
        let total_usd = positions
            .iter()
            .fold(Amount::ZERO, |acc, p| acc + p.value_usd);

        let txs = self.store_txs(store_id);
        let first_tx_at = txs
            .iter()
            .map(|t| t.timestamp.as_str())
            .min()
            .unwrap_or("")
            .to_string();
        let last_tx_at = txs
            .iter()
            .map(|t| t.timestamp.as_str())
            .max()
            .unwrap_or("")
            .to_string();

        Ok(StoreOverview {
            store,
            base_currency: base.clone(),
            fx_per_usd: fx,
            total_usd,
            total_base: total_usd * fx,
            realized_pnl_base: self.realized_pnl_filtered(Some(store_id)).total_base,
            positions,
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

        for h in &self.doc.holdings {
            let price = self.price_usd_for(&h.asset.key());
            let value_usd = h.quantity * price;
            let asset_key = h.asset.key();

            let st = by_asset
                .entry(asset_key.clone())
                .or_insert_with(|| AssetStat {
                    asset: h.asset.clone(),
                    quantity: Amount::ZERO,
                    price_usd: price,
                    value_usd: Amount::ZERO,
                });
            st.quantity += h.quantity;
            st.price_usd = price;
            st.value_usd += value_usd;

            let chain_key = h
                .asset
                .chain
                .map(|c| c.label().to_string())
                .unwrap_or_else(|| "Other".into());
            *by_chain.entry(chain_key).or_default() += value_usd;
            *by_store.entry(h.store_id.clone()).or_default() += value_usd;
            total_usd += value_usd;
        }

        let total_base = total_usd * fx;
        let mut stats: Vec<AssetStat> = by_asset.into_values().collect();
        stats.sort_by_key(|s| std::cmp::Reverse(s.value_usd));
        let pct = |v: Amount| {
            if total_usd == Amount::ZERO {
                0.0
            } else {
                (v / total_usd).to_f64() * 100.0
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
                .map(|(k, v)| NamedValue {
                    label: k,
                    value_usd: v,
                    value_base: v * fx,
                    pct: pct(v),
                })
                .collect(),
            by_store: by_store
                .into_iter()
                .map(|(k, v)| {
                    let name = self
                        .doc
                        .stores
                        .iter()
                        .find(|s| s.id == k)
                        .map(|s| s.name.clone())
                        .unwrap_or(k.clone());
                    StoreValue {
                        store_id: k,
                        name,
                        value_usd: v,
                        value_base: v * fx,
                        pct: pct(v),
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
    /// valuation set to that store (`None` covers the whole vault, as before).
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
            .find(|p| p.currency == base)
            .map(|p| p.per_usd)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));

        let source: Vec<Transaction> = match store_id {
            Some(id) => self.store_txs(id),
            None => self.doc.transactions.clone(),
        };
        let mut events: Vec<String> = source.iter().map(|t| t.timestamp.clone()).collect();
        events.push(now_iso());
        events.sort();
        events.dedup();

        let mut txs = source;
        txs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        let mut idx = 0usize;

        // (asset_key -> running qty)
        let mut qty: HashMap<String, Amount> = HashMap::new();
        let mut out = Vec::with_capacity(events.len());

        for ev in &events {
            while idx < txs.len() && txs[idx].timestamp <= *ev {
                let tx = &txs[idx];
                *qty.entry(tx.asset.key()).or_default() += signed_delta(tx);
                idx += 1;
            }
            let mut value_usd = Amount::ZERO;
            for h in &self.doc.holdings {
                // a per-store series must not be valued against the other
                // stores' positions
                if store_id.is_some_and(|id| h.store_id != id) {
                    continue;
                }
                let akey = h.asset.key();
                let q = qty.get(&akey).copied().unwrap_or(Amount::ZERO);
                let price = self
                    .doc
                    .price_cache
                    .iter()
                    .filter(|p| p.asset_key == akey && p.ts <= *ev)
                    .max_by(|a, b| a.ts.cmp(&b.ts))
                    .map(|p| p.value_usd)
                    .or_else(|| {
                        // no live quote yet: fall back to the latest recorded
                        // transaction price for this asset (e.g. the last buy)
                        self.doc
                            .transactions
                            .iter()
                            .filter(|t| {
                                t.asset.key() == akey && t.timestamp <= *ev && t.price_usd.is_some()
                            })
                            .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
                            .and_then(|t| t.price_usd)
                    });
                if let Some(p) = price {
                    value_usd += q * p;
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

    pub fn realized_pnl(&self) -> RealizedPnl {
        self.realized_pnl_filtered(None)
    }

    /// Shared PnL replay. `store_id` restricts it to the legs booked on that
    /// store (`None` replays the whole vault, as before).
    fn realized_pnl_filtered(&self, store_id: Option<&str>) -> RealizedPnl {
        let (base, _fx) = self.base_and_fx();
        let mut txs: Vec<Transaction> = match store_id {
            Some(id) => self.store_txs(id),
            None => self.doc.transactions.clone(),
        };
        txs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        // avg cost tracking per (store, asset)
        let mut cost: HashMap<(String, String), Amount> = HashMap::new();
        let mut qty: HashMap<(String, String), Amount> = HashMap::new();
        let mut total = Amount::ZERO;
        let mut legs = Vec::new();

        for tx in &txs {
            let delta = signed_delta(tx);
            if delta.is_zero() {
                continue;
            }
            let key = (tx.store_id.clone(), tx.asset.key());
            let price = tx_price_in_base(tx, &base);
            let cur = qty.get(&key).copied().unwrap_or(Amount::ZERO);
            if delta.is_negative() {
                let avg = cost.get(&key).copied().unwrap_or(Amount::ZERO);
                let pnl = (price - avg) * delta.abs();
                total += pnl;
                legs.push(PnlLeg {
                    ts: tx.timestamp.clone(),
                    store_id: tx.store_id.clone(),
                    asset_key: tx.asset.key(),
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

    // -- helpers ------------------------------------------------------------

    fn ensure_store(&self, id: &str) -> Result<()> {
        match self.doc.stores.iter().find(|s| s.id == id) {
            Some(s) if s.archived => Err(Error::Archived(id.into())),
            Some(_) => Ok(()),
            None => Err(Error::Missing(format!("store {id}"))),
        }
    }

    fn ensure_positive(&self, a: &Amount, what: &str) -> Result<()> {
        if a.is_negative() || a.is_zero() {
            return Err(Error::InvalidInput(format!(
                "{what} amount must be positive"
            )));
        }
        Ok(())
    }
}

/// Signed quantity effect of a transaction on its `store_id`.
fn signed_delta(tx: &Transaction) -> Amount {
    match tx.kind {
        TxKind::Income => tx.amount,
        TxKind::Expense | TxKind::Fee => -tx.amount.abs(),
        TxKind::Transfer | TxKind::Trade | TxKind::PaperTrade => tx.amount, // legs carry sign themselves
    }
}

/// Price of one unit of the tx asset in the given base currency at tx time.
fn tx_price_in_base(tx: &Transaction, base: &str) -> Amount {
    if let Some(p) = tx.price_base {
        return p;
    }
    match (tx.price_usd, tx.fx_override) {
        (Some(usd), Some(fx)) => usd * fx,
        (Some(usd), None) if base == "usd" => usd,
        (Some(_), None) => {
            // fall back to price_usd (fx unknown yet); acceptable for v0
            tx.price_usd.unwrap_or(Amount::ZERO)
        }
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
pub struct StoreOverview {
    pub store: Store,
    pub base_currency: String,
    pub fx_per_usd: Amount,
    pub total_usd: Amount,
    pub total_base: Amount,
    pub realized_pnl_base: Amount,
    /// Sorted by `value_base`, descending.
    pub positions: Vec<StorePosition>,
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
    use crate::storage::SnapshotFormat;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut v = UnlockedVault::create(path, "pw").unwrap();
        v.create_store("Binance", StoreKind::Exchange, "#f0b90b", "")
            .unwrap();
        v.create_store("Cold", StoreKind::ColdWallet, "#555", "")
            .unwrap();
        (dir, v)
    }

    fn btc() -> AssetRef {
        AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin)
    }

    fn usd() -> AssetRef {
        AssetRef::fiat("USD", "US Dollar")
    }

    #[test]
    fn holdings_are_derived_from_transactions() {
        let (_d, mut v) = setup();
        let store = v.doc.stores[0].id.clone();

        let mut t1 = Transaction::new(
            TxKind::Income,
            btc(),
            Amount::parse("1").unwrap(),
            &store,
            "buy",
        );
        t1.timestamp = "2026-01-01T00:00:00Z".into();
        t1.price_usd = Some(Amount::parse("50000").unwrap());
        v.add_transaction(t1.clone()).unwrap();

        let mut t2 = Transaction::new(
            TxKind::Expense,
            btc(),
            Amount::parse("0.4").unwrap(),
            &store,
            "sold some",
        );
        t2.timestamp = "2026-01-02T00:00:00Z".into();
        t2.price_usd = Some(Amount::parse("60000").unwrap());
        v.add_transaction(t2.clone()).unwrap();

        assert_eq!(v.doc.holdings.len(), 1);
        assert_eq!(v.doc.holdings[0].quantity.to_string(), "0.6");
        assert_eq!(v.doc.holdings[0].avg_cost.unwrap().to_string(), "50000");
    }

    #[test]
    fn transfer_moves_quantities_between_stores() {
        let (_d, mut v) = setup();
        let s1 = v.doc.stores[0].id.clone();
        let s2 = v.doc.stores[1].id.clone();
        let mut t = Transaction::new(
            TxKind::Income,
            btc(),
            Amount::parse("1").unwrap(),
            &s1,
            "deposit",
        );
        t.price_usd = Some(Amount::parse("50000").unwrap());
        v.add_transaction(t).unwrap();

        let (out, inn) = v
            .transfer(
                &s1,
                &s2,
                btc(),
                Amount::parse("0.3").unwrap(),
                None,
                Some(Chain::Bitcoin),
                Some("abc123".to_string()),
                "to cold",
                None,
            )
            .unwrap();
        assert!(out.explorer_url().unwrap().contains("abc123"));
        assert_eq!(inn.group_id, out.group_id);

        let h1 = v.doc.holdings.iter().find(|h| h.store_id == s1).unwrap();
        let h2 = v.doc.holdings.iter().find(|h| h.store_id == s2).unwrap();
        assert_eq!(h1.quantity.to_string(), "0.7");
        assert_eq!(h2.quantity.to_string(), "0.3");
    }

    #[test]
    fn trade_tracks_both_legs() {
        let (_d, mut v) = setup();
        let store = v.doc.stores[0].id.clone();
        v.add_trade(
            &store,
            btc(),
            Amount::parse("1").unwrap(),
            usd(),
            Amount::parse("50000").unwrap(),
            None,
            "sold btc",
            None,
        )
        .unwrap();

        assert_eq!(v.doc.transactions.len(), 2);
        let usd_hold = v
            .doc
            .holdings
            .iter()
            .find(|h| h.asset.symbol == "USD")
            .unwrap();
        assert_eq!(usd_hold.quantity.to_string(), "50000");
    }

    #[test]
    fn summary_and_history() {
        let (_d, mut v) = setup();
        let store = v.doc.stores[0].id.clone();
        let mut t = Transaction::new(
            TxKind::Income,
            btc(),
            Amount::parse("2").unwrap(),
            &store,
            "dep",
        );
        t.timestamp = "2026-01-01T00:00:00Z".into();
        t.price_usd = Some(Amount::parse("50000").unwrap());
        v.add_transaction(t).unwrap();
        v.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: btc().key(),
            value_usd: Amount::parse("70000").unwrap(),
            ts: "2026-01-02T00:00:00Z".into(),
            source: "test".into(),
        });
        v.doc.fx_cache.push(crate::model::vault::FxPoint {
            currency: "usd".into(),
            per_usd: Amount::parse("1").unwrap(),
            ts: "2026-01-02T00:00:00Z".into(),
            source: "test".into(),
        });

        let s = v.portfolio_summary();
        assert_eq!(s.assets.len(), 1);
        assert_eq!(s.total_usd.to_string(), "140000");

        let hist = v.history_series("usd");
        assert!(hist.len() >= 2);
        assert_eq!(hist[0].value_usd.to_string(), "100000");
        assert_eq!(hist.last().unwrap().value_usd.to_string(), "140000");
    }

    #[test]
    fn dev_format_is_json() {
        assert_eq!(SnapshotFormat::for_profile(), SnapshotFormat::Json);
    }

    /// Two stores, same asset in both, plus a cached price — the shape every
    /// per-store method has to slice correctly.
    fn two_stores_with_quotes() -> (tempfile::TempDir, UnlockedVault) {
        let (d, mut v) = setup();
        let s1 = v.doc.stores[0].id.clone();
        let s2 = v.doc.stores[1].id.clone();

        for (store, qty) in [(&s1, "1"), (&s2, "3")] {
            let mut t = Transaction::new(
                TxKind::Income,
                btc(),
                Amount::parse(qty).unwrap(),
                store,
                "dep",
            );
            t.timestamp = "2026-01-01T00:00:00Z".into();
            t.price_usd = Some(Amount::parse("50000").unwrap());
            v.add_transaction(t).unwrap();
        }
        v.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: btc().key(),
            value_usd: Amount::parse("70000").unwrap(),
            ts: "2026-01-02T00:00:00Z".into(),
            source: "test".into(),
        });
        v.doc.fx_cache.push(crate::model::vault::FxPoint {
            currency: "usd".into(),
            per_usd: Amount::parse("1").unwrap(),
            ts: "2026-01-02T00:00:00Z".into(),
            source: "test".into(),
        });
        (d, v)
    }

    #[test]
    fn store_overview_matches_global_slice() {
        let (_d, v) = two_stores_with_quotes();
        let s1 = v.doc.stores[0].id.clone();
        let s2 = v.doc.stores[1].id.clone();

        let o1 = v.store_overview(&s1).unwrap();
        assert_eq!(o1.store.name, "Binance");
        assert_eq!(o1.positions.len(), 1);
        assert_eq!(o1.positions[0].quantity.to_string(), "1");
        assert_eq!(o1.positions[0].value_usd.to_string(), "70000");
        assert_eq!(o1.positions[0].avg_cost.unwrap().to_string(), "50000");
        assert_eq!(o1.positions[0].unrealized_base.to_string(), "20000");
        assert_eq!(o1.total_usd.to_string(), "70000");
        assert_eq!(o1.tx_count, 1);
        assert_eq!(o1.first_tx_at, "2026-01-01T00:00:00Z");

        let o2 = v.store_overview(&s2).unwrap();
        assert_eq!(o2.total_usd.to_string(), "210000");

        // the two per-store totals add up to the global one
        assert_eq!((o1.total_usd + o2.total_usd).to_string(), "280000");
        assert_eq!(v.portfolio_summary().total_usd.to_string(), "280000");

        assert!(matches!(v.store_overview("nope"), Err(Error::Missing(_))));
    }

    #[test]
    fn store_history_isolates_store() {
        let (_d, v) = two_stores_with_quotes();
        let s2 = v.doc.stores[1].id.clone();

        let hist = v.store_history(&s2, "usd");
        assert!(hist.len() >= 2);
        assert_eq!(hist[0].value_usd.to_string(), "150000"); // 3 BTC at 50k
        assert_eq!(hist.last().unwrap().value_usd.to_string(), "210000"); // at 70k

        // both stores share the event timestamps, so the axis is identical
        assert_eq!(hist.len(), v.history_series("usd").len());
    }

    #[test]
    fn store_pnl_filters_by_store() {
        let (_d, mut v) = two_stores_with_quotes();
        let s1 = v.doc.stores[0].id.clone();
        let s2 = v.doc.stores[1].id.clone();

        // sell all of store1's BTC at 70k (bought at 50k) -> +20k
        let mut sell = Transaction::new(
            TxKind::Expense,
            btc(),
            Amount::parse("1").unwrap(),
            &s1,
            "out",
        );
        sell.timestamp = "2026-01-03T00:00:00Z".into();
        sell.price_usd = Some(Amount::parse("70000").unwrap());
        v.add_transaction(sell).unwrap();

        let p1 = v.store_pnl(&s1);
        assert_eq!(p1.total_base.to_string(), "20000");
        assert_eq!(p1.legs.len(), 1);
        assert_eq!(p1.legs[0].store_id, s1);
        assert_eq!(p1.legs[0].quantity.to_string(), "1");

        let p2 = v.store_pnl(&s2);
        assert_eq!(p2.total_base.to_string(), "0");
        assert!(p2.legs.is_empty());

        assert_eq!(v.realized_pnl().total_base.to_string(), "20000");
    }
}
