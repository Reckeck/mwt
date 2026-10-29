//! "Where it went" (Money 1 · 6): where the period's money went — spent, lent,
//! moved between the user's own wallets, invested, lost to fees — each with
//! what it was, in the units it moved in.

use std::collections::BTreeMap;

use crate::Amount;
use crate::model::transaction::{AccountRef, DebtEffect, Transaction};
use crate::pricing::history::secs_or_now;

use super::rates::RateBook;
use super::reports::{Ctx, ReportMeta};
use super::{FlowClass, UnlockedVault};

/// Items a row lists by name; the rest only counts.
pub const WHERE_ITEMS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum WhereKind {
    Spending,
    Lent,
    Moved,
    Invested,
    Losses,
}

/// One thing behind a row: a category, a loan, a move, a buy.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WhereItem {
    /// A category key (spending), a contact (lent), the source wallet (moved,
    /// invested).
    pub label: String,
    /// The other wallet: the receiving one of a move, the paying one of a loan.
    pub to: Option<String>,
    /// What left, in its own units; zero for a category.
    pub qty: Amount,
    pub unit: String,
    /// What came back: a buy, a conversion between wallets.
    pub got_qty: Option<Amount>,
    pub got_unit: Option<String>,
    /// In the report currency; `None` without a price that day.
    pub value: Option<Amount>,
    pub comment: String,
    /// The operation's moment, RFC3339; empty for a category's total.
    #[serde(default)]
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WhereRow {
    pub kind: WhereKind,
    /// In the report currency: the priced part when `partial`.
    pub value: Amount,
    pub count: u32,
    /// Biggest first, at most [`WHERE_ITEMS`].
    pub items: Vec<WhereItem>,
    /// Some of it had no price.
    pub partial: bool,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WhereReport {
    pub meta: ReportMeta,
    pub income: Amount,
    /// Only the directions that had anything, in a fixed order.
    pub rows: Vec<WhereRow>,
    /// Income not spent: it stays, or went into the rows below spending.
    pub left: Amount,
}

impl WhereRow {
    fn new(kind: WhereKind) -> Self {
        Self {
            kind,
            value: Amount::ZERO,
            count: 0,
            items: Vec::new(),
            partial: false,
        }
    }

    fn add(&mut self, item: WhereItem) {
        match item.value {
            Some(value) => self.value += value,
            None => self.partial = true,
        }
        self.count += 1;
        self.items.push(item);
    }

    /// Biggest first; a row with nothing is dropped.
    fn finish(mut self) -> Option<Self> {
        if self.count == 0 && self.value.is_zero() {
            return None;
        }
        self.items.sort_by(|left, right| {
            right
                .value
                .unwrap_or_default()
                .cmp(&left.value.unwrap_or_default())
        });
        self.items.truncate(WHERE_ITEMS);
        Some(self)
    }
}

impl UnlockedVault {
    pub(crate) fn where_report(&self, ctx: &Ctx) -> WhereReport {
        let (from, to) = (ctx.start(), ctx.end());
        let mut income = Amount::ZERO;
        let mut categories: BTreeMap<String, (Amount, u32)> = BTreeMap::new();
        for flow in self.report_flows(ctx, from, to) {
            let month = crate::calendar::first_of_month(flow.date);
            let value = ctx.shown(flow.value, month);
            match flow.class {
                FlowClass::Income => income += value,
                FlowClass::Fixed | FlowClass::Daily => {
                    let key = flow.categories.first().cloned().unwrap_or_default();
                    let entry = categories.entry(key).or_default();
                    entry.0 += value;
                    entry.1 += 1;
                }
                _ => {}
            }
        }
        let mut spending = WhereRow::new(WhereKind::Spending);
        for (key, (value, count)) in categories {
            spending.add(WhereItem {
                label: key,
                value: Some(value),
                ..WhereItem::default()
            });
            spending.count += count - 1;
        }
        let spent = spending.value;

        let book = self.rate_book();
        let mut lent = WhereRow::new(WhereKind::Lent);
        let mut moved = WhereRow::new(WhereKind::Moved);
        let mut invested = WhereRow::new(WhereKind::Invested);
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            if secs < from || secs >= to || !self.in_scope(ctx, tx) {
                continue;
            }
            match tx {
                Transaction::Transfer(inner) => {
                    let value = self.value_of(
                        &book,
                        &ctx.currency,
                        inner.amount,
                        &inner.asset.symbol,
                        secs,
                    );
                    let item = WhereItem {
                        qty: inner.amount,
                        unit: inner.asset.symbol.clone(),
                        value,
                        comment: inner.meta.comment.clone(),
                        at: inner.meta.timestamp.clone(),
                        ..WhereItem::default()
                    };
                    match (inner.debt, &inner.from, &inner.to) {
                        (Some(DebtEffect::Lend), from, to) => lent.add(WhereItem {
                            label: self.account_name(to),
                            to: Some(self.account_name(from)),
                            ..item
                        }),
                        (None, AccountRef::Store(from), AccountRef::Store(to)) => {
                            moved.add(WhereItem {
                                label: self.store_label(from),
                                to: Some(self.store_label(to)),
                                ..item
                            })
                        }
                        _ => {}
                    }
                }
                Transaction::Convert(inner) => {
                    if let (AccountRef::Store(from), AccountRef::Store(to)) =
                        (&inner.from.account, &inner.to.account)
                    {
                        moved.add(WhereItem {
                            label: self.store_label(from),
                            to: Some(self.store_label(to)),
                            qty: inner.from.amount,
                            unit: inner.from.asset.symbol.clone(),
                            got_qty: Some(inner.to.amount),
                            got_unit: Some(inner.to.asset.symbol.clone()),
                            value: self.value_of(
                                &book,
                                &ctx.currency,
                                inner.from.amount,
                                &inner.from.asset.symbol,
                                secs,
                            ),
                            comment: inner.meta.comment.clone(),
                            at: inner.meta.timestamp.clone(),
                        });
                    }
                }
                Transaction::TradeSpot(inner) => invested.add(WhereItem {
                    label: self.store_label(&inner.store_id),
                    qty: inner.sold.qty,
                    unit: inner.sold.asset.symbol.clone(),
                    got_qty: Some(inner.bought.qty),
                    got_unit: Some(inner.bought.asset.symbol.clone()),
                    value: self.value_of(
                        &book,
                        &ctx.currency,
                        inner.sold.qty,
                        &inner.sold.asset.symbol,
                        secs,
                    ),
                    comment: inner.meta.comment.clone(),
                    at: inner.meta.timestamp.clone(),
                    ..WhereItem::default()
                }),
                _ => {}
            }
        }
        let losses = self.loss_report(ctx);
        let mut lost = WhereRow::new(WhereKind::Losses);
        lost.value = losses.total.total;
        lost.count = losses.by_kind.iter().map(|bucket| bucket.count).sum();
        lost.partial = losses.incomplete > 0;

        WhereReport {
            meta: ctx.meta(),
            income,
            rows: [spending, lent, moved, invested, lost]
                .into_iter()
                .filter_map(WhereRow::finish)
                .collect(),
            left: income - spent,
        }
    }

    fn in_scope(&self, ctx: &Ctx, tx: &Transaction) -> bool {
        ctx.scope
            .as_ref()
            .is_none_or(|ids| tx.legs().iter().any(|leg| ids.contains(&leg.store_id)))
    }

    fn value_of(
        &self,
        book: &RateBook,
        currency: &str,
        qty: Amount,
        unit: &str,
        secs: i64,
    ) -> Option<Amount> {
        book.convert(qty.abs(), unit, currency, secs)
    }

    fn store_label(&self, id: &str) -> String {
        self.find_store(id)
            .map(|store| store.name.clone())
            .unwrap_or_default()
    }

    fn account_name(&self, account: &AccountRef) -> String {
        match account {
            AccountRef::Store(id) => self.store_label(id),
            AccountRef::Contact { contact_id, .. } => self
                .doc
                .contacts
                .iter()
                .find(|contact| &contact.id == contact_id)
                .map(|contact| contact.name.clone())
                .unwrap_or_default(),
            AccountRef::Raw(text) => text.clone(),
        }
    }
}
