//! What a DeFi position is worth, and what it earned (spec D0): the rules and the
//! reads. Storing what a pass of the adapters read is the scenario in
//! `usecases::cryptotrack::positions`.
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
//!   (`crypto_list_positions` takes them as an argument),
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
//! and, through it, a holding. [`crypto_position_receipts`]
//! names those assets from the stored operations alone — so every device
//! agrees, synced or not — and `totals.rs` leaves them out of the capital and
//! says so ([`super::totals::CryptoLeftOut::InPosition`]).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use crate::Amount;
use crate::cryptotrack::engine::position_value::PositionInputs;
use crate::cryptotrack::engine::{ids, math};
use crate::cryptotrack::prices;
use crate::cryptotrack::protocols::{
    AssetAmount, PositionKey, PositionReading, ProtocolReview, is_closed, receipt_tokens,
    record_id_prefixes,
};
use crate::cryptotrack::tokens;
use crate::kernel::books::Books;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoLeg, CryptoPosition, CryptoTx, CryptoTxKind,
};
use crate::model::vault::VaultDoc;
use crate::money::price_info::price_quote_for;
use crate::time::unix_secs;

use super::tokens::counts_asset;
use super::totals::crypto_asset_ref;

/// What takes part in a position's value: the stored positions of the
/// followed, live accounts (no shadowed twins) and the receipts, owned so a
/// caller can hand them to the pure function (`engine::position_value`).
pub(crate) struct CryptoPositionInputs<'doc> {
    doc: &'doc VaultDoc,
    positions: Vec<&'doc CryptoPosition>,
    /// Asset keys the module holds only as the receipt of a position.
    pub receipts: BTreeSet<String>,
}

impl CryptoPositionInputs<'_> {
    /// Run `run` with the borrowed form the pure function takes.
    pub fn with<R>(&self, run: impl FnOnce(&PositionInputs<'_>) -> R) -> R {
        let counts = |asset: &CryptoAsset| counts_asset(self.doc, asset);
        run(&PositionInputs {
            positions: &self.positions,
            receipts: &self.receipts,
            counts: &counts,
        })
    }
}

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
fn migrate_legacy_position_records(doc: &mut VaultDoc, account_id: &str) -> usize {
    let wanted: Vec<(String, String)> = doc
        .crypto_positions
        .iter()
        .filter(|record| record.account_id == account_id && is_legacy_keyed(record))
        .map(|record| (record.id.clone(), current_id_of(record)))
        .collect();
    for (legacy, current) in &wanted {
        if doc.crypto_positions.iter().any(|have| have.id == *current) {
            doc.crypto_positions.retain(|have| have.id != *legacy);
        } else if let Some(record) = doc
            .crypto_positions
            .iter_mut()
            .find(|have| have.id == *legacy)
        {
            record.id.clone_from(current);
        }
    }
    wanted.len()
}

/// Store the positions the adapters read for one account, and close the ones a
/// protocol that *answered* no longer lists. Returns what changed; the
/// operations and the tokens of the pass are stored by [`apply_events`] and by
/// the scenario, which then re-derives the holdings and saves, because a
/// position changes what the capital counts.
///
/// `reviewed` is what each protocol that *answered* still sees
/// ([`ProtocolReview`]). A stored position of this account that such a
/// review no longer lists is closed: its legs go to zero and the row
/// stays, so its history and the periods it was open in survive, and the
/// next reading under the same id simply overwrites it. A protocol that
/// refused has no review, and nothing is closed on its word.
///
/// A reading of another account is ignored: a pass answers for exactly
/// the account it was started for.
pub fn apply_readings(
    doc: &mut VaultDoc,
    account_id: &str,
    readings: &[PositionReading],
    reviewed: &[ProtocolReview],
) -> CryptoPositionsApplied {
    let mut out = CryptoPositionsApplied {
        migrated: migrate_legacy_position_records(doc, account_id),
        ..CryptoPositionsApplied::default()
    };
    for reading in readings
        .iter()
        .filter(|reading| reading.account_id == account_id)
    {
        let record = reading.stored();
        match doc
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
                doc.crypto_positions.push(record);
                out.added += 1;
            }
        }
    }
    for review in reviewed {
        for record in doc.crypto_positions.iter_mut().filter(|record| {
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
    out
}

/// Store the operations a pass read for one account, each added or replaced by
/// its id. Returns how many were added or changed.
pub fn apply_events(doc: &mut VaultDoc, account_id: &str, events: &[CryptoTx]) -> usize {
    let mut changed = 0;
    for event in events.iter().filter(|event| event.account_id == account_id) {
        match doc
            .crypto_events
            .iter_mut()
            .find(|have| have.id == event.id)
        {
            Some(have) if *have == *event => {}
            Some(have) => {
                *have = event.clone();
                changed += 1;
            }
            None => {
                doc.crypto_events.push(event.clone());
                changed += 1;
            }
        }
    }
    changed
}

/// Put the positions and the operations back in their stored order.
pub fn sort_records(doc: &mut VaultDoc) {
    doc.crypto_positions
        .sort_by(|left, right| left.id.cmp(&right.id));
    doc.crypto_events
        .sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
}

/// Whether `record` is keyed the old way and the current key holds the
/// same position of the same account.
pub fn is_shadowed_legacy_position(doc: &VaultDoc, record: &CryptoPosition) -> bool {
    if !is_legacy_keyed(record) {
        return false;
    }
    let current = current_id_of(record);
    doc.crypto_positions.iter().any(|have| have.id == current)
}

/// Whether the capital counts what this account holds: it is followed
/// and not archived. An address watched without being counted stays out.
pub fn crypto_account_counts(doc: &VaultDoc, account_id: &str) -> bool {
    doc.crypto_accounts
        .iter()
        .find(|account| account.id == account_id)
        .is_some_and(|account| !account.archived)
}

/// What takes part in a position's value, built once for the present
/// capital and the snapshots: positions of followed, live accounts, no
/// stale twin of an old key, plus the receipts. Token trust is
/// `tokens::counts_in_totals`.
pub(crate) fn crypto_position_inputs(doc: &VaultDoc) -> CryptoPositionInputs<'_> {
    CryptoPositionInputs {
        doc,
        positions: doc
            .crypto_positions
            .iter()
            .filter(|record| crypto_account_counts(doc, &record.account_id))
            .filter(|record| !is_shadowed_legacy_position(doc, record))
            .collect(),
        receipts: crypto_position_receipts(doc),
    }
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
pub fn crypto_position_receipts(doc: &VaultDoc) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for event in &doc.crypto_events {
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
    for record in &doc.crypto_positions {
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

/// What the legs are worth now: a supply adds, a debt takes away.
/// `None` as soon as one price is unknown — a partial sum would read as a
/// smaller position rather than as a gap.
pub(crate) fn legs_value_usd(books: &Books<'_>, legs: &[CryptoLeg]) -> Option<Amount> {
    let mut total = Amount::ZERO;
    for leg in legs {
        let price = price_now_usd(books, &leg.asset)?;
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
pub(crate) fn price_now_usd(books: &Books<'_>, asset: &CryptoAsset) -> Option<Amount> {
    let doc = books.doc();
    // A token that is quarantined or rejected has no price here, whatever
    // its symbol says: the dollar peg would otherwise price a stranger's
    // "USDC" at 1 USD and put it into the position's value.
    if let CryptoAsset::Token(token) = asset
        && !tokens::counts_in_totals(doc, token)
    {
        return None;
    }
    let quote = price_quote_for(books, &crypto_asset_ref(doc, asset).key());
    // The default origin is `PriceOrigin::None`: no price at all.
    if quote.info.origin != Default::default() {
        return Some(quote.usd);
    }
    // A token the ordinary refresh has not quoted yet, but the module's
    // own history (spec E4) has: its newest point stands in, which is
    // still a price somebody measured and not a zero.
    doc.crypto_prices
        .iter()
        .filter(|point| point.token_key == asset.key() && !point.value_usd.is_zero())
        .max_by(|left, right| left.ts.cmp(&right.ts))
        .map(|point| point.value_usd)
}

/// USD for one unit on the day of `time`, out of the module's own price
/// history (spec E4).
fn price_then_usd(doc: &VaultDoc, asset: &CryptoAsset, time: &str) -> Option<Amount> {
    let date = NaiveDate::parse_from_str(&ids::day(time), "%Y-%m-%d").ok()?;
    prices::lookup(doc, asset, date).value
}

/// What the position's own operations say about it.
pub(crate) fn position_history(
    books: &Books<'_>,
    key: &PositionKey,
    account_id: &str,
) -> PositionHistory {
    let doc = books.doc();
    let tracking = key.tracking();
    let mut out = PositionHistory::default();
    let mut invested = Some(Amount::ZERO);
    let mut put_in = Some(Amount::ZERO);
    let mut collected = Some(Amount::ZERO);
    let mut gas = Some(Amount::ZERO);
    let mut deposited: BTreeMap<String, AssetAmount> = BTreeMap::new();
    for event in doc
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
                    let price = price_then_usd(doc, &leg.asset, &event.time);
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
                        sub_worth(invested, price_then_usd(doc, &leg.asset, &event.time), leg);
                    bump(&mut deposited, leg, -leg.amount);
                }
            }
            // What the protocol paid out and the user took: the fees that
            // have already left the position.
            CryptoTxKind::Claim => {
                for leg in legs_of(event, CryptoDirection::In) {
                    collected =
                        add_worth(collected, price_then_usd(doc, &leg.asset, &event.time), leg);
                }
            }
            _ => {}
        }
        if let Some(fee) = &event.gas {
            gas = match (gas, price_then_usd(doc, &fee.asset, &event.time)) {
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

/// What the stored operations of one position add up to.
#[derive(Debug, Default)]
pub(crate) struct PositionHistory {
    /// Capital still in, at the prices of the days it moved: what went in
    /// minus what came back out. This is the figure of the net identity —
    /// the change of value is measured against it.
    pub(crate) invested_usd: Option<Amount>,
    /// Capital ever put in, at the price of each deposit's own day, never
    /// lowered by a withdrawal.
    ///
    /// The rate is measured on this and not on [`Self::invested_usd`]: a
    /// position that was closed at a profit has *less* than nothing still in
    /// it (1 000 in, 1 020 back out leaves −20), and a rate on a negative
    /// stake would read as a loss on a position that earned. What the user
    /// risked is what they put in.
    pub(crate) put_in_usd: Option<Amount>,
    /// Fees already taken out, at the prices of their days.
    pub(crate) fees_collected_usd: Option<Amount>,
    /// What the network charged for all of it.
    pub(crate) gas_usd: Option<Amount>,
    /// The quantities the hold comparison is made against.
    pub(crate) deposited: Vec<AssetAmount>,
    /// The first and the last operation (RFC3339 UTC).
    pub(crate) first_at: Option<String>,
    pub(crate) last_at: Option<String>,
}

/// The protocol's own id of a position, out of its record key
/// (`<account_id>|<protocol>|<chain>|<id>`, or the older
/// `<protocol>|<chain>|<id>`). The id may itself contain a `|`, so only the
/// known prefix is cut off.
pub(crate) fn position_id_of(record: &CryptoPosition) -> &str {
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
pub(crate) fn held_secs(from: Option<&str>, until: &str) -> Option<i64> {
    let start = unix_secs(from?)?;
    let end = unix_secs(until)?;
    (end > start).then_some(end - start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::CryptoAccount;

    fn account(id: &str, archived: bool) -> CryptoAccount {
        let mut account = CryptoAccount::new(
            id,
            "Cold",
            Chain::Ethereum,
            "0xabcd00000000000000000000000000000000ef01",
            "2026-02-01T00:00:00Z",
        );
        account.archived = archived;
        account
    }

    #[test]
    fn only_a_followed_live_account_counts() {
        let doc = VaultDoc {
            crypto_accounts: vec![account("live", false), account("old", true)],
            ..VaultDoc::default()
        };
        assert!(crypto_account_counts(&doc, "live"));
        assert!(!crypto_account_counts(&doc, "old"));
        assert!(!crypto_account_counts(&doc, "nobody"));
    }

    #[test]
    fn a_pass_with_no_readings_and_no_events_changes_nothing() {
        let mut doc = VaultDoc::default();
        let applied = apply_readings(&mut doc, "acc", &[], &[]);
        assert!(applied.is_unchanged());
        assert_eq!(apply_events(&mut doc, "acc", &[]), 0);
        assert!(doc.crypto_positions.is_empty() && doc.crypto_events.is_empty());
    }
}
