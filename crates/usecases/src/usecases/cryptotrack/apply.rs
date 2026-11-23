//! Scenarios of the background tracking (spec E6): the read targets, the pages of a
//! read, the adapters' memory, the cursors and the fetched prices. The rules
//! (what a target is, how a cursor moves) are in `cryptotrack::service::apply`.
//!
//! Pages are written in batches, not one by one: a save encrypts the whole
//! document, so a long history is cheaper to store a few hundred events at a
//! time than fifty. Every scenario is one pass over the document and ends in
//! one `transact` (`touch_if(changed)`), however many cursors, tokens and
//! operations the pass met.

use crate::cryptotrack::service::apply as rules;
use crate::cryptotrack::service::recompute::CryptoBatch;
use crate::cryptotrack::sources::evm::{RawAsset, SOURCE_ID, SourceBatch};
use crate::cryptotrack::{chains, tokens};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::sync::{CryptoPages, CryptoPagesApplied, CryptoSyncTarget};
use crate::model::cryptotrack::{CryptoPricePoint, normalize_address};
use crate::model::vault::{VaultDoc, iso_to_millis, now_iso};
use crate::service::Session;

use super::recompute::apply_batch_unsaved;

/// The accounts a pass of the tracking reads. An archived account is
/// watched but not followed, and an exchange account is another spec's
/// source (E7); only the EVM networks have a source here.
pub fn crypto_sync_targets(doc: &VaultDoc) -> Vec<CryptoSyncTarget> {
    doc.crypto_accounts
        .iter()
        .filter(|account| !account.archived)
        .filter(|account| matches!(account.source.as_deref(), None | Some(SOURCE_ID)))
        .filter(|account| chains::evm_chain(account.chain).is_some())
        .map(|account| {
            let record = rules::cursor_of(doc, &account.id);
            let held = doc
                .crypto_tokens
                .iter()
                .filter(|token| token.chain == account.chain)
                .filter(|token| tokens::counts_in_totals(doc, &token.id()))
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
                protocol_memory: rules::protocol_memory(doc, &account.id),
            }
        })
        .collect()
}

/// Keep what the protocol adapters want to remember about an account
/// (`None` forgets), beside the source cursors and as local as they are:
/// a block a scan has reached names this device's own reading. Saves
/// only when something differs.
pub fn crypto_remember_protocols(
    vault: &mut Session,
    account_id: &str,
    memory: &[(String, Option<String>)],
) -> Result<()> {
    let changed = rules::remember_protocols(&mut vault.doc, account_id, memory, &now_iso());
    vault.transact(|tx| {
        tx.touch_if(changed);
        Ok(())
    })
}

/// Store pages a source read and move the account's cursor to where the
/// batch ends, in the same save: an interruption resumes exactly after
/// what was stored, and a page read twice produces the same operations.
///
/// A failed store puts the cursor back, so the next pass reads the same
/// pages again.
pub fn crypto_apply_pages(
    vault: &mut Session,
    request: &CryptoPages<'_>,
    batch: &SourceBatch,
) -> Result<CryptoPagesApplied> {
    if !rules::has_account(&vault.doc, request.account_id) {
        return Err(Error::NotFound(format!(
            "crypto account {}",
            request.account_id
        )));
    }
    let now = now_iso();
    let before = rules::cursor_of(&vault.doc, request.account_id).cloned();
    let next = if request.finished {
        None
    } else {
        batch.next_cursor.clone()
    };
    let mut changed = rules::set_cursor(&mut vault.doc, request.account_id, next.clone(), &now);
    let stored = apply_batch_unsaved(
        vault,
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
        Ok((applied, batch_changed)) => {
            changed |= batch_changed;
            applied
        }
        Err(err) => {
            rules::restore_cursor(&mut vault.doc, request.account_id, before);
            return Err(err);
        }
    };

    // A refresh of a list whose newest page brought nothing new has read
    // all there is: go on to the next list.
    let mut next = next;
    if request.refresh
        && applied.added == 0
        && applied.rewritten > 0
        && let Some(skipped) = rules::next_list_after(next.as_deref())
    {
        next = skipped;
        changed |= rules::set_cursor(&mut vault.doc, request.account_id, next.clone(), &now);
    }
    // One save for the whole pass: the batch and both cursor moves.
    vault.transact(|tx| {
        tx.touch_if(changed);
        Ok(())
    })?;
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
pub fn crypto_reset_cursor(vault: &mut Session, account_id: &str) -> Result<()> {
    let changed = rules::reset_cursor(&mut vault.doc, account_id);
    vault.transact(|tx| {
        tx.touch_if(changed);
        Ok(())
    })
}

/// Store the historic prices a pass fetched (spec D7, wave 0), saving
/// once and only if something changed. Returns how many points are new or
/// different.
pub fn crypto_apply_prices(vault: &mut Session, fresh: &[CryptoPricePoint]) -> Result<usize> {
    let changed = rules::store_prices(&mut vault.doc, fresh);
    vault.transact(|tx| {
        tx.touch_if(changed > 0);
        Ok(changed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cryptotrack::sources::evm::{Cursor, RawAsset, RawEvent, RawKind, Stream, Uint256};
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::CryptoAccount;
    use crate::service::test_support::empty_vault;

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
        let targets = crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc);
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

        let stored = crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut vault,
            &pages(false, false),
            &page(2, Some(next.clone())),
        )
        .unwrap();
        assert_eq!(
            (stored.added, stored.next_cursor.as_deref()),
            (2, Some(next.as_str()))
        );
        let target = &crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc)[0];
        assert_eq!(
            target.cursor.as_deref(),
            Some(next.as_str()),
            "resumes there"
        );
        assert!(!target.refresh);

        crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut vault,
            &pages(true, false),
            &page(0, None),
        )
        .unwrap();
        let target = &crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc)[0];
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
        crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut vault,
            &pages(false, false),
            &page(3, Some(more.clone())),
        )
        .unwrap();

        // The same page again, as a refresh of a quiet address sees it.
        let again = crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut vault,
            &pages(false, true),
            &page(3, Some(more)),
        )
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
        crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut fresh,
            &pages(false, false),
            &page(3, Some(more.clone())),
        )
        .unwrap();
        let again = crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut fresh,
            &pages(false, false),
            &page(3, Some(more.clone())),
        )
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
        crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut vault,
            &pages(true, false),
            &page(1, None),
        )
        .unwrap();
        let remember = |vault: &mut Session, account: &str, text: Option<&str>| {
            crate::usecases::cryptotrack::apply::crypto_remember_protocols(
                vault,
                account,
                &[("uniswap-v4".to_string(), text.map(str::to_string))],
            )
            .unwrap();
        };

        remember(&mut vault, "acc-1", Some("scan-1"));
        remember(&mut vault, "acc-2", Some("scan-2"));
        let targets = crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc);
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
            crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc)[0].protocol_memory
                ["uniswap-v4"],
            "scan-1b",
            "an update replaces, it does not add a record"
        );
        remember(&mut vault, "acc-1", None);
        assert!(
            crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc)[0]
                .protocol_memory
                .is_empty()
        );
        assert_eq!(vault.doc.crypto_cursors.len(), records - 1);
        assert_eq!(
            crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc)[1]
                .protocol_memory
                .len(),
            1,
            "another account's memory is not touched"
        );
    }

    #[test]
    fn an_unknown_account_is_refused_without_touching_the_cursors() {
        let (_dir, mut vault) = empty_vault();
        let err = crate::usecases::cryptotrack::apply::crypto_apply_pages(
            &mut vault,
            &pages(false, false),
            &page(1, None),
        )
        .unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
        assert!(vault.doc.crypto_cursors.is_empty());
    }
}

#[cfg(test)]
mod price_tests {
    use super::*;
    use crate::Amount;
    use crate::cryptotrack::prices;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoAsset, TokenId};

    #[test]
    fn prices_are_stored_once_and_a_repeat_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = Session::create(dir.path().join("p.wallet"), "pw").unwrap();
        let asset = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xabc"));
        let point = CryptoPricePoint {
            token_key: asset.key(),
            value_usd: Amount::parse("2").unwrap(),
            ts: prices::date_ts(chrono::NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
            source: "defillama".into(),
        };
        assert_eq!(
            crate::usecases::cryptotrack::apply::crypto_apply_prices(
                &mut vault,
                std::slice::from_ref(&point)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            crate::usecases::cryptotrack::apply::crypto_apply_prices(&mut vault, &[point]).unwrap(),
            0
        );
        assert_eq!(vault.doc.crypto_prices.len(), 1);
    }
}
