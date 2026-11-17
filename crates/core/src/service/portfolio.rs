use super::*;

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetStat {
    pub asset: AssetRef,
    pub quantity: Amount,
    pub price_usd: Amount,
    pub value_usd: Amount,
    /// One unit in the base currency at the latest rate.
    pub price_base: Amount,
    pub value_base: Amount,
    /// Change of the price over the last 24 hours, percent, when the provider said it.
    #[serde(default)]
    pub change_24h_pct: Option<Amount>,
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
    /// One unit in the base currency at the latest rate.
    pub price_base: Amount,
    pub value_usd: Amount,
    pub value_base: Amount,
    /// `(price_base − avg_cost) × |quantity|`; zero when `avg_cost` is unknown.
    pub unrealized_base: Amount,
    /// Percent over the last 24 hours, when known.
    #[serde(default)]
    pub change_24h_pct: Option<Amount>,
    /// Where the price came from and when.
    #[serde(default)]
    pub price_info: PriceInfo,
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
    /// Open futures positions (derivatives); never part of the totals.
    pub open_positions: Vec<StorePosition>,
    /// Direct sub-accounts.
    pub children: Vec<StoreChildRow>,
    /// Whether a sub-account may be created here (depth and archive rules).
    pub can_add_child: bool,
    pub tx_count: u32,
    /// RFC3339; empty when there are no transactions.
    pub first_tx_at: String,
    pub last_tx_at: String,
    /// Unrealized result of the own positions and of the whole subtree.
    #[serde(default)]
    pub unrealized_base: Amount,
    #[serde(default)]
    pub subtree_unrealized_base: Amount,
    /// The oldest provider quote the positions are valued at; `None` when no
    /// position has one.
    #[serde(default)]
    pub prices_as_of: Option<String>,
    /// A bonus programme whose points symbol another programme also uses.
    /// Computed, never stored.
    #[serde(default)]
    pub symbol_clash: bool,
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
    /// Up to three biggest assets, its own first, then its sub-accounts'.
    #[serde(default)]
    pub top_assets: Vec<AssetRef>,
    /// Every currency here and in the sub-accounts with its amount, biggest
    /// value first: the wallets table shows the balance in its own currency.
    #[serde(default)]
    pub balances: Vec<AssetBalance>,
    /// RFC3339 of the newest transaction here or in a sub-account.
    #[serde(default)]
    pub last_tx_at: Option<String>,
}

/// One currency a wallet holds: how much of it, and its worth in the base.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetBalance {
    pub asset: AssetRef,
    pub quantity: Amount,
    pub value_base: Amount,
}

/// Lookups shared by every node while building the tree.
pub(crate) struct TreeCtx<'doc> {
    fx: Amount,
    own_usd: HashMap<&'doc str, Amount>,
    tx_count: HashMap<String, u32>,
    last_tx: HashMap<String, String>,
    /// Positive holdings per store as (USD value, quantity, asset), biggest value first.
    held: HashMap<&'doc str, Vec<(Amount, Amount, AssetRef)>>,
}

/// Assets a wallet card names.
pub(crate) const TOP_ASSETS: usize = 3;

/// Live before archived, then by name.
pub(crate) fn position_key(leg: &Leg) -> PositionKey {
    PositionKey {
        store_id: leg.store_id.clone(),
        asset_key: leg.asset.key(),
    }
}

/// How a leg moves its position. A leg outside cost basis (fiat, derivative,
/// fee, settled PnL) still moves the quantity, but at an unknown price, so it
/// neither sets a cost nor realizes a profit.
pub(crate) fn movement_of(leg: &Leg, price_base: Amount) -> Movement {
    let price_base = if cost_basis::takes_part(leg) {
        price_base
    } else {
        Amount::ZERO
    };
    if leg.amount.is_positive() {
        Movement::Incoming { price_base }
    } else {
        Movement::Outgoing {
            price_base,
            leaves: leg.role == LegRole::Principal && leg.to_store_id.is_none(),
        }
    }
}

pub(crate) fn sort_tree(nodes: &mut [StoreTreeNode]) {
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
pub(crate) fn find_tree_node(nodes: Vec<StoreTreeNode>, id: &str) -> Option<StoreTreeNode> {
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

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PortfolioSummary {
    pub base_currency: String,
    pub fx_per_usd: Amount,
    pub total_usd: Amount,
    pub total_base: Amount,
    pub assets: Vec<AssetStat>,
    pub by_chain: Vec<NamedValue>,
    pub by_store: Vec<StoreValue>,
    /// Everything owned, incl. property and what people owe (base currency).
    pub assets_base: Amount,
    /// Loans and what I owe people, as a positive number.
    pub liabilities_base: Amount,
    pub net_worth_base: Amount,
    /// When the freshest fetched price or FX rate was stored, RFC3339; `None`
    /// before the first successful refresh.
    pub prices_as_of: Option<String>,
    /// When the base currency's rate was taken; `None` means no rate is known
    /// and every `*_base` figure is really USD. Empty for a USD base.
    pub base_fx_as_of: Option<String>,
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
    /// The transaction that closed the position.
    #[serde(default)]
    pub tx_id: String,
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
    /// Fees paid in scope, at their stamped price; not part of `total_base`.
    pub fees_base: Amount,
    pub legs: Vec<PnlLeg>,
}

impl UnlockedVault {
    pub(crate) fn recompute_holdings(&mut self) {
        // A correction keeps the real balance; its difference follows history.
        self.rebase_adjustments();
        let mut cost_book = CostBook::default();
        let mut seen: HashSet<PositionKey> = HashSet::new();
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
        let book = self.rate_book();

        for tx in &txs {
            let secs = secs_or_now(&tx.meta().timestamp);
            for leg in tx.legs() {
                let delta = leg.amount;
                if delta.is_zero() {
                    continue;
                }
                let key = position_key(&leg);
                let price = book.leg_price_in_base(&leg, &base, secs);
                cost_book.apply(&key, delta, movement_of(&leg, price));
                seen.insert(key);
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

        self.doc.holdings = seen
            .into_iter()
            .filter_map(|key| {
                let position = cost_book.position(&key);
                if position.quantity.is_zero() {
                    return None;
                }
                let asset = identity
                    .remove(&(key.store_id.clone(), key.asset_key.clone()))
                    .unwrap_or_else(|| AssetRef::fiat("UNKNOWN", "Unknown"));
                Some(Holding {
                    id: uuid::Uuid::new_v4().to_string(),
                    store_id: key.store_id,
                    asset,
                    quantity: position.quantity,
                    avg_cost: position.cost,
                })
            })
            .collect();

        // The crypto module's wallets hold no transactions at all (decision
        // P5): their balances come from the engine's fold, so the capital
        // reads one `doc.holdings` either way (spec E8,
        // `cryptotrack::totals`).
        self.apply_crypto_holdings();
    }

    /// Base currency code (never empty) and its **latest** cached FX rate.
    ///
    /// The cache keeps history sorted oldest first, so the first match used to
    /// be the oldest rate ever seen. With no rate at all the factor is 1; the
    /// summary reports that through `base_fx_as_of: None`.
    pub(crate) fn base_and_fx(&self) -> (String, Amount) {
        let base = self.base_currency.to_ascii_lowercase();
        let base = if base.is_empty() {
            crate::appconfig::DEFAULT_BASE_CURRENCY.to_string()
        } else {
            base
        };
        let fx = self
            .per_usd(&base)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        (base, fx)
    }

    /// Timestamp of the base currency's latest rate; `None` when it was never
    /// fetched (USD needs no rate and reports `Some("")`).
    pub(crate) fn base_fx_as_of(&self) -> Option<String> {
        let base = self.base_and_fx().0;
        if base == "usd" {
            return Some(String::new());
        }
        self.doc
            .fx_cache
            .iter()
            .filter(|point| point.currency == base && !point.per_usd.is_zero())
            .map(|point| point.ts.clone())
            .max()
    }

    /// Current USD price of one unit: a hand-set price (property), fiat
    /// through its latest FX rate, else the newest cached quote, else the last price a transaction recorded for
    /// the asset, else zero. Derivatives are never valued.
    /// A dollar stablecoin with no price yet is still worth a dollar, not
    /// nothing: exchange balances are mostly in them. See [`Self::price_quote_for`],
    /// which also says where the price came from.
    pub(crate) fn price_usd_for(&self, asset_key: &str) -> Amount {
        self.price_quote_for(asset_key).usd
    }

    /// Base-currency price of one unit. The base's own fiat asset is exactly 1
    /// (the `RateBook` identity, `L13`), rate or no rate; the rest goes through
    /// the USD price and the base rate `fx`.
    pub(crate) fn price_base_for(&self, asset_key: &str, fx: Amount) -> Amount {
        if rates::is_fiat_of(asset_key, &self.base_and_fx().0) {
            return Amount::new(rust_decimal::Decimal::ONE);
        }
        self.price_usd_for(asset_key) * fx
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
    pub(crate) fn scoped_txs(&self, ids: &HashSet<String>) -> Vec<Transaction> {
        self.doc
            .transactions
            .iter()
            .filter(|tx| ids.iter().any(|id| tx.touches(id)))
            .cloned()
            .collect()
    }

    /// A store's positions, sorted by value descending. `derivatives` picks
    /// the open futures positions instead of the holdings that have value.
    pub(crate) fn store_positions_of(
        &self,
        store_id: &str,
        fx: Amount,
        derivatives: bool,
    ) -> Vec<StorePosition> {
        let mut out: Vec<StorePosition> = self
            .doc
            .holdings
            .iter()
            .filter(|holding| {
                holding.store_id == store_id && holding.asset.is_derivative() == derivatives
            })
            .map(|holding| {
                let quote = if derivatives {
                    price_info::PriceQuote::default()
                } else {
                    self.price_quote_for(&holding.asset.key())
                };
                let price_usd = quote.usd;
                let value_usd = holding.quantity * price_usd;
                let price_base = self.price_base_for(&holding.asset.key(), fx);
                let value_base = holding.quantity * price_base;
                let unrealized_base =
                    cost_basis::unrealized(holding.quantity, holding.avg_cost, price_base);
                StorePosition {
                    asset: holding.asset.clone(),
                    quantity: holding.quantity,
                    avg_cost: holding.avg_cost,
                    price_usd,
                    price_base,
                    value_usd,
                    value_base,
                    unrealized_base,
                    change_24h_pct: quote.change_24h_pct,
                    price_info: quote.info,
                }
            })
            .collect();
        out.sort_by_key(|position| std::cmp::Reverse(position.value_base));
        out
    }

    pub(crate) fn store_positions(&self, store_id: &str, fx: Amount) -> Vec<StorePosition> {
        self.store_positions_of(store_id, fx, false)
    }

    /// One row per asset across several stores. The average cost is weighted
    /// by quantity and only kept when every store knows its own.
    pub(crate) fn merged_positions(&self, ids: &[String], fx: Amount) -> Vec<StorePosition> {
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
    pub(crate) fn children_of(&self, parent_id: &str) -> Vec<&Store> {
        self.doc
            .stores
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(parent_id))
            .collect()
    }

    /// Every descendant id of `root_id`, without `root_id` itself.
    pub(crate) fn subtree_ids(&self, root_id: &str) -> Vec<String> {
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
        let mut held: HashMap<&str, Vec<(Amount, Amount, AssetRef)>> = HashMap::new();
        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|holding| !holding.asset.is_derivative())
        {
            let value = holding.quantity * self.price_usd_for(&holding.asset.key());
            *own_usd.entry(holding.store_id.as_str()).or_default() += value;
            if holding.quantity.is_positive() {
                held.entry(holding.store_id.as_str()).or_default().push((
                    value,
                    holding.quantity,
                    holding.asset.clone(),
                ));
            }
        }
        let mut tx_count: HashMap<String, u32> = HashMap::new();
        let mut last_tx: HashMap<String, String> = HashMap::new();
        for tx in &self.doc.transactions {
            let mut touched: HashSet<String> = HashSet::new();
            touched.extend(tx.store_ids());
            let at = &tx.meta().timestamp;
            for id in touched {
                let latest = last_tx.entry(id.clone()).or_default();
                if at > latest {
                    latest.clone_from(at);
                }
                *tx_count.entry(id).or_default() += 1;
            }
        }
        for assets in held.values_mut() {
            assets.sort_by_key(|(value, _, _)| std::cmp::Reverse(*value));
        }
        let ctx = TreeCtx {
            fx,
            own_usd,
            tx_count,
            last_tx,
            held,
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

    pub(crate) fn tree_node(
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
        // The wallet's own biggest assets, topped up from its sub-accounts' by ticker.
        let mut top_assets: Vec<AssetRef> = Vec::new();
        let own_assets = ctx
            .held
            .get(store.id.as_str())
            .into_iter()
            .flatten()
            .map(|(_, _, asset)| asset);
        for asset in own_assets.chain(children.iter().flat_map(|child| child.top_assets.iter())) {
            if top_assets.len() < TOP_ASSETS
                && !top_assets
                    .iter()
                    .any(|seen| seen.symbol.eq_ignore_ascii_case(&asset.symbol))
            {
                top_assets.push(asset.clone());
            }
        }
        let balances = self.tree_balances(store, own_usd, &children, ctx);
        let last_tx_at = children
            .iter()
            .filter_map(|child| child.last_tx_at.clone())
            .chain(ctx.last_tx.get(&store.id).cloned())
            .max();
        StoreTreeNode {
            top_assets,
            balances,
            last_tx_at,
            store: store.clone(),
            depth: depth as u32,
            path,
            own_usd,
            own_base: own_usd * ctx.fx,
            subtree_usd,
            subtree_base: subtree_usd * ctx.fx,
            own_tx_count: ctx.tx_count.get(&store.id).copied().unwrap_or(0),
            // A kind without sub-account types (property, cash) cannot take one.
            can_add_child: depth < MAX_STORE_DEPTH
                && !store.archived
                && !store.kind.sub_types().is_empty(),
            children,
        }
    }

    /// What a wallet holds per currency, its sub-accounts merged in by ticker,
    /// biggest value first. Property counts in its own currency (a flat bought
    /// in roubles shows roubles), as long as that currency has a rate.
    pub(crate) fn tree_balances(
        &self,
        store: &Store,
        own_usd: Amount,
        children: &[StoreTreeNode],
        ctx: &TreeCtx,
    ) -> Vec<AssetBalance> {
        let property_currency = store
            .property
            .as_ref()
            .map(|info| info.currency.to_ascii_lowercase())
            .filter(|code| !code.is_empty());
        let mut own: Vec<AssetBalance> = match property_currency
            .as_deref()
            .and_then(|code| self.per_usd(code).map(|rate| (code, rate)))
        {
            Some((code, rate)) => vec![AssetBalance {
                asset: AssetRef::fiat(code, &code.to_uppercase()),
                quantity: own_usd * rate,
                value_base: own_usd * ctx.fx,
            }],
            None => ctx
                .held
                .get(store.id.as_str())
                .into_iter()
                .flatten()
                .map(|(value, quantity, asset)| AssetBalance {
                    asset: asset.clone(),
                    quantity: *quantity,
                    value_base: *value * ctx.fx,
                })
                .collect(),
        };
        for balance in children.iter().flat_map(|child| child.balances.iter()) {
            match own.iter_mut().find(|seen| {
                seen.asset
                    .symbol
                    .eq_ignore_ascii_case(&balance.asset.symbol)
            }) {
                Some(seen) => {
                    seen.quantity += balance.quantity;
                    seen.value_base += balance.value_base;
                }
                None => own.push(balance.clone()),
            }
        }
        own.sort_by_key(|balance| std::cmp::Reverse(balance.value_base));
        own
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

        let positions = self.store_positions(store_id, fx);
        let subtree_positions = self.merged_positions(&subtree, fx);
        let unrealized = |list: &[StorePosition]| {
            list.iter()
                .fold(Amount::ZERO, |sum, position| sum + position.unrealized_base)
        };
        // The oldest market quote shown: «prices as of» must not claim fresher.
        let prices_as_of = subtree_positions
            .iter()
            .filter(|position| position.price_info.origin == PriceOrigin::Market)
            .filter_map(|position| position.price_info.at.clone())
            .min();
        Ok(StoreOverview {
            unrealized_base: unrealized(&positions),
            subtree_unrealized_base: unrealized(&subtree_positions),
            prices_as_of,
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
            positions,
            subtree_positions,
            open_positions: self.store_positions_of(store_id, fx, true),
            children,
            can_add_child: node.can_add_child,
            tx_count: txs.len() as u32,
            first_tx_at,
            last_tx_at,
            symbol_clash: self.loyalty_symbol_clash(store_id),
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

    /// Timestamp of the freshest cached price or FX point.
    pub fn prices_as_of(&self) -> Option<String> {
        let prices = self.doc.price_cache.iter().map(|point| point.ts.as_str());
        let fx = self.doc.fx_cache.iter().map(|point| point.ts.as_str());
        prices
            .chain(fx)
            .filter_map(|ts| {
                chrono::DateTime::parse_from_rfc3339(ts)
                    .ok()
                    .map(|parsed| (parsed, ts))
            })
            .max_by_key(|(parsed, _)| *parsed)
            .map(|(_, ts)| ts.to_string())
    }

    /// The latest cached 24-hour change of an asset's price.
    pub(crate) fn change_24h_of(&self, asset_key: &str) -> Option<Amount> {
        self.doc
            .price_cache
            .iter()
            .filter(|point| point.asset_key == asset_key)
            .max_by(|left, right| left.ts.cmp(&right.ts))
            .and_then(|point| point.change_24h_pct)
    }

    pub fn portfolio_summary(&self) -> PortfolioSummary {
        let (base, fx) = self.base_and_fx();
        let mut total_usd = Amount::ZERO;
        let mut by_asset: HashMap<String, AssetStat> = HashMap::new();
        let mut by_chain: HashMap<String, Amount> = HashMap::new();
        let mut by_store: HashMap<String, Amount> = HashMap::new();

        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|holding| !holding.asset.is_derivative())
        {
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
                    price_base: price * fx,
                    value_base: Amount::ZERO,
                    change_24h_pct: self.change_24h_of(&asset_key),
                });
            st.quantity += holding.quantity;
            st.price_usd = price;
            st.value_usd += value_usd;
            st.price_base = price * fx;
            st.value_base += value_usd * fx;

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

        let worth = self.net_worth();
        PortfolioSummary {
            prices_as_of: self.prices_as_of(),
            base_fx_as_of: self.base_fx_as_of(),
            assets_base: worth.financial + worth.non_financial + worth.receivables,
            liabilities_base: worth.liabilities + worth.payables,
            net_worth_base: worth.net,
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
    pub(crate) fn history_series_filtered(
        &self,
        scope: Option<&HashSet<String>>,
        base_currency: &str,
    ) -> Vec<HistoryPoint> {
        self.history_series_filtered_in(&self.rate_book(), scope, base_currency)
    }

    /// [`Self::history_series_filtered`] over a book the caller already built.
    pub(crate) fn history_series_filtered_in(
        &self,
        book: &RateBook,
        scope: Option<&HashSet<String>>,
        base_currency: &str,
    ) -> Vec<HistoryPoint> {
        let base = base_currency.to_lowercase();
        let fx_now = self
            .per_usd(&base)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        let in_scope = |store_id: &str| scope.is_none_or(|ids| ids.contains(store_id));

        // Flatten to legs first: a transfer contributes to one store's series
        // from its outgoing leg and to another's from its incoming one, and the
        // event axis is the set of leg timestamps.
        let mut movements: Vec<(String, String, Amount)> = Vec::new();
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                if !in_scope(&leg.store_id) || leg.asset.is_derivative() {
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
            let secs = secs_or_now(ev);
            let mut value_usd = Amount::ZERO;
            for (akey, quantity) in &qty {
                if quantity.is_zero() {
                    continue;
                }
                if let Some(price) = book.history_price(akey, secs) {
                    value_usd += *quantity * price;
                }
            }
            // Each point in the base currency of its own day: a ruble
            // portfolio does not grow just because the ruble fell later.
            let fx = book.fx_at(&base, secs).unwrap_or(fx_now);
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

    /// Shared PnL replay. `scope` restricts it to the legs booked on those
    /// stores (`None` replays the whole vault).
    pub(crate) fn realized_pnl_filtered(&self, scope: Option<&HashSet<String>>) -> RealizedPnl {
        let (base, _fx) = self.base_and_fx();
        let book = self.rate_book();

        // Flatten to (timestamp, leg) and replay. Only legs booked in scope
        // count, which is what makes a per-store PnL differ from the
        // vault-wide one even though they share this code path.
        let mut movements: Vec<(String, &str, Leg)> = Vec::new();
        for tx in &self.doc.transactions {
            let ts = tx.meta().timestamp.clone();
            for leg in tx.legs() {
                if scope.is_some_and(|ids| !ids.contains(&leg.store_id)) {
                    continue;
                }
                movements.push((ts.clone(), tx.meta().id.as_str(), leg));
            }
        }
        movements.sort_by(|left, right| left.0.cmp(&right.0));

        let mut cost_book = CostBook::default();
        let mut total = Amount::ZERO;
        let mut fees = Amount::ZERO;
        let mut legs = Vec::new();

        for (ts, tx_id, leg) in &movements {
            let delta = leg.amount;
            if delta.is_zero() || leg.asset.is_derivative() {
                continue;
            }
            let key = position_key(leg);
            let price = book.leg_price_in_base(leg, &base, secs_or_now(ts));
            let realized = cost_book.apply(&key, delta, movement_of(leg, price));
            match leg.role {
                // A fee is a cost of its own, never a sale at a profit.
                LegRole::Fee => fees += delta.abs() * price,
                // Settled by the exchange: the PnL is the amount itself.
                LegRole::Pnl => {
                    let pnl = delta * price;
                    total += pnl;
                    legs.push(PnlLeg {
                        ts: ts.clone(),
                        tx_id: tx_id.to_string(),
                        store_id: leg.store_id.clone(),
                        asset_key: leg.asset.key(),
                        quantity: delta.abs(),
                        pnl_base: pnl,
                        side: "settled".into(),
                    });
                }
                // Only the covered part of a sale that leaves the user's
                // wallets is a profit; the rule lives in `cost_basis`.
                LegRole::Principal | LegRole::Adjustment => {
                    if let Some(sale) = realized {
                        total += sale.pnl_base;
                        legs.push(PnlLeg {
                            ts: ts.clone(),
                            tx_id: tx_id.to_string(),
                            store_id: leg.store_id.clone(),
                            asset_key: leg.asset.key(),
                            quantity: sale.quantity,
                            pnl_base: sale.pnl_base,
                            side: "sell".into(),
                        });
                    }
                }
            }
        }
        RealizedPnl {
            total_base: total,
            fees_base: fees,
            legs,
        }
    }
}
