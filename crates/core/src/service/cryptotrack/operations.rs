//! The module's operations as the screens read them, and the corrections the
//! user lays over them (spec E10).
//!
//! **Nothing here creates or deletes an operation.** Events come from the
//! sources and are rewritten by a refetch; the one write in this file is a
//! [`CryptoOverride`], which carries classification and attributes (kind,
//! comment, categories, contact, hidden, chain) and never an amount. The
//! chain write also sets `flow_id` on a fiat operation the user chose, which
//! is the same id the fiat `link_transactions` writes.
//!
//! Reading builds no URL: a row only says whether a link exists
//! (`linkable`). The address of the page is made in the detail, when the user
//! opens the operation, and the frontend opens it only on a click.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate};

use super::super::UnlockedVault;
use crate::Amount;
use crate::cryptotrack::chains::evm_chain;
use crate::cryptotrack::engine::{ids, math, overrides};
use crate::cryptotrack::prices;
use crate::cryptotrack::tracking::ExplorerSettings;
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::operations::{
    CandidateReason, ChainSide, CryptoChainMember, CryptoContractRule, CryptoExchangeSuggestion,
    CryptoLinkCandidate, CryptoListOperationsInput, CryptoOpFacets, CryptoOpFilter, CryptoOpLeg,
    CryptoOpRow, CryptoOpTokenFacet, CryptoOperationDetail, CryptoOperationDetailInput,
    CryptoOpsPage, CryptoSetOverrideInput, CryptoSetOverrideResult, OPERATIONS_PAGE, OverrideField,
    OwnEntry,
};
use crate::model::chain::Chain;
use crate::model::contact::{ContactAccount, DetailKind};
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoOverride, CryptoTx, TrackingRef, normalize_address,
};
use crate::model::transaction::LegRole;
use crate::model::vault::now_iso;

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

/// The coin a network pays gas in.
fn native_symbol(chain: Chain) -> &'static str {
    if let Some(row) = evm_chain(chain) {
        return row.native.symbol;
    }
    match chain {
        Chain::Bitcoin => "BTC",
        Chain::Solana => "SOL",
        Chain::Tron => "TRX",
        Chain::Litecoin => "LTC",
        Chain::Dogecoin => "DOGE",
        Chain::Cardano => "ADA",
        Chain::Xrp => "XRP",
        Chain::Polkadot => "DOT",
        Chain::Cosmos => "ATOM",
        Chain::Near => "NEAR",
        Chain::Ton => "TON",
        Chain::Algorand => "ALGO",
        _ => "",
    }
}

fn short(text: &str) -> String {
    let count = text.chars().count();
    if count <= 12 {
        return text.to_string();
    }
    let head: String = text.chars().take(6).collect();
    let tail: String = text.chars().skip(count - 4).collect();
    format!("{head}…{tail}")
}

fn secs(time: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(time)
        .ok()
        .map(|moment| moment.timestamp())
}

fn day_of(time: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(time.get(..10)?, "%Y-%m-%d").ok()
}

/// A rule for the contract of `event`, if the user set one.
fn rule_for<'doc>(
    rules: &BTreeMap<&str, &'doc CryptoOverride>,
    event: &CryptoTx,
) -> Option<&'doc CryptoOverride> {
    overrides::contract_key(event).and_then(|key| rules.get(key.as_str()).copied())
}

/// The other side of an operation, from the followed account's view.
fn other_side(event: &CryptoTx) -> &str {
    match event.legs.first().map(|leg| leg.direction) {
        Some(CryptoDirection::In) => &event.from,
        _ => &event.to,
    }
}

impl UnlockedVault {
    pub(crate) fn crypto_symbol(&self, asset: &CryptoAsset) -> String {
        match asset {
            CryptoAsset::Native { chain } => native_symbol(*chain).to_string(),
            // A coin on an exchange is `Token(Fiat, "<TICKER>")`.
            CryptoAsset::Token(token) if token.chain == Chain::Fiat => {
                token.contract.to_uppercase()
            }
            CryptoAsset::Token(token) => self
                .doc
                .crypto_tokens
                .iter()
                .find(|record| record.id() == *token)
                .map_or_else(|| short(&token.contract), |record| record.symbol.clone()),
        }
    }

    pub(crate) fn crypto_account_name(&self, account_id: &str) -> String {
        if let Some(store) = self
            .doc
            .stores
            .iter()
            .find(|store| store.crypto_account_id.as_deref() == Some(account_id))
        {
            return store.name.clone();
        }
        self.doc
            .crypto_accounts
            .iter()
            .find(|account| account.id == account_id)
            .map(|account| account.name.clone())
            .unwrap_or_default()
    }

    /// The contact whose saved crypto address is `address` on `chain`.
    pub(crate) fn crypto_contact_by_address(&self, chain: Chain, address: &str) -> Option<&str> {
        let wanted = normalize_address(chain, address);
        if wanted.is_empty() {
            return None;
        }
        self.doc
            .contacts
            .iter()
            .filter(|contact| !contact.archived)
            .find(|contact| {
                contact.accounts.iter().any(|account| {
                    account.kind == DetailKind::CryptoAddress
                        && account.active
                        && account.network.is_none_or(|network| network == chain)
                        && normalize_address(chain, &account.value) == wanted
                })
            })
            .map(|contact| contact.id.as_str())
    }

    pub(crate) fn crypto_signed_value(&self, event: &CryptoTx) -> Option<Amount> {
        let day = day_of(&event.time)?;
        // Quantities and prices come from a chain, where a token may carry
        // 1e24 units: `Amount`'s own operators panic on overflow and the
        // release profile aborts, so every step goes through `math`, which
        // answers `None` instead (the figure then shows as a dash).
        let mut total = Amount::ZERO;
        for leg in &event.legs {
            let price = prices::lookup(&self.doc, &leg.asset, day).value?;
            let value = math::mul(leg.amount, price)?;
            total = match leg.direction {
                CryptoDirection::In => math::add(total, value)?,
                CryptoDirection::Out => math::sub(total, value)?,
            };
        }
        (!event.legs.is_empty()).then_some(total)
    }

    pub(crate) fn crypto_row(
        &self,
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
            self.crypto_contact_by_address(event.chain, other_side(event))
                .map(str::to_string)
        } else {
            None
        };
        let contact_guessed = guessed.is_some();
        let shown_contact = contact_id.clone().or(guessed);
        let contact_name = shown_contact.as_deref().and_then(|id| {
            self.doc
                .contacts
                .iter()
                .find(|contact| contact.id == id)
                .map(|contact| contact.name.clone())
        });
        CryptoOpRow {
            id: event.id.clone(),
            account_id: event.account_id.clone(),
            account_name: self.crypto_account_name(&event.account_id),
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
                    symbol: self.crypto_symbol(&leg.asset),
                    amount: leg.amount,
                })
                .collect(),
            gas: event.gas.clone(),
            gas_symbol: event.gas.as_ref().map(|gas| self.crypto_symbol(&gas.asset)),
            protocol: event.protocol.clone(),
            from: event.from.clone(),
            to: event.to.clone(),
            reference,
            value_usd: self.crypto_signed_value(event),
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
    pub fn crypto_list_operations(&self, input: &CryptoListOperationsInput) -> CryptoOpsPage {
        let filter = &input.filter;
        let fixes = overrides::index(&self.doc.crypto_overrides);
        let rules = overrides::rules(&self.doc.crypto_overrides);
        let account_ok = |event: &CryptoTx| {
            filter.account_ids.is_empty() || filter.account_ids.contains(&event.account_id)
        };
        let mut in_scope: Vec<(&CryptoTx, Option<&CryptoOverride>)> = Vec::new();
        for event in &self.doc.crypto_events {
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
                    .or_insert_with(|| self.crypto_symbol(&leg.asset));
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
            .filter(|(event, fix)| {
                self.crypto_matches(filter, event, *fix, rule_for(&rules, event))
            })
            .collect();
        matched
            .sort_by(|left, right| (&right.0.time, &right.0.id).cmp(&(&left.0.time, &left.0.id)));
        let total = matched.len() as u32;
        let limit = input.limit.map_or(OPERATIONS_PAGE, |asked| {
            (asked as usize).min(OPERATIONS_PAGE)
        });
        // The user's own explorers, read once for the whole page.
        let links = self.crypto_explorer_settings();
        let items = matched
            .iter()
            .skip(input.offset as usize)
            .take(limit)
            .map(|(event, fix)| self.crypto_row(event, *fix, rule_for(&rules, event), &links))
            .collect();
        let suggestion = (input.offset == 0)
            .then(|| self.crypto_exchange_suggestion(&matched))
            .flatten();
        CryptoOpsPage {
            items,
            total,
            facets,
            suggestion,
        }
    }

    pub(crate) fn crypto_matches(
        &self,
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
                leg.asset.key() == token
                    || self.crypto_symbol(&leg.asset).eq_ignore_ascii_case(token)
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
    pub(crate) fn crypto_candidates(&self, event: &CryptoTx) -> Vec<CryptoLinkCandidate> {
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
        let symbol = self.crypto_symbol(&incoming.asset);
        let market_usd = day_of(&event.time)
            .and_then(|day| prices::lookup(&self.doc, &incoming.asset, day).value)
            .and_then(|price| math::mul(price, incoming.amount));
        // A fiat operation already tied to another crypto one is not offered.
        let taken: BTreeSet<&str> = self
            .doc
            .crypto_overrides
            .iter()
            .filter(|fix| fix.event_id != event.id)
            .filter_map(|fix| fix.flow_id.as_deref())
            .collect();
        let mut out: Vec<(i64, CryptoLinkCandidate)> = Vec::new();
        for tx in &self.doc.transactions {
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
                self.convert_at(spent, &leg.asset.symbol, "usd", &meta.timestamp)
                    .is_some_and(|usd| {
                        if market.is_zero() {
                            return false;
                        }
                        let apart = math::sub(usd, market).map(|gap| gap.abs());
                        let scaled = apart.and_then(|gap| math::mul(gap, Amount::from(100)));
                        let allowed =
                            math::mul(market, Amount::from(AMOUNT_TOLERANCE_PERCENT as u32));
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
                    place: self
                        .doc
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
    pub(crate) fn crypto_exchange_suggestion(
        &self,
        newest_first: &[(&CryptoTx, Option<&CryptoOverride>)],
    ) -> Option<CryptoExchangeSuggestion> {
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
                let candidate = self
                    .crypto_candidates(event)
                    .into_iter()
                    .find(|candidate| candidate.reasons.contains(&CandidateReason::AmountFits))?;
                let leg = &event.legs[0];
                Some(CryptoExchangeSuggestion {
                    event_id: event.id.clone(),
                    symbol: self.crypto_symbol(&leg.asset),
                    amount: leg.amount,
                    candidate,
                })
            })
    }

    /// One operation with its chain and, when asked, what could have paid
    /// for it.
    pub fn crypto_operation_detail(
        &self,
        input: &CryptoOperationDetailInput,
    ) -> Result<CryptoOperationDetail> {
        // Any entry, the module's own ones too: the card says where they come from.
        let event = self
            .doc
            .crypto_events
            .iter()
            .find(|event| event.id == input.id)
            .ok_or_else(|| Error::NotFound(format!("operation {}", input.id)))?;
        let fixes = overrides::index(&self.doc.crypto_overrides);
        let rules = overrides::rules(&self.doc.crypto_overrides);
        let fix = fixes.get(event.id.as_str()).copied();
        let rule = rule_for(&rules, event);
        let links = self.crypto_explorer_settings();
        let row = self.crypto_row(event, fix, rule, &links);
        // The module's own entries have no page anywhere: no explorer link.
        let link = row
            .own_entry
            .is_none()
            .then(|| event.tracking.link(&links))
            .flatten();
        let key = overrides::contract_key(event).filter(|_| row.own_entry.is_none());
        let same_contract_count = key.as_ref().map_or(1, |wanted| {
            self.doc
                .crypto_events
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
            .map(|flow| self.crypto_chain_members(flow, &event.id))
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
                self.crypto_candidates(event)
            } else {
                Vec::new()
            },
            chain,
            row,
        })
    }

    pub(crate) fn crypto_event_ref(&self, id: &str) -> Result<&CryptoTx> {
        self.doc
            .crypto_events
            .iter()
            .find(|event| event.id == id && ids::adjustment(&event.id).is_none())
            .ok_or_else(|| Error::NotFound(format!("operation {id}")))
    }

    pub(crate) fn crypto_chain_members(&self, flow: &str, this: &str) -> Vec<CryptoChainMember> {
        let mut out = Vec::new();
        for tx in &self.doc.transactions {
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
                place: self
                    .doc
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
        for fix in self
            .doc
            .crypto_overrides
            .iter()
            .filter(|fix| fix.flow_id.as_deref() == Some(flow))
        {
            let Some(event) = self
                .doc
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
                place: self.crypto_account_name(&event.account_id),
                amount: match leg.direction {
                    CryptoDirection::In => leg.amount,
                    CryptoDirection::Out => -leg.amount,
                },
                currency: self.crypto_symbol(&leg.asset),
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
        &mut self,
        input: &CryptoSetOverrideInput,
    ) -> Result<CryptoSetOverrideResult> {
        let event = self.crypto_event_ref(&input.event_id)?.clone();
        if let Some(comment) = &input.comment
            && comment.chars().count() > COMMENT_MAX
        {
            return Err(Error::InvalidInput(
                "crypto: the comment is too long".into(),
            ));
        }
        if let Some(categories) = &input.categories {
            for key in categories {
                if !self
                    .doc
                    .categories
                    .iter()
                    .any(|category| &category.key == key)
                {
                    return Err(Error::NotFound(format!("category {key}")));
                }
            }
        }
        if let Some(contact) = &input.contact_id
            && !self.doc.contacts.iter().any(|known| &known.id == contact)
        {
            return Err(Error::NotFound(format!("contact {contact}")));
        }
        if let Some(tx_id) = &input.link_transaction_id
            && !self
                .doc
                .transactions
                .iter()
                .any(|tx| &tx.meta().id == tx_id)
        {
            return Err(Error::NotFound(format!("transaction {tx_id}")));
        }

        let now = now_iso();

        // 1. The operation's own correction.
        let latest = overrides::index(&self.doc.crypto_overrides)
            .get(event.id.as_str())
            .map(|fix| fix.id.clone());
        let mut fix = match latest
            .as_ref()
            .and_then(|id| self.doc.crypto_overrides.iter().find(|fix| &fix.id == id))
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
                updated_at: now.clone(),
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
                        self.crypto_release_chain(&old, &fix.event_id);
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
            let flow = self.crypto_join_chain(tx_id, fix.flow_id.as_deref())?;
            fix.flow_id = Some(flow);
            // Linked is the opposite of «not an exchange».
            fix.not_exchange = false;
        }
        let flow_now = fix.flow_id.clone();
        fix.updated_at = now.clone();
        fix.validate()?;
        match self
            .doc
            .crypto_overrides
            .iter_mut()
            .find(|have| have.id == fix.id)
        {
            Some(have) => *have = fix,
            None => self.doc.crypto_overrides.push(fix),
        }

        // 2. The rule for the contract, which outlives any refetch.
        let mut applied = 1u32;
        if (input.same_contract || input.remove_contract_rule)
            && let Some(key) = overrides::contract_key(&event)
        {
            applied = self.crypto_write_rule(&key, input, &now)?;
        }

        if let Some(contact_id) = &input.contact_id {
            self.crypto_remember_address(contact_id, &event);
        }
        // The chain decides a cost basis and a kind decides how an operation
        // folds, so the holdings are derived again before the save.
        self.crypto_sync_holdings()?;
        Ok(CryptoSetOverrideResult {
            flow_id: flow_now,
            applied,
        })
    }

    /// Set, change or remove the rule of one contract; the rule is a record of
    /// `crypto_overrides` under [`overrides::rule_event_id`]. A rule left with
    /// nothing in it is removed. Returns how many operations it reaches.
    pub(crate) fn crypto_write_rule(
        &mut self,
        key: &str,
        input: &CryptoSetOverrideInput,
        now: &str,
    ) -> Result<u32> {
        let event_id = overrides::rule_event_id(key);
        let mut rule = overrides::rules(&self.doc.crypto_overrides)
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
        self.doc
            .crypto_overrides
            .retain(|have| have.event_id != event_id);
        if !empty {
            rule.updated_at = now.to_string();
            rule.validate()?;
            self.doc.crypto_overrides.push(rule);
        }
        Ok(self
            .doc
            .crypto_events
            .iter()
            .filter(|other| ids::adjustment(&other.id).is_none())
            .filter(|other| overrides::contract_key(other).as_deref() == Some(key))
            .count() as u32)
    }

    /// Put the fiat operation `tx_id` and a crypto one into one chain: the
    /// crypto one's own, else the fiat one's, else a new one.
    pub(crate) fn crypto_join_chain(
        &mut self,
        tx_id: &str,
        current: Option<&str>,
    ) -> Result<String> {
        let tx = self
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().id == tx_id)
            .ok_or_else(|| Error::NotFound(format!("transaction {tx_id}")))?;
        let flow = current
            .map(str::to_string)
            .or_else(|| tx.meta().flow_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        tx.meta_mut().flow_id = Some(flow.clone());
        Ok(flow)
    }

    /// After a crypto operation left chain `flow`: a chain that is left with
    /// one fiat operation and nothing else was made by this link, so it goes.
    pub(crate) fn crypto_release_chain(&mut self, flow: &str, leaving: &str) {
        let coins = self
            .doc
            .crypto_overrides
            .iter()
            .filter(|fix| fix.event_id != leaving && fix.flow_id.as_deref() == Some(flow))
            .count();
        let fiat = self
            .doc
            .transactions
            .iter()
            .filter(|tx| tx.meta().flow_id.as_deref() == Some(flow))
            .count();
        if coins == 0 && fiat <= 1 {
            for tx in self
                .doc
                .transactions
                .iter_mut()
                .filter(|tx| tx.meta().flow_id.as_deref() == Some(flow))
            {
                tx.meta_mut().flow_id = None;
            }
        }
    }

    /// «Through whom» also teaches the contact the address, so the next
    /// operation with it is named on its own.
    pub(crate) fn crypto_remember_address(&mut self, contact_id: &str, event: &CryptoTx) {
        let address = normalize_address(event.chain, other_side(event));
        if address.is_empty() || event.chain == Chain::Fiat {
            return;
        }
        let own: BTreeSet<String> = self
            .doc
            .crypto_accounts
            .iter()
            .map(|account| normalize_address(account.chain, &account.address))
            .collect();
        if own.contains(&address) {
            return;
        }
        let chain = event.chain;
        let Some(contact) = self
            .doc
            .contacts
            .iter_mut()
            .find(|known| known.id == contact_id)
        else {
            return;
        };
        let known = contact.accounts.iter().any(|account| {
            account.kind == DetailKind::CryptoAddress
                && normalize_address(chain, &account.value) == address
        });
        if known {
            return;
        }
        contact.accounts.push(ContactAccount {
            id: uuid::Uuid::new_v4().to_string(),
            group: None,
            label: chain.label().to_string(),
            kind: DetailKind::CryptoAddress,
            value: address,
            currency: None,
            network: Some(chain),
            active: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_contract::cryptotrack::operations::CryptoOpFilter;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoLeg, CryptoPricePoint, CryptoTxKind, TokenId,
    };
    use crate::model::store::StoreKind;
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
        vault.crypto_list_operations(&CryptoListOperationsInput {
            filter,
            ..Default::default()
        })
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
        let second = vault.crypto_list_operations(&CryptoListOperationsInput {
            offset: 100,
            limit: Some(10_000),
            ..Default::default()
        });
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
        let detail = vault
            .crypto_operation_detail(&CryptoOperationDetailInput {
                id: "fill".into(),
                with_candidates: false,
            })
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
            vault
                .crypto_operation_detail(&CryptoOperationDetailInput {
                    id: "cev:1".into(),
                    with_candidates: false,
                })
                .unwrap()
                .url
        }
        let preset = ask(&vault).expect("a chain operation has a page");
        assert!(preset.starts_with("https://etherscan.io/"), "{preset}");

        vault
            .crypto_set_network_settings(&SetNetworkSettingsInput {
                chain: Chain::Ethereum,
                rpc: String::new(),
                blockscout: String::new(),
                explorer: "https://eth.example.org".into(),
                blockscout_key: None,
            })
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
        let detail = vault
            .crypto_operation_detail(&CryptoOperationDetailInput {
                id: "a".into(),
                with_candidates: false,
            })
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
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                kind: Some(CryptoTxKind::Claim),
                comment: Some("  pay  ".into()),
                ..Default::default()
            })
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
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                clear: vec![OverrideField::Kind, OverrideField::Comment],
                ..Default::default()
            })
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
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                hidden: Some(true),
                ..Default::default()
            })
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
            vault.crypto_set_override(&input)
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
        let detail = vault
            .crypto_operation_detail(&CryptoOperationDetailInput {
                id: opening.id.clone(),
                with_candidates: false,
            })
            .unwrap();
        assert_eq!(detail.url, None);
        assert_eq!(detail.url_note, None);
        assert_eq!(detail.row.own_entry, Some(OwnEntry::Opening));
        assert!(
            vault
                .crypto_set_override(&CryptoSetOverrideInput {
                    event_id: opening.id,
                    kind: Some(CryptoTxKind::Claim),
                    ..Default::default()
                })
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
        let spend = vault.add_transaction(spend).unwrap();
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
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                not_exchange: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert!(vault.doc.crypto_overrides[0].not_exchange);
        assert!(
            page(&vault, CryptoOpFilter::default()).suggestion.is_none(),
            "«not an exchange» is respected"
        );
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                not_exchange: Some(false),
                ..Default::default()
            })
            .unwrap();
        assert!(page(&vault, CryptoOpFilter::default()).suggestion.is_some());

        let result = vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                link_transaction_id: Some(spend_id.clone()),
                ..Default::default()
            })
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
        let detail = vault
            .crypto_operation_detail(&CryptoOperationDetailInput {
                id: "a".into(),
                with_candidates: true,
            })
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

        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                clear: vec![OverrideField::Flow],
                ..Default::default()
            })
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
            vault
                .crypto_operation_detail(&CryptoOperationDetailInput {
                    id: "a".into(),
                    with_candidates: false,
                })
                .unwrap()
        };
        assert_eq!(detail(&vault).same_contract_count, 2);
        assert!(detail(&vault).contract_rule.is_none());
        let result = vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                kind: Some(CryptoTxKind::Claim),
                comment: Some("only here".into()),
                same_contract: true,
                ..Default::default()
            })
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
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "tomorrow".into(),
                kind: Some(CryptoTxKind::Stake),
                ..Default::default()
            })
            .unwrap();
        let rows = page(&vault, CryptoOpFilter::default()).items;
        assert_eq!(
            rows.iter().find(|row| row.id == "tomorrow").unwrap().kind,
            CryptoTxKind::Stake
        );
        // Removing the rule takes the decision off everything it reached.
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "b".into(),
                remove_contract_rule: true,
                ..Default::default()
            })
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
        vault
            .crypto_set_override(&CryptoSetOverrideInput {
                event_id: "a".into(),
                contact_id: Some("c1".into()),
                ..Default::default()
            })
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
        let registered = include_str!("../../../../app/src/lib.rs");
        let own = include_str!("../../../../app/src/commands/cryptotrack/operations.rs");
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
        // The service surface of this file: reads, and the one correction.
        let service = include_str!("operations.rs");
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
}
