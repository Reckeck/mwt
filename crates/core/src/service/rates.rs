//! Rates at a moment (plan §6.1–§6.2): the FX and price history answered as
//! "what was it then", the price stamp a transaction gets when saved, and the
//! conversion the budget uses for its own currency (D20).

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::ipc_contract::CachedPrice;
use crate::model::store::AssetRef;
use crate::model::transaction::{Leg, Transaction};
use crate::model::vault::{FxPoint, PricePoint};
use crate::pricing::history::{self, RateIndex, secs_or_now};

use super::UnlockedVault;
use super::valuation::ValuationCurve;

const FIAT_PREFIX: &str = "fiat:";
const PERP_PREFIX: &str = "perp:";

/// Everything a replay needs to price things at past moments, parsed once.
#[derive(Debug, Default, Clone)]
pub struct RateBook {
    fx: RateIndex,
    prices: RateIndex,
    /// Prices transactions recorded, for assets no provider quotes.
    stamps: RateIndex,
    /// Hand-set unit prices: `(price, currency)` per asset key (D24).
    manual: HashMap<String, (Amount, String)>,
    /// Dated property valuations (G3); win over `manual`.
    curves: HashMap<String, ValuationCurve>,
    /// Asset key by upper-case symbol, so a currency code can name a crypto asset.
    keys_by_symbol: HashMap<String, String>,
}

impl RateBook {
    /// Units of `currency` per USD at `secs`; USD itself is 1.
    pub fn fx_at(&self, currency: &str, secs: i64) -> Option<Amount> {
        let currency = currency.trim().to_lowercase();
        if currency == "usd" {
            return Some(one());
        }
        self.fx.at(&currency, secs)
    }

    /// USD price of one unit at `secs`: a hand-set price (whole history),
    /// fiat through its FX rate, else the cached quote nearest that moment.
    /// Derivatives have none — their notional is not the user's money.
    pub fn price_at(&self, asset_key: &str, secs: i64) -> Option<Amount> {
        if asset_key.starts_with(PERP_PREFIX) {
            return None;
        }
        if let Some(valued) = self.curve_usd(asset_key, secs) {
            return Some(valued);
        }
        if let Some(manual) = self.manual_usd(asset_key) {
            return Some(manual);
        }
        if let Some(code) = asset_key.strip_prefix(FIAT_PREFIX) {
            return self.fx_at(code, secs).map(|rate| one() / rate);
        }
        self.prices.at(asset_key, secs)
    }

    /// [`RateBook::price_at`] with the moment its rate was taken; `None` for
    /// the moment of a hand-set price, which holds for all time.
    pub fn quote_at(&self, asset_key: &str, secs: i64) -> Option<(Amount, Option<i64>)> {
        if asset_key.starts_with(PERP_PREFIX) {
            return None;
        }
        if let Some(valued) = self.curve_usd(asset_key, secs) {
            return Some((valued, None));
        }
        if let Some(manual) = self.manual_usd(asset_key) {
            return Some((manual, None));
        }
        if let Some(code) = asset_key.strip_prefix(FIAT_PREFIX) {
            let code = code.to_lowercase();
            if code == "usd" {
                return Some((one(), None));
            }
            let (at, rate) = self.fx.point_at(&code, secs)?;
            return (!rate.is_zero()).then(|| (one() / rate, Some(at)));
        }
        let (at, price) = self.prices.point_at(asset_key, secs)?;
        Some((price, Some(at)))
    }

    /// The price a history replay uses: nothing from the future while
    /// anything from the past is known, the transactions' own stamps
    /// included, and only then the first later quote.
    pub fn history_price(&self, asset_key: &str, secs: i64) -> Option<Amount> {
        if asset_key.starts_with(PERP_PREFIX) {
            return None;
        }
        if let Some(valued) = self.curve_usd(asset_key, secs) {
            return Some(valued);
        }
        if let Some(manual) = self.manual_usd(asset_key) {
            return Some(manual);
        }
        if let Some(code) = asset_key.strip_prefix(FIAT_PREFIX) {
            return self.fx_at(code, secs).map(|rate| one() / rate);
        }
        self.prices
            .at_or_before(asset_key, secs)
            .or_else(|| self.stamps.at_or_before(asset_key, secs))
            .or_else(|| self.prices.at(asset_key, secs))
    }

    /// `amount` of `from` in `to` at `secs`, both currency codes (fiat, or the
    /// symbol of a crypto asset the vault knows). Crosses go through USD.
    pub fn convert(&self, amount: Amount, from: &str, to: &str, secs: i64) -> Option<Amount> {
        let from = from.trim().to_lowercase();
        let to = to.trim().to_lowercase();
        if from == to {
            return Some(amount);
        }
        let usd = amount * self.code_usd(&from, secs)?;
        let to_unit = self.code_usd(&to, secs)?;
        (!to_unit.is_zero()).then(|| usd / to_unit)
    }

    /// Price of one unit of a leg's asset in `base` at the moment of the
    /// movement: an explicit base price, then a hand-entered USD→base rate,
    /// then the base rate of that day. A USD price with no known rate still
    /// falls through as the USD value rather than zero: a wrong magnitude is
    /// easier to notice than a silent zero.
    ///
    /// An explicit base price carries the base currency of the device that
    /// wrote it. When that is not the reader's base, the USD price is
    /// converted at the rate of the moment instead; a record without the
    /// label (older ones) is read as written.
    pub fn leg_price_in_base(&self, leg: &Leg, base: &str, secs: i64) -> Amount {
        if let Some(price_base) = leg.price_base {
            let foreign = leg
                .price_base_currency
                .as_deref()
                .is_some_and(|label| !label.trim().eq_ignore_ascii_case(base.trim()));
            let converted = leg
                .price_usd
                .filter(|_| foreign)
                .and_then(|usd| self.fx_at(base, secs).map(|rate| usd * rate));
            return converted.unwrap_or(price_base);
        }
        match (leg.price_usd, leg.fx_override) {
            (Some(usd), Some(fx)) => usd * fx,
            (Some(usd), None) => self.fx_at(base, secs).map_or(usd, |rate| usd * rate),
            _ => Amount::ZERO,
        }
    }

    /// USD per one unit of a currency code.
    fn code_usd(&self, code: &str, secs: i64) -> Option<Amount> {
        if let Some(rate) = self.fx_at(code, secs).filter(|rate| !rate.is_zero()) {
            return Some(one() / rate);
        }
        let key = self.keys_by_symbol.get(&code.to_uppercase())?;
        self.price_at(key, secs)
    }

    /// USD of a valued property unit at `secs`, through the rate of that
    /// moment (the latest one when the history has none).
    fn curve_usd(&self, asset_key: &str, secs: i64) -> Option<Amount> {
        let curve = self.curves.get(asset_key)?;
        let value = curve.value_at(secs)?;
        let rate = self
            .fx_at(&curve.currency, secs)
            .or_else(|| self.fx.latest(&curve.currency))?;
        (!rate.is_zero()).then(|| value / rate)
    }

    fn manual_usd(&self, asset_key: &str) -> Option<Amount> {
        let (price, currency) = self.manual.get(asset_key)?;
        let currency = currency.to_lowercase();
        let rate = if currency == "usd" {
            one()
        } else {
            self.fx.latest(&currency)?
        };
        (!rate.is_zero()).then(|| *price / rate)
    }
}

pub(crate) fn iso_of(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|moment| moment.format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn one() -> Amount {
    Amount::new(rust_decimal::Decimal::ONE)
}

impl UnlockedVault {
    /// Parse the caches once for a batch of lookups.
    pub fn rate_book(&self) -> RateBook {
        let mut stamps = RateIndex::default();
        let mut keys_by_symbol = HashMap::new();
        for holding in &self.doc.holdings {
            keys_by_symbol
                .entry(holding.asset.symbol.to_uppercase())
                .or_insert_with(|| holding.asset.key());
        }
        // A favourite coin the vault does not hold is still convertible once
        // its quote is cached.
        for (ticker, coingecko_id, _) in crate::exchange::well_known_coins() {
            keys_by_symbol
                .entry(ticker.to_string())
                .or_insert_with(|| format!("coin:{coingecko_id}"));
        }
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            for leg in tx.legs() {
                if let Some(price) = leg.price_usd {
                    stamps.insert(&leg.asset.key(), secs, price);
                }
            }
        }
        stamps.sort();
        RateBook {
            fx: RateIndex::of_fx(&self.doc.fx_cache),
            prices: RateIndex::of_prices(&self.doc.price_cache),
            stamps,
            curves: self.valuation_curves(),
            manual: self
                .doc
                .manual_prices
                .iter()
                .map(|price| {
                    (
                        price.asset_key.clone(),
                        (price.unit_price, price.currency.clone()),
                    )
                })
                .collect(),
            keys_by_symbol,
        }
    }

    /// Units of `currency` per USD at `ts` (RFC3339; empty = now).
    pub fn fx_at(&self, currency: &str, ts: &str) -> Option<Amount> {
        self.rate_book().fx_at(currency, secs_or_now(ts))
    }

    /// USD price of one unit of an asset at `ts` (RFC3339; empty = now).
    pub fn price_at(&self, asset_key: &str, ts: &str) -> Option<Amount> {
        self.rate_book().price_at(asset_key, secs_or_now(ts))
    }

    /// `amount` of one currency in another at `ts` (RFC3339; empty = now).
    pub fn convert_at(&self, amount: Amount, from: &str, to: &str, ts: &str) -> Option<Amount> {
        self.rate_book().convert(amount, from, to, secs_or_now(ts))
    }

    /// The cached USD quote of an asset nearest `ts` (RFC3339; empty = now),
    /// with the moment that rate was taken.
    pub fn cached_quote(&self, asset: &AssetRef, ts: &str) -> CachedPrice {
        let secs = secs_or_now(ts);
        match self.rate_book().quote_at(&asset.key(), secs) {
            Some((price_usd, taken)) => CachedPrice {
                price_usd,
                ts: iso_of(taken.unwrap_or(secs)),
            },
            None => CachedPrice::default(),
        }
    }

    /// Currencies whose rate history matters: the base and budget
    /// currencies, every fiat the vault holds or moved, and the currencies of
    /// hand-set prices. The rest of the table keeps only its latest rate.
    pub fn fx_currencies_in_use(&self) -> HashSet<String> {
        let mut out: HashSet<String> = HashSet::from(["usd".to_string()]);
        out.insert(self.base_and_fx().0);
        let budget = self.doc.settings.budget_currency.trim().to_lowercase();
        if !budget.is_empty() {
            out.insert(budget);
        }
        for holding in &self.doc.holdings {
            if holding.asset.is_fiat() {
                out.insert(holding.asset.symbol.to_lowercase());
            }
        }
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                if leg.asset.is_fiat() {
                    out.insert(leg.asset.symbol.to_lowercase());
                }
            }
        }
        for price in &self.doc.manual_prices {
            out.insert(price.currency.to_lowercase());
        }
        for valuation in &self.doc.valuations {
            out.insert(valuation.currency.to_lowercase());
        }
        out
    }

    /// Turn a price typed in any currency into the USD price the ledger keeps,
    /// at the rate of the transaction's moment (the last one at or before it,
    /// else the first after). A price in the base currency is also kept as is.
    ///
    /// Fails with `FX_MISSING` when the currency has no rate at all: guessing 1
    /// would silently store rubles as dollars.
    pub fn resolve_input_price(
        &self,
        input: &mut crate::ipc_contract::TransactionInput,
    ) -> crate::Result<()> {
        let common = input.common_mut();
        let Some(price) = common.price.take() else {
            return Ok(());
        };
        let base = self.base_and_fx().0;
        let currency = common
            .price_currency
            .as_deref()
            .map(|code| code.trim().to_lowercase())
            .filter(|code| !code.is_empty())
            .unwrap_or_else(|| base.clone());
        let secs = secs_or_now(common.timestamp.as_deref().unwrap_or_default());
        let rate = self
            .rate_book()
            .fx_at(&currency, secs)
            .filter(|rate| rate.is_positive())
            .ok_or_else(|| crate::Error::FxMissing(currency.clone()))?;
        common.price_usd = Some(price / rate);
        if currency == base {
            common.price_base = Some(price);
            common.price_base_currency = Some(base);
        }
        Ok(())
    }

    /// A code that names a coin rather than a fiat currency: no FX rate is
    /// cached for it, and the vault holds a coin with that ticker or it is a
    /// well-known one. Its rate comes from the price cache, not FX sources.
    pub fn is_crypto_code(&self, code: &str) -> bool {
        let lower = code.trim().to_lowercase();
        if lower.is_empty() || lower == "usd" {
            return false;
        }
        if self
            .doc
            .fx_cache
            .iter()
            .any(|point| point.currency == lower)
        {
            return false;
        }
        self.held_coin(&lower).is_some() || crate::exchange::well_known_coin(&lower).is_some()
    }

    /// The asset a crypto favourite is priced as: the identity the vault
    /// already holds, else the well-known coin. `None` for fiat and unknowns.
    pub fn favorite_asset(&self, code: &str) -> Option<AssetRef> {
        if !self.is_crypto_code(code) {
            return None;
        }
        if let Some(asset) = self.held_coin(code) {
            return Some(asset.clone());
        }
        let upper = code.trim().to_uppercase();
        let (coingecko_id, name) = crate::exchange::well_known_coin(&upper)?;
        Some(AssetRef {
            chain: None,
            ..AssetRef::crypto(
                &upper,
                name,
                Some(coingecko_id),
                crate::model::chain::Chain::Fiat,
            )
        })
    }

    pub(crate) fn held_coin(&self, code: &str) -> Option<&AssetRef> {
        self.doc
            .holdings
            .iter()
            .map(|holding| &holding.asset)
            .find(|asset| {
                asset.is_crypto()
                    && !asset.is_derivative()
                    && asset.symbol.eq_ignore_ascii_case(code.trim())
            })
    }

    /// Which of `currencies` have no cached rate taken on `date` (UTC day).
    /// USD never needs one, and coins are priced, not looked up as FX.
    pub fn fx_missing_on(&self, currencies: &[String], date: chrono::NaiveDate) -> Vec<String> {
        let day = date.format("%Y-%m-%d").to_string();
        let mut out: Vec<String> = currencies
            .iter()
            .map(|code| code.trim().to_lowercase())
            .filter(|code| !code.is_empty() && code != "usd")
            .filter(|code| !self.is_crypto_code(code))
            .filter(|code| {
                !self.doc.fx_cache.iter().any(|point| {
                    &point.currency == code
                        && point.ts.starts_with(&day)
                        && point.per_usd.is_positive()
                })
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Add a fetched FX table to the cache. Does not persist: the refresh
    /// that fetched it writes once at the end.
    pub fn apply_fx(&mut self, fresh: &[FxPoint]) {
        let keep_history = self.fx_history_filter();
        history::merge_fx(&mut self.doc.fx_cache, fresh, keep_history);
    }

    /// Which currencies keep their rate history: the ones in use. Owns its
    /// data, so it can be used while the cache is borrowed mutably.
    pub(crate) fn fx_history_filter(&self) -> impl Fn(&str) -> bool + use<> {
        let tracked = self.fx_currencies_in_use();
        move |code| tracked.contains(code)
    }

    /// Thin both rate caches by the refresh rules, adding nothing.
    pub(crate) fn thin_rate_caches(&mut self) {
        let keep_history = self.fx_history_filter();
        history::thin_fx(&mut self.doc.fx_cache, keep_history);
        history::thin_prices(&mut self.doc.price_cache);
    }

    /// Add fetched quotes to the cache. Does not persist.
    pub fn apply_prices(&mut self, fresh: &[PricePoint]) {
        history::merge_prices(&mut self.doc.price_cache, fresh);
    }

    /// Give every unpriced side the USD price of its moment (plan §6.2).
    /// A price already recorded — typed by the user, imported — is kept.
    /// Returns how many sides got a price.
    pub(crate) fn stamp_prices(book: &RateBook, tx: &mut Transaction) -> usize {
        let secs = secs_or_now(&tx.meta().timestamp);
        let mut stamped = 0;
        for (asset, stamp) in tx.stamps_mut() {
            if stamp.price_usd.is_some() || stamp.price_base.is_some() {
                continue;
            }
            if let Some(price) = book.price_at(&asset.key(), secs) {
                stamp.price_usd = Some(price);
                stamped += 1;
            }
        }
        stamped
    }

    /// Stamp what was saved without a price — imported, automated, or
    /// entered before the rate was cached. Run after a refresh; does not
    /// persist. Returns how many sides got a price.
    pub fn restamp_missing(&mut self) -> usize {
        let book = self.rate_book();
        let mut stamped = 0;
        for tx in &mut self.doc.transactions {
            stamped += Self::stamp_prices(&book, tx);
        }
        if stamped > 0 {
            self.recompute_holdings();
        }
        stamped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::{PriceStamp, Transaction};
    use crate::service::test_support::*;

    fn fx(currency: &str, per_usd: &str, ts: &str) -> FxPoint {
        FxPoint {
            currency: currency.into(),
            per_usd: amount(per_usd),
            ts: ts.into(),
            source: "test".into(),
        }
    }

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "Russian ruble")
    }

    /// RUB income on `ts` with no price typed.
    fn rub_income(store_id: &str, sum: &str, ts: &str) -> Transaction {
        let mut tx = income_of(store_id, rub(), sum, "1", "salary");
        if let Transaction::Topup(inner) = &mut tx {
            inner.price = PriceStamp::default();
        }
        at(&mut tx, ts);
        tx
    }

    fn first_price(vault: &mut UnlockedVault) -> Option<Amount> {
        vault.doc.transactions[0].stamps_mut()[0].1.price_usd
    }

    /// RUB is 80 per USD in January and 100 from June. RUB is the base and
    /// THB the budget currency, so both keep their history.
    fn with_rub_history(vault: &mut UnlockedVault) {
        vault.base_currency = "rub".into();
        vault.doc.settings.budget_currency = "thb".into();
        vault.apply_fx(&[
            fx("rub", "80", "2026-01-01T00:00:00Z"),
            fx("thb", "32", "2026-01-01T00:00:00Z"),
        ]);
        vault.apply_fx(&[
            fx("rub", "100", "2026-06-01T00:00:00Z"),
            fx("thb", "40", "2026-06-01T00:00:00Z"),
        ]);
    }

    #[test]
    fn convert_crosses_through_usd_at_the_date() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        // 8000 RUB in January = 100 USD = 3200 THB; in July 8000 RUB = 80 USD.
        assert_eq!(
            vault.convert_at(amount("8000"), "rub", "thb", "2026-01-15T00:00:00Z"),
            Some(amount("3200"))
        );
        assert_eq!(
            vault.convert_at(amount("8000"), "RUB", "usd", "2026-07-01T00:00:00Z"),
            Some(amount("80"))
        );
        assert_eq!(vault.convert_at(amount("1"), "rub", "xyz", ""), None);
    }

    #[test]
    fn saving_stamps_the_rate_of_that_day_and_keeps_a_typed_price() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        with_rub_history(&mut vault);

        vault
            .add_transaction(rub_income(&card, "8000", "2026-02-01T00:00:00Z"))
            .unwrap();
        assert_eq!(first_price(&mut vault), Some(amount("0.0125")));

        let mut typed = rub_income(&card, "100", "2026-02-02T00:00:00Z");
        if let Transaction::Topup(inner) = &mut typed {
            inner.price.price_usd = Some(amount("0.02"));
        }
        let mut saved = vault.add_transaction(typed).unwrap();
        assert_eq!(saved.stamps_mut()[0].1.price_usd, Some(amount("0.02")));
    }

    #[test]
    fn a_fiat_holding_has_value() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault.apply_fx(&[fx("rub", "100", "2026-01-01T00:00:00Z")]);
        vault
            .add_transaction(rub_income(&card, "10000", "2026-02-01T00:00:00Z"))
            .unwrap();
        let worth = vault.net_worth();
        assert!(worth.financial.is_positive(), "{worth:?}");
    }

    #[test]
    fn restamp_fills_only_what_is_missing() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        vault
            .add_transaction(rub_income(&card, "8000", "2026-02-01T00:00:00Z"))
            .unwrap();
        assert_eq!(first_price(&mut vault), None);

        vault.apply_fx(&[fx("rub", "80", "2026-01-01T00:00:00Z")]);
        assert_eq!(vault.restamp_missing(), 1);
        assert_eq!(vault.restamp_missing(), 0);
        assert_eq!(first_price(&mut vault), Some(amount("0.0125")));
    }

    #[test]
    fn leg_price_in_base_uses_the_rate_of_the_moment() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        let book = vault.rate_book();
        let leg = Leg::in_coming("s", &usd(), amount("1"), &usd_price("1"));
        let feb = secs_or_now("2026-02-01T00:00:00Z");
        let jul = secs_or_now("2026-07-01T00:00:00Z");
        assert_eq!(book.leg_price_in_base(&leg, "rub", feb), amount("80"));
        assert_eq!(book.leg_price_in_base(&leg, "rub", jul), amount("100"));
    }

    /// A stamp whose base price was written by a device with base `label`.
    fn labelled_stamp(usd: &str, in_base: &str, label: Option<&str>) -> PriceStamp {
        PriceStamp {
            price_usd: Some(amount(usd)),
            price_base: Some(amount(in_base)),
            price_base_currency: label.map(str::to_string),
            fx_override: None,
        }
    }

    #[test]
    fn a_base_price_of_another_currency_is_read_through_usd() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        vault.apply_fx(&[fx("eur", "0.5", "2026-01-01T00:00:00Z")]);
        let book = vault.rate_book();
        let feb = secs_or_now("2026-02-01T00:00:00Z");
        // Written on a device whose base is rub: 100 USD = 8000 RUB in February.
        let stamp = labelled_stamp("100", "8000", Some("rub"));
        let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
        // The reader's base is EUR (0.5 per USD): 50, not the 8000 rubles.
        assert_eq!(book.leg_price_in_base(&leg, "eur", feb), amount("50"));
        // The label is compared without regard to case.
        let upper = labelled_stamp("100", "8000", Some("RUB"));
        let same = Leg::in_coming("s", &usd(), amount("1"), &upper);
        assert_eq!(book.leg_price_in_base(&same, "rub", feb), amount("8000"));
    }

    #[test]
    fn a_base_price_with_the_readers_label_is_kept_as_is() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        let book = vault.rate_book();
        let feb = secs_or_now("2026-02-01T00:00:00Z");
        let stamp = labelled_stamp("100", "7777", Some("rub"));
        let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
        assert_eq!(book.leg_price_in_base(&leg, "rub", feb), amount("7777"));
    }

    #[test]
    fn a_base_price_without_a_label_reads_as_before() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        let book = vault.rate_book();
        let feb = secs_or_now("2026-02-01T00:00:00Z");
        let stamp = labelled_stamp("100", "7777", None);
        let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
        assert_eq!(book.leg_price_in_base(&leg, "thb", feb), amount("7777"));
    }

    #[test]
    fn a_labelled_base_price_with_no_usd_price_or_rate_is_kept() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        let book = vault.rate_book();
        let feb = secs_or_now("2026-02-01T00:00:00Z");
        let mut stamp = labelled_stamp("100", "8000", Some("rub"));
        stamp.price_usd = None;
        let no_usd = Leg::in_coming("s", &usd(), amount("1"), &stamp);
        assert_eq!(book.leg_price_in_base(&no_usd, "thb", feb), amount("8000"));
        // No rate for the reader's base: the number is kept, as for a USD price.
        let stamp = labelled_stamp("100", "8000", Some("rub"));
        let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
        assert_eq!(book.leg_price_in_base(&leg, "xyz", feb), amount("8000"));
    }

    #[test]
    fn a_price_typed_in_the_base_currency_is_labelled_with_it() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("rub");
        vault.apply_fx(&[
            fx("rub", "80", "2026-01-01T00:00:00Z"),
            fx("thb", "32", "2026-01-01T00:00:00Z"),
        ]);
        let mut input = priced_payment("8000", "rub", "2026-02-01T00:00:00Z");
        vault.resolve_input_price(&mut input).unwrap();
        assert_eq!(
            input.common_mut().price_base_currency.as_deref(),
            Some("rub")
        );
        // Not the base: no base price, so no label.
        let mut other = priced_payment("3200", "thb", "2026-02-01T00:00:00Z");
        vault.resolve_input_price(&mut other).unwrap();
        assert_eq!(other.common_mut().price_base, None);
        assert_eq!(other.common_mut().price_base_currency, None);
    }

    #[test]
    fn untracked_currencies_do_not_grow_history() {
        let (_dir, mut vault) = empty_vault();
        vault.apply_fx(&[fx("jpy", "150", "2026-01-01T00:00:00Z")]);
        vault.apply_fx(&[fx("jpy", "151", "2026-02-01T00:00:00Z")]);
        let jpy: Vec<_> = vault
            .doc
            .fx_cache
            .iter()
            .filter(|point| point.currency == "jpy")
            .collect();
        assert_eq!(jpy.len(), 1);
        assert_eq!(jpy[0].per_usd, amount("151"));
    }

    #[test]
    fn a_hand_set_price_holds_for_the_whole_history() {
        let (_dir, mut vault) = empty_vault();
        vault
            .doc
            .manual_prices
            .push(crate::model::vault::ManualPrice {
                asset_key: "asset:FLAT".into(),
                unit_price: amount("100000"),
                currency: "usd".into(),
                updated_at: "2026-09-01T00:00:00Z".into(),
            });
        let book = vault.rate_book();
        let long_ago = secs_or_now("2020-01-01T00:00:00Z");
        assert_eq!(
            book.history_price("asset:FLAT", long_ago),
            Some(amount("100000"))
        );
    }

    #[test]
    fn the_quote_carries_the_moment_of_its_rate() {
        let (_dir, mut vault) = empty_vault();
        with_rub_history(&mut vault);
        let quote = vault.cached_quote(&rub(), "2026-03-01T12:00:00Z");
        assert_eq!(quote.price_usd, amount("0.0125"));
        assert_eq!(quote.ts, "2026-01-01T00:00:00Z");
        assert_eq!(vault.cached_quote(&btc(), ""), CachedPrice::default());
    }

    #[test]
    fn fx_missing_on_checks_the_calendar_day() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        vault.apply_fx(&[fx("rub", "90", "2026-10-09T12:00:00Z")]);
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        let wanted = ["rub".to_string(), "EUR".to_string(), "usd".to_string()];
        assert_eq!(vault.fx_missing_on(&wanted, day), vec!["eur".to_string()]);
        let next = day.succ_opt().unwrap();
        assert_eq!(
            vault.fx_missing_on(&wanted, next),
            vec!["eur".to_string(), "rub".to_string()]
        );
    }

    #[test]
    fn fx_missing_on_skips_crypto_codes() {
        let (_dir, vault) = crate::service::test_support::empty_vault();
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        let wanted = ["btc".to_string(), "TON".to_string(), "eur".to_string()];
        assert_eq!(vault.fx_missing_on(&wanted, day), vec!["eur".to_string()]);
    }

    #[test]
    fn convert_reaches_a_well_known_coin_not_held() {
        let (_dir, mut vault) = empty_vault();
        vault.apply_prices(&[PricePoint {
            asset_key: "coin:ethereum".into(),
            value_usd: amount("2000"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        }]);
        assert_eq!(
            vault.convert_at(amount("1"), "eth", "usd", ""),
            Some(amount("2000"))
        );
    }

    #[test]
    fn favorite_asset_prefers_the_held_identity() {
        let (_dir, mut vault) = empty_vault();
        assert_eq!(
            vault.favorite_asset("btc").map(|asset| asset.key()),
            Some("coin:bitcoin".to_string())
        );
        assert!(vault.favorite_asset("eur").is_none());
        assert!(vault.favorite_asset("usd").is_none());

        let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
        let held = AssetRef::crypto("TON", "Toncoin", None, crate::model::chain::Chain::Ton);
        vault
            .add_transaction(income_of(&wallet, held.clone(), "5", "3", "gift"))
            .unwrap();
        assert_eq!(vault.favorite_asset("ton"), Some(held));
    }

    fn priced_payment(
        price: &str,
        currency: &str,
        ts: &str,
    ) -> crate::ipc_contract::TransactionInput {
        serde_json::from_value(serde_json::json!({
            "type": "payment",
            "common": {
                "timestamp": ts, "comment": "", "categories": [], "flow_id": null, "contact_id": null,
                "price_usd": null, "fx_override": null, "price": price, "price_currency": currency,
            },
            "store_id": "s", "asset": {"symbol": "BTC", "name": "", "coingecko_id": null, "chain": null, "contract": null},
            "amount": "1", "fee": null, "to": null,
            "details": {"cash_breakdown": null, "unit_price": null, "network": null, "tx_hash": null},
        }))
        .unwrap()
    }

    /// A price typed in rubles is stored as USD at the rate of the moment,
    /// and kept as is when rubles are the base currency.
    #[test]
    fn a_price_in_any_currency_becomes_usd_at_the_moment_rate() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("rub");
        vault.apply_fx(&[
            fx("rub", "80", "2026-01-01T00:00:00Z"),
            fx("rub", "100", "2026-06-01T00:00:00Z"),
        ]);
        let mut input = priced_payment("8000", "rub", "2026-02-01T00:00:00Z");
        vault.resolve_input_price(&mut input).unwrap();
        let common = input.common_mut();
        assert_eq!(common.price_usd, Some(amount("100")));
        assert_eq!(common.price_base, Some(amount("8000")));
        assert_eq!(common.price, None);

        let mut unknown = priced_payment("10", "thb", "2026-02-01T00:00:00Z");
        assert!(
            matches!(vault.resolve_input_price(&mut unknown), Err(crate::Error::FxMissing(code)) if code == "thb")
        );
    }
}
