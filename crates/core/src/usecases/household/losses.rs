//! Scenarios of the losses area. The rates come from the rates area's book
//! (called by method: OD6) and reach the pure functions through `Valuation`
//! and `FxSources`. The two chain writes each end in one `vault.transact`.
//!
//! The month report lives here, not in `household::losses`, because it carries
//! the reports area's `ReportMeta` and reads its `Ctx`; both move with that area.

use std::collections::{BTreeMap, HashSet};

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::household::losses::{
    CryptoQuote, FlowSummary, FxSources, FxTable, LossBucket, LossBuckets, LossRow, TxLoss,
    Valuation, chain_summaries, loss_by_id, loss_of, losses_by_id, plan_link, rates_table,
    touches_any,
};
use crate::kernel::books::Books;
use crate::model::store::AssetRef;
use crate::model::transaction::Leg;
use crate::pricing::history::secs_or_now;
use crate::service::reports::{Ctx, ReportMeta};
use crate::service::{RateBook, UnlockedVault};

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct LossReport {
    pub meta: ReportMeta,
    pub rows: Vec<LossRow>,
    pub total: LossRow,
    pub by_kind: Vec<LossBucket>,
    pub by_store: Vec<LossBucket>,
    pub by_contact: Vec<LossBucket>,
    pub by_flow: Vec<LossBucket>,
    /// Transactions whose loss is only partly known.
    pub incomplete: u32,
}

/// The book of one request as the pure functions' valuation.
struct BookValues<'book>(&'book RateBook);

impl Valuation for BookValues<'_> {
    fn leg_value_in(&self, leg: &Leg, currency: &str, secs: i64) -> Option<Amount> {
        UnlockedVault::leg_value_in(self.0, leg, currency, secs)
    }

    fn value_at(&self, asset: &AssetRef, qty: Amount, currency: &str, secs: i64) -> Amount {
        UnlockedVault::value_at(self.0, asset, qty, currency, secs)
    }
}

/// The vault's rates as the table's source.
struct VaultRates<'vault> {
    vault: &'vault UnlockedVault,
    books: &'vault Books<'vault>,
    now: i64,
}

impl FxSources for VaultRates<'_> {
    fn per_usd(&self, currency: &str) -> Option<Amount> {
        self.vault.per_usd(currency)
    }

    fn tracked(&self) -> HashSet<String> {
        self.vault.fx_currencies_in_use()
    }

    fn crypto_quote(&self, code: &str) -> Option<CryptoQuote> {
        let asset = self.vault.favorite_asset(code)?;
        let (usd, taken_at) = self.books.rate_book().quote_at(&asset.key(), self.now)?;
        Some(CryptoQuote {
            usd,
            taken_at,
            held: self.vault.held_coin(code).is_some(),
        })
    }
}

/// The loss of one transaction in the base currency.
pub fn tx_loss(vault: &UnlockedVault, id: &str) -> Result<TxLoss> {
    let books = vault.books();
    loss_by_id(
        &vault.doc,
        &BookValues(books.rate_book()),
        books.base_and_fx().0,
        id,
    )
}

/// Losses of every transaction that has one, in the base currency, by id
/// — for the loss column of the list.
pub fn tx_losses(vault: &UnlockedVault) -> BTreeMap<String, TxLoss> {
    let books = vault.books();
    losses_by_id(
        &vault.doc,
        &BookValues(books.rate_book()),
        books.base_and_fx().0,
    )
}

/// Losses by month and by kind, wallet, contact and chain.
pub(crate) fn loss_report(vault: &UnlockedVault, ctx: &Ctx) -> LossReport {
    let books = vault.books();
    let values = BookValues(books.rate_book());
    let mut out = LossReport {
        meta: ctx.meta(),
        rows: ctx
            .months
            .iter()
            .map(|month| LossRow {
                month: calendar::month_key(*month),
                ..LossRow::default()
            })
            .collect(),
        ..LossReport::default()
    };
    let mut buckets = LossBuckets::default();
    for tx in &vault.doc.transactions {
        let secs = secs_or_now(&tx.meta().timestamp);
        let Some(index) = ctx.index_of(ctx.cal.date_of(secs)) else {
            continue;
        };
        if let Some(ids) = &ctx.scope
            && !touches_any(tx, ids)
        {
            continue;
        }
        let loss = loss_of(&vault.doc, &values, tx, &ctx.currency);
        if loss.total.is_zero() && loss.complete {
            continue;
        }
        let month = ctx.months[index];
        let (fee, slippage) = (ctx.shown(loss.fee, month), ctx.shown(loss.slippage, month));
        let row = &mut out.rows[index];
        row.fee += fee;
        row.slippage += slippage;
        row.total += fee + slippage;
        if !loss.complete {
            out.incomplete += 1;
        }
        buckets.add(tx, fee + slippage);
    }
    for row in &out.rows {
        out.total.fee += row.fee;
        out.total.slippage += row.slippage;
        out.total.total += row.total;
    }
    let [by_kind, by_store, by_contact, by_flow] = buckets.ranked();
    out.by_kind = by_kind;
    out.by_store = by_store;
    out.by_contact = by_contact;
    out.by_flow = by_flow;
    out
}

/// Put transactions into one chain: the given one, else the chain one of
/// them is already in, else a new one. Returns the chain id.
pub fn link_transactions(
    vault: &mut UnlockedVault,
    ids: &[String],
    flow_id: Option<&str>,
) -> Result<String> {
    let flow = plan_link(&vault.doc, ids, flow_id)?;
    let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
    vault.transact(|tx| {
        for linked in tx
            .doc_mut()
            .transactions
            .iter_mut()
            .filter(|linked| wanted.contains(linked.meta().id.as_str()))
        {
            linked.meta_mut().flow_id = Some(flow.clone());
        }
        Ok(flow)
    })
}

/// Take a transaction out of its chain.
pub fn unlink_transaction(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    vault.transact(|tx| {
        let unlinked = tx
            .doc_mut()
            .transactions
            .iter_mut()
            .find(|unlinked| unlinked.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        unlinked.meta_mut().flow_id = None;
        Ok(())
    })
}

/// Every chain with a link in `[from, to)` (RFC3339 bounds; empty = open),
/// newest first, valued in the base currency.
pub fn list_flows(
    vault: &UnlockedVault,
    from: Option<&str>,
    to: Option<&str>,
    store_ids: &[String],
) -> Vec<FlowSummary> {
    let books = vault.books();
    chain_summaries(
        &vault.doc,
        &BookValues(books.rate_book()),
        books.base_and_fx().0,
        from,
        to,
        store_ids,
    )
}

/// Every cached currency against the base, latest rates (plan F3b, "Rates"),
/// plus a row per crypto favourite that has a cached price.
pub fn fx_table(vault: &UnlockedVault, favorites: &[String]) -> FxTable {
    let books = vault.books();
    let rates = VaultRates {
        vault,
        books: &books,
        now: secs_or_now(""),
    };
    rates_table(&vault.doc, &rates, books.base_and_fx().0, favorites)
}
