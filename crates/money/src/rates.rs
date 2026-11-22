//! Rates at a moment (plan §6.1–§6.2): the FX and price history answered as
//! "what was it then", the price stamp a transaction gets when saved, and the
//! conversion the budget uses for its own currency (D20).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::rate_index::RateIndex;
use crate::books::Books;
use mwt_base::Amount;
use mwt_base::time::secs_or_now;
use mwt_model::AssetRef;
use mwt_model::{FxPoint, VaultDoc};
use mwt_model::{Leg, Transaction};

const FIAT_PREFIX: &str = "fiat:";
const PERP_PREFIX: &str = "perp:";

/// What the book asks of the dated value of a property: its currency and its
/// unit price at a moment. The module that builds the curves implements it, so
/// the rates do not name that module (the cycle `rates` <-> `valuation` is open
/// on this side).
pub trait ValueCurve: std::fmt::Debug + Send + Sync {
    /// The currency the curve is quoted in.
    fn currency(&self) -> &str;
    /// The unit price at `secs`.
    fn value_at(&self, secs: i64) -> Option<Amount>;
}

/// Everything a replay needs to price things at past moments, parsed once.
#[derive(Debug, Default, Clone)]
pub struct RateBook {
    fx: RateIndex,
    prices: RateIndex,
    /// Historic crypto prices (`crypto_prices`), by `<chain>|<contract>` key.
    crypto: RateIndex,
    /// Prices transactions recorded, for assets no provider quotes.
    stamps: RateIndex,
    /// Hand-set unit prices: `(price, currency)` per asset key (D24).
    manual: HashMap<String, (Amount, String)>,
    /// Dated property valuations (G3); win over `manual`.
    curves: HashMap<String, Arc<dyn ValueCurve>>,
    /// Asset key by upper-case symbol, so a currency code can name a crypto asset.
    keys_by_symbol: HashMap<String, String>,
    /// The base currency code, lower case; empty until [`RateBook::with_base`].
    base: String,
}

impl RateBook {
    /// The book of a document: the caches parsed once, `curves` the dated
    /// valuations of its property assets. Pure: it reads nothing but its input.
    pub fn build(doc: &VaultDoc, curves: HashMap<String, Arc<dyn ValueCurve>>) -> Self {
        let mut stamps = RateIndex::default();
        let mut keys_by_symbol = HashMap::new();
        for holding in &doc.holdings {
            keys_by_symbol
                .entry(holding.asset.symbol.to_uppercase())
                .or_insert_with(|| holding.asset.key());
        }
        // A favourite coin the vault does not hold is still convertible once
        // its quote is cached.
        for (ticker, coingecko_id, _) in mwt_model::well_known_coins() {
            keys_by_symbol
                .entry(ticker.to_string())
                .or_insert_with(|| format!("coin:{coingecko_id}"));
        }
        for tx in &doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            for leg in tx.legs() {
                if let Some(price) = leg.price_usd {
                    stamps.insert(&leg.asset.key(), secs, price);
                }
            }
        }
        stamps.sort();
        Self {
            fx: RateIndex::of_fx(&doc.fx_cache),
            prices: RateIndex::of_prices(&doc.price_cache),
            crypto: RateIndex::of_crypto(&doc.crypto_prices),
            stamps,
            curves,
            manual: doc
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
            base: String::new(),
        }
    }

    /// The same book that knows the base currency, so the fiat asset of the
    /// base is priced as exactly one of itself.
    #[must_use]
    pub fn with_base(mut self, base: &str) -> Self {
        self.base = base.trim().to_lowercase();
        self
    }

    /// The base currency code, lower case (empty before [`RateBook::with_base`]).
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The dated valuations of the property assets, by asset key.
    pub fn curves(&self) -> &HashMap<String, Arc<dyn ValueCurve>> {
        &self.curves
    }

    /// Price of one unit of `asset_key` in the base currency at `secs`: exactly
    /// 1 for the base's own fiat asset, before any rate is consulted; else the
    /// USD price through the base's rate. `None` with no base set or no rate.
    pub fn price_in_base(&self, asset_key: &str, secs: i64) -> Option<Amount> {
        if is_fiat_of(asset_key, &self.base) {
            return Some(one());
        }
        let usd = self.price_at(asset_key, secs)?;
        let rate = self
            .fx_at(&self.base, secs)
            .filter(|_| !self.base.is_empty())?;
        usd.checked_mul(rate)
    }
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
        self.prices
            .at(asset_key, secs)
            .or_else(|| self.crypto.at(asset_key, secs))
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
        let (at, price) = self
            .prices
            .point_at(asset_key, secs)
            .or_else(|| self.crypto.point_at(asset_key, secs))?;
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
            .or_else(|| self.crypto.at_or_before(asset_key, secs))
            .or_else(|| self.crypto.at(asset_key, secs))
    }

    /// `amount` of `from` in `to` at `secs`, both currency codes (fiat, or the
    /// symbol of a crypto asset the vault knows). Crosses go through USD.
    pub fn convert(&self, amount: Amount, from: &str, to: &str, secs: i64) -> Option<Amount> {
        let from = from.trim().to_lowercase();
        let to = to.trim().to_lowercase();
        if from == to {
            return Some(amount);
        }
        let usd = amount.checked_mul(self.code_usd(&from, secs)?)?;
        let to_unit = self.code_usd(&to, secs)?;
        usd.checked_div(to_unit)
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
        // The base currency is worth one of itself, whatever a rate says (L3).
        if is_fiat_of(&leg.asset.key(), base) {
            return one();
        }
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
            .fx_at(curve.currency(), secs)
            .or_else(|| self.fx.latest(curve.currency()))?;
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

pub fn iso_of(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|moment| moment.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// Whether `asset_key` is the fiat asset of currency `code` (any case, trimmed).
pub fn is_fiat_of(asset_key: &str, code: &str) -> bool {
    let code = code.trim();
    !code.is_empty()
        && asset_key
            .strip_prefix(FIAT_PREFIX)
            .is_some_and(|fiat| fiat.eq_ignore_ascii_case(code))
}

fn one() -> Amount {
    Amount::new(rust_decimal::Decimal::ONE)
}

/// The newest cached FX point of `currency` that carries a rate.
pub fn latest_fx_point<'doc>(doc: &'doc VaultDoc, currency: &str) -> Option<&'doc FxPoint> {
    doc.fx_cache
        .iter()
        .filter(|point| point.currency == currency && !point.per_usd.is_zero())
        .max_by(|left, right| left.ts.cmp(&right.ts))
}

/// Units of `currency` per USD from the FX cache; USD itself is 1.
pub fn per_usd(doc: &VaultDoc, currency: &str) -> Option<Amount> {
    let currency = currency.to_lowercase();
    if currency == "usd" {
        return Some(one());
    }
    latest_fx_point(doc, &currency).map(|point| point.per_usd)
}

/// The caches parsed once for a batch of lookups: a fresh book, not the
/// one `books` shares.
pub fn rate_book(books: &Books) -> RateBook {
    books.build_rate_book()
}

/// Units of `currency` per USD at `ts` (RFC3339; empty = now).
pub fn fx_at(books: &Books, currency: &str, ts: &str) -> Option<Amount> {
    books.rate_book().fx_at(currency, secs_or_now(ts))
}

/// USD price of one unit of an asset at `ts` (RFC3339; empty = now).
pub fn price_at(books: &Books, asset_key: &str, ts: &str) -> Option<Amount> {
    books.rate_book().price_at(asset_key, secs_or_now(ts))
}

/// `amount` of one currency in another at `ts` (RFC3339; empty = now).
pub fn convert_at(books: &Books, amount: Amount, from: &str, to: &str, ts: &str) -> Option<Amount> {
    books.rate_book().convert(amount, from, to, secs_or_now(ts))
}

/// Currencies whose rate history matters: the base and budget
/// currencies, every fiat the vault holds or moved, and the currencies of
/// hand-set prices. The rest of the table keeps only its latest rate.
pub fn fx_currencies_in_use(books: &Books) -> HashSet<String> {
    let doc = books.doc();
    let mut out: HashSet<String> = HashSet::from(["usd".to_string()]);
    out.insert(books.base_and_fx().0.to_string());
    let budget = doc.settings.budget_currency.trim().to_lowercase();
    if !budget.is_empty() {
        out.insert(budget);
    }
    for holding in &doc.holdings {
        if holding.asset.is_fiat() {
            out.insert(holding.asset.symbol.to_lowercase());
        }
    }
    for tx in &doc.transactions {
        for leg in tx.legs() {
            if leg.asset.is_fiat() {
                out.insert(leg.asset.symbol.to_lowercase());
            }
        }
    }
    for price in &doc.manual_prices {
        out.insert(price.currency.to_lowercase());
    }
    for dated in &doc.valuations {
        out.insert(dated.currency.to_lowercase());
    }
    out
}

/// Which currencies keep their rate history: the ones in use. Owns its
/// data, so it can be used while the cache is borrowed mutably.
pub fn fx_history_filter(books: &Books) -> impl Fn(&str) -> bool + use<> {
    let tracked = fx_currencies_in_use(books);
    move |code| tracked.contains(code)
}

/// A code that names a coin rather than a fiat currency: no FX rate is
/// cached for it, and the vault holds a coin with that ticker or it is a
/// well-known one. Its rate comes from the price cache, not FX sources.
pub fn is_crypto_code(doc: &VaultDoc, code: &str) -> bool {
    let lower = code.trim().to_lowercase();
    if lower.is_empty() || lower == "usd" {
        return false;
    }
    if doc.fx_cache.iter().any(|point| point.currency == lower) {
        return false;
    }
    held_coin(doc, &lower).is_some() || mwt_model::well_known_coin(&lower).is_some()
}

/// The asset a crypto favourite is priced as: the identity the vault
/// already holds, else the well-known coin. `None` for fiat and unknowns.
pub fn favorite_asset(doc: &VaultDoc, code: &str) -> Option<AssetRef> {
    if !is_crypto_code(doc, code) {
        return None;
    }
    if let Some(asset) = held_coin(doc, code) {
        return Some(asset.clone());
    }
    let upper = code.trim().to_uppercase();
    let (coingecko_id, name) = mwt_model::well_known_coin(&upper)?;
    Some(AssetRef {
        chain: None,
        ..AssetRef::crypto(&upper, name, Some(coingecko_id), mwt_model::Chain::Fiat)
    })
}

/// The held spot coin with this ticker.
pub fn held_coin<'doc>(doc: &'doc VaultDoc, code: &str) -> Option<&'doc AssetRef> {
    doc.holdings
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
pub fn fx_missing_on(
    doc: &VaultDoc,
    currencies: &[String],
    date: chrono::NaiveDate,
) -> Vec<String> {
    let day = date.format("%Y-%m-%d").to_string();
    let mut out: Vec<String> = currencies
        .iter()
        .map(|code| code.trim().to_lowercase())
        .filter(|code| !code.is_empty() && code != "usd")
        .filter(|code| !is_crypto_code(doc, code))
        .filter(|code| {
            !doc.fx_cache.iter().any(|point| {
                &point.currency == code && point.ts.starts_with(&day) && point.per_usd.is_positive()
            })
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Give every unpriced side the USD price of its moment (plan §6.2).
/// A price already recorded — typed by the user, imported — is kept.
/// Returns how many sides got a price.
pub fn stamp_prices(book: &RateBook, tx: &mut Transaction) -> usize {
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

/// Value of a quantity in `currency` at `secs`; zero without a rate.
pub fn value_at(
    book: &RateBook,
    asset: &AssetRef,
    qty: Amount,
    currency: &str,
    secs: i64,
) -> Amount {
    if qty.is_zero() {
        return Amount::ZERO;
    }
    if asset.is_fiat() && asset.symbol.eq_ignore_ascii_case(currency) {
        return qty;
    }
    book.history_price(&asset.key(), secs)
        .and_then(|usd| book.convert(qty * usd, "usd", currency, secs))
        .unwrap_or(Amount::ZERO)
}
