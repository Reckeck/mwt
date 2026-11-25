//! What a period or a spread would do, asked before anything is saved (spec op-period I5-I7).
//! Nothing here writes: the period of a rule, the shares of a spread, and the months a changed
//! operation counts in before and after.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{MonthEffect, PeriodEffect};
use crate::model::device_secrets::DeviceSecrets;
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;
use crate::money::books::{Books, BooksSource};
use crate::money::interest::calendar_of;
use crate::money::validate::check_period_meta;
use crate::money::valuation::ValuationCurve;
use crate::planning::budget::budget_currency;
use crate::planning::flows::{Basis, flows_for_basis};
use crate::planning::period_rule::{period_of_rule, settle_spread};
use crate::service::Session;
use crate::usecases::money::transactions::{settle_tx_spread, spreadable};
use mwt_model::{
    AssetRef, CryptoHistory, MonthShare, OpPeriod, PeriodRule, Spread, SpreadMode, TxKind, TxMeta,
};

/// I5: the period a rule gives an operation of `money_date` (`YYYY-MM-DD`).
pub fn resolve_period_rule(rule: PeriodRule, money_date: &str) -> Result<OpPeriod> {
    let day = NaiveDate::parse_from_str(money_date, "%Y-%m-%d").map_err(|_| {
        Error::InvalidInput(format!("money date {money_date}: expected YYYY-MM-DD"))
    })?;
    Ok(period_of_rule(rule, day))
}

/// I6: the shares `mode` gives `amount` over `period`, cut to the places of `asset`. `shares` are
/// the manual ones the user typed. A refusal of the save (a one-month period, shares that do not
/// add up) is the refusal here.
pub fn preview_spread(
    amount: Amount,
    asset: &AssetRef,
    period: &OpPeriod,
    mode: SpreadMode,
    shares: Option<Vec<MonthShare>>,
) -> Result<Vec<MonthShare>> {
    let given = Spread {
        mode,
        shares: shares.unwrap_or_default(),
    };
    let settled = settle_spread(
        Some(period),
        Some(given),
        amount,
        asset.minor_places(),
        true,
        None,
    );
    let mut meta = TxMeta::new("");
    meta.period = Some(period.clone());
    meta.spread = settled.clone();
    check_period_meta(TxKind::Payment, &meta, Some(amount))?;
    Ok(settled.map(|spread| spread.shares).unwrap_or_default())
}

/// The stored operation `id` as an edit would leave it: `period` and `spread` replace the stored
/// ones when given, and the shares are settled against the stored record (P8).
fn edited(
    stored: &Transaction,
    period: Option<OpPeriod>,
    spread: Option<Spread>,
) -> Result<Transaction> {
    let Some((amount, _)) = spreadable(stored) else {
        return Err(Error::InvalidInput(
            "only an income or an expense has a period".into(),
        ));
    };
    let mut tx = stored.clone();
    let meta = tx.meta_mut();
    if period.is_some() {
        meta.period = period;
    }
    if spread.is_some() {
        meta.spread = spread;
    }
    settle_tx_spread(&mut tx, true, Some(stored));
    check_period_meta(tx.kind(), tx.meta(), Some(amount))?;
    Ok(tx)
}

/// The vault as it would be with one operation edited: the document differs, every read of the
/// open vault stays its own.
struct EditedBooks<'vault> {
    vault: &'vault Session,
    doc: VaultDoc,
}

impl BooksSource for EditedBooks<'_> {
    fn doc(&self) -> &VaultDoc {
        &self.doc
    }

    fn base_and_fx(&self) -> (String, Amount) {
        BooksSource::base_and_fx(self.vault)
    }

    fn price_usd_for(&self, asset_key: &str) -> Amount {
        BooksSource::price_usd_for(self.vault, asset_key)
    }

    fn price_base_for(&self, asset_key: &str, fx: Amount) -> Amount {
        BooksSource::price_base_for(self.vault, asset_key, fx)
    }

    fn valuation_curves(&self) -> std::collections::HashMap<String, ValuationCurve> {
        BooksSource::valuation_curves(self.vault)
    }

    fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()> {
        BooksSource::prepare_transaction(self.vault, tx)
    }

    fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount {
        BooksSource::earmarked(self.vault, store_id, asset_key)
    }

    fn keys_on_device(&self) -> bool {
        BooksSource::keys_on_device(self.vault)
    }

    fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        BooksSource::device_secrets_for_request(self.vault)
    }

    fn crypto_history(&self, cutoffs: &[i64], in_scope: &dyn Fn(&str) -> bool) -> CryptoHistory {
        BooksSource::crypto_history(self.vault, cutoffs, in_scope)
    }
}

/// What operation `id` counts for in each month, read by the period (R2) in the budget currency.
fn months_of(books: &Books<'_>, id: &str) -> BTreeMap<String, Amount> {
    let calendar = calendar_of(books.doc());
    let mut months = BTreeMap::new();
    for flow in flows_for_basis(
        books,
        &calendar,
        &budget_currency(books),
        i64::MIN,
        i64::MAX,
        Basis::Period,
    ) {
        if flow.tx_id == id {
            *months.entry(flow.month).or_insert(Amount::ZERO) += flow.value;
        }
    }
    months
}

/// I7: the months operation `id` counts in before and after its period and spread become
/// `period` and `spread` (`None` keeps the stored one). Refused as the save would be.
pub fn preview_period_change(
    vault: &Session,
    id: &str,
    period: Option<OpPeriod>,
    spread: Option<Spread>,
) -> Result<PeriodEffect> {
    let index = vault
        .doc
        .transactions
        .iter()
        .position(|known| known.meta().id == id)
        .ok_or_else(|| Error::Missing("transaction".into()))?;
    let changed = edited(&vault.doc.transactions[index], period, spread)?;
    let mut doc = vault.doc.clone();
    doc.transactions[index] = changed;
    let before = months_of(&vault.books(), id);
    let after_source = EditedBooks { vault, doc };
    let after = months_of(&Books::new(&after_source), id);
    let mut months: Vec<String> = before.keys().chain(after.keys()).cloned().collect();
    months.sort();
    months.dedup();
    let value_in = |side: &BTreeMap<String, Amount>, month: &str| {
        side.get(month).copied().unwrap_or(Amount::ZERO)
    };
    Ok(PeriodEffect {
        months: months
            .into_iter()
            .map(|month| MonthEffect {
                before: value_in(&before, &month),
                after: value_in(&after, &month),
                month,
            })
            .filter(|row| !(row.before.is_zero() && row.after.is_zero()))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;
    use crate::usecases::money::transactions::add_transaction;
    use mwt_model::PeriodKind;

    fn day(year: i32, month: u32, date: u32) -> String {
        format!("{year:04}-{month:02}-{date:02}")
    }

    fn quarter() -> OpPeriod {
        OpPeriod {
            kind: PeriodKind::Quarter,
            start: day(2026, 10, 1),
            end: day(2026, 12, 31),
        }
    }

    fn shares(rows: &[(&str, &str)]) -> Vec<MonthShare> {
        rows.iter()
            .map(|(month, raw)| MonthShare {
                month: (*month).into(),
                amount: amount(raw),
            })
            .collect()
    }

    fn even_hundred() -> Vec<MonthShare> {
        shares(&[
            ("2026-10", "33.33"),
            ("2026-11", "33.33"),
            ("2026-12", "33.34"),
        ])
    }

    #[test]
    fn i5_prev_half_second_on_the_fifth_of_november_is_the_second_half_of_october() {
        let period = resolve_period_rule(PeriodRule::PrevHalfSecond, "2026-11-05").unwrap();
        assert_eq!(period.kind, PeriodKind::Half);
        assert_eq!(
            (period.start.as_str(), period.end.as_str()),
            ("2026-10-16", "2026-10-31")
        );
        assert!(resolve_period_rule(PeriodRule::ThisMonth, "05.11.2026").is_err());
    }

    #[test]
    fn i6_even_spread_of_a_hundred_over_a_quarter_keeps_the_rest_in_the_last_month() {
        let rows =
            preview_spread(amount("100.00"), &usd(), &quarter(), SpreadMode::Even, None).unwrap();
        assert_eq!(rows, even_hundred());
    }

    #[test]
    fn i6_manual_shares_that_do_not_add_up_and_a_one_month_period_are_refused() {
        let wrong = shares(&[("2026-10", "50"), ("2026-11", "40"), ("2026-12", "9")]);
        assert!(
            preview_spread(
                amount("100.00"),
                &usd(),
                &quarter(),
                SpreadMode::Manual,
                Some(wrong)
            )
            .is_err()
        );
        let month = OpPeriod {
            kind: PeriodKind::Month,
            start: day(2026, 10, 1),
            end: day(2026, 10, 31),
        };
        assert!(preview_spread(amount("100.00"), &usd(), &month, SpreadMode::Even, None).is_err());
    }

    fn vault_with_expense() -> (tempfile::TempDir, Session, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let funding = add_transaction(&mut vault, income(&card, "1000", "salary")).unwrap();
        let mut expense = payment(&card, "100", "rent");
        at(&mut expense, "2026-11-05T10:00:00Z");
        let saved = add_transaction(&mut vault, expense).unwrap();
        assert_ne!(funding.meta().id, saved.meta().id);
        (dir, vault, saved.meta().id.clone())
    }

    #[test]
    fn i7_a_quarter_spread_moves_a_november_expense_into_three_months() {
        let (_dir, vault, id) = vault_with_expense();
        let spread = Spread {
            mode: SpreadMode::Even,
            shares: Vec::new(),
        };
        let effect = preview_period_change(&vault, &id, Some(quarter()), Some(spread)).unwrap();
        let rows: Vec<(&str, String, String)> = effect
            .months
            .iter()
            .map(|row| {
                (
                    row.month.as_str(),
                    row.before.to_string(),
                    row.after.to_string(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("2026-10", "0".to_string(), "33.33".to_string()),
                ("2026-11", "100".to_string(), "33.33".to_string()),
                ("2026-12", "0".to_string(), "33.34".to_string()),
            ]
        );
        let stored = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == id)
            .unwrap();
        assert!(stored.meta().period.is_none(), "a preview writes nothing");
    }

    #[test]
    fn i7_an_unknown_id_is_refused() {
        let (_dir, vault, _id) = vault_with_expense();
        assert!(preview_period_change(&vault, "missing", Some(quarter()), None).is_err());
    }
}
