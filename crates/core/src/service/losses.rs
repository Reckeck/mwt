//! Losses, chains and the rates table (plan §6.3–§6.4).
//!
//! A transaction loses money two ways: the fee, and the spread between what
//! was given and what came back, both valued at the **market** rate of that
//! moment — never at the rate the user typed (`rate_override`), since that
//! rate is exactly what is being judged. A chain (`flow_id`) strings
//! operations together — RUB → USDT through an exchanger, USDT → THB — to see
//! what went in, what came out and what was lost on the way.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::model::store::AssetRef;
use crate::model::transaction::{AccountRef, LegRole, Transaction};
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;
use super::rates::RateBook;
use super::reports::{Ctx, ReportMeta};

/// What one transaction cost beyond what it moved.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TxLoss {
    pub tx_id: String,
    pub currency: String,
    /// Fees at their stamped price.
    pub fee: Amount,
    /// Value given minus value received at the market rate of the moment;
    /// negative when the deal beat the market.
    pub slippage: Amount,
    pub total: Amount,
    /// A move between the user's own sub-accounts: only the fee and the
    /// spread count, it is neither income nor spending.
    pub internal: bool,
    /// Every side had a price; otherwise the missing side counts as zero.
    pub complete: bool,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct LossRow {
    pub month: String,
    pub fee: Amount,
    pub slippage: Amount,
    pub total: Amount,
}

/// Losses grouped by one key (a kind, a store, a contact, a chain).
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct LossBucket {
    pub key: String,
    pub total: Amount,
    pub count: u32,
}

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

/// One side of a chain's result.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct FlowAmount {
    pub asset: Option<AssetRef>,
    pub amount: Amount,
    pub value: Amount,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct FlowLink {
    pub tx: Transaction,
    pub loss: TxLoss,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct FlowSummary {
    pub flow_id: String,
    pub currency: String,
    /// Oldest first.
    pub links: Vec<FlowLink>,
    pub started_at: String,
    pub ended_at: String,
    /// What the first link took from the user's stores.
    pub gave: Vec<FlowAmount>,
    /// What the last link brought into them.
    pub got: Vec<FlowAmount>,
    pub gave_value: Amount,
    pub got_value: Amount,
    /// Fees and spreads of every link.
    pub loss: Amount,
    /// `gave_value − got_value`: the loss plus how rates moved meanwhile.
    pub difference: Amount,
}

/// One currency against the base.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct FxRow {
    pub currency: String,
    /// Units of this currency per one unit of the base.
    pub per_base: Amount,
    /// Base units one unit of this currency buys.
    pub in_base: Amount,
    /// When the rate was taken.
    pub updated_at: String,
    /// The vault uses this currency (it keeps rate history).
    pub tracked: bool,
    /// A coin from the favourites, rated from its cached price.
    #[serde(default)]
    pub crypto: bool,
    /// The user marked this code as a favourite.
    #[serde(default)]
    pub favorite: bool,
    /// Change of `in_base` against the rate a day earlier, percent, when the cache has one.
    #[serde(default)]
    pub change_pct: Option<Amount>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct FxTable {
    pub base: String,
    pub rows: Vec<FxRow>,
}

impl UnlockedVault {
    // -- losses -----------------------------------------------------------------

    /// The loss of one transaction in `currency` (plan §6.3).
    pub(crate) fn loss_of(&self, book: &RateBook, tx: &Transaction, currency: &str) -> TxLoss {
        let secs = secs_or_now(&tx.meta().timestamp);
        let mut complete = true;
        let mut fee = Amount::ZERO;
        for leg in tx.legs().into_iter().filter(|leg| leg.role == LegRole::Fee) {
            match Self::leg_value_in(book, &leg, currency, secs) {
                Some(value) => fee += value,
                None => complete = false,
            }
        }
        // Fee of a side the user does not hold (an exchanger's) has no leg.
        if fee.is_zero()
            && let Some(paid) = tx.fee()
            && !paid.amount.is_zero()
            && !tx.legs().iter().any(|leg| leg.role == LegRole::Fee)
        {
            fee = Self::value_at(book, &paid.asset, paid.amount.abs(), currency, secs);
        }
        let mut market = |asset: &AssetRef, qty: Amount| {
            let value = Self::value_at(book, asset, qty.abs(), currency, secs);
            if value.is_zero() && !qty.is_zero() {
                complete = false;
            }
            value
        };
        let slippage = match tx {
            Transaction::Convert(inner) => {
                market(&inner.from.asset, inner.from.amount)
                    - market(&inner.to.asset, inner.to.amount)
            }
            Transaction::TradeSpot(inner) => {
                market(&inner.sold.asset, inner.sold.qty)
                    - market(&inner.bought.asset, inner.bought.qty)
            }
            Transaction::AssetSell(inner) => {
                market(&inner.asset, inner.quantity)
                    - market(&inner.currency, inner.unit_price * inner.quantity)
            }
            _ => Amount::ZERO,
        };
        let fee = fee.round(2);
        let slippage = slippage.round(2);
        TxLoss {
            tx_id: tx.meta().id.clone(),
            currency: currency.to_string(),
            fee,
            slippage,
            total: fee + slippage,
            internal: tx.is_internal(|id| self.root_of(id)),
            complete,
        }
    }

    /// The loss of one transaction in the base currency.
    pub fn tx_loss(&self, id: &str) -> Result<TxLoss> {
        let tx = self
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        Ok(self.loss_of(&self.rate_book(), tx, &self.base_and_fx().0))
    }

    /// Losses of every transaction that has one, in the base currency, by id
    /// — for the loss column of the list.
    pub fn tx_losses(&self) -> BTreeMap<String, TxLoss> {
        let book = self.rate_book();
        let base = self.base_and_fx().0;
        self.doc
            .transactions
            .iter()
            .map(|tx| self.loss_of(&book, tx, &base))
            .filter(|loss| !loss.total.is_zero() || !loss.complete)
            .map(|loss| (loss.tx_id.clone(), loss))
            .collect()
    }

    /// Losses by month and by kind, wallet, contact and chain.
    pub(crate) fn loss_report(&self, ctx: &Ctx) -> LossReport {
        let book = self.rate_book();
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
        let mut buckets: [BTreeMap<String, (Amount, u32)>; 4] = Default::default();
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            let Some(index) = ctx.index_of(ctx.cal.date_of(secs)) else {
                continue;
            };
            let legs = tx.legs();
            let source = legs
                .iter()
                .find(|leg| leg.amount.is_negative())
                .or(legs.first())
                .map(|leg| leg.store_id.clone());
            if let Some(ids) = &ctx.scope
                && !legs.iter().any(|leg| ids.contains(&leg.store_id))
            {
                continue;
            }
            let loss = self.loss_of(&book, tx, &ctx.currency);
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
            let contact = tx.meta().contact_id.clone().or_else(|| {
                let (from, to) = sides(tx);
                [from, to]
                    .into_iter()
                    .flatten()
                    .find_map(|side| side.contact_id().map(str::to_string))
            });
            let keys = [
                Some(tx.kind().tag().to_string()),
                source,
                contact,
                tx.meta().flow_id.clone(),
            ];
            for (bucket, key) in buckets.iter_mut().zip(keys) {
                if let Some(key) = key {
                    let entry = bucket.entry(key).or_default();
                    entry.0 += fee + slippage;
                    entry.1 += 1;
                }
            }
        }
        for row in &out.rows {
            out.total.fee += row.fee;
            out.total.slippage += row.slippage;
            out.total.total += row.total;
        }
        let ranked = |bucket: &BTreeMap<String, (Amount, u32)>| {
            let mut list: Vec<LossBucket> = bucket
                .iter()
                .map(|(key, (total, count))| LossBucket {
                    key: key.clone(),
                    total: *total,
                    count: *count,
                })
                .collect();
            list.sort_by(|left, right| right.total.cmp(&left.total).then(left.key.cmp(&right.key)));
            list
        };
        out.by_kind = ranked(&buckets[0]);
        out.by_store = ranked(&buckets[1]);
        out.by_contact = ranked(&buckets[2]);
        out.by_flow = ranked(&buckets[3]);
        out
    }

    // -- chains -------------------------------------------------------------------

    /// Put transactions into one chain: the given one, else the chain one of
    /// them is already in, else a new one. Returns the chain id.
    pub fn link_transactions(&mut self, ids: &[String], flow_id: Option<&str>) -> Result<String> {
        if ids.is_empty() {
            return Err(Error::InvalidInput("choose transactions to link".into()));
        }
        let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let found = self
            .doc
            .transactions
            .iter()
            .filter(|tx| wanted.contains(tx.meta().id.as_str()))
            .count();
        if found != wanted.len() {
            return Err(Error::Missing("transaction".into()));
        }
        let flow = flow_id
            .map(str::to_string)
            .filter(|id| !id.trim().is_empty())
            .or_else(|| {
                self.doc
                    .transactions
                    .iter()
                    .filter(|tx| wanted.contains(tx.meta().id.as_str()))
                    .find_map(|tx| tx.meta().flow_id.clone())
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        for tx in self
            .doc
            .transactions
            .iter_mut()
            .filter(|tx| wanted.contains(tx.meta().id.as_str()))
        {
            tx.meta_mut().flow_id = Some(flow.clone());
        }
        self.persist()?;
        Ok(flow)
    }

    /// Take a transaction out of its chain.
    pub fn unlink_transaction(&mut self, id: &str) -> Result<()> {
        let tx = self
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        tx.meta_mut().flow_id = None;
        self.persist()
    }

    /// Every chain with a link in `[from, to)` (RFC3339 bounds; empty = open),
    /// newest first, valued in the base currency.
    pub fn list_flows(
        &self,
        from: Option<&str>,
        to: Option<&str>,
        store_ids: &[String],
    ) -> Vec<FlowSummary> {
        let book = self.rate_book();
        let base = self.base_and_fx().0;
        let lower = from.map(secs_or_now).unwrap_or(i64::MIN);
        let upper = to.map(secs_or_now).unwrap_or(i64::MAX);
        // A chain belongs to the wallets any of its links moved money in.
        let scope = (!store_ids.is_empty()).then(|| {
            self.resolve_scope(&crate::model::store::StoreScope {
                store_ids: store_ids.to_vec(),
                include_children: true,
            })
        });
        let touches_scope = |tx: &Transaction| {
            scope
                .as_ref()
                .is_none_or(|ids| tx.legs().iter().any(|leg| ids.contains(&leg.store_id)))
        };
        let mut chains: HashMap<String, Vec<&Transaction>> = HashMap::new();
        for tx in &self.doc.transactions {
            if let Some(flow) = &tx.meta().flow_id {
                chains.entry(flow.clone()).or_default().push(tx);
            }
        }
        let mut out: Vec<FlowSummary> = chains
            .into_iter()
            .filter(|(_, links)| {
                links.iter().any(|tx| {
                    let secs = secs_or_now(&tx.meta().timestamp);
                    secs >= lower && secs < upper
                }) && links.iter().any(|tx| touches_scope(tx))
            })
            .map(|(flow_id, mut links)| {
                links.sort_by_key(|tx| secs_or_now(&tx.meta().timestamp));
                self.flow_summary(&book, &base, flow_id, &links)
            })
            .collect();
        out.sort_by(|left, right| right.ended_at.cmp(&left.ended_at));
        out
    }

    fn flow_summary(
        &self,
        book: &RateBook,
        currency: &str,
        flow_id: String,
        links: &[&Transaction],
    ) -> FlowSummary {
        let side = |tx: &Transaction, outgoing: bool| -> Vec<FlowAmount> {
            let secs = secs_or_now(&tx.meta().timestamp);
            tx.legs()
                .into_iter()
                .filter(|leg| {
                    leg.role == LegRole::Principal && leg.amount.is_negative() == outgoing
                })
                .map(|leg| FlowAmount {
                    value: Self::value_at(book, &leg.asset, leg.amount.abs(), currency, secs)
                        .round(2),
                    amount: leg.amount.abs(),
                    asset: Some(leg.asset),
                })
                .collect()
        };
        let first = links.first().copied();
        let last = links.last().copied();
        let gave = first.map(|tx| side(tx, true)).unwrap_or_default();
        let got = last.map(|tx| side(tx, false)).unwrap_or_default();
        let total =
            |list: &[FlowAmount]| list.iter().fold(Amount::ZERO, |sum, item| sum + item.value);
        let links: Vec<FlowLink> = links
            .iter()
            .map(|tx| FlowLink {
                loss: self.loss_of(book, tx, currency),
                tx: (*tx).clone(),
            })
            .collect();
        let loss = links
            .iter()
            .fold(Amount::ZERO, |sum, link| sum + link.loss.total);
        let (gave_value, got_value) = (total(&gave), total(&got));
        FlowSummary {
            flow_id,
            currency: currency.to_string(),
            started_at: first
                .map(|tx| tx.meta().timestamp.clone())
                .unwrap_or_default(),
            ended_at: last
                .map(|tx| tx.meta().timestamp.clone())
                .unwrap_or_default(),
            links,
            gave,
            got,
            gave_value,
            got_value,
            loss,
            difference: gave_value - got_value,
        }
    }

    // -- rates --------------------------------------------------------------------

    /// Every cached currency against the base, latest rates (plan Ф3b, «Курсы»),
    /// plus a row per crypto favourite that has a cached price.
    /// The rate `ts` minus a day or earlier: the newest such point of `code`.
    fn fx_rate_before(&self, code: &str, limit: i64) -> Option<Amount> {
        if code == "usd" {
            return Some(Amount::new(rust_decimal::Decimal::ONE));
        }
        self.doc
            .fx_cache
            .iter()
            .filter(|point| point.currency == code)
            .filter_map(|point| {
                let secs = crate::pricing::history::unix_secs(&point.ts)?;
                (secs <= limit).then_some((secs, point.per_usd))
            })
            .max_by_key(|(secs, _)| *secs)
            .map(|(_, rate)| rate)
    }

    /// How much one unit of `code` gained in `base` against a day before `ts`.
    fn fx_change_pct(&self, code: &str, base: &str, ts: &str) -> Option<Amount> {
        const DAY_SECS: i64 = 20 * 3600;
        let limit = crate::pricing::history::unix_secs(ts)? - DAY_SECS;
        let now_code = self.per_usd(code)?;
        let now_base = self.per_usd(base)?;
        let then_code = self.fx_rate_before(code, limit)?;
        let then_base = self.fx_rate_before(base, limit)?;
        if [now_code, now_base, then_code, then_base]
            .iter()
            .any(Amount::is_zero)
        {
            return None;
        }
        let now_in_base = now_base / now_code;
        let then_in_base = then_base / then_code;
        let hundred = Amount::new(rust_decimal::Decimal::ONE_HUNDRED);
        Some(
            ((now_in_base / then_in_base - Amount::new(rust_decimal::Decimal::ONE)) * hundred)
                .round(2),
        )
    }

    pub fn fx_table(&self, favorites: &[String]) -> FxTable {
        let is_favorite = |code: &str| {
            favorites
                .iter()
                .any(|fav| fav.trim().eq_ignore_ascii_case(code))
        };
        let base = self.base_and_fx().0;
        let tracked = self.fx_currencies_in_use();
        let mut latest: BTreeMap<String, (Amount, String)> = BTreeMap::new();
        for point in &self.doc.fx_cache {
            let entry = latest
                .entry(point.currency.clone())
                .or_insert_with(|| (point.per_usd, point.ts.clone()));
            if point.ts > entry.1 {
                *entry = (point.per_usd, point.ts.clone());
            }
        }
        latest
            .entry("usd".into())
            .or_insert_with(|| (Amount::new(rust_decimal::Decimal::ONE), String::new()));
        let base_per_usd = latest.get(&base).map(|(rate, _)| *rate);
        let mut rows: Vec<FxRow> = latest
            .iter()
            .filter(|(code, _)| **code != base)
            .filter_map(|(code, (per_usd, ts))| {
                let base_rate = base_per_usd?;
                if base_rate.is_zero() || per_usd.is_zero() {
                    return None;
                }
                let per_base = *per_usd / base_rate;
                Some(FxRow {
                    currency: code.clone(),
                    per_base: per_base.round(6),
                    in_base: (Amount::new(rust_decimal::Decimal::ONE) / per_base).round(6),
                    updated_at: ts.clone(),
                    tracked: tracked.contains(code),
                    crypto: false,
                    favorite: is_favorite(code),
                    change_pct: self.fx_change_pct(code, &base, ts),
                })
            })
            .collect();
        if let Some(base_rate) = base_per_usd.filter(|rate| !rate.is_zero()) {
            let crypto = self.crypto_fx_rows(favorites, &base, base_rate, &rows);
            rows.extend(crypto);
        }
        FxTable { base, rows }
    }

    /// Rows for the crypto favourites, from the latest cached quote.
    fn crypto_fx_rows(
        &self,
        favorites: &[String],
        base: &str,
        base_per_usd: Amount,
        fiat_rows: &[FxRow],
    ) -> Vec<FxRow> {
        let book = self.rate_book();
        let now = crate::pricing::history::secs_or_now("");
        let mut rows: Vec<FxRow> = Vec::new();
        for code in favorites {
            let code = code.trim().to_lowercase();
            let taken = code == base
                || fiat_rows.iter().any(|row| row.currency == code)
                || rows.iter().any(|row| row.currency == code);
            if taken {
                continue;
            }
            let Some(asset) = self.favorite_asset(&code) else {
                continue;
            };
            let Some((usd, at)) = book.quote_at(&asset.key(), now) else {
                continue;
            };
            let in_base = usd * base_per_usd;
            if in_base.is_zero() {
                continue;
            }
            rows.push(FxRow {
                // A coin is worth thousands of base units: keep enough digits.
                per_base: (Amount::new(rust_decimal::Decimal::ONE) / in_base).round(12),
                in_base: in_base.round(6),
                updated_at: at.map(super::rates::iso_of).unwrap_or_default(),
                tracked: self.held_coin(&code).is_some(),
                crypto: true,
                favorite: true,
                change_pct: None,
                currency: code,
            });
        }
        rows
    }
}

/// The two sides of a movement, when it has them.
fn sides(tx: &Transaction) -> (Option<&AccountRef>, Option<&AccountRef>) {
    match tx {
        Transaction::Transfer(inner) => (Some(&inner.from), Some(&inner.to)),
        Transaction::Convert(inner) => (Some(&inner.from.account), Some(&inner.to.account)),
        Transaction::Topup(inner) => (inner.from.as_ref(), None),
        Transaction::Payment(inner) => (None, inner.to.as_ref()),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::store::StoreKind;
    use crate::model::transaction::{ConvertSide, ConvertTx, PriceStamp, TxMeta};
    use crate::model::vault::{FxPoint, PricePoint};
    use crate::service::test_support::*;

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "Ruble")
    }

    fn thb() -> AssetRef {
        AssetRef::fiat("THB", "Baht")
    }

    fn usdt() -> AssetRef {
        AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Tron)
    }

    fn convert(
        from: (&str, AssetRef, &str),
        to: (&str, AssetRef, &str),
        rate: Option<&str>,
        ts: &str,
    ) -> Transaction {
        let side = |(store, asset, sum): (&str, AssetRef, &str)| ConvertSide {
            account: AccountRef::Store(store.into()),
            asset,
            amount: amount(sum),
            price: PriceStamp::default(),
        };
        let mut meta = TxMeta::new("exchange");
        meta.timestamp = ts.into();
        Transaction::Convert(ConvertTx {
            meta,
            from: side(from),
            to: side(to),
            fee: None,
            rate_override: rate.map(amount),
        })
    }

    /// USD base; 100 RUB and 35 THB per USD; USDT at 1.
    fn setup() -> (tempfile::TempDir, UnlockedVault, String, String, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        let card = store(&mut vault, "RUB card", StoreKind::Bank, None);
        let wallet = store(&mut vault, "Wallet", StoreKind::Crypto, None);
        let baht = store(&mut vault, "THB card", StoreKind::Bank, None);
        let mut seed = income_of(&card, rub(), "20000", "0.01", "salary");
        seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
        vault.add_transaction(seed).unwrap();
        vault.apply_fx(&[
            FxPoint {
                currency: "rub".into(),
                per_usd: amount("100"),
                ts: "2026-01-01T00:00:00Z".into(),
                source: "test".into(),
            },
            FxPoint {
                currency: "thb".into(),
                per_usd: amount("35"),
                ts: "2026-01-01T00:00:00Z".into(),
                source: "test".into(),
            },
        ]);
        vault.apply_prices(&[PricePoint {
            asset_key: usdt().key(),
            value_usd: amount("1"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        }]);
        (dir, vault, card, wallet, baht)
    }

    #[test]
    fn a_convert_at_a_worse_rate_than_the_market_loses_the_spread() {
        let (_dir, mut vault, card, wallet, _) = setup();
        let saved = vault
            .add_transaction(convert(
                (&card, rub(), "10000"),
                (&wallet, usdt(), "95"),
                Some("105.26"),
                "2026-02-01T10:00:00Z",
            ))
            .unwrap();
        let loss = vault.tx_loss(&saved.meta().id).unwrap();
        assert_eq!(loss.slippage, amount("5"));
        assert_eq!(loss.total, amount("5"));
        assert!(loss.complete);
        assert!(!loss.internal);
    }

    #[test]
    fn the_preview_of_a_convert_shows_the_overpay_over_the_market() {
        let (_dir, vault, card, wallet, _) = setup();
        let draft = convert(
            (&card, rub(), "10000"),
            (&wallet, usdt(), "95"),
            None,
            "2026-02-01T10:00:00Z",
        );
        let preview = vault.convert_preview(&draft).unwrap();
        assert_eq!(preview.market_price, amount("100"));
        assert_eq!(preview.overpay, amount("500"));
        assert_eq!(preview.overpay_base, amount("5"));
        assert_eq!(preview.overpay_pct, amount("5"));
    }

    #[test]
    fn an_internal_convert_still_counts_its_spread() {
        let (_dir, mut vault, card, _, _) = setup();
        let saved = vault
            .add_transaction(convert(
                (&card, rub(), "10000"),
                (&card, AssetRef::fiat("USD", "Dollar"), "98"),
                None,
                "2026-02-01T10:00:00Z",
            ))
            .unwrap();
        let loss = vault.tx_loss(&saved.meta().id).unwrap();
        assert!(loss.internal);
        assert_eq!((loss.fee, loss.slippage), (Amount::ZERO, amount("2")));
    }

    #[test]
    fn a_rub_usdt_thb_chain_adds_up_its_links() {
        let (_dir, mut vault, card, wallet, baht) = setup();
        let first = vault
            .add_transaction(convert(
                (&card, rub(), "10000"),
                (&wallet, usdt(), "98"),
                None,
                "2026-02-01T10:00:00Z",
            ))
            .unwrap();
        let second = vault
            .add_transaction(convert(
                (&wallet, usdt(), "98"),
                (&baht, thb(), "3400"),
                None,
                "2026-02-01T12:00:00Z",
            ))
            .unwrap();
        let flow = vault
            .link_transactions(&[first.meta().id.clone(), second.meta().id.clone()], None)
            .unwrap();
        let flows = vault.list_flows(None, None, &[]);
        assert_eq!(flows.len(), 1);
        let chain = &flows[0];
        assert_eq!(chain.flow_id, flow);
        assert_eq!(chain.links.len(), 2);
        assert_eq!(chain.links[0].loss.total, amount("2"));
        assert_eq!(chain.links[1].loss.total, amount("0.86"));
        assert_eq!(chain.gave_value, amount("100"));
        assert_eq!(chain.got_value, amount("97.14"));
        assert_eq!(chain.loss, amount("2.86"));
        assert_eq!(chain.difference, amount("2.86"));

        vault.unlink_transaction(&second.meta().id).unwrap();
        assert_eq!(vault.list_flows(None, None, &[])[0].links.len(), 1);
        // Another wallet's chains stay out; one of the chain's own wallets keeps it.
        assert!(
            vault
                .list_flows(None, None, std::slice::from_ref(&baht))
                .is_empty()
        );
        assert_eq!(
            vault
                .list_flows(None, None, std::slice::from_ref(&card))
                .len(),
            1
        );
    }

    #[test]
    fn the_rates_table_is_against_the_base() {
        let (_dir, mut vault, ..) = setup();
        vault.set_base_currency("rub");
        let table = vault.fx_table(&[]);
        assert_eq!(table.base, "rub");
        let usd = table.rows.iter().find(|row| row.currency == "usd").unwrap();
        assert_eq!(usd.in_base, amount("100"));
        let thb = table.rows.iter().find(|row| row.currency == "thb").unwrap();
        assert_eq!(thb.per_base, amount("0.35"));
        assert!(!thb.crypto && !thb.favorite);
    }

    #[test]
    fn crypto_favorites_join_the_rates_table() {
        let (_dir, mut vault, ..) = setup();
        vault.set_base_currency("rub");
        vault.apply_prices(&[PricePoint {
            asset_key: "coin:bitcoin".into(),
            value_usd: amount("50000"),
            ts: "2026-01-02T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        }]);
        let favorites = ["BTC".to_string(), "THB".to_string()];
        let table = vault.fx_table(&favorites);
        let btc = table.rows.iter().find(|row| row.currency == "btc").unwrap();
        assert!(btc.crypto && btc.favorite);
        assert_eq!(btc.in_base, amount("5000000"));
        assert_eq!(btc.per_base, amount("0.0000002"));
        assert!(btc.updated_at.starts_with("2026-01-02"));
        let thb = table.rows.iter().find(|row| row.currency == "thb").unwrap();
        assert!(thb.favorite && !thb.crypto);
    }

    #[test]
    fn an_unpriced_favorite_has_no_row() {
        let (_dir, vault, ..) = setup();
        let table = vault.fx_table(&["DOGE".to_string()]);
        assert!(table.rows.iter().all(|row| row.currency != "doge"));
    }
}
