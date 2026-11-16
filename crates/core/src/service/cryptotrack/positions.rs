//! What a DeFi position is worth, and what it earned (spec D0).
//!
//! This is where the protocol layer meets the vault: the adapters
//! ([`crate::cryptotrack::protocols`]) read quantities and facts, the pure
//! arithmetic ([`crate::cryptotrack::protocols::fees`]) turns two numbers
//! into a third, and this file is the one place that knows the prices, the
//! stored operations and what "now" is.
//!
//! # Nothing derived is stored
//!
//! The vault keeps only what a reading cannot be recomputed from:
//! `doc.crypto_positions` — the legs and the name — and the position's
//! operations in `doc.crypto_events`, each carrying
//! [`TrackingRef::Position`]. Every money figure the screen shows is folded
//! again on every read, like `doc.holdings`, so the vault schema does not
//! change for DeFi (`AGENTS.md`, rule 5, and the promise E0 made when it
//! added the field).
//!
//! What that buys, and what it costs:
//!
//! * **invested at the price of the deposit** comes from the position's own
//!   `Stake` operations priced at their day, and a withdrawal takes its share
//!   back out, so the figure is the capital really left in;
//! * **the collected fees** are its `Claim` operations, priced the same way;
//! * **the gas** is the gas of its operations, priced the same way;
//! * **the value now** is the stored legs at today's prices;
//! * **the uncollected fees** cannot be derived from anything stored — they
//!   are a reading of a moment, like a node's balance. The device that talked
//!   to the chain hands its latest readings in
//!   ([`UnlockedVault::crypto_list_positions`] takes them as an argument),
//!   and without one the figure is absent rather than a zero, except for a
//!   kind of position that keeps no fees apart from its principal
//!   ([`crate::cryptotrack::protocols::PositionKind::keeps_fees_apart`]) —
//!   lending and staking accrue inside
//!   the balance, so there a zero is the truth and not a guess.
//!
//! # A position is counted once
//!
//! A protocol hands over a receipt for what it took: `aUSDC` for `USDC`,
//! `stETH` for `ETH`, an NFT for a Uniswap range. That receipt sits in the
//! wallet and a node reports it, while the supply itself is already in the
//! capital as the position. Counting both doubles the money.
//!
//! The engine already refuses the receipt where it can see it: a leg of a
//! `Stake` or an `Unstake` operation that comes *in* is
//! `Action::Ignore`, so it gets no holding of its own
//! ([`crate::cryptotrack::engine::basis`]). What is left is the node: a
//! balance read straight off the chain becomes a reconciliation adjustment
//! and, through it, a holding. [`UnlockedVault::crypto_position_receipts`]
//! names those assets from the stored operations alone — so every device
//! agrees, synced or not — and `totals.rs` leaves them out of the capital and
//! says so ([`super::totals::CryptoLeftOut::InPosition`]).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use crate::Amount;
use crate::cryptotrack::engine::{ids, math};
use crate::cryptotrack::prices;
use crate::cryptotrack::protocols::{
    AssetAmount, NetParts, PositionKey, PositionReading, PositionStatus, ProtocolReview,
    annual_percentage_rate, fees_usd, impermanent_loss, impermanent_loss_ratio, is_closed, kind_of,
    net_result, receipt_tokens, record_id_prefixes, worth_usd,
};
use crate::cryptotrack::tokens;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::positions::{PositionAssetView, PositionView};
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoLeg, CryptoPosition, CryptoTx, CryptoTxKind,
};
use crate::pricing::history::unix_secs;
use crate::service::price_info::PriceOrigin;

use super::super::UnlockedVault;

/// Whether a re-read position says what the stored one already says: the
/// legs, the name and the receipts, whatever the moment of the reading.
fn same_but_for_the_moment(stored: &CryptoPosition, read: &CryptoPosition) -> bool {
    CryptoPosition {
        updated_at: read.updated_at.clone(),
        ..stored.clone()
    } == *read
}

/// What a pass of the protocol adapters stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CryptoPositionsApplied {
    /// Positions added.
    pub added: usize,
    /// Positions that were already there and whose legs, name or receipts
    /// changed, so they were rewritten.
    pub rewritten: usize,
    /// Operations of those positions added or changed.
    pub events: usize,
    /// Positions closed because their protocol, asked successfully, no
    /// longer reports them.
    pub closed: usize,
    /// Records an older build keyed without the account that were renamed to
    /// the current key, or dropped because the current key already held the
    /// same position.
    pub migrated: usize,
}

impl CryptoPositionsApplied {
    /// Whether the pass stored nothing at all.
    pub fn is_unchanged(&self) -> bool {
        *self == Self::default()
    }
}

impl UnlockedVault {
    /// Store what the protocol adapters read for one account: the positions
    /// and their operations. Saves once, and re-derives the holdings, because
    /// a position changes what the capital counts.
    ///
    /// `reviewed` is what each protocol that *answered* still sees
    /// ([`ProtocolReview`]). A stored position of this account that such a
    /// review no longer lists is closed: its legs go to zero and the row
    /// stays, so its history and the periods it was open in survive, and the
    /// next reading under the same id simply overwrites it. A protocol that
    /// refused has no review, and nothing is closed on its word.
    ///
    /// A reading of another account is ignored: a pass answers for exactly
    /// the account it was started for. A pass with nothing to change saves
    /// nothing.
    pub fn crypto_apply_positions(
        &mut self,
        account_id: &str,
        readings: &[PositionReading],
        events: &[CryptoTx],
        reviewed: &[ProtocolReview],
    ) -> Result<CryptoPositionsApplied> {
        let mut out = CryptoPositionsApplied {
            migrated: self.migrate_legacy_position_records(account_id),
            ..CryptoPositionsApplied::default()
        };
        for reading in readings
            .iter()
            .filter(|reading| reading.account_id == account_id)
        {
            let record = reading.stored();
            match self
                .doc
                .crypto_positions
                .iter_mut()
                .find(|have| have.id == record.id)
            {
                // The same legs, name and receipts as stored: only the moment
                // of the reading differs. Rewriting that would encrypt the
                // vault, tell the screens and queue a sync record every pass
                // for nothing, so it is not a change.
                Some(have) if same_but_for_the_moment(have, &record) => {}
                Some(have) => {
                    *have = record;
                    out.rewritten += 1;
                }
                None => {
                    self.doc.crypto_positions.push(record);
                    out.added += 1;
                }
            }
        }
        for review in reviewed {
            for record in self.doc.crypto_positions.iter_mut().filter(|record| {
                record.account_id == account_id
                    && record.chain == review.chain
                    && record.protocol == review.protocol
                    && !review.kept.contains(&record.id)
                    && !is_closed(&record.legs)
                    && !readings
                        .iter()
                        .any(|reading| reading.record_id() == record.id)
            }) {
                for leg in &mut record.legs {
                    leg.amount = Amount::ZERO;
                }
                record.updated_at.clone_from(&review.read_at);
                out.closed += 1;
            }
        }
        for event in events.iter().filter(|event| event.account_id == account_id) {
            match self
                .doc
                .crypto_events
                .iter_mut()
                .find(|have| have.id == event.id)
            {
                Some(have) if *have == *event => {}
                Some(have) => {
                    *have = event.clone();
                    out.events += 1;
                }
                None => {
                    self.doc.crypto_events.push(event.clone());
                    out.events += 1;
                }
            }
        }
        if out.is_unchanged() {
            return Ok(out);
        }
        self.doc
            .crypto_positions
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.doc
            .crypto_events
            .sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
        // The capital is read off `doc.holdings` (spec E8) and `persist` does
        // not re-derive it, so a pass that changed what a protocol holds has
        // to say so.
        self.crypto_sync_holdings()?;
        Ok(out)
    }

    /// Bring this account's records written under the old key
    /// (`<protocol>|<chain>|<id>`, no account) to the current one.
    ///
    /// The old key was shared by every account holding the same Aave reserve
    /// or stETH, so such a record carries the legs of whichever account wrote
    /// last, and `account_id` says which. It is renamed in place (history of
    /// a closed position survives, ADR-02); when the current key already
    /// holds the position — an older build re-created the old record after
    /// a newer one migrated — the old one is dropped, since the newer build
    /// reads the chain itself. The other account's record that the old key
    /// had overwritten comes back with that account's next pass.
    ///
    /// Returns how many records it touched.
    fn migrate_legacy_position_records(&mut self, account_id: &str) -> usize {
        let wanted: Vec<(String, String)> = self
            .doc
            .crypto_positions
            .iter()
            .filter(|record| record.account_id == account_id && is_legacy_keyed(record))
            .map(|record| (record.id.clone(), current_id_of(record)))
            .collect();
        for (legacy, current) in &wanted {
            if self
                .doc
                .crypto_positions
                .iter()
                .any(|have| have.id == *current)
            {
                self.doc.crypto_positions.retain(|have| have.id != *legacy);
            } else if let Some(record) = self
                .doc
                .crypto_positions
                .iter_mut()
                .find(|have| have.id == *legacy)
            {
                record.id.clone_from(current);
            }
        }
        wanted.len()
    }

    /// Whether `record` is keyed the old way and the current key holds the
    /// same position of the same account.
    pub(super) fn is_shadowed_legacy_position(&self, record: &CryptoPosition) -> bool {
        if !is_legacy_keyed(record) {
            return false;
        }
        let current = current_id_of(record);
        self.doc
            .crypto_positions
            .iter()
            .any(|have| have.id == current)
    }

    /// Every stored position with its money, newest reading first.
    ///
    /// `live` is what this device's own last pass read
    /// (`crates/app/src/cryptotrack/jobs.rs`); an empty slice is a device
    /// that has only synced, and then the figures a reading alone can give
    /// are absent rather than zero.
    ///
    /// `at` is the moment an open position's holding period is measured to
    /// (RFC3339 UTC), the way [`UnlockedVault::crypto_totals`] takes one: the
    /// caller owns the clock, so the whole fold is a function of its inputs.
    /// It does not choose the prices — those are the newest the vault
    /// knows.
    pub fn crypto_list_positions(&self, live: &[PositionReading], at: &str) -> Vec<PositionView> {
        let mut out: Vec<PositionView> = self
            .doc
            .crypto_positions
            .iter()
            // A record of the old key that the current key already holds is a
            // stale twin (an older build on another device wrote it): counting
            // both would count the position twice.
            .filter(|record| !self.is_shadowed_legacy_position(record))
            .map(|record| {
                let reading = live.iter().find(|reading| reading.record_id() == record.id);
                self.position_view(record, reading, at)
            })
            .collect();
        // Open positions first, then the newest reading: a closed one is
        // history and belongs under what is still running.
        out.sort_by(|left, right| {
            (left.status != PositionStatus::Open, &right.updated_at)
                .cmp(&(right.status != PositionStatus::Open, &left.updated_at))
        });
        out
    }

    /// Asset keys the module holds only as the receipt of a position (an
    /// aToken, stETH, a position NFT), so the capital counts the money once.
    ///
    /// Two sources, unioned, and both work on a device that has never talked
    /// to a chain:
    ///
    /// * the stored operations: what came *in* on a `Stake` and what went
    ///   *out* on an `Unstake` is the receipt by construction — those are
    ///   exactly the legs the fold already ignores;
    /// * the protocol's own table
    ///   ([`crate::cryptotrack::protocols::receipt_tokens`]), for a protocol
    ///   the account holds a position in. A position read without any history
    ///   behind it still names its receipt that way.
    pub(in crate::service) fn crypto_position_receipts(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for event in &self.doc.crypto_events {
            let receipt_side = match event.kind {
                CryptoTxKind::Stake => CryptoDirection::In,
                CryptoTxKind::Unstake => CryptoDirection::Out,
                _ => continue,
            };
            for leg in event
                .legs
                .iter()
                .filter(|leg| leg.direction == receipt_side)
            {
                out.insert(leg.asset.key());
            }
        }
        for record in &self.doc.crypto_positions {
            // What the adapter saw on this very position, and what the
            // protocol publishes in advance for the whole market: the first
            // covers a reserve no table could list, the second a device that
            // only synced and never read the chain itself.
            for asset in &record.receipts {
                out.insert(asset.key());
            }
            for asset in receipt_tokens(&record.protocol, record.chain) {
                out.insert(asset.key());
            }
        }
        out
    }

    /// One stored position, with the device's own reading of it when there is
    /// one.
    fn position_view(
        &self,
        record: &CryptoPosition,
        reading: Option<&PositionReading>,
        now: &str,
    ) -> PositionView {
        let key = PositionKey::new(&record.protocol, record.chain, position_id_of(record));
        let account_id = record.account_id.as_str();
        let legs: &[CryptoLeg] =
            reading.map_or(record.legs.as_slice(), |read| read.legs.as_slice());
        let kind = reading.map_or_else(|| kind_of(legs), |read| read.kind);
        let status = reading.map_or_else(
            || {
                if is_closed(legs) {
                    PositionStatus::Closed
                } else {
                    PositionStatus::Open
                }
            },
            |read| read.status,
        );
        let history = self.position_history(&key, account_id);
        let value_usd = self.legs_value_usd(legs);
        // With nothing stored under this position the base for the comparison
        // with holding is unknown, not zero: `worth_usd` of an empty list is a
        // real zero, and that zero would turn the whole balance into an
        // accrual and the whole value into impermanent loss.
        let hold_usd = history
            .first_at
            .is_some()
            .then(|| worth_usd(&history.deposited, &|asset| self.price_now_usd(asset)))
            .flatten();
        let compares = kind.compares_with_holding();
        let fees_uncollected_usd = match reading {
            Some(read) => fees_usd(read.fees.uncollected.as_ref(), &|asset| {
                self.price_now_usd(asset)
            }),
            // A kind that keeps no fees apart from its principal has a real
            // zero here on any device; a pool's uncollected fees are a
            // reading of a moment and absent without one.
            None if !kind.keeps_fees_apart() => Some(Amount::ZERO),
            None => None,
        };
        let parts = NetParts {
            value_usd,
            invested_usd: history.invested_usd,
            fees_collected_usd: history.fees_collected_usd,
            fees_uncollected_usd,
            gas_usd: history.gas_usd,
        };
        let net_usd = net_result(&parts);
        let opened_at = reading
            .and_then(|read| read.opened_at.clone())
            .or_else(|| history.first_at.clone());
        let closed_at = reading.and_then(|read| read.closed_at.clone()).or_else(|| {
            (status == PositionStatus::Closed)
                .then(|| history.last_at.clone())
                .flatten()
        });
        // A closed position's figures are frozen: its period ends where it
        // was closed, so the rate is not diluted by the time since.
        let until = closed_at.as_deref().unwrap_or(now);
        let apr = held_secs(opened_at.as_deref(), until)
            .and_then(|secs| annual_percentage_rate(net_usd?, history.put_in_usd?, secs));
        PositionView {
            id: record.id.clone(),
            account_id: record.account_id.clone(),
            store_id: self
                .doc
                .stores
                .iter()
                .find(|store| store.crypto_account_id.as_deref() == Some(&record.account_id))
                .map(|store| store.id.clone()),
            chain: record.chain,
            protocol: record.protocol.clone(),
            name: record.name.clone(),
            kind,
            status,
            assets: legs
                .iter()
                .map(|leg| self.position_asset_view(leg))
                .collect(),
            invested_usd: history.invested_usd,
            put_in_usd: history.put_in_usd,
            value_usd,
            accrued_usd: (!compares)
                .then(|| math::sub(value_usd?, hold_usd?))
                .flatten(),
            fees_collected_usd: history.fees_collected_usd,
            fees_uncollected_usd,
            il_usd: compares
                .then(|| impermanent_loss(value_usd?, hold_usd?))
                .flatten(),
            il_ratio: compares
                .then(|| impermanent_loss_ratio(value_usd?, hold_usd?))
                .flatten(),
            gas_usd: history.gas_usd,
            net_usd,
            apr,
            approximate: reading.is_some_and(|read| read.approximate),
            opened_at,
            closed_at,
            updated_at: record.updated_at.clone(),
            tracking: key.tracking(),
        }
    }

    fn position_asset_view(&self, leg: &CryptoLeg) -> PositionAssetView {
        let asset_ref = self.crypto_asset_ref(&leg.asset);
        PositionAssetView {
            asset_key: leg.asset.key(),
            symbol: asset_ref.symbol,
            direction: leg.direction,
            quantity: leg.amount,
            value_usd: self
                .price_now_usd(&leg.asset)
                .and_then(|price| math::mul(leg.amount, price)),
        }
    }

    /// What the legs are worth now: a supply adds, a debt takes away.
    /// `None` as soon as one price is unknown — a partial sum would read as a
    /// smaller position rather than as a gap.
    fn legs_value_usd(&self, legs: &[CryptoLeg]) -> Option<Amount> {
        let mut total = Amount::ZERO;
        for leg in legs {
            let price = self.price_now_usd(&leg.asset)?;
            let worth = math::mul(leg.amount, price)?;
            total = match leg.direction {
                CryptoDirection::In => math::add(total, worth)?,
                CryptoDirection::Out => math::sub(total, worth)?,
            };
        }
        Some(total)
    }

    /// USD for one unit now, by the same order of sources every other
    /// position of the app is valued at; `None`, never zero, when nothing
    /// knows it.
    fn price_now_usd(&self, asset: &CryptoAsset) -> Option<Amount> {
        // A token that is quarantined or rejected has no price here, whatever
        // its symbol says: the dollar peg would otherwise price a stranger's
        // "USDC" at 1 USD and put it into the position's value.
        if let CryptoAsset::Token(token) = asset
            && !tokens::counts_in_totals(&self.doc, token)
        {
            return None;
        }
        let quote = self.price_quote_for(&self.crypto_asset_ref(asset).key());
        if quote.info.origin != PriceOrigin::None {
            return Some(quote.usd);
        }
        // A token the ordinary refresh has not quoted yet, but the module's
        // own history (spec E4) has: its newest point stands in, which is
        // still a price somebody measured and not a zero.
        self.doc
            .crypto_prices
            .iter()
            .filter(|point| point.token_key == asset.key() && !point.value_usd.is_zero())
            .max_by(|left, right| left.ts.cmp(&right.ts))
            .map(|point| point.value_usd)
    }

    /// USD for one unit on the day of `time`, out of the module's own price
    /// history (spec E4).
    fn price_then_usd(&self, asset: &CryptoAsset, time: &str) -> Option<Amount> {
        let date = NaiveDate::parse_from_str(&ids::day(time), "%Y-%m-%d").ok()?;
        prices::lookup(&self.doc, asset, date).value
    }

    /// What the position's own operations say about it.
    fn position_history(&self, key: &PositionKey, account_id: &str) -> PositionHistory {
        let tracking = key.tracking();
        let mut out = PositionHistory::default();
        let mut invested = Some(Amount::ZERO);
        let mut put_in = Some(Amount::ZERO);
        let mut collected = Some(Amount::ZERO);
        let mut gas = Some(Amount::ZERO);
        let mut deposited: BTreeMap<String, AssetAmount> = BTreeMap::new();
        for event in self
            .doc
            .crypto_events
            .iter()
            .filter(|event| event.tracking == tracking && event.account_id == account_id)
        {
            if out.first_at.is_none() {
                out.first_at = Some(event.time.clone());
            }
            out.last_at = Some(event.time.clone());
            match event.kind {
                // Into the protocol: capital in, and the baseline the hold
                // comparison is made against.
                CryptoTxKind::Stake => {
                    for leg in legs_of(event, CryptoDirection::Out) {
                        let price = self.price_then_usd(&leg.asset, &event.time);
                        invested = add_worth(invested, price, leg);
                        put_in = add_worth(put_in, price, leg);
                        bump(&mut deposited, leg, leg.amount);
                    }
                }
                // Out of the protocol: the capital that came back lowers what
                // is still in, and the baseline with it.
                CryptoTxKind::Unstake => {
                    for leg in legs_of(event, CryptoDirection::In) {
                        invested =
                            sub_worth(invested, self.price_then_usd(&leg.asset, &event.time), leg);
                        bump(&mut deposited, leg, -leg.amount);
                    }
                }
                // What the protocol paid out and the user took: the fees that
                // have already left the position.
                CryptoTxKind::Claim => {
                    for leg in legs_of(event, CryptoDirection::In) {
                        collected =
                            add_worth(collected, self.price_then_usd(&leg.asset, &event.time), leg);
                    }
                }
                _ => {}
            }
            if let Some(fee) = &event.gas {
                gas = match (gas, self.price_then_usd(&fee.asset, &event.time)) {
                    (Some(sum), Some(price)) => {
                        math::mul(fee.amount, price).and_then(|worth| math::add(sum, worth))
                    }
                    _ => None,
                };
            }
        }
        // Nothing stored under this position means its history is unknown,
        // not empty: a zero here would read as "nothing was ever put in" and
        // turn the whole value into profit. Unknown is a dash on the screen.
        if out.first_at.is_none() {
            return out;
        }
        out.invested_usd = invested;
        out.put_in_usd = put_in;
        out.fees_collected_usd = collected;
        out.gas_usd = gas;
        out.deposited = deposited
            .into_values()
            .filter(|item| item.amount.is_positive())
            .collect();
        out
    }
}

/// What the stored operations of one position add up to.
#[derive(Debug, Default)]
struct PositionHistory {
    /// Capital still in, at the prices of the days it moved: what went in
    /// minus what came back out. This is the figure of the net identity —
    /// the change of value is measured against it.
    invested_usd: Option<Amount>,
    /// Capital ever put in, at the price of each deposit's own day, never
    /// lowered by a withdrawal.
    ///
    /// The rate is measured on this and not on [`Self::invested_usd`]: a
    /// position that was closed at a profit has *less* than nothing still in
    /// it (1 000 in, 1 020 back out leaves −20), and a rate on a negative
    /// stake would read as a loss on a position that earned. What the user
    /// risked is what they put in.
    put_in_usd: Option<Amount>,
    /// Fees already taken out, at the prices of their days.
    fees_collected_usd: Option<Amount>,
    /// What the network charged for all of it.
    gas_usd: Option<Amount>,
    /// The quantities the hold comparison is made against.
    deposited: Vec<AssetAmount>,
    /// The first and the last operation (RFC3339 UTC).
    first_at: Option<String>,
    last_at: Option<String>,
}

/// The protocol's own id of a position, out of its record key
/// (`<account_id>|<protocol>|<chain>|<id>`, or the older
/// `<protocol>|<chain>|<id>`). The id may itself contain a `|`, so only the
/// known prefix is cut off.
fn position_id_of(record: &CryptoPosition) -> &str {
    let (current, legacy) = record_id_prefixes(record);
    record
        .id
        .strip_prefix(&current)
        .or_else(|| record.id.strip_prefix(&legacy))
        .unwrap_or_else(|| {
            record
                .id
                .splitn(3, '|')
                .nth(2)
                .unwrap_or(record.id.as_str())
        })
}

/// Whether the record is keyed the way builds before the account joined the
/// key did.
fn is_legacy_keyed(record: &CryptoPosition) -> bool {
    let (current, legacy) = record_id_prefixes(record);
    !record.id.starts_with(&current) && record.id.starts_with(&legacy)
}

/// The key the record has, or would have under the current format.
fn current_id_of(record: &CryptoPosition) -> String {
    PositionKey::new(&record.protocol, record.chain, position_id_of(record))
        .record_id(&record.account_id)
}

fn legs_of(event: &CryptoTx, direction: CryptoDirection) -> impl Iterator<Item = &CryptoLeg> {
    event
        .legs
        .iter()
        .filter(move |leg| leg.direction == direction)
}

/// `total + leg.amount * price`, and `None` as soon as one price is missing:
/// a sum that skipped a leg would read as less capital, not as a gap.
fn add_worth(total: Option<Amount>, price: Option<Amount>, leg: &CryptoLeg) -> Option<Amount> {
    math::add(total?, math::mul(leg.amount, price?)?)
}

fn sub_worth(total: Option<Amount>, price: Option<Amount>, leg: &CryptoLeg) -> Option<Amount> {
    math::sub(total?, math::mul(leg.amount, price?)?)
}

/// Add `delta` to the running quantity of the leg's asset.
fn bump(into: &mut BTreeMap<String, AssetAmount>, leg: &CryptoLeg, delta: Amount) {
    let slot = into.entry(leg.asset.key()).or_insert_with(|| AssetAmount {
        asset: leg.asset.clone(),
        amount: Amount::ZERO,
    });
    slot.amount = math::add(slot.amount, delta).unwrap_or(slot.amount);
}

/// Seconds between two RFC3339 moments; `None` when either cannot be read or
/// the second is not after the first.
fn held_secs(from: Option<&str>, until: &str) -> Option<i64> {
    let start = unix_secs(from?)?;
    let end = unix_secs(until)?;
    (end > start).then_some(end - start)
}

#[cfg(test)]
#[path = "positions_tests.rs"]
mod tests;
