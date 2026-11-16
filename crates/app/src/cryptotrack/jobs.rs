//! The crypto module's background tick (spec E6).
//!
//! The scheduler calls [`cryptotrack_tick`] on every pass of an unlocked
//! vault on the device that runs the jobs (`crate::scheduler::spawn`).
//!
//! What a pass respects:
//!
//! - **Consent first.** Until the user said yes ([`crypto_tracking_allowed`])
//!   there is no network call at all, not even a probe. The first thing a pass
//!   does is look at the consent, and the reader (the only thing that talks to
//!   a provider) is made after it.
//! - **No lock across the network** (`AGENTS.md`, rule 3). The vault is
//!   reached only through [`Host::with_vault`], a closure that runs and lets go
//!   of the guard before it returns, so a guard cannot be held across an
//!   `.await`; the compiler checks it, because the cycle's future must be
//!   `Send` and a `MutexGuard` is not (`the_cycle_is_send`).
//!   A pass takes what it needs, goes to the network with nothing held, then
//!   takes the vault again to store.
//! - **Batches.** Events are written a few hundred at a time, together with
//!   the cursor, because every write encrypts the whole document.
//! - **One source does not drop the others.** An account that fails is
//!   marked and retried later; the rest of the pass goes on.
//! - **Locking ends the pass.** The vault is looked at before every page and
//!   on every write, and it must still be the one the pass started with: a
//!   vault locked (or swapped) mid-poll is never written to.
//!
//! The pass is an `async` function over two small traits ([`Host`] for the
//! vault, the consent and the event, [`Reader`] for the provider), so a test
//! drives it with a stub and no network.
//!
//! - **The user's own endpoints.** Before every request the reader asks the
//!   vault what was saved for that network (`crypto_network_for_request`): their
//!   node, their Blockscout instance and its optional key (decision R8, spec
//!   E17). Nothing assumes a key exists, and the node never sees the key.
//!
//! [`crypto_tracking_allowed`]: trade_core::appconfig::AppSettings::crypto_tracking_allowed

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use tauri::{AppHandle, Emitter, Manager};
use trade_core::crypto::device_secrets::BlockscoutKey;
use trade_core::cryptotrack::prices;
use trade_core::cryptotrack::protocols::{
    PositionReading, ProtocolRequest, ProtocolReview, Registry,
};
use trade_core::cryptotrack::rpc::{RpcClient, RpcConfig};
use trade_core::cryptotrack::sources::evm::{
    BalanceRequest, Cursor, EvmSource, HistoryRequest, RawAsset, SourceBatch, Stream,
};
use trade_core::error::{Error, Result};
use trade_core::ipc_contract::cryptotrack::sync::{
    CryptoPages, CryptoSyncAccountStatus, CryptoSyncState, CryptoSyncStatus, CryptoSyncTarget,
};
use trade_core::model::chain::Chain;
use trade_core::model::cryptotrack::{CryptoAsset, CryptoPricePoint, CryptoTx};
use trade_core::model::vault::now_iso;
use trade_core::pricing::PricingService;
use trade_core::service::NetworkRequest;
use trade_core::service::UnlockedVault;
use trade_core::sync::stamp::now_ms;

use crate::commands::{AppState, lock_vault};

/// A finished account is read again after this long.
const REFRESH_EVERY: Duration = Duration::from_secs(15 * 60);
/// A failed account waits this long before the next try.
const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
/// Events collected before they are written.
const BATCH_EVENTS: usize = 400;
/// Pages one account may read in one pass; the cursor resumes the rest.
const MAX_PAGES_PER_PASS: usize = 100;
/// Days of historic prices one account asks for in one pass: one request per
/// day, all the contracts of that day together. The rest waits for the next
/// pass, oldest first, so an empty cache is filled in steps and not at once.
const MAX_PRICE_DAYS_PER_PASS: usize = 20;
/// A price the provider did not have is not asked for again this long: it
/// would answer the same on every pass.
const PRICE_MISS_PAUSE: Duration = Duration::from_secs(24 * 60 * 60);

// -- the two edges of a pass ----------------------------------------------------

/// What a pass needs from the application.
pub trait Host: Send + Sync {
    /// The user said yes to reading the networks.
    fn consent(&self) -> bool;

    /// Run `work` on the open vault and let go of it. `RequiresUnlock` when
    /// the vault is locked. The guard never leaves this call.
    fn with_vault<T>(&self, work: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T>;

    /// Something new was stored for the account (`crypto-updated`).
    fn updated(&self, account_id: &str);
}

/// What a pass needs from the provider. Touches no document.
pub trait Reader: Send + Sync {
    /// The next page of an account's history from `cursor`.
    fn page(
        &self,
        target: &CryptoSyncTarget,
        cursor: Option<&str>,
    ) -> impl Future<Output = Result<SourceBatch>> + Send;

    /// The node's balances of the native coin and of `tokens`.
    fn balances(
        &self,
        target: &CryptoSyncTarget,
        tokens: &[RawAsset],
    ) -> impl Future<Output = Result<SourceBatch>> + Send;

    /// What the protocols hold for this account: the readings of the
    /// positions and the operations behind them (spec D0). No cursor — an
    /// adapter answers for the position as it is now, and the operations it
    /// returns are deduplicated by their id when they are stored.
    fn positions(
        &self,
        target: &CryptoSyncTarget,
    ) -> impl Future<Output = Result<ProtocolsRead>> + Send;

    /// The USD prices of `wanted` assets on one day (spec D7, wave 0): one
    /// request, no address and no key in it. A price the provider does not
    /// have is missing from the answer, never zero.
    fn historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> impl Future<Output = Result<Vec<CryptoPricePoint>>> + Send;
}

/// What one pass over the protocols of a network read.
#[derive(Debug, Default)]
pub struct ProtocolsRead {
    pub readings: Vec<PositionReading>,
    pub events: Vec<CryptoTx>,
    /// What each protocol that *answered* still sees for the account, so a
    /// position that is no longer in it can be closed in the vault. A
    /// protocol that refused has no entry: its silence closes nothing.
    pub reviewed: Vec<ProtocolReview>,
    /// The protocols whose read failed, each as `<protocol> (<CODE>)`: one
    /// endpoint that refuses a method, one rate limit or one position too old
    /// for a log scan must not turn the other protocols' positions into "this
    /// account holds nothing". Only the error's own code travels here —
    /// never the message, which can carry the endpoint URL and with it a key.
    pub missed: Vec<String>,
    /// What each protocol that answered wants kept for its next pass, by
    /// protocol id; `None` forgets what was kept.
    pub memory: Vec<(String, Option<String>)>,
}

// -- status ---------------------------------------------------------------------

struct AccountRun {
    status: CryptoSyncAccountStatus,
    /// No new try before this (after a failure).
    retry_at: Option<Instant>,
}

/// What this device remembers of the pass: the state of each account and
/// whether a pass is running. It is not stored: after a restart every account
/// is judged again by its cursor (`finished_at_ms`), which is how a pass
/// catches up after an unlock or after the app was closed for a day.
pub struct Tracker {
    running: AtomicBool,
    force: AtomicBool,
    accounts: Mutex<BTreeMap<String, AccountRun>>,
    /// The last readings of the protocol adapters, by account (spec D0).
    ///
    /// Not stored, and deliberately: a position's uncollected fees and its
    /// `approximate` flag are a reading of a moment, exactly like the
    /// balances a node reports, so they live here beside the pass instead of
    /// in the vault — the schema does not change for DeFi. A device that has
    /// only synced holds none, and
    /// [`UnlockedVault::crypto_list_positions`] then leaves those figures
    /// absent rather than zero.
    positions: Mutex<BTreeMap<String, Vec<PositionReading>>>,
    /// `(asset key, day)` the provider had no price for, and when it said so.
    /// Device-local and not stored: after a restart one request per day finds
    /// out again.
    price_misses: Mutex<BTreeMap<(String, NaiveDate), Instant>>,
}

impl Tracker {
    pub const fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            force: AtomicBool::new(false),
            accounts: Mutex::new(BTreeMap::new()),
            positions: Mutex::new(BTreeMap::new()),
            price_misses: Mutex::new(BTreeMap::new()),
        }
    }

    fn begin(&'static self) -> Option<RunGuard> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| RunGuard(self))
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Read everything on the next pass, ignoring the refresh interval and
    /// the wait after a failure.
    pub fn force_next(&self) {
        self.force.store(true, Ordering::SeqCst);
    }

    fn take_force(&self) -> bool {
        self.force.swap(false, Ordering::SeqCst)
    }

    fn with_accounts<T>(&self, work: impl FnOnce(&mut BTreeMap<String, AccountRun>) -> T) -> T {
        // A poisoned lock only means a panic elsewhere: the map is still usable.
        let mut guard = self
            .accounts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        work(&mut guard)
    }

    fn set(&self, account_id: &str, state: CryptoSyncState, error: Option<String>, added: u32) {
        self.with_accounts(|accounts| {
            let run = accounts
                .entry(account_id.to_string())
                .or_insert_with(|| AccountRun {
                    status: CryptoSyncAccountStatus {
                        account_id: account_id.to_string(),
                        state: CryptoSyncState::Waiting,
                        last_ok_at: None,
                        error: None,
                        added: 0,
                    },
                    retry_at: None,
                });
            run.status.state = state;
            run.status.error = error;
            run.status.added = added;
            match state {
                CryptoSyncState::Done => {
                    run.status.last_ok_at = Some(now_iso());
                    run.retry_at = None;
                }
                CryptoSyncState::Failed => run.retry_at = Some(Instant::now() + RETRY_AFTER),
                CryptoSyncState::Waiting | CryptoSyncState::Reading => {}
            }
        });
    }

    /// Keep what the adapters just read for one account, replacing the
    /// previous reading of it: a position that is gone from the answer is
    /// gone from the device's picture too.
    fn remember_positions(&self, account_id: &str, readings: Vec<PositionReading>) {
        let mut guard = self
            .positions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if readings.is_empty() {
            guard.remove(account_id);
        } else {
            guard.insert(account_id.to_string(), readings);
        }
    }

    /// Every reading this device has, for the screen that asks for the
    /// positions.
    pub fn positions(&self) -> Vec<PositionReading> {
        let guard = self
            .positions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.values().flatten().cloned().collect()
    }

    /// Forget every reading: they carry the amounts of the legs, so they must
    /// not outlive the vault they were read for (lock, swap, wipe).
    pub fn clear_positions(&self) {
        self.positions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// What is still worth asking for out of `missing` (oldest day first):
    /// without what the provider recently said it does not have, and cut to
    /// [`MAX_PRICE_DAYS_PER_PASS`] days.
    fn prices_to_ask(
        &self,
        missing: Vec<(CryptoAsset, NaiveDate)>,
    ) -> Vec<(CryptoAsset, NaiveDate)> {
        let mut guard = self
            .price_misses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.retain(|_, said_at| said_at.elapsed() < PRICE_MISS_PAUSE);
        let mut days = Vec::new();
        let mut out = Vec::new();
        for (asset, date) in missing {
            if guard.contains_key(&(asset.key(), date)) {
                continue;
            }
            if !days.contains(&date) {
                if days.len() >= MAX_PRICE_DAYS_PER_PASS {
                    break;
                }
                days.push(date);
            }
            out.push((asset, date));
        }
        out
    }

    /// Remember which of `asked` the provider answered nothing for.
    fn note_price_misses(&self, asked: &[(CryptoAsset, NaiveDate)], answered: &[CryptoPricePoint]) {
        let mut guard = self
            .price_misses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (asset, date) in asked {
            let key = asset.key();
            let stamp = prices::date_ts(*date);
            let found = answered
                .iter()
                .any(|point| point.token_key == key && point.ts == stamp);
            if !found {
                guard.insert((key, *date), Instant::now());
            }
        }
    }

    fn waiting_for_retry(&self, account_id: &str) -> bool {
        self.with_accounts(|accounts| {
            accounts
                .get(account_id)
                .and_then(|run| run.retry_at)
                .is_some_and(|at| Instant::now() < at)
        })
    }

    /// The state of the given accounts, `Waiting` for one not read yet.
    pub fn statuses(&self, ids: &[String]) -> Vec<CryptoSyncAccountStatus> {
        self.with_accounts(|accounts| {
            ids.iter()
                .map(|id| {
                    accounts.get(id).map_or_else(
                        || CryptoSyncAccountStatus {
                            account_id: id.clone(),
                            state: CryptoSyncState::Waiting,
                            last_ok_at: None,
                            error: None,
                            added: 0,
                        },
                        |run| run.status.clone(),
                    )
                })
                .collect()
        })
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Frees the "a pass is running" flag when the pass ends, even by a panic.
struct RunGuard(&'static Tracker);

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

static TRACKER: Tracker = Tracker::new();

// -- the pass -------------------------------------------------------------------

/// What a pass did, for the tests and the log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassReport {
    /// Accounts read to the end of what this pass reads.
    pub read: usize,
    /// Accounts that failed.
    pub failed: usize,
    /// The pass stopped because the vault was locked or swapped, or the
    /// consent was withdrawn.
    pub cancelled: bool,
}

/// Why an account's read ended early.
enum Stop {
    /// Locked, swapped or withdrawn: the whole pass ends.
    Cancelled,
    /// This account failed; the others go on.
    Failed(Error),
    /// The account was forgotten while the pass ran: nothing to store, the
    /// others go on.
    Gone,
}

impl From<Error> for Stop {
    fn from(err: Error) -> Self {
        match err {
            Error::RequiresUnlock => Stop::Cancelled,
            Error::NotFound(_) => Stop::Gone,
            other => Stop::Failed(other),
        }
    }
}

/// One pass over the followed accounts. Without consent it returns before
/// anything else happens.
pub async fn run_pass<H: Host, R: Reader>(
    host: &H,
    reader: &R,
    tracker: &Tracker,
    force: bool,
) -> PassReport {
    let mut report = PassReport::default();
    if !host.consent() {
        return report;
    }
    let force = tracker.take_force() || force;
    // The data the pass needs, copied; the vault is free from here on.
    let Ok((vault_id, targets)) =
        host.with_vault(|vault| Ok((vault.doc.id.clone(), vault.crypto_sync_targets())))
    else {
        report.cancelled = true;
        return report;
    };
    for target in targets {
        if !is_due(&target, tracker, force) {
            continue;
        }
        tracker.set(&target.account_id, CryptoSyncState::Reading, None, 0);
        match read_account(host, reader, tracker, &vault_id, &target).await {
            Ok(()) => report.read += 1,
            Err(Stop::Cancelled) => {
                tracker.set(&target.account_id, CryptoSyncState::Waiting, None, 0);
                report.cancelled = true;
                return report;
            }
            Err(Stop::Gone) => {}
            Err(Stop::Failed(err)) => {
                report.failed += 1;
                tracker.set(
                    &target.account_id,
                    CryptoSyncState::Failed,
                    Some(err.code().to_string()),
                    0,
                );
            }
        }
    }
    report
}

/// Whether the account is read in this pass: an unfinished read resumes at
/// once, a finished one is refreshed after [`REFRESH_EVERY`], and a failed
/// one waits out its pause. `force` ignores all of it.
fn is_due(target: &CryptoSyncTarget, tracker: &Tracker, force: bool) -> bool {
    if force {
        return true;
    }
    if tracker.waiting_for_retry(&target.account_id) {
        return false;
    }
    let Some(finished) = target.finished_at_ms else {
        return true;
    };
    let waited = i64::try_from(now_ms()).unwrap_or(i64::MAX) - finished;
    waited >= i64::try_from(REFRESH_EVERY.as_millis()).unwrap_or(i64::MAX)
}

fn stream_of(cursor: Option<&str>) -> Option<Stream> {
    Cursor::decode(cursor).ok().map(|cursor| cursor.stream)
}

/// Whether two readings are of the same asset.
fn same_asset(left: &RawAsset, right: &RawAsset) -> bool {
    match (left, right) {
        (RawAsset::Native { .. }, RawAsset::Native { .. }) => true,
        (RawAsset::Token { contract: one, .. }, RawAsset::Token { contract: two, .. }) => {
            one.eq_ignore_ascii_case(two)
        }
        _ => false,
    }
}

/// The vault is still the one this pass started with.
fn still_here<H: Host>(host: &H, vault_id: &str) -> Result<()> {
    if host.with_vault(|vault| Ok(vault.doc.id == vault_id))? {
        Ok(())
    } else {
        Err(Error::RequiresUnlock)
    }
}

async fn read_account<H: Host, R: Reader>(
    host: &H,
    reader: &R,
    tracker: &Tracker,
    vault_id: &str,
    target: &CryptoSyncTarget,
) -> std::result::Result<(), Stop> {
    let mut cursor = target.cursor.clone();
    let mut pending = SourceBatch::default();
    let mut added = 0u32;
    for _ in 0..MAX_PAGES_PER_PASS {
        // Locked, swapped or withdrawn since the last page: stop before the
        // next request, not after it.
        if !host.consent() {
            return Err(Stop::Cancelled);
        }
        still_here(host, vault_id)?;

        let page = reader.page(target, cursor.as_deref()).await?;
        let next = page.next_cursor.clone();
        pending.raw_events.extend(page.raw_events);
        pending.balances.extend(page.balances);
        pending.rejected.extend(page.rejected);
        pending.next_cursor = next.clone();

        let finished = next.is_none();
        // A write ends at a list's end: whether the next list is read at all
        // depends on what this one brought.
        let list_ended = stream_of(next.as_deref()) != stream_of(cursor.as_deref());
        if !(finished || list_ended || pending.raw_events.len() >= BATCH_EVENTS) {
            cursor = next;
            continue;
        }
        if finished {
            let mut tokens = target.tokens.clone();
            for asset in pending
                .raw_events
                .iter()
                .map(|event| &event.asset)
                .chain(pending.balances.iter().map(|balance| &balance.asset))
            {
                if matches!(asset, RawAsset::Token { .. })
                    && !tokens.iter().any(|known| same_asset(known, asset))
                {
                    tokens.push(asset.clone());
                }
            }
            // The node is the authority on what is held; the explorer's list
            // only adds what the node was not asked about.
            if !host.consent() {
                return Err(Stop::Cancelled);
            }
            still_here(host, vault_id)?;
            let node = reader.balances(target, &tokens).await?;
            let mut merged = node.balances;
            for held in std::mem::take(&mut pending.balances) {
                if !merged
                    .iter()
                    .any(|have| same_asset(&have.asset, &held.asset))
                {
                    merged.push(held);
                }
            }
            pending.balances = merged;
        }

        let read_at = now_iso();
        let batch = std::mem::take(&mut pending);
        let applied = host.with_vault(|vault| {
            // The vault may have been locked and another opened while the
            // network answered: it is written only if it is the same one.
            if vault.doc.id != vault_id {
                return Err(Error::RequiresUnlock);
            }
            vault.crypto_apply_pages(
                &CryptoPages {
                    account_id: &target.account_id,
                    read_at: &read_at,
                    finished,
                    refresh: target.refresh,
                },
                &batch,
            )
        })?;
        added = added.saturating_add(u32::try_from(applied.added).unwrap_or(u32::MAX));
        tracker.set(&target.account_id, CryptoSyncState::Reading, None, added);
        host.updated(&target.account_id);

        cursor = applied.next_cursor;
        if finished || cursor.is_none() {
            // The history is in, so the position's own operations have
            // something to sit next to: what a protocol holds is read last.
            let mut missed = read_positions(host, reader, tracker, vault_id, target).await?;
            // The operations are in: the price of each one's day is what
            // every figure of the module stands on.
            missed.extend(fill_prices(host, reader, tracker, vault_id, target).await?);
            let note = (!missed.is_empty()).then(|| format!("PARTIAL: {}", missed.join(", ")));
            tracker.set(&target.account_id, CryptoSyncState::Done, note, added);
            return Ok(());
        }
    }
    // Out of pages for this pass: the cursor is saved, the next pass goes on.
    tracker.set(&target.account_id, CryptoSyncState::Waiting, None, added);
    Ok(())
}

/// What the protocols hold for one account, after its history is read
/// (spec D0).
///
/// The stored half — the legs, the name and the operations — goes into the
/// vault; the reading itself stays on this device, because nothing derived
/// and nothing momentary is stored (see [`Tracker::positions`]).
async fn read_positions<H: Host, R: Reader>(
    host: &H,
    reader: &R,
    tracker: &Tracker,
    vault_id: &str,
    target: &CryptoSyncTarget,
) -> std::result::Result<Vec<String>, Stop> {
    if !host.consent() {
        return Err(Stop::Cancelled);
    }
    still_here(host, vault_id)?;
    let read = reader.positions(target).await?;
    tracker.remember_positions(&target.account_id, read.readings.clone());
    // Nothing read, no operations and no protocol that answered: there is
    // nothing to store and nothing to close. A protocol that answered with an
    // empty list is not nothing — that is how a full withdrawal looks.
    if read.readings.is_empty()
        && read.events.is_empty()
        && read.reviewed.is_empty()
        && read.memory.is_empty()
    {
        return Ok(read.missed);
    }
    let applied = host.with_vault(|vault| {
        if vault.doc.id != vault_id {
            return Err(Error::RequiresUnlock);
        }
        let applied = vault.crypto_apply_positions(
            &target.account_id,
            &read.readings,
            &read.events,
            &read.reviewed,
        )?;
        vault.crypto_remember_protocols(&target.account_id, &read.memory)?;
        Ok(applied)
    })?;
    if !applied.is_unchanged() {
        host.updated(&target.account_id);
    }
    Ok(read.missed)
}

/// The prices of the days of an account's operations that the cache lacks
/// (spec D7, wave 0), after its history is read.
///
/// Copy, release, network, apply (`AGENTS.md`, rules 3 and 4): the missing
/// `(asset, day)` pairs are listed under the vault, the vault is let go, each
/// day is one request (consent and vault checked before it), and what came
/// back is stored in one write. A failed day does not drop the others and does
/// not touch what is stored: the pass is partial, and the answer names the
/// error's code only, never its text.
async fn fill_prices<H: Host, R: Reader>(
    host: &H,
    reader: &R,
    tracker: &Tracker,
    vault_id: &str,
    target: &CryptoSyncTarget,
) -> std::result::Result<Option<String>, Stop> {
    if !host.consent() {
        return Err(Stop::Cancelled);
    }
    let missing = host.with_vault(|vault| {
        if vault.doc.id != vault_id {
            return Err(Error::RequiresUnlock);
        }
        Ok(prices::missing_for_account(&vault.doc, &target.account_id))
    })?;
    let mut by_day: BTreeMap<NaiveDate, Vec<(CryptoAsset, NaiveDate)>> = BTreeMap::new();
    for (asset, date) in tracker.prices_to_ask(missing) {
        by_day.entry(date).or_default().push((asset, date));
    }
    let mut fetched = Vec::new();
    let mut failure = None;
    for group in by_day.values() {
        if !host.consent() {
            return Err(Stop::Cancelled);
        }
        still_here(host, vault_id)?;
        match reader.historic_prices(group).await {
            Ok(points) => {
                tracker.note_price_misses(group, &points);
                fetched.extend(points);
            }
            Err(err) => failure = Some(err.code().to_string()),
        }
    }
    if !fetched.is_empty() {
        let stored = host.with_vault(|vault| {
            if vault.doc.id != vault_id {
                return Err(Error::RequiresUnlock);
            }
            vault.crypto_apply_prices(&fetched)
        })?;
        if stored > 0 {
            host.updated(&target.account_id);
        }
    }
    Ok(failure.map(|code| format!("prices ({code})")))
}

// -- the application's side -----------------------------------------------------

struct AppHost(AppHandle);

impl Host for AppHost {
    fn consent(&self) -> bool {
        crate::commands::app_settings::current_settings(&self.0).crypto_tracking_allowed()
    }

    fn with_vault<T>(&self, work: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T> {
        let state = self.0.state::<AppState>();
        let mut guard = lock_vault(&state)?;
        match guard.as_mut() {
            Some(vault) => work(vault),
            None => Err(Error::RequiresUnlock),
        }
    }

    fn updated(&self, account_id: &str) {
        let _ = self.0.emit(
            "crypto-updated",
            serde_json::json!({ "account_id": account_id }),
        );
    }
}

/// The providers of the EVM networks. The Blockscout key of spec E17, when
/// there is one, joins the settings of this reader; there is none yet.
struct EvmReader {
    app: AppHandle,
    /// Talks to the explorer: the history and the token list. The per-network
    /// key is added to a copy of it for one request, so a network without a key
    /// never sends another network's.
    explorer: RpcClient,
    /// Talks to the node: balances. It never carries the Blockscout key, and it
    /// accepts a loopback address, because the user's own node is often one.
    node: RpcClient,
    /// Historic prices: DeFiLlama, no key.
    pricing: PricingService,
}

impl EvmReader {
    fn new(app: AppHandle) -> Result<Self> {
        Ok(Self {
            app,
            explorer: RpcClient::new(RpcConfig::default())?,
            node: RpcClient::new(RpcConfig {
                allow_loopback_http: true,
                ..RpcConfig::default()
            })?,
            pricing: PricingService::default(),
        })
    }

    /// What the user saved for this network on this device: their node, their
    /// Blockscout instance and its optional key (spec E17). The guard is taken
    /// and let go inside this call, before any `.await` (`AGENTS.md`, rule 3);
    /// a locked vault simply means the presets, and the pass ends on the next
    /// look at the vault anyway.
    fn network(&self, chain: Chain) -> NetworkRequest {
        AppHost(self.app.clone())
            .with_vault(|vault| Ok(vault.crypto_network_for_request(chain)))
            .unwrap_or_default()
    }
}

impl Reader for EvmReader {
    async fn page(&self, target: &CryptoSyncTarget, cursor: Option<&str>) -> Result<SourceBatch> {
        let settings = self.network(target.chain);
        // A client carries one key, so the keyed copy is made per request; the
        // clone shares the host limiter, so the pacing of the pass is kept.
        let keyed = self
            .explorer
            .with_api_key(settings.key.as_ref().map(BlockscoutKey::expose));
        EvmSource::new(keyed, self.node.clone())
            .fetch_page(&HistoryRequest {
                chain: target.chain,
                address: &target.address,
                cursor,
                since: Some(&target.start),
                explorer: settings.blockscout.as_deref(),
            })
            .await
    }

    async fn balances(
        &self,
        target: &CryptoSyncTarget,
        tokens: &[RawAsset],
    ) -> Result<SourceBatch> {
        let settings = self.network(target.chain);
        EvmSource::new(self.explorer.clone(), self.node.clone())
            .read_balances(&BalanceRequest {
                chain: target.chain,
                address: &target.address,
                tokens,
                rpc: settings.rpc.as_deref(),
            })
            .await
    }

    /// Every protocol of this network, asked in turn: discover, then read,
    /// then the operations. The registry ships empty until wave 1 adds the
    /// first adapter (spec D1), so today this reads nothing and makes no
    /// request — the path is here and the four specs only fill
    /// [`Registry::builtin`].
    async fn positions(&self, target: &CryptoSyncTarget) -> Result<ProtocolsRead> {
        let settings = self.network(target.chain);
        let registry = Registry::builtin(self.node.clone());
        let request = ProtocolRequest {
            chain: target.chain,
            address: &target.address,
            account_id: &target.account_id,
            since: Some(&target.start),
            rpc: settings.rpc.as_deref(),
            memory: None,
        };
        read_protocols(&registry, target.chain, request, &target.protocol_memory).await
    }

    async fn historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> Result<Vec<CryptoPricePoint>> {
        self.pricing.fetch_historic_prices(wanted).await
    }
}

/// Every protocol of one network, each isolated from the next (spec D0, with
/// the reliability note of wave 1).
///
/// One endpoint that refuses a method, one rate limit, one position older
/// than a log scan can reach: any of them is one protocol's problem, and the
/// others' positions are read anyway. The protocols that failed are named in
/// [`ProtocolsRead::missed`] so the account's status can say the pass was
/// partial — a silent empty answer would read as "this address holds nothing
/// in any protocol", which is a different and much worse claim.
///
/// When *every* protocol refused and nothing at all was read, that is a
/// failed pass and the error travels up.
async fn read_protocols(
    registry: &Registry,
    chain: Chain,
    request: ProtocolRequest<'_>,
    memory: &std::collections::BTreeMap<String, String>,
) -> Result<ProtocolsRead> {
    let mut out = ProtocolsRead::default();
    let mut asked = 0_usize;
    let mut refusals = 0_usize;
    let mut last_refusal = None;
    for adapter in registry.for_chain(chain) {
        asked += 1;
        let request = ProtocolRequest {
            memory: memory.get(adapter.protocol()).map(String::as_str),
            ..request
        };
        match read_one_protocol(adapter, request).await {
            Ok(part) => {
                out.memory.extend(part.memory);
                out.readings.extend(part.readings);
                out.events.extend(part.events);
                out.reviewed.extend(part.reviewed);
                // A position whose own read refused is named here too: the
                // pass is partial, whatever the protocol's discovery said.
                out.missed.extend(part.missed);
            }
            Err(error) => {
                refusals += 1;
                out.missed
                    .push(format!("{} ({})", adapter.protocol(), error.code()));
                last_refusal = Some(error);
            }
        }
    }
    match last_refusal {
        // Only when every protocol asked refused was the account not read at
        // all. One refusal beside an adapter that answered "nothing here" is a
        // partial pass: it keeps its `missed` note and its time, instead of
        // failing the account and rolling the whole address back.
        Some(error) if refusals == asked => Err(error),
        _ => Ok(out),
    }
}

/// One protocol's whole answer for one account: discovery, then each
/// position it found. A position whose read refuses is left out with its
/// protocol's other positions kept — a reading of one token id says nothing
/// about the next.
async fn read_one_protocol(
    adapter: &dyn trade_core::cryptotrack::protocols::ProtocolAdapter,
    request: ProtocolRequest<'_>,
) -> Result<ProtocolsRead> {
    let mut out = ProtocolsRead::default();
    let mut kept = std::collections::BTreeSet::new();
    let found = adapter.discover_scan(request).await?;
    out.memory
        .push((adapter.protocol().to_string(), found.memory));
    for key in found.keys {
        match adapter.read_position(request, &key).await {
            Ok(Some(reading)) => {
                kept.insert(key.record_id(request.account_id));
                out.readings.push(reading);
                if let Ok(events) = adapter.events(request, &key).await {
                    out.events.extend(events);
                }
            }
            // The protocol says this position is gone: nothing to store, and
            // not kept, so the vault closes the row it holds for it.
            Ok(None) => {}
            // A refused read says nothing about the position. Swallowing it
            // would leave the pass looking complete while the position is
            // simply missing from the answer, which reads as "closed" — so it
            // is named as missed and kept, never closed.
            Err(error) => {
                kept.insert(key.record_id(request.account_id));
                out.missed
                    .push(format!("{} ({})", adapter.protocol(), error.code()));
            }
        }
    }
    if found.complete {
        out.reviewed.push(ProtocolReview {
            protocol: adapter.protocol().to_string(),
            chain: request.chain,
            kept,
            read_at: trade_core::model::vault::now_iso(),
        });
    } else {
        // The scan could not see all of the history: what it found is read
        // and stored, but a position missing from it may only be further
        // back, so it is no review and closes nothing (ADR-02), and the pass
        // is named partial.
        out.missed
            .push(format!("{} (INCOMPLETE)", adapter.protocol()));
    }
    Ok(out)
}

/// Whether a pass may start here: the sandbox talks to no provider, only the
/// device that runs the jobs reads (the others get the result by sync), and
/// without the user's yes nothing is read at all.
pub fn may_start(sandbox: bool, runs_jobs: bool, consent: bool) -> bool {
    !sandbox && runs_jobs && consent
}

fn start(app: &AppHandle, force: bool) {
    let host = AppHost(app.clone());
    if !may_start(
        crate::commands::sandbox::is_active_app(app),
        crate::sync::runs_jobs(app),
        host.consent(),
    ) {
        return;
    }
    let Some(guard) = TRACKER.begin() else {
        // A pass is running; a forced one is picked up by the next.
        if force {
            TRACKER.force_next();
        }
        return;
    };
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let Ok(reader) = EvmReader::new(host.0.clone()) else {
            return;
        };
        run_pass(&host, &reader, &TRACKER, force).await;
    });
}

/// One pass of the crypto module's tracking, from the scheduler. Returns at
/// once: the pass runs on the async runtime and may take minutes.
pub fn cryptotrack_tick(app: &AppHandle) {
    start(app, false);
}

/// Read now, ignoring the refresh interval (the user asked).
pub fn read_now(app: &AppHandle) -> Result<()> {
    let host = AppHost(app.clone());
    if !host.consent() {
        return Err(Error::InvalidInput(
            "reading the addresses is turned off".into(),
        ));
    }
    if !crate::sync::runs_jobs(app) {
        return Err(Error::InvalidInput(
            "the main device of the group reads the addresses".into(),
        ));
    }
    host.with_vault(|_| Ok(()))?;
    TRACKER.force_next();
    start(app, true);
    Ok(())
}

/// The consent changed: a yes starts the catch-up at once.
pub fn consent_changed(app: &AppHandle) {
    start(app, false);
}

/// What the protocol adapters last read on this device (spec D0), for the
/// command that lists the positions. Empty on a device that has only synced,
/// and that is what makes a momentary figure absent instead of zero.
pub fn live_positions() -> Vec<PositionReading> {
    TRACKER.positions()
}

/// Drop the readings of the protocol adapters from memory: called when the
/// vault locks or is replaced and when the module is wiped.
pub fn forget_live_positions() {
    TRACKER.clear_positions();
}

/// What the module is doing, for the screen.
pub fn status(app: &AppHandle) -> Result<CryptoSyncStatus> {
    let host = AppHost(app.clone());
    let ids = host.with_vault(|vault| {
        Ok(vault
            .crypto_sync_targets()
            .into_iter()
            .map(|target| target.account_id)
            .collect::<Vec<_>>())
    })?;
    Ok(CryptoSyncStatus {
        consent: crate::commands::app_settings::current_settings(app).crypto_tracking_consent,
        running: TRACKER.running(),
        runs_here: crate::sync::runs_jobs(app),
        accounts: TRACKER.statuses(&ids),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use trade_core::Amount;
    use trade_core::cryptotrack::protocols::{
        AdapterFuture, Discovery, FeeReading, PositionKey, PositionKind, PositionStatus,
        ProtocolAdapter, ProtocolReview,
    };
    use trade_core::cryptotrack::sources::evm::{RawEvent, RawKind, Uint256};
    use trade_core::model::chain::Chain;
    use trade_core::model::cryptotrack::{CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg};

    use super::*;

    const ME: &str = "0xabcd00000000000000000000000000000000ef01";
    const OTHER: &str = "0xabcd00000000000000000000000000000000ef02";
    const PEER: &str = "0x1111111111111111111111111111111111111111";

    // -- one protocol's failure is not the others' ------------------------------

    /// An adapter that answers from a script instead of a network: it either
    /// refuses discovery, or lists `keys` and refuses the reads named in
    /// `broken`.
    struct StubAdapter {
        name: &'static str,
        keys: Vec<PositionKey>,
        broken: Vec<String>,
        /// Discovered, but the protocol says it is gone when it is read.
        gone: Vec<String>,
        refuses: bool,
        /// Discovery found `keys` but could not see all the history.
        incomplete: bool,
    }

    impl StubAdapter {
        fn working(name: &'static str, ids: &[&str]) -> Self {
            Self {
                name,
                keys: ids
                    .iter()
                    .map(|id| PositionKey::new(name, Chain::Ethereum, id))
                    .collect(),
                broken: Vec::new(),
                gone: Vec::new(),
                refuses: false,
                incomplete: false,
            }
        }

        fn refusing(name: &'static str) -> Self {
            Self {
                name,
                keys: Vec::new(),
                broken: Vec::new(),
                gone: Vec::new(),
                refuses: true,
                incomplete: false,
            }
        }

        fn incomplete(mut self) -> Self {
            self.incomplete = true;
            self
        }

        fn with_gone(mut self, id: &str) -> Self {
            self.gone.push(id.to_string());
            self
        }

        fn with_broken_read(mut self, id: &str) -> Self {
            self.broken.push(id.to_string());
            self
        }
    }

    impl ProtocolAdapter for StubAdapter {
        fn protocol(&self) -> &'static str {
            self.name
        }

        fn supports(&self, _chain: Chain) -> bool {
            true
        }

        fn discover<'fut>(
            &'fut self,
            _request: ProtocolRequest<'fut>,
        ) -> AdapterFuture<'fut, Vec<PositionKey>> {
            Box::pin(async move {
                if self.refuses {
                    return Err(Error::Network("the endpoint refused the method".into()));
                }
                Ok(self.keys.clone())
            })
        }

        /// Echoes the memory it was given, so a test sees what travelled.
        fn discover_scan<'fut>(
            &'fut self,
            request: ProtocolRequest<'fut>,
        ) -> AdapterFuture<'fut, Discovery> {
            Box::pin(async move {
                let keys = self.discover(request).await?;
                Ok(Discovery {
                    keys,
                    complete: !self.incomplete,
                    memory: Some(format!("after {}", request.memory.unwrap_or("nothing"))),
                })
            })
        }

        fn read_position<'fut>(
            &'fut self,
            _request: ProtocolRequest<'fut>,
            key: &'fut PositionKey,
        ) -> AdapterFuture<'fut, Option<PositionReading>> {
            Box::pin(async move {
                if self.broken.contains(&key.id) {
                    return Err(Error::Network("this one position refused".into()));
                }
                if self.gone.contains(&key.id) {
                    return Ok(None);
                }
                Ok(Some(PositionReading {
                    key: key.clone(),
                    account_id: "cacc-1".into(),
                    name: key.id.clone(),
                    kind: PositionKind::Lending,
                    status: PositionStatus::Open,
                    legs: Vec::new(),
                    fees: FeeReading::none_earned(),
                    receipts: Vec::new(),
                    approximate: false,
                    opened_at: None,
                    closed_at: None,
                    read_at: "2026-11-16T10:00:00Z".into(),
                }))
            })
        }

        fn events<'fut>(
            &'fut self,
            _request: ProtocolRequest<'fut>,
            _key: &'fut PositionKey,
        ) -> AdapterFuture<'fut, Vec<CryptoTx>> {
            Box::pin(async move { Ok(Vec::new()) })
        }
    }

    fn protocol_request<'req>(address: &'req str, account_id: &'req str) -> ProtocolRequest<'req> {
        ProtocolRequest {
            chain: Chain::Ethereum,
            address,
            account_id,
            since: None,
            rpc: None,
            memory: None,
        }
    }

    #[tokio::test]
    async fn one_protocol_that_refuses_does_not_hide_the_others_positions() {
        // A rate limit on Uniswap must not turn an Aave supply into "this
        // address holds nothing in any protocol".
        let registry = Registry::new(vec![
            Box::new(StubAdapter::working("aave-v3", &["usdc-reserve"])),
            Box::new(StubAdapter::refusing("uniswap-v4")),
        ]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await
        .expect("a partial pass is still a pass");
        assert_eq!(read.readings.len(), 1);
        assert_eq!(read.readings[0].key.protocol, "aave-v3");
        assert_eq!(
            read.missed,
            vec!["uniswap-v4 (NETWORK)".to_string()],
            "the protocol that failed is named, with its code and nothing else"
        );
    }

    #[tokio::test]
    async fn a_pass_where_every_protocol_refused_is_an_error_and_not_an_empty_answer() {
        let registry = Registry::new(vec![
            Box::new(StubAdapter::refusing("aave-v3")),
            Box::new(StubAdapter::refusing("uniswap-v4")),
        ]);
        let outcome = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await;
        assert!(
            outcome.is_err(),
            "nothing was read, so the pass failed and the user has to be told"
        );
    }

    #[tokio::test]
    async fn one_position_that_refuses_leaves_the_protocols_other_positions() {
        let registry = Registry::new(vec![Box::new(
            StubAdapter::working("aave-v3", &["usdc-reserve", "weth-reserve"])
                .with_broken_read("usdc-reserve"),
        )]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await
        .expect("the other position is still read");
        assert_eq!(read.readings.len(), 1);
        assert_eq!(read.readings[0].key.id, "weth-reserve");
        assert_eq!(
            read.missed,
            vec!["aave-v3 (NETWORK)".to_string()],
            "a position whose read refused makes the pass partial: it must not look complete"
        );
    }

    fn record_ids(protocol: &str, ids: &[&str]) -> std::collections::BTreeSet<String> {
        ids.iter()
            .map(|id| PositionKey::new(protocol, Chain::Ethereum, id).record_id("cacc-1"))
            .collect()
    }

    #[tokio::test]
    async fn a_protocol_that_answered_with_nothing_is_reviewed_and_keeps_nothing() {
        let registry = Registry::new(vec![Box::new(StubAdapter::working("aave-v3", &[]))]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await
        .expect("an empty answer is an answer");
        assert!(read.readings.is_empty());
        assert_eq!(read.reviewed.len(), 1);
        assert_eq!(read.reviewed[0].protocol, "aave-v3");
        assert_eq!(read.reviewed[0].chain, Chain::Ethereum);
        assert!(read.reviewed[0].kept.is_empty());
        assert!(!read.reviewed[0].read_at.is_empty());
    }

    #[tokio::test]
    async fn a_position_that_is_gone_is_not_kept_and_one_that_refused_is() {
        let registry = Registry::new(vec![Box::new(
            StubAdapter::working("aave-v3", &["usdc-reserve", "weth-reserve", "dai-reserve"])
                .with_gone("usdc-reserve")
                .with_broken_read("weth-reserve"),
        )]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await
        .expect("the pass is partial, not failed");
        assert_eq!(
            read.reviewed[0].kept,
            record_ids("aave-v3", &["weth-reserve", "dai-reserve"]),
            "gone is out; read or merely unreadable is kept"
        );
    }

    #[tokio::test]
    async fn a_protocol_that_refused_has_no_review_so_nothing_is_closed_on_its_word() {
        let registry = Registry::new(vec![
            Box::new(StubAdapter::working("aave-v3", &[])),
            Box::new(StubAdapter::refusing("uniswap-v4")),
        ]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await
        .expect("a partial pass");
        let reviewed: Vec<&str> = read
            .reviewed
            .iter()
            .map(|review| review.protocol.as_str())
            .collect();
        assert_eq!(reviewed, vec!["aave-v3"]);
    }

    #[tokio::test]
    async fn an_incomplete_discovery_is_read_and_named_but_is_not_a_review() {
        // ADR-02: a scan that may not have reached an old position must not
        // tell the vault what the account "still holds", or the position it
        // missed would be closed while it is alive.
        let registry = Registry::new(vec![
            Box::new(StubAdapter::working("aave-v3", &["usdc-reserve"])),
            Box::new(StubAdapter::working("uniswap-v4", &["11", "22"]).incomplete()),
        ]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &Default::default(),
        )
        .await
        .expect("an incomplete scan is not an error for the account");
        assert_eq!(read.readings.len(), 3, "what was found is still read");
        let reviewed: Vec<&str> = read
            .reviewed
            .iter()
            .map(|review| review.protocol.as_str())
            .collect();
        assert_eq!(reviewed, vec!["aave-v3"], "no review for the partial one");
        assert_eq!(read.missed, vec!["uniswap-v4 (INCOMPLETE)".to_string()]);
    }

    #[tokio::test]
    async fn what_an_adapter_remembers_goes_back_to_it_on_the_next_pass() {
        let registry = Registry::new(vec![
            Box::new(StubAdapter::working("aave-v3", &[])),
            Box::new(StubAdapter::working("uniswap-v4", &["11"]).incomplete()),
        ]);
        let memory =
            std::collections::BTreeMap::from([("uniswap-v4".to_string(), "scan-1".to_string())]);
        let read = read_protocols(
            &registry,
            Chain::Ethereum,
            protocol_request(ME, "cacc-1"),
            &memory,
        )
        .await
        .expect("a pass");
        assert_eq!(
            read.memory,
            vec![
                ("aave-v3".to_string(), Some("after nothing".to_string())),
                ("uniswap-v4".to_string(), Some("after scan-1".to_string())),
            ],
            "each adapter gets its own memory and only that"
        );
    }

    type Script = Box<dyn Fn(&str, usize) -> Result<SourceBatch> + Send + Sync>;
    type Reading = (Vec<PositionReading>, Vec<CryptoTx>, Vec<ProtocolReview>);
    type Protocols = Box<dyn Fn(&str) -> Result<Reading> + Send + Sync>;
    type PriceScript =
        Box<dyn Fn(&[(CryptoAsset, NaiveDate)]) -> Result<Vec<CryptoPricePoint>> + Send + Sync>;
    type SharedVault = Arc<Mutex<Option<UnlockedVault>>>;

    struct TestHost {
        vault: SharedVault,
        consent: AtomicBool,
        updates: Mutex<Vec<String>>,
    }

    impl TestHost {
        fn new(vault: UnlockedVault, consent: bool) -> Self {
            Self {
                vault: Arc::new(Mutex::new(Some(vault))),
                consent: AtomicBool::new(consent),
                updates: Mutex::new(Vec::new()),
            }
        }

        fn updates(&self) -> Vec<String> {
            self.updates.lock().unwrap().clone()
        }
    }

    impl Host for TestHost {
        fn consent(&self) -> bool {
            self.consent.load(Ordering::SeqCst)
        }

        fn with_vault<T>(&self, work: impl FnOnce(&mut UnlockedVault) -> Result<T>) -> Result<T> {
            let mut guard = self.vault.lock().unwrap();
            match guard.as_mut() {
                Some(vault) => work(vault),
                None => Err(Error::RequiresUnlock),
            }
        }

        fn updated(&self, account_id: &str) {
            self.updates.lock().unwrap().push(account_id.to_string());
        }
    }

    /// Answers from a script and counts every call: a test proves "no
    /// network" by this count staying at zero. It also checks, on every call,
    /// that the vault's lock is free: a pass that held it across the network
    /// would fail here.
    struct Stub {
        calls: AtomicUsize,
        vault: SharedVault,
        /// `(address, how many pages were asked before)` to the answer.
        script: Script,
        asked: Mutex<HashMap<String, usize>>,
        /// Runs in the middle of every page, while the network "answers".
        during: Box<dyn Fn() + Send + Sync>,
        /// What the protocol adapters would have read for an address.
        protocols: Protocols,
        /// What the price provider answers for one day's request.
        prices: PriceScript,
        /// Every price request, as the days and asset keys it named.
        priced: Mutex<Vec<(NaiveDate, Vec<String>)>>,
    }

    impl Stub {
        fn new(
            vault: &SharedVault,
            script: impl Fn(&str, usize) -> Result<SourceBatch> + Send + Sync + 'static,
        ) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                vault: vault.clone(),
                script: Box::new(script),
                asked: Mutex::new(HashMap::new()),
                during: Box::new(|| {}),
                protocols: Box::new(|_| Ok((Vec::new(), Vec::new(), Vec::new()))),
                prices: Box::new(|_| Ok(Vec::new())),
                priced: Mutex::new(Vec::new()),
            }
        }

        /// The answer of the protocol adapters for every address.
        fn holding(mut self, reading: Reading) -> Self {
            self.protocols = Box::new(move |_| Ok(reading.clone()));
            self
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        /// The provider's answer for a day's price request.
        fn quoting(
            mut self,
            script: impl Fn(&[(CryptoAsset, NaiveDate)]) -> Result<Vec<CryptoPricePoint>>
            + Send
            + Sync
            + 'static,
        ) -> Self {
            self.prices = Box::new(script);
            self
        }

        fn price_requests(&self) -> Vec<(NaiveDate, Vec<String>)> {
            self.priced.lock().unwrap().clone()
        }
    }

    impl Reader for Stub {
        async fn page(
            &self,
            target: &CryptoSyncTarget,
            _cursor: Option<&str>,
        ) -> Result<SourceBatch> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                self.vault.try_lock().is_ok(),
                "the vault lock was held while the network was read"
            );
            (self.during)();
            let index = {
                let mut asked = self.asked.lock().unwrap();
                let slot = asked.entry(target.address.clone()).or_default();
                *slot += 1;
                *slot - 1
            };
            (self.script)(&target.address, index)
        }

        async fn balances(
            &self,
            _target: &CryptoSyncTarget,
            _tokens: &[RawAsset],
        ) -> Result<SourceBatch> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                self.vault.try_lock().is_ok(),
                "the vault lock was held while the network was read"
            );
            Ok(SourceBatch::default())
        }

        async fn positions(&self, target: &CryptoSyncTarget) -> Result<ProtocolsRead> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                self.vault.try_lock().is_ok(),
                "the vault lock was held while the network was read"
            );
            (self.protocols)(&target.address).map(|(readings, events, reviewed)| ProtocolsRead {
                readings,
                events,
                reviewed,
                missed: Vec::new(),
                memory: Vec::new(),
            })
        }

        async fn historic_prices(
            &self,
            wanted: &[(CryptoAsset, NaiveDate)],
        ) -> Result<Vec<CryptoPricePoint>> {
            assert!(
                self.vault.try_lock().is_ok(),
                "the vault lock was held while the prices were fetched"
            );
            let day = wanted.first().map(|(_, date)| *date).unwrap();
            assert!(
                wanted.iter().all(|(_, date)| *date == day),
                "one day per request"
            );
            self.priced
                .lock()
                .unwrap()
                .push((day, wanted.iter().map(|(asset, _)| asset.key()).collect()));
            (self.prices)(wanted)
        }
    }

    fn vault_with(dir: &std::path::Path, name: &str, accounts: &[(&str, &str)]) -> UnlockedVault {
        let mut vault = UnlockedVault::create(dir.join(name), "pw").unwrap();
        for (id, address) in accounts {
            vault.doc.crypto_accounts.push(CryptoAccount::new(
                id,
                id,
                Chain::Ethereum,
                address,
                "2026-01-01T00:00:00Z",
            ));
        }
        vault.persist().unwrap();
        vault
    }

    fn event(index: u64, address: &str) -> RawEvent {
        let hash = format!("0x{index:064x}");
        RawEvent {
            id: format!("evm:1:{hash}:tx"),
            chain_id: 1,
            hash,
            kind: RawKind::Transaction,
            block: 21_000_000 + index,
            time: "2026-03-01T10:00:00Z".into(),
            from: PEER.into(),
            to: address.into(),
            asset: RawAsset::Native { decimals: 18 },
            value: Uint256::from_decimal("1000000000000000000").unwrap(),
            method: None,
            succeeded: true,
            fee: None,
            payload: "{}".into(),
        }
    }

    fn one_page(address: &str, index: u64) -> Result<SourceBatch> {
        Ok(SourceBatch {
            raw_events: vec![event(index, address)],
            ..SourceBatch::default()
        })
    }

    fn assert_send<T: Send>(_: &T) {}

    #[tokio::test]
    async fn without_consent_there_are_no_network_calls() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), false);
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1));
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, true).await;

        assert_eq!(stub.calls(), 0);
        assert_eq!(report, PassReport::default());
        assert!(host.updates().is_empty());
        // Even a forced pass (the "read now" button) goes through the same gate.
        assert!(!may_start(false, true, false));
    }

    fn eth() -> CryptoAsset {
        CryptoAsset::Native {
            chain: Chain::Ethereum,
        }
    }

    fn march_first() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()
    }

    fn quote(wanted: &[(CryptoAsset, NaiveDate)]) -> Result<Vec<CryptoPricePoint>> {
        Ok(wanted
            .iter()
            .map(|(asset, date)| CryptoPricePoint {
                token_key: asset.key(),
                value_usd: Amount::parse("2000").unwrap(),
                ts: prices::date_ts(*date),
                source: "defillama".into(),
            })
            .collect())
    }

    #[tokio::test]
    async fn a_missing_price_is_asked_once_and_stored() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1)).quoting(quote);
        let tracker = Tracker::new();

        run_pass(&host, &stub, &tracker, false).await;

        assert_eq!(
            stub.price_requests(),
            vec![(march_first(), vec![eth().key()])]
        );
        host.with_vault(|vault| {
            let found = prices::lookup(&vault.doc, &eth(), march_first());
            assert_eq!(found.value, Amount::parse("2000").ok());
            assert!(!found.stale);
            Ok(())
        })
        .unwrap();
        let status = tracker.statuses(&["acc-a".to_string()]);
        assert_eq!(status[0].state, CryptoSyncState::Done);
        assert_eq!(status[0].error, None);
    }

    #[tokio::test]
    async fn without_consent_no_price_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), false);
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1)).quoting(quote);
        run_pass(&host, &stub, &Tracker::new(), true).await;
        assert!(stub.price_requests().is_empty());
    }

    #[tokio::test]
    async fn consent_withdrawn_before_the_price_request_stops_it() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(TestHost::new(
            vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]),
            true,
        ));
        let mut stub = Stub::new(&host.vault, |address, _| one_page(address, 1)).quoting(quote);
        let withdrawn = host.clone();
        // The user says no while the protocols are being read.
        stub.protocols = Box::new(move |_| {
            withdrawn.consent.store(false, Ordering::SeqCst);
            Ok((Vec::new(), Vec::new(), Vec::new()))
        });
        let report = run_pass(&*host, &stub, &Tracker::new(), false).await;
        assert!(report.cancelled);
        assert!(stub.price_requests().is_empty());
    }

    #[tokio::test]
    async fn a_second_pass_asks_neither_for_what_is_stored_nor_for_what_was_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1)).quoting(quote);
        let tracker = Tracker::new();
        run_pass(&host, &stub, &tracker, false).await;
        run_pass(&host, &stub, &tracker, true).await;
        assert_eq!(stub.price_requests().len(), 1, "stored: not asked again");

        // A provider with nothing for the day is not asked again either.
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "b.wallet", &[("acc-a", ME)]), true);
        let stub =
            Stub::new(&host.vault, |address, _| one_page(address, 1)).quoting(|_| Ok(Vec::new()));
        let tracker = Tracker::new();
        run_pass(&host, &stub, &tracker, false).await;
        run_pass(&host, &stub, &tracker, true).await;
        assert_eq!(stub.price_requests().len(), 1, "not found: not asked again");
        let status = tracker.statuses(&["acc-a".to_string()]);
        assert_eq!(status[0].error, None, "no price is not a failure");
    }

    #[tokio::test]
    async fn a_refused_price_request_makes_the_pass_partial_and_keeps_the_prices() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        host.with_vault(|vault| {
            let kept = quote(&[(eth(), NaiveDate::from_ymd_opt(2026, 1, 5).unwrap())])?;
            vault.crypto_apply_prices(&kept).map(|_| ())
        })
        .unwrap();
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1))
            .quoting(|_| Err(Error::Network("http://secret.example/0xabc".into())));
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert_eq!((report.read, report.failed), (1, 0));
        let status = tracker.statuses(&["acc-a".to_string()]);
        assert_eq!(status[0].state, CryptoSyncState::Done);
        assert_eq!(
            status[0].error.as_deref(),
            Some("PARTIAL: prices (NETWORK)")
        );
        host.with_vault(|vault| {
            assert_eq!(vault.doc.crypto_prices.len(), 1, "what was stored stays");
            assert_eq!(vault.doc.crypto_events.len(), 1, "the history is in");
            Ok(())
        })
        .unwrap();
        // An outage is not "no price": the next pass tries again.
        run_pass(&host, &stub, &tracker, true).await;
        assert_eq!(stub.price_requests().len(), 2);
    }

    #[test]
    fn a_pass_asks_for_a_bounded_number_of_days() {
        let tracker = Tracker::new();
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let missing: Vec<_> = (0..60)
            .map(|offset| (eth(), start + chrono::Days::new(offset)))
            .collect();
        let asked = tracker.prices_to_ask(missing);
        assert_eq!(asked.len(), MAX_PRICE_DAYS_PER_PASS);
        assert_eq!(asked[0].1, start, "the oldest day goes first");
    }

    #[tokio::test]
    async fn with_consent_the_account_is_read_and_stored() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1));
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert_eq!(
            (report.read, report.failed, report.cancelled),
            (1, 0, false)
        );
        assert!(stub.calls() >= 2, "a page and the node's balances");
        assert_eq!(host.updates(), vec!["acc-a".to_string()]);
        host.with_vault(|vault| {
            assert_eq!(vault.doc.crypto_events.len(), 1);
            let cursor = &vault.doc.crypto_cursors[0];
            assert_eq!(cursor.id, "evm|acc-a");
            assert!(cursor.cursor.is_none(), "a finished read keeps no page");
            Ok(())
        })
        .unwrap();
        let status = tracker.statuses(&["acc-a".to_string()]);
        assert_eq!(status[0].state, CryptoSyncState::Done);
        assert_eq!(status[0].added, 1);

        // Read again at once: nothing is due, so nothing is asked.
        let before = stub.calls();
        run_pass(&host, &stub, &tracker, false).await;
        assert_eq!(stub.calls(), before);
    }

    /// `MutexGuard` is not `Send`: a pass that held the vault across an
    /// `.await` could not be spawned on the runtime, and would not compile
    /// here. The stub also checks the lock is free while it "talks".
    #[tokio::test]
    async fn the_cycle_is_send() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1));
        let tracker = Tracker::new();
        let pass = run_pass(&host, &stub, &tracker, false);
        assert_send(&pass);
        pass.await;
        assert_eq!(host.updates().len(), 1);
    }

    #[tokio::test]
    async fn locking_during_a_poll_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wallet");
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let on_disk = std::fs::read(&path).unwrap();
        let mut stub = Stub::new(&host.vault, |address, _| one_page(address, 1));
        let vault = host.vault.clone();
        // The user locks the app while the explorer is answering.
        stub.during = Box::new(move || {
            *vault.lock().unwrap() = None;
        });
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert!(report.cancelled);
        assert!(host.updates().is_empty());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            on_disk,
            "the closed vault was written"
        );
        assert_eq!(stub.calls(), 1, "no request after the lock");
    }

    #[tokio::test]
    async fn another_vault_opened_during_a_poll_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let other_path = dir.path().join("b.wallet");
        let other = vault_with(dir.path(), "b.wallet", &[("acc-a", ME)]);
        let other_bytes = std::fs::read(&other_path).unwrap();
        let mut stub = Stub::new(&host.vault, |address, _| one_page(address, 1));
        let vault = host.vault.clone();
        let swap = Mutex::new(Some(other));
        stub.during = Box::new(move || {
            *vault.lock().unwrap() = swap.lock().unwrap().take();
        });
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert!(report.cancelled);
        assert!(host.updates().is_empty());
        assert_eq!(std::fs::read(&other_path).unwrap(), other_bytes);
        host.with_vault(|vault| {
            assert!(vault.doc.crypto_events.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn one_failing_account_leaves_the_others_alone() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(
            vault_with(dir.path(), "a.wallet", &[("acc-a", ME), ("acc-b", OTHER)]),
            true,
        );
        let stub = Stub::new(&host.vault, |address, _| {
            if address == ME {
                Err(Error::Network("explorer down".into()))
            } else {
                one_page(address, 7)
            }
        });
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert_eq!(
            (report.read, report.failed, report.cancelled),
            (1, 1, false)
        );
        let status = tracker.statuses(&["acc-a".to_string(), "acc-b".to_string()]);
        assert_eq!(status[0].state, CryptoSyncState::Failed);
        assert_eq!(status[0].error.as_deref(), Some("NETWORK"));
        assert_eq!(status[1].state, CryptoSyncState::Done);
        host.with_vault(|vault| {
            assert_eq!(vault.doc.crypto_events.len(), 1);
            assert_eq!(vault.doc.crypto_events[0].account_id, "acc-b");
            Ok(())
        })
        .unwrap();

        // The failed one waits before the next try instead of being hammered.
        let before = stub.calls();
        run_pass(&host, &stub, &tracker, false).await;
        assert_eq!(stub.calls(), before);
    }

    #[tokio::test]
    async fn withdrawing_the_consent_stops_the_pass_before_the_next_request() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(TestHost::new(
            vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]),
            true,
        ));
        let next = Cursor::at(Stream::TokenTransfers, None).encode();
        let stub = Stub::new(&host.vault, move |address, _| {
            Ok(SourceBatch {
                raw_events: vec![event(1, address)],
                next_cursor: Some(next.clone()),
                ..SourceBatch::default()
            })
        });
        let flag = host.clone();
        let mut stub = stub;
        stub.during = Box::new(move || flag.consent.store(false, Ordering::SeqCst));
        let tracker = Tracker::new();

        let report = run_pass(&*host, &stub, &tracker, false).await;

        assert!(report.cancelled);
        assert_eq!(stub.calls(), 1);
    }

    #[tokio::test]
    async fn events_are_written_in_batches_not_per_page() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(vault_with(dir.path(), "a.wallet", &[("acc-a", ME)]), true);
        let next = |block: u64| {
            Cursor::at(
                Stream::Transactions,
                Some(
                    [("block_number".to_string(), serde_json::json!(block))]
                        .into_iter()
                        .collect(),
                ),
            )
            .encode()
        };
        let stub = Stub::new(&host.vault, move |address, index| {
            let first = u64::try_from(index).unwrap() * 250;
            Ok(SourceBatch {
                raw_events: (first..first + 250)
                    .map(|index| event(index, address))
                    .collect(),
                // Three pages of one list, the last one ends the read.
                next_cursor: (index < 2).then(|| next(first)),
                ..SourceBatch::default()
            })
        });
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert_eq!(report.read, 1);
        // 3 pages, 2 saves: 500 events cross the batch size, the last page ends the read.
        assert_eq!(host.updates().len(), 2);
        host.with_vault(|vault| {
            assert_eq!(vault.doc.crypto_events.len(), 750);
            Ok(())
        })
        .unwrap();
    }

    /// Spec D0: the pass stores the durable half of a position and keeps the
    /// reading itself on the device.
    #[tokio::test]
    async fn a_position_is_stored_and_its_reading_stays_on_this_device() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(
            vault_with(dir.path(), "one.wallet", &[("cacc-1", ME)]),
            true,
        );
        let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve");
        let reading = PositionReading {
            key: key.clone(),
            account_id: "cacc-1".into(),
            name: "USDC supply".into(),
            kind: PositionKind::Lending,
            status: PositionStatus::Open,
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Native {
                    chain: Chain::Ethereum,
                },
                amount: Amount::parse("1").unwrap(),
                counterparty: None,
            }],
            fees: FeeReading::none_earned(),
            receipts: Vec::new(),
            approximate: true,
            opened_at: None,
            closed_at: None,
            read_at: "2026-03-02T10:00:00Z".into(),
        };
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1)).holding((
            vec![reading.clone()],
            Vec::new(),
            Vec::new(),
        ));
        let tracker = Tracker::new();

        let report = run_pass(&host, &stub, &tracker, false).await;

        assert_eq!(report.read, 1);
        host.with_vault(|vault| {
            assert_eq!(vault.doc.crypto_positions, vec![reading.stored()]);
            Ok(())
        })
        .unwrap();
        // The momentary half is not in the document and is on the device.
        let live = tracker.positions();
        assert_eq!(live, vec![reading]);
        assert!(live[0].approximate);
    }

    #[test]
    fn clearing_the_positions_forgets_every_reading() {
        let tracker = Tracker::new();
        tracker.remember_positions("cacc-1", vec![supply("2026-11-16T10:00:00Z")]);
        tracker.remember_positions("cacc-2", vec![supply("2026-11-16T10:00:00Z")]);
        assert_eq!(tracker.positions().len(), 2);
        tracker.clear_positions();
        assert!(tracker.positions().is_empty());
    }

    fn supply(read_at: &str) -> PositionReading {
        PositionReading {
            key: PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve"),
            account_id: "cacc-1".into(),
            name: "USDC supply".into(),
            kind: PositionKind::Lending,
            status: PositionStatus::Open,
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Native {
                    chain: Chain::Ethereum,
                },
                amount: Amount::parse("1").unwrap(),
                counterparty: None,
            }],
            fees: FeeReading::none_earned(),
            receipts: Vec::new(),
            approximate: false,
            opened_at: None,
            closed_at: None,
            read_at: read_at.into(),
        }
    }

    fn aave_review(kept: &[&PositionReading], read_at: &str) -> ProtocolReview {
        ProtocolReview {
            protocol: "aave-v3".into(),
            chain: Chain::Ethereum,
            kept: kept.iter().map(|live| live.record_id()).collect(),
            read_at: read_at.into(),
        }
    }

    /// One more pass over `host` that the protocols answer with `answer`.
    async fn pass_answering(host: &TestHost, answer: Reading) {
        let stub = Stub::new(&host.vault, |address, _| one_page(address, 1)).holding(answer);
        run_pass(host, &stub, &Tracker::new(), true).await;
    }

    fn stored_supply(host: &TestHost) -> Option<trade_core::model::cryptotrack::CryptoPosition> {
        host.with_vault(|vault| Ok(vault.doc.crypto_positions.first().cloned()))
            .unwrap()
    }

    /// Spec D0 / review item 7.4: a full withdrawal answers with no reading
    /// at all, and used to leave the stored position open for good.
    #[tokio::test]
    async fn a_full_withdrawal_closes_the_stored_position_and_tells_the_screens() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(
            vault_with(dir.path(), "one.wallet", &[("cacc-1", ME)]),
            true,
        );
        let held = supply("2026-03-02T10:00:00Z");
        pass_answering(
            &host,
            (
                vec![held.clone()],
                Vec::new(),
                vec![aave_review(&[&held], "2026-03-02T10:00:00Z")],
            ),
        )
        .await;
        let before = host.updates().len();

        // Everything withdrawn: Aave is asked, answers, and lists nothing.
        pass_answering(
            &host,
            (
                Vec::new(),
                Vec::new(),
                vec![aave_review(&[], "2026-03-03T10:00:00Z")],
            ),
        )
        .await;

        let closed = stored_supply(&host).expect("the row stays");
        assert!(closed.legs.iter().all(|leg| leg.amount.is_zero()));
        assert_eq!(closed.updated_at, "2026-03-03T10:00:00Z");
        // The pass's own completion tells the screens once; the close is a
        // second, separate change they must hear about.
        assert_eq!(host.updates().len(), before + 2, "the screens are told");
    }

    #[tokio::test]
    async fn an_empty_answer_without_a_review_leaves_a_stored_position_open_and_quiet() {
        let dir = tempfile::tempdir().unwrap();
        let host = TestHost::new(
            vault_with(dir.path(), "one.wallet", &[("cacc-1", ME)]),
            true,
        );
        let held = supply("2026-03-02T10:00:00Z");
        pass_answering(
            &host,
            (
                vec![held.clone()],
                Vec::new(),
                vec![aave_review(&[&held], "2026-03-02T10:00:00Z")],
            ),
        )
        .await;
        let before = host.updates().len();

        // The protocol refused: no reading, no review.
        pass_answering(&host, (Vec::new(), Vec::new(), Vec::new())).await;

        assert_eq!(stored_supply(&host), Some(held.stored()));
        assert_eq!(
            host.updates().len(),
            before + 1,
            "only the pass's own completion: the positions changed nothing"
        );
    }

    #[test]
    fn only_the_device_that_runs_the_jobs_reads() {
        assert!(may_start(false, true, true));
        // A client of a sync group: the main device reads, this one gets it by sync.
        assert!(!may_start(false, false, true));
        assert!(!may_start(true, true, true));
        assert!(!may_start(false, true, false));
    }
}
