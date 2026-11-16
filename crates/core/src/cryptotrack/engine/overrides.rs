//! The user's corrections, laid over the operations at the moment of the
//! recalculation.
//!
//! The events are immutable. A refetch rewrites `crypto_events` whole — the
//! same ids, the source's latest answer — so anything the user decided has to
//! live somewhere a refetch does not reach, which is `crypto_overrides`. The
//! calculation therefore never reads an event's own `kind` when a correction
//! names one: it reads the pair.
//!
//! One operation can end up with two corrections: two devices, each creating
//! its own record while apart, both arriving through a sync. Nothing can
//! merge them for the user, so the rule is stated instead of left to chance:
//! the latest `updated_at` wins, and on the same moment the larger id wins.
//! Both devices then show the same thing.

use std::collections::BTreeMap;

use crate::model::cryptotrack::{
    CryptoAsset, CryptoOverride, CryptoTx, CryptoTxKind, chain_slug, normalize_address,
};

/// An override whose `event_id` starts with this is not a correction of one
/// operation but a **rule for a contract**: what the user decided for every
/// operation that goes through it, those stored now and those a later refetch
/// brings. It is stored as an ordinary override record (so it syncs, is wiped
/// with the module and needs no new vault field) and is laid over an event at
/// the moment of the calculation, below the event's own correction.
pub const RULE_PREFIX: &str = "rule:";

/// The contract an operation went through: the first token leg's contract,
/// else the address it was sent to. `<chain>|<contract>`, normalized.
pub fn contract_key(event: &CryptoTx) -> Option<String> {
    let token = event.legs.iter().find_map(|leg| match &leg.asset {
        CryptoAsset::Token(token) => Some((token.chain, token.contract.clone())),
        CryptoAsset::Native { .. } => None,
    });
    let (chain, contract) = token.or_else(|| {
        (!event.to.trim().is_empty())
            .then(|| (event.chain, normalize_address(event.chain, &event.to)))
    })?;
    Some(format!("{}|{contract}", chain_slug(chain)))
}

/// The `event_id` the rule for a contract is stored under.
pub fn rule_event_id(contract: &str) -> String {
    format!("{RULE_PREFIX}{contract}")
}

/// The rules in force, by contract: the latest record of each.
pub fn rules(corrections: &[CryptoOverride]) -> BTreeMap<&str, &CryptoOverride> {
    let mut out: BTreeMap<&str, &CryptoOverride> = BTreeMap::new();
    for rule in corrections
        .iter()
        .filter(|fix| fix.event_id.starts_with(RULE_PREFIX))
    {
        let contract = &rule.event_id[RULE_PREFIX.len()..];
        out.entry(contract)
            .and_modify(|have| {
                if (&rule.updated_at, &rule.id) > (&have.updated_at, &have.id) {
                    *have = rule;
                }
            })
            .or_insert(rule);
    }
    out
}

/// One operation as the calculation sees it: the stored event, plus what the
/// user said about it.
#[derive(Debug, Clone, PartialEq)]
pub struct EventView<'doc> {
    pub event: &'doc CryptoTx,
    /// The corrected kind, or the event's own.
    pub kind: CryptoTxKind,
    /// The chain of operations this one belongs to (decision P6): how an
    /// exchange of fiat for coins is told, and where a cost basis above the
    /// market price comes from.
    pub flow_id: Option<&'doc str>,
    /// Who it went through.
    pub contact_id: Option<&'doc str>,
    /// Which correction decided this view, if any.
    pub override_id: Option<&'doc str>,
}

/// The correction that stands for each operation: by event id, the latest one.
pub fn index(corrections: &[CryptoOverride]) -> BTreeMap<&str, &CryptoOverride> {
    let mut out: BTreeMap<&str, &CryptoOverride> = BTreeMap::new();
    for correction in corrections {
        out.entry(correction.event_id.as_str())
            .and_modify(|have| {
                if (&correction.updated_at, &correction.id) > (&have.updated_at, &have.id) {
                    *have = correction;
                }
            })
            .or_insert(correction);
    }
    out
}

/// Every operation that counts, in `(time, id)` order, with its correction
/// applied. A hidden operation is not in the list at all.
pub fn apply<'doc>(
    events: &'doc [CryptoTx],
    corrections: &'doc [CryptoOverride],
) -> Vec<EventView<'doc>> {
    let rules = rules(corrections);
    let corrections = index(corrections);
    let mut out: Vec<EventView<'doc>> = events
        .iter()
        .filter_map(|event| {
            let correction = corrections.get(event.id.as_str()).copied();
            if correction.is_some_and(|fix| fix.hidden) {
                return None;
            }
            let rule = contract_key(event).and_then(|key| rules.get(key.as_str()).copied());
            Some(EventView {
                event,
                kind: correction
                    .and_then(|fix| fix.kind)
                    .or(rule.and_then(|rule| rule.kind))
                    .unwrap_or(event.kind),
                flow_id: correction.and_then(|fix| fix.flow_id.as_deref()),
                contact_id: correction
                    .and_then(|fix| fix.contact_id.as_deref())
                    .or(rule.and_then(|rule| rule.contact_id.as_deref())),
                override_id: correction.map(|fix| fix.id.as_str()),
            })
        })
        .collect();
    out.sort_by(|left, right| {
        (&left.event.time, &left.event.id).cmp(&(&right.event.time, &right.event.id))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoOverride, CryptoTx, CryptoTxKind, TrackingRef};

    fn event(id: &str) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: "2026-03-01T10:00:00Z".into(),
            block: Some(1),
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: Vec::new(),
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{id}"),
            },
        }
    }

    fn correction(id: &str, event_id: &str, updated_at: &str) -> CryptoOverride {
        CryptoOverride {
            id: id.into(),
            event_id: event_id.into(),
            kind: None,
            comment: None,
            categories: Vec::new(),
            flow_id: None,
            contact_id: None,
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: updated_at.into(),
        }
    }

    #[test]
    fn a_correction_changes_the_kind_without_touching_the_event() {
        let events = vec![event("ev-1")];
        let mut fix = correction("ov-1", "ev-1", "2026-03-02T00:00:00Z");
        fix.kind = Some(CryptoTxKind::Stake);
        let fixes = [fix];
        let views = apply(&events, &fixes);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].kind, CryptoTxKind::Stake);
        // The stored event is untouched: a refetch rewrites it and the
        // correction has to survive that.
        assert_eq!(events[0].kind, CryptoTxKind::Transfer);
        assert_eq!(views[0].event.kind, CryptoTxKind::Transfer);
    }

    #[test]
    fn a_hidden_operation_leaves_the_calculation_entirely() {
        let events = vec![event("ev-1"), event("ev-2")];
        let mut fix = correction("ov-1", "ev-2", "2026-03-02T00:00:00Z");
        fix.hidden = true;
        let fixes = [fix];
        let views = apply(&events, &fixes);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].event.id, "ev-1");
    }

    #[test]
    fn two_devices_that_both_corrected_one_operation_resolve_the_same_way() {
        let events = vec![event("ev-1")];
        let mut older = correction("ov-a", "ev-1", "2026-03-02T00:00:00Z");
        older.kind = Some(CryptoTxKind::Stake);
        let mut newer = correction("ov-b", "ev-1", "2026-03-03T00:00:00Z");
        newer.kind = Some(CryptoTxKind::Claim);
        let one_order = [older.clone(), newer.clone()];
        let other_order = [newer, older];
        let one_way = apply(&events, &one_order);
        let other_way = apply(&events, &other_order);
        assert_eq!(one_way[0].kind, CryptoTxKind::Claim);
        assert_eq!(other_way[0].kind, CryptoTxKind::Claim);

        // Same moment on two devices: the id decides, so both agree.
        let mut tie_a = correction("ov-a", "ev-1", "2026-03-02T00:00:00Z");
        tie_a.kind = Some(CryptoTxKind::Stake);
        let mut tie_b = correction("ov-b", "ev-1", "2026-03-02T00:00:00Z");
        tie_b.kind = Some(CryptoTxKind::Claim);
        let one_order = [tie_a.clone(), tie_b.clone()];
        let other_order = [tie_b, tie_a];
        assert_eq!(
            apply(&events, &one_order)[0].kind,
            apply(&events, &other_order)[0].kind
        );
    }

    #[test]
    fn the_chain_and_the_counterparty_of_a_correction_reach_the_calculation() {
        let events = vec![event("ev-1")];
        let mut fix = correction("ov-1", "ev-1", "2026-03-02T00:00:00Z");
        fix.flow_id = Some("flow-7".into());
        fix.contact_id = Some("contact-3".into());
        let fixes = [fix];
        let views = apply(&events, &fixes);
        assert_eq!(views[0].flow_id, Some("flow-7"));
        assert_eq!(views[0].contact_id, Some("contact-3"));
    }

    #[test]
    fn an_operation_with_no_correction_is_itself_and_the_order_is_fixed() {
        let events = vec![event("ev-2"), event("ev-1")];
        let views = apply(&events, &[]);
        let ids: Vec<_> = views.iter().map(|view| view.event.id.as_str()).collect();
        assert_eq!(ids, vec!["ev-1", "ev-2"]);
        assert_eq!(views[0].kind, CryptoTxKind::Transfer);
        assert_eq!(views[0].flow_id, None);
    }

    #[test]
    fn a_correction_of_an_operation_that_is_gone_changes_nothing() {
        let events = vec![event("ev-1")];
        let orphans = [correction("ov-1", "ev-404", "2026-03-02T00:00:00Z")];
        assert_eq!(apply(&events, &orphans).len(), 1);
    }
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoDirection, CryptoLeg, TokenId, TrackingRef};

    fn event(id: &str) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: "2026-03-01T10:00:00Z".into(),
            block: None,
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xUSDT")),
                amount: crate::Amount::from(1),
                counterparty: None,
            }],
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{id}"),
            },
        }
    }

    fn rule(key: &str) -> CryptoOverride {
        CryptoOverride {
            id: format!("crule:{key}"),
            event_id: rule_event_id(key),
            kind: Some(CryptoTxKind::Claim),
            comment: None,
            categories: Vec::new(),
            flow_id: None,
            contact_id: Some("c1".into()),
            not_exchange: false,
            hidden: false,
            fiat_tx_id: None,
            updated_at: "2026-03-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn a_rule_reaches_an_operation_the_engine_reads_later_and_a_correction_stands_above_it() {
        let key = contract_key(&event("x")).unwrap();
        assert_eq!(key, "ethereum|0xusdt");
        let rules = vec![rule(&key)];
        let events = [event("later")];
        let views = apply(&events, &rules);
        assert_eq!(views[0].kind, CryptoTxKind::Claim);
        assert_eq!(views[0].contact_id, Some("c1"));
        let mut own = rules.clone();
        own.push(CryptoOverride {
            id: "cov:later".into(),
            event_id: "later".into(),
            kind: Some(CryptoTxKind::Stake),
            ..rule(&key)
        });
        own[1].contact_id = None;
        let views = apply(&events, &own);
        assert_eq!(views[0].kind, CryptoTxKind::Stake);
        assert_eq!(
            views[0].contact_id,
            Some("c1"),
            "a field the correction leaves empty falls to the rule"
        );
    }
}
