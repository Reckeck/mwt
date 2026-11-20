//! Property value over time (plan G3): dated valuations joined by straight
//! lines, and after the last one an automatic rule — a yearly percent applied
//! month by month, held at a floor when it depreciates.
//!
//! An object nobody revalued and gave no rule keeps the old behaviour: its
//! hand-set price (`manual_prices`) holds for the whole history. Once it has
//! a valuation or a rule, that old price becomes the point at the purchase
//! date, so the history before the first valuation does not change.
//! Everything here reads the document; the scenarios that write it live in
//! `usecases::money::valuation`.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::ids::StoreId;
use crate::model::store::{PropertyCategory, RevalueFloor, RevalueRule, Store, StoreKind};
use crate::model::transaction::Transaction;
use crate::model::vault::{Valuation, VaultDoc};
use crate::money::inflation::{inflation_on, inflation_series, real_coefficients};
use crate::money::price_info::last_stamp;
use crate::money::rates::per_usd;
use crate::money::tree::find_store;
use crate::pricing::history::secs_or_now;

/// Average month, for the rule's position inside a month.
pub(super) const MONTH_SECS: i64 = 2_629_746;
/// The rule is not run further than this many months ahead.
pub(super) const MAX_MONTHS: i64 = 1200;

/// The value of one unit of a property asset at any moment, in `currency`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValuationCurve {
    pub currency: String,
    /// `(unix secs, unit price)`, oldest first.
    pub(super) points: Vec<(i64, Amount)>,
    pub(super) rule: Option<CurveRule>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CurveRule {
    /// Growth over one month: `(1 + pct/100)^(1/12)`.
    pub(super) monthly: Amount,
    pub(super) floor: Option<Amount>,
}

impl ValuationCurve {
    /// The unit price at `secs`: the first point before it, a straight line
    /// between two points, the rule after the last one.
    pub fn value_at(&self, secs: i64) -> Option<Amount> {
        let (first_at, first) = *self.points.first()?;
        if secs <= first_at {
            return Some(first);
        }
        for pair in self.points.windows(2) {
            let ((from_at, from), (to_at, to)) = (pair[0], pair[1]);
            if secs <= to_at {
                if to_at == from_at {
                    return Some(to);
                }
                let share = secs_amount(secs - from_at) / secs_amount(to_at - from_at);
                return Some(from + (to - from) * share);
            }
        }
        let (last_at, last) = *self.points.last()?;
        let Some(rule) = &self.rule else {
            return Some(last);
        };
        let elapsed = (secs - last_at).min(MAX_MONTHS * MONTH_SECS);
        let whole = elapsed / MONTH_SECS;
        let growth = rule.monthly.pow_int(u32::try_from(whole).unwrap_or(0));
        let start = last.checked_mul(growth).unwrap_or(Amount::MAX);
        let end = start * rule.monthly;
        let share = secs_amount(elapsed - whole * MONTH_SECS) / secs_amount(MONTH_SECS);
        let value = start + (end - start) * share;
        // A valuation already below the floor is the user's word: the rule
        // does not lift it, it only stops lowering it.
        Some(match rule.floor {
            Some(floor) => value.max(floor.min(last)),
            None => value,
        })
    }
}

/// Where the first point of a curve comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ValuationBaseKind {
    /// The hand-set price from before valuations had dates.
    Manual,
    /// The price of the purchase.
    Purchase,
}

/// The purchase of one property asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PropertyPurchase {
    /// YYYY-MM-DD.
    pub date: String,
    pub unit_price: Amount,
    pub currency: String,
    /// The first "Acquire" operation, else the old form fields.
    pub from_operation: bool,
}

/// Everything the value of one asset of a property shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PropertyValue {
    pub store_id: String,
    pub asset_key: String,
    pub symbol: String,
    pub quantity: Amount,
    /// Currency of the prices below.
    pub currency: String,
    /// Unit price today; `None` when nothing priced it yet.
    pub unit_price: Option<Amount>,
    pub purchase: Option<PropertyPurchase>,
    /// The first point when it is not a valuation of its own.
    pub base: Option<ValuationBaseKind>,
    pub base_date: Option<String>,
    pub base_price: Option<Amount>,
    pub valuations: Vec<Valuation>,
    pub rule: Option<RevalueRule>,
    /// Today's value against the purchase, per unit.
    pub change: Option<Amount>,
    pub change_pct: Option<Amount>,
    /// The same against the purchase price grown by inflation since.
    pub real_change_pct: Option<Amount>,
}

/// A new dated valuation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ValuationInput {
    pub store_id: String,
    pub asset_key: String,
    /// YYYY-MM-DD.
    pub date: String,
    pub unit_price: Amount,
    pub currency: String,
}

fn date_secs(date: &str) -> Option<i64> {
    let day = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    Some(day.and_hms_opt(0, 0, 0)?.and_utc().timestamp())
}

fn secs_amount(secs: i64) -> Amount {
    Amount::new(rust_decimal::Decimal::from(secs))
}

pub(super) fn hundred() -> Amount {
    Amount::from(100)
}

/// Check a rule before it is saved: depreciation needs somewhere to stop.
fn check_rule(rule: &RevalueRule) -> Result<()> {
    let invalid = |text: &str| Err(Error::InvalidInput(text.into()));
    if let Some(pct) = rule.pct_per_year {
        if pct <= -hundred() {
            return invalid("the yearly change must be above -100%");
        }
        if pct.is_negative() && rule.floor.is_none() {
            return invalid("depreciation needs a floor");
        }
    }
    match rule.floor {
        Some(RevalueFloor::Amount { amount }) if !amount.is_positive() => {
            invalid("the floor must be positive")
        }
        Some(RevalueFloor::PctOfPurchase { pct }) if !pct.is_positive() || pct > hundred() => {
            invalid("the floor must be between 0 and 100% of the purchase price")
        }
        _ => Ok(()),
    }
}

/// Whether any curve exists at all: the fast path keeps vaults without
/// valuations and rules exactly as they were.
pub(crate) fn has_curves(doc: &VaultDoc) -> bool {
    !doc.valuations.is_empty()
        || doc.stores.iter().any(|store| {
            store
                .property
                .as_ref()
                .is_some_and(|info| info.revalue.is_some())
        })
}

/// Curves of every property asset that has a valuation or a rule.
pub(crate) fn valuation_curves(books: &Books<'_>) -> HashMap<String, ValuationCurve> {
    let doc = books.doc();
    let mut out = HashMap::new();
    if !has_curves(doc) {
        return out;
    }
    for store in doc
        .stores
        .iter()
        .filter(|store| store.kind == StoreKind::Property)
    {
        for key in property_assets(doc, store) {
            if out.contains_key(&key) {
                continue;
            }
            if let Some(curve) = curve_in(books, store, &key) {
                out.insert(key, curve);
            }
        }
    }
    out
}

/// The assets a property held at any time, in the order they appear: the
/// holdings, then what its valuations and operations name. A sold asset is
/// no holding any more, but the months it was held keep their curve (`L9`).
fn property_assets(doc: &VaultDoc, store: &Store) -> Vec<String> {
    let held = doc
        .holdings
        .iter()
        .filter(|row| row.store_id == store.id)
        .map(|row| row.asset.key());
    let valued = doc
        .valuations
        .iter()
        .filter(|row| row.store_id == store.id)
        .map(|row| row.asset_key.clone());
    let traded = doc
        .transactions
        .iter()
        .flat_map(Transaction::legs)
        .filter(|leg| leg.store_id == store.id && !leg.asset.is_fiat())
        .map(|leg| leg.asset.key());
    let mut keys: Vec<String> = Vec::new();
    for key in held.chain(valued).chain(traded) {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// The curve of one asset, if it has one.
pub(crate) fn valuation_curve(books: &Books<'_>, asset_key: &str) -> Option<ValuationCurve> {
    let doc = books.doc();
    if !has_curves(doc) {
        return None;
    }
    let store = property_of(doc, asset_key)?;
    curve_in(books, store, asset_key)
}

pub(crate) fn property_of<'doc>(doc: &'doc VaultDoc, asset_key: &str) -> Option<&'doc Store> {
    doc.holdings.iter().find_map(|holding| {
        (holding.asset.key() == asset_key)
            .then(|| find_store(doc, &holding.store_id))
            .flatten()
            .filter(|store| store.kind == StoreKind::Property)
    })
}

pub(crate) fn curve_in(
    books: &Books<'_>,
    store: &Store,
    asset_key: &str,
) -> Option<ValuationCurve> {
    let doc = books.doc();
    let info = store.property.as_ref();
    let rule = info.and_then(|info| info.revalue.as_ref());
    let mut dated: Vec<&Valuation> = doc
        .valuations
        .iter()
        .filter(|row| row.asset_key == asset_key)
        .collect();
    if dated.is_empty() && rule.is_none() {
        return None;
    }
    dated.sort_by(|left, right| left.date.cmp(&right.date));
    let purchase = purchase_of(books, store, asset_key);
    let base = base_point(doc, asset_key, purchase.as_ref());
    let currency = dated
        .last()
        .map(|row| row.currency.clone())
        .or_else(|| base.as_ref().map(|(_, _, currency, _)| currency.clone()))?;
    let mut points: Vec<(i64, Amount)> = Vec::new();
    let first_dated = dated.first().and_then(|row| date_secs(&row.date));
    if let Some((at, price, from, _)) = &base {
        // The base point sits at the purchase; a valuation dated earlier
        // than that wins.
        if first_dated.is_none_or(|first| *at < first) {
            points.push((*at, latest_convert(doc, *price, from, &currency)?));
        }
    }
    for row in &dated {
        let Some(at) = date_secs(&row.date) else {
            continue;
        };
        let price = latest_convert(doc, row.unit_price, &row.currency, &currency)?;
        match points.last_mut() {
            Some(last) if last.0 == at => last.1 = price,
            _ => points.push((at, price)),
        }
    }
    if points.is_empty() {
        return None;
    }
    let rule = rule.map(|rule| {
        let pct = rule
            .pct_per_year
            .or_else(|| {
                inflation_series(doc, &currency)
                    .last()
                    .map(|row| row.yoy_pct)
            })
            .unwrap_or_default();
        let floor = match rule.floor {
            Some(RevalueFloor::Amount { amount }) => {
                let from = info
                    .map(|info| info.currency.trim().to_lowercase())
                    .filter(|code| !code.is_empty())
                    .unwrap_or_else(|| currency.clone());
                latest_convert(doc, amount, &from, &currency)
            }
            Some(RevalueFloor::PctOfPurchase { pct }) => purchase.as_ref().and_then(|row| {
                latest_convert(doc, row.unit_price, &row.currency, &currency)
                    .map(|price| price * pct / hundred())
            }),
            None => None,
        };
        CurveRule {
            monthly: (Amount::from(1) + pct / hundred()).root_n(12),
            floor,
        }
    });
    Some(ValuationCurve {
        currency,
        points,
        rule,
    })
}

/// `amount` of `from` in `to` at the latest rates.
pub(crate) fn latest_convert(
    doc: &VaultDoc,
    amount: Amount,
    from: &str,
    to: &str,
) -> Option<Amount> {
    let (from, to) = (from.trim().to_lowercase(), to.trim().to_lowercase());
    if from == to {
        return Some(amount);
    }
    let from_rate = per_usd(doc, &from).filter(|rate| !rate.is_zero())?;
    Some(amount / from_rate * per_usd(doc, &to)?)
}

/// The first point of a curve: the old hand-set price at the purchase,
/// else the purchase itself. `(secs, price, currency, kind)`.
pub(crate) fn base_point(
    doc: &VaultDoc,
    asset_key: &str,
    purchase: Option<&PropertyPurchase>,
) -> Option<(i64, Amount, String, ValuationBaseKind)> {
    let at = purchase
        .and_then(|row| date_secs(&row.date))
        .unwrap_or_default();
    if let Some(manual) = doc
        .manual_prices
        .iter()
        .find(|price| price.asset_key == asset_key)
    {
        return Some((
            at,
            manual.unit_price,
            manual.currency.clone(),
            ValuationBaseKind::Manual,
        ));
    }
    let purchase = purchase?;
    Some((
        at,
        purchase.unit_price,
        purchase.currency.clone(),
        ValuationBaseKind::Purchase,
    ))
}

/// The purchase of an asset: the first "Acquire" into the property
/// (its unit price is in the currency its label names, else the base of
/// the reader), else the old
/// purchase fields of the form.
pub(crate) fn purchase_of(
    books: &Books<'_>,
    store: &Store,
    asset_key: &str,
) -> Option<PropertyPurchase> {
    let first = books
        .doc()
        .transactions
        .iter()
        .filter_map(|tx| match tx {
            Transaction::Topup(inner)
                if inner.store_id == store.id && inner.asset.key() == asset_key =>
            {
                // A base price is in the currency its author labelled it with.
                let (price, label) = match inner.price.price_base {
                    Some(price) => (price, inner.price.price_base_currency.as_deref()),
                    None => (inner.details.unit_price?, None),
                };
                Some((inner.meta.timestamp.as_str(), price, label))
            }
            _ => None,
        })
        .min_by(|left, right| left.0.cmp(right.0));
    if let Some((timestamp, unit_price, label)) = first {
        let date = calendar::Calendar::new("UTC").date_of(secs_or_now(timestamp));
        return Some(PropertyPurchase {
            date: date.format("%Y-%m-%d").to_string(),
            unit_price,
            currency: label
                .map(|code| code.trim().to_lowercase())
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| books.base_and_fx().0.to_string()),
            from_operation: true,
        });
    }
    let info = store.property.as_ref()?;
    let unit_price = info.purchase_price.filter(|price| price.is_positive())?;
    let currency = info.currency.trim().to_lowercase();
    Some(PropertyPurchase {
        date: info.purchased_at.clone().unwrap_or_default(),
        unit_price,
        currency: if currency.is_empty() {
            books.base_and_fx().0.to_string()
        } else {
            currency
        },
        from_operation: false,
    })
}

/// The value story of each asset a property holds.
pub fn property_values(
    books: &Books<'_>,
    store_id: &StoreId,
    zone: &str,
) -> Result<Vec<PropertyValue>> {
    let doc = books.doc();
    let store = property_store(doc, store_id)?;
    let now = secs_or_now("");
    let mut out = Vec::new();
    for holding in doc.holdings.iter().filter(|row| row.store_id == store.id) {
        let key = holding.asset.key();
        let curve = curve_in(books, store, &key);
        let purchase = purchase_of(books, store, &key);
        let base = base_point(doc, &key, purchase.as_ref());
        let currency = curve
            .as_ref()
            .map(|curve| curve.currency.clone())
            .or_else(|| base.as_ref().map(|row| row.2.clone()))
            .unwrap_or_else(|| books.base_and_fx().0.to_string());
        let unit_price = match &curve {
            Some(curve) => curve.value_at(now),
            None => unpriced_unit(books, &key, &currency),
        };
        let mut valuations: Vec<Valuation> = doc
            .valuations
            .iter()
            .filter(|row| row.asset_key == key)
            .cloned()
            .collect();
        valuations.sort_by(|left, right| left.date.cmp(&right.date));
        let paid = purchase
            .as_ref()
            .and_then(|row| latest_convert(doc, row.unit_price, &row.currency, &currency));
        let change = unit_price.zip(paid).map(|(now, paid)| now - paid);
        let change_pct = change
            .zip(paid)
            .filter(|(_, paid)| !paid.is_zero())
            .map(|(change, paid)| change / paid * hundred());
        let real_change_pct =
            purchase
                .as_ref()
                .zip(unit_price)
                .zip(paid)
                .and_then(|((row, now), paid)| {
                    let grown = paid * inflation_since(doc, &currency, &row.date, zone)?;
                    (!grown.is_zero()).then(|| (now / grown - Amount::from(1)) * hundred())
                });
        let own_base = base.filter(|(at, ..)| {
            valuations
                .first()
                .and_then(|row| date_secs(&row.date))
                .is_none_or(|first| *at < first)
        });
        out.push(PropertyValue {
            store_id: store.id.clone(),
            asset_key: key,
            symbol: holding.asset.symbol.clone(),
            quantity: holding.quantity,
            currency,
            unit_price: unit_price.map(|price| price.round(2)),
            purchase,
            base: own_base.as_ref().map(|row| row.3),
            base_date: own_base.as_ref().and_then(|row| {
                let date = chrono::DateTime::from_timestamp(row.0, 0)?.date_naive();
                (row.0 != 0).then(|| date.format("%Y-%m-%d").to_string())
            }),
            base_price: own_base.as_ref().map(|row| row.1),
            valuations,
            rule: store
                .property
                .as_ref()
                .and_then(|info| info.revalue.clone()),
            change: change.map(|value| value.round(2)),
            change_pct: change_pct.map(|value| value.round(2)),
            real_change_pct: real_change_pct.map(|value| value.round(2)),
        });
    }
    Ok(out)
}

/// The unit price of an asset without a curve: its hand-set price (which
/// only the vault values), else the last price a transaction recorded for
/// it, in `currency`.
fn unpriced_unit(books: &Books<'_>, key: &str, currency: &str) -> Option<Amount> {
    books
        .manual_price_usd(key)
        .or_else(|| last_stamp(books.doc(), key).map(|(_, usd)| usd))
        .and_then(|usd| Some(usd * per_usd(books.doc(), currency)?))
}

/// What one unit of money of `date` is worth today: the monthly series
/// when the currency follows it, else the flat rate compounded monthly.
pub(crate) fn inflation_since(
    doc: &VaultDoc,
    currency: &str,
    date: &str,
    zone: &str,
) -> Option<Amount> {
    let from = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let today = calendar::Calendar::new(zone).date_of(secs_or_now(""));
    if from > today {
        return None;
    }
    if let Some(coefficients) = real_coefficients(doc, currency, from, today) {
        return coefficients.get(&calendar::first_of_month(from)).copied();
    }
    let rate = inflation_on(doc, currency, "9999-12")?;
    let months = (today.year() - from.year()) * 12 + today.month() as i32 - from.month() as i32;
    let monthly = (Amount::from(1) + rate / hundred()).root_n(12);
    Some(monthly.pow_int(u32::try_from(months.max(0)).unwrap_or(0)))
}

pub(crate) fn property_store<'doc>(doc: &'doc VaultDoc, store_id: &StoreId) -> Result<&'doc Store> {
    find_store(doc, store_id.as_str())
        .filter(|store| store.kind == StoreKind::Property)
        .ok_or_else(|| Error::NotFound(format!("property {store_id}")))
}

/// Check a new dated valuation; returns it as it is to be stored: the date
/// and the currency cleaned up.
pub fn plan_valuation(doc: &VaultDoc, input: ValuationInput) -> Result<ValuationInput> {
    let store = property_store(doc, &StoreId::from(&input.store_id))?;
    let held = doc
        .holdings
        .iter()
        .any(|row| row.store_id == store.id && row.asset.key() == input.asset_key);
    if !held {
        return Err(Error::InvalidInput(
            "the property holds no such asset".into(),
        ));
    }
    if !input.unit_price.is_positive() {
        return Err(Error::InvalidInput("the price must be positive".into()));
    }
    let currency = input.currency.trim().to_lowercase();
    if currency.is_empty() {
        return Err(Error::InvalidInput("choose a currency".into()));
    }
    if date_secs(&input.date).is_none() {
        return Err(Error::InvalidInput("the date must be YYYY-MM-DD".into()));
    }
    Ok(ValuationInput {
        date: input.date.trim().to_string(),
        currency,
        ..input
    })
}

/// The valuations to add when a rule is set or switched off. Switching it on
/// where nothing was valued yet first values every asset at today's price, so
/// the value continues from where it is instead of jumping (plan §11.4).
pub fn plan_revalue_rule(
    books: &Books<'_>,
    store_id: &StoreId,
    rule: Option<&RevalueRule>,
    zone: &str,
) -> Result<Vec<ValuationInput>> {
    if let Some(rule) = rule {
        check_rule(rule)?;
    }
    let doc = books.doc();
    let store = property_store(doc, store_id)?;
    let was_on = store
        .property
        .as_ref()
        .is_some_and(|info| info.revalue.is_some());
    if rule.is_none() || was_on {
        return Ok(Vec::new());
    }
    let today = calendar::Calendar::new(zone)
        .date_of(secs_or_now(""))
        .format("%Y-%m-%d")
        .to_string();
    let currency = store
        .property
        .as_ref()
        .map(|info| info.currency.trim().to_lowercase())
        .filter(|code| !code.is_empty())
        .unwrap_or_else(|| books.base_and_fx().0.to_string());
    let mut starts: Vec<ValuationInput> = Vec::new();
    for holding in doc
        .holdings
        .iter()
        .filter(|row| row.store_id == store.id && !row.asset.is_fiat())
    {
        let key = holding.asset.key();
        let valued = doc.valuations.iter().any(|row| row.asset_key == key)
            || starts.iter().any(|row| row.asset_key == key);
        if valued {
            continue;
        }
        let usd = crate::money::price_info::price_quote_for(books, &key).usd;
        let Some(rate) = per_usd(doc, &currency) else {
            continue;
        };
        if !usd.is_positive() {
            continue;
        }
        starts.push(ValuationInput {
            store_id: store.id.clone(),
            asset_key: key,
            date: today.clone(),
            unit_price: (usd * rate).round(2),
            currency: currency.clone(),
        });
    }
    Ok(starts)
}

/// The rule a new property starts with.
pub(crate) fn initial_rule(category: PropertyCategory) -> Option<RevalueRule> {
    RevalueRule::default_for(category)
}
