//! Raw facts of a network into operations of the module.
//!
//! A source hands over what the explorer said, one fact per row
//! ([`RawEvent`]): the transaction itself, each ERC-20 `Transfer` log, each
//! internal value move. One operation of the module is all the facts of one
//! transaction, seen from one followed address: a swap is a token leaving and
//! another arriving in the same transaction, and the gas is what the sender
//! paid for it.
//!
//! Three rules decide everything here.
//!
//! 1. **Nothing is invented.** A leg exists only where the followed address
//!    is one of the two sides. A fact of the same transaction that does not
//!    touch the address (the explorer lists a transaction for many reasons)
//!    adds no leg.
//! 2. **Nothing is dropped.** An operation the module cannot read is still an
//!    operation: it gets [`CryptoTxKind::Unknown`], keeps the source's own
//!    answer in `raw`, and the reason comes back in [`Normalized::unread`]
//!    for the screen to show. Silence is the one outcome that is not allowed.
//! 3. **A doubtful quantity poisons the whole operation, not one leg.** If
//!    one leg of a swap cannot be read exactly, keeping the other would
//!    invent a purchase with no sale behind it. All the legs go, the
//!    operation stays marked, and the holding it would have changed is caught
//!    by the reconciliation against the network's own balance
//!    ([`super::reconcile`]).
//!
//! What a kind is decided from is the *shape* of the legs and nothing else.
//! Deposits into a protocol and exits from it (`Stake`, `Unstake`, `Claim`)
//! need to know the protocol behind an address, which is the DeFi stage's
//! adapters and the user's own correction; the engine does not guess them
//! from a method selector it cannot verify. It does handle them once they are
//! set (see [`super::basis`]).

use std::collections::{BTreeMap, BTreeSet};

use crate::chains;
use crate::sources::evm::{RawAsset, RawEvent, RawKind};
use mwt_base::Amount;
use mwt_model::Chain;
use mwt_model::{
    CryptoAsset, CryptoDirection, CryptoFee, CryptoLeg, CryptoTx, CryptoTxKind, TokenId,
    TrackingRef,
};

use super::ids;

/// The address the facts are read for.
#[derive(Debug, Clone, Copy)]
pub struct Followed<'who> {
    pub account_id: &'who str,
    pub chain: Chain,
    /// The account's address, already normalized.
    pub address: &'who str,
}

/// An operation that could not be read in full, and why. Not stored: the
/// event itself carries the mark, this is the explanation beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    pub event_id: String,
    pub reason: String,
}

/// What [`normalize`] made of a batch.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Normalized {
    /// Ordered by `(time, id)`, so two devices that saw the same facts hold
    /// the same bytes whatever order the pages arrived in.
    pub events: Vec<CryptoTx>,
    pub unread: Vec<Unread>,
    /// Ids of the events whose batch held the transaction fact itself: only
    /// those speak for the sender, the recipient, the method and the gas.
    pub anchored: BTreeSet<String>,
    /// Ids of the events with a doubtful quantity (rule 3).
    pub poisoned: BTreeSet<String>,
}

/// How an operation arrived, for [`combine`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Arrival {
    pub anchored: bool,
    pub poisoned: bool,
}

/// Facts of one network into operations of one followed address.
pub fn normalize(raw: &[RawEvent], who: Followed<'_>) -> Normalized {
    let mut groups: BTreeMap<&str, Vec<&RawEvent>> = BTreeMap::new();
    for fact in raw {
        groups.entry(fact.hash.as_str()).or_default().push(fact);
    }
    let mut out = Normalized::default();
    for (hash, mut facts) in groups {
        facts.sort_by_key(|fact| (fact.block, rank(fact.kind), position(fact), fact.id.clone()));
        let unread_before = out.unread.len();
        let event = operation(hash, &facts, who, &mut out.unread);
        if facts.iter().any(|fact| fact.kind == RawKind::Transaction) {
            out.anchored.insert(event.id.clone());
        }
        if out.unread[unread_before..]
            .iter()
            .any(|entry| entry.reason.starts_with("a quantity could not be read"))
        {
            out.poisoned.insert(event.id.clone());
        }
        out.events.push(event);
    }
    out.events
        .sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
    out
}

/// Which list a fact came from, so legs of one operation keep one order
/// however the pages arrived: the transaction, then its internal moves, then
/// its token transfers.
fn rank(kind: RawKind) -> u8 {
    match kind {
        RawKind::Transaction => 0,
        RawKind::InternalTransfer => 1,
        RawKind::TokenTransfer => 2,
    }
}

/// The index inside the transaction, from the tail of the fact's id
/// (`…:{logIndex}`, `…:i{index}`, `…:tx`): a number, so leg 10 follows leg 9
/// instead of sorting as text.
fn position(fact: &RawEvent) -> u64 {
    fact.id
        .rsplit(':')
        .next()
        .map(|tail| tail.trim_start_matches('i'))
        .and_then(|tail| tail.parse::<u64>().ok())
        .unwrap_or(0)
}

fn operation(
    hash: &str,
    facts: &[&RawEvent],
    who: Followed<'_>,
    unread: &mut Vec<Unread>,
) -> CryptoTx {
    let tracking = TrackingRef::ChainTx {
        chain: who.chain,
        hash: hash.to_owned(),
    };
    let id = ids::event(who.account_id, &tracking);
    let transaction = facts.iter().find(|fact| fact.kind == RawKind::Transaction);
    let first = facts.first().copied().expect("a group holds a fact");
    let anchor = transaction.copied().unwrap_or(first);

    let mut legs = Vec::new();
    let mut doubtful: Option<String> = None;
    for fact in facts {
        match leg(fact, who) {
            Ok(Some(found)) => legs.push(found),
            Ok(None) => {}
            Err(reason) => doubtful = Some(doubtful.take().unwrap_or(reason)),
        }
    }
    if let Some(reason) = &doubtful {
        // Rule 3: an operation with one unreadable leg keeps none of them.
        legs.clear();
        unread.push(Unread {
            event_id: id.clone(),
            reason: reason.clone(),
        });
    }
    let legs = merge(legs);

    let gas = transaction.and_then(|fact| gas_of(fact, who));
    let kind = if doubtful.is_some() {
        CryptoTxKind::Unknown
    } else {
        shape(&legs, gas.is_some(), anchor.method.is_some())
    };
    if kind == CryptoTxKind::Unknown && doubtful.is_none() {
        unread.push(Unread {
            event_id: id.clone(),
            reason: if anchor.succeeded {
                "nothing the module can read moved".into()
            } else {
                "the transaction was reverted".into()
            },
        });
    }
    CryptoTx {
        id,
        chain: who.chain,
        account_id: who.account_id.to_owned(),
        time: facts
            .iter()
            .map(|fact| fact.time.as_str())
            .min()
            .unwrap_or("")
            .to_owned(),
        block: Some(anchor.block),
        from: anchor.from.clone(),
        to: anchor.to.clone(),
        legs,
        gas,
        protocol: None,
        method: anchor.method.clone(),
        kind,
        raw: Some(anchor.payload.clone()),
        tracking,
    }
}

/// The leg this fact is for the followed address: `Ok(None)` when it moved
/// nothing of the address's, `Err` when the quantity cannot be read exactly.
fn leg(fact: &RawEvent, who: Followed<'_>) -> std::result::Result<Option<CryptoLeg>, String> {
    // A reverted transaction paid its gas and moved nothing.
    if !fact.succeeded || fact.value.is_zero() {
        return Ok(None);
    }
    let inbound = fact.to == who.address;
    let outbound = fact.from == who.address;
    // Neither side, or both (a send to oneself): no change of the holding.
    if inbound == outbound {
        return Ok(None);
    }
    let amount = fact
        .amount()
        .map_err(|err| format!("a quantity could not be read exactly: {err}"))?;
    Ok(Some(CryptoLeg {
        direction: if inbound {
            CryptoDirection::In
        } else {
            CryptoDirection::Out
        },
        asset: asset_of(&fact.asset, who.chain),
        amount,
        counterparty: Some(if inbound {
            fact.from.clone()
        } else {
            fact.to.clone()
        }),
    }))
}

fn asset_of(asset: &RawAsset, chain: Chain) -> CryptoAsset {
    match asset {
        RawAsset::Native { .. } => CryptoAsset::Native { chain },
        RawAsset::Token { contract, .. } => CryptoAsset::Token(TokenId::new(chain, contract)),
    }
}

/// What the network charged, for the sender of the transaction only.
fn gas_of(fact: &RawEvent, who: Followed<'_>) -> Option<CryptoFee> {
    if fact.from != who.address {
        return None;
    }
    let decimals = chains::evm_chain(who.chain)
        .map(|row| row.native.decimals)
        .unwrap_or(18);
    let paid = fact.fee.as_ref()?.scaled(decimals).ok()?;
    (!paid.is_zero()).then_some(CryptoFee {
        asset: CryptoAsset::Native { chain: who.chain },
        amount: paid,
    })
}

/// Legs of one asset in one direction are one leg: a swap through a router
/// splits into several `Transfer` logs of the same token, and a refetch may
/// order them differently.
fn merge(legs: Vec<CryptoLeg>) -> Vec<CryptoLeg> {
    let mut out: Vec<CryptoLeg> = Vec::with_capacity(legs.len());
    for leg in legs {
        match out
            .iter_mut()
            .find(|have| have.direction == leg.direction && have.asset == leg.asset)
        {
            Some(have) => {
                have.amount = super::math::add(have.amount, leg.amount).unwrap_or(have.amount);
                if have.counterparty != leg.counterparty {
                    have.counterparty = None;
                }
            }
            None => out.push(leg),
        }
    }
    out
}

/// The kind the shape of the legs gives. A protocol kind (`Stake`,
/// `Unstake`, `Claim`) is never guessed here: it comes from an adapter or
/// from the user's correction.
fn shape(legs: &[CryptoLeg], paid_gas: bool, called_a_method: bool) -> CryptoTxKind {
    let inbound = legs
        .iter()
        .filter(|leg| leg.direction == CryptoDirection::In)
        .count();
    let outbound = legs.len() - inbound;
    match (inbound, outbound) {
        (0, 0) if paid_gas && called_a_method => CryptoTxKind::Contract,
        (0, 0) => CryptoTxKind::Unknown,
        (_, 0) | (0, _) => CryptoTxKind::Transfer,
        _ => CryptoTxKind::Swap,
    }
}

/// Folds an operation read from one batch into the stored one of the same
/// id. The explorer lists a transaction in several lists, and a long one can
/// straddle a batch boundary, so one batch rarely holds every fact of it:
/// replacing the stored operation would erase what the other batches found
/// (the gas, the leg of the other list).
///
/// The merge is idempotent and does not depend on the order of the batches:
/// chain facts never change, so a leg only grows as more facts are seen. A
/// leg of one direction and asset keeps the larger quantity seen (a re-read
/// of the same facts, or of a different slice of them, never adds a leg
/// twice); gas known wins; the kind is derived again from the merged shape.
/// What it cannot do: two logs of one token and direction on both sides of a
/// boundary give the larger of the two, not their sum; the reconciliation
/// against the node's balance adjusts that remainder.
pub fn combine(have: &mut CryptoTx, incoming: CryptoTx, arrival: Arrival) {
    if arrival.poisoned {
        // Rule 3: a doubtful quantity poisons the whole operation.
        *have = incoming;
        return;
    }
    for leg in incoming.legs {
        match have
            .legs
            .iter_mut()
            .find(|seen| seen.direction == leg.direction && seen.asset == leg.asset)
        {
            Some(seen) => {
                if seen.counterparty != leg.counterparty {
                    seen.counterparty = None;
                }
                if leg.amount > seen.amount {
                    seen.amount = leg.amount;
                }
            }
            None => have.legs.push(leg),
        }
    }
    have.legs
        .sort_by_key(|leg| (leg.direction == CryptoDirection::In, leg.asset.key()));
    if incoming.gas.is_some() {
        have.gas = incoming.gas;
    }
    if arrival.anchored {
        have.block = incoming.block;
        have.from = incoming.from;
        have.to = incoming.to;
        have.method = incoming.method;
        have.raw = incoming.raw;
    } else if have.method.is_none() {
        have.method = incoming.method;
    }
    if incoming.time < have.time {
        have.time = incoming.time;
    }
    if have.protocol.is_none() {
        have.protocol = incoming.protocol;
    }
    if matches!(
        have.kind,
        CryptoTxKind::Transfer
            | CryptoTxKind::Swap
            | CryptoTxKind::Contract
            | CryptoTxKind::Unknown
    ) || have.legs.is_empty()
    {
        have.kind = shape(&have.legs, have.gas.is_some(), have.method.is_some());
    }
}

/// The quantity of one asset an operation moved, `In` positive.
pub fn net_of(event: &CryptoTx, asset: &CryptoAsset) -> Amount {
    event
        .legs
        .iter()
        .filter(|leg| leg.asset == *asset)
        .fold(Amount::ZERO, |sum, leg| match leg.direction {
            CryptoDirection::In => super::math::add(sum, leg.amount).unwrap_or(sum),
            CryptoDirection::Out => super::math::sub(sum, leg.amount).unwrap_or(sum),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::evm::Uint256;
    use crate::validate::Validate;

    const ME: &str = "0x1111111111111111111111111111111111111111";
    const OTHER: &str = "0x2222222222222222222222222222222222222222";
    const ROUTER: &str = "0x3333333333333333333333333333333333333333";
    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const HASH: &str = "0xaaaa000000000000000000000000000000000000000000000000000000000001";

    fn who() -> Followed<'static> {
        Followed {
            account_id: "acc-1",
            chain: Chain::Ethereum,
            address: ME,
        }
    }

    fn units(text: &str) -> Uint256 {
        Uint256::from_decimal(text).expect("a test quantity")
    }

    fn fact(
        kind: RawKind,
        tail: &str,
        from: &str,
        to: &str,
        asset: RawAsset,
        value: Uint256,
    ) -> RawEvent {
        RawEvent {
            id: format!("evm:1:{HASH}:{tail}"),
            chain_id: 1,
            hash: HASH.into(),
            kind,
            block: 21_000_000,
            time: "2026-03-01T10:00:00Z".into(),
            from: from.into(),
            to: to.into(),
            asset,
            value,
            method: None,
            succeeded: true,
            fee: None,
            payload: "{}".into(),
        }
    }

    fn native() -> RawAsset {
        RawAsset::Native { decimals: 18 }
    }

    fn token(contract: &str, decimals: u8) -> RawAsset {
        RawAsset::Token {
            contract: contract.into(),
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals,
            suspect: false,
        }
    }

    /// The transaction row of a swap: no value, a method, a paid fee.
    fn swap_transaction() -> RawEvent {
        let mut row = fact(RawKind::Transaction, "tx", ME, ROUTER, native(), units("0"));
        row.method = Some("0x04e45aaf".into());
        row.fee = Some(units("2100000000000000"));
        row
    }

    fn swap_facts() -> Vec<RawEvent> {
        vec![
            swap_transaction(),
            // 0.5 ETH leaves (the router unwraps it), 1800 USDC arrives.
            fact(
                RawKind::InternalTransfer,
                "i0",
                ME,
                ROUTER,
                native(),
                units("500000000000000000"),
            ),
            fact(
                RawKind::TokenTransfer,
                "7",
                ROUTER,
                ME,
                token(USDC, 6),
                units("1800000000"),
            ),
        ]
    }

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount")
    }

    #[test]
    fn a_swap_is_one_operation_with_two_legs_and_its_gas() {
        let made = normalize(&swap_facts(), who());
        assert_eq!(made.unread, Vec::new());
        assert_eq!(made.events.len(), 1);
        let event = &made.events[0];
        assert_eq!(
            event.id,
            "cev:acc-1:ethereum:0xaaaa000000000000000000000000000000000000000000000000000000000001"
        );
        assert_eq!(event.kind, CryptoTxKind::Swap);
        assert_eq!(event.legs.len(), 2);
        assert_eq!(event.legs[0].direction, CryptoDirection::Out);
        assert_eq!(event.legs[0].amount, amount("0.5"));
        assert_eq!(event.legs[1].direction, CryptoDirection::In);
        assert_eq!(event.legs[1].amount, amount("1800"));
        assert_eq!(
            event.gas.as_ref().map(|fee| fee.amount),
            Some(amount("0.0021"))
        );
        assert!(event.validate().is_ok());
    }

    #[test]
    fn the_same_facts_in_any_order_give_the_same_operation() {
        let straight = normalize(&swap_facts(), who());
        let mut shuffled = swap_facts();
        shuffled.reverse();
        let reversed = normalize(&shuffled, who());
        assert_eq!(straight, reversed);
    }

    #[test]
    fn gas_is_charged_to_the_sender_only() {
        let mut incoming = swap_transaction();
        incoming.from = OTHER.into();
        incoming.to = ME.into();
        let facts = vec![
            incoming,
            fact(
                RawKind::TokenTransfer,
                "1",
                OTHER,
                ME,
                token(USDC, 6),
                units("5000000"),
            ),
        ];
        let event = &normalize(&facts, who()).events[0];
        assert_eq!(event.gas, None);
        assert_eq!(event.kind, CryptoTxKind::Transfer);
        assert_eq!(event.legs.len(), 1);
        assert_eq!(event.legs[0].direction, CryptoDirection::In);
        assert_eq!(event.legs[0].amount, amount("5"));
    }

    #[test]
    fn a_reverted_transaction_keeps_its_gas_and_moves_nothing() {
        let mut reverted = fact(
            RawKind::Transaction,
            "tx",
            ME,
            OTHER,
            native(),
            units("1000000000000000000"),
        );
        reverted.succeeded = false;
        reverted.fee = Some(units("2100000000000000"));
        reverted.method = Some("0x095ea7b3".into());
        let made = normalize(&[reverted], who());
        let event = &made.events[0];
        assert!(event.legs.is_empty(), "a reverted value did not move");
        assert_eq!(
            event.gas.as_ref().map(|fee| fee.amount),
            Some(amount("0.0021"))
        );
        assert_eq!(event.kind, CryptoTxKind::Contract);
        assert_eq!(made.unread, Vec::new());
    }

    #[test]
    fn an_operation_the_module_cannot_read_is_kept_and_marked() {
        // A contract call that moved nothing the module understands, with no
        // fee reported either: visible, marked, not dropped.
        let opaque = fact(RawKind::Transaction, "tx", ME, ROUTER, native(), units("0"));
        let made = normalize(&[opaque], who());
        assert_eq!(made.events.len(), 1);
        assert_eq!(made.events[0].kind, CryptoTxKind::Unknown);
        assert_eq!(made.unread.len(), 1);
        assert_eq!(made.unread[0].event_id, made.events[0].id);
        assert!(made.events[0].raw.is_some(), "the source's answer is kept");
    }

    #[test]
    fn one_unreadable_quantity_voids_every_leg_of_that_operation() {
        // 10^30 units of a token with no decimals does not fit an exact
        // amount. Keeping the other leg would invent a purchase.
        let junk = fact(
            RawKind::TokenTransfer,
            "2",
            ROUTER,
            ME,
            token("0x9999999999999999999999999999999999999999", 0),
            units("1000000000000000000000000000000"),
        );
        let mut facts = swap_facts();
        facts.push(junk);
        let made = normalize(&facts, who());
        assert_eq!(made.events.len(), 1);
        assert!(made.events[0].legs.is_empty());
        assert_eq!(made.events[0].kind, CryptoTxKind::Unknown);
        assert_eq!(made.unread.len(), 1);
        assert!(
            made.unread[0].reason.contains("exactly"),
            "{:?}",
            made.unread[0]
        );
        // The gas was still paid, and is still known.
        assert!(made.events[0].gas.is_some());
    }

    #[test]
    fn a_send_to_oneself_moves_nothing_and_is_still_listed() {
        let mut self_send = fact(
            RawKind::Transaction,
            "tx",
            ME,
            ME,
            native(),
            units("1000000000000000000"),
        );
        self_send.fee = Some(units("2100000000000000"));
        let made = normalize(&[self_send], who());
        assert_eq!(made.events.len(), 1);
        assert!(made.events[0].legs.is_empty());
        assert_eq!(made.events[0].kind, CryptoTxKind::Unknown);
        assert!(made.events[0].gas.is_some());
    }

    #[test]
    fn a_transaction_that_does_not_touch_the_address_adds_no_leg() {
        let foreign = fact(
            RawKind::TokenTransfer,
            "3",
            OTHER,
            ROUTER,
            token(USDC, 6),
            units("1000000"),
        );
        let made = normalize(&[foreign], who());
        assert!(made.events[0].legs.is_empty());
        assert_eq!(made.events[0].kind, CryptoTxKind::Unknown);
    }

    #[test]
    fn legs_of_one_asset_in_one_direction_are_merged() {
        let facts = vec![
            fact(
                RawKind::TokenTransfer,
                "1",
                ROUTER,
                ME,
                token(USDC, 6),
                units("1000000"),
            ),
            fact(
                RawKind::TokenTransfer,
                "2",
                OTHER,
                ME,
                token(USDC, 6),
                units("2500000"),
            ),
        ];
        let event = &normalize(&facts, who()).events[0];
        assert_eq!(event.legs.len(), 1);
        assert_eq!(event.legs[0].amount, amount("3.5"));
        // Two different senders: no single counterparty to name.
        assert_eq!(event.legs[0].counterparty, None);
    }

    #[test]
    fn leg_ten_follows_leg_nine_instead_of_sorting_as_text() {
        let mut facts = Vec::new();
        for (index, contract) in [(9u64, "0x99"), (10, "0xaa")] {
            let mut row = fact(
                RawKind::TokenTransfer,
                &index.to_string(),
                ROUTER,
                ME,
                token(
                    &format!("{contract}99999999999999999999999999999999999999"),
                    18,
                ),
                units("1000000000000000000"),
            );
            row.id = format!("evm:1:{HASH}:{index}");
            facts.push(row);
        }
        facts.reverse();
        let event = &normalize(&facts, who()).events[0];
        let contracts: Vec<_> = event
            .legs
            .iter()
            .map(|leg| match &leg.asset {
                CryptoAsset::Token(token) => token.contract.clone(),
                CryptoAsset::Native { .. } => "native".into(),
            })
            .collect();
        assert_eq!(contracts[0], "0x9999999999999999999999999999999999999999");
    }

    #[test]
    fn two_followed_addresses_of_one_transfer_get_one_operation_each() {
        let mut row = fact(
            RawKind::Transaction,
            "tx",
            ME,
            OTHER,
            native(),
            units("1000000000000000000"),
        );
        row.fee = Some(units("2100000000000000"));
        let sender = normalize(std::slice::from_ref(&row), who());
        let receiver = normalize(
            std::slice::from_ref(&row),
            Followed {
                account_id: "acc-2",
                chain: Chain::Ethereum,
                address: OTHER,
            },
        );
        assert_ne!(sender.events[0].id, receiver.events[0].id);
        assert_eq!(sender.events[0].tracking, receiver.events[0].tracking);
        assert_eq!(sender.events[0].legs[0].direction, CryptoDirection::Out);
        assert_eq!(receiver.events[0].legs[0].direction, CryptoDirection::In);
        assert!(receiver.events[0].gas.is_none(), "the receiver paid no gas");
        assert_eq!(
            net_of(
                &sender.events[0],
                &CryptoAsset::Native {
                    chain: Chain::Ethereum
                }
            ),
            amount("-1")
        );
        assert_eq!(
            net_of(
                &receiver.events[0],
                &CryptoAsset::Native {
                    chain: Chain::Ethereum
                }
            ),
            amount("1")
        );
    }

    #[test]
    fn operations_come_back_in_time_order_whatever_the_pages_did() {
        let mut later = fact(
            RawKind::Transaction,
            "tx",
            ME,
            OTHER,
            native(),
            units("1000000000000000000"),
        );
        later.hash = format!("0xbbbb{}", &HASH[6..]);
        later.id = format!("evm:1:{}:tx", later.hash);
        later.time = "2026-03-02T10:00:00Z".into();
        let facts = vec![later.clone(), swap_transaction()];
        let made = normalize(&facts, who());
        assert_eq!(made.events.len(), 2);
        assert_eq!(made.events[0].time, "2026-03-01T10:00:00Z");
        assert_eq!(made.events[1].time, "2026-03-02T10:00:00Z");
    }
}
