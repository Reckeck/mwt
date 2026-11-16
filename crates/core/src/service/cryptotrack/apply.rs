//! Where the background tracking meets the vault (spec E6).
//!
//! The tick in `crates/app/src/cryptotrack/jobs.rs` works in three steps so
//! that no vault lock is held while the network answers (`AGENTS.md`, rule 3):
//!
//! 1. [`UnlockedVault::crypto_sync_targets`] copies what a read needs out of
//!    the document: which accounts, from which cursor, which tokens;
//! 2. the network is read with the vault free;
//! 3. [`UnlockedVault::crypto_apply_pages`] takes the pages, keeps the cursor
//!    with the events of the same write, and saves once.
//!
//! Pages are written in batches, not one by one: a save encrypts the whole
//! document, so a long history is cheaper to store a few hundred events at a
//! time than fifty.
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

use std::collections::BTreeMap;

use super::super::UnlockedVault;
use super::recompute::CryptoBatch;
use crate::cryptotrack::sources::evm::{Cursor, RawAsset, SOURCE_ID, SourceBatch, Stream};
use crate::cryptotrack::{chains, prices, tokens};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::sync::{CryptoPages, CryptoPagesApplied, CryptoSyncTarget};
use crate::model::cryptotrack::{CryptoCursor, CryptoPricePoint, normalize_address};
use crate::model::vault::{iso_to_millis, now_iso};

impl UnlockedVault {
    /// The accounts a pass of the tracking reads. An archived account is
    /// watched but not followed, and an exchange account is another spec's
    /// source (E7); only the EVM networks have a source here.
    pub fn crypto_sync_targets(&self) -> Vec<CryptoSyncTarget> {
        self.doc
            .crypto_accounts
            .iter()
            .filter(|account| !account.archived)
            .filter(|account| matches!(account.source.as_deref(), None | Some(SOURCE_ID)))
            .filter(|account| chains::evm_chain(account.chain).is_some())
            .map(|account| {
                let record = self.crypto_cursor_of(&account.id);
                let held = self
                    .doc
                    .crypto_tokens
                    .iter()
                    .filter(|token| token.chain == account.chain)
                    .filter(|token| tokens::counts_in_totals(&self.doc, &token.id()))
                    .map(|token| RawAsset::Token {
                        contract: token.contract.clone(),
                        symbol: token.symbol.clone(),
                        name: token.name.clone(),
                        decimals: token.decimals,
                        suspect: false,
                    })
                    .collect();
                CryptoSyncTarget {
                    account_id: account.id.clone(),
                    chain: account.chain,
                    address: normalize_address(account.chain, &account.address),
                    start: account.created_at.clone(),
                    cursor: record.and_then(|record| record.cursor.clone()),
                    refresh: record.is_some_and(|record| record.cursor.is_none()),
                    finished_at_ms: record
                        .filter(|record| record.cursor.is_none())
                        .map(|record| iso_to_millis(&record.updated_at)),
                    tokens: held,
                    protocol_memory: self.crypto_protocol_memory(&account.id),
                }
            })
            .collect()
    }

    fn crypto_cursor_of(&self, account_id: &str) -> Option<&CryptoCursor> {
        let id = cursor_id(account_id);
        self.doc
            .crypto_cursors
            .iter()
            .find(|record| record.id == id)
    }

    /// What the protocol adapters left for themselves, by protocol id.
    fn crypto_protocol_memory(&self, account_id: &str) -> BTreeMap<String, String> {
        let prefix = memory_id(account_id, "");
        self.doc
            .crypto_cursors
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
    /// a block a scan has reached names this device's own reading. Saves
    /// only when something differs.
    pub fn crypto_remember_protocols(
        &mut self,
        account_id: &str,
        memory: &[(String, Option<String>)],
    ) -> Result<()> {
        let mut changed = false;
        for (protocol, state) in memory {
            let id = memory_id(account_id, protocol);
            let slot = self
                .doc
                .crypto_cursors
                .iter()
                .position(|record| record.id == id);
            match (slot, state) {
                (Some(at), Some(text)) => {
                    if self.doc.crypto_cursors[at].cursor.as_deref() != Some(text) {
                        self.doc.crypto_cursors[at].cursor = Some(text.clone());
                        self.doc.crypto_cursors[at].updated_at = now_iso();
                        changed = true;
                    }
                }
                (None, Some(text)) => {
                    self.doc.crypto_cursors.push(CryptoCursor {
                        id,
                        source: PROTOCOL_SOURCE.to_string(),
                        account_id: account_id.to_string(),
                        cursor: Some(text.clone()),
                        updated_at: now_iso(),
                    });
                    changed = true;
                }
                (Some(at), None) => {
                    self.doc.crypto_cursors.remove(at);
                    changed = true;
                }
                (None, None) => {}
            }
        }
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    fn crypto_set_cursor(&mut self, account_id: &str, cursor: Option<String>) {
        let id = cursor_id(account_id);
        let updated_at = now_iso();
        match self
            .doc
            .crypto_cursors
            .iter_mut()
            .find(|record| record.id == id)
        {
            Some(record) => {
                record.cursor = cursor;
                record.updated_at = updated_at;
            }
            None => self.doc.crypto_cursors.push(CryptoCursor {
                id,
                source: SOURCE_ID.to_string(),
                account_id: account_id.to_string(),
                cursor,
                updated_at,
            }),
        }
    }

    /// Store pages a source read and move the account's cursor to where the
    /// batch ends, in the same save: an interruption resumes exactly after
    /// what was stored, and a page read twice produces the same operations.
    ///
    /// A failed store puts the cursor back, so the next pass reads the same
    /// pages again.
    pub fn crypto_apply_pages(
        &mut self,
        request: &CryptoPages<'_>,
        batch: &SourceBatch,
    ) -> Result<CryptoPagesApplied> {
        if !self
            .doc
            .crypto_accounts
            .iter()
            .any(|account| account.id == request.account_id)
        {
            return Err(Error::NotFound(format!(
                "crypto account {}",
                request.account_id
            )));
        }
        let before = self.crypto_cursor_of(request.account_id).cloned();
        let next = if request.finished {
            None
        } else {
            batch.next_cursor.clone()
        };
        self.crypto_set_cursor(request.account_id, next.clone());
        let stored = self.crypto_apply_batch(
            &CryptoBatch {
                account_id: request.account_id,
                read_at: request.read_at,
                start: None,
                history_done: request.finished,
                // The node was asked about what the module knows, not about
                // everything the address holds.
                balances_complete: false,
            },
            batch,
        );
        let applied = match stored {
            Ok(applied) => applied,
            Err(err) => {
                let id = cursor_id(request.account_id);
                self.doc.crypto_cursors.retain(|record| record.id != id);
                self.doc.crypto_cursors.extend(before);
                return Err(err);
            }
        };

        // A refresh of a list whose newest page brought nothing new has read
        // all there is: go on to the next list.
        let mut next = next;
        if request.refresh
            && applied.added == 0
            && applied.rewritten > 0
            && let Some(skipped) = next
                .as_deref()
                .and_then(|text| Cursor::decode(Some(text)).ok())
                .filter(|cursor| is_history(cursor.stream))
        {
            next = skipped
                .stream
                .next()
                .map(|stream| Cursor::at(stream, None).encode());
            self.crypto_set_cursor(request.account_id, next.clone());
            self.persist()?;
        }
        Ok(CryptoPagesApplied {
            added: applied.added,
            rewritten: applied.rewritten,
            unread: applied.unread.len(),
            rejected: applied.rejected,
            next_cursor: next,
        })
    }

    /// Forget where an account's read stands, so the next pass starts over
    /// (the user asked to read it again).
    pub fn crypto_reset_cursor(&mut self, account_id: &str) -> Result<()> {
        let id = cursor_id(account_id);
        let before = self.doc.crypto_cursors.len();
        self.doc.crypto_cursors.retain(|record| record.id != id);
        if self.doc.crypto_cursors.len() != before {
            self.persist()?;
        }
        Ok(())
    }

    /// Store the historic prices a pass fetched (spec D7, wave 0), saving
    /// once and only if something changed. Returns how many points are new or
    /// different. `crypto_prices` is a cache that is never thinned, so this is
    /// the only writer it needs.
    pub fn crypto_apply_prices(&mut self, fresh: &[CryptoPricePoint]) -> Result<usize> {
        let before = self.doc.crypto_prices.clone();
        prices::apply_historic(&mut self.doc, fresh);
        let changed = fresh
            .iter()
            .filter(|point| !point.value_usd.is_zero() && !before.contains(point))
            .count();
        if changed > 0 {
            self.persist()?;
        }
        Ok(changed)
    }
}

fn cursor_id(account_id: &str) -> String {
    format!("{SOURCE_ID}|{account_id}")
}

/// The `source` of the records that hold an adapter's memory, apart from the
/// source cursors (their id never starts with `protocol|`).
const PROTOCOL_SOURCE: &str = "protocol";

fn memory_id(account_id: &str, protocol: &str) -> String {
    format!("{PROTOCOL_SOURCE}|{account_id}|{protocol}")
}

/// The lists of events; the held tokens are a state, not a history.
fn is_history(stream: Stream) -> bool {
    !matches!(stream, Stream::Tokens)
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::empty_vault;
    use super::*;
    use crate::cryptotrack::sources::evm::{RawEvent, RawKind, Uint256};
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::CryptoAccount;

    const ME: &str = "0xabcd00000000000000000000000000000000ef01";
    const PEER: &str = "0x1111111111111111111111111111111111111111";
    const NOW: &str = "2026-03-10T00:00:00Z";

    fn account(id: &str, chain: Chain) -> CryptoAccount {
        CryptoAccount::new(id, id, chain, ME, "2026-01-01T00:00:00Z")
    }

    fn page(count: u64, next: Option<String>) -> SourceBatch {
        let raw_events = (0..count)
            .map(|index| {
                let hash = format!("0x{index:064x}");
                RawEvent {
                    id: format!("evm:1:{hash}:tx"),
                    chain_id: 1,
                    hash,
                    kind: RawKind::Transaction,
                    block: 21_000_000 + index,
                    time: "2026-03-01T10:00:00Z".into(),
                    from: PEER.into(),
                    to: ME.into(),
                    asset: RawAsset::Native { decimals: 18 },
                    value: Uint256::from_decimal("1000000000000000000").unwrap(),
                    method: None,
                    succeeded: true,
                    fee: None,
                    payload: "{}".into(),
                }
            })
            .collect();
        SourceBatch {
            raw_events,
            next_cursor: next,
            ..SourceBatch::default()
        }
    }

    fn pages<'run>(finished: bool, refresh: bool) -> CryptoPages<'run> {
        CryptoPages {
            account_id: "acc-1",
            read_at: NOW,
            finished,
            refresh,
        }
    }

    #[test]
    fn only_followed_evm_accounts_are_read() {
        let (_dir, mut vault) = empty_vault();
        let mut archived = account("archived", Chain::Ethereum);
        archived.archived = true;
        let mut exchange = account("exchange", Chain::Ethereum);
        exchange.source = Some("bybit".into());
        vault.doc.crypto_accounts = vec![
            account("acc-1", Chain::Ethereum),
            archived,
            exchange,
            account("bitcoin", Chain::Bitcoin),
        ];
        let targets = vault.crypto_sync_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].account_id, "acc-1");
        assert_eq!(targets[0].address, ME);
        assert!(!targets[0].refresh);
        assert_eq!(targets[0].finished_at_ms, None);
    }

    #[test]
    fn the_cursor_is_stored_with_the_events_and_a_finished_read_is_a_refresh_next_time() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts = vec![account("acc-1", Chain::Ethereum)];
        let next = Cursor::at(Stream::TokenTransfers, None).encode();

        let stored = vault
            .crypto_apply_pages(&pages(false, false), &page(2, Some(next.clone())))
            .unwrap();
        assert_eq!(
            (stored.added, stored.next_cursor.as_deref()),
            (2, Some(next.as_str()))
        );
        let target = &vault.crypto_sync_targets()[0];
        assert_eq!(
            target.cursor.as_deref(),
            Some(next.as_str()),
            "resumes there"
        );
        assert!(!target.refresh);

        vault
            .crypto_apply_pages(&pages(true, false), &page(0, None))
            .unwrap();
        let target = &vault.crypto_sync_targets()[0];
        assert_eq!(target.cursor, None);
        assert!(target.refresh);
        assert!(target.finished_at_ms.is_some());
    }

    #[test]
    fn a_refresh_skips_a_list_that_brings_nothing_new() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts = vec![account("acc-1", Chain::Ethereum)];
        let more = Cursor::at(
            Stream::Transactions,
            Some(
                [("block_number".to_string(), serde_json::json!(5))]
                    .into_iter()
                    .collect(),
            ),
        )
        .encode();
        vault
            .crypto_apply_pages(&pages(false, false), &page(3, Some(more.clone())))
            .unwrap();

        // The same page again, as a refresh of a quiet address sees it.
        let again = vault
            .crypto_apply_pages(&pages(false, true), &page(3, Some(more)))
            .unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(
            again.next_cursor,
            Some(Cursor::at(Stream::TokenTransfers, None).encode()),
            "the next list, not the next page of this one"
        );
        assert_eq!(
            vault.doc.crypto_events.len(),
            3,
            "a refetch adds no duplicates"
        );

        // The same page on a first read is not skipped.
        let (_dir, mut fresh) = empty_vault();
        fresh.doc.crypto_accounts = vec![account("acc-1", Chain::Ethereum)];
        let more = Cursor::at(Stream::Transactions, None).encode();
        fresh
            .crypto_apply_pages(&pages(false, false), &page(3, Some(more.clone())))
            .unwrap();
        let again = fresh
            .crypto_apply_pages(&pages(false, false), &page(3, Some(more.clone())))
            .unwrap();
        assert_eq!(again.next_cursor, Some(more));
    }

    #[test]
    fn an_adapters_memory_is_kept_per_account_and_protocol_and_leaves_the_cursor_alone() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts = vec![
            account("acc-1", Chain::Ethereum),
            account("acc-2", Chain::Ethereum),
        ];
        vault
            .crypto_apply_pages(&pages(true, false), &page(1, None))
            .unwrap();
        let remember = |vault: &mut UnlockedVault, account: &str, text: Option<&str>| {
            vault
                .crypto_remember_protocols(
                    account,
                    &[("uniswap-v4".to_string(), text.map(str::to_string))],
                )
                .unwrap();
        };

        remember(&mut vault, "acc-1", Some("scan-1"));
        remember(&mut vault, "acc-2", Some("scan-2"));
        let targets = vault.crypto_sync_targets();
        assert_eq!(targets[0].protocol_memory["uniswap-v4"], "scan-1");
        assert_eq!(targets[1].protocol_memory["uniswap-v4"], "scan-2");
        assert!(
            targets[0].refresh,
            "the source cursor is still the finished one"
        );
        assert_eq!(targets[0].cursor, None);

        remember(&mut vault, "acc-1", Some("scan-1b"));
        let records = vault.doc.crypto_cursors.len();
        assert_eq!(
            vault.crypto_sync_targets()[0].protocol_memory["uniswap-v4"],
            "scan-1b",
            "an update replaces, it does not add a record"
        );
        remember(&mut vault, "acc-1", None);
        assert!(vault.crypto_sync_targets()[0].protocol_memory.is_empty());
        assert_eq!(vault.doc.crypto_cursors.len(), records - 1);
        assert_eq!(
            vault.crypto_sync_targets()[1].protocol_memory.len(),
            1,
            "another account's memory is not touched"
        );
    }

    #[test]
    fn an_unknown_account_is_refused_without_touching_the_cursors() {
        let (_dir, mut vault) = empty_vault();
        let err = vault
            .crypto_apply_pages(&pages(false, false), &page(1, None))
            .unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
        assert!(vault.doc.crypto_cursors.is_empty());
    }
}

#[cfg(test)]
mod price_tests {
    use super::*;
    use crate::Amount;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoAsset, TokenId};

    #[test]
    fn prices_are_stored_once_and_a_repeat_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("p.wallet"), "pw").unwrap();
        let asset = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xabc"));
        let point = CryptoPricePoint {
            token_key: asset.key(),
            value_usd: Amount::parse("2").unwrap(),
            ts: prices::date_ts(chrono::NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
            source: "defillama".into(),
        };
        assert_eq!(
            vault
                .crypto_apply_prices(std::slice::from_ref(&point))
                .unwrap(),
            1
        );
        assert_eq!(vault.crypto_apply_prices(&[point]).unwrap(), 0);
        assert_eq!(vault.doc.crypto_prices.len(), 1);
    }
}
