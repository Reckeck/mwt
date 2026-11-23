//! Scenarios of the operations as the screens read them, and the correction the
//! user lays over one (spec E10): the rows, the page with its facets, the
//! candidates that could have paid for an operation, the detail, and the
//! correction itself. What is read off the document alone (a coin's symbol, an
//! account's name, the contact behind an address, joining a chain) is in
//! `cryptotrack::service::operations`.
//!
//! **Nothing here creates or deletes an operation.** The one write in this file
//! is a [`CryptoOverride`], which carries classification and attributes (kind,
//! comment, categories, contact, hidden, chain) and never an amount; it ends in
//! one save that re-derives the holdings.
//!
//! Reading builds no URL: a row only says whether a link exists (`linkable`).
//! The address of the page is made in the detail, when the user opens the
//! operation, and the frontend opens it only on a click.

use std::collections::{BTreeMap, BTreeSet};

use crate::Amount;
use crate::cryptotrack::engine::{ids, math, overrides};
use crate::cryptotrack::prices;
use crate::cryptotrack::service::network::crypto_explorer_settings;
use crate::cryptotrack::service::operations::{
    crypto_account_name, crypto_contact_by_address, crypto_event_ref, crypto_join_chain,
    crypto_release_chain, crypto_remember_address, crypto_signed_value, crypto_symbol, day_of,
    other_side, rule_for, secs, short,
};
use crate::cryptotrack::tracking::{ExplorerSettings, TrackingRefExt};
use crate::cryptotrack::validate::Validate;
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::operations::{
    CandidateReason, ChainSide, CryptoChainMember, CryptoContractRule, CryptoExchangeSuggestion,
    CryptoLinkCandidate, CryptoListOperationsInput, CryptoOpFacets, CryptoOpFilter, CryptoOpLeg,
    CryptoOpRow, CryptoOpTokenFacet, CryptoOperationDetail, CryptoOperationDetailInput,
    CryptoOpsPage, CryptoSetOverrideInput, CryptoSetOverrideResult, OPERATIONS_PAGE, OverrideField,
    OwnEntry,
};
use crate::kernel::tx::Tx;
use crate::model::cryptotrack::{CryptoDirection, CryptoOverride, CryptoTx, TrackingRef};
use crate::model::transaction::LegRole;
use crate::model::vault::{VaultDoc, now_iso};
use crate::money::books::Books;
use crate::money::rates::convert_at;
use crate::service::UnlockedVault;
use crate::usecases::cryptotrack::recompute::refold_holdings;

/// How far apart the two sides of an exchange may be dated, in days.
const CANDIDATE_DAYS: i64 = 3;
/// How far the fiat sum may be from the coin's market value, as a share.
const AMOUNT_TOLERANCE_PERCENT: i64 = 5;
/// The most candidates a card lists.
const CANDIDATE_CAP: usize = 40;
/// The most recent incoming operations the banner looks at.
const SUGGEST_SCAN: usize = 40;
/// A comment longer than this is refused.
const COMMENT_MAX: usize = 1000;

fn crypto_row(
    doc: &VaultDoc,
    event: &CryptoTx,
    fix: Option<&CryptoOverride>,
    rule: Option<&CryptoOverride>,
    links: &ExplorerSettings,
) -> CryptoOpRow {
    let own_entry = ids::adjustment(&event.id).map(|kind| match kind {
        ids::Adjustment::Opening => OwnEntry::Opening,
        ids::Adjustment::Reconciled => OwnEntry::Reconciliation,
    });
    let by_rule = fix.is_none_or(|fix| {
        fix.kind.is_none() && fix.categories.is_empty() && fix.contact_id.is_none()
    }) && rule.is_some_and(|rule| {
        rule.kind.is_some() || !rule.categories.is_empty() || rule.contact_id.is_some()
    });
    let exchange = match &event.tracking {
        TrackingRef::ExchangeHistory { exchange, .. } => Some(exchange.clone()),
        _ => None,
    };
    let reference = match &event.tracking {
        TrackingRef::ChainTx { hash, .. } => short(hash),
        TrackingRef::ExchangeHistory { exec_id, .. } => exec_id.clone(),
        _ => String::new(),
    };
    let contact_id = fix
        .and_then(|fix| fix.contact_id.clone())
        .or_else(|| rule.and_then(|rule| rule.contact_id.clone()));
    let guessed = if contact_id.is_none() {
        crypto_contact_by_address(doc, event.chain, other_side(event)).map(str::to_string)
    } else {
        None
    };
    let contact_guessed = guessed.is_some();
    let shown_contact = contact_id.clone().or(guessed);
    let contact_name = shown_contact.as_deref().and_then(|id| {
        doc.contacts
            .iter()
            .find(|contact| contact.id == id)
            .map(|contact| contact.name.clone())
    });
    CryptoOpRow {
        id: event.id.clone(),
        account_id: event.account_id.clone(),
        account_name: crypto_account_name(doc, &event.account_id),
        network: exchange.is_none().then_some(event.chain),
        exchange,
        time: event.time.clone(),
        kind: fix
            .and_then(|fix| fix.kind)
            .or(rule.and_then(|rule| rule.kind))
            .unwrap_or(event.kind),
        source_kind: event.kind,
        legs: event
            .legs
            .iter()
            .map(|leg| CryptoOpLeg {
                direction: leg.direction,
                asset_key: leg.asset.key(),
                symbol: crypto_symbol(doc, &leg.asset),
                amount: leg.amount,
            })
            .collect(),
        gas: event.gas.clone(),
        gas_symbol: event.gas.as_ref().map(|gas| crypto_symbol(doc, &gas.asset)),
        protocol: event.protocol.clone(),
        from: event.from.clone(),
        to: event.to.clone(),
        reference,
        value_usd: crypto_signed_value(doc, event),
        flow_id: fix.and_then(|fix| fix.flow_id.clone()),
        contact_id: shown_contact.filter(|_| !contact_guessed),
        contact_name,
        contact_guessed,
        comment: fix.and_then(|fix| fix.comment.clone()),
        categories: fix
            .map(|fix| fix.categories.clone())
            .filter(|list| !list.is_empty())
            .or_else(|| rule.map(|rule| rule.categories.clone()))
            .unwrap_or_default(),
        hidden: fix.is_some_and(|fix| fix.hidden),
        corrected: fix.is_some_and(CryptoOverride::corrects_the_operation),
        linkable: own_entry.is_none() && event.tracking.url(links).is_some(),
        own_entry,
        by_rule,
    }
}

/// One page of the operations, newest first, narrowed by the filter.
/// Filtering and paging happen here, so a long history never travels
/// whole to the screen.
pub fn crypto_list_operations(
    books: &Books<'_>,
    input: &CryptoListOperationsInput,
) -> CryptoOpsPage {
    let doc = books.doc();
    let filter = &input.filter;
    let fixes = overrides::index(&doc.crypto_overrides);
    let rules = overrides::rules(&doc.crypto_overrides);
    let account_ok = |event: &CryptoTx| {
        filter.account_ids.is_empty() || filter.account_ids.contains(&event.account_id)
    };
    let mut in_scope: Vec<(&CryptoTx, Option<&CryptoOverride>)> = Vec::new();
    for event in &doc.crypto_events {
        // The module's own entries (an opening, a reconciliation) are listed
        // too, marked as such: they explain a balance nothing else does.
        if !account_ok(event) {
            continue;
        }
        let fix = fixes.get(event.id.as_str()).copied();
        if fix.is_some_and(|fix| fix.hidden) && !filter.show_hidden {
            continue;
        }
        in_scope.push((event, fix));
    }

    let mut facets = CryptoOpFacets::default();
    let mut chains = BTreeSet::new();
    let mut protocols = BTreeSet::new();
    let mut tokens: BTreeMap<String, String> = BTreeMap::new();
    for (event, _) in &in_scope {
        if !matches!(event.tracking, TrackingRef::ExchangeHistory { .. }) {
            chains.insert(format!("{:?}", event.chain).to_lowercase());
        }
        if let Some(protocol) = &event.protocol {
            protocols.insert(protocol.clone());
        }
        for leg in &event.legs {
            tokens
                .entry(leg.asset.key())
                .or_insert_with(|| crypto_symbol(doc, &leg.asset));
        }
    }
    facets.chains = crate::model::chain::ALL_CHAINS
        .iter()
        .copied()
        .filter(|chain| chains.contains(&format!("{chain:?}").to_lowercase()))
        .collect();
    facets.protocols = protocols.into_iter().collect();
    facets.tokens = tokens
        .into_iter()
        .map(|(key, symbol)| CryptoOpTokenFacet { key, symbol })
        .collect();

    let mut matched: Vec<(&CryptoTx, Option<&CryptoOverride>)> = in_scope
        .into_iter()
        .filter(|(event, fix)| crypto_matches(doc, filter, event, *fix, rule_for(&rules, event)))
        .collect();
    matched.sort_by(|left, right| (&right.0.time, &right.0.id).cmp(&(&left.0.time, &left.0.id)));
    let total = matched.len() as u32;
    let limit = input.limit.map_or(OPERATIONS_PAGE, |asked| {
        (asked as usize).min(OPERATIONS_PAGE)
    });
    // The user's own explorers, read once for the whole page.
    let links = crypto_explorer_settings(books);
    let items = matched
        .iter()
        .skip(input.offset as usize)
        .take(limit)
        .map(|(event, fix)| crypto_row(doc, event, *fix, rule_for(&rules, event), &links))
        .collect();
    let suggestion = (input.offset == 0)
        .then(|| crypto_exchange_suggestion(books, &matched))
        .flatten();
    CryptoOpsPage {
        items,
        total,
        facets,
        suggestion,
    }
}

fn crypto_matches(
    doc: &VaultDoc,
    filter: &CryptoOpFilter,
    event: &CryptoTx,
    fix: Option<&CryptoOverride>,
    rule: Option<&CryptoOverride>,
) -> bool {
    if filter.chain.is_some_and(|chain| chain != event.chain) {
        return false;
    }
    if let Some(kind) = filter.kind
        && fix
            .and_then(|fix| fix.kind)
            .or(rule.and_then(|rule| rule.kind))
            .unwrap_or(event.kind)
            != kind
    {
        return false;
    }
    if let Some(protocol) = filter.protocol.as_deref().filter(|text| !text.is_empty())
        && event.protocol.as_deref() != Some(protocol)
    {
        return false;
    }
    if let Some(token) = filter.token.as_deref().filter(|text| !text.is_empty()) {
        let hit = event.legs.iter().any(|leg| {
            leg.asset.key() == token || crypto_symbol(doc, &leg.asset).eq_ignore_ascii_case(token)
        });
        if !hit {
            return false;
        }
    }
    if let Some(address) = filter
        .address
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        let needle = address.to_lowercase();
        if !event.from.to_lowercase().contains(&needle)
            && !event.to.to_lowercase().contains(&needle)
        {
            return false;
        }
    }
    true
}

/// The fiat outflows near an incoming operation, best guesses first.
fn crypto_candidates(books: &Books<'_>, event: &CryptoTx) -> Vec<CryptoLinkCandidate> {
    let doc = books.doc();
    let Some(incoming) = event
        .legs
        .iter()
        .find(|leg| leg.direction == CryptoDirection::In)
    else {
        return Vec::new();
    };
    let Some(event_secs) = secs(&event.time) else {
        return Vec::new();
    };
    let symbol = crypto_symbol(doc, &incoming.asset);
    let market_usd = day_of(&event.time)
        .and_then(|day| prices::lookup(doc, &incoming.asset, day).value)
        .and_then(|price| math::mul(price, incoming.amount));
    // A fiat operation already tied to another crypto one is not offered.
    let taken: BTreeSet<&str> = doc
        .crypto_overrides
        .iter()
        .filter(|fix| fix.event_id != event.id)
        .filter_map(|fix| fix.flow_id.as_deref())
        .collect();
    let mut out: Vec<(i64, CryptoLinkCandidate)> = Vec::new();
    for tx in &doc.transactions {
        let meta = tx.meta();
        if meta
            .flow_id
            .as_deref()
            .is_some_and(|flow| taken.contains(flow))
        {
            continue;
        }
        let Some(tx_secs) = secs(&meta.timestamp) else {
            continue;
        };
        let apart = (tx_secs - event_secs).abs();
        if apart > CANDIDATE_DAYS * 86_400 + 86_400 {
            continue;
        }
        let Some(leg) = tx.legs().into_iter().find(|leg| {
            leg.role == LegRole::Principal && leg.asset.is_fiat() && leg.amount.is_negative()
        }) else {
            continue;
        };
        let days = match (day_of(&event.time), day_of(&meta.timestamp)) {
            (Some(left), Some(right)) => (left - right).num_days().unsigned_abs() as u32,
            _ => 0,
        };
        if i64::from(days) > CANDIDATE_DAYS {
            continue;
        }
        let mut reasons = vec![if days == 0 {
            CandidateReason::SameDay
        } else {
            CandidateReason::NearDate { days }
        }];
        let spent = leg.amount.abs();
        let fits = market_usd.is_some_and(|market| {
            convert_at(books, spent, &leg.asset.symbol, "usd", &meta.timestamp).is_some_and(|usd| {
                if market.is_zero() {
                    return false;
                }
                let apart = math::sub(usd, market).map(|gap| gap.abs());
                let scaled = apart.and_then(|gap| math::mul(gap, Amount::from(100)));
                let allowed = math::mul(market, Amount::from(AMOUNT_TOLERANCE_PERCENT as u32));
                matches!((scaled, allowed), (Some(gap), Some(limit)) if gap <= limit)
            })
        });
        if fits {
            reasons.push(CandidateReason::AmountFits);
        }
        let rate = math::div(spent, incoming.amount);
        let score = i64::from(fits) * 1_000 - i64::from(days) * 10 - apart / 86_400;
        out.push((
            score,
            CryptoLinkCandidate {
                transaction_id: meta.id.clone(),
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
                reasons,
                rate,
                rate_symbol: Some(symbol.clone()),
            },
        ));
    }
    out.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then(left.1.transaction_id.cmp(&right.1.transaction_id))
    });
    out.into_iter()
        .take(CANDIDATE_CAP)
        .map(|(_, candidate)| candidate)
        .collect()
}

/// The banner: the newest unlinked incoming operation that a fiat one
/// matches by date and by sum. Only a suggestion: nothing is linked here.
fn crypto_exchange_suggestion(
    books: &Books<'_>,
    newest_first: &[(&CryptoTx, Option<&CryptoOverride>)],
) -> Option<CryptoExchangeSuggestion> {
    let doc = books.doc();
    newest_first
        .iter()
        .filter(|(event, fix)| {
            fix.is_none_or(|fix| fix.flow_id.is_none() && !fix.hidden && !fix.not_exchange)
                && ids::adjustment(&event.id).is_none()
                && event.legs.len() == 1
                && event.legs[0].direction == CryptoDirection::In
        })
        .take(SUGGEST_SCAN)
        .find_map(|(event, _)| {
            let candidate = crypto_candidates(books, event)
                .into_iter()
                .find(|candidate| candidate.reasons.contains(&CandidateReason::AmountFits))?;
            let leg = &event.legs[0];
            Some(CryptoExchangeSuggestion {
                event_id: event.id.clone(),
                symbol: crypto_symbol(doc, &leg.asset),
                amount: leg.amount,
                candidate,
            })
        })
}

/// One operation with its chain and, when asked, what could have paid
/// for it.
pub fn crypto_operation_detail(
    books: &Books<'_>,
    input: &CryptoOperationDetailInput,
) -> Result<CryptoOperationDetail> {
    let doc = books.doc();
    // Any entry, the module's own ones too: the card says where they come from.
    let event = doc
        .crypto_events
        .iter()
        .find(|event| event.id == input.id)
        .ok_or_else(|| Error::NotFound(format!("operation {}", input.id)))?;
    let fixes = overrides::index(&doc.crypto_overrides);
    let rules = overrides::rules(&doc.crypto_overrides);
    let fix = fixes.get(event.id.as_str()).copied();
    let rule = rule_for(&rules, event);
    let links = crypto_explorer_settings(books);
    let row = crypto_row(doc, event, fix, rule, &links);
    // The module's own entries have no page anywhere: no explorer link.
    let link = row
        .own_entry
        .is_none()
        .then(|| event.tracking.link(&links))
        .flatten();
    let key = overrides::contract_key(event).filter(|_| row.own_entry.is_none());
    let same_contract_count = key.as_ref().map_or(1, |wanted| {
        doc.crypto_events
            .iter()
            .filter(|other| ids::adjustment(&other.id).is_none())
            .filter(|other| overrides::contract_key(other).as_ref() == Some(wanted))
            .count() as u32
    });
    let contract_rule = rule.map(|rule| CryptoContractRule {
        kind: rule.kind,
        categories: rule.categories.clone(),
        contact_id: rule.contact_id.clone(),
    });
    let contract = key.map(|key| {
        key.split_once('|')
            .map_or(key.clone(), |(_, rest)| rest.to_string())
    });
    let chain = row
        .flow_id
        .as_deref()
        .map(|flow| crypto_chain_members(doc, flow, &event.id))
        .unwrap_or_default();
    let fiat = chain
        .iter()
        .filter(|member| member.side == ChainSide::Fiat)
        .count();
    let coins = chain
        .iter()
        .filter(|member| member.side == ChainSide::Crypto)
        .count();
    Ok(CryptoOperationDetail {
        url: link.as_ref().map(|link| link.url.clone()),
        url_note: link.and_then(|link| link.note),
        method: event.method.clone(),
        block: event.block,
        tracking: event.tracking.clone(),
        basis_from_chain: fiat == 1 && coins == 1,
        same_contract_count,
        contract_rule,
        contract,
        candidates: if input.with_candidates {
            crypto_candidates(books, event)
        } else {
            Vec::new()
        },
        chain,
        row,
    })
}

fn crypto_chain_members(doc: &VaultDoc, flow: &str, this: &str) -> Vec<CryptoChainMember> {
    let mut out = Vec::new();
    for tx in &doc.transactions {
        if tx.meta().flow_id.as_deref() != Some(flow) {
            continue;
        }
        let Some(leg) = tx
            .legs()
            .into_iter()
            .find(|leg| leg.role == LegRole::Principal && leg.asset.is_fiat())
            .or_else(|| {
                tx.legs()
                    .into_iter()
                    .find(|leg| leg.role == LegRole::Principal)
            })
        else {
            continue;
        };
        out.push(CryptoChainMember {
            side: ChainSide::Fiat,
            id: tx.meta().id.clone(),
            place: doc
                .stores
                .iter()
                .find(|store| store.id == leg.store_id)
                .map(|store| store.name.clone())
                .unwrap_or_default(),
            amount: leg.amount,
            currency: leg.asset.symbol.clone(),
            time: tx.meta().timestamp.clone(),
            is_this: false,
        });
    }
    for fix in doc
        .crypto_overrides
        .iter()
        .filter(|fix| fix.flow_id.as_deref() == Some(flow))
    {
        let Some(event) = doc
            .crypto_events
            .iter()
            .find(|event| event.id == fix.event_id)
        else {
            continue;
        };
        let Some(leg) = event.legs.first() else {
            continue;
        };
        out.push(CryptoChainMember {
            side: ChainSide::Crypto,
            id: event.id.clone(),
            place: crypto_account_name(doc, &event.account_id),
            amount: match leg.direction {
                CryptoDirection::In => leg.amount,
                CryptoDirection::Out => -leg.amount,
            },
            currency: crypto_symbol(doc, &leg.asset),
            time: event.time.clone(),
            is_this: event.id == this,
        });
    }
    out.sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
    out
}

/// Lay a correction over an operation (see [`CryptoSetOverrideInput`]).
/// The event is left as it came; the correction survives a refetch.
pub fn crypto_set_override(
    vault: &mut UnlockedVault,
    input: &CryptoSetOverrideInput,
) -> Result<CryptoSetOverrideResult> {
    let now = now_iso();
    let (result, _, _) = vault.run(|tx| lay_override(tx, input, &now))?;
    // The chain decides a cost basis and a kind decides how an operation
    // folds, so the holdings are derived again before the one save.
    refold_holdings(vault);
    vault.transact(|tx| {
        tx.touch();
        Ok(result)
    })
}

/// The edit of the document a correction makes: the operation's own record,
/// the rule for its contract and the contact's address. Saves nothing.
fn lay_override(
    tx: &mut Tx<'_>,
    input: &CryptoSetOverrideInput,
    now: &str,
) -> Result<CryptoSetOverrideResult> {
    let doc = tx.doc();
    let event = crypto_event_ref(doc, &input.event_id)?.clone();
    if let Some(comment) = &input.comment
        && comment.chars().count() > COMMENT_MAX
    {
        return Err(Error::InvalidInput(
            "crypto: the comment is too long".into(),
        ));
    }
    if let Some(categories) = &input.categories {
        for key in categories {
            if !doc.categories.iter().any(|category| &category.key == key) {
                return Err(Error::NotFound(format!("category {key}")));
            }
        }
    }
    if let Some(contact) = &input.contact_id
        && !doc.contacts.iter().any(|known| &known.id == contact)
    {
        return Err(Error::NotFound(format!("contact {contact}")));
    }
    if let Some(tx_id) = &input.link_transaction_id
        && !doc.transactions.iter().any(|tx| &tx.meta().id == tx_id)
    {
        return Err(Error::NotFound(format!("transaction {tx_id}")));
    }

    // 1. The operation's own correction.
    let latest = overrides::index(&doc.crypto_overrides)
        .get(event.id.as_str())
        .map(|fix| fix.id.clone());
    let mut fix = match latest
        .as_ref()
        .and_then(|id| doc.crypto_overrides.iter().find(|fix| &fix.id == id))
    {
        Some(found) => found.clone(),
        None => CryptoOverride {
            id: format!("cov:{}", event.id),
            event_id: event.id.clone(),
            kind: None,
            comment: None,
            categories: Vec::new(),
            flow_id: None,
            contact_id: None,
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: now.to_string(),
        },
    };
    for field in &input.clear {
        match field {
            OverrideField::Kind => fix.kind = None,
            OverrideField::Comment => fix.comment = None,
            OverrideField::Categories => fix.categories.clear(),
            OverrideField::Contact => fix.contact_id = None,
            OverrideField::Flow => {
                if let Some(old) = fix.flow_id.take() {
                    crypto_release_chain(tx.doc_mut(), &old, &fix.event_id);
                }
            }
        }
    }
    if let Some(kind) = input.kind {
        fix.kind = Some(kind);
    }
    if let Some(categories) = &input.categories {
        fix.categories = categories.clone();
    }
    if let Some(contact) = &input.contact_id {
        fix.contact_id = Some(contact.clone());
    }
    if let Some(comment) = &input.comment {
        let text = comment.trim();
        fix.comment = (!text.is_empty()).then(|| text.to_string());
    }
    if let Some(hidden) = input.hidden {
        fix.hidden = hidden;
    }
    if let Some(flag) = input.not_exchange {
        fix.not_exchange = flag;
    }
    if let Some(tx_id) = &input.link_transaction_id {
        let flow = crypto_join_chain(tx.doc_mut(), tx_id, fix.flow_id.as_deref())?;
        fix.flow_id = Some(flow);
        // Linked is the opposite of «not an exchange».
        fix.not_exchange = false;
    }
    let flow_now = fix.flow_id.clone();
    fix.updated_at = now.to_string();
    fix.validate()?;
    upsert_override(tx, fix)?;

    // 2. The rule for the contract, which outlives any refetch.
    let mut applied = 1u32;
    if (input.same_contract || input.remove_contract_rule)
        && let Some(key) = overrides::contract_key(&event)
    {
        applied = crypto_write_rule(tx, &key, input, now)?;
    }

    if let Some(contact_id) = &input.contact_id {
        crypto_remember_address(tx.doc_mut(), contact_id, &event);
    }
    Ok(CryptoSetOverrideResult {
        flow_id: flow_now,
        applied,
    })
}

/// Store `fix` under its id: a new record is inserted, a known one rewritten.
fn upsert_override(tx: &mut Tx<'_>, fix: CryptoOverride) -> Result<()> {
    let mut stored = tx.collection::<CryptoOverride>();
    if stored.get(&fix.id).is_some() {
        let id = fix.id.clone();
        stored.update(&id, |have| *have = fix)
    } else {
        stored.insert(fix)
    }
}

/// Set, change or remove the rule of one contract; the rule is a record of
/// `crypto_overrides` under [`overrides::rule_event_id`]. A rule left with
/// nothing in it is removed. Returns how many operations it reaches.
fn crypto_write_rule(
    tx: &mut Tx<'_>,
    key: &str,
    input: &CryptoSetOverrideInput,
    now: &str,
) -> Result<u32> {
    let event_id = overrides::rule_event_id(key);
    let doc = tx.doc();
    let mut rule = overrides::rules(&doc.crypto_overrides)
        .get(key)
        .map(|found| (*found).clone())
        .unwrap_or_else(|| CryptoOverride {
            id: format!("crule:{key}"),
            event_id: event_id.clone(),
            kind: None,
            comment: None,
            categories: Vec::new(),
            flow_id: None,
            contact_id: None,
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: now.to_string(),
        });
    if !input.remove_contract_rule {
        for field in &input.clear {
            match field {
                OverrideField::Kind => rule.kind = None,
                OverrideField::Categories => rule.categories.clear(),
                OverrideField::Contact => rule.contact_id = None,
                OverrideField::Comment | OverrideField::Flow => {}
            }
        }
        if let Some(kind) = input.kind {
            rule.kind = Some(kind);
        }
        if let Some(categories) = &input.categories {
            rule.categories = categories.clone();
        }
        if let Some(contact) = &input.contact_id {
            rule.contact_id = Some(contact.clone());
        }
    }
    let empty = input.remove_contract_rule
        || (rule.kind.is_none() && rule.categories.is_empty() && rule.contact_id.is_none());
    if !empty {
        rule.updated_at = now.to_string();
        rule.validate()?;
    }
    let stale: Vec<String> = doc
        .crypto_overrides
        .iter()
        .filter(|have| have.event_id == event_id && (empty || have.id != rule.id))
        .map(|have| have.id.clone())
        .collect();
    let mut stored = tx.collection::<CryptoOverride>();
    for id in &stale {
        stored.remove(id)?;
    }
    if !empty {
        upsert_override(tx, rule)?;
    }
    Ok(tx
        .doc()
        .crypto_events
        .iter()
        .filter(|other| ids::adjustment(&other.id).is_none())
        .filter(|other| overrides::contract_key(other).as_deref() == Some(key))
        .count() as u32)
}
