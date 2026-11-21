//! The operations list and the operations graph (plan F4).
//!
//! One filter drives both: which wallets, which kinds, a contact, a chain, a
//! date range in the user's zone, whether moves inside one wallet show, and
//! a free-text search. The graph lays the filtered operations out as lanes —
//! one per store, in the order of the wallet tree — with the coordinates
//! computed here, so the UI only draws.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::{Error, Result};
use crate::model::store::StoreScope;
use crate::model::transaction::{LegRole, Transaction, TxKind, TxOrigin};
use crate::time::secs_or_now;

use crate::model::vault::VaultDoc;
use crate::money::books::Books;
use crate::money::tree::{ancestors_of, find_store, resolve_scope, root_of};

/// More nodes than this and the graph is unreadable (and slow): narrow the filter.
pub const GRAPH_NODE_CAP: usize = 2000;

/// What the filter reads from areas that have not moved yet (OD6). Each answer
/// is asked for only when the filter needs it; the vault implements it in the
/// scenario.
pub trait FilterSources {
    /// Ids of the operations that closed a position, replayed over `scope`.
    fn realized_tx_ids(&self, scope: Option<&HashSet<String>>) -> HashSet<String>;
    /// Ids of the operations with a file or a link.
    fn tx_ids_with_attachments(&self) -> Vec<String>;
    /// Ids of the operations the budget could not value.
    fn unpriced_tx_ids(&self) -> HashSet<String>;
}

/// One wallet of the tree in display order, parents before their sub-accounts:
/// what a lane needs of the portfolio area's tree (OD6).
#[derive(Debug, Clone, PartialEq)]
pub struct WalletRow {
    pub store_id: String,
    /// Names from the top-level wallet down to this one, inclusive.
    pub path: Vec<String>,
    /// 0 for a top-level wallet.
    pub depth: u32,
    /// The store has sub-accounts.
    pub has_children: bool,
}

/// One page of the operations list, newest first, and how many match in all.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TxPage {
    pub items: Vec<Transaction>,
    pub total: u32,
}

/// What the operations list and graph show.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TxFilter {
    /// Wallets; empty = every one.
    #[serde(default)]
    pub scope: StoreScope,
    /// Empty = every kind.
    #[serde(default)]
    pub kinds: Vec<TxKind>,
    #[serde(default)]
    pub contact_id: Option<String>,
    #[serde(default)]
    pub flow_id: Option<String>,
    /// First day, `YYYY-MM-DD` in the user's zone, inclusive.
    #[serde(default)]
    pub from: Option<String>,
    /// Last day, `YYYY-MM-DD` in the user's zone, inclusive.
    #[serde(default)]
    pub to: Option<String>,
    /// Leave out transfers and conversions inside one top-level wallet.
    #[serde(default)]
    pub hide_internal: bool,
    /// Words to find in the comment, categories, assets, stores, contacts or hash.
    #[serde(default)]
    pub text: Option<String>,
    /// The main category key, as reports group by it; `""` = without a category.
    #[serde(default)]
    pub category: Option<String>,
    /// Only transactions that paid a fee (what "Losses" are made of).
    #[serde(default)]
    pub with_fee: bool,
    /// Only transactions that closed a position with a profit or a loss
    /// (what the P&L report adds up).
    #[serde(default)]
    pub realized: bool,
    /// Value in the base currency, by the larger side, at least this much.
    #[serde(default)]
    pub amount_min: Option<Amount>,
    /// Value in the base currency, by the larger side, at most this much.
    #[serde(default)]
    pub amount_max: Option<Amount>,
    /// Only what an automation or a workflow wrote.
    #[serde(default)]
    pub auto_only: bool,
    /// Only transfers that opened or settled a personal debt.
    #[serde(default)]
    pub debt_only: bool,
    /// Only operations with a file or a link (Full-New-Attach 6).
    #[serde(default)]
    pub with_attachments: bool,
    /// Only operations the budget could not value: a side had no rate
    /// (Currency-1 · 7).
    #[serde(default)]
    pub unpriced: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeKind {
    /// The previous operation on the same lane.
    Seq,
    /// Money moved from one lane to another.
    Transfer,
    /// An exchange whose sides sit on two lanes.
    Convert,
    /// The next link of the same chain (`flow_id`).
    Flow,
}

/// One row of the graph: a store, or a store with its sub-accounts folded in.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GraphLane {
    pub store_id: String,
    /// From the top-level wallet down, `A › B`.
    pub name: String,
    pub root_id: String,
    pub depth: u32,
    /// The store has sub-accounts (it can be folded).
    pub has_children: bool,
    /// Its sub-accounts are folded into this lane.
    pub collapsed: bool,
}

/// One transaction on one lane; a transfer between two lanes is two nodes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GraphNode {
    /// `tx_id:lane`.
    pub id: String,
    pub tx_id: String,
    pub kind: TxKind,
    pub timestamp: String,
    /// Index in `TxGraph::lanes` — the y coordinate.
    pub lane: u32,
    /// Rank in time — the x coordinate; both nodes of a transfer share it.
    pub column: u32,
    /// What moved, in the base currency at the rate of the moment.
    pub value: Amount,
    pub flow_id: Option<String>,
    pub internal: bool,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct GraphEdge {
    /// Node ids; the source is always earlier, so the graph has no cycles.
    pub source: String,
    pub target: String,
    pub kind: GraphEdgeKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TxGraph {
    pub currency: String,
    pub lanes: Vec<GraphLane>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Columns used: `max(column) + 1`.
    pub columns: u32,
}

/// The filtered transactions, newest first.
pub fn list_transactions(
    books: &Books,
    sources: &dyn FilterSources,
    filter: &TxFilter,
    zone: &str,
) -> Result<Vec<Transaction>> {
    let mut out = filtered(books, sources, filter, zone)?;
    out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    Ok(out.into_iter().cloned().collect())
}

/// The filtered operations as lines of fields, header first, oldest first, one
/// line per leg: the "Export ▾ › Transactions by filter" of the list.
pub fn transaction_lines(
    books: &Books,
    sources: &dyn FilterSources,
    filter: &TxFilter,
    zone: &str,
) -> Result<Vec<Vec<String>>> {
    let doc = books.doc();
    let mut txs = filtered(books, sources, filter, zone)?;
    txs.sort_by(|left, right| left.meta().timestamp.cmp(&right.meta().timestamp));
    let word = |value: serde_json::Value| value.as_str().unwrap_or_default().to_string();
    let mut lines = vec![
        [
            "timestamp",
            "id",
            "kind",
            "store",
            "asset",
            "amount",
            "role",
            "category",
            "comment",
        ]
        .map(String::from)
        .to_vec(),
    ];
    for tx in txs {
        let meta = tx.meta();
        let kind = word(serde_json::to_value(tx.kind()).unwrap_or_default());
        for leg in tx.legs() {
            lines.push(vec![
                meta.timestamp.clone(),
                meta.id.clone(),
                kind.clone(),
                find_store(doc, &leg.store_id)
                    .map(|store| store.name.clone())
                    .unwrap_or_else(|| leg.store_id.clone()),
                leg.asset.symbol.clone(),
                leg.amount.to_string(),
                word(serde_json::to_value(leg.role).unwrap_or_default()),
                meta.categories.first().cloned().unwrap_or_default(),
                meta.comment.clone(),
            ]);
        }
    }
    Ok(lines)
}

/// `limit` operations from `offset`, newest first: a long journal goes to
/// the screen a page at a time instead of whole on every refresh.
pub fn list_transactions_page(
    books: &Books,
    sources: &dyn FilterSources,
    filter: &TxFilter,
    offset: u32,
    limit: u32,
    zone: &str,
) -> Result<TxPage> {
    let mut out = filtered(books, sources, filter, zone)?;
    out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    let total = u32::try_from(out.len()).unwrap_or(u32::MAX);
    let items = out
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .cloned()
        .collect();
    Ok(TxPage { items, total })
}

/// The filtered transactions as lanes and edges. `collapsed` lists stores
/// whose sub-accounts fold into the store's own lane.
pub fn tx_graph(
    books: &Books,
    sources: &dyn FilterSources,
    wallets: &[WalletRow],
    filter: &TxFilter,
    collapsed: &[String],
    zone: &str,
) -> Result<TxGraph> {
    let doc = books.doc();
    let mut txs = filtered(books, sources, filter, zone)?;
    // Oldest first; equal moments keep the order they were written in.
    txs.sort_by_key(|tx| secs_or_now(&tx.meta().timestamp));

    let in_scope = (!filter.scope.store_ids.is_empty()).then(|| resolve_scope(doc, &filter.scope));
    let folded: HashSet<&str> = collapsed.iter().map(String::as_str).collect();
    let lane_of = |store_id: &str| -> Option<String> {
        if in_scope.as_ref().is_some_and(|ids| !ids.contains(store_id)) {
            return None;
        }
        let chain = ancestors_of(doc, store_id);
        let outer = chain
            .iter()
            .find(|store| folded.contains(store.id.as_str()));
        Some(outer.map_or_else(|| store_id.to_string(), |store| store.id.clone()))
    };

    // Occurrences: one per lane a transaction touches, source side first.
    let mut occurrences: Vec<(usize, Vec<String>)> = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    for (index, tx) in txs.iter().enumerate() {
        let mut lanes: Vec<String> = Vec::new();
        let sides = tx
            .accounts()
            .into_iter()
            .filter_map(|account| account.store_id());
        for store_id in sides.map(str::to_string).chain(tx.store_ids()) {
            if let Some(lane) = lane_of(&store_id)
                && !lanes.contains(&lane)
            {
                lanes.push(lane);
            }
        }
        used.extend(lanes.iter().cloned());
        occurrences.push((index, lanes));
    }
    let node_count: usize = occurrences.iter().map(|(_, lanes)| lanes.len()).sum();
    if node_count > GRAPH_NODE_CAP {
        return Err(Error::TooLarge(format!(
            "{node_count} nodes, at most {GRAPH_NODE_CAP}"
        )));
    }

    let lanes: Vec<GraphLane> = wallets
        .iter()
        .filter(|wallet| used.contains(&wallet.store_id))
        .map(|wallet| GraphLane {
            store_id: wallet.store_id.clone(),
            name: wallet.path.join(" › "),
            root_id: root_of(doc, &wallet.store_id),
            depth: wallet.depth,
            has_children: wallet.has_children,
            collapsed: folded.contains(wallet.store_id.as_str()),
        })
        .collect();
    let lane_index: HashMap<&str, u32> = lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| (lane.store_id.as_str(), index as u32))
        .collect();

    let currency = books.base().to_string();
    let mut nodes = Vec::with_capacity(node_count);
    let mut edges = Vec::new();
    let mut last_on_lane: HashMap<String, String> = HashMap::new();
    let mut last_in_flow: HashMap<String, String> = HashMap::new();
    let mut column = 0u32;
    for (index, lanes_of_tx) in &occurrences {
        if lanes_of_tx.is_empty() {
            continue;
        }
        let tx = txs[*index];
        let meta = tx.meta();
        let value = value_in_base(books, tx, &currency);
        let internal = tx.is_internal(|id| root_of(doc, id));
        let ids: Vec<String> = lanes_of_tx
            .iter()
            .map(|lane| format!("{}:{lane}", meta.id))
            .collect();
        for (lane, id) in lanes_of_tx.iter().zip(&ids) {
            nodes.push(GraphNode {
                id: id.clone(),
                tx_id: meta.id.clone(),
                kind: tx.kind(),
                timestamp: meta.timestamp.clone(),
                lane: lane_index.get(lane.as_str()).copied().unwrap_or_default(),
                column,
                value,
                flow_id: meta.flow_id.clone(),
                internal,
                comment: meta.comment.clone(),
            });
            if let Some(previous) = last_on_lane.insert(lane.clone(), id.clone()) {
                edges.push(edge(previous, id.clone(), GraphEdgeKind::Seq));
            }
        }
        let across = if tx.kind() == TxKind::Convert {
            GraphEdgeKind::Convert
        } else {
            GraphEdgeKind::Transfer
        };
        for pair in ids.windows(2) {
            edges.push(edge(pair[0].clone(), pair[1].clone(), across));
        }
        if let Some(flow) = &meta.flow_id {
            if let Some(previous) = last_in_flow.get(flow) {
                edges.push(edge(previous.clone(), ids[0].clone(), GraphEdgeKind::Flow));
            }
            last_in_flow.insert(flow.clone(), ids[ids.len() - 1].clone());
        }
        column += 1;
    }

    Ok(TxGraph {
        currency,
        lanes,
        nodes,
        edges,
        columns: column,
    })
}

fn filtered<'doc>(
    books: &Books<'doc>,
    sources: &dyn FilterSources,
    filter: &TxFilter,
    zone: &str,
) -> Result<Vec<&'doc Transaction>> {
    let doc = books.doc();
    let cal = Calendar::new(zone);
    let from = parse_day(filter.from.as_deref())?;
    let to = parse_day(filter.to.as_deref())?;
    let ids = (!filter.scope.store_ids.is_empty()).then(|| resolve_scope(doc, &filter.scope));
    let needle = filter
        .text
        .as_deref()
        .map(|text| text.trim().to_lowercase())
        .filter(|text| !text.is_empty());
    let contact = filter.contact_id.as_deref().filter(|id| !id.is_empty());
    let flow = filter.flow_id.as_deref().filter(|id| !id.is_empty());
    // The P&L replay of the same wallets says which transactions realized something.
    let realized = filter
        .realized
        .then(|| sources.realized_tx_ids(ids.as_ref()));

    let attached: HashSet<String> = if filter.with_attachments {
        sources.tx_ids_with_attachments().into_iter().collect()
    } else {
        HashSet::new()
    };

    let unpriced: HashSet<String> = if filter.unpriced {
        sources.unpriced_tx_ids()
    } else {
        HashSet::new()
    };

    // Converting every operation only when the filter asks for a value.
    let priced = (filter.amount_min.is_some() || filter.amount_max.is_some()).then(|| books.base());

    Ok(doc
        .transactions
        .iter()
        .filter(|tx| {
            ids.as_ref()
                .is_none_or(|ids| ids.iter().any(|id| tx.touches(id)))
        })
        .filter(|tx| filter.kinds.is_empty() || filter.kinds.contains(&tx.kind()))
        .filter(|tx| contact.is_none_or(|id| tx.contact_ids().contains(&id)))
        .filter(|tx| flow.is_none_or(|id| tx.meta().flow_id.as_deref() == Some(id)))
        .filter(|tx| {
            if from.is_none() && to.is_none() {
                return true;
            }
            let day = cal.date_of(secs_or_now(&tx.meta().timestamp));
            from.is_none_or(|first| day >= first) && to.is_none_or(|last| day <= last)
        })
        .filter(|tx| !filter.hide_internal || !tx.is_internal(|id| root_of(doc, id)))
        .filter(|tx| !filter.with_fee || tx.legs().iter().any(|leg| leg.role == LegRole::Fee))
        .filter(|tx| !filter.with_attachments || attached.contains(&tx.meta().id))
        .filter(|tx| !filter.unpriced || unpriced.contains(&tx.meta().id))
        .filter(|tx| {
            realized
                .as_ref()
                .is_none_or(|set| set.contains(&tx.meta().id))
        })
        .filter(|tx| {
            !filter.auto_only
                || !matches!(tx.meta().origin, TxOrigin::Manual | TxOrigin::Import { .. })
        })
        .filter(|tx| {
            !filter.debt_only
                || matches!(tx, Transaction::Transfer(transfer) if transfer.debt.is_some())
        })
        .filter(|tx| {
            priced.is_none_or(|base| {
                let value = value_in_base(books, tx, base);
                filter.amount_min.is_none_or(|min| value >= min)
                    && filter.amount_max.is_none_or(|max| value <= max)
            })
        })
        .filter(|tx| {
            filter
                .category
                .as_deref()
                .is_none_or(|key| tx.meta().categories.first().map_or("", String::as_str) == key)
        })
        .filter(|tx| {
            needle
                .as_deref()
                .is_none_or(|needle| search_text(doc, tx).contains(needle))
        })
        .collect())
}

/// Everything a search can match, lowercased.
fn search_text(doc: &VaultDoc, tx: &Transaction) -> String {
    let meta = tx.meta();
    let mut parts: Vec<String> = vec![meta.comment.clone()];
    parts.extend(meta.categories.iter().cloned());
    for leg in tx.legs() {
        parts.push(leg.asset.symbol.clone());
        parts.push(leg.asset.name.clone());
    }
    for id in tx.store_ids() {
        if let Some(store) = find_store(doc, &id) {
            parts.push(store.name.clone());
        }
    }
    for id in tx.contact_ids() {
        if let Some(contact) = doc.contacts.iter().find(|contact| contact.id == id) {
            parts.push(contact.name.clone());
        }
    }
    for account in tx.accounts() {
        if let crate::model::transaction::AccountRef::Raw(raw) = account {
            parts.push(raw.clone());
        }
    }
    for (_, hash) in tx.on_chain_refs() {
        parts.extend(hash.map(str::to_string));
    }
    // File names and link captions: "where was the skycapital receipt?".
    for item in doc.attachments.iter().filter(|item| {
        item.removed_at.is_none()
            && item.owner.kind == crate::model::entity_ref::EntityKind::Tx
            && item.owner.id == meta.id
    }) {
        parts.push(item.title.clone());
        parts.extend(item.url.clone());
    }
    parts.join(" ").to_lowercase()
}

fn parse_day(raw: Option<&str>) -> Result<Option<NaiveDate>> {
    match raw.map(str::trim).filter(|raw| !raw.is_empty()) {
        None => Ok(None),
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| Error::InvalidInput(format!("date {raw}: expected YYYY-MM-DD"))),
    }
}

fn edge(source: String, target: String, kind: GraphEdgeKind) -> GraphEdge {
    GraphEdge {
        source,
        target,
        kind,
    }
}

/// The larger of what left and what arrived, by principal legs, in `base`.
fn value_in_base(books: &Books, tx: &Transaction, base: &str) -> Amount {
    let secs = secs_or_now(&tx.meta().timestamp);
    let (mut gave, mut got) = (Amount::ZERO, Amount::ZERO);
    for leg in tx
        .legs()
        .iter()
        .filter(|leg| leg.role == LegRole::Principal)
    {
        let value = leg.amount.abs() * books.rate_book().leg_price_in_base(leg, base, secs);
        if leg.amount.is_negative() {
            gave += value;
        } else {
            got += value;
        }
    }
    gave.max(got).round(2)
}
