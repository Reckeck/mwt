//! Portfolio baskets with target shares and the top-up helper (plan F):
//! where this month's free money should go to bring the shares back.

use std::collections::{HashMap, HashSet};

use chrono::Duration;

use super::UnlockedVault;
use crate::calendar::{self, Calendar};
use crate::model::planning::{AssetClass, AssetClassLink};
use crate::model::store::{Holding, StoreKind};
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

impl UnlockedVault {
    /// The baskets in force: the saved ones or the preset.
    pub(crate) fn effective_classes(&self) -> Vec<AssetClass> {
        if self.doc.asset_classes.is_empty() {
            preset_classes()
        } else {
            self.doc.asset_classes.clone()
        }
    }

    pub fn asset_classes(&self) -> AssetClassesView {
        AssetClassesView {
            classes: self.effective_classes(),
            preset: self.doc.asset_classes.is_empty(),
            links: self.doc.asset_class_of.clone(),
        }
    }

    /// Replace the baskets. Shares must add up to 100; a basket that is gone
    /// takes its assignments with it. An empty list brings the preset back.
    pub fn set_asset_classes(&mut self, classes: Vec<AssetClass>) -> Result<AssetClassesView> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<AssetClass> = Vec::with_capacity(classes.len());
        for mut class in classes {
            class.name = class.name.trim().to_string();
            if class.id.trim().is_empty() {
                class.id = uuid::Uuid::new_v4().to_string();
            }
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
        self.doc
            .asset_class_of
            .retain(|link| known.contains(&link.class_id));
        for store in &mut self.doc.stores {
            if store
                .asset_class
                .as_ref()
                .is_some_and(|id| !known.contains(id))
            {
                store.asset_class = None;
            }
        }
        self.doc.asset_classes = out;
        self.persist()?;
        Ok(self.asset_classes())
    }

    /// Put an asset or a store in a basket, or take it out with `None`.
    /// Assigning to a preset basket saves the preset first.
    pub fn assign_asset_class(
        &mut self,
        target: AssetClassTarget,
        class_id: Option<String>,
    ) -> Result<AssetClassesView> {
        if let Some(id) = &class_id
            && !self.effective_classes().iter().any(|class| &class.id == id)
        {
            return Err(Error::Missing(format!("asset class {id}")));
        }
        if self.doc.asset_classes.is_empty() && class_id.is_some() {
            self.doc.asset_classes = preset_classes();
        }
        match target {
            AssetClassTarget::Asset { asset_key } => {
                if asset_key.trim().is_empty() {
                    return Err(Error::InvalidInput("asset key is empty".into()));
                }
                self.doc
                    .asset_class_of
                    .retain(|link| link.asset_key != asset_key);
                if let Some(class_id) = class_id {
                    self.doc.asset_class_of.push(AssetClassLink {
                        asset_key,
                        class_id,
                    });
                }
            }
            AssetClassTarget::Store { store_id } => {
                if class_id.is_some() && self.fund_store_ids().contains(&store_id) {
                    return Err(Error::InvalidInput(
                        "an emergency fund account stays outside the baskets".into(),
                    ));
                }
                let store = self
                    .doc
                    .stores
                    .iter_mut()
                    .find(|store| store.id == store_id)
                    .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
                store.asset_class = class_id;
            }
        }
        self.persist()?;
        Ok(self.asset_classes())
    }

    fn fund_store_ids(&self) -> HashSet<String> {
        self.doc
            .emergency_fund
            .scope
            .store_ids
            .iter()
            .cloned()
            .collect()
    }

    /// The slot of every financial holding: asset basket, else the store's,
    /// else none; fund accounts apart.
    pub(crate) fn slot_resolver(&self) -> impl Fn(&Holding) -> Slot + '_ {
        let fund = self.fund_store_ids();
        let known: HashSet<String> = self
            .effective_classes()
            .into_iter()
            .map(|class| class.id)
            .collect();
        let by_asset: HashMap<&str, &str> = self
            .doc
            .asset_class_of
            .iter()
            .filter(|link| known.contains(&link.class_id))
            .map(|link| (link.asset_key.as_str(), link.class_id.as_str()))
            .collect();
        let by_store: HashMap<&str, &str> = self
            .doc
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
    fn class_values(&self) -> (HashMap<String, Amount>, Amount, Amount) {
        let (_, fx) = self.base_and_fx();
        let investable: HashSet<&str> = self
            .doc
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
        let slot_of = self.slot_resolver();
        let mut by_class: HashMap<String, Amount> = HashMap::new();
        let mut unclassified = Amount::ZERO;
        let mut emergency = Amount::ZERO;
        for holding in self.doc.holdings.iter().filter(|holding| {
            !holding.asset.is_derivative() && investable.contains(holding.store_id.as_str())
        }) {
            let value = holding.quantity * self.price_usd_for(&holding.asset.key()) * fx;
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
    pub fn topup_advice(&self, amount: Option<Amount>, now: i64, zone: &str) -> TopupAdvice {
        let (base, _) = self.base_and_fx();
        let (amount, from_savings, savings_month) = match amount {
            Some(given) => (given.max(Amount::ZERO), false, None),
            None => {
                let (saved, month) = self.last_month_savings(now, zone, &base);
                (saved, true, Some(month))
            }
        };
        let (by_class, unclassified, emergency) = self.class_values();
        let classes = self.effective_classes();
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
            preset: self.doc.asset_classes.is_empty(),
        }
    }

    /// Last month's `savings_fact` in `currency`, never below zero.
    fn last_month_savings(&self, now: i64, zone: &str, currency: &str) -> (Amount, String) {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let first = today - Duration::days(i64::from(chrono::Datelike::day(&today)) - 1);
        let month = calendar::month_key(first - Duration::days(1));
        let saved = self
            .month_summary(&month, zone)
            .map(|summary| {
                self.rate_book()
                    .convert(summary.savings_fact, &self.budget_currency(), currency, now)
                    .unwrap_or(summary.savings_fact)
            })
            .unwrap_or_default();
        (saved.max(Amount::ZERO), month)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{AssetRef, Purpose, StoreScope};
    use crate::model::vault::PricePoint;
    use crate::service::test_support::{amount, empty_vault, income_of, store};

    fn vault() -> (tempfile::TempDir, UnlockedVault) {
        empty_vault()
    }

    /// A store holding fiat at one USD a unit.
    fn store_with(
        vault: &mut UnlockedVault,
        name: &str,
        kind: StoreKind,
        assets: &[(&str, &str)],
    ) -> String {
        let id = store(vault, name, kind, None);
        for (symbol, quantity) in assets {
            let asset = AssetRef::fiat(symbol, symbol);
            vault.doc.price_cache.push(PricePoint {
                asset_key: asset.key(),
                value_usd: amount("1"),
                ts: "2000-01-01T00:00:00Z".into(),
                source: "test".into(),
                change_24h_pct: None,
            });
            vault
                .add_transaction(income_of(&id, asset, quantity, "1", "seed"))
                .unwrap();
        }
        id
    }

    fn class(id: &str, pct: u32, cap: bool) -> AssetClass {
        AssetClass {
            id: id.into(),
            name: id.into(),
            target_pct: Amount::from(pct),
            cap,
        }
    }

    #[test]
    fn preset_until_saved() {
        let (_dir, vault) = vault();
        let view = vault.asset_classes();
        assert!(view.preset);
        let ids: Vec<&str> = view.classes.iter().map(|class| class.id.as_str()).collect();
        assert_eq!(ids, [PRESET_LOCAL, PRESET_GLOBAL, PRESET_RISKY]);
        assert!(view.classes[2].cap);
        assert!(vault.doc.asset_classes.is_empty(), "reading saves nothing");
    }

    #[test]
    fn shares_must_add_up_to_hundred() {
        let (_dir, mut vault) = vault();
        let err = vault
            .set_asset_classes(vec![class("one", 60, false), class("two", 30, false)])
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
        let err = vault
            .set_asset_classes(vec![class("one", 50, false), class("one", 50, false)])
            .unwrap_err();
        assert!(matches!(err, Error::AlreadyExists(_)), "{err:?}");
        let saved = vault
            .set_asset_classes(vec![class("", 70, false), class("two", 30, true)])
            .unwrap();
        assert!(!saved.preset);
        assert!(!saved.classes[0].id.is_empty(), "a new basket gets an id");
    }

    #[test]
    fn removing_a_basket_drops_its_assignments() {
        let (_dir, mut vault) = vault();
        let store = store_with(&mut vault, "Broker", StoreKind::Exchange, &[("USD", "100")]);
        vault
            .assign_asset_class(
                AssetClassTarget::Store {
                    store_id: store.clone(),
                },
                Some(PRESET_GLOBAL.into()),
            )
            .unwrap();
        assert_eq!(vault.doc.asset_classes.len(), 3, "the preset got saved");
        vault
            .assign_asset_class(
                AssetClassTarget::Asset {
                    asset_key: "fiat:usd".into(),
                },
                Some(PRESET_RISKY.into()),
            )
            .unwrap();
        vault
            .set_asset_classes(vec![class(PRESET_LOCAL, 100, false)])
            .unwrap();
        assert!(vault.doc.asset_class_of.is_empty());
        assert_eq!(vault.find_store(&store).unwrap().asset_class, None);
    }

    #[test]
    fn asset_basket_beats_store_basket() {
        let (_dir, mut vault) = vault();
        let store = store_with(
            &mut vault,
            "Broker",
            StoreKind::Exchange,
            &[("USD", "100"), ("EUR", "50")],
        );
        vault
            .assign_asset_class(
                AssetClassTarget::Store { store_id: store },
                Some(PRESET_GLOBAL.into()),
            )
            .unwrap();
        vault
            .assign_asset_class(
                AssetClassTarget::Asset {
                    asset_key: "fiat:usd".into(),
                },
                Some(PRESET_LOCAL.into()),
            )
            .unwrap();
        assert_eq!(vault.doc.holdings.len(), 2);
        let resolve = vault.slot_resolver();
        for holding in &vault.doc.holdings {
            let expected = if holding.asset.key() == "fiat:usd" {
                PRESET_LOCAL
            } else {
                PRESET_GLOBAL
            };
            assert_eq!(resolve(holding), Slot::Class(Some(expected.into())));
        }
    }

    #[test]
    fn unassigned_holdings_have_no_basket() {
        let (_dir, mut vault) = vault();
        store_with(&mut vault, "Bank", StoreKind::Bank, &[("USD", "100")]);
        let advice = vault.topup_advice(Some(amount("10")), 0, "UTC");
        assert_eq!(advice.unclassified, amount("100"));
        assert!(advice.rows.iter().all(|row| row.value.is_zero()));
    }

    #[test]
    fn fund_accounts_stay_outside() {
        let (_dir, mut vault) = vault();
        let reserve = store_with(&mut vault, "Reserve", StoreKind::Bank, &[("USD", "300")]);
        if let Some(store) = vault
            .doc
            .stores
            .iter_mut()
            .find(|store| store.id == reserve)
        {
            store.purpose = Purpose::Savings;
        }
        vault
            .set_emergency_fund(crate::model::planning::EmergencyFund {
                scope: StoreScope {
                    store_ids: vec![reserve.clone()],
                    include_children: false,
                },
                ..Default::default()
            })
            .unwrap();
        let err = vault
            .assign_asset_class(
                AssetClassTarget::Store { store_id: reserve },
                Some(PRESET_LOCAL.into()),
            )
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
        let advice = vault.topup_advice(Some(amount("10")), 0, "UTC");
        assert_eq!(advice.emergency, amount("300"));
        assert!(advice.unclassified.is_zero());
    }

    #[test]
    fn net_worth_splits_financial_by_basket() {
        let (_dir, mut vault) = vault();
        let broker = store_with(&mut vault, "Broker", StoreKind::Exchange, &[("USD", "70")]);
        store_with(&mut vault, "Bank", StoreKind::Bank, &[("USD", "30")]);
        let reserve = store_with(&mut vault, "Reserve", StoreKind::Bank, &[("USD", "50")]);
        if let Some(store) = vault
            .doc
            .stores
            .iter_mut()
            .find(|store| store.id == reserve)
        {
            store.purpose = Purpose::Savings;
        }
        vault.emergency_fund_toggle(&reserve, true).unwrap();
        vault
            .assign_asset_class(
                AssetClassTarget::Store { store_id: broker },
                Some(PRESET_GLOBAL.into()),
            )
            .unwrap();
        let worth = vault.net_worth();
        assert_eq!(worth.financial, amount("150"));
        assert_eq!(worth.financial_fund, amount("50"));
        assert_eq!(
            worth.financial_by_class,
            vec![
                crate::service::ClassShare {
                    class_id: Some(PRESET_GLOBAL.into()),
                    amount: amount("70"),
                },
                crate::service::ClassShare {
                    class_id: None,
                    amount: amount("30"),
                },
            ]
        );
    }

    #[test]
    fn deficits_share_the_amount() {
        let classes = [
            class("local", 40, false),
            class("global", 40, false),
            class("risky", 20, true),
        ];
        // 600 / 300 / 100 of 1000; +200 → 1200: targets 480 / 480 / 240.
        let values = [amount("600"), amount("300"), amount("100")];
        let adds = split_topup(&classes, &values, amount("200"));
        // Deficits: 0, 180, 140 → 200 × 180/320 and 200 × 140/320.
        assert_eq!(adds[0], (Amount::ZERO, false));
        assert_eq!(adds[1], (amount("112.50"), false));
        assert_eq!(adds[2], (amount("87.50"), false));
    }

    #[test]
    fn capped_basket_above_target_gets_nothing() {
        let classes = [class("safe", 70, false), class("risky", 30, true)];
        let values = [amount("500"), amount("500")];
        let adds = split_topup(&classes, &values, amount("100"));
        assert_eq!(adds[0], (amount("100"), false));
        assert_eq!(adds[1], (Amount::ZERO, true));
    }

    #[test]
    fn empty_portfolio_follows_the_targets() {
        let classes = [class("one", 50, false), class("two", 50, false)];
        let adds = split_topup(&classes, &[Amount::ZERO, Amount::ZERO], amount("0.03"));
        let sum = adds.iter().fold(Amount::ZERO, |sum, (add, _)| sum + *add);
        assert_eq!(sum, amount("0.03"), "rounding leftovers are not lost");
        let none = split_topup(&classes, &[Amount::ZERO, Amount::ZERO], Amount::ZERO);
        assert!(none.iter().all(|(add, _)| add.is_zero()));
    }

    #[test]
    fn advice_defaults_to_last_month_savings() {
        let (_dir, vault) = vault();
        let advice = vault.topup_advice(None, 0, "UTC");
        assert!(advice.from_savings);
        assert_eq!(advice.savings_month.as_deref(), Some("1969-12"));
        assert!(advice.amount.is_zero());
    }
}
