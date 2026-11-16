//! The one place where the engine meets the vault (spec E5).
//!
//! The engine itself ([`crate::cryptotrack::engine`]) is pure: it knows
//! nothing about a document, a clock or a network. This file is where a
//! source's answer is stored, where the corrections and the prices are read
//! out of the document, and where "now" comes from — so everything that can
//! go wrong with a holding can be reproduced with a function call and a
//! fixture.
//!
//! What happens when a batch arrives ([`UnlockedVault::crypto_apply_batch`]):
//!
//! 1. the facts are normalized into operations and stored by id — a refetch
//!    lands on the same ids and overwrites rather than appends;
//! 2. the tokens met are registered (spec E2), which never touches a verdict
//!    the user gave;
//! 3. everything is folded again, with the user's corrections on top;
//! 4. the result is compared with the balances the node reported and the
//!    difference is written down (spec E5, point 4);
//! 5. a snapshot of the value is written for the capital history.
//!
//! The document is saved once, at the end.
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

use super::super::UnlockedVault;
use crate::Amount;
use crate::cryptotrack::engine::{
    self, Adjustment, AssetState, Doubt, Inputs, Ledger, Prices, Quote, Reading,
};
use crate::cryptotrack::prices;
use crate::cryptotrack::sources::evm::{RawAsset, SourceBatch};
use crate::cryptotrack::tokens;
use crate::error::{Error, Result};
use crate::model::cryptotrack::{
    CryptoAsset, CryptoSnapshot, CryptoTx, TokenId, normalize_address,
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
/// ([`tests::the_price_index_answers_what_the_cache_lookup_answers`]) holds
/// the two side by side so they cannot drift apart.
#[derive(Debug, Default)]
struct PriceIndex {
    points: BTreeMap<String, BTreeMap<String, Amount>>,
}

impl PriceIndex {
    fn of(doc: &VaultDoc) -> Self {
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
    fn at(&self, asset: &CryptoAsset, date: NaiveDate) -> (Option<Amount>, bool) {
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
fn day_of(time: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&engine::ids::day(time), "%Y-%m-%d").ok()
}

impl UnlockedVault {
    /// Store a source's batch, fold everything again, reconcile with the
    /// network and snapshot the value. Saves once.
    pub fn crypto_apply_batch(
        &mut self,
        request: &CryptoBatch<'_>,
        batch: &SourceBatch,
    ) -> Result<CryptoApplied> {
        let account = self
            .doc
            .crypto_accounts
            .iter()
            .find(|account| account.id == request.account_id)
            .ok_or_else(|| Error::NotFound(format!("crypto account {}", request.account_id)))?;
        let chain = account.chain;
        let address = normalize_address(chain, &account.address);
        let start = request
            .start
            .unwrap_or(&account.created_at)
            .trim()
            .to_owned();

        let made = engine::normalize(
            &batch.raw_events,
            engine::Followed {
                account_id: request.account_id,
                chain,
                address: &address,
            },
        );
        let mut out = CryptoApplied {
            unread: made.unread,
            rejected: batch.rejected.len(),
            ..CryptoApplied::default()
        };

        // Every token the batch met, registered before the fold so a
        // quarantined one is already out of the totals. Named once, however
        // many operations it appears in: registering is a write.
        let mut met: BTreeMap<String, (&str, &str, &str, u8)> = BTreeMap::new();
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
                met.entry(contract.to_lowercase())
                    .or_insert((contract, symbol, name, *decimals));
            }
        }
        for (_, (contract, symbol, name, decimals)) in met {
            // Saved once with the rest of the batch, not once per token.
            self.register_token_unsaved(&TokenId::new(chain, contract), symbol, name, decimals)?;
        }

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
            match self
                .doc
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
                    self.doc.crypto_events.push(event);
                    out.added += 1;
                }
            }
        }
        sort_events(&mut self.doc.crypto_events);

        if request.history_done {
            let reconciled = {
                let ledger = self.crypto_fold();
                let reading = Reading {
                    account_id: request.account_id,
                    chain,
                    address: &address,
                    balances: network_balances(chain, batch),
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
                let mut assigned: BTreeMap<Adjustment, Vec<(CryptoAsset, Amount)>> =
                    BTreeMap::new();
                for (asset_key, (asset, quantity)) in network {
                    let computed = ledger.balance(request.account_id, &asset_key);
                    let missing = engine::math::sub(quantity, computed).unwrap_or(Amount::ZERO);
                    let opened = self.crypto_has_opening(request.account_id, &asset_key);
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
                        Adjustment::Opening => start.as_str(),
                        Adjustment::Reconciled => request.read_at,
                    };
                    // Every asset of this pass is named with its quantity, so
                    // the pass answers for exactly its own assets.
                    let narrowed = Reading {
                        balances,
                        complete: false,
                        ..reading
                    };
                    let made =
                        engine::adjust(&narrowed, &ledger, &self.doc.crypto_events, kind, at);
                    corrected.extend(made.corrected);
                    events.extend(made.events);
                }
                (events, corrected)
            };
            out.corrected = reconciled.1;
            for event in reconciled.0 {
                match self
                    .doc
                    .crypto_events
                    .iter_mut()
                    .find(|have| have.id == event.id)
                {
                    Some(have) => *have = event,
                    None => self.doc.crypto_events.push(event),
                }
            }
            sort_events(&mut self.doc.crypto_events);
        }

        out.snapshots = self.crypto_take_snapshot(request.read_at);
        // The balances the capital is read from live in `doc.holdings`, and
        // `persist` does not re-derive them (spec E8): a batch that changed
        // what the module holds has to say so, or the wallet would show the
        // new operations with the old total.
        self.crypto_sync_holdings()?;
        Ok(out)
    }

    /// Whether this account already has an opening adjustment for an asset.
    fn crypto_has_opening(&self, account_id: &str, asset_key: &str) -> bool {
        let id = engine::ids::opening(account_id, asset_key);
        self.doc.crypto_events.iter().any(|event| event.id == id)
    }

    /// Fold every operation of the module, with the corrections on top.
    pub(super) fn crypto_fold(&self) -> Ledger {
        let own: BTreeSet<String> = self
            .doc
            .crypto_accounts
            .iter()
            .map(|account| normalize_address(account.chain, &account.address))
            .collect();
        let prices = DocPrices {
            doc: &self.doc,
            market: PriceIndex::of(&self.doc),
            linked: self.crypto_linked_prices(),
        };
        let views = engine::overrides::apply(&self.doc.crypto_events, &self.doc.crypto_overrides);
        let counts = |asset: &CryptoAsset| match asset {
            CryptoAsset::Native { .. } => true,
            CryptoAsset::Token(token) => tokens::counts_in_totals(&self.doc, token),
        };
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
    pub fn crypto_totals(&self, at: &str) -> CryptoTotals {
        let ledger = self.crypto_fold();
        let prices = DocPrices {
            doc: &self.doc,
            market: PriceIndex::of(&self.doc),
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

    /// Write the value of each account and of the module for the day of `at`,
    /// replacing that day's snapshots. Does not save on its own.
    fn crypto_take_snapshot(&mut self, at: &str) -> usize {
        let taken = {
            let ledger = self.crypto_fold();
            let prices = DocPrices {
                doc: &self.doc,
                market: PriceIndex::of(&self.doc),
                linked: BTreeMap::new(),
            };
            engine::snapshot::take(&ledger, at, &prices)
        };
        let count = taken.records.len();
        for record in taken.records {
            match self
                .doc
                .crypto_snapshots
                .iter_mut()
                .find(|have| have.id == record.id)
            {
                Some(have) => *have = record,
                None => self.doc.crypto_snapshots.push(record),
            }
        }
        self.doc
            .crypto_snapshots
            .sort_by(|left, right| (&left.at, &left.id).cmp(&(&right.at, &right.id)));
        count
    }

    /// Write the snapshots of `at` and save.
    pub fn crypto_snapshot(&mut self, at: &str) -> Result<Vec<CryptoSnapshot>> {
        self.crypto_take_snapshot(at);
        self.persist()?;
        let day = engine::ids::day(at);
        Ok(self
            .doc
            .crypto_snapshots
            .iter()
            .filter(|record| engine::ids::day(&record.at) == day)
            .cloned()
            .collect())
    }

    /// USD per unit implied by the fiat operation a crypto operation is
    /// chained to (decision P6).
    ///
    /// Only an unambiguous chain counts: one crypto operation with one
    /// incoming leg, and fiat legs that paid for it. Anything else keeps the
    /// market price and the "imprecise" mark, because a wrong cost basis is
    /// worse than an honest market guess.
    fn crypto_linked_prices(&self) -> BTreeMap<String, Amount> {
        let mut flows: BTreeMap<&str, Vec<&CryptoTx>> = BTreeMap::new();
        for correction in &self.doc.crypto_overrides {
            let Some(flow_id) = correction.flow_id.as_deref() else {
                continue;
            };
            if correction.hidden {
                continue;
            }
            if let Some(event) = self
                .doc
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
            if leg.direction != crate::model::cryptotrack::CryptoDirection::In
                || !leg.amount.is_positive()
            {
                continue;
            }
            let paid = self.crypto_flow_paid_usd(flow_id, &event.time);
            if let Some(paid) = paid.filter(|paid| paid.is_positive())
                && let Some(unit) = engine::math::div(paid, leg.amount)
            {
                out.insert(event.id.clone(), unit);
            }
        }
        out
    }

    /// What the fiat side of a chain paid, in USD.
    fn crypto_flow_paid_usd(&self, flow_id: &str, when: &str) -> Option<Amount> {
        let mut paid = Amount::ZERO;
        let mut seen = false;
        for transaction in &self.doc.transactions {
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
                    .or_else(|| self.price_at(&leg.asset.key(), at))?;
                paid = engine::math::add(paid, engine::math::mul(leg.amount.abs(), unit)?)?;
                seen = true;
            }
        }
        seen.then_some(paid)
    }
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::super::super::test_support::empty_vault;
    use super::*;
    use crate::cryptotrack::sources::evm::{BalanceSource, RawBalance, RawEvent, RawKind, Uint256};
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoDirection, CryptoOverride, CryptoPricePoint, CryptoTxKind,
        TokenVerdict,
    };
    use crate::model::store::StoreKind;
    use crate::model::transaction::{PaymentTx, PriceStamp, Transaction, TxMeta};
    use crate::service::UnlockedVault;
    use crate::storage::{SnapshotFormat, encode};

    const ME: &str = "0xabcd00000000000000000000000000000000ef01";
    const OTHER: &str = "0x2222222222222222222222222222222222222222";
    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const READ_AT: &str = "2026-03-09T12:00:00Z";
    const START: &str = "2026-03-01T00:00:00Z";

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount")
    }

    fn eth() -> CryptoAsset {
        CryptoAsset::Native {
            chain: Chain::Ethereum,
        }
    }

    fn usdc() -> CryptoAsset {
        CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC))
    }

    /// A vault with one followed address and a price for every day of March.
    fn followed() -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts.push(CryptoAccount::new(
            "cacc-1",
            "Cold EVM",
            Chain::Ethereum,
            ME,
            "2026-02-01T00:00:00Z",
        ));
        for day in 1..=31 {
            for (asset, price) in [(eth(), "2000"), (usdc(), "1")] {
                vault.doc.crypto_prices.push(CryptoPricePoint {
                    token_key: asset.key(),
                    value_usd: amount(price),
                    ts: format!("2026-03-{day:02}T00:00:00Z"),
                    source: "test".into(),
                });
            }
        }
        (dir, vault)
    }

    fn units(text: &str) -> Uint256 {
        Uint256::from_decimal(text).expect("a test quantity")
    }

    fn native() -> RawAsset {
        RawAsset::Native { decimals: 18 }
    }

    fn token() -> RawAsset {
        RawAsset::Token {
            contract: USDC.into(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
            suspect: false,
        }
    }

    /// One transfer of the history: `index` makes its hash and its day.
    fn transfer(index: u64, day: u32, asset: RawAsset, value: Uint256, inbound: bool) -> RawEvent {
        let hash = format!("0x{index:064x}");
        RawEvent {
            id: format!("evm:1:{hash}:tx"),
            chain_id: 1,
            hash,
            kind: RawKind::Transaction,
            block: 21_000_000 + index,
            time: format!("2026-03-{day:02}T10:00:00Z"),
            from: if inbound { OTHER.into() } else { ME.into() },
            to: if inbound { ME.into() } else { OTHER.into() },
            asset,
            value,
            method: None,
            succeeded: true,
            fee: None,
            payload: "{}".into(),
        }
    }

    fn balance(asset: RawAsset, value: Uint256) -> RawBalance {
        RawBalance {
            asset,
            value,
            source: BalanceSource::Node,
        }
    }

    fn batch(raw_events: Vec<RawEvent>, balances: Vec<RawBalance>) -> SourceBatch {
        SourceBatch {
            raw_events,
            balances,
            next_cursor: None,
            rejected: Vec::new(),
        }
    }

    fn request() -> CryptoBatch<'static> {
        CryptoBatch {
            account_id: "cacc-1",
            read_at: READ_AT,
            start: Some(START),
            history_done: true,
            balances_complete: true,
        }
    }

    fn eth_total(totals: &CryptoTotals) -> CryptoAssetTotal {
        totals
            .assets
            .iter()
            .find(|asset| asset.key == eth().key())
            .cloned()
            .expect("the coin is in the totals")
    }

    #[test]
    fn a_batch_leaves_the_capital_already_counted() {
        // `persist` does not re-derive `doc.holdings` (spec E8), so a batch
        // that changed what the module holds must re-derive them itself:
        // otherwise the wallet shows the new operations beside the old total
        // until the next poll.
        let (_dir, mut vault) = followed();
        let wallet = crate::model::store::Store::new("Cold EVM", StoreKind::Crypto, "#000000");
        let wallet_id = wallet.id.clone();
        vault.doc.stores.push(wallet);
        vault
            .doc
            .stores
            .iter_mut()
            .find(|store| store.id == wallet_id)
            .expect("the wallet is there")
            .crypto_account_id = Some("cacc-1".to_string());

        let seen = batch(
            vec![transfer(1, 5, native(), units("2000000000000000000"), true)],
            vec![balance(native(), units("2000000000000000000"))],
        );
        vault
            .crypto_apply_batch(&request(), &seen)
            .expect("applied");

        let held = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == wallet_id)
            .expect("the batch left the balance in the holdings");
        assert_eq!(held.quantity, amount("2"));
    }

    #[test]
    fn the_starting_amount_and_the_operations_give_the_network_balance() {
        let (_dir, mut vault) = followed();
        // The module saw 1 ETH arrive; the node says the address holds 3.
        let seen = batch(
            vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
            vec![balance(native(), units("3000000000000000000"))],
        );
        let applied = vault
            .crypto_apply_batch(&request(), &seen)
            .expect("applied");
        assert_eq!(applied.added, 1);
        assert!(applied.corrected.contains(&eth().key()));

        let opening = vault
            .doc
            .crypto_events
            .iter()
            .find(|event| event.id == "copen:cacc-1:ethereum|native")
            .expect("an opening adjustment");
        assert_eq!(opening.legs[0].amount, amount("2"));
        assert_eq!(opening.legs[0].direction, CryptoDirection::In);
        assert_eq!(opening.time, "2026-03-01T00:00:00Z");

        let totals = vault.crypto_totals(READ_AT);
        let coin = eth_total(&totals);
        assert_eq!(
            coin.quantity,
            amount("3"),
            "the module agrees with the node"
        );
        assert_eq!(coin.value_usd, Some(amount("6000")));
        assert!(
            coin.imprecise,
            "an opening holding is a guess about its cost"
        );
        assert!(coin.doubts.contains(&Doubt::OpeningAtMarket));
    }

    #[test]
    fn a_correction_survives_a_resync_and_the_balance_still_matches() {
        let (_dir, mut vault) = followed();
        let seen = batch(
            vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
            vec![balance(native(), units("3000000000000000000"))],
        );
        vault
            .crypto_apply_batch(&request(), &seen)
            .expect("applied");
        let event_id = "cev:cacc-1:ethereum:0x0000000000000000000000000000000000000000000000000000000000000001";
        assert!(
            vault
                .doc
                .crypto_events
                .iter()
                .any(|event| event.id == event_id)
        );

        // The user says the arrival was a reward, not a purchase.
        vault.doc.crypto_overrides.push(CryptoOverride {
            id: "cov-1".into(),
            event_id: event_id.into(),
            kind: Some(CryptoTxKind::Claim),
            comment: Some("staking reward".into()),
            categories: Vec::new(),
            flow_id: None,
            contact_id: None,
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: "2026-03-10T00:00:00Z".into(),
        });
        let before = vault.crypto_totals(READ_AT);
        assert_eq!(eth_total(&before).income_usd, amount("2000"));

        // The same pages arrive again: the events are rewritten, the
        // correction is not.
        let applied = vault
            .crypto_apply_batch(&request(), &seen)
            .expect("resynced");
        assert_eq!(applied.added, 0, "nothing new");
        assert_eq!(applied.rewritten, 1);
        assert_eq!(vault.doc.crypto_overrides.len(), 1);
        assert_eq!(
            vault.doc.crypto_overrides[0].comment.as_deref(),
            Some("staking reward")
        );
        let after = vault.crypto_totals(READ_AT);
        assert_eq!(eth_total(&after).income_usd, amount("2000"));
        assert_eq!(eth_total(&after).quantity, amount("3"));
        assert_eq!(before.value_usd, after.value_usd);
        // And a second opening adjustment was not stacked on the first.
        assert_eq!(
            vault
                .doc
                .crypto_events
                .iter()
                .filter(|event| event.id.starts_with("copen:"))
                .count(),
            1
        );
    }

    #[test]
    fn a_second_run_changes_no_total_whatever_order_the_batches_came_in() {
        // A small history: arrivals, a departure, a token, in several pages.
        let facts = vec![
            transfer(1, 2, native(), units("2000000000000000000"), true),
            transfer(2, 3, native(), units("500000000000000000"), false),
            transfer(3, 4, token(), units("1500000000"), true),
            transfer(4, 5, token(), units("500000000"), false),
            transfer(5, 6, native(), units("250000000000000000"), true),
            transfer(6, 7, token(), units("100000000"), true),
        ];
        let balances = vec![
            balance(native(), units("1750000000000000000")),
            balance(token(), units("1100000000")),
        ];

        let straight = {
            let (_dir, mut vault) = followed();
            vault
                .crypto_apply_batch(&request(), &batch(facts.clone(), balances.clone()))
                .expect("applied");
            vault.crypto_totals(READ_AT)
        };
        // No adjustment was needed: the operations already add up.
        assert!(!straight.imprecise, "{:?}", straight.assets);

        // Every page order, in two batches, then the whole thing again.
        let mut seed = 0x5eedu64;
        for _ in 0..20 {
            let mut shuffled = facts.clone();
            for index in (1..shuffled.len()).rev() {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                let pick = (seed >> 33) as usize % (index + 1);
                shuffled.swap(index, pick);
            }
            let (_dir, mut vault) = followed();
            let (first, second) = shuffled.split_at(shuffled.len() / 2);
            let mut page = request();
            page.history_done = false;
            vault
                .crypto_apply_batch(&page, &batch(first.to_vec(), Vec::new()))
                .expect("first page");
            vault
                .crypto_apply_batch(&request(), &batch(second.to_vec(), balances.clone()))
                .expect("second page");
            assert_eq!(vault.crypto_totals(READ_AT), straight, "order {shuffled:?}");

            // A second run over the same facts changes nothing.
            vault
                .crypto_apply_batch(&request(), &batch(shuffled.clone(), balances.clone()))
                .expect("again");
            assert_eq!(vault.crypto_totals(READ_AT), straight, "second run");
        }
    }

    #[test]
    fn two_devices_hold_the_same_bytes_for_the_same_facts() {
        let facts = vec![
            transfer(1, 2, native(), units("2000000000000000000"), true),
            transfer(2, 3, token(), units("1500000000"), true),
            transfer(3, 4, native(), units("500000000000000000"), false),
        ];
        let balances = vec![
            balance(native(), units("1500000000000000000")),
            balance(token(), units("1500000000")),
        ];
        let bytes_of = |pages: Vec<Vec<RawEvent>>| {
            let (_dir, mut vault) = followed();
            let last = pages.len() - 1;
            for (index, page) in pages.into_iter().enumerate() {
                let mut step = request();
                step.history_done = index == last;
                let with_balances = if index == last {
                    balances.clone()
                } else {
                    Vec::new()
                };
                vault
                    .crypto_apply_batch(&step, &batch(page, with_balances))
                    .expect("applied");
            }
            serde_json::to_vec(&vault.doc.crypto_events).expect("events serialize")
        };
        let one_device = bytes_of(vec![facts.clone()]);
        let other_device = bytes_of(vec![
            vec![facts[2].clone()],
            vec![facts[1].clone(), facts[0].clone()],
        ]);
        assert_eq!(
            String::from_utf8_lossy(&one_device),
            String::from_utf8_lossy(&other_device)
        );
    }

    /// Counts the saves a vault asks for.
    struct CountSaves(std::sync::atomic::AtomicUsize);

    impl crate::service::saving::SaveHook for CountSaves {
        fn dirty(&self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Saves one batch of `tokens` junk airdrops costs.
    fn saves_for_a_batch_of(tokens: u64) -> usize {
        let (_dir, mut vault) = followed();
        let saves = std::sync::Arc::new(CountSaves(std::sync::atomic::AtomicUsize::new(0)));
        vault.set_write_behind(Some(saves.clone()));
        let facts = (1..=tokens)
            .map(|index| {
                let junk = RawAsset::Token {
                    contract: format!("0x{index:040x}"),
                    symbol: format!("JUNK{index}"),
                    name: "Junk".into(),
                    decimals: 18,
                    suspect: true,
                };
                transfer(index, 5, junk, units("1000"), true)
            })
            .collect();
        vault
            .crypto_apply_batch(&request(), &batch(facts, Vec::new()))
            .expect("applied");
        assert_eq!(vault.doc.crypto_tokens.len() as u64, tokens);
        saves.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn a_batch_saves_once_however_many_new_tokens_it_met() {
        let one = saves_for_a_batch_of(1);
        assert_eq!(
            saves_for_a_batch_of(40),
            one,
            "the saves must not grow with the number of tokens"
        );
    }

    #[test]
    fn a_quarantined_token_is_outside_the_totals_until_it_is_confirmed() {
        let (_dir, mut vault) = followed();
        let scam = RawAsset::Token {
            contract: "0x00000000000000000000000000000000deadbeef".into(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
            suspect: true,
        };
        let scam_id = TokenId::new(
            Chain::Ethereum,
            "0x00000000000000000000000000000000deadbeef",
        );
        vault.doc.crypto_prices.push(CryptoPricePoint {
            token_key: CryptoAsset::Token(scam_id.clone()).key(),
            value_usd: amount("1000000"),
            ts: "2026-03-05T00:00:00Z".into(),
            source: "test".into(),
        });
        let seen = batch(
            vec![transfer(1, 5, scam, units("1000000000"), true)],
            Vec::new(),
        );
        let mut page = request();
        page.history_done = false;
        vault.crypto_apply_batch(&page, &seen).expect("applied");
        let totals = vault.crypto_totals(READ_AT);
        assert_eq!(
            totals.value_usd,
            Amount::ZERO,
            "a scam airdrop is worth nothing here"
        );
        assert!(totals.assets.is_empty());

        // The user takes it in: now it counts, with the same events.
        vault
            .doc
            .crypto_tokens
            .iter_mut()
            .find(|record| record.id() == scam_id)
            .expect("the token was registered")
            .verdict = Some(TokenVerdict::Confirmed);
        let totals = vault.crypto_totals(READ_AT);
        assert_eq!(totals.assets.len(), 1);
        assert_eq!(totals.value_usd, amount("1000000000"));
    }

    #[test]
    fn a_linked_fiat_operation_sets_the_cost_basis_above_the_market() {
        let (_dir, mut vault) = followed();
        let seen = batch(
            vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
            Vec::new(),
        );
        let mut page = request();
        page.history_done = false;
        vault.crypto_apply_batch(&page, &seen).expect("applied");
        let event_id = "cev:cacc-1:ethereum:0x0000000000000000000000000000000000000000000000000000000000000001";

        // Without a chain, the market price of the day is the basis.
        assert_eq!(
            eth_total(&vault.crypto_totals(READ_AT)).average_usd,
            Some(amount("2000"))
        );

        // The user says this arrival is the other half of a payment: 2 400
        // dollars left the bank for it.
        let mut meta = TxMeta::new("bought ETH");
        meta.id = "tx-1".into();
        meta.timestamp = "2026-03-05T09:00:00Z".into();
        meta.flow_id = Some("flow-1".into());
        vault.doc.transactions.push(Transaction::Payment(PaymentTx {
            meta,
            store_id: "store-1".into(),
            asset: crate::model::store::AssetRef::fiat("usd", "US Dollar"),
            amount: amount("2400"),
            fee: None,
            to: None,
            details: crate::model::transaction::KindDetails::default(),
            price: PriceStamp {
                price_usd: Some(amount("1")),
                ..PriceStamp::default()
            },
        }));
        vault.doc.crypto_overrides.push(CryptoOverride {
            id: "cov-1".into(),
            event_id: event_id.into(),
            kind: None,
            comment: None,
            categories: Vec::new(),
            flow_id: Some("flow-1".into()),
            contact_id: None,
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: "2026-03-10T00:00:00Z".into(),
        });
        let coin = eth_total(&vault.crypto_totals(READ_AT));
        assert_eq!(
            coin.average_usd,
            Some(amount("2400")),
            "the user's own number beats the market quote"
        );
        assert!(coin.linked_basis);
        assert_eq!(coin.cost_usd, amount("2400"));
    }

    #[test]
    fn a_chain_paid_in_another_currency_is_converted_at_the_rate_of_the_day() {
        let (_dir, mut vault) = followed();
        let seen = batch(
            vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
            Vec::new(),
        );
        let mut page = request();
        page.history_done = false;
        vault.crypto_apply_batch(&page, &seen).expect("applied");
        let event_id = "cev:cacc-1:ethereum:0x0000000000000000000000000000000000000000000000000000000000000001";

        // 200 000 roubles at 100 to the dollar: the chain says this coin cost
        // 2 000 dollars, and no price of the day is consulted.
        vault.apply_fx(&[crate::model::vault::FxPoint {
            currency: "rub".into(),
            per_usd: amount("100"),
            ts: "2026-03-05T00:00:00Z".into(),
            source: "test".into(),
        }]);
        let mut meta = TxMeta::new("bought ETH with roubles");
        meta.id = "tx-1".into();
        meta.timestamp = "2026-03-05T09:00:00Z".into();
        meta.flow_id = Some("flow-1".into());
        vault.doc.transactions.push(Transaction::Payment(PaymentTx {
            meta,
            store_id: "store-1".into(),
            asset: crate::model::store::AssetRef::fiat("rub", "Rouble"),
            amount: amount("200000"),
            fee: None,
            to: None,
            details: crate::model::transaction::KindDetails::default(),
            price: PriceStamp::default(),
        }));
        vault.doc.crypto_overrides.push(CryptoOverride {
            id: "cov-1".into(),
            event_id: event_id.into(),
            kind: None,
            comment: None,
            categories: Vec::new(),
            flow_id: Some("flow-1".into()),
            contact_id: None,
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: "2026-03-10T00:00:00Z".into(),
        });
        let coin = eth_total(&vault.crypto_totals(READ_AT));
        assert_eq!(coin.average_usd, Some(amount("2000")));
        assert!(coin.linked_basis);
    }

    #[test]
    fn a_declared_basis_is_used_for_what_was_held_before_the_start_date() {
        let (_dir, mut vault) = followed();
        let seen = batch(
            vec![transfer(1, 5, token(), units("100000000"), true)],
            vec![balance(token(), units("1100000000"))],
        );
        vault
            .crypto_apply_batch(&request(), &seen)
            .expect("applied");
        let token_id = TokenId::new(Chain::Ethereum, USDC);
        let before = vault.crypto_totals(READ_AT);
        let held = before
            .assets
            .iter()
            .find(|asset| asset.key == usdc().key())
            .expect("the token");
        assert_eq!(held.quantity, amount("1100"));
        assert!(held.doubts.contains(&Doubt::OpeningAtMarket));

        // The user says the old thousand cost 0.95 each.
        vault
            .crypto_set_token_basis(&token_id, Some(amount("0.95")))
            .expect("a basis");
        let after = vault.crypto_totals(READ_AT);
        let held = after
            .assets
            .iter()
            .find(|asset| asset.key == usdc().key())
            .expect("the token");
        // 1 000 before the start at 0.95 plus 100 seen arriving at 1.00.
        assert_eq!(held.cost_usd, amount("1050"));
        assert!(held.linked_basis);
        assert!(!held.doubts.contains(&Doubt::OpeningAtMarket));
    }

    #[test]
    fn the_snapshots_of_a_day_are_written_once_and_rewritten_in_place() {
        let (_dir, mut vault) = followed();
        let seen = batch(
            vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
            Vec::new(),
        );
        let mut page = request();
        page.history_done = false;
        vault.crypto_apply_batch(&page, &seen).expect("applied");
        // One for the account, one for the module.
        assert_eq!(vault.doc.crypto_snapshots.len(), 2);
        let module = vault
            .doc
            .crypto_snapshots
            .iter()
            .find(|record| record.account_id.is_none())
            .expect("the module's own");
        assert_eq!(module.value_usd, amount("2000.00"));

        let same_day = vault
            .crypto_snapshot("2026-03-09T23:00:00Z")
            .expect("again");
        assert_eq!(same_day.len(), 2);
        assert_eq!(vault.doc.crypto_snapshots.len(), 2, "one record per day");
        let next_day = vault
            .crypto_snapshot("2026-03-10T10:00:00Z")
            .expect("later");
        assert_eq!(next_day.len(), 2);
        assert_eq!(vault.doc.crypto_snapshots.len(), 4);
    }

    #[test]
    fn the_price_index_answers_what_the_cache_lookup_answers() {
        let (_dir, mut vault) = followed();
        // A zero (which E4 ignores), two days of one asset, a gap, and a
        // second asset that has nothing at all.
        vault.doc.crypto_prices.push(CryptoPricePoint {
            token_key: eth().key(),
            value_usd: Amount::ZERO,
            ts: "2026-04-01T00:00:00Z".into(),
            source: "test".into(),
        });
        vault.doc.crypto_prices.push(CryptoPricePoint {
            token_key: eth().key(),
            value_usd: amount("2500"),
            ts: "2026-04-05T00:00:00Z".into(),
            source: "test".into(),
        });
        let nothing = CryptoAsset::Token(TokenId::new(
            Chain::Ethereum,
            "0x00000000000000000000000000000000deadbeef",
        ));
        let index = PriceIndex::of(&vault.doc);
        for asset in [eth(), usdc(), nothing] {
            for day in [
                "2026-02-28",
                "2026-03-01",
                "2026-03-15",
                "2026-04-01",
                "2026-04-05",
                "2026-04-06",
            ] {
                let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").expect("a date");
                let want = prices::lookup(&vault.doc, &asset, date);
                assert_eq!(
                    index.at(&asset, date),
                    (want.value, want.stale),
                    "{} on {day}",
                    asset.key()
                );
            }
        }
    }

    #[test]
    fn an_unknown_account_is_refused() {
        let (_dir, mut vault) = followed();
        let mut missing = request();
        missing.account_id = "nope";
        assert!(matches!(
            vault.crypto_apply_batch(&missing, &batch(Vec::new(), Vec::new())),
            Err(Error::NotFound(_))
        ));
    }

    /// A `Prices` that asks E4's cache the way a fold would without an index:
    /// one scan of `crypto_prices` per leg.
    struct Scanning<'doc>(&'doc VaultDoc);

    impl Prices for Scanning<'_> {
        fn unit_usd(&self, asset: &CryptoAsset, time: &str, _event_id: &str) -> Quote {
            let Some(date) = day_of(time) else {
                return Quote::default();
            };
            let found = prices::lookup(self.0, asset, date);
            Quote {
                value: found.value,
                stale: found.stale,
                linked: false,
            }
        }
    }

    /// What 1 000, 10 000 and 50 000 events cost: the fold, the sync stamp
    /// and the save. Not part of the gate; run it on purpose, and in release
    /// (a debug build is some ten times slower here):
    ///
    /// ```text
    /// cargo test --release -p trade-core crypto_engine_volume -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement, not a check: run it with --ignored --nocapture"]
    fn crypto_engine_volume() {
        println!(
            "{:>7} {:>10} {:>10} {:>10} {:>10} {:>12} {:>11} {:>11}",
            "events",
            "fold",
            "stamp",
            "persist",
            "file KiB",
            "pack KiB",
            "fold/5k px",
            "scan/5k px"
        );
        for count in [1_000usize, 10_000, 50_000] {
            let (_dir, mut vault) = followed();
            let facts: Vec<RawEvent> = (0..count)
                .map(|index| {
                    transfer(
                        index as u64 + 1,
                        (index % 28 + 1) as u32,
                        if index % 2 == 0 { native() } else { token() },
                        units("1000000000000000"),
                        index % 3 != 0,
                    )
                })
                .collect();
            let made = engine::normalize(
                &facts,
                engine::Followed {
                    account_id: "cacc-1",
                    chain: Chain::Ethereum,
                    address: ME,
                },
            );
            vault.doc.crypto_events = made.events;
            assert_eq!(vault.doc.crypto_events.len(), count);

            let started = Instant::now();
            let totals = vault.crypto_totals(READ_AT);
            let fold = started.elapsed();
            assert_eq!(totals.counted, count);

            // The same fold against a price cache of the size a few years of
            // daily history really has, once through the index and once
            // through E4's scan, which is what the engine would do without
            // one.
            for day in 0..5_000u32 {
                let date = NaiveDate::from_ymd_opt(2020, 1, 1)
                    .expect("a date")
                    .checked_add_days(chrono::Days::new(u64::from(day)))
                    .expect("a later date");
                vault.doc.crypto_prices.push(CryptoPricePoint {
                    token_key: eth().key(),
                    value_usd: amount("2000"),
                    ts: prices::date_ts(date),
                    source: "test".into(),
                });
            }
            let started = Instant::now();
            let fat = vault.crypto_totals(READ_AT);
            let indexed = started.elapsed();
            assert_eq!(fat.counted, count);
            let own: BTreeSet<String> = [ME.to_owned()].into_iter().collect();
            let counts = |_asset: &CryptoAsset| true;
            let scanning = Scanning(&vault.doc);
            let views = engine::overrides::apply(&vault.doc.crypto_events, &[]);
            let started = Instant::now();
            let scanned = engine::fold(
                &views,
                &Inputs {
                    own: &own,
                    prices: &scanning,
                    counts: &counts,
                },
            );
            let scan = started.elapsed();
            assert_eq!(scanned.counted, count);
            vault.doc.crypto_prices.truncate(62);

            vault.track_changes().expect("the first pass remembers");
            let started = Instant::now();
            vault.track_changes().expect("the diff every save pays");
            let stamp = started.elapsed();

            let started = Instant::now();
            vault.persist_now().expect("saved");
            let persist = started.elapsed();

            let file = std::fs::metadata(&vault.path).expect("a file").len();
            let pack = encode(&vault.doc, SnapshotFormat::Pack)
                .expect("packed")
                .len();
            println!(
                "{count:>7} {:>7.0?} {:>7.0?} {:>7.0?} {:>10} {:>12} {:>9.0?} {:>9.0?}",
                fold,
                stamp,
                persist,
                file / 1024,
                pack / 1024,
                indexed,
                scan
            );
        }
    }
}
