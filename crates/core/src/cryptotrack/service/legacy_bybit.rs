//! Bybit on the engine: the rules of storing exchange records as operations of
//! the crypto module (stage 2, spec E7). The scenario that calls them is
//! `usecases::cryptotrack::legacy_bybit`. The one-time conversion of the old queue lives
//! in `storage::legacy_v4`.
//!
//! Until now an exchange sweep *staged* every record it found
//! (`VaultDoc::pending_imports`) and the user confirmed it into a ledger
//! transaction (`TxOrigin::Import { exec_id }`). From here on an exchange is
//! read by the crypto module like any other followed account: a record
//! becomes a [`CryptoTx`] at once, with no queue in between, and the
//! verification screen is gone.
//!
//! ## What must not be lost
//!
//! The old mechanics stay readable, because a user's file holds them
//! (`AGENTS.md`, rule 5: a stored field is never renamed, removed or
//! repurposed — it simply stops being written):
//!
//! * `pending_imports` — the queue. Its entries were put through once by the
//!   v4 transition. Nothing writes the field again.
//! * `discarded_imports` — records the user refused. They stay refused: a
//!   discarded `exec_id` is never converted and never fetched in again.
//! * `TxOrigin::Import { exec_id }` — records already booked as ledger
//!   transactions. Those transactions are left exactly as they are, and
//!   their `exec_id`s are treated as decided, so no operation of the module
//!   is written for them. That is what keeps the old records and the new
//!   events from both counting.
//! * `import_runs` — the sync history, still written and still read.
//!
//! ## Why it is idempotent
//!
//! An event's id is a function of the account and of the fill
//! ([`crate::cryptotrack::engine::ids::event`] over
//! [`crate::model::cryptotrack::TrackingRef::ExchangeHistory`]), the
//! account's own id is a function of the wallet's id
//! ([`bybit::account_id_for`]), and the price points the conversion writes
//! are keyed by `(asset, day)`. Not one of them reads a clock or a counter.
//! So running the conversion twice rewrites the same records with the same
//! bytes, and two devices that converted the same file hold the same records
//! — which is what lets the sync treat an operation as one record instead of
//! two.
//!
//! ## Where the cost basis comes from
//!
//! The engine prices every leg of an operation through the document's price
//! cache, by day ([`prices`]). A fill quoted in a dollar stablecoin states
//! its own dollar price exactly, so the conversion writes that price into the
//! cache instead of leaving the basis to a market quote fetched later. When a
//! day holds several fills of one asset, the point is their
//! quantity-weighted average — which is precisely what average cost needs:
//! the dollars the day's purchases added to the pool are the dollars the
//! fills actually cost (`the_pool_costs_what_the_fills_cost`). A fill quoted
//! in anything else (`ETHBTC`) implies no dollar price and gets none; the
//! engine then says the cost is not known rather than inventing one.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::cryptotrack::prices;
use crate::cryptotrack::sources::bybit;
use crate::cryptotrack::validate::Validate;
use crate::error::{Error, Result};
use crate::model::cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoPricePoint, CryptoToken, TokenId, TokenVerdict,
};
use crate::model::store::StoreKind;
use crate::model::vault::{PendingImport, VaultDoc};

/// What storing a batch of exchange records did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BybitStored {
    /// Operations written for the first time.
    pub added: u32,
    /// Operations that were already there and were written again, byte for
    /// byte the same unless the exchange changed its answer.
    pub rewritten: u32,
    /// Records settled before: already an operation, already a ledger
    /// transaction, or refused by the user.
    pub known: u32,
    /// `exec_id`s the module could not read into an operation. They are
    /// never dropped: the caller leaves them where they were.
    pub unread: Vec<String>,
}

impl BybitStored {
    /// Operations this batch put into the module.
    pub fn recorded(&self) -> u32 {
        self.added + self.rewritten
    }
}

/// `exec_id`s of the exchange operations the module already holds.
pub fn bybit_event_ids(doc: &VaultDoc) -> BTreeSet<String> {
    doc.crypto_events
        .iter()
        .filter_map(bybit::exec_id_of)
        .map(str::to_owned)
        .collect()
}

/// The followed account of the exchange wallet `root`, made if it is not there
/// yet, and bound to the wallet (`Store::crypto_account_id`, decision
/// P5).
///
/// `earliest` is the moment of the oldest record about to be stored: it
/// becomes the account's `created_at`, which is the date the module
/// follows the exchange from. It only ever moves earlier, so a later
/// batch of older fills widens the followed period and never narrows it —
/// and no clock takes part, so both devices write the same account.
fn bybit_account(doc: &mut VaultDoc, root: &str, earliest: &str) -> Result<String> {
    let store = doc
        .stores
        .iter()
        .find(|store| store.id == root)
        .ok_or_else(|| Error::Missing(format!("store {root}")))?;
    if store.kind != StoreKind::Exchange {
        return Err(Error::InvalidInput(format!(
            "store {root} is not an exchange wallet"
        )));
    }
    let name = store.name.clone();
    let account_id = bybit::account_id_for(root);
    let handle = bybit::account_handle(root);
    match doc
        .crypto_accounts
        .iter_mut()
        .find(|account| account.id == account_id)
    {
        Some(account) => {
            if !earliest.is_empty()
                && (account.created_at.is_empty() || *earliest < *account.created_at)
            {
                account.created_at = earliest.to_owned();
            }
            account.source = Some(bybit::SOURCE_ID.to_owned());
        }
        None => {
            let mut account =
                CryptoAccount::new(&account_id, &name, bybit::EXCHANGE_CHAIN, &handle, earliest);
            account.source = Some(bybit::SOURCE_ID.to_owned());
            account.validate()?;
            doc.crypto_accounts.push(account);
        }
    }
    if let Some(store) = doc.stores.iter_mut().find(|store| store.id == root)
        && store.crypto_account_id.as_deref() != Some(account_id.as_str())
    {
        store.crypto_account_id = Some(account_id.clone());
    }
    Ok(account_id)
}

/// Store exchange records as operations of the module. The one way in:
/// both a fresh sweep and the conversion of the old queue come through
/// here, so neither can write an operation the other would not.
///
/// `root` is the top-level wallet the records belong to. `decided` is every
/// `exec_id` settled before (refused, already an operation, already a ledger
/// transaction). Does not save; the caller does,
/// once.
pub fn store_records(
    doc: &mut VaultDoc,
    decided: &HashSet<String>,
    root: &str,
    records: &[PendingImport],
) -> Result<BybitStored> {
    let mut out = BybitStored::default();
    let mut held = bybit_event_ids(doc);
    // Deterministic order: the day's price point is folded in this order,
    // and so is the account's start date.
    let mut wanted: Vec<&PendingImport> = Vec::new();
    for record in records {
        let exec_id = record.exec_id.trim();
        if exec_id.is_empty() {
            out.unread.push(record.exec_id.clone());
            continue;
        }
        if decided.contains(exec_id) || held.contains(exec_id) {
            out.known += 1;
            continue;
        }
        held.insert(exec_id.to_owned());
        wanted.push(record);
    }
    if wanted.is_empty() {
        return Ok(out);
    }
    wanted.sort_by(|left, right| {
        (&left.occurred_at, &left.exec_id).cmp(&(&right.occurred_at, &right.exec_id))
    });
    let earliest = wanted
        .first()
        .map(|record| record.occurred_at.clone())
        .unwrap_or_default();
    let account_id = bybit_account(doc, root, &earliest)?;

    let mut events = Vec::new();
    let mut kept: Vec<&PendingImport> = Vec::new();
    for record in wanted {
        match bybit::event_from_pending(&account_id, record) {
            Some(event) => {
                events.push(event);
                kept.push(record);
            }
            None => out.unread.push(record.exec_id.clone()),
        }
    }
    for symbol in coins_of(&kept) {
        register_coin(doc, &symbol);
    }
    for point in day_prices(&kept) {
        prices::apply_historic(doc, &[point]);
    }
    for event in events {
        match doc
            .crypto_events
            .iter_mut()
            .find(|have| have.id == event.id)
        {
            Some(have) => {
                if *have != event {
                    *have = event;
                }
                out.rewritten += 1;
            }
            None => {
                doc.crypto_events.push(event);
                out.added += 1;
            }
        }
    }
    doc.crypto_events
        .sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
    Ok(out)
}

/// An exchange coin as a token of the module, counted in the totals.
///
/// An exchange ticker is on no contract list and would be quarantined out
/// of every total (spec E2), so a coin the conversion met is marked
/// confirmed: it was already in the user's ledger, and silently dropping
/// it would lose money the user can see. A verdict the user has given
/// already is never overwritten — rejecting a coin must stick.
fn register_coin(doc: &mut VaultDoc, symbol: &str) {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        return;
    }
    let token = TokenId::new(bybit::EXCHANGE_CHAIN, &symbol);
    let known = crate::model::coins::well_known_coin(&symbol);
    if let Some(record) = doc
        .crypto_tokens
        .iter_mut()
        .find(|record| record.id() == token)
    {
        record.symbol = symbol;
        if record.verdict.is_none() {
            record.verdict = Some(TokenVerdict::Confirmed);
        }
        if record.coingecko_id.is_none() {
            record.coingecko_id = known.map(|(id, _)| id.to_owned());
        }
        return;
    }
    let mut record = CryptoToken::new(token, &symbol, bybit::COIN_DECIMALS);
    record.name = known.map(|(_, name)| name.to_owned()).unwrap_or(symbol);
    record.coingecko_id = known.map(|(id, _)| id.to_owned());
    record.verdict = Some(TokenVerdict::Confirmed);
    doc.crypto_tokens.push(record);
}

/// Every coin named by a batch, each once, in a fixed order.
fn coins_of(records: &[&PendingImport]) -> BTreeSet<String> {
    records
        .iter()
        .flat_map(|record| {
            [
                record.asset_bought.clone(),
                record.asset_sold.clone(),
                record.fee_asset.clone(),
            ]
        })
        .map(|symbol| symbol.trim().to_ascii_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect()
}

/// The dollar price of each asset for each day the records state one: the
/// quantity-weighted average of what the records themselves say.
///
/// Weighted, and not "the last fill of the day", because the engine can only
/// price a day (`PriceIndex::at` asks for midnight UTC): with a weighted
/// point the dollars a day's purchases add to the cost pool are exactly the
/// dollars the fills cost, which is what "the cost basis is preserved" means
/// under average cost.
fn day_prices(records: &[&PendingImport]) -> Vec<CryptoPricePoint> {
    // (asset key, day) -> (asset, Σ quantity × price, Σ quantity)
    let mut sums: BTreeMap<(String, String), (CryptoAsset, Amount, Amount)> = BTreeMap::new();
    for record in records {
        let day = crate::cryptotrack::engine::ids::day(&record.occurred_at);
        if day.len() != 10 {
            continue;
        }
        for (asset, unit) in bybit::implied_usd(record) {
            let Some(quantity) = quantity_of(record, &asset) else {
                continue;
            };
            let Some(value) = math::mul(quantity, unit) else {
                continue;
            };
            let slot = sums
                .entry((asset.key(), day.clone()))
                .or_insert_with(|| (asset.clone(), Amount::ZERO, Amount::ZERO));
            slot.1 = math::add(slot.1, value).unwrap_or(slot.1);
            slot.2 = math::add(slot.2, quantity).unwrap_or(slot.2);
        }
    }
    sums.into_iter()
        .filter_map(|((key, day), (_, value, quantity))| {
            let unit = math::div(value, quantity)?;
            let date = chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").ok()?;
            unit.is_positive().then(|| CryptoPricePoint {
                token_key: key,
                value_usd: unit,
                ts: prices::date_ts(date),
                source: bybit::SOURCE_ID.to_owned(),
            })
        })
        .collect()
}

/// How much of `asset` the record moved: the weight of its price in the day's
/// average.
fn quantity_of(record: &PendingImport, asset: &CryptoAsset) -> Option<Amount> {
    let ticker = bybit::asset_ticker(asset)?;
    let bought = record.asset_bought.trim().eq_ignore_ascii_case(ticker);
    let sold = record.asset_sold.trim().eq_ignore_ascii_case(ticker);
    let quantity = match (bought, sold) {
        (true, _) => record.qty_bought,
        (false, true) => record.qty_sold,
        (false, false) => return None,
    };
    quantity.is_positive().then_some(quantity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{ExchangeProfile, ExchangeProvider, Store};

    fn doc_with_bybit_wallet() -> (VaultDoc, String) {
        let mut doc = VaultDoc::default();
        let mut wallet = Store::new("Bybit", StoreKind::Exchange, "#000");
        wallet.exchange = Some(ExchangeProfile {
            provider: ExchangeProvider::Bybit,
            ..ExchangeProfile::default()
        });
        let id = wallet.id.clone();
        doc.stores.push(wallet);
        (doc, id)
    }

    #[test]
    fn the_account_only_ever_starts_earlier_and_is_made_once() {
        let (mut doc, wallet) = doc_with_bybit_wallet();
        let first = bybit_account(&mut doc, &wallet, "2023-11-14T00:00:00Z").unwrap();
        let again = bybit_account(&mut doc, &wallet, "2023-11-10T00:00:00Z").unwrap();
        let later = bybit_account(&mut doc, &wallet, "2023-12-01T00:00:00Z").unwrap();
        assert_eq!((&first, &later), (&again, &again));
        assert_eq!(doc.crypto_accounts.len(), 1);
        assert_eq!(doc.crypto_accounts[0].created_at, "2023-11-10T00:00:00Z");
    }

    #[test]
    fn a_coin_met_is_confirmed_but_a_verdict_the_user_gave_sticks() {
        let mut doc = VaultDoc::default();
        register_coin(&mut doc, " btc ");
        assert_eq!(doc.crypto_tokens.len(), 1);
        assert_eq!(doc.crypto_tokens[0].symbol, "BTC");
        assert_eq!(doc.crypto_tokens[0].verdict, Some(TokenVerdict::Confirmed));

        doc.crypto_tokens[0].verdict = Some(TokenVerdict::Rejected);
        register_coin(&mut doc, "BTC");
        assert_eq!(doc.crypto_tokens.len(), 1);
        assert_eq!(doc.crypto_tokens[0].verdict, Some(TokenVerdict::Rejected));
    }
}
