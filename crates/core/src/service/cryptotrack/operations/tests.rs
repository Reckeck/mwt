use chrono::NaiveDate;

use crate::cryptotrack::engine::ids;
use crate::cryptotrack::prices;
use crate::ipc_contract::cryptotrack::operations::CryptoOpFilter;
use crate::ipc_contract::cryptotrack::operations::{
    CandidateReason, ChainSide, CryptoListOperationsInput, CryptoOperationDetailInput,
    CryptoOpsPage, CryptoSetOverrideInput, OPERATIONS_PAGE, OverrideField, OwnEntry,
};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAccount, CryptoLeg, CryptoPricePoint, CryptoTxKind, TokenId,
};
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoOverride, CryptoTx, TrackingRef,
};
use crate::model::store::StoreKind;
use crate::service::UnlockedVault;
use crate::service::test_support::{amount, at, empty_vault, payment, store};

const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
const SENDER: &str = "0x9f3c000000000000000000000000000000001a71";

fn event(id: &str, time: &str, direction: CryptoDirection, qty: &str) -> CryptoTx {
    CryptoTx {
        id: id.into(),
        chain: Chain::Ethereum,
        account_id: "acc-1".into(),
        time: time.into(),
        block: Some(1),
        from: SENDER.into(),
        to: "0xbbbb".into(),
        legs: vec![CryptoLeg {
            direction,
            asset: CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDT)),
            amount: amount(qty),
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

fn vault_with(events: Vec<CryptoTx>) -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = empty_vault();
    vault.doc.crypto_accounts.push(CryptoAccount::new(
        "acc-1",
        "Main",
        Chain::Ethereum,
        "0xbbbb",
        "2026-01-01T00:00:00Z",
    ));
    vault.doc.crypto_events = events;
    vault
        .doc
        .crypto_tokens
        .push(crate::model::cryptotrack::CryptoToken::new(
            TokenId::new(Chain::Ethereum, USDT),
            "USDT",
            6,
        ));
    (dir, vault)
}

fn page(vault: &UnlockedVault, filter: CryptoOpFilter) -> CryptoOpsPage {
    crate::usecases::cryptotrack::operations::crypto_list_operations(
        &vault.books(),
        &CryptoListOperationsInput {
            filter,
            ..Default::default()
        },
    )
}

fn pin_usdt_price(vault: &mut UnlockedVault, day: NaiveDate) {
    vault.doc.crypto_prices.push(CryptoPricePoint {
        token_key: CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDT)).key(),
        value_usd: amount("1"),
        ts: prices::date_ts(day),
        source: "test".into(),
    });
}

#[test]
fn the_list_is_newest_first_and_a_page_is_capped() {
    let events = (0..250)
        .map(|index| {
            event(
                &format!("e{index:03}"),
                &format!("2026-03-01T10:{:02}:{:02}Z", index / 60, index % 60),
                CryptoDirection::In,
                "1",
            )
        })
        .collect();
    let (_dir, vault) = vault_with(events);
    let first = page(&vault, CryptoOpFilter::default());
    assert_eq!(first.total, 250);
    assert_eq!(first.items.len(), OPERATIONS_PAGE);
    assert_eq!(first.items[0].id, "e249");
    let second = crate::usecases::cryptotrack::operations::crypto_list_operations(
        &vault.books(),
        &CryptoListOperationsInput {
            offset: 100,
            limit: Some(10_000),
            ..Default::default()
        },
    );
    assert_eq!(second.items.len(), OPERATIONS_PAGE, "the limit is capped");
    assert_eq!(second.items[0].id, "e149");
}

#[test]
fn ten_thousand_operations_page_quickly() {
    let events = (0..10_000)
        .map(|index| {
            event(
                &format!("e{index:05}"),
                &format!(
                    "2026-03-{:02}T{:02}:{:02}:00Z",
                    1 + index % 28,
                    (index / 60) % 24,
                    index % 60
                ),
                CryptoDirection::In,
                "1",
            )
        })
        .collect();
    let (_dir, vault) = vault_with(events);
    let started = std::time::Instant::now();
    let first = page(&vault, CryptoOpFilter::default());
    let filtered = page(
        &vault,
        CryptoOpFilter {
            token: Some("USDT".into()),
            address: Some("9f3c".into()),
            ..Default::default()
        },
    );
    let spent = started.elapsed();
    assert_eq!(first.total, 10_000);
    assert_eq!(filtered.items.len(), OPERATIONS_PAGE);
    assert!(
        spent < std::time::Duration::from_secs(2),
        "two pages of 10 000 operations took {spent:?}"
    );
}

#[test]
fn the_filter_narrows_by_kind_token_address_and_protocol() {
    let mut swap = event("swap", "2026-03-02T10:00:00Z", CryptoDirection::Out, "5");
    swap.kind = CryptoTxKind::Swap;
    swap.protocol = Some("uniswap".into());
    swap.to = "0xrouter".into();
    let plain = event("plain", "2026-03-01T10:00:00Z", CryptoDirection::In, "1");
    let (_dir, vault) = vault_with(vec![swap, plain]);
    let ids = |filter: CryptoOpFilter| -> Vec<String> {
        page(&vault, filter)
            .items
            .into_iter()
            .map(|row| row.id)
            .collect()
    };
    assert_eq!(
        ids(CryptoOpFilter {
            kind: Some(CryptoTxKind::Swap),
            ..Default::default()
        }),
        ["swap"]
    );
    assert_eq!(
        ids(CryptoOpFilter {
            protocol: Some("uniswap".into()),
            ..Default::default()
        }),
        ["swap"]
    );
    assert_eq!(
        ids(CryptoOpFilter {
            address: Some("ROUTER".into()),
            ..Default::default()
        }),
        ["swap"]
    );
    assert_eq!(
        ids(CryptoOpFilter {
            token: Some("usdt".into()),
            ..Default::default()
        })
        .len(),
        2
    );
    assert!(
        ids(CryptoOpFilter {
            token: Some("dai".into()),
            ..Default::default()
        })
        .is_empty()
    );
    let facets = page(&vault, CryptoOpFilter::default()).facets;
    assert_eq!(facets.protocols, ["uniswap"]);
    assert_eq!(facets.chains, [Chain::Ethereum]);
}

#[test]
fn a_coin_on_an_exchange_shows_the_exchange_and_never_the_word_fiat() {
    let mut fill = event("fill", "2026-03-02T10:00:00Z", CryptoDirection::In, "1");
    fill.chain = Chain::Fiat;
    fill.legs[0].asset = CryptoAsset::Token(TokenId::new(Chain::Fiat, "BTC"));
    fill.tracking = TrackingRef::ExchangeHistory {
        exchange: "bybit".into(),
        exec_id: "ex-1".into(),
    };
    let (_dir, vault) = vault_with(vec![fill]);
    let row = &page(&vault, CryptoOpFilter::default()).items[0];
    assert_eq!(row.network, None);
    assert_eq!(row.exchange.as_deref(), Some("bybit"));
    assert_eq!(row.legs[0].symbol, "BTC");
    assert_eq!(row.reference, "ex-1");
    let detail = crate::usecases::cryptotrack::operations::crypto_operation_detail(
        &vault.books(),
        &CryptoOperationDetailInput {
            id: "fill".into(),
            with_candidates: false,
        },
    )
    .unwrap();
    // The site, and the id to look for: no deep link to a fill exists.
    assert_eq!(detail.url.as_deref(), Some("https://www.bybit.com/"));
    assert_eq!(detail.url_note.as_deref(), Some("ex-1"));
}

#[test]
fn an_explorer_of_the_users_own_choosing_builds_the_link() {
    use crate::ipc_contract::cryptotrack::network::SetNetworkSettingsInput;

    let (_dir, mut vault) = vault_with(vec![event(
        "cev:1",
        "2026-11-11T10:00:00Z",
        CryptoDirection::In,
        "500",
    )]);
    vault.raise_to_level3_for_tests();
    fn ask(vault: &UnlockedVault) -> Option<String> {
        crate::usecases::cryptotrack::operations::crypto_operation_detail(
            &vault.books(),
            &CryptoOperationDetailInput {
                id: "cev:1".into(),
                with_candidates: false,
            },
        )
        .unwrap()
        .url
    }
    let preset = ask(&vault).expect("a chain operation has a page");
    assert!(preset.starts_with("https://etherscan.io/"), "{preset}");

    crate::usecases::cryptotrack::network::crypto_set_network_settings(
        &mut vault,
        &SetNetworkSettingsInput {
            chain: Chain::Ethereum,
            rpc: String::new(),
            blockscout: String::new(),
            explorer: "https://eth.example.org".into(),
            blockscout_key: None,
        },
    )
    .unwrap();
    let own = ask(&vault).expect("the link survives a change of explorer");
    assert!(own.starts_with("https://eth.example.org/"), "{own}");
    // The hash is what the page is about, so it must still be in the link.
    assert!(own.contains("cev:1") || own.contains("0x"), "{own}");
}

#[test]
fn a_row_carries_no_address_and_the_detail_only_an_https_one() {
    let (_dir, vault) = vault_with(vec![event(
        "a",
        "2026-03-02T10:00:00Z",
        CryptoDirection::In,
        "1",
    )]);
    let row = page(&vault, CryptoOpFilter::default()).items.remove(0);
    assert!(row.linkable);
    let text = serde_json::to_string(&row).unwrap();
    assert!(
        !text.contains("http"),
        "the list must not name an explorer page: {text}"
    );
    let detail = crate::usecases::cryptotrack::operations::crypto_operation_detail(
        &vault.books(),
        &CryptoOperationDetailInput {
            id: "a".into(),
            with_candidates: false,
        },
    )
    .unwrap();
    assert!(detail.url.as_deref().unwrap().starts_with("https://"));
}

#[test]
fn a_correction_changes_the_attributes_and_nothing_else() {
    let (_dir, mut vault) = vault_with(vec![
        event("a", "2026-03-02T10:00:00Z", CryptoDirection::In, "1"),
        event("b", "2026-03-01T10:00:00Z", CryptoDirection::Out, "2"),
    ]);
    let events_before = vault.doc.crypto_events.clone();
    let txs_before = vault.doc.transactions.len();
    let stores_before = vault.doc.stores.len();
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            kind: Some(CryptoTxKind::Claim),
            comment: Some("  pay  ".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        vault.doc.crypto_events, events_before,
        "an event is never rewritten"
    );
    assert_eq!(
        vault.doc.transactions.len(),
        txs_before,
        "no fiat operation appears"
    );
    assert_eq!(vault.doc.stores.len(), stores_before);
    assert_eq!(vault.doc.crypto_overrides.len(), 1);
    let row = &page(&vault, CryptoOpFilter::default()).items[0];
    assert_eq!(row.kind, CryptoTxKind::Claim);
    assert_eq!(row.source_kind, CryptoTxKind::Transfer);
    assert_eq!(row.comment.as_deref(), Some("pay"));
    assert_eq!(row.legs[0].amount, amount("1"));
    // A refetch rewrites the event; the correction stands.
    vault.doc.crypto_events = events_before;
    assert_eq!(
        page(&vault, CryptoOpFilter::default()).items[0].kind,
        CryptoTxKind::Claim
    );
    // And it goes back to what the source said.
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            clear: vec![OverrideField::Kind, OverrideField::Comment],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        page(&vault, CryptoOpFilter::default()).items[0].kind,
        CryptoTxKind::Transfer
    );
}

#[test]
fn a_hidden_operation_leaves_the_list_until_asked_for() {
    let (_dir, mut vault) = vault_with(vec![event(
        "a",
        "2026-03-02T10:00:00Z",
        CryptoDirection::In,
        "1",
    )]);
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            hidden: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(page(&vault, CryptoOpFilter::default()).items.is_empty());
    let shown = page(
        &vault,
        CryptoOpFilter {
            show_hidden: true,
            ..Default::default()
        },
    );
    assert!(shown.items[0].hidden);
}

#[test]
fn an_unknown_category_contact_or_operation_is_refused() {
    let (_dir, mut vault) = vault_with(vec![event(
        "a",
        "2026-03-02T10:00:00Z",
        CryptoDirection::In,
        "1",
    )]);
    let ask = |input: CryptoSetOverrideInput, vault: &mut UnlockedVault| {
        crate::usecases::cryptotrack::operations::crypto_set_override(vault, &input)
    };
    assert!(
        ask(
            CryptoSetOverrideInput {
                event_id: "zzz".into(),
                ..Default::default()
            },
            &mut vault
        )
        .is_err()
    );
    assert!(
        ask(
            CryptoSetOverrideInput {
                event_id: "a".into(),
                categories: Some(vec!["nope".into()]),
                ..Default::default()
            },
            &mut vault
        )
        .is_err()
    );
    assert!(
        ask(
            CryptoSetOverrideInput {
                event_id: "a".into(),
                contact_id: Some("nope".into()),
                ..Default::default()
            },
            &mut vault
        )
        .is_err()
    );
    assert!(
        ask(
            CryptoSetOverrideInput {
                event_id: "a".into(),
                link_transaction_id: Some("nope".into()),
                ..Default::default()
            },
            &mut vault
        )
        .is_err()
    );
    assert!(
        vault.doc.crypto_overrides.is_empty(),
        "a refused correction writes nothing"
    );
}
/// A pointer at the fiat record (spec E13) says where the other half of
/// the movement is written down. It corrects nothing about the operation,
/// so the row must not wear the «corrected» mark for it alone — while a
/// real correction still does.
#[test]
fn a_pointer_at_a_fiat_record_is_not_a_correction() {
    let one = event(
        "evm:1:0xaa:0",
        "2026-03-01T10:00:00Z",
        CryptoDirection::In,
        "1",
    );
    let (_dir, mut vault) = vault_with(vec![one.clone()]);
    let pointer = CryptoOverride {
        id: format!("cov:{}", one.id),
        event_id: one.id.clone(),
        kind: None,
        comment: None,
        categories: Vec::new(),
        flow_id: None,
        contact_id: None,
        not_exchange: false,
        hidden: false,
        fiat_tx_id: Some("tx-1".into()),
        updated_at: "2026-03-02T00:00:00Z".into(),
    };
    vault.doc.crypto_overrides.push(pointer.clone());
    let row = |vault: &UnlockedVault| page(vault, CryptoOpFilter::default()).items.remove(0);
    assert!(
        !row(&vault).corrected,
        "a pointer alone leaves the operation as the chain reported it"
    );
    vault.doc.crypto_overrides[0].comment = Some("paid for the trip".into());
    assert!(
        row(&vault).corrected,
        "a correction of the operation itself still shows"
    );
}

#[test]
fn the_modules_own_entries_are_listed_marked_and_cannot_be_corrected() {
    let mut opening = event(
        &ids::opening("acc-1", "ethereum|native"),
        "2026-03-01T10:00:00Z",
        CryptoDirection::In,
        "1",
    );
    opening.kind = CryptoTxKind::Unknown;
    opening.tracking = TrackingRef::Address {
        chain: Chain::Ethereum,
        address: "0x00000000000000000000000000000000000000aa".into(),
    };
    let mut reconciled = opening.clone();
    reconciled.id = ids::reconciled("acc-1", "ethereum|native", "2026-03-02T00:00:00Z");
    let (_dir, mut vault) = vault_with(vec![opening.clone(), reconciled.clone()]);
    let rows = page(&vault, CryptoOpFilter::default()).items;
    assert_eq!(rows.len(), 2, "what explains a balance stays visible");
    let own = |id: &str| rows.iter().find(|row| row.id == id).unwrap();
    assert_eq!(own(&opening.id).own_entry, Some(OwnEntry::Opening));
    assert_eq!(
        own(&reconciled.id).own_entry,
        Some(OwnEntry::Reconciliation)
    );
    assert!(
        rows.iter().all(|row| !row.linkable),
        "no explorer link for them"
    );
    let detail = crate::usecases::cryptotrack::operations::crypto_operation_detail(
        &vault.books(),
        &CryptoOperationDetailInput {
            id: opening.id.clone(),
            with_candidates: false,
        },
    )
    .unwrap();
    assert_eq!(detail.url, None);
    assert_eq!(detail.url_note, None);
    assert_eq!(detail.row.own_entry, Some(OwnEntry::Opening));
    assert!(
        crate::usecases::cryptotrack::operations::crypto_set_override(
            &mut vault,
            &CryptoSetOverrideInput {
                event_id: opening.id,
                kind: Some(CryptoTxKind::Claim),
                ..Default::default()
            }
        )
        .is_err(),
        "the kind of an own entry is not corrected"
    );
    assert!(vault.doc.crypto_overrides.is_empty());
    // A contract rule never reclassifies them either.
    assert!(page(&vault, CryptoOpFilter::default()).suggestion.is_none());
}

#[test]
fn declaring_an_exchange_ties_both_sides_by_one_chain_and_can_be_undone() {
    let (_dir, mut vault) = vault_with(vec![event(
        "a",
        "2026-11-11T14:20:00Z",
        CryptoDirection::In,
        "500",
    )]);
    let bank = store(&mut vault, "Bank", StoreKind::Cash, None);
    let mut spend = payment(&bank, "500", "to the exchanger");
    at(&mut spend, "2026-11-11T09:00:00Z");
    let spend = crate::usecases::money::transactions::add_transaction(&mut vault, spend).unwrap();
    let spend_id = spend.meta().id.clone();
    pin_usdt_price(&mut vault, NaiveDate::from_ymd_opt(2026, 11, 11).unwrap());

    // The banner and the card only suggest.
    let shown = page(&vault, CryptoOpFilter::default());
    let suggestion = shown
        .suggestion
        .expect("a sum that fits on the same day is suggested");
    assert_eq!(suggestion.candidate.transaction_id, spend_id);
    assert!(
        suggestion
            .candidate
            .reasons
            .contains(&CandidateReason::SameDay)
    );
    assert!(
        suggestion
            .candidate
            .reasons
            .contains(&CandidateReason::AmountFits)
    );
    assert!(
        vault.doc.crypto_overrides.is_empty()
            && vault
                .doc
                .transactions
                .iter()
                .all(|tx| tx.meta().flow_id.is_none()),
        "the app never links by itself"
    );
    // «Not an exchange» is the user's answer about their own data: it is
    // kept in the correction (so it syncs) and the banner stops asking.
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            not_exchange: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(vault.doc.crypto_overrides[0].not_exchange);
    assert!(
        page(&vault, CryptoOpFilter::default()).suggestion.is_none(),
        "«not an exchange» is respected"
    );
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            not_exchange: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(page(&vault, CryptoOpFilter::default()).suggestion.is_some());

    let result = crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            link_transaction_id: Some(spend_id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let flow = result.flow_id.expect("a chain was made");
    let fiat_flow = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == spend_id)
        .unwrap()
        .meta()
        .flow_id
        .clone();
    assert_eq!(fiat_flow.as_deref(), Some(flow.as_str()));
    let detail = crate::usecases::cryptotrack::operations::crypto_operation_detail(
        &vault.books(),
        &CryptoOperationDetailInput {
            id: "a".into(),
            with_candidates: true,
        },
    )
    .unwrap();
    assert_eq!(detail.chain.len(), 2);
    assert!(detail.basis_from_chain);
    assert!(
        detail
            .chain
            .iter()
            .any(|member| member.is_this && member.side == ChainSide::Crypto)
    );
    assert!(page(&vault, CryptoOpFilter::default()).suggestion.is_none());

    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            clear: vec![OverrideField::Flow],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        vault
            .doc
            .transactions
            .iter()
            .all(|tx| tx.meta().flow_id.is_none()),
        "a chain this link made goes with it"
    );
    assert_eq!(vault.doc.crypto_overrides[0].flow_id, None);
}

#[test]
fn a_contract_rule_classifies_what_arrives_tomorrow_and_can_be_removed() {
    let mut other = event("other", "2026-03-01T10:00:00Z", CryptoDirection::Out, "2");
    other.legs[0].asset = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xaaaa"));
    let (_dir, mut vault) = vault_with(vec![
        event("a", "2026-03-03T10:00:00Z", CryptoDirection::In, "1"),
        event("b", "2026-03-02T10:00:00Z", CryptoDirection::In, "2"),
        other,
    ]);
    let detail = |vault: &UnlockedVault| {
        crate::usecases::cryptotrack::operations::crypto_operation_detail(
            &vault.books(),
            &CryptoOperationDetailInput {
                id: "a".into(),
                with_candidates: false,
            },
        )
        .unwrap()
    };
    assert_eq!(detail(&vault).same_contract_count, 2);
    assert!(detail(&vault).contract_rule.is_none());
    let result = crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            kind: Some(CryptoTxKind::Claim),
            comment: Some("only here".into()),
            same_contract: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.applied, 2);
    let rows = page(&vault, CryptoOpFilter::default()).items;
    let find = |id: &str| rows.iter().find(|row| row.id == id).unwrap().clone();
    assert_eq!(find("b").kind, CryptoTxKind::Claim);
    assert!(find("b").by_rule);
    assert_eq!(find("b").comment, None, "a comment is never a rule");
    assert_eq!(find("a").comment.as_deref(), Some("only here"));
    assert_eq!(find("other").kind, CryptoTxKind::Transfer);
    assert_eq!(
        detail(&vault).contract_rule.map(|rule| rule.kind),
        Some(Some(CryptoTxKind::Claim)),
        "the rule is visible"
    );
    // The next refetch brings an operation of the same contract: it is
    // classified without anyone touching it, and the filter agrees.
    vault.doc.crypto_events.push(event(
        "tomorrow",
        "2026-03-04T10:00:00Z",
        CryptoDirection::In,
        "3",
    ));
    let rows = page(&vault, CryptoOpFilter::default()).items;
    let tomorrow = rows.iter().find(|row| row.id == "tomorrow").unwrap();
    assert_eq!(tomorrow.kind, CryptoTxKind::Claim);
    assert_eq!(tomorrow.source_kind, CryptoTxKind::Transfer);
    assert!(tomorrow.by_rule && !tomorrow.corrected);
    let claims = page(
        &vault,
        CryptoOpFilter {
            kind: Some(CryptoTxKind::Claim),
            ..Default::default()
        },
    );
    assert_eq!(claims.total, 3);
    // An operation's own correction stands above the rule.
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "tomorrow".into(),
            kind: Some(CryptoTxKind::Stake),
            ..Default::default()
        },
    )
    .unwrap();
    let rows = page(&vault, CryptoOpFilter::default()).items;
    assert_eq!(
        rows.iter().find(|row| row.id == "tomorrow").unwrap().kind,
        CryptoTxKind::Stake
    );
    // Removing the rule takes the decision off everything it reached.
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "b".into(),
            remove_contract_rule: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(detail(&vault).contract_rule.is_none());
    let rows = page(&vault, CryptoOpFilter::default()).items;
    let find = |id: &str| rows.iter().find(|row| row.id == id).unwrap().clone();
    assert_eq!(find("b").kind, CryptoTxKind::Transfer);
    assert_eq!(
        find("tomorrow").kind,
        CryptoTxKind::Stake,
        "its own correction stays"
    );
}

#[test]
fn a_counterparty_is_named_by_address_and_remembered_when_chosen() {
    use crate::model::contact::{Contact, ContactKind};
    let (_dir, mut vault) = vault_with(vec![event(
        "a",
        "2026-03-02T10:00:00Z",
        CryptoDirection::In,
        "1",
    )]);
    vault.doc.contacts.push(Contact {
        id: "c1".into(),
        name: "CoinRush".into(),
        kind: ContactKind::Exchanger,
        ..Default::default()
    });
    assert_eq!(
        page(&vault, CryptoOpFilter::default()).items[0].contact_name,
        None
    );
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            contact_id: Some("c1".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let saved = &vault.doc.contacts[0].accounts;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].value, SENDER);
    // The same address on another operation is now named by itself.
    let mut next = event("b", "2026-03-03T10:00:00Z", CryptoDirection::In, "1");
    next.from = SENDER.to_uppercase().replace("0X", "0x");
    vault.doc.crypto_events.push(next);
    let rows = page(&vault, CryptoOpFilter::default()).items;
    let guessed = rows.iter().find(|row| row.id == "b").unwrap();
    assert_eq!(guessed.contact_name.as_deref(), Some("CoinRush"));
    assert!(guessed.contact_guessed);
    assert_eq!(guessed.contact_id, None, "a guess is shown, never stored");
}

/// The module only reads from its sources: no command of the operations
/// creates or deletes one, here or anywhere in the application's list.
#[test]
fn no_command_creates_or_deletes_an_operation() {
    let registered = include_str!("../../../../../app/src/lib.rs");
    let own = include_str!("../../../../../app/src/commands/cryptotrack/operations.rs");
    let forbidden = [
        "create", "delete", "remove", "add", "new", "insert", "import",
    ];
    for name in registered
        .lines()
        .filter_map(|line| line.trim().strip_prefix("commands::cryptotrack::"))
        .filter_map(|line| line.trim_end_matches(',').rsplit("::").next())
        .chain(
            own.lines()
                .filter_map(|line| line.strip_prefix("pub async fn "))
                .filter_map(|line| line.split('(').next()),
        )
    {
        let about_operations =
            name.contains("operation") || name.contains("override") || name.contains("event");
        if about_operations {
            assert!(
                !forbidden
                    .iter()
                    .any(|word| name.split('_').any(|part| part == *word)),
                "`{name}` looks like it creates or deletes an operation"
            );
        }
    }
    // The surface of the operations scenarios: reads, and the one correction.
    let service = include_str!("../../../usecases/cryptotrack/operations.rs");
    let body = service.split("#[cfg(test)]").next().unwrap();
    for line in body.lines().filter(|line| line.contains("pub fn ")) {
        assert!(
            line.contains("crypto_list_operations")
                || line.contains("crypto_operation_detail")
                || line.contains("crypto_set_override"),
            "an unexpected public function in the operations service: {line}"
        );
    }
    assert!(
        !body.contains("crypto_events.push")
            && !body.contains("crypto_events.remove")
            && !body.contains("crypto_events.retain")
    );
}

struct NoWake;

impl crate::service::SaveHook for NoWake {
    fn dirty(&self) {}
}

fn flush(vault: &mut UnlockedVault) {
    if let Some(job) = vault.prepare_flush().unwrap() {
        let done = job.write().unwrap();
        vault.finish_flush(&done);
    }
}

#[test]
fn clearing_a_contract_rule_leaves_a_tombstone() {
    let (_dir, mut vault) = vault_with(vec![event(
        "a",
        "2026-03-03T10:00:00Z",
        CryptoDirection::In,
        "1",
    )]);
    vault.set_device("aaaaaaaa-rule");
    vault.set_write_behind(Some(std::sync::Arc::new(NoWake)));
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            kind: Some(CryptoTxKind::Claim),
            same_contract: true,
            ..Default::default()
        },
    )
    .unwrap();
    flush(&mut vault);
    let rule = vault
        .doc
        .crypto_overrides
        .iter()
        .find(|fix| fix.id.starts_with("crule:"))
        .expect("the rule is stored")
        .id
        .clone();
    crate::usecases::cryptotrack::operations::crypto_set_override(
        &mut vault,
        &CryptoSetOverrideInput {
            event_id: "a".into(),
            remove_contract_rule: true,
            ..Default::default()
        },
    )
    .unwrap();
    flush(&mut vault);
    assert!(
        vault
            .doc
            .sync
            .tombstones
            .keys()
            .any(|key| key.contains(&rule)),
        "removing the rule is a deletion the sync must carry"
    );
}
