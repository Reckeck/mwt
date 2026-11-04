//! Property value over time (plan G3): dated valuations joined by straight
//! lines, and after the last one an automatic rule — a yearly percent applied
//! month by month, held at a floor when it depreciates.
//!
//! An object nobody revalued and gave no rule keeps the old behaviour: its
//! hand-set price (`manual_prices`) holds for the whole history. Once it has
//! a valuation or a rule, that old price becomes the point at the purchase
//! date, so the history before the first valuation does not change.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::model::store::{PropertyCategory, RevalueFloor, RevalueRule, Store, StoreKind};
use crate::model::transaction::Transaction;
use crate::model::vault::Valuation;
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;

/// Average month, for the rule's position inside a month.
const MONTH_SECS: i64 = 2_629_746;
/// The rule is not run further than this many months ahead.
const MAX_MONTHS: i64 = 1200;

/// The value of one unit of a property asset at any moment, in `currency`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValuationCurve {
    pub currency: String,
    /// `(unix secs, unit price)`, oldest first.
    points: Vec<(i64, Amount)>,
    rule: Option<CurveRule>,
}

#[derive(Debug, Clone, PartialEq)]
struct CurveRule {
    /// Growth over one month: `(1 + pct/100)^(1/12)`.
    monthly: Amount,
    floor: Option<Amount>,
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
        let start = last * rule.monthly.pow_int(u32::try_from(whole).unwrap_or(0));
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

fn hundred() -> Amount {
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

impl UnlockedVault {
    /// Whether any curve exists at all: the fast path keeps vaults without
    /// valuations and rules exactly as they were.
    fn has_curves(&self) -> bool {
        !self.doc.valuations.is_empty()
            || self.doc.stores.iter().any(|store| {
                store
                    .property
                    .as_ref()
                    .is_some_and(|info| info.revalue.is_some())
            })
    }

    /// Curves of every property asset that has a valuation or a rule.
    pub(crate) fn valuation_curves(&self) -> HashMap<String, ValuationCurve> {
        let mut out = HashMap::new();
        if !self.has_curves() {
            return out;
        }
        for store in &self.doc.stores {
            if store.kind != StoreKind::Property {
                continue;
            }
            for holding in self
                .doc
                .holdings
                .iter()
                .filter(|row| row.store_id == store.id)
            {
                let key = holding.asset.key();
                if out.contains_key(&key) {
                    continue;
                }
                if let Some(curve) = self.curve_in(store, &key) {
                    out.insert(key, curve);
                }
            }
        }
        out
    }

    /// The curve of one asset, if it has one.
    pub(crate) fn valuation_curve(&self, asset_key: &str) -> Option<ValuationCurve> {
        if !self.has_curves() {
            return None;
        }
        let store = self.property_of(asset_key)?;
        self.curve_in(store, asset_key)
    }

    fn property_of(&self, asset_key: &str) -> Option<&Store> {
        self.doc.holdings.iter().find_map(|holding| {
            (holding.asset.key() == asset_key)
                .then(|| self.find_store(&holding.store_id))
                .flatten()
                .filter(|store| store.kind == StoreKind::Property)
        })
    }

    fn curve_in(&self, store: &Store, asset_key: &str) -> Option<ValuationCurve> {
        let info = store.property.as_ref();
        let rule = info.and_then(|info| info.revalue.as_ref());
        let mut dated: Vec<&Valuation> = self
            .doc
            .valuations
            .iter()
            .filter(|row| row.asset_key == asset_key)
            .collect();
        if dated.is_empty() && rule.is_none() {
            return None;
        }
        dated.sort_by(|left, right| left.date.cmp(&right.date));
        let purchase = self.purchase_of(store, asset_key);
        let base = self.base_point(asset_key, purchase.as_ref());
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
                points.push((*at, self.latest_convert(*price, from, &currency)?));
            }
        }
        for row in &dated {
            let Some(at) = date_secs(&row.date) else {
                continue;
            };
            let price = self.latest_convert(row.unit_price, &row.currency, &currency)?;
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
                    self.inflation_series(&currency)
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
                    self.latest_convert(amount, &from, &currency)
                }
                Some(RevalueFloor::PctOfPurchase { pct }) => purchase.as_ref().and_then(|row| {
                    self.latest_convert(row.unit_price, &row.currency, &currency)
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
    fn latest_convert(&self, amount: Amount, from: &str, to: &str) -> Option<Amount> {
        let (from, to) = (from.trim().to_lowercase(), to.trim().to_lowercase());
        if from == to {
            return Some(amount);
        }
        let from_rate = self.per_usd(&from).filter(|rate| !rate.is_zero())?;
        Some(amount / from_rate * self.per_usd(&to)?)
    }

    /// The first point of a curve: the old hand-set price at the purchase,
    /// else the purchase itself. `(secs, price, currency, kind)`.
    fn base_point(
        &self,
        asset_key: &str,
        purchase: Option<&PropertyPurchase>,
    ) -> Option<(i64, Amount, String, ValuationBaseKind)> {
        let at = purchase
            .and_then(|row| date_secs(&row.date))
            .unwrap_or_default();
        if let Some(manual) = self
            .doc
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
    /// (its unit price is in the base currency of that moment), else the old
    /// purchase fields of the form.
    pub(crate) fn purchase_of(&self, store: &Store, asset_key: &str) -> Option<PropertyPurchase> {
        let first = self
            .doc
            .transactions
            .iter()
            .filter_map(|tx| match tx {
                Transaction::Topup(inner)
                    if inner.store_id == store.id && inner.asset.key() == asset_key =>
                {
                    let price = inner.price.price_base.or(inner.details.unit_price)?;
                    Some((inner.meta.timestamp.as_str(), price))
                }
                _ => None,
            })
            .min_by(|left, right| left.0.cmp(right.0));
        if let Some((timestamp, unit_price)) = first {
            let date = calendar::Calendar::new("UTC").date_of(secs_or_now(timestamp));
            return Some(PropertyPurchase {
                date: date.format("%Y-%m-%d").to_string(),
                unit_price,
                currency: self.base_and_fx().0,
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
                self.base_and_fx().0
            } else {
                currency
            },
            from_operation: false,
        })
    }

    /// The value story of each asset a property holds.
    pub fn property_values(
        &self,
        store_id: &str,
        manual_inflation: &BTreeMap<String, Amount>,
        zone: &str,
    ) -> Result<Vec<PropertyValue>> {
        let store = self.property_store(store_id)?;
        let now = secs_or_now("");
        let mut out = Vec::new();
        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|row| row.store_id == store.id)
        {
            let key = holding.asset.key();
            let curve = self.curve_in(store, &key);
            let purchase = self.purchase_of(store, &key);
            let base = self.base_point(&key, purchase.as_ref());
            let currency = curve
                .as_ref()
                .map(|curve| curve.currency.clone())
                .or_else(|| base.as_ref().map(|row| row.2.clone()))
                .unwrap_or_else(|| self.base_and_fx().0);
            let unit_price = match &curve {
                Some(curve) => curve.value_at(now),
                None => self
                    .manual_price_usd(&key)
                    .or_else(|| self.last_stamp(&key).map(|(_, usd)| usd))
                    .and_then(|usd| Some(usd * self.per_usd(&currency)?)),
            };
            let mut valuations: Vec<Valuation> = self
                .doc
                .valuations
                .iter()
                .filter(|row| row.asset_key == key)
                .cloned()
                .collect();
            valuations.sort_by(|left, right| left.date.cmp(&right.date));
            let paid = purchase
                .as_ref()
                .and_then(|row| self.latest_convert(row.unit_price, &row.currency, &currency));
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
                        let grown = paid
                            * self.inflation_since(&currency, &row.date, manual_inflation, zone)?;
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

    /// What one unit of money of `date` is worth today: the monthly series
    /// when the currency follows it, else the flat rate compounded monthly.
    fn inflation_since(
        &self,
        currency: &str,
        date: &str,
        manual: &BTreeMap<String, Amount>,
        zone: &str,
    ) -> Option<Amount> {
        let from = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
        let today = calendar::Calendar::new(zone).date_of(secs_or_now(""));
        if from > today {
            return None;
        }
        if let Some(coefficients) = self.real_coefficients(currency, manual, from, today) {
            return coefficients.get(&calendar::first_of_month(from)).copied();
        }
        let rate = self.inflation_on(currency, "9999-12", manual)?;
        let months = (today.year() - from.year()) * 12 + today.month() as i32 - from.month() as i32;
        let monthly = (Amount::from(1) + rate / hundred()).root_n(12);
        Some(monthly.pow_int(u32::try_from(months.max(0)).unwrap_or(0)))
    }

    fn property_store(&self, store_id: &str) -> Result<&Store> {
        self.find_store(store_id)
            .filter(|store| store.kind == StoreKind::Property)
            .ok_or_else(|| Error::NotFound(format!("property {store_id}")))
    }

    /// Add a dated valuation; one on the same day replaces it.
    pub fn add_valuation(&mut self, input: ValuationInput) -> Result<Valuation> {
        let store = self.property_store(&input.store_id)?;
        let held = self
            .doc
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
        let date = input.date.trim().to_string();
        let valuation = Valuation {
            id: super::sync_ids::valuation_id(&input.asset_key, &date),
            store_id: input.store_id,
            asset_key: input.asset_key,
            date,
            unit_price: input.unit_price,
            currency,
        };
        self.doc
            .valuations
            .retain(|row| !(row.asset_key == valuation.asset_key && row.date == valuation.date));
        self.doc.valuations.push(valuation.clone());
        self.persist()?;
        Ok(valuation)
    }

    /// Remove a valuation; returns it so the UI can put it back.
    pub fn delete_valuation(&mut self, id: &str) -> Result<Valuation> {
        let index = self
            .doc
            .valuations
            .iter()
            .position(|row| row.id == id)
            .ok_or_else(|| Error::NotFound(format!("valuation {id}")))?;
        let removed = self.doc.valuations.remove(index);
        self.persist()?;
        Ok(removed)
    }

    /// Set or switch off a property's rule. Switching it on where nothing was
    /// valued yet first values every asset at today's price, so the value
    /// continues from where it is instead of jumping (plan §11.4).
    pub fn set_revalue_rule(
        &mut self,
        store_id: &str,
        rule: Option<RevalueRule>,
        zone: &str,
    ) -> Result<()> {
        if let Some(rule) = &rule {
            check_rule(rule)?;
        }
        let store = self.property_store(store_id)?.clone();
        let was_on = store
            .property
            .as_ref()
            .is_some_and(|info| info.revalue.is_some());
        if rule.is_some() && !was_on {
            let today = calendar::Calendar::new(zone)
                .date_of(secs_or_now(""))
                .format("%Y-%m-%d")
                .to_string();
            let fallback = self.base_and_fx().0;
            let currency = store
                .property
                .as_ref()
                .map(|info| info.currency.trim().to_lowercase())
                .filter(|code| !code.is_empty())
                .unwrap_or(fallback);
            let keys: Vec<String> = self
                .doc
                .holdings
                .iter()
                .filter(|row| row.store_id == store.id && !row.asset.is_fiat())
                .map(|row| row.asset.key())
                .collect();
            for key in keys {
                if self.doc.valuations.iter().any(|row| row.asset_key == key) {
                    continue;
                }
                let usd = self.price_quote_for(&key).usd;
                let Some(rate) = self.per_usd(&currency) else {
                    continue;
                };
                if !usd.is_positive() {
                    continue;
                }
                self.doc.valuations.push(Valuation {
                    id: super::sync_ids::valuation_id(&key, &today),
                    store_id: store.id.clone(),
                    asset_key: key,
                    date: today.clone(),
                    unit_price: (usd * rate).round(2),
                    currency: currency.clone(),
                });
            }
        }
        let found = self
            .doc
            .stores
            .iter_mut()
            .find(|row| row.id == store_id)
            .ok_or_else(|| Error::NotFound(format!("property {store_id}")))?;
        let info = found.property.get_or_insert_with(Default::default);
        info.revalue = rule;
        self.persist()
    }
}

/// The rule a new property starts with.
pub(crate) fn initial_rule(category: PropertyCategory) -> Option<RevalueRule> {
    RevalueRule::default_for(category)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::store::{AssetRef, PropertyInfo};
    use crate::model::transaction::{KindDetails, PriceStamp, TopupTx};
    use crate::model::vault::ManualPrice;
    use crate::service::StoreExtras;

    const DAY: i64 = 86_400;

    fn curve(points: &[(i64, &str)], rule: Option<(&str, Option<&str>)>) -> ValuationCurve {
        ValuationCurve {
            currency: "usd".into(),
            points: points
                .iter()
                .map(|(at, price)| (*at, amount(price)))
                .collect(),
            rule: rule.map(|(pct, floor)| CurveRule {
                monthly: (Amount::from(1) + amount(pct) / hundred()).root_n(12),
                floor: floor.map(amount),
            }),
        }
    }

    fn near(left: Amount, right: &str) -> bool {
        (left - amount(right)).abs() < amount("0.01")
    }

    #[test]
    fn points_join_by_straight_lines() {
        let line = curve(&[(0, "100"), (10 * DAY, "200")], None);
        assert_eq!(line.value_at(-DAY), Some(amount("100")));
        assert!(near(line.value_at(5 * DAY).unwrap(), "150"));
        assert_eq!(line.value_at(20 * DAY), Some(amount("200")));
    }

    #[test]
    fn the_rule_runs_after_the_last_point() {
        let grow = curve(&[(0, "1000")], Some(("12", None)));
        let year = grow.value_at(12 * MONTH_SECS).unwrap();
        assert!(near(year, "1120"), "{year}");
        let half = grow.value_at(6 * MONTH_SECS).unwrap();
        assert!(half > amount("1050") && half < amount("1060"), "{half}");
    }

    #[test]
    fn the_floor_stops_depreciation() {
        let car = curve(&[(0, "1000")], Some(("-50", Some("400"))));
        let late = car.value_at(10 * 12 * MONTH_SECS).unwrap();
        assert_eq!(late, amount("400"));
        // A valuation below the floor is kept, not lifted.
        let below = curve(&[(0, "300")], Some(("-10", Some("400"))));
        assert_eq!(below.value_at(24 * MONTH_SECS), Some(amount("300")));
    }

    fn thing() -> AssetRef {
        AssetRef {
            symbol: "FLAT".into(),
            name: "Flat".into(),
            coingecko_id: None,
            chain: None,
            contract: None,
            quote: "FLAT".into(),
        }
    }

    fn property(vault: &mut UnlockedVault, category: PropertyCategory) -> String {
        vault
            .create_store_with(
                &format!("{category:?}"),
                StoreKind::Property,
                "#555",
                "",
                None,
                StoreExtras {
                    property: Some(PropertyInfo {
                        category,
                        ..PropertyInfo::default()
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    fn buy(vault: &mut UnlockedVault, store_id: &str, price: &str, when: &str) {
        let mut tx = Transaction::Topup(TopupTx {
            meta: meta("bought", &["income_other"]),
            store_id: store_id.to_string(),
            asset: thing(),
            amount: amount("1"),
            fee: None,
            from: None,
            details: KindDetails {
                unit_price: Some(amount(price)),
                ..KindDetails::default()
            },
            price: PriceStamp::default(),
        });
        at(&mut tx, when);
        vault.add_transaction(tx).unwrap();
    }

    /// An object from before this change: no rule, a hand-set price.
    fn old_flat(vault: &mut UnlockedVault) -> String {
        vault.set_base_currency("usd");
        let flat = property(vault, PropertyCategory::Flat);
        buy(vault, &flat, "100000", "2020-01-10T00:00:00Z");
        let store = vault
            .doc
            .stores
            .iter_mut()
            .find(|row| row.id == flat)
            .unwrap();
        store.property.as_mut().unwrap().revalue = None;
        vault.doc.manual_prices.push(ManualPrice {
            asset_key: thing().key(),
            unit_price: amount("150000"),
            currency: "usd".into(),
            updated_at: String::new(),
        });
        flat
    }

    #[test]
    fn new_objects_get_the_rule_of_their_type() {
        let (_dir, mut vault) = empty_vault();
        let car = property(&mut vault, PropertyCategory::Car);
        let rule = vault
            .find_store(&car)
            .unwrap()
            .property
            .clone()
            .unwrap()
            .revalue;
        assert_eq!(rule, RevalueRule::default_for(PropertyCategory::Car));
        assert!(rule.unwrap().floor.is_some());
        let other = property(&mut vault, PropertyCategory::Other);
        let info = vault.find_store(&other).unwrap().property.clone().unwrap();
        assert_eq!(info.revalue, None);
        // Saving the form keeps the rule.
        vault
            .update_store(
                &car,
                "Car",
                "#555",
                "",
                StoreExtras {
                    property: Some(PropertyInfo {
                        category: PropertyCategory::Car,
                        ..PropertyInfo::default()
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap();
        let kept = vault.find_store(&car).unwrap().property.clone().unwrap();
        assert!(kept.revalue.is_some());
    }

    #[test]
    fn old_objects_keep_their_value_and_history() {
        let (_dir, mut vault) = empty_vault();
        old_flat(&mut vault);
        let key = thing().key();
        let early = secs_or_now("2021-01-01T00:00:00Z");
        let before = vault.rate_book().history_price(&key, early);
        assert_eq!(before, Some(amount("150000")));
        assert_eq!(vault.net_worth().non_financial, amount("150000"));
        assert!(vault.valuation_curves().is_empty());
    }

    #[test]
    fn a_valuation_keeps_the_old_price_as_the_start() {
        let (_dir, mut vault) = empty_vault();
        let flat = old_flat(&mut vault);
        let key = thing().key();
        vault
            .add_valuation(ValuationInput {
                store_id: flat.clone(),
                asset_key: key.clone(),
                date: "2024-01-10".into(),
                unit_price: amount("200000"),
                currency: "usd".into(),
            })
            .unwrap();
        let book = vault.rate_book();
        let purchase = secs_or_now("2020-01-10T00:00:00Z");
        assert_eq!(book.history_price(&key, purchase), Some(amount("150000")));
        assert_eq!(vault.net_worth().non_financial, amount("200000"));
        let values = vault
            .property_values(&flat, &BTreeMap::new(), "UTC")
            .unwrap();
        assert_eq!(values[0].base, Some(ValuationBaseKind::Manual));
        assert_eq!(values[0].change, Some(amount("100000")));
        assert_eq!(values[0].change_pct, Some(amount("100")));
        // The same day replaces, delete returns the point.
        let again = vault
            .add_valuation(ValuationInput {
                store_id: flat,
                asset_key: key,
                date: "2024-01-10".into(),
                unit_price: amount("210000"),
                currency: "usd".into(),
            })
            .unwrap();
        assert_eq!(vault.doc.valuations.len(), 1);
        assert_eq!(vault.delete_valuation(&again.id).unwrap(), again);
        assert!(vault.doc.valuations.is_empty());
    }

    #[test]
    fn depreciation_needs_a_floor() {
        let (_dir, mut vault) = empty_vault();
        let flat = old_flat(&mut vault);
        let rule = RevalueRule {
            pct_per_year: Some(amount("-10")),
            floor: None,
        };
        assert!(vault.set_revalue_rule(&flat, Some(rule), "UTC").is_err());
        let rule = RevalueRule {
            pct_per_year: Some(amount("-10")),
            floor: Some(RevalueFloor::PctOfPurchase { pct: amount("20") }),
        };
        vault.set_revalue_rule(&flat, Some(rule), "UTC").unwrap();
        // Switching on starts from today's value, so capital does not jump.
        assert_eq!(vault.doc.valuations.len(), 1);
        assert_eq!(vault.doc.valuations[0].unit_price, amount("150000"));
        // Only the hours since midnight of depreciation, not years.
        let now = vault.net_worth().non_financial;
        assert!(now <= amount("150000") && now > amount("149000"), "{now}");
    }

    #[test]
    fn loyalty_prices_stay_as_they_were() {
        let (_dir, mut vault) = empty_vault();
        let flat = old_flat(&mut vault);
        vault
            .add_valuation(ValuationInput {
                store_id: flat,
                asset_key: thing().key(),
                date: "2024-01-10".into(),
                unit_price: amount("200000"),
                currency: "usd".into(),
            })
            .unwrap();
        vault.doc.manual_prices.push(ManualPrice {
            asset_key: "asset:POINTS".into(),
            unit_price: amount("0.5"),
            currency: "usd".into(),
            updated_at: String::new(),
        });
        assert_eq!(vault.manual_price_usd("asset:POINTS"), Some(amount("0.5")));
        assert!(vault.valuation_curve("asset:POINTS").is_none());
    }
}
