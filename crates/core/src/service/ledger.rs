//! The operations list and the operations graph (plan Ф4).
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
use crate::model::transaction::{LegRole, Transaction, TxKind};
use crate::pricing::history::secs_or_now;

use super::rates::RateBook;
use super::{StoreTreeNode, UnlockedVault};

/// More nodes than this and the graph is unreadable (and slow): narrow the filter.
pub const GRAPH_NODE_CAP: usize = 2000;

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
    /// Only transactions that paid a fee (what «Потери» are made of).
    #[serde(default)]
    pub with_fee: bool,
    /// Only transactions that closed a position with a profit or a loss
    /// (what the P&L report adds up).
    #[serde(default)]
    pub realized: bool,
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

impl UnlockedVault {
    /// The filtered transactions, newest first.
    pub fn list_transactions(&self, filter: &TxFilter, zone: &str) -> Result<Vec<Transaction>> {
        let mut out = self.filtered(filter, zone)?;
        out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
        Ok(out.into_iter().cloned().collect())
    }

    /// `limit` operations from `offset`, newest first: a long journal goes to
    /// the screen a page at a time instead of whole on every refresh.
    pub fn list_transactions_page(
        &self,
        filter: &TxFilter,
        offset: u32,
        limit: u32,
        zone: &str,
    ) -> Result<TxPage> {
        let mut out = self.filtered(filter, zone)?;
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
    pub fn tx_graph(&self, filter: &TxFilter, collapsed: &[String], zone: &str) -> Result<TxGraph> {
        let mut txs = self.filtered(filter, zone)?;
        // Oldest first; equal moments keep the order they were written in.
        txs.sort_by_key(|tx| secs_or_now(&tx.meta().timestamp));

        let tree = self.store_tree();
        let order = flatten(&tree);
        let in_scope =
            (!filter.scope.store_ids.is_empty()).then(|| self.resolve_scope(&filter.scope));
        let folded: HashSet<&str> = collapsed.iter().map(String::as_str).collect();
        let lane_of = |store_id: &str| -> Option<String> {
            if in_scope.as_ref().is_some_and(|ids| !ids.contains(store_id)) {
                return None;
            }
            let chain = self.ancestors_of(store_id);
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

        let lanes: Vec<GraphLane> = order
            .iter()
            .filter(|node| used.contains(&node.store.id))
            .map(|node| GraphLane {
                store_id: node.store.id.clone(),
                name: node.path.join(" › "),
                root_id: self.root_of(&node.store.id),
                depth: node.depth,
                has_children: !node.children.is_empty(),
                collapsed: folded.contains(node.store.id.as_str()),
            })
            .collect();
        let lane_index: HashMap<&str, u32> = lanes
            .iter()
            .enumerate()
            .map(|(index, lane)| (lane.store_id.as_str(), index as u32))
            .collect();

        let (currency, _) = self.base_and_fx();
        let book = self.rate_book();
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
            let value = value_in_base(&book, tx, &currency);
            let internal = tx.is_internal(|id| self.root_of(id));
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

    fn filtered(&self, filter: &TxFilter, zone: &str) -> Result<Vec<&Transaction>> {
        let cal = Calendar::new(zone);
        let from = parse_day(filter.from.as_deref())?;
        let to = parse_day(filter.to.as_deref())?;
        let ids = (!filter.scope.store_ids.is_empty()).then(|| self.resolve_scope(&filter.scope));
        let needle = filter
            .text
            .as_deref()
            .map(|text| text.trim().to_lowercase())
            .filter(|text| !text.is_empty());
        let contact = filter.contact_id.as_deref().filter(|id| !id.is_empty());
        let flow = filter.flow_id.as_deref().filter(|id| !id.is_empty());
        // The P&L replay of the same wallets says which transactions realized something.
        let realized: Option<HashSet<String>> = filter.realized.then(|| {
            self.realized_pnl_filtered(ids.as_ref())
                .legs
                .into_iter()
                .filter(|leg| !leg.pnl_base.is_zero())
                .map(|leg| leg.tx_id)
                .collect()
        });

        Ok(self
            .doc
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
            .filter(|tx| !filter.hide_internal || !tx.is_internal(|id| self.root_of(id)))
            .filter(|tx| !filter.with_fee || tx.legs().iter().any(|leg| leg.role == LegRole::Fee))
            .filter(|tx| {
                realized
                    .as_ref()
                    .is_none_or(|set| set.contains(&tx.meta().id))
            })
            .filter(|tx| {
                filter.category.as_deref().is_none_or(|key| {
                    tx.meta().categories.first().map_or("", String::as_str) == key
                })
            })
            .filter(|tx| {
                needle
                    .as_deref()
                    .is_none_or(|needle| self.search_text(tx).contains(needle))
            })
            .collect())
    }

    /// Everything a search can match, lowercased.
    fn search_text(&self, tx: &Transaction) -> String {
        let meta = tx.meta();
        let mut parts: Vec<String> = vec![meta.comment.clone()];
        parts.extend(meta.categories.iter().cloned());
        for leg in tx.legs() {
            parts.push(leg.asset.symbol.clone());
            parts.push(leg.asset.name.clone());
        }
        for id in tx.store_ids() {
            if let Some(store) = self.find_store(&id) {
                parts.push(store.name.clone());
            }
        }
        for id in tx.contact_ids() {
            if let Some(contact) = self.doc.contacts.iter().find(|contact| contact.id == id) {
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
        parts.join(" ").to_lowercase()
    }
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

/// The wallet tree in display order, parents before their sub-accounts.
fn flatten(tree: &[StoreTreeNode]) -> Vec<&StoreTreeNode> {
    let mut out = Vec::new();
    for node in tree {
        out.push(node);
        out.extend(flatten(&node.children));
    }
    out
}

/// The larger of what left and what arrived, by principal legs, in `base`.
fn value_in_base(book: &RateBook, tx: &Transaction, base: &str) -> Amount {
    let secs = secs_or_now(&tx.meta().timestamp);
    let (mut gave, mut got) = (Amount::ZERO, Amount::ZERO);
    for leg in tx
        .legs()
        .iter()
        .filter(|leg| leg.role == LegRole::Principal)
    {
        let value = leg.amount.abs() * book.leg_price_in_base(leg, base, secs);
        if leg.amount.is_negative() {
            gave += value;
        } else {
            got += value;
        }
    }
    gave.max(got).round(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::AccountRef;
    use crate::service::test_support::*;

    fn at(mut tx: Transaction, iso: &str) -> Transaction {
        tx.meta_mut().timestamp = iso.into();
        tx
    }

    /// Card with a sub-account «Savings», an exchange, four operations.
    fn ledger() -> (tempfile::TempDir, UnlockedVault, [String; 3]) {
        let (dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let savings = store(&mut vault, "Savings", StoreKind::Bank, Some(&card));
        let exchange = store(&mut vault, "Exchange", StoreKind::Exchange, None);
        for tx in [
            at(income(&card, "1000", "salary"), "2026-03-01T10:00:00Z"),
            at(
                transfer_between(
                    AccountRef::Store(card.clone()),
                    AccountRef::Store(savings.clone()),
                    usd(),
                    "300",
                ),
                "2026-03-02T10:00:00Z",
            ),
            at(
                transfer_between(
                    AccountRef::Store(card.clone()),
                    AccountRef::Store(exchange.clone()),
                    usd(),
                    "200",
                ),
                "2026-03-03T10:00:00Z",
            ),
            at(payment(&card, "50", "coffee"), "2026-03-04T10:00:00Z"),
        ] {
            vault.add_transaction(tx).unwrap();
        }
        (dir, vault, [card, savings, exchange])
    }

    #[test]
    fn a_page_is_a_slice_of_the_full_list() {
        let (_dir, vault, _ids) = ledger();
        let filter = TxFilter::default();
        let all = vault.list_transactions(&filter, "UTC").unwrap();
        let first = vault.list_transactions_page(&filter, 0, 3, "UTC").unwrap();
        let second = vault.list_transactions_page(&filter, 3, 3, "UTC").unwrap();
        assert_eq!(first.total, 4);
        assert_eq!(first.items, all[..3]);
        assert_eq!(second.items, all[3..]);
        assert!(
            vault
                .list_transactions_page(&filter, 9, 3, "UTC")
                .unwrap()
                .items
                .is_empty()
        );
    }

    #[test]
    fn filters_by_scope_kind_period_and_text() {
        let (_dir, vault, [card, savings, _exchange]) = ledger();
        let list = |filter: TxFilter| vault.list_transactions(&filter, "UTC").unwrap();

        let only_savings = TxFilter {
            scope: StoreScope::store(&savings, true),
            ..TxFilter::default()
        };
        assert_eq!(list(only_savings).len(), 1);

        let card_tree = TxFilter {
            scope: StoreScope::store(&card, true),
            ..TxFilter::default()
        };
        let all = list(card_tree);
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].meta().comment, "coffee", "newest first");

        let payments = TxFilter {
            kinds: vec![TxKind::Payment],
            ..TxFilter::default()
        };
        assert_eq!(list(payments).len(), 1);

        let march_2_3 = TxFilter {
            from: Some("2026-03-02".into()),
            to: Some("2026-03-03".into()),
            ..TxFilter::default()
        };
        assert_eq!(list(march_2_3).len(), 2);

        let by_store_name = TxFilter {
            text: Some("exch".into()),
            ..TxFilter::default()
        };
        assert_eq!(list(by_store_name).len(), 1);
    }

    #[test]
    fn filter_covers_several_wallets() {
        let (_dir, vault, [_card, savings, exchange]) = ledger();
        let filter = TxFilter {
            scope: StoreScope {
                store_ids: vec![savings, exchange],
                include_children: true,
            },
            ..TxFilter::default()
        };
        assert_eq!(vault.list_transactions(&filter, "UTC").unwrap().len(), 2);
    }

    #[test]
    fn with_fee_keeps_only_operations_with_a_fee() {
        let (_dir, mut vault, [card, _savings, _exchange]) = ledger();
        let mut paid = at(payment(&card, "20", "atm"), "2026-03-05T10:00:00Z");
        if let Transaction::Payment(inner) = &mut paid {
            inner.fee = fee_usd("1");
        }
        let paid = vault.add_transaction(paid).unwrap();
        let filter = TxFilter {
            with_fee: true,
            ..TxFilter::default()
        };
        let list = vault.list_transactions(&filter, "UTC").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].meta().id, paid.meta().id);
    }

    #[test]
    fn realized_keeps_operations_that_closed_a_position() {
        let (_dir, mut vault, [_card, _savings, exchange]) = ledger();
        vault
            .add_transaction(at(
                income_of(&exchange, btc(), "1", "100", "buy"),
                "2026-03-06T10:00:00Z",
            ))
            .unwrap();
        let sale = vault
            .add_transaction(at(
                sell_btc(&exchange, "1", "150", "150"),
                "2026-03-07T10:00:00Z",
            ))
            .unwrap();
        let filter = TxFilter {
            realized: true,
            ..TxFilter::default()
        };
        let list = vault.list_transactions(&filter, "UTC").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].meta().id, sale.meta().id);
    }

    #[test]
    fn filters_by_the_main_category() {
        let (_dir, mut vault, [card, _savings, _exchange]) = ledger();
        let mut food = at(payment(&card, "20", "lunch"), "2026-03-05T10:00:00Z");
        food.meta_mut().categories = vec!["taxi".into(), "groceries".into()];
        vault.add_transaction(food).unwrap();
        let list = |key: &str| {
            let filter = TxFilter {
                category: Some(key.into()),
                ..TxFilter::default()
            };
            vault.list_transactions(&filter, "UTC").unwrap()
        };
        assert_eq!(list("taxi").len(), 1);
        // «coffee» is groceries too; lunch names it only second.
        assert_eq!(list("groceries").len(), 1, "only the first category counts");
        assert!(list("").iter().all(|tx| tx.meta().comment != "lunch"));
    }

    #[test]
    fn hide_internal_drops_moves_inside_one_wallet() {
        let (_dir, vault, [_card, savings, _exchange]) = ledger();
        let filter = TxFilter {
            hide_internal: true,
            ..TxFilter::default()
        };
        let list = vault.list_transactions(&filter, "UTC").unwrap();
        assert_eq!(
            list.len(),
            3,
            "card → savings is internal, card → exchange is not"
        );
        assert!(!list.iter().any(|tx| tx.touches(&savings)));
    }

    #[test]
    fn day_bounds_follow_the_zone() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        // 20:00 UTC on the 1st is already the 2nd in Bangkok.
        vault
            .add_transaction(at(payment(&card, "5", "late"), "2026-03-01T20:00:00Z"))
            .unwrap();
        let filter = TxFilter {
            from: Some("2026-03-02".into()),
            ..TxFilter::default()
        };
        assert_eq!(vault.list_transactions(&filter, "UTC").unwrap().len(), 0);
        assert_eq!(
            vault
                .list_transactions(&filter, "Asia/Bangkok")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn graph_chains_lanes_and_links_transfers() {
        let (_dir, vault, [card, savings, exchange]) = ledger();
        let graph = vault.tx_graph(&TxFilter::default(), &[], "UTC").unwrap();

        let lane_ids: Vec<&str> = graph
            .lanes
            .iter()
            .map(|lane| lane.store_id.as_str())
            .collect();
        let card_at = lane_ids.iter().position(|id| *id == card).unwrap();
        let savings_at = lane_ids.iter().position(|id| *id == savings).unwrap();
        assert_eq!(
            savings_at,
            card_at + 1,
            "sub-account right under its parent"
        );
        assert!(lane_ids.contains(&exchange.as_str()));

        // 4 operations: 1 + 2 + 2 + 1 nodes.
        assert_eq!(graph.nodes.len(), 6);
        assert_eq!(graph.columns, 4);
        let of = |kind| graph.edges.iter().filter(|edge| edge.kind == kind).count();
        assert_eq!(
            of(GraphEdgeKind::Seq),
            3,
            "four operations on the card in a row"
        );
        assert_eq!(of(GraphEdgeKind::Transfer), 2);

        // Every edge points forward in time: no cycles.
        let column: HashMap<&str, (u32, usize)> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), (node.column, index)))
            .collect();
        for edge in &graph.edges {
            assert!(
                column[edge.source.as_str()] < column[edge.target.as_str()],
                "{edge:?}"
            );
        }
    }

    #[test]
    fn collapsing_folds_a_sub_account_into_its_parent() {
        let (_dir, vault, [card, _savings, _exchange]) = ledger();
        let graph = vault
            .tx_graph(&TxFilter::default(), std::slice::from_ref(&card), "UTC")
            .unwrap();
        assert_eq!(graph.lanes.len(), 2);
        assert!(graph.lanes[0].collapsed);
        // card → savings is now one node on one lane.
        assert_eq!(graph.nodes.len(), 5);
        assert_eq!(
            graph
                .edges
                .iter()
                .filter(|edge| edge.kind == GraphEdgeKind::Transfer)
                .count(),
            1
        );
    }

    #[test]
    fn flow_links_the_next_operation_of_a_chain() {
        let (_dir, mut vault, [card, _savings, exchange]) = ledger();
        let ids: Vec<String> = vault
            .list_transactions(&TxFilter::default(), "UTC")
            .unwrap()
            .iter()
            .filter(|tx| tx.touches(&exchange) || tx.meta().comment == "coffee")
            .map(|tx| tx.meta().id.clone())
            .collect();
        vault.link_transactions(&ids, None).unwrap();
        let graph = vault.tx_graph(&TxFilter::default(), &[], "UTC").unwrap();
        let flows: Vec<&GraphEdge> = graph
            .edges
            .iter()
            .filter(|edge| edge.kind == GraphEdgeKind::Flow)
            .collect();
        assert_eq!(flows.len(), 1);
        assert!(flows[0].target.ends_with(&card), "the coffee on the card");
    }

    #[test]
    fn graph_refuses_more_than_the_cap() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        for _ in 0..=GRAPH_NODE_CAP {
            vault.doc.transactions.push(payment(&card, "1", "x"));
        }
        let err = vault
            .tx_graph(&TxFilter::default(), &[], "UTC")
            .unwrap_err();
        assert_eq!(err.code(), "TOO_LARGE");
    }
}
