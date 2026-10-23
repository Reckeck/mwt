//! Bonus programmes and cashback: points kept in a loyalty wallet and valued
//! by the programme's price of a point, cashback as income in its category,
//! both tied back to the card whose purchases earned them.

use std::collections::BTreeMap;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::store::{LoyaltyInfo, Store, StoreKind};
use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, TopupTx, Transaction, TxMeta};
use crate::model::vault::{ManualPrice, now_iso};
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;

/// Income category of cashback.
pub const CASHBACK_CATEGORY: &str = "cashback";

/// Cashback and points earned through one card or account.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CashbackSource {
    /// The card that earned it; `None` when the operation does not say.
    pub store_id: Option<String>,
    pub name: String,
    pub cashback_base: Amount,
    pub points_base: Amount,
}

/// One bonus programme.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct LoyaltyBalance {
    pub store_id: String,
    pub name: String,
    pub points_symbol: String,
    pub points: Amount,
    pub value_base: Amount,
    pub expires_at: Option<String>,
    pub expiring_points: Option<Amount>,
}

/// Cashback and bonuses over a period, in the base currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CashbackReport {
    pub base_currency: String,
    pub cashback_base: Amount,
    pub points_base: Amount,
    pub by_source: Vec<CashbackSource>,
    pub programmes: Vec<LoyaltyBalance>,
}

impl UnlockedVault {
    /// Set a programme's points and price of a point. The price becomes the
    /// points' hand-set price, so the wallet counts in net worth.
    pub fn set_loyalty_info(&mut self, store_id: &str, info: LoyaltyInfo) -> Result<Store> {
        let store = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        if store.kind != StoreKind::Loyalty {
            return Err(Error::InvalidInput("not a bonus programme".into()));
        }
        let info = self.checked_loyalty_info(Some(store_id), info)?;
        let key = info.asset(&store.name).key();
        self.doc
            .manual_prices
            .retain(|price| price.asset_key != key);
        if info.point_value.is_positive() {
            self.doc.manual_prices.push(ManualPrice {
                asset_key: key,
                unit_price: info.point_value,
                currency: info.currency.clone(),
                updated_at: now_iso(),
            });
        }
        let stored = self
            .doc
            .stores
            .iter_mut()
            .find(|candidate| candidate.id == store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        stored.loyalty = Some(info);
        let saved = stored.clone();
        self.persist()?;
        Ok(saved)
    }

    /// Create a bonus programme and set its points in one action, so a
    /// programme never exists without them.
    pub fn create_loyalty_program(
        &mut self,
        name: &str,
        color: &str,
        contact_id: Option<String>,
        info: LoyaltyInfo,
    ) -> Result<Store> {
        // Checked before the wallet exists: a refusal leaves nothing behind.
        let info = self.checked_loyalty_info(None, info)?;
        let extras = super::StoreExtras {
            contact_id: contact_id.filter(|id| !id.is_empty()),
            ..super::StoreExtras::default()
        };
        let store = self.create_store_with(name, StoreKind::Loyalty, color, "", None, extras)?;
        self.set_loyalty_info(&store.id, info)
    }

    /// Normalize and check a programme's settings; `store_id` is the programme
    /// itself, `None` while it is being created.
    fn checked_loyalty_info(
        &self,
        store_id: Option<&str>,
        mut info: LoyaltyInfo,
    ) -> Result<LoyaltyInfo> {
        info.points_symbol = info.points_symbol.trim().to_uppercase();
        info.currency = info.currency.trim().to_lowercase();
        if info.points_symbol.is_empty() || info.currency.is_empty() {
            return Err(Error::InvalidInput(
                "name the points and the currency of a point".into(),
            ));
        }
        if info.point_value.is_negative() {
            return Err(Error::InvalidInput(
                "a point cannot be worth less than nothing".into(),
            ));
        }
        let taken = self.doc.stores.iter().any(|other| {
            Some(other.id.as_str()) != store_id
                && other
                    .loyalty
                    .as_ref()
                    .is_some_and(|known| known.points_symbol == info.points_symbol)
        });
        if taken {
            return Err(Error::InvalidInput(format!(
                "{} already names another programme's points",
                info.points_symbol
            )));
        }
        if let Some(expires) = info.expires_at.as_deref().filter(|raw| !raw.is_empty())
            && chrono::NaiveDate::parse_from_str(expires, "%Y-%m-%d").is_err()
        {
            return Err(Error::InvalidInput(format!(
                "burns on {expires}: expected YYYY-MM-DD"
            )));
        }
        info.login = info
            .login
            .map(|login| login.trim().to_string())
            .filter(|login| !login.is_empty());
        let mut cards: Vec<String> = Vec::new();
        for id in info.source_store_ids {
            let card = self
                .find_store(&id)
                .ok_or_else(|| Error::Missing(format!("store {id}")))?;
            if matches!(card.kind, StoreKind::Loyalty | StoreKind::Property) {
                return Err(Error::InvalidInput(format!(
                    "{} cannot earn points",
                    card.name
                )));
            }
            if !cards.contains(&id) {
                cards.push(id);
            }
        }
        info.source_store_ids = cards;
        Ok(info)
    }

    /// Credit (`delta` > 0) or spend points of a programme, optionally tied
    /// to the card whose purchase earned them.
    pub fn adjust_points(
        &mut self,
        store_id: &str,
        delta: Amount,
        comment: &str,
        source_store_id: Option<String>,
        timestamp: Option<String>,
    ) -> Result<Transaction> {
        let store = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let info = store
            .loyalty
            .clone()
            .ok_or_else(|| Error::InvalidInput("set up the programme first".into()))?;
        if delta.is_zero() {
            return Err(Error::InvalidInput("nothing to credit or spend".into()));
        }
        let asset = info.asset(&store.name);
        let mut meta = TxMeta::new(comment);
        if let Some(raw) = timestamp.filter(|raw| !raw.trim().is_empty()) {
            meta.timestamp = raw;
        }
        let details = KindDetails {
            source_store_id: source_store_id.filter(|id| !id.is_empty()),
            ..KindDetails::default()
        };
        let tx = if delta.is_positive() {
            meta.categories = vec![CASHBACK_CATEGORY.to_string()];
            Transaction::Topup(TopupTx {
                meta,
                store_id: store.id.clone(),
                asset,
                amount: delta,
                fee: None,
                from: None,
                details,
                price: PriceStamp::default(),
            })
        } else {
            Transaction::Payment(PaymentTx {
                meta,
                store_id: store.id.clone(),
                asset,
                amount: delta.abs(),
                fee: None,
                to: None,
                details,
                price: PriceStamp::default(),
            })
        };
        self.add_transaction(tx)
    }

    /// Cashback (income in the `cashback` category) and points credited to
    /// bonus programmes between two moments (RFC3339, inclusive), by card.
    pub fn cashback_report(&self, from: &str, to: &str) -> CashbackReport {
        let (base, _) = self.base_and_fx();
        let book = self.rate_book();
        let kinds: BTreeMap<&str, StoreKind> = self
            .doc
            .stores
            .iter()
            .map(|store| (store.id.as_str(), store.kind))
            .collect();
        let mut by_source: BTreeMap<Option<String>, (Amount, Amount)> = BTreeMap::new();
        for tx in &self.doc.transactions {
            let Transaction::Topup(topup) = tx else {
                continue;
            };
            let stamp = topup.meta.timestamp.as_str();
            if stamp < from || stamp > to {
                continue;
            }
            let to_loyalty = kinds.get(topup.store_id.as_str()) == Some(&StoreKind::Loyalty);
            let is_cashback = topup
                .meta
                .categories
                .iter()
                .any(|key| key == CASHBACK_CATEGORY);
            if !to_loyalty && !is_cashback {
                continue;
            }
            let secs = secs_or_now(stamp);
            let usd = book
                .price_at(&topup.asset.key(), secs)
                .unwrap_or(Amount::ZERO)
                * topup.amount;
            let value = book
                .convert(usd, "usd", &base, secs)
                .unwrap_or(Amount::ZERO);
            // Cashback credited to the card itself names that card.
            let source = topup
                .details
                .source_store_id
                .clone()
                .or_else(|| (!to_loyalty).then(|| topup.store_id.clone()));
            let entry = by_source.entry(source).or_default();
            if to_loyalty {
                entry.1 += value;
            } else {
                entry.0 += value;
            }
        }
        let name_of = |id: &Option<String>| {
            id.as_deref()
                .and_then(|id| self.find_store(id))
                .map(|store| store.name.clone())
                .unwrap_or_default()
        };
        let mut sources: Vec<CashbackSource> = by_source
            .into_iter()
            .map(|(store_id, (cashback, points))| CashbackSource {
                name: name_of(&store_id),
                store_id,
                cashback_base: cashback.round(2),
                points_base: points.round(2),
            })
            .collect();
        sources.sort_by(|left, right| {
            (right.cashback_base + right.points_base).cmp(&(left.cashback_base + left.points_base))
        });
        let fx = self.base_and_fx().1;
        let programmes = self
            .doc
            .stores
            .iter()
            .filter(|store| store.kind == StoreKind::Loyalty && !store.archived)
            .filter_map(|store| {
                let info = store.loyalty.as_ref()?;
                let asset = info.asset(&store.name);
                let points = self
                    .doc
                    .holdings
                    .iter()
                    .filter(|holding| {
                        holding.store_id == store.id && holding.asset.key() == asset.key()
                    })
                    .fold(Amount::ZERO, |sum, holding| sum + holding.quantity);
                Some(LoyaltyBalance {
                    store_id: store.id.clone(),
                    name: store.name.clone(),
                    points_symbol: info.points_symbol.clone(),
                    points,
                    value_base: (points * self.price_usd_for(&asset.key()) * fx).round(2),
                    expires_at: info.expires_at.clone(),
                    expiring_points: info.expiring_points,
                })
            })
            .collect();
        CashbackReport {
            base_currency: base,
            cashback_base: sources
                .iter()
                .fold(Amount::ZERO, |sum, row| sum + row.cashback_base),
            points_base: sources
                .iter()
                .fold(Amount::ZERO, |sum, row| sum + row.points_base),
            by_source: sources,
            programmes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::AssetRef;
    use crate::model::vault::FxPoint;
    use crate::service::test_support::*;

    fn info(symbol: &str) -> LoyaltyInfo {
        LoyaltyInfo {
            points_symbol: symbol.into(),
            point_value: amount("1"),
            currency: "rub".into(),
            expires_at: None,
            expiring_points: None,
            login: None,
            source_store_ids: Vec::new(),
            unit: Default::default(),
        }
    }

    #[test]
    fn points_are_valued_by_the_programme_and_reported_by_card() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("rub");
        vault.apply_fx(&[FxPoint {
            currency: "rub".into(),
            per_usd: amount("100"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
        }]);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let programme = store(&mut vault, "Спасибо", StoreKind::Loyalty, None);
        vault.set_loyalty_info(&programme, info("спасибо")).unwrap();
        vault
            .adjust_points(
                &programme,
                amount("500"),
                "groceries",
                Some(card.clone()),
                Some("2026-03-01T10:00:00Z".into()),
            )
            .unwrap();
        let mut cashback = income_of(
            &card,
            AssetRef::fiat("RUB", "RUB"),
            "300",
            "0.01",
            "cashback",
        );
        cashback.meta_mut().categories = vec![CASHBACK_CATEGORY.into()];
        cashback.meta_mut().timestamp = "2026-03-02T10:00:00Z".into();
        vault.add_transaction(cashback).unwrap();

        let report = vault.cashback_report("2026-03-01T00:00:00Z", "2026-03-31T23:59:59Z");
        assert_eq!(report.cashback_base, amount("300"));
        assert_eq!(report.points_base, amount("500"));
        assert_eq!(report.by_source.len(), 1, "both earned through the card");
        assert_eq!(report.by_source[0].store_id.as_deref(), Some(card.as_str()));
        assert_eq!(report.programmes[0].points, amount("500"));
        assert_eq!(report.programmes[0].value_base, amount("500"));
    }

    #[test]
    fn programmes_keep_their_points_apart() {
        let (_dir, mut vault) = empty_vault();
        let first = store(&mut vault, "A", StoreKind::Loyalty, None);
        let second = store(&mut vault, "B", StoreKind::Loyalty, None);
        vault.set_loyalty_info(&first, info("PTS")).unwrap();
        assert!(vault.set_loyalty_info(&second, info("pts")).is_err());
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        assert!(vault.set_loyalty_info(&bank, info("X")).is_err());
    }

    #[test]
    fn a_programme_is_created_with_its_points_or_not_at_all() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let mut settings = info("миля");
        settings.login = Some("  +7 900 000-00-00 ".into());
        settings.source_store_ids = vec![card.clone(), card.clone()];
        let created = vault
            .create_loyalty_program("Аэрофлот", "#db2777", None, settings)
            .unwrap();
        assert_eq!(created.kind, StoreKind::Loyalty);
        let saved = created.loyalty.unwrap();
        assert_eq!(saved.points_symbol, "МИЛЯ");
        assert_eq!(saved.login.as_deref(), Some("+7 900 000-00-00"));
        assert_eq!(saved.source_store_ids, vec![card]);

        let before = vault.doc.stores.len();
        let taken = vault.create_loyalty_program("Другая", "#db2777", None, info("МИЛЯ"));
        assert!(taken.is_err());
        let mut bad_card = info("БАЛЛ");
        bad_card.source_store_ids = vec![created.id.clone()];
        assert!(
            vault
                .create_loyalty_program("Третья", "#db2777", None, bad_card)
                .is_err()
        );
        assert_eq!(
            vault.doc.stores.len(),
            before,
            "a refusal leaves no wallet behind"
        );
    }

    #[test]
    fn programmes_saved_before_the_login_field_still_open() {
        let raw = r#"{"points_symbol":"СПАСИБО","point_value":"1","currency":"rub"}"#;
        let old: LoyaltyInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(old.login, None);
        assert!(old.source_store_ids.is_empty());
    }
}
