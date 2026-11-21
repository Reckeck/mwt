//! The one-time conversion of the old Bybit queue, frozen.
//!
//! A copy of `UnlockedVault::adopt_legacy_bybit` and the helpers it used
//! (`service/cryptotrack/legacy_bybit.rs`), reading and writing the v4 document instead of a
//! vault. It turns every staged record of `pending_imports` into an operation of the crypto
//! module, exactly as the shim did when an exchange sync met the queue, so the transition does it
//! once and the live code has no queue to convert.
//!
//! No record is lost: an entry is converted, or found settled already (booked into a transaction
//! or refused) and dropped from the queue without being counted twice, or left where it is when it
//! cannot be read or its wallet is gone. Ids, times and prices come from the facts of a record and
//! never from a clock, so two devices that convert the same file write the same records.
//!
//! The holdings are not refolded here: the caller refolds them once the document is a vault.

use std::collections::{BTreeMap, BTreeSet, HashSet};

mod frozen;

use super::{PendingImportV4, VaultDocV4};
use frozen::{bybit, date_ts, day, math, validate_account};
use mwt_base::Amount;
use mwt_model::PendingImport;
use mwt_model::TxOrigin;
use mwt_model::{CryptoAccount, CryptoAsset, CryptoPricePoint, CryptoToken, TokenId, TokenVerdict};
use mwt_model::{Store, StoreKind};

/// What the one-time conversion of the old queue did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyAdoption {
    /// Queue entries that became operations of the module.
    pub converted: u32,
    /// Queue entries that were settled already (booked or refused) and were
    /// therefore only dropped from the queue, never counted twice.
    pub known: u32,
    /// Queue entries left in place because they could not be read or their
    /// wallet is gone. Nothing is ever thrown away silently.
    pub kept: u32,
}

/// What storing a batch of exchange records did (read by the facade's tests of the conversion).
#[derive(Debug, Default)]
pub struct Stored {
    added: u32,
    rewritten: u32,
    /// Records the module held already.
    pub known: u32,
    /// `exec_id`s the module could not read into an operation; they stay in the queue.
    unread: Vec<String>,
}

impl Stored {
    /// How many records were added or rewritten.
    pub fn recorded(&self) -> u32 {
        self.added + self.rewritten
    }
}

/// Put the old verification queue through the conversion. Idempotent: a document with no queue
/// does nothing.
pub fn adopt_bybit(doc: &mut VaultDocV4) -> LegacyAdoption {
    let mut out = LegacyAdoption::default();
    if doc.pending_imports.is_empty() {
        return out;
    }
    // By wallet, so each wallet's records reach their own account; sorted, so two devices do the
    // same work in the same order.
    let mut by_wallet: BTreeMap<String, Vec<PendingImportV4>> = BTreeMap::new();
    let mut kept: Vec<PendingImportV4> = Vec::new();
    for record in std::mem::take(&mut doc.pending_imports) {
        let root = root_of(&doc.stores, &record.store_id);
        let known_wallet =
            find_store(&doc.stores, &root).is_some_and(|store| store.kind == StoreKind::Exchange);
        if known_wallet {
            by_wallet.entry(root).or_default().push(record);
        } else {
            kept.push(record);
        }
    }
    for (root, records) in &by_wallet {
        let live: Vec<PendingImport> = records.iter().cloned().map(PendingImport::from).collect();
        let stored = store_batch(doc, root, &live);
        out.converted += stored.recorded();
        out.known += stored.known;
        let unread: BTreeSet<&str> = stored.unread.iter().map(String::as_str).collect();
        kept.extend(
            records
                .iter()
                .filter(|record| unread.contains(record.exec_id.as_str()))
                .cloned(),
        );
    }
    out.kept = u32::try_from(kept.len()).unwrap_or(u32::MAX);
    doc.pending_imports = kept;
    out
}

fn find_store<'stores>(stores: &'stores [Store], id: &str) -> Option<&'stores Store> {
    stores.iter().find(|store| store.id == id)
}

/// Top-level wallet that `id` belongs to (itself for a root). Stops at a missing parent or at a
/// cycle left by a hand-edited vault.
fn root_of(stores: &[Store], id: &str) -> String {
    let mut root = id.to_owned();
    let mut seen = HashSet::from([id.to_owned()]);
    let mut parent = find_store(stores, id).and_then(|store| store.parent_id.clone());
    while let Some(parent_id) = parent {
        if !seen.insert(parent_id.clone()) {
            break;
        }
        let Some(store) = find_store(stores, &parent_id) else {
            break;
        };
        root = store.id.clone();
        parent = store.parent_id.clone();
    }
    root
}

/// `exec_id`s of the exchange operations the module already holds (also read by the facade's
/// tests of the conversion).
pub fn event_ids(doc: &VaultDocV4) -> BTreeSet<String> {
    doc.crypto_events
        .iter()
        .filter_map(bybit::exec_id_of)
        .map(str::to_owned)
        .collect()
}

/// Records that must never become an operation: held by the module, booked as a ledger
/// transaction, or refused by the user. Together they keep one record from being counted twice.
fn decided_ids(doc: &VaultDocV4) -> HashSet<String> {
    let mut decided: HashSet<String> = doc.discarded_imports.iter().cloned().collect();
    decided.extend(event_ids(doc));
    decided.extend(
        doc.transactions
            .iter()
            .filter_map(|tx| match &tx.meta().origin {
                TxOrigin::Import { exec_id } => Some(exec_id.clone()),
                _ => None,
            }),
    );
    decided
}

/// The followed account of an exchange wallet, made if it is not there yet and bound to the
/// wallet. `earliest` becomes the account's `created_at` and only ever moves earlier. `None` when
/// the wallet is not an exchange or the account would not validate.
fn bybit_account(doc: &mut VaultDocV4, store_id: &str, earliest: &str) -> Option<String> {
    let root = root_of(&doc.stores, store_id);
    let store = find_store(&doc.stores, &root)?;
    if store.kind != StoreKind::Exchange {
        return None;
    }
    let name = store.name.clone();
    let account_id = bybit::account_id_for(&root);
    let handle = bybit::account_handle(&root);
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
            validate_account(&account).ok()?;
            doc.crypto_accounts.push(account);
        }
    }
    if let Some(store) = doc.stores.iter_mut().find(|store| store.id == root)
        && store.crypto_account_id.as_deref() != Some(account_id.as_str())
    {
        store.crypto_account_id = Some(account_id.clone());
    }
    Some(account_id)
}

/// Store exchange records of one wallet as operations of the module (also called by the
/// facade's tests of the conversion).
pub fn store_batch(doc: &mut VaultDocV4, store_id: &str, records: &[PendingImport]) -> Stored {
    let mut out = Stored::default();
    let decided = decided_ids(doc);
    let mut held = event_ids(doc);
    // Deterministic order: the day's price point is folded in this order, and so is the
    // account's start date.
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
        return out;
    }
    wanted.sort_by(|left, right| {
        (&left.occurred_at, &left.exec_id).cmp(&(&right.occurred_at, &right.exec_id))
    });
    let earliest = wanted
        .first()
        .map(|record| record.occurred_at.clone())
        .unwrap_or_default();
    let Some(account_id) = bybit_account(doc, store_id, &earliest) else {
        out.unread
            .extend(wanted.iter().map(|record| record.exec_id.clone()));
        return out;
    };

    let mut events = Vec::new();
    let mut readable: Vec<&PendingImport> = Vec::new();
    for record in wanted {
        match bybit::event_from_pending(&account_id, record) {
            Some(event) => {
                events.push(event);
                readable.push(record);
            }
            None => out.unread.push(record.exec_id.clone()),
        }
    }
    for symbol in coins_of(&readable) {
        register_coin(doc, &symbol);
    }
    for point in day_prices(&readable) {
        apply_price(doc, point);
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
    out
}

/// A price point of the cache: one of the same `(token key, ts)` is replaced.
fn apply_price(doc: &mut VaultDocV4, point: CryptoPricePoint) {
    if point.value_usd.is_zero() {
        return;
    }
    match doc
        .crypto_prices
        .iter_mut()
        .find(|have| have.token_key == point.token_key && have.ts == point.ts)
    {
        Some(have) => *have = point,
        None => doc.crypto_prices.push(point),
    }
}

/// An exchange coin as a token of the module, counted in the totals.
///
/// An exchange ticker is on no contract list and would be quarantined out of every total, so a
/// coin the conversion met is marked confirmed: it was already in the user's ledger, and silently
/// dropping it would lose money the user can see. A verdict the user has given already is never
/// overwritten.
fn register_coin(doc: &mut VaultDocV4, symbol: &str) {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        return;
    }
    let token = TokenId::new(bybit::EXCHANGE_CHAIN, &symbol);
    let known = mwt_model::well_known_coin(&symbol);
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

/// The dollar price of each asset for each day the records state one: the quantity-weighted
/// average of what the records themselves say.
///
/// Weighted, and not "the last fill of the day", because the engine can only price a day: with a
/// weighted point the dollars a day's purchases add to the cost pool are exactly the dollars the
/// fills cost, which is what "the cost basis is preserved" means under average cost.
fn day_prices(records: &[&PendingImport]) -> Vec<CryptoPricePoint> {
    // (asset key, day) -> (asset, sum of quantity x price, sum of quantity)
    let mut sums: BTreeMap<(String, String), (CryptoAsset, Amount, Amount)> = BTreeMap::new();
    for record in records {
        let day = day(&record.occurred_at);
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
                ts: date_ts(date),
                source: bybit::SOURCE_ID.to_owned(),
            })
        })
        .collect()
}

/// How much of `asset` the record moved: the weight of its price in the day's average.
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
