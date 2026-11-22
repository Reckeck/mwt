//! Tests of the ledger area that need the vault (`UnlockedVault`): moved from
//! `money/ledger_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::store::StoreKind;
use crate::model::store::StoreScope;
use crate::model::transaction::AccountRef;
use crate::model::transaction::{Transaction, TxKind, TxOrigin};
use crate::money::ledger::*;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use std::collections::HashMap;

fn at(mut tx: Transaction, iso: &str) -> Transaction {
    tx.meta_mut().timestamp = iso.into();
    tx
}

/// A filter without a realized, attachment or unpriced term asks nothing of
/// the areas that have not moved.
struct NoSources;

impl FilterSources for NoSources {
    fn realized_tx_ids(
        &self,
        _scope: Option<&std::collections::HashSet<String>>,
    ) -> std::collections::HashSet<String> {
        unreachable!("the filter does not ask for realized operations")
    }

    fn tx_ids_with_attachments(&self) -> Vec<String> {
        unreachable!("the filter does not ask for attachments")
    }

    fn unpriced_tx_ids(&self) -> std::collections::HashSet<String> {
        unreachable!("the filter does not ask for unpriced operations")
    }
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
fn csv_has_a_line_per_leg_of_what_the_filter_keeps() {
    let (_dir, vault, _ids) = ledger();
    let filter = TxFilter {
        kinds: vec![TxKind::Payment],
        ..TxFilter::default()
    };
    let csv = vault.transactions_csv(&filter, "UTC").unwrap();
    let lines: Vec<&str> = csv
        .trim_start_matches('\u{feff}')
        .trim_end()
        .split("\r\n")
        .collect();
    assert_eq!(lines.len(), 2, "header and the coffee payment");
    assert!(lines[0].starts_with("timestamp;id;kind;store"));
    assert!(lines[1].contains(";payment;Card;USD;-50;principal;"));
    assert!(lines[1].ends_with(";coffee"));
}

#[test]
fn filters_by_value_origin_and_debt() {
    let (_dir, mut vault, [card, _savings, _exchange]) = ledger();
    let list = |vault: &UnlockedVault, filter: TxFilter| {
        vault.list_transactions(&filter, "UTC").unwrap().len()
    };

    let between = TxFilter {
        amount_min: Some(amount("100")),
        amount_max: Some(amount("500")),
        ..TxFilter::default()
    };
    assert_eq!(list(&vault, between), 2, "the 300 and 200 transfers");

    let mut auto = payment(&card, "10", "auto");
    auto.meta_mut().origin = TxOrigin::Automation {
        automation_id: "a1".into(),
        planned_at: "2026-03-05T10:00:00Z".into(),
    };
    vault
        .add_transaction(at(auto, "2026-03-05T10:00:00Z"))
        .unwrap();
    let auto_only = TxFilter {
        auto_only: true,
        ..TxFilter::default()
    };
    assert_eq!(list(&vault, auto_only), 1);

    let debt_only = TxFilter {
        debt_only: true,
        ..TxFilter::default()
    };
    assert_eq!(list(&vault, debt_only), 0, "no transfer is a debt");
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

#[test]
fn the_pure_list_reads_a_read_context_and_the_delegate_agrees() {
    let (_dir, vault, [card, ..]) = ledger();
    let filter = TxFilter {
        scope: StoreScope::store(&card, true),
        ..TxFilter::default()
    };
    let pure = list_transactions(&vault.books(), &NoSources, &filter, "UTC").unwrap();
    assert_eq!(pure, vault.list_transactions(&filter, "UTC").unwrap());
    assert!(!pure.is_empty());
}
