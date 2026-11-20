//! The rules of the engine's meeting with the vault (spec E5).
//!
//! The engine itself ([`crate::cryptotrack::engine`]) is pure: it knows
//! nothing about a document, a clock or a network. This file is where a
//! source's answer is turned into what the document stores, where the
//! corrections and the prices are read out of the document, and where the
//! totals are added up, so everything that can go wrong with a holding can
//! be reproduced with a function call and a fixture. Every function works
//! on the document or the [`Books`] it is given and reads no clock; the
//! scenario (`usecases::cryptotrack::recompute`) passes the moment and owns
//! the one write.
//!
//! What happens when a batch arrives:
//!
//! 1. the facts are normalized into operations and stored by id
//!    ([`store_events`]) — a refetch lands on the same ids and overwrites
//!    rather than appends;
//! 2. the tokens met are registered (spec E2), which never touches a verdict
//!    the user gave;
//! 3. everything is folded again ([`fold`]), with the user's corrections on top;
//! 4. the result is compared with the balances the node reported and the
//!    difference is written down ([`reconcile`], spec E5, point 4);
//! 5. a snapshot of the value is written for the capital history
//!    ([`snapshot_records`]).
//!
//! **Where the cost basis comes from**, in order:
//!
//! 1. the fiat operation this one is chained to (`CryptoOverride::flow_id`,
//!    decision P6): the user's own number, and so above the market;
//! 2. for the opening adjustment of a token, the basis the user declared for
//!    its old purchases (`CryptoToken::basis_usd`);
//! 3. the price of that day from `crypto_prices` (spec E4), marked stale when
//!    an older point stood in;
//! 4. nothing — and then the asset says its cost is not known, instead of
//!    pretending it is zero.
//!
//! **What this file does not decide.** The account has no start date of its
//! own in the stored model, so the caller passes one
//! ([`CryptoBatch::start`]); without it the account's `created_at` stands in.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use crate::Amount;
use crate::cryptotrack::engine::{
    self, Adjustment, AssetState, Doubt, Inputs, Ledger, Normalized, Prices, Quote, Reading,
};
use crate::cryptotrack::prices;
use crate::cryptotrack::service::tokens::counts_asset;
use crate::cryptotrack::sources::evm::{RawAsset, SourceBatch};
use crate::cryptotrack::tokens;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoSnapshot, CryptoTx, TokenId, normalize_address,
};
use crate::model::vault::VaultDoc;

/// What to do with one batch of a source.
#[derive(Debug, Clone, Copy)]
pub struct CryptoBatch<'run> {
    /// The followed account the batch was read for.
    pub account_id: &'run str,
    /// When the batch was read (RFC3339 UTC): the day of a difference and of
    /// the snapshot.
    pub read_at: &'run str,
    /// The date the user chose to follow this address from; the account's
    /// `created_at` when `None`.
    pub start: Option<&'run str>,
    /// Whether the source has read the history back to the start date. Only
    /// then can a difference against the node mean anything.
    pub history_done: bool,
    /// Whether the balances in the batch list every asset of the address
    /// ([`Reading::complete`]).
    pub balances_complete: bool,
}

/// What a batch changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CryptoApplied {
    /// Operations added.
    pub added: usize,
    /// Operations that were already there and were rewritten.
    pub rewritten: usize,
    /// Operations the module could not read, with the reason.
    pub unread: Vec<engine::Unread>,
    /// Items the source itself refused, passed through for the screen.
    pub rejected: usize,
    /// Asset keys the network had to correct.
    pub corrected: BTreeSet<String>,
    /// Snapshots written.
    pub snapshots: usize,
}

/// One asset, as the engine sees it now.
#[derive(Debug, Clone, PartialEq)]
pub struct CryptoAssetTotal {
    pub asset: CryptoAsset,
    pub key: String,
    /// In the wallets.
    pub quantity: Amount,
    /// Inside protocols.
    pub in_positions: Amount,
    pub cost_usd: Amount,
    /// USD per unit of the priced part; `None`, never zero, when unknown.
    pub average_usd: Option<Amount>,
    /// What it is worth now; `None` when no price is known.
    pub value_usd: Option<Amount>,
    pub realized_usd: Amount,
    pub gas_usd: Amount,
    pub income_usd: Amount,
    /// The cost or the profit carries a known gap.
    pub imprecise: bool,
    pub doubts: Vec<Doubt>,
    /// A price of another day stood in somewhere.
    pub stale_prices: bool,
    /// Some of the cost is the user's own number, not a market quote.
    pub linked_basis: bool,
}

/// What the module adds up to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CryptoTotals {
    pub assets: Vec<CryptoAssetTotal>,
    /// Sum of what is priced.
    pub value_usd: Amount,
    pub cost_usd: Amount,
    pub realized_usd: Amount,
    pub gas_usd: Amount,
    pub income_usd: Amount,
    /// How many operations were counted.
    pub counted: usize,
    /// Any asset's cost or profit carries a gap.
    pub imprecise: bool,
}

/// Every price point of the document, by asset and by moment.
///
/// E4's [`prices::lookup`] answers one question with a scan of
/// `crypto_prices`. A fold asks that question once per leg, and a vault that
/// has followed a few assets for a few years holds thousands of daily points,
/// so the scan is the fold's whole cost. This index answers exactly the same
/// question — the newest point not later than the day asked about, zeros
/// ignored, `stale` unless it is that very day — in a lookup rather than a
/// scan, and a test
/// (the test `the_price_index_answers_what_the_cache_lookup_answers`) holds
/// the two side by side so they cannot drift apart.
#[derive(Debug, Default)]
pub(crate) struct PriceIndex {
    points: BTreeMap<String, BTreeMap<String, Amount>>,
}

impl PriceIndex {
    pub(crate) fn of(doc: &VaultDoc) -> Self {
        let mut points: BTreeMap<String, BTreeMap<String, Amount>> = BTreeMap::new();
        for point in &doc.crypto_prices {
            if point.value_usd.is_zero() {
                continue;
            }
            points
                .entry(point.token_key.clone())
                .or_default()
                .insert(point.ts.clone(), point.value_usd);
        }
        Self { points }
    }

    /// The price to use for a day, and whether it belongs to another one.
    pub(crate) fn at(&self, asset: &CryptoAsset, date: NaiveDate) -> (Option<Amount>, bool) {
        let wanted = prices::date_ts(date);
        match self
            .points
            .get(&asset.key())
            .and_then(|points| points.range(..=wanted.clone()).next_back())
        {
            Some((at, value)) => (Some(*value), *at != wanted),
            None => (None, true),
        }
    }
}

/// Prices as the document knows them, with the user's own numbers above the
/// market (see the order at the top of this file).
struct DocPrices<'doc> {
    doc: &'doc VaultDoc,
    market: PriceIndex,
    /// Operation id to USD per unit, implied by the fiat operation it is
    /// chained to.
    linked: BTreeMap<String, Amount>,
}

impl Prices for DocPrices<'_> {
    fn unit_usd(&self, asset: &CryptoAsset, time: &str, event_id: &str) -> Quote {
        if let Some(price) = self.linked.get(event_id) {
            return Quote {
                value: Some(*price),
                stale: false,
                linked: true,
            };
        }
        if engine::ids::adjustment(event_id) == Some(Adjustment::Opening)
            && let CryptoAsset::Token(token) = asset
            && let Some(declared) = tokens::basis(self.doc, token)
        {
            return Quote {
                value: Some(declared),
                stale: false,
                linked: true,
            };
        }
        let Some(date) = day_of(time) else {
            return Quote::default();
        };
        let (value, stale) = self.market.at(asset, date);
        Quote {
            value,
            stale,
            linked: false,
        }
    }
}

/// The calendar day of an RFC3339 moment.
pub(crate) fn day_of(time: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&engine::ids::day(time), "%Y-%m-%d").ok()
}

/// Where a batch lands: the network of the account, its address in the
/// engine's spelling and the date it is followed from.
#[derive(Debug, Clone)]
pub struct BatchTarget {
    pub chain: Chain,
    pub address: String,
    pub start: String,
}

/// Where `request`'s batch lands, or `NotFound` for an account the document
/// does not follow.
pub fn batch_target(doc: &VaultDoc, request: &CryptoBatch<'_>) -> Result<BatchTarget> {
    let account = doc
        .crypto_accounts
        .iter()
        .find(|account| account.id == request.account_id)
        .ok_or_else(|| Error::NotFound(format!("crypto account {}", request.account_id)))?;
    Ok(BatchTarget {
        chain: account.chain,
        address: normalize_address(account.chain, &account.address),
        start: request
            .start
            .unwrap_or(&account.created_at)
            .trim()
            .to_owned(),
    })
}

/// A token a batch mentions, as the source wrote it.
#[derive(Debug, Clone, Copy)]
pub struct MetToken<'batch> {
    pub contract: &'batch str,
    pub symbol: &'batch str,
    pub name: &'batch str,
    pub decimals: u8,
}

/// Every token the batch met, by lower-cased contract: named once, however
/// many operations it appears in, because registering is a write.
pub fn met_tokens(batch: &SourceBatch) -> BTreeMap<String, MetToken<'_>> {
    let mut met = BTreeMap::new();
    for asset in batch
        .raw_events
        .iter()
        .map(|fact| &fact.asset)
        .chain(batch.balances.iter().map(|balance| &balance.asset))
    {
        if let RawAsset::Token {
            contract,
            symbol,
            name,
            decimals,
            ..
        } = asset
        {
            met.entry(contract.to_lowercase()).or_insert(MetToken {
                contract,
                symbol,
                name,
                decimals: *decimals,
            });
        }
    }
    met
}

/// Store the normalized operations by id: a known one is merged, not
/// replaced (a batch holds only some of the facts of a transaction), a
/// refused one is reported as unread. Returns what was stored and whether the
/// document changed.
pub fn store_events(
    doc: &mut VaultDoc,
    made: Normalized,
    rejected: usize,
) -> (CryptoApplied, bool) {
    let mut changed = false;
    let mut out = CryptoApplied {
        unread: made.unread,
        rejected,
        ..CryptoApplied::default()
    };
    for event in made.events {
        if event.validate().is_err() {
            // Refused by the module's own rules: it cannot be fetched
            // again or deduplicated, so it must not reach the document.
            out.unread.push(engine::Unread {
                event_id: event.id.clone(),
                reason: "the operation is missing what identifies it".into(),
            });
            continue;
        }
        match doc
            .crypto_events
            .iter_mut()
            .find(|have| have.id == event.id)
        {
            Some(have) => {
                let arrival = engine::Arrival {
                    anchored: made.anchored.contains(&event.id),
                    poisoned: made.poisoned.contains(&event.id),
                };
                let mut merged = have.clone();
                engine::combine(&mut merged, event, arrival);
                if *have != merged {
                    *have = merged;
                    changed = true;
                }
                out.rewritten += 1;
            }
            None => {
                doc.crypto_events.push(event);
                out.added += 1;
                changed = true;
            }
        }
    }
    sort_events(&mut doc.crypto_events);
    (out, changed)
}

/// What the node's balances ask of the document: the adjustments to write and
/// the asset keys the network had to correct. Reads only; the caller stores
/// them with [`store_adjustments`].
pub fn reconcile(
    books: &Books<'_>,
    request: &CryptoBatch<'_>,
    target: &BatchTarget,
    batch: &SourceBatch,
) -> (Vec<CryptoTx>, BTreeSet<String>) {
    let doc = books.doc();
    let ledger = fold(books);
    let reading = Reading {
        account_id: request.account_id,
        chain: target.chain,
        address: &target.address,
        balances: network_balances(target.chain, batch),
        complete: request.balances_complete,
    };
    // What the node answers for: what it listed, plus — when the
    // reading is complete — what the module thinks this account
    // holds and the node did not mention.
    let mut network: BTreeMap<String, (CryptoAsset, Amount)> = BTreeMap::new();
    for (asset, quantity) in &reading.balances {
        let slot = network
            .entry(asset.key())
            .or_insert_with(|| (asset.clone(), Amount::ZERO));
        slot.1 = engine::math::add(slot.1, *quantity).unwrap_or(slot.1);
    }
    if reading.complete {
        for ((account_id, asset_key), held) in &ledger.balances {
            if account_id != request.account_id || held.is_zero() {
                continue;
            }
            if let Some(state) = ledger.state(asset_key) {
                network
                    .entry(asset_key.clone())
                    .or_insert_with(|| (state.asset.clone(), Amount::ZERO));
            }
        }
    }
    // Which of the two adjustments each asset gets. The history
    // is read back to the start date here, so a node balance
    // *above* what the operations account for is what the address
    // already held then — written once, as the opening. Anything
    // else is a disagreement the start date does not explain: a
    // difference of the day, which says the asset's profit is no
    // longer exact.
    let mut assigned: BTreeMap<Adjustment, Vec<(CryptoAsset, Amount)>> = BTreeMap::new();
    for (asset_key, (asset, quantity)) in network {
        let computed = ledger.balance(request.account_id, &asset_key);
        let missing = engine::math::sub(quantity, computed).unwrap_or(Amount::ZERO);
        let opened = has_opening(doc, request.account_id, &asset_key);
        let kind = if !opened && missing.is_positive() {
            Adjustment::Opening
        } else if missing.is_zero() {
            continue;
        } else {
            Adjustment::Reconciled
        };
        assigned.entry(kind).or_default().push((asset, quantity));
    }

    let mut events = Vec::new();
    let mut corrected = BTreeSet::new();
    for (kind, balances) in assigned {
        let at = match kind {
            Adjustment::Opening => target.start.as_str(),
            Adjustment::Reconciled => request.read_at,
        };
        // Every asset of this pass is named with its quantity, so
        // the pass answers for exactly its own assets.
        let narrowed = Reading {
            balances,
            complete: false,
            ..reading
        };
        let made = engine::adjust(&narrowed, &ledger, &doc.crypto_events, kind, at);
        corrected.extend(made.corrected);
        events.extend(made.events);
    }
    (events, corrected)
}

/// Store the adjustments [`reconcile`] made; reports whether the document
/// changed.
pub fn store_adjustments(doc: &mut VaultDoc, events: Vec<CryptoTx>) -> bool {
    let mut changed = false;
    for event in events {
        match doc
            .crypto_events
            .iter_mut()
            .find(|have| have.id == event.id)
        {
            Some(have) if *have == event => {}
            Some(have) => {
                *have = event;
                changed = true;
            }
            None => {
                doc.crypto_events.push(event);
                changed = true;
            }
        }
    }
    sort_events(&mut doc.crypto_events);
    changed
}

/// Whether this account already has an opening adjustment for an asset.
pub fn has_opening(doc: &VaultDoc, account_id: &str, asset_key: &str) -> bool {
    let id = engine::ids::opening(account_id, asset_key);
    doc.crypto_events.iter().any(|event| event.id == id)
}

/// Fold every operation of the module, with the corrections on top.
pub fn fold(books: &Books<'_>) -> Ledger {
    let doc = books.doc();
    let own: BTreeSet<String> = doc
        .crypto_accounts
        .iter()
        .map(|account| normalize_address(account.chain, &account.address))
        .collect();
    let prices = DocPrices {
        doc,
        market: PriceIndex::of(doc),
        linked: linked_prices(books),
    };
    let views = engine::overrides::apply(&doc.crypto_events, &doc.crypto_overrides);
    let counts = |asset: &CryptoAsset| counts_asset(doc, asset);
    engine::fold(
        &views,
        &Inputs {
            own: &own,
            prices: &prices,
            counts: &counts,
        },
    )
}

/// Holdings, cost and profit as of `at`.
pub fn crypto_totals(books: &Books<'_>, at: &str) -> CryptoTotals {
    let doc = books.doc();
    let ledger = fold(books);
    let prices = DocPrices {
        doc,
        market: PriceIndex::of(doc),
        linked: BTreeMap::new(),
    };
    let mut out = CryptoTotals {
        counted: ledger.counted,
        ..CryptoTotals::default()
    };
    for (key, state) in &ledger.assets {
        let value = value_of(state, at, &prices);
        let mut imprecise = state.imprecise();
        for (total, more) in [
            (&mut out.value_usd, value.unwrap_or(Amount::ZERO)),
            (&mut out.cost_usd, state.cost_usd),
            (&mut out.realized_usd, state.realized_usd),
            (&mut out.gas_usd, state.gas_usd),
            (&mut out.income_usd, state.income_usd),
        ] {
            // A sum that does not fit leaves the total alone and says so:
            // a panic in the money core would be worse than a number the
            // screen admits it cannot vouch for.
            match engine::math::add(*total, more) {
                Some(sum) => *total = sum,
                None => imprecise = true,
            }
        }
        out.imprecise |= imprecise;
        out.assets.push(CryptoAssetTotal {
            asset: state.asset.clone(),
            key: key.clone(),
            quantity: state.quantity,
            in_positions: state.in_positions,
            cost_usd: state.cost_usd,
            average_usd: state.average_usd(),
            value_usd: value,
            realized_usd: state.realized_usd,
            gas_usd: state.gas_usd,
            income_usd: state.income_usd,
            imprecise: state.imprecise(),
            doubts: state.doubts.iter().copied().collect(),
            stale_prices: state.stale_prices,
            linked_basis: state.linked_basis,
        });
    }
    out
}

/// The value of each account and of the module for the day of `at`, as the
/// records the document keeps. What counts as a followed account and what
/// a position is worth come from the positions area.
pub fn snapshot_records(books: &Books<'_>, at: &str) -> Vec<CryptoSnapshot> {
    let doc = books.doc();
    let ledger = fold(books);
    let prices = DocPrices {
        doc,
        market: PriceIndex::of(doc),
        linked: BTreeMap::new(),
    };
    let inputs = super::positions::crypto_position_inputs(doc);
    let counts_account =
        |account_id: &str| super::positions::crypto_account_counts(doc, account_id);
    inputs
        .with(|taking| engine::snapshot::take(&ledger, taking, &counts_account, at, &prices))
        .records
}

/// Write `records` into the document, replacing that day's snapshots.
/// Returns how many there were and whether any of them differs from what was
/// kept.
pub fn store_snapshots(doc: &mut VaultDoc, records: Vec<CryptoSnapshot>) -> (usize, bool) {
    let count = records.len();
    let mut changed = false;
    for record in records {
        match doc
            .crypto_snapshots
            .iter_mut()
            .find(|have| have.id == record.id)
        {
            // `at` is the moment of reading, not a fact about the day: the
            // same id and value is the same snapshot.
            Some(have) if have.value_usd == record.value_usd => {}
            Some(have) => {
                *have = record;
                changed = true;
            }
            None => {
                doc.crypto_snapshots.push(record);
                changed = true;
            }
        }
    }
    doc.crypto_snapshots
        .sort_by(|left, right| (&left.at, &left.id).cmp(&(&right.at, &right.id)));
    (count, changed)
}

/// The snapshots kept for the day of `at`.
pub fn snapshots_of_day(doc: &VaultDoc, at: &str) -> Vec<CryptoSnapshot> {
    let day = engine::ids::day(at);
    doc.crypto_snapshots
        .iter()
        .filter(|record| engine::ids::day(&record.at) == day)
        .cloned()
        .collect()
}

/// USD per unit implied by the fiat operation a crypto operation is
/// chained to (decision P6).
///
/// Only an unambiguous chain counts: one crypto operation with one
/// incoming leg, and fiat legs that paid for it. Anything else keeps the
/// market price and the "imprecise" mark, because a wrong cost basis is
/// worse than an honest market guess.
pub fn linked_prices(books: &Books<'_>) -> BTreeMap<String, Amount> {
    let doc = books.doc();
    let mut flows: BTreeMap<&str, Vec<&CryptoTx>> = BTreeMap::new();
    for correction in &doc.crypto_overrides {
        let Some(flow_id) = correction.flow_id.as_deref() else {
            continue;
        };
        if correction.hidden {
            continue;
        }
        if let Some(event) = doc
            .crypto_events
            .iter()
            .find(|event| event.id == correction.event_id)
        {
            flows.entry(flow_id).or_default().push(event);
        }
    }
    let mut out = BTreeMap::new();
    for (flow_id, events) in flows {
        let [event] = events[..] else { continue };
        let [leg] = &event.legs[..] else { continue };
        if leg.direction != CryptoDirection::In || !leg.amount.is_positive() {
            continue;
        }
        let paid = flow_paid_usd(books, flow_id, &event.time);
        if let Some(paid) = paid.filter(|paid| paid.is_positive())
            && let Some(unit) = engine::math::div(paid, leg.amount)
        {
            out.insert(event.id.clone(), unit);
        }
    }
    out
}

/// What the fiat side of a chain paid, in USD.
fn flow_paid_usd(books: &Books<'_>, flow_id: &str, when: &str) -> Option<Amount> {
    let mut paid = Amount::ZERO;
    let mut seen = false;
    for transaction in &books.doc().transactions {
        if transaction.meta().flow_id.as_deref() != Some(flow_id) {
            continue;
        }
        for leg in transaction.legs() {
            // Only the fiat side of the chain sets a price. A crypto leg
            // in the same chain is an operation the module sees on the
            // network itself, and counting it would pay twice.
            if !leg.amount.is_negative() || !leg.asset.is_fiat() {
                continue;
            }
            let at = if transaction.meta().timestamp.is_empty() {
                when
            } else {
                &transaction.meta().timestamp
            };
            let unit = leg
                .price_usd
                .or_else(|| crate::money::rates::price_at(books, &leg.asset.key(), at))?;
            paid = engine::math::add(paid, engine::math::mul(leg.amount.abs(), unit)?)?;
            seen = true;
        }
    }
    seen.then_some(paid)
}

/// Operations in one order on every device: by moment, then by id.
fn sort_events(events: &mut [CryptoTx]) {
    events.sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
}

/// What the node said, as the engine wants it. A quantity that does not fit
/// an exact amount is left out rather than rounded: the asset then keeps the
/// holding the operations give it.
fn network_balances(
    chain: crate::model::chain::Chain,
    batch: &SourceBatch,
) -> Vec<(CryptoAsset, Amount)> {
    batch
        .balances
        .iter()
        .filter_map(|balance| {
            let asset = match &balance.asset {
                RawAsset::Native { .. } => CryptoAsset::Native { chain },
                RawAsset::Token { contract, .. } => {
                    CryptoAsset::Token(TokenId::new(chain, contract))
                }
            };
            Some((asset, balance.amount().ok()?))
        })
        .collect()
}

/// What an asset is worth at `at`, or `None` when no price is known.
fn value_of<P: Prices>(state: &AssetState, at: &str, prices: &P) -> Option<Amount> {
    let held = state.owned();
    if !held.is_positive() {
        return Some(Amount::ZERO);
    }
    let price = prices.unit_usd(&state.asset, at, "").value?;
    engine::math::mul(held, price)
}
