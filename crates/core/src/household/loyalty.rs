//! Bonus programmes and cashback: points kept in a loyalty wallet and valued
//! by the programme's price of a point, cashback as income in its category,
//! both tied back to the card whose purchases earned them.
//!
//! Pure part of the area: decisions and reports over a document or a read
//! context. The writes are the scenarios in `usecases::household::loyalty`.

use std::collections::BTreeMap;

use crate::Amount;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::ids::StoreId;
use crate::model::store::{LoyaltyInfo, StoreKind};
use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, TopupTx, Transaction, TxMeta};
use crate::model::vault::{ManualPrice, VaultDoc};
use crate::money::portfolio::{normalized_symbol, symbol_clash, symbol_taken_by_other};
use crate::money::tree::find_store;
use crate::time::secs_or_now;

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

/// What `set_loyalty_info` will write: the checked settings and the hand-set
/// price of a point (`None` when a point is worth nothing).
#[derive(Debug, Clone, PartialEq)]
pub struct LoyaltyUpdate {
    pub store_id: StoreId,
    /// `asset_key` of the programme's points, whose old price is replaced.
    pub asset_key: String,
    pub info: LoyaltyInfo,
    pub price: Option<ManualPrice>,
}

/// What `create_loyalty_program` will do after the check: the settings to set
/// once the wallet exists, and the contact it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct NewProgramme {
    pub info: LoyaltyInfo,
    pub contact_id: Option<String>,
}

/// The programme shares its symbol with another one. Computed for the
/// view, never stored; the form asks to rename.
pub fn loyalty_symbol_clash(doc: &VaultDoc, store_id: &StoreId) -> bool {
    symbol_clash(doc, store_id.as_str())
}

/// Normalize and check a programme's settings; `store_id` is the programme
/// itself, `None` while it is being created.
pub(crate) fn checked_loyalty_info(
    doc: &VaultDoc,
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
    // Only a NEW clash is refused: a programme that already shares its
    // symbol (two devices chose it before a sync) stays editable.
    let unchanged = store_id
        .and_then(|id| find_store(doc, id))
        .and_then(|store| store.loyalty.as_ref())
        .is_some_and(|known| normalized_symbol(&known.points_symbol) == info.points_symbol);
    let taken = !unchanged && symbol_taken_by_other(doc, store_id, &info.points_symbol);
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
        let card = find_store(doc, &id).ok_or_else(|| Error::Missing(format!("store {id}")))?;
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

/// Decide what setting a programme's points and price of a point writes.
/// `now` stamps the hand-set price.
pub fn plan_set_loyalty_info(
    doc: &VaultDoc,
    store_id: &StoreId,
    info: LoyaltyInfo,
    now: &str,
) -> Result<LoyaltyUpdate> {
    let store = find_store(doc, store_id.as_str())
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
    if store.kind != StoreKind::Loyalty {
        return Err(Error::InvalidInput("not a bonus programme".into()));
    }
    let info = checked_loyalty_info(doc, Some(store_id.as_str()), info)?;
    let asset_key = info.asset(&store.name).key();
    let price = info.point_value.is_positive().then(|| ManualPrice {
        asset_key: asset_key.clone(),
        unit_price: info.point_value,
        currency: info.currency.clone(),
        updated_at: now.to_string(),
    });
    Ok(LoyaltyUpdate {
        store_id: store_id.clone(),
        asset_key,
        info,
        price,
    })
}

/// Check a new programme's settings before the wallet exists: a refusal
/// leaves nothing behind.
pub fn plan_create_loyalty_program(
    doc: &VaultDoc,
    contact_id: Option<String>,
    info: LoyaltyInfo,
) -> Result<NewProgramme> {
    Ok(NewProgramme {
        info: checked_loyalty_info(doc, None, info)?,
        contact_id: contact_id.filter(|id| !id.is_empty()),
    })
}

/// Build the transaction that credits (`delta` > 0) or spends points of a
/// programme, optionally tied to the card whose purchase earned them.
pub fn plan_adjust_points(
    doc: &VaultDoc,
    store_id: &StoreId,
    delta: Amount,
    comment: &str,
    source_store_id: Option<StoreId>,
    timestamp: Option<String>,
) -> Result<Transaction> {
    let store = find_store(doc, store_id.as_str())
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
        source_store_id: source_store_id
            .map(|id| id.to_string())
            .filter(|id| !id.is_empty()),
        ..KindDetails::default()
    };
    Ok(if delta.is_positive() {
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
    })
}

/// Cashback (income in the `cashback` category) and points credited to
/// bonus programmes between two moments (RFC3339, inclusive), by card.
pub fn cashback_report(books: &Books<'_>, from: &str, to: &str) -> CashbackReport {
    let (base, fx) = books.base_and_fx();
    let rates = CashbackRates {
        base,
        fx,
        value_in_base: &|key, quantity, secs| {
            let book = books.rate_book();
            let usd = book.price_at(key, secs).unwrap_or(Amount::ZERO) * quantity;
            book.convert(usd, "usd", base, secs).unwrap_or(Amount::ZERO)
        },
        price_usd: &|key| books.price_usd(key),
    };
    cashback_over(books.doc(), &rates, from, to)
}

/// What the cashback report reads besides the document: the rates of one moment.
pub(crate) struct CashbackRates<'rates> {
    pub base: &'rates str,
    pub fx: Amount,
    /// The value of `quantity` of the asset `key` in `base` at `secs`, zero when unpriced.
    pub value_in_base: &'rates dyn Fn(&str, Amount, i64) -> Amount,
    pub price_usd: &'rates dyn Fn(&str) -> Amount,
}

/// The report over a document and given rates; no vault is needed.
pub(crate) fn cashback_over(
    doc: &VaultDoc,
    rates: &CashbackRates<'_>,
    from: &str,
    to: &str,
) -> CashbackReport {
    let CashbackRates {
        base,
        fx,
        value_in_base,
        price_usd,
    } = *rates;
    let kinds: BTreeMap<&str, StoreKind> = doc
        .stores
        .iter()
        .map(|store| (store.id.as_str(), store.kind))
        .collect();
    let mut by_source: BTreeMap<Option<String>, (Amount, Amount)> = BTreeMap::new();
    for tx in &doc.transactions {
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
        let value = value_in_base(&topup.asset.key(), topup.amount, secs);
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
            .and_then(|id| find_store(doc, id))
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
    let programmes = doc
        .stores
        .iter()
        .filter(|store| store.kind == StoreKind::Loyalty && !store.archived)
        .filter_map(|store| {
            let info = store.loyalty.as_ref()?;
            let asset = info.asset(&store.name);
            let points = doc
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
                value_base: (points * price_usd(&asset.key()) * fx).round(2),
                expires_at: info.expires_at.clone(),
                expiring_points: info.expiring_points,
            })
        })
        .collect();
    CashbackReport {
        base_currency: base.to_string(),
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
