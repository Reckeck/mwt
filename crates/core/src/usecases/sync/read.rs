//! Scenarios of the sync read side: each stamps what changed since the last save, builds
//! the serialized records once, and asks `sync::manifest_view` (a sandbox copy for the
//! balances after a batch). The diff alone reads no records.

use std::collections::{BTreeMap, HashMap};

use crate::error::Result;
use crate::ipc_contract::security::SecurityLevel;
use crate::ipc_contract::{AfterApply, StoreBalanceChange};
use crate::service::{PeerRef, UnlockedVault};
use crate::sync::history::{StoreValues, store_changes};
use crate::sync::manifest::Manifest;
use crate::sync::manifest_view::{
    check_incoming, diff_wanted, manifest_for, preview_summary, records_for,
};
use crate::sync::{RecordState, SyncSummary, view};

/// One digest per record and deletion. `union_since_ms` leaves out cached
/// prices and rates older than that (a first sync sends 90 days).
pub fn sync_manifest(vault: &mut UnlockedVault, union_since_ms: Option<u64>) -> Result<Manifest> {
    Ok(sync_manifest_for(vault, union_since_ms, SecurityLevel::Crypto)?.0)
}

/// [`sync_manifest`] for a peer protected at `peer`: below level 3 the crypto
/// module's records are left out, so their keys never reach the network. Also
/// says how many records were left out.
pub fn sync_manifest_for(
    vault: &mut UnlockedVault,
    union_since_ms: Option<u64>,
    peer: SecurityLevel,
) -> Result<(Manifest, usize)> {
    vault.track_changes()?;
    let value = vault.doc_value()?;
    let records = view::records(&value);
    let keeps_fx = crate::money::rates::fx_history_filter(&vault.books());
    Ok(manifest_for(
        &vault.doc,
        &records,
        &keeps_fx,
        union_since_ms,
        peer,
    ))
}

/// `(to_request, to_send)` for two manifests: the plain diff without the cache
/// points this device would thin away as soon as they arrive.
pub fn sync_diff(
    vault: &UnlockedVault,
    mine: &Manifest,
    theirs: &Manifest,
) -> (Vec<String>, Vec<String>) {
    diff_wanted(
        mine,
        theirs,
        &crate::money::rates::fx_history_filter(&vault.books()),
    )
}

/// The full state of each asked record (absent keys come back empty).
pub fn sync_records(
    vault: &mut UnlockedVault,
    keys: &[String],
) -> Result<BTreeMap<String, RecordState>> {
    sync_records_for(vault, keys, SecurityLevel::Crypto)
}

/// [`sync_records`] for a peer protected at `peer`: below level 3 a crypto
/// record is not answered even when asked for (a broken or hostile peer may
/// ask for any key), so it never leaves this device.
pub fn sync_records_for(
    vault: &mut UnlockedVault,
    keys: &[String],
    peer: SecurityLevel,
) -> Result<BTreeMap<String, RecordState>> {
    vault.track_changes()?;
    let value = vault.doc_value()?;
    let records = view::records(&value);
    Ok(records_for(&vault.doc, &records, keys, peer))
}

/// What applying `incoming` would change, without changing anything.
pub fn sync_preview(
    vault: &mut UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
) -> Result<SyncSummary> {
    sync_preview_from(vault, incoming, "")
}

/// [`sync_preview`] for a batch from the device `device_id`: what that device
/// has already seen of this one is not listed as overwritten.
pub fn sync_preview_from(
    vault: &mut UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
    device_id: &str,
) -> Result<SyncSummary> {
    check_incoming(incoming)?;
    vault.sync_check_level(incoming)?;
    vault.track_changes()?;
    let value = vault.doc_value()?;
    let records = view::records(&value);
    Ok(preview_summary(&vault.doc, &records, incoming, device_id))
}

/// Capital and the balances of the stores the batch moves, as they would
/// be once applied: the batch goes into an in-memory copy of the vault
/// (a sandbox writes nothing), this vault is not touched.
pub fn sync_preview_after(
    vault: &mut UnlockedVault,
    incoming: &BTreeMap<String, RecordState>,
) -> Result<Option<AfterApply>> {
    check_incoming(incoming)?;
    vault.sync_check_level(incoming)?;
    vault.track_changes()?;
    let mut copy = vault.sandbox();
    let peer = PeerRef {
        device_id: String::new(),
        name: String::new(),
    };
    // A sandbox never persists, mirrors or announces: the copy takes the
    // batch exactly as the real apply would and is dropped.
    if copy.apply_batch(incoming, &peer, true)?.is_none() {
        return Ok(None);
    }
    let (currency, fx) = crate::usecases::money::portfolio::base_and_fx(vault);
    let (_, copy_fx) = crate::usecases::money::portfolio::base_and_fx(&copy);
    let before = crate::planning::goals::store_values_usd(&vault.books());
    let after = crate::planning::goals::store_values_usd(&copy.books());
    let names: HashMap<&str, &str> = vault
        .doc
        .stores
        .iter()
        .chain(&copy.doc.stores)
        .map(|store| (store.id.as_str(), store.name.as_str()))
        .collect();
    let stores = store_changes(
        &StoreValues { usd: &before, fx },
        &StoreValues {
            usd: &after,
            fx: copy_fx,
        },
        &names,
    )
    .into_iter()
    .map(|change| StoreBalanceChange {
        id: change.id,
        name: change.name,
        before: change.before,
        after: change.after,
        currency: currency.clone(),
    })
    .collect();
    Ok(Some(AfterApply {
        currency,
        net_worth_before: crate::usecases::money::capital::net_worth(&vault.books()).net,
        net_worth_after: crate::usecases::money::capital::net_worth(&copy.books()).net,
        stores,
    }))
}
