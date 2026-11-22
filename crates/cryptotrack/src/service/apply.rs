//! Rules of the tracking's cursors and of the read targets (spec E6).
//!
//! The tick in `crates/app/src/cryptotrack/jobs.rs` works in three steps so
//! that no vault lock is held while the network answers (`AGENTS.md`, rule 3):
//!
//! 1. `usecases::cryptotrack::apply::crypto_sync_targets` copies what a read needs out of the document:
//!    which accounts, from which cursor, which tokens;
//! 2. the network is read with the vault free;
//! 3. the scenario `usecases::cryptotrack::apply::crypto_apply_pages` takes
//!    the pages, keeps the cursor with the events of the same write, and
//!    saves once.
//!
//! **The cursor** says where this device's read of an account stands. It is
//! the device's own (it names a page of a provider this device talked to and
//! never syncs, `sync::registry::LOCAL`):
//!
//! * no record: the account was never read;
//! * a record with a cursor: a read was cut short and resumes there;
//! * a record without one: a read finished at `updated_at`. The next read
//!   starts again at the newest page, and a list whose first page brings
//!   nothing new is skipped, so a refresh of a quiet address costs a few
//!   requests instead of the whole history.
//!
//! Every function here works on the document it is given and reads no clock:
//! the scenario passes the moment of the write.

use std::collections::BTreeMap;

use crate::prices;
use crate::sources::evm::{Cursor, SOURCE_ID, Stream};
use mwt_model::VaultDoc;
use mwt_model::{CryptoCursor, CryptoPricePoint};

/// The `source` of the records that hold an adapter's memory, apart from the
/// source cursors (their id never starts with `protocol|`).
const PROTOCOL_SOURCE: &str = "protocol";

/// Whether the document follows an account of this id.
pub fn has_account(doc: &VaultDoc, account_id: &str) -> bool {
    doc.crypto_accounts
        .iter()
        .any(|account| account.id == account_id)
}

/// Where this device's read of an account stands.
pub fn cursor_of<'doc>(doc: &'doc VaultDoc, account_id: &str) -> Option<&'doc CryptoCursor> {
    let id = cursor_id(account_id);
    doc.crypto_cursors.iter().find(|record| record.id == id)
}

/// What the protocol adapters left for themselves, by protocol id.
pub fn protocol_memory(doc: &VaultDoc, account_id: &str) -> BTreeMap<String, String> {
    let prefix = memory_id(account_id, "");
    doc.crypto_cursors
        .iter()
        .filter(|record| record.source == PROTOCOL_SOURCE && record.account_id == account_id)
        .filter_map(|record| {
            let protocol = record.id.strip_prefix(&prefix)?;
            Some((protocol.to_string(), record.cursor.clone()?))
        })
        .collect()
}

/// Keep what the protocol adapters want to remember about an account
/// (`None` forgets), beside the source cursors and as local as they are:
/// a block a scan has reached names this device's own reading. Reports
/// whether anything differs.
pub fn remember_protocols(
    doc: &mut VaultDoc,
    account_id: &str,
    memory: &[(String, Option<String>)],
    now: &str,
) -> bool {
    let mut changed = false;
    for (protocol, state) in memory {
        let id = memory_id(account_id, protocol);
        let slot = doc.crypto_cursors.iter().position(|record| record.id == id);
        match (slot, state) {
            (Some(at), Some(text)) => {
                if doc.crypto_cursors[at].cursor.as_deref() != Some(text) {
                    doc.crypto_cursors[at].cursor = Some(text.clone());
                    doc.crypto_cursors[at].updated_at = now.to_string();
                    changed = true;
                }
            }
            (None, Some(text)) => {
                doc.crypto_cursors.push(CryptoCursor {
                    id,
                    source: PROTOCOL_SOURCE.to_string(),
                    account_id: account_id.to_string(),
                    cursor: Some(text.clone()),
                    updated_at: now.to_string(),
                });
                changed = true;
            }
            (Some(at), None) => {
                doc.crypto_cursors.remove(at);
                changed = true;
            }
            (None, None) => {}
        }
    }
    changed
}

/// Move the account's cursor; reports whether it moved (the same cursor
/// again is no change, and leaves `updated_at` alone).
pub fn set_cursor(doc: &mut VaultDoc, account_id: &str, cursor: Option<String>, now: &str) -> bool {
    let id = cursor_id(account_id);
    match doc.crypto_cursors.iter_mut().find(|record| record.id == id) {
        Some(record) if record.cursor == cursor => false,
        Some(record) => {
            record.cursor = cursor;
            record.updated_at = now.to_string();
            true
        }
        None => {
            doc.crypto_cursors.push(CryptoCursor {
                id,
                source: SOURCE_ID.to_string(),
                account_id: account_id.to_string(),
                cursor,
                updated_at: now.to_string(),
            });
            true
        }
    }
}

/// Put the account's cursor record back as it was (`None`: there was none),
/// when the write it belonged to failed.
pub fn restore_cursor(doc: &mut VaultDoc, account_id: &str, before: Option<CryptoCursor>) {
    let id = cursor_id(account_id);
    doc.crypto_cursors.retain(|record| record.id != id);
    doc.crypto_cursors.extend(before);
}

/// Forget where an account's read stands; reports whether there was a record.
pub fn reset_cursor(doc: &mut VaultDoc, account_id: &str) -> bool {
    let id = cursor_id(account_id);
    let before = doc.crypto_cursors.len();
    doc.crypto_cursors.retain(|record| record.id != id);
    doc.crypto_cursors.len() != before
}

/// Where a refresh goes on when the page it just read brought nothing new:
/// the start of the list after the one `next` points into. `None` when `next`
/// is no history list (the held tokens are a state, not a history) or the
/// list was the last.
pub fn next_list_after(next: Option<&str>) -> Option<Option<String>> {
    let skipped = next
        .and_then(|text| Cursor::decode(Some(text)).ok())
        .filter(|cursor| is_history(cursor.stream))?;
    Some(
        skipped
            .stream
            .next()
            .map(|stream| Cursor::at(stream, None).encode()),
    )
}

/// Store the historic prices a pass fetched; returns how many points are new
/// or different. `crypto_prices` is a cache that is never thinned, so this is
/// the only writer it needs.
pub fn store_prices(doc: &mut VaultDoc, fresh: &[CryptoPricePoint]) -> usize {
    let before = doc.crypto_prices.clone();
    prices::apply_historic(doc, fresh);
    fresh
        .iter()
        .filter(|point| !point.value_usd.is_zero() && !before.contains(point))
        .count()
}

fn cursor_id(account_id: &str) -> String {
    format!("{SOURCE_ID}|{account_id}")
}

fn memory_id(account_id: &str, protocol: &str) -> String {
    format!("{PROTOCOL_SOURCE}|{account_id}|{protocol}")
}

/// The lists of events; the held tokens are a state, not a history.
fn is_history(stream: Stream) -> bool {
    !matches!(stream, Stream::Tokens)
}
