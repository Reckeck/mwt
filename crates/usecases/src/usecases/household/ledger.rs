//! Scenarios of the ledger area: reads, so no write. They hand the pure
//! functions what the portfolio, attachments and budget areas still hold
//! (OD6) through `FilterSources`, so the pure side never names the vault.

use std::collections::HashSet;

use crate::error::Result;
use crate::model::transaction::Transaction;
use crate::money::ledger::{self, FilterSources, TxFilter, TxGraph, TxPage, WalletRow};
use crate::money::portfolio::StoreTreeNode;
use crate::service::Session;

/// The vault as the filter's source for the areas that have not moved.
struct VaultSources<'vault>(&'vault Session);

impl FilterSources for VaultSources<'_> {
    fn realized_tx_ids(&self, scope: Option<&HashSet<String>>) -> HashSet<String> {
        crate::money::portfolio::realized_pnl_filtered(&self.0.books(), scope)
            .legs
            .into_iter()
            .filter(|leg| !leg.pnl_base.is_zero())
            .map(|leg| leg.tx_id)
            .collect()
    }

    fn tx_ids_with_attachments(&self) -> Vec<String> {
        crate::household::attachments::transactions_with_attachments(&self.0.doc)
    }

    fn unpriced_tx_ids(&self) -> HashSet<String> {
        crate::usecases::planning::budget::unpriced_tx_ids(&self.0.books())
    }
}

/// The wallet tree in display order, parents before their sub-accounts.
fn wallet_rows(tree: &[StoreTreeNode]) -> Vec<WalletRow> {
    let mut rows = Vec::new();
    for node in tree {
        rows.push(WalletRow {
            store_id: node.store.id.clone(),
            path: node.path.clone(),
            depth: node.depth,
            has_children: !node.children.is_empty(),
        });
        rows.extend(wallet_rows(&node.children));
    }
    rows
}

/// The filtered transactions, newest first.
pub fn list_transactions(
    vault: &Session,
    filter: &TxFilter,
    zone: &str,
) -> Result<Vec<Transaction>> {
    ledger::list_transactions(&vault.books(), &VaultSources(vault), filter, zone)
}

/// The filtered operations as CSV, oldest first, one line per leg: the
/// "Export ▾ › Transactions by filter" of the list.
pub fn transactions_csv(vault: &Session, filter: &TxFilter, zone: &str) -> Result<String> {
    ledger::transaction_lines(&vault.books(), &VaultSources(vault), filter, zone)
        .map(mwt_reports::overview::csv_text)
}

/// `limit` operations from `offset`, newest first: a long journal goes to
/// the screen a page at a time instead of whole on every refresh.
pub fn list_transactions_page(
    vault: &Session,
    filter: &TxFilter,
    offset: u32,
    limit: u32,
    zone: &str,
) -> Result<TxPage> {
    ledger::list_transactions_page(
        &vault.books(),
        &VaultSources(vault),
        filter,
        offset,
        limit,
        zone,
    )
}

/// The filtered transactions as lanes and edges. `collapsed` lists stores
/// whose sub-accounts fold into the store's own lane.
pub fn tx_graph(
    vault: &Session,
    filter: &TxFilter,
    collapsed: &[String],
    zone: &str,
) -> Result<TxGraph> {
    ledger::tx_graph(
        &vault.books(),
        &VaultSources(vault),
        &wallet_rows(&crate::money::portfolio::store_tree(&vault.books())),
        filter,
        collapsed,
        zone,
    )
}
