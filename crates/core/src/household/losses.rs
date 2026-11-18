//! Losses, chains and the rates table (plan §6.3–§6.4).
//!
//! A transaction loses money two ways: the fee, and the spread between what
//! was given and what came back, both valued at the **market** rate of that
//! moment — never at the rate the user typed (`rate_override`), since that
//! rate is exactly what is being judged. A chain (`flow_id`) strings
//! operations together — RUB → USDT through an exchanger, USDT → THB — to see
//! what went in, what came out and what was lost on the way.
//!
//! Pure part of the area: it reads a document. The rates come in through
//! `Valuation` and `FxSources` (the rates area keeps them, OD6). The month
//! report and the chain writes are scenarios in `usecases::household::losses`.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::store::{AssetRef, StoreScope};
use crate::model::transaction::{AccountRef, Leg, LegRole, Transaction};
use crate::model::vault::{VaultDoc, unix_to_iso};
use crate::money::tree::{resolve_scope, root_of};
use crate::pricing::history::{secs_or_now, unix_secs};

/// What the rates book answers: the value of a leg or a quantity in a
/// currency at a moment.
pub trait Valuation {
    /// Value of one leg in `currency` at `secs`; `None` without a rate.
    fn leg_value_in(&self, leg: &Leg, currency: &str, secs: i64) -> Option<Amount>;
    /// Value of a quantity in `currency` at `secs`; zero without a rate.
    fn value_at(&self, asset: &AssetRef, qty: Amount, currency: &str, secs: i64) -> Amount;
}

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

/// A coin from the favourites with its latest cached quote.
pub struct CryptoQuote {
    pub usd: Amount,
    /// When the quote was taken, if the book knows.
    pub taken_at: Option<i64>,
    /// The vault holds this coin.
    pub held: bool,
}

/// What the rates table asks of the rates area.
pub trait FxSources {
    /// Units of `currency` per USD from the FX cache; USD itself is 1.
    fn per_usd(&self, currency: &str) -> Option<Amount>;
    /// Currencies whose rate history matters.
    fn tracked(&self) -> HashSet<String>;
    /// The latest quote of a crypto favourite; `None` for fiat and unknowns.
    fn crypto_quote(&self, code: &str) -> Option<CryptoQuote>;
}

fn one() -> Amount {
    Amount::new(rust_decimal::Decimal::ONE)
}

/// The loss of one transaction in `currency` (plan §6.3).
pub fn loss_of(
    doc: &VaultDoc,
    values: &impl Valuation,
    tx: &Transaction,
    currency: &str,
) -> TxLoss {
    let secs = secs_or_now(&tx.meta().timestamp);
    let mut complete = true;
    let mut fee = Amount::ZERO;
    for leg in tx.legs().into_iter().filter(|leg| leg.role == LegRole::Fee) {
        match values.leg_value_in(&leg, currency, secs) {
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
        fee = values.value_at(&paid.asset, paid.amount.abs(), currency, secs);
    }
    let mut priced = true;
    let mut market = |asset: &AssetRef, qty: Amount| {
        let value = values.value_at(asset, qty.abs(), currency, secs);
        if value.is_zero() && !qty.is_zero() {
            priced = false;
        }
        value
    };
    let spread = match tx {
        Transaction::Convert(inner) => {
            market(&inner.from.asset, inner.from.amount) - market(&inner.to.asset, inner.to.amount)
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
    // A side without a price is no evidence of a spread (L5): the spread counts
    // only when every side was priced; `complete` says so.
    let slippage = if priced { spread } else { Amount::ZERO };
    complete &= priced;
    let fee = fee.round(2);
    let slippage = slippage.round(2);
    TxLoss {
        tx_id: tx.meta().id.clone(),
        currency: currency.to_string(),
        fee,
        slippage,
        total: fee + slippage,
        internal: tx.is_internal(|id| root_of(doc, id)),
        complete,
    }
}

/// The loss of one transaction in `currency`.
pub fn loss_by_id(
    doc: &VaultDoc,
    values: &impl Valuation,
    currency: &str,
    id: &str,
) -> Result<TxLoss> {
    let tx = doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == id)
        .ok_or_else(|| Error::Missing("transaction".into()))?;
    Ok(loss_of(doc, values, tx, currency))
}

/// Losses of every transaction that has one, in `currency`, by id — for the
/// loss column of the list.
pub fn losses_by_id(
    doc: &VaultDoc,
    values: &impl Valuation,
    currency: &str,
) -> BTreeMap<String, TxLoss> {
    doc.transactions
        .iter()
        .map(|tx| loss_of(doc, values, tx, currency))
        .filter(|loss| !loss.total.is_zero() || !loss.complete)
        .map(|loss| (loss.tx_id.clone(), loss))
        .collect()
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

/// Some leg of the transaction moves money in one of the wallets `ids`.
pub fn touches_any(tx: &Transaction, ids: &HashSet<String>) -> bool {
    tx.legs().iter().any(|leg| ids.contains(&leg.store_id))
}

/// Losses summed by one key each: the kind, the source wallet, the contact
/// and the chain of a transaction.
#[derive(Default)]
pub struct LossBuckets {
    by_key: [BTreeMap<String, (Amount, u32)>; 4],
}

impl LossBuckets {
    /// Count `amount` (fee plus slippage as the report shows it) under every
    /// key the transaction has.
    pub fn add(&mut self, tx: &Transaction, amount: Amount) {
        let legs = tx.legs();
        let source = legs
            .iter()
            .find(|leg| leg.amount.is_negative())
            .or(legs.first())
            .map(|leg| leg.store_id.clone());
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
        for (bucket, key) in self.by_key.iter_mut().zip(keys) {
            if let Some(key) = key {
                let entry = bucket.entry(key).or_default();
                entry.0 += amount;
                entry.1 += 1;
            }
        }
    }

    /// The buckets of kind, wallet, contact and chain, biggest loss first.
    pub fn ranked(&self) -> [Vec<LossBucket>; 4] {
        self.by_key.each_ref().map(|bucket| {
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
        })
    }
}

/// The chain the transactions `ids` go into: the given one, else the chain
/// one of them is already in, else a new one. A refusal when none is chosen
/// or one is unknown.
pub fn plan_link(doc: &VaultDoc, ids: &[String], flow_id: Option<&str>) -> Result<String> {
    if ids.is_empty() {
        return Err(Error::InvalidInput("choose transactions to link".into()));
    }
    let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let chosen: Vec<&Transaction> = doc
        .transactions
        .iter()
        .filter(|tx| wanted.contains(tx.meta().id.as_str()))
        .collect();
    if chosen.len() != wanted.len() {
        return Err(Error::Missing("transaction".into()));
    }
    Ok(flow_id
        .map(str::to_string)
        .filter(|id| !id.trim().is_empty())
        .or_else(|| chosen.iter().find_map(|tx| tx.meta().flow_id.clone()))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
}

/// Every chain with a link in `[from, to)` (RFC3339 bounds; empty = open),
/// newest first, valued in `currency`.
pub fn chain_summaries(
    doc: &VaultDoc,
    values: &impl Valuation,
    currency: &str,
    from: Option<&str>,
    to: Option<&str>,
    store_ids: &[String],
) -> Vec<FlowSummary> {
    let lower = from.map(secs_or_now).unwrap_or(i64::MIN);
    let upper = to.map(secs_or_now).unwrap_or(i64::MAX);
    // A chain belongs to the wallets any of its links moved money in.
    let scope = (!store_ids.is_empty()).then(|| {
        resolve_scope(
            doc,
            &StoreScope {
                store_ids: store_ids.to_vec(),
                include_children: true,
            },
        )
    });
    let touches_scope = |tx: &Transaction| scope.as_ref().is_none_or(|ids| touches_any(tx, ids));
    let mut chains: HashMap<String, Vec<&Transaction>> = HashMap::new();
    for tx in &doc.transactions {
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
            flow_summary(doc, values, currency, flow_id, &links)
        })
        .collect();
    out.sort_by(|left, right| right.ended_at.cmp(&left.ended_at));
    out
}

fn flow_summary(
    doc: &VaultDoc,
    values: &impl Valuation,
    currency: &str,
    flow_id: String,
    links: &[&Transaction],
) -> FlowSummary {
    let side = |tx: &Transaction, outgoing: bool| -> Vec<FlowAmount> {
        let secs = secs_or_now(&tx.meta().timestamp);
        tx.legs()
            .into_iter()
            .filter(|leg| leg.role == LegRole::Principal && leg.amount.is_negative() == outgoing)
            .map(|leg| FlowAmount {
                value: values
                    .value_at(&leg.asset, leg.amount.abs(), currency, secs)
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
    let total = |list: &[FlowAmount]| list.iter().fold(Amount::ZERO, |sum, item| sum + item.value);
    let links: Vec<FlowLink> = links
        .iter()
        .map(|tx| FlowLink {
            loss: loss_of(doc, values, tx, currency),
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

/// The rate `ts` minus a day or earlier: the newest such point of `code`.
fn fx_rate_before(doc: &VaultDoc, code: &str, limit: i64) -> Option<Amount> {
    if code == "usd" {
        return Some(one());
    }
    doc.fx_cache
        .iter()
        .filter(|point| point.currency == code)
        .filter_map(|point| {
            let secs = unix_secs(&point.ts)?;
            (secs <= limit).then_some((secs, point.per_usd))
        })
        .max_by_key(|(secs, _)| *secs)
        .map(|(_, rate)| rate)
}

/// How much one unit of `code` gained in `base` against a day before `ts`.
fn fx_change_pct(
    doc: &VaultDoc,
    sources: &impl FxSources,
    code: &str,
    base: &str,
    ts: &str,
) -> Option<Amount> {
    const DAY_SECS: i64 = 20 * 3600;
    let limit = unix_secs(ts)? - DAY_SECS;
    let now_code = sources.per_usd(code)?;
    let now_base = sources.per_usd(base)?;
    let then_code = fx_rate_before(doc, code, limit)?;
    let then_base = fx_rate_before(doc, base, limit)?;
    if [now_code, now_base, then_code, then_base]
        .iter()
        .any(Amount::is_zero)
    {
        return None;
    }
    let now_in_base = now_base / now_code;
    let then_in_base = then_base / then_code;
    let hundred = Amount::new(rust_decimal::Decimal::ONE_HUNDRED);
    Some(((now_in_base / then_in_base - one()) * hundred).round(2))
}

/// Every cached currency against `base`, latest rates (plan F3b, "Rates"),
/// plus a row per crypto favourite that has a cached price.
pub fn rates_table(
    doc: &VaultDoc,
    sources: &impl FxSources,
    base: &str,
    favorites: &[String],
) -> FxTable {
    let is_favorite = |code: &str| {
        favorites
            .iter()
            .any(|fav| fav.trim().eq_ignore_ascii_case(code))
    };
    let tracked = sources.tracked();
    let mut latest: BTreeMap<String, (Amount, String)> = BTreeMap::new();
    for point in &doc.fx_cache {
        let entry = latest
            .entry(point.currency.clone())
            .or_insert_with(|| (point.per_usd, point.ts.clone()));
        if point.ts > entry.1 {
            *entry = (point.per_usd, point.ts.clone());
        }
    }
    latest
        .entry("usd".into())
        .or_insert_with(|| (one(), String::new()));
    let base_per_usd = latest.get(base).map(|(rate, _)| *rate);
    let mut rows: Vec<FxRow> = latest
        .iter()
        .filter(|(code, _)| code.as_str() != base)
        .filter_map(|(code, (per_usd, ts))| {
            let base_rate = base_per_usd?;
            if base_rate.is_zero() || per_usd.is_zero() {
                return None;
            }
            let per_base = *per_usd / base_rate;
            Some(FxRow {
                currency: code.clone(),
                per_base: per_base.round(6),
                in_base: (one() / per_base).round(6),
                updated_at: ts.clone(),
                tracked: tracked.contains(code),
                crypto: false,
                favorite: is_favorite(code),
                change_pct: fx_change_pct(doc, sources, code, base, ts),
            })
        })
        .collect();
    if let Some(base_rate) = base_per_usd.filter(|rate| !rate.is_zero()) {
        let crypto = crypto_fx_rows(sources, favorites, base, base_rate, &rows);
        rows.extend(crypto);
    }
    FxTable {
        base: base.to_string(),
        rows,
    }
}

/// Rows for the crypto favourites, from the latest cached quote.
fn crypto_fx_rows(
    sources: &impl FxSources,
    favorites: &[String],
    base: &str,
    base_per_usd: Amount,
    fiat_rows: &[FxRow],
) -> Vec<FxRow> {
    let mut rows: Vec<FxRow> = Vec::new();
    for code in favorites {
        let code = code.trim().to_lowercase();
        let taken = code == base
            || fiat_rows.iter().any(|row| row.currency == code)
            || rows.iter().any(|row| row.currency == code);
        if taken {
            continue;
        }
        let Some(quote) = sources.crypto_quote(&code) else {
            continue;
        };
        let in_base = quote.usd * base_per_usd;
        if in_base.is_zero() {
            continue;
        }
        rows.push(FxRow {
            // A coin is worth thousands of base units: keep enough digits.
            per_base: (one() / in_base).round(12),
            in_base: in_base.round(6),
            updated_at: quote.taken_at.map(unix_to_iso).unwrap_or_default(),
            tracked: quote.held,
            crypto: true,
            favorite: true,
            change_pct: None,
            currency: code,
        });
    }
    rows
}
