//! Scenarios of the links between a fiat record and a crypto operation (spec
//! E13): the suggestions, the pointers, and setting or clearing one. The rules
//! are in `cryptotrack::service::links`; a pointer change is one
//! `vault.transact` over `tx.collection::<CryptoOverride>()`, and nothing
//! recomputes the holdings.

use crate::cryptotrack::engine::overrides;
use crate::cryptotrack::service::links::{self as rules, LinkPlan};
use crate::error::Result;
use crate::ipc_contract::cryptotrack::links::{
    CryptoLinkCandidatesInput, CryptoLinkInput, CryptoLinkOption, CryptoPair, CryptoPairFiat,
    CryptoPairsInput, CryptoUnlinkInput,
};
use crate::ipc_contract::cryptotrack::operations::{CryptoOpRow, CryptoOperationDetailInput};
use crate::model::cryptotrack::CryptoOverride;
use crate::model::transaction::LegRole;
use crate::model::vault::{VaultDoc, now_iso};
use crate::money::books::Books;
use crate::money::rates::convert_at;
use crate::service::Session;
use crate::usecases::cryptotrack::operations::crypto_operation_detail;

/// Crypto operations that look like the same movement of money as a fiat
/// record: close in time, the ones whose value fits first. Only a
/// suggestion, nothing is written.
pub fn crypto_link_candidates(
    books: &Books<'_>,
    input: &CryptoLinkCandidatesInput,
) -> Vec<CryptoLinkOption> {
    let Some(day) = rules::day_of(&input.time) else {
        return Vec::new();
    };
    let mut options: Vec<CryptoLinkOption> = rules::nearby_operations(books.doc(), day)
        .into_iter()
        .filter_map(|(days, id)| {
            let row = crypto_row_of(books, id)?;
            let fits = value_fits(books, input, &row);
            Some(CryptoLinkOption { row, days, fits })
        })
        .collect();
    options.sort_by(|left, right| {
        right
            .fits
            .cmp(&left.fits)
            .then(left.days.cmp(&right.days))
            .then(right.row.time.cmp(&left.row.time))
            .then(left.row.id.cmp(&right.row.id))
    });
    options.truncate(rules::OPTION_CAP);
    options
}

/// The row of one operation, as the list shows it. The detail is built by the
/// operations area, which has not moved yet: one `Books::vault()` call.
fn crypto_row_of(books: &Books<'_>, event_id: &str) -> Option<CryptoOpRow> {
    crypto_operation_detail(
        books,
        &CryptoOperationDetailInput {
            id: event_id.to_string(),
            with_candidates: false,
        },
    )
    .ok()
    .map(|detail| detail.row)
}

fn value_fits(books: &Books<'_>, input: &CryptoLinkCandidatesInput, row: &CryptoOpRow) -> bool {
    let (Some(amount), Some(currency), Some(value)) =
        (input.amount, input.currency.as_deref(), row.value_usd)
    else {
        return false;
    };
    if value.is_zero() {
        return false;
    }
    convert_at(books, amount.abs(), currency, "usd", &input.time)
        .is_some_and(|usd| rules::within_tolerance(usd, value))
}

/// Point a crypto operation at a fiat record. Both must exist now.
pub fn crypto_link_fiat(vault: &mut Session, input: &CryptoLinkInput) -> Result<()> {
    let now = now_iso();
    match rules::plan_link(&vault.doc, &input.event_id, &input.fiat_tx_id, &now)? {
        LinkPlan::Unchanged => Ok(()),
        LinkPlan::Insert(fix) => vault.transact(|tx| tx.collection::<CryptoOverride>().insert(fix)),
        LinkPlan::Repoint(id) => vault.transact(|tx| {
            tx.collection::<CryptoOverride>().update(&id, |fix| {
                fix.fiat_tx_id = Some(input.fiat_tx_id.clone());
                fix.updated_at = now;
            })
        }),
    }
}

/// Remove the pointer. Nothing to remove is not an error.
pub fn crypto_unlink_fiat(vault: &mut Session, input: &CryptoUnlinkInput) -> Result<()> {
    let Some(id) = rules::pointer_of(&vault.doc, &input.event_id) else {
        return Ok(());
    };
    vault.transact(|tx| {
        tx.collection::<CryptoOverride>().update(&id, |fix| {
            fix.fiat_tx_id = None;
            fix.updated_at = now_iso();
        })
    })
}

/// The pointers of one operation or one fiat record, both ends resolved.
/// A vault without crypto data has none, so the fiat form shows nothing.
pub fn crypto_pairs(books: &Books<'_>, input: &CryptoPairsInput) -> Vec<CryptoPair> {
    let doc = books.doc();
    overrides::index(&doc.crypto_overrides)
        .values()
        .filter_map(|fix| {
            let fiat_id = fix.fiat_tx_id.as_deref()?;
            if input
                .event_id
                .as_deref()
                .is_some_and(|id| id != fix.event_id)
                || input.fiat_tx_id.as_deref().is_some_and(|id| id != fiat_id)
            {
                return None;
            }
            Some(CryptoPair {
                event_id: fix.event_id.clone(),
                fiat_tx_id: fiat_id.to_string(),
                event: crypto_row_of(books, &fix.event_id),
                fiat: pair_fiat(doc, fiat_id),
            })
        })
        .collect()
}

fn pair_fiat(doc: &VaultDoc, id: &str) -> Option<CryptoPairFiat> {
    let tx = doc.transactions.iter().find(|tx| tx.meta().id == id)?;
    let legs = tx.legs();
    let leg = legs
        .iter()
        .find(|leg| leg.role == LegRole::Principal)
        .or_else(|| legs.first())?;
    let meta = tx.meta();
    Some(CryptoPairFiat {
        id: meta.id.clone(),
        place: doc
            .stores
            .iter()
            .find(|store| store.id == leg.store_id)
            .map(|store| store.name.clone())
            .unwrap_or_default(),
        amount: leg.amount,
        currency: leg.asset.symbol.clone(),
        time: meta.timestamp.clone(),
        comment: meta.comment.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg, CryptoToken, CryptoTx,
        CryptoTxKind, TokenId, TrackingRef,
    };
    use crate::model::store::StoreKind;
    use crate::service::test_support::{amount, at, empty_vault, payment, store};

    const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";

    fn event(id: &str, time: &str) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: time.into(),
            block: Some(1),
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDT)),
                amount: amount("500"),
                counterparty: None,
            }],
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{}", "a".repeat(64)),
            },
        }
    }

    /// A vault with two operations and one fiat payment near them.
    fn setup() -> (tempfile::TempDir, Session, String) {
        let (dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts.push(CryptoAccount::new(
            "acc-1",
            "Main",
            Chain::Ethereum,
            "0xbbbb",
            "2026-01-01T00:00:00Z",
        ));
        vault.doc.crypto_tokens.push(CryptoToken::new(
            TokenId::new(Chain::Ethereum, USDT),
            "USDT",
            6,
        ));
        vault.doc.crypto_events = vec![
            event("near", "2026-11-11T14:20:00Z"),
            event("far", "2026-10-01T10:00:00Z"),
        ];
        let bank = store(&mut vault, "Bank", StoreKind::Cash, None);
        let mut spend = payment(&bank, "500", "to the exchanger");
        at(&mut spend, "2026-11-11T09:00:00Z");
        let id = crate::usecases::money::transactions::add_transaction(&mut vault, spend)
            .unwrap()
            .meta()
            .id
            .clone();
        (dir, vault, id)
    }

    fn link(vault: &mut Session, event_id: &str, fiat_tx_id: &str) {
        crate::usecases::cryptotrack::links::crypto_link_fiat(
            vault,
            &CryptoLinkInput {
                event_id: event_id.into(),
                fiat_tx_id: fiat_tx_id.into(),
            },
        )
        .unwrap();
    }

    #[test]
    fn candidates_are_the_operations_close_in_time_and_write_nothing() {
        let (_dir, vault, _id) = setup();
        let before = vault.doc.crypto_overrides.clone();
        let options = crate::usecases::cryptotrack::links::crypto_link_candidates(
            &vault.books(),
            &CryptoLinkCandidatesInput {
                time: "2026-11-11T09:00:00Z".into(),
                ..Default::default()
            },
        );
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].row.id, "near");
        assert_eq!(options[0].days, 0);
        assert_eq!(
            vault.doc.crypto_overrides, before,
            "a suggestion writes nothing"
        );
    }

    #[test]
    fn a_pointer_is_visible_from_both_sides_and_can_be_removed() {
        let (_dir, mut vault, id) = setup();
        link(&mut vault, "near", &id);
        for input in [
            CryptoPairsInput {
                event_id: Some("near".into()),
                ..Default::default()
            },
            CryptoPairsInput {
                fiat_tx_id: Some(id.clone()),
                ..Default::default()
            },
        ] {
            let pairs = crate::usecases::cryptotrack::links::crypto_pairs(&vault.books(), &input);
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].event.as_ref().unwrap().id, "near");
            assert_eq!(pairs[0].fiat.as_ref().unwrap().id, id);
        }
        crate::usecases::cryptotrack::links::crypto_unlink_fiat(
            &mut vault,
            &CryptoUnlinkInput {
                event_id: "near".into(),
            },
        )
        .unwrap();
        assert!(
            crate::usecases::cryptotrack::links::crypto_pairs(
                &vault.books(),
                &CryptoPairsInput::default()
            )
            .is_empty()
        );
        // Removing what is not there is not an error.
        crate::usecases::cryptotrack::links::crypto_unlink_fiat(
            &mut vault,
            &CryptoUnlinkInput {
                event_id: "near".into(),
            },
        )
        .unwrap();
    }

    #[test]
    fn a_missing_target_is_a_calm_empty_end_not_an_error() {
        let (_dir, mut vault, id) = setup();
        link(&mut vault, "near", &id);
        crate::usecases::money::transactions::delete_transaction(&mut vault, &id).unwrap();
        let pairs = crate::usecases::cryptotrack::links::crypto_pairs(
            &vault.books(),
            &CryptoPairsInput::default(),
        );
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].fiat.is_none());
        assert!(pairs[0].event.is_some());
        vault.doc.crypto_events.clear();
        let pairs = crate::usecases::cryptotrack::links::crypto_pairs(
            &vault.books(),
            &CryptoPairsInput::default(),
        );
        assert!(pairs[0].event.is_none() && pairs[0].fiat.is_none());
        // A pointer cannot be made to something that is not there.
        assert!(
            crate::usecases::cryptotrack::links::crypto_link_fiat(
                &mut vault,
                &CryptoLinkInput {
                    event_id: "near".into(),
                    fiat_tx_id: id,
                }
            )
            .is_err()
        );
    }

    #[test]
    fn a_pointer_changes_no_amount_no_total_and_no_chain() {
        let (_dir, mut vault, id) = setup();
        let at_time = "2026-11-12T00:00:00Z";
        let snapshot = |vault: &Session| {
            (
                format!(
                    "{:?}",
                    crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), at_time)
                ),
                serde_json::to_string(&crate::usecases::money::capital::portfolio_summary(
                    &vault.books(),
                ))
                .unwrap(),
                // Holding ids are fresh on every recompute; what they hold is not.
                vault
                    .doc
                    .holdings
                    .iter()
                    .map(|held| {
                        format!(
                            "{}|{}|{}|{:?}",
                            held.store_id, held.asset.symbol, held.quantity, held.avg_cost
                        )
                    })
                    .collect::<Vec<_>>(),
                serde_json::to_string(&vault.doc.transactions).unwrap(),
                serde_json::to_string(&vault.doc.crypto_events).unwrap(),
            )
        };
        let without = snapshot(&vault);
        link(&mut vault, "near", &id);
        assert_eq!(snapshot(&vault), without);
        let fix = &vault.doc.crypto_overrides[0];
        assert_eq!(fix.flow_id, None, "a pointer joins no chain");
        assert!(vault.doc.transactions[0].meta().flow_id.is_none());
        // The same after a recompute from scratch.
        crate::usecases::cryptotrack::recompute::crypto_sync_holdings(&mut vault).unwrap();
        assert_eq!(snapshot(&vault), without);
    }

    /// The pointer must stay in a gated, wiped collection: if someone moves it
    /// onto a fiat record it would travel past the level-3 gate.
    #[test]
    fn the_pointer_only_lives_in_a_gated_collection() {
        let doc = serde_json::to_value(crate::model::fixtures::full_doc()).unwrap();
        let mut holders = Vec::new();
        for (name, value) in doc.as_object().unwrap() {
            if value.to_string().contains("\"fiat_tx_id\"") {
                holders.push(name.clone());
            }
        }
        assert_eq!(holders, vec!["crypto_overrides".to_string()]);
        for name in holders {
            assert!(crate::sync::crypto_gate::is_crypto_collection(&name));
        }
    }

    /// A vault written before `CryptoOverride::fiat_tx_id` opens in both
    /// payload formats (JSON, and MessagePack with field names), keeps its
    /// correction, and the pointer reads as unset. The field is additive, so
    /// no format bump is needed (rule 5).
    #[test]
    fn a_vault_written_before_the_field_opens_in_json_and_pack() {
        use crate::model::vault::VaultDoc;
        let doc = crate::model::fixtures::full_doc();
        assert!(
            doc.crypto_overrides[0].fiat_tx_id.is_some(),
            "the fixture fills it"
        );
        let mut json: serde_json::Value = serde_json::to_value(&doc).unwrap();
        for fix in json["crypto_overrides"].as_array_mut().unwrap() {
            fix.as_object_mut().unwrap().remove("fiat_tx_id");
        }
        let from_json: VaultDoc =
            serde_json::from_slice(&serde_json::to_vec(&json).unwrap()).unwrap();
        let from_pack: VaultDoc =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&json).unwrap()).unwrap();
        for out in [from_json, from_pack] {
            assert_eq!(out.crypto_overrides.len(), doc.crypto_overrides.len());
            assert_eq!(out.crypto_overrides[0].fiat_tx_id, None);
            assert_eq!(
                out.crypto_overrides[0].flow_id,
                doc.crypto_overrides[0].flow_id
            );
            assert_eq!(out.contacts, doc.contacts);
        }
    }
}
