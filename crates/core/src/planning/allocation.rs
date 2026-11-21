//! Portfolio baskets with target shares and the top-up helper (plan F):
//! where this month's free money should go to bring the shares back.

use std::collections::{HashMap, HashSet};

use chrono::Duration;

use crate::calendar::{self, Calendar};
use crate::model::planning::{AssetClass, AssetClassLink};
use crate::model::store::{Holding, StoreKind};
use crate::model::vault::VaultDoc;
use crate::money::books::Books;
use crate::{Amount, Error, Result};

/// Ids of the preset baskets; an empty name on one of them means «the UI's
/// own name for it».
pub const PRESET_LOCAL: &str = "local";
pub const PRESET_GLOBAL: &str = "global";
pub const PRESET_RISKY: &str = "risky";

/// The 40/40/20 split offered until the user saves their own (decision 23).
pub fn preset_classes() -> Vec<AssetClass> {
    [
        (PRESET_LOCAL, 40, false),
        (PRESET_GLOBAL, 40, false),
        (PRESET_RISKY, 20, true),
    ]
    .into_iter()
    .map(|(id, pct, cap)| AssetClass {
        id: id.into(),
        name: String::new(),
        target_pct: Amount::from(pct),
        cap,
    })
    .collect()
}

/// The baskets as the editor shows them.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetClassesView {
    pub classes: Vec<AssetClass>,
    /// Nothing is saved yet: these are the preset ones.
    pub preset: bool,
    /// Assets placed in a basket by hand.
    pub links: Vec<AssetClassLink>,
}

/// What a basket is assigned to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssetClassTarget {
    /// An asset by `AssetRef::key()`, wherever it is held.
    Asset { asset_key: String },
    /// Everything in a store that has no basket of its own.
    Store { store_id: String },
}

/// One basket in the top-up advice, in the base currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TopupRow {
    pub class: AssetClass,
    pub value: Amount,
    /// Share of the classified part now, percent; `None` while it is empty.
    pub pct_before: Option<Amount>,
    /// What to put here out of the amount.
    pub add: Amount,
    pub pct_after: Option<Amount>,
    /// A capped basket above its target: it gets nothing.
    pub capped: bool,
}

/// Where the free money of the month should go (plan F).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TopupAdvice {
    pub currency: String,
    /// The amount split; last month's savings unless given.
    pub amount: Amount,
    /// `amount` is last month's savings, not a figure the user typed.
    pub from_savings: bool,
    /// The month `amount` was saved in, when it comes from savings.
    pub savings_month: Option<String>,
    pub rows: Vec<TopupRow>,
    /// Holdings in no basket: outside the shares.
    pub unclassified: Amount,
    /// Emergency fund accounts: outside the baskets altogether (decision 35).
    pub emergency: Amount,
    /// The baskets are the preset, not saved by the user.
    pub preset: bool,
}

/// Which part of the financial capital a holding belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Slot {
    /// An emergency fund account.
    Fund,
    /// A basket, or `None` for «no basket».
    Class(Option<String>),
}

/// The baskets in force: the saved ones or the preset.
pub(crate) fn effective_classes(doc: &VaultDoc) -> Vec<AssetClass> {
    if doc.asset_classes.is_empty() {
        preset_classes()
    } else {
        doc.asset_classes.clone()
    }
}

pub fn asset_classes(doc: &VaultDoc) -> AssetClassesView {
    AssetClassesView {
        classes: effective_classes(doc),
        preset: doc.asset_classes.is_empty(),
        links: doc.asset_class_of.clone(),
    }
}

/// The baskets a save leaves, and the ids a gone basket's assignments are
/// swept by.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetClassesUpdate {
    pub classes: Vec<AssetClass>,
    /// Ids of the baskets that stay (the preset ones for an empty list).
    pub known: HashSet<String>,
}

/// Check the baskets of a save: names trimmed, shares in 0..=100 adding up
/// to 100, no id twice. Nothing is written; an empty list means the preset.
pub fn plan_set_asset_classes(classes: Vec<AssetClass>) -> Result<AssetClassesUpdate> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<AssetClass> = Vec::with_capacity(classes.len());
    for mut class in classes {
        class.name = class.name.trim().to_string();
        if class.target_pct.is_negative() || class.target_pct > Amount::from(100) {
            return Err(Error::InvalidInput(format!(
                "asset class {}: target share must be 0..=100",
                class.id
            )));
        }
        if !seen.insert(class.id.clone()) {
            return Err(Error::AlreadyExists(format!("asset class {}", class.id)));
        }
        out.push(class);
    }
    let total = out
        .iter()
        .fold(Amount::ZERO, |sum, class| sum + class.target_pct);
    if !out.is_empty() && total != Amount::from(100) {
        return Err(Error::InvalidInput(format!(
            "asset class shares add up to {total}, not 100"
        )));
    }
    let known: HashSet<String> = if out.is_empty() {
        preset_classes().into_iter().map(|class| class.id).collect()
    } else {
        seen
    };
    Ok(AssetClassesUpdate {
        classes: out,
        known,
    })
}

/// Refuse an assignment that cannot be made: an unknown basket, an empty
/// asset key, a fund account, a missing store. Nothing is written.
pub fn check_assign_asset_class(
    doc: &VaultDoc,
    target: &AssetClassTarget,
    class_id: Option<&str>,
) -> Result<()> {
    if let Some(id) = class_id
        && !effective_classes(doc).iter().any(|class| class.id == id)
    {
        return Err(Error::Missing(format!("asset class {id}")));
    }
    match target {
        AssetClassTarget::Asset { asset_key } if asset_key.trim().is_empty() => {
            Err(Error::InvalidInput("asset key is empty".into()))
        }
        AssetClassTarget::Asset { .. } => Ok(()),
        AssetClassTarget::Store { store_id } => {
            if class_id.is_some() && fund_store_ids(doc).contains(store_id) {
                return Err(Error::InvalidInput(
                    "an emergency fund account stays outside the baskets".into(),
                ));
            }
            if doc.stores.iter().any(|store| &store.id == store_id) {
                Ok(())
            } else {
                Err(Error::Missing(format!("store {store_id}")))
            }
        }
    }
}

pub(crate) fn fund_store_ids(doc: &VaultDoc) -> HashSet<String> {
    doc.emergency_fund.scope.store_ids.iter().cloned().collect()
}

/// The slot of every financial holding: asset basket, else the store's,
/// else none; fund accounts apart.
pub(crate) fn slot_resolver(doc: &VaultDoc) -> impl Fn(&Holding) -> Slot + '_ {
    let fund = fund_store_ids(doc);
    let known: HashSet<String> = effective_classes(doc)
        .into_iter()
        .map(|class| class.id)
        .collect();
    let by_asset: HashMap<&str, &str> = doc
        .asset_class_of
        .iter()
        .filter(|link| known.contains(&link.class_id))
        .map(|link| (link.asset_key.as_str(), link.class_id.as_str()))
        .collect();
    let by_store: HashMap<&str, &str> = doc
        .stores
        .iter()
        .filter_map(|store| {
            let class = store.asset_class.as_deref()?;
            known.contains(class).then_some((store.id.as_str(), class))
        })
        .collect();
    move |holding: &Holding| {
        if fund.contains(&holding.store_id) {
            return Slot::Fund;
        }
        let key = holding.asset.key();
        let class = by_asset
            .get(key.as_str())
            .or_else(|| by_store.get(holding.store_id.as_str()))
            .map(|id| (*id).to_string());
        Slot::Class(class)
    }
}

/// Value of each basket, unclassified and fund parts, in the base
/// currency. Only money and investments count: no property, points,
/// loans or credit cards.
pub(crate) fn class_values(books: &Books<'_>) -> (HashMap<String, Amount>, Amount, Amount) {
    let doc = books.doc();
    let investable: HashSet<&str> = doc
        .stores
        .iter()
        .filter(|store| {
            !matches!(
                store.kind,
                StoreKind::Property | StoreKind::Loyalty | StoreKind::Liability
            ) && !store.is_debt()
        })
        .map(|store| store.id.as_str())
        .collect();
    let slot_of = slot_resolver(doc);
    let mut by_class: HashMap<String, Amount> = HashMap::new();
    let mut unclassified = Amount::ZERO;
    let mut emergency = Amount::ZERO;
    for holding in doc.holdings.iter().filter(|holding| {
        !holding.asset.is_derivative() && investable.contains(holding.store_id.as_str())
    }) {
        let value = holding.quantity * books.price_base(&holding.asset.key());
        match slot_of(holding) {
            Slot::Fund => emergency += value,
            Slot::Class(Some(id)) => *by_class.entry(id).or_default() += value,
            Slot::Class(None) => unclassified += value,
        }
    }
    (by_class, unclassified, emergency)
}

/// Split `amount` (default: last month's savings) over the baskets so
/// the shares move towards their targets (plan F). Creates nothing.
pub fn topup_advice(
    books: &Books<'_>,
    amount: Option<Amount>,
    now: i64,
    zone: &str,
) -> TopupAdvice {
    let base = books.base_and_fx().0.to_string();
    let (amount, from_savings, savings_month) = match amount {
        Some(given) => (given.max(Amount::ZERO), false, None),
        None => {
            let (saved, month) = last_month_savings(books, now, zone, &base);
            (saved, true, Some(month))
        }
    };
    let (by_class, unclassified, emergency) = class_values(books);
    let classes = effective_classes(books.doc());
    let values: Vec<Amount> = classes
        .iter()
        .map(|class| by_class.get(&class.id).copied().unwrap_or_default())
        .collect();
    let adds = split_topup(&classes, &values, amount);
    let total = values.iter().fold(Amount::ZERO, |sum, value| sum + *value);
    let share = |part: Amount, whole: Amount| {
        (!whole.is_zero()).then(|| (part / whole * Amount::from(100)).round(2))
    };
    let rows = classes
        .into_iter()
        .zip(values)
        .zip(adds)
        .map(|((class, value), (add, capped))| TopupRow {
            pct_before: share(value, total),
            pct_after: share(value + add, total + amount),
            value: value.round(2),
            add,
            capped,
            class,
        })
        .collect();
    TopupAdvice {
        currency: base,
        amount: amount.round(2),
        from_savings,
        savings_month,
        rows,
        unclassified: unclassified.round(2),
        emergency: emergency.round(2),
        preset: books.doc().asset_classes.is_empty(),
    }
}

/// Last month's `savings_fact` in `currency`, never below zero.
pub(crate) fn last_month_savings(
    books: &Books<'_>,
    now: i64,
    zone: &str,
    currency: &str,
) -> (Amount, String) {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let first = today - Duration::days(i64::from(chrono::Datelike::day(&today)) - 1);
    let month = calendar::month_key(first - Duration::days(1));
    let saved = crate::planning::budget::month_summary(books, &month, zone)
        .map(|summary| {
            books
                .rate_book()
                .convert(summary.savings_fact, &summary.currency, currency, now)
                .unwrap_or(summary.savings_fact)
        })
        .unwrap_or_default();
    (saved.max(Amount::ZERO), month)
}

/// How much of `amount` each basket gets, and whether it was left out as a
/// capped basket above its target. Positive deficits share the amount
/// pro rata; with none, the targets do. Sums to `amount` to the cent.
pub(crate) fn split_topup(
    classes: &[AssetClass],
    values: &[Amount],
    amount: Amount,
) -> Vec<(Amount, bool)> {
    let hundred = Amount::from(100);
    let total = values.iter().fold(Amount::ZERO, |sum, value| sum + *value);
    let after = total + amount;
    let capped: Vec<bool> = classes
        .iter()
        .zip(values)
        .map(|(class, value)| {
            class.cap && !total.is_zero() && *value / total * hundred > class.target_pct
        })
        .collect();
    let deficits: Vec<Amount> = classes
        .iter()
        .zip(values)
        .zip(&capped)
        .map(|((class, value), capped)| {
            if *capped {
                Amount::ZERO
            } else {
                (after * class.target_pct / hundred - *value).max(Amount::ZERO)
            }
        })
        .collect();
    let mut weights = deficits;
    if weights.iter().all(Amount::is_zero) {
        weights = classes
            .iter()
            .zip(&capped)
            .map(|(class, capped)| {
                if *capped {
                    Amount::ZERO
                } else {
                    class.target_pct
                }
            })
            .collect();
    }
    let weight_sum = weights
        .iter()
        .fold(Amount::ZERO, |sum, weight| sum + *weight);
    let amount = amount.round(2);
    let mut adds: Vec<Amount> = weights
        .iter()
        .map(|weight| {
            if weight_sum.is_zero() {
                Amount::ZERO
            } else {
                (amount * *weight / weight_sum).round(2)
            }
        })
        .collect();
    // Rounding leftovers go to the largest share, so the parts add up.
    let rest = amount - adds.iter().fold(Amount::ZERO, |sum, add| sum + *add);
    if !rest.is_zero()
        && let Some(largest) = (0..adds.len()).max_by_key(|index| weights[*index])
        && !weights[largest].is_zero()
    {
        adds[largest] += rest;
    }
    adds.into_iter().zip(capped).collect()
}
