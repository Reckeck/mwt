//! Scenarios of the DeFi positions (spec D0): storing what the protocol adapters
//! read for one account, and the list of positions with their money. The rules
//! (what a pass stores, what it closes, how the old key migrates, what a
//! position's history adds up to) are in `cryptotrack::service::positions`;
//! the views are made here from them.

use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::cryptotrack::protocols::{
    NetParts, PositionKey, PositionReading, PositionStatus, ProtocolReview, annual_percentage_rate,
    fees_usd, impermanent_loss, impermanent_loss_ratio, is_closed, kind_of, net_result, worth_usd,
};
use crate::cryptotrack::service::positions::{self as rules, CryptoPositionsApplied};
use crate::cryptotrack::service::totals::crypto_asset_ref;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::positions::{PositionAssetView, PositionView};
use crate::model::cryptotrack::{CryptoLeg, CryptoPosition, CryptoTx};
use crate::money::books::Books;
use crate::service::UnlockedVault;

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
    vault: &mut UnlockedVault,
    account_id: &str,
    readings: &[PositionReading],
    events: &[CryptoTx],
    reviewed: &[ProtocolReview],
) -> Result<CryptoPositionsApplied> {
    let mut out = rules::apply_readings(&mut vault.doc, account_id, readings, reviewed);
    // Registration runs every pass, also when the positions themselves are
    // unchanged, so records an older build stored are fixed on the next read.
    let mut tokens_changed = false;
    for token in readings
        .iter()
        .filter(|reading| reading.account_id == account_id)
        .flat_map(|reading| &reading.tokens)
    {
        tokens_changed |= vault.trust_adapter_token(token)?;
    }
    out.events = rules::apply_events(&mut vault.doc, account_id, events);
    if out.is_unchanged() && !tokens_changed {
        return Ok(out);
    }
    rules::sort_records(&mut vault.doc);
    // The capital is read off `doc.holdings` (spec E8) and a save does not
    // re-derive it, so a pass that changed what a protocol holds has to say so.
    vault.crypto_sync_holdings()?;
    Ok(out)
}

/// Every stored position with its money, newest reading first.
///
/// `live` is what this device's own last pass read
/// (`crates/app/src/cryptotrack/jobs.rs`); an empty slice is a device
/// that has only synced, and then the figures a reading alone can give
/// are absent rather than zero.
///
/// `at` is the moment an open position's holding period is measured to
/// (RFC3339 UTC), the way `crypto_totals` takes one: the
/// caller owns the clock, so the whole fold is a function of its inputs.
/// It does not choose the prices — those are the newest the vault
/// knows.
pub fn crypto_list_positions(
    books: &Books<'_>,
    live: &[PositionReading],
    at: &str,
) -> Vec<PositionView> {
    let doc = books.doc();
    let mut out: Vec<PositionView> = doc
        .crypto_positions
        .iter()
        // A record of the old key that the current key already holds is a
        // stale twin (an older build on another device wrote it): counting
        // both would count the position twice.
        .filter(|record| !rules::is_shadowed_legacy_position(doc, record))
        .map(|record| {
            let reading = live.iter().find(|reading| reading.record_id() == record.id);
            position_view(books, record, reading, at)
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

/// One stored position, with the device's own reading of it when there is
/// one.
fn position_view(
    books: &Books<'_>,
    record: &CryptoPosition,
    reading: Option<&PositionReading>,
    now: &str,
) -> PositionView {
    let key = PositionKey::new(
        &record.protocol,
        record.chain,
        rules::position_id_of(record),
    );
    let account_id = record.account_id.as_str();
    let legs: &[CryptoLeg] = reading.map_or(record.legs.as_slice(), |read| read.legs.as_slice());
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
    let history = rules::position_history(books, &key, account_id);
    let value_usd = rules::legs_value_usd(books, legs);
    // With nothing stored under this position the base for the comparison
    // with holding is unknown, not zero: `worth_usd` of an empty list is a
    // real zero, and that zero would turn the whole balance into an
    // accrual and the whole value into impermanent loss.
    let hold_usd = history
        .first_at
        .is_some()
        .then(|| {
            worth_usd(&history.deposited, &|asset| {
                rules::price_now_usd(books, asset)
            })
        })
        .flatten();
    let compares = kind.compares_with_holding();
    let fees_uncollected_usd = match reading {
        Some(read) => fees_usd(read.fees.uncollected.as_ref(), &|asset| {
            rules::price_now_usd(books, asset)
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
    let apr = rules::held_secs(opened_at.as_deref(), until)
        .and_then(|secs| annual_percentage_rate(net_usd?, history.put_in_usd?, secs));
    PositionView {
        id: record.id.clone(),
        account_id: record.account_id.clone(),
        store_id: books
            .doc()
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
            .map(|leg| position_asset_view(books, leg))
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

fn position_asset_view(books: &Books<'_>, leg: &CryptoLeg) -> PositionAssetView {
    let asset_ref = crypto_asset_ref(books.doc(), &leg.asset);
    PositionAssetView {
        asset_key: leg.asset.key(),
        symbol: asset_ref.symbol,
        direction: leg.direction,
        quantity: leg.amount,
        value_usd: rules::price_now_usd(books, &leg.asset)
            .and_then(|price| math::mul(leg.amount, price)),
    }
}
