//! IPC types of the module's background tracking: its status per account, the
//! one-time consent that has to be given before any network call, and a
//! refresh on demand.
//!
//! Owner: spec E6, together with the tick in
//! `crates/app/src/cryptotrack/jobs.rs`.
//!
//! Commands: `crates/app/src/commands/cryptotrack/sync.rs`.
//!
//! The second half of the file is not IPC: plain data the tick passes between
//! the vault and the network (`service/cryptotrack/apply.rs`), kept here
//! because the service module is private to the crate and the tick lives in
//! another one.

use serde::{Deserialize, Serialize};

use mwt_cryptotrack::sources::evm::RawAsset;
use mwt_model::Chain;

/// What an account's last read did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CryptoSyncState {
    /// Not read in this session yet.
    Waiting,
    /// Being read now.
    Reading,
    /// The last read finished.
    Done,
    /// The last read failed; `error` says why and the next try is scheduled.
    Failed,
}

/// One followed account's reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoSyncAccountStatus {
    pub account_id: String,
    pub state: CryptoSyncState,
    /// When the last read finished well (RFC3339).
    pub last_ok_at: Option<String>,
    /// The error code of the last failed read (`NETWORK`, `UNSUPPORTED`...).
    pub error: Option<String>,
    /// Operations added by the read in progress or the last one.
    pub added: u32,
}

/// What the module is doing in the background.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CryptoSyncStatus {
    /// The user's answer to "may the module talk to the providers": `None`
    /// until asked. Without a yes there is no network call at all.
    pub consent: Option<bool>,
    /// A pass is running now.
    pub running: bool,
    /// This device runs the tracking (in a sync group only the main device
    /// does; the others get the result by sync).
    pub runs_here: bool,
    pub accounts: Vec<CryptoSyncAccountStatus>,
}

// -- not IPC: what the tick passes between the vault and the network ----------

/// One account to read, copied out of the document.
#[derive(Debug, Clone, PartialEq)]
pub struct CryptoSyncTarget {
    pub account_id: String,
    pub chain: Chain,
    /// Normalized.
    pub address: String,
    /// The day the history is read back to (RFC3339).
    pub start: String,
    /// Where an interrupted read stands; `None` starts at the newest page.
    pub cursor: Option<String>,
    /// An earlier read of this account finished: this one is a refresh and
    /// may skip a list that has nothing new.
    pub refresh: bool,
    /// When that read finished, in Unix milliseconds; `None` while the first
    /// read has not finished.
    pub finished_at_ms: Option<i64>,
    /// Tokens whose balance the node is asked for.
    pub tokens: Vec<RawAsset>,
    /// What each protocol adapter left for its next pass over this account,
    /// by protocol id (opaque text, the device's own).
    pub protocol_memory: std::collections::BTreeMap<String, String>,
}

/// What to store from the pages read so far.
#[derive(Debug, Clone, Copy)]
pub struct CryptoPages<'run> {
    pub account_id: &'run str,
    /// When the pages were read (RFC3339 UTC).
    pub read_at: &'run str,
    /// These pages end the read: the history is back to the start date.
    pub finished: bool,
    /// The account's earlier read had finished
    /// ([`CryptoSyncTarget::refresh`]).
    pub refresh: bool,
}

/// What storing the pages did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoPagesApplied {
    pub added: usize,
    pub rewritten: usize,
    /// Operations the module could not read.
    pub unread: usize,
    /// Items the source itself refused.
    pub rejected: usize,
    /// The cursor the read continues from; `None` when it is finished.
    pub next_cursor: Option<String>,
}
