//! The shared shape of a DeFi position: what every adapter answers with
//! (spec D0).
//!
//! One shape has to fit a Uniswap v3 range (two tokens, fees kept apart from
//! the principal), a Uniswap v4 pool (the same, and approximate when a hook
//! is in the way), an Aave v3 reserve (a supply, a debt, interest inside the
//! balance) and Lido (one token whose balance grows by itself). What they
//! have in common is small: a key, a name, legs, fees, and a handful of
//! facts. Everything that is *money* is computed afterwards by
//! [`super::fees`] out of these quantities and the prices the vault knows,
//! because an adapter has no prices and no clock.
//!
//! # What is durable and what is not
//!
//! [`crate::model::cryptotrack::CryptoPosition`] — `legs` and the name — is
//! the only part that is stored, and the vault schema does not change for
//! DeFi ([`crate::model::cryptotrack`], spec E0). A reading's other facts
//! (the uncollected fees, [`PositionReading::approximate`], the kind) are a
//! reading of a moment, exactly like the balances a node reports: the device
//! that talked to the chain keeps the latest one in memory and hands it to
//! the screen, and a device that has only synced shows what the stored legs
//! and the stored operations can say. Nothing derived is stored, so nothing
//! derived can go stale in the vault.
//!
//! # Where a position's history comes from
//!
//! [`super::ProtocolAdapter::events`] answers with ordinary operations
//! ([`CryptoTx`]) whose [`TrackingRef::Position`] names this position. That
//! is what makes "invested at the price of the deposit", the collected fees
//! and the gas computable from the document alone, with no second storage
//! shape and no second price path.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoLeg, CryptoPosition, TrackingRef, chain_slug,
};

use super::fees::FeeReading;

/// A quantity of one asset, in whole units (never raw integer units: money is
/// [`Amount`], `AGENTS.md` rule 2).
#[derive(Debug, Clone, PartialEq)]
pub struct AssetAmount {
    pub asset: CryptoAsset,
    pub amount: Amount,
}

/// Which position, in the protocol's own terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionKey {
    /// [`super::ProtocolAdapter::protocol`]: `uniswap-v3`, `aave-v3`, …
    pub protocol: String,
    pub chain: Chain,
    /// What the protocol calls this position: an NFT token id, a reserve's
    /// contract, the staking token's contract.
    pub id: String,
}

impl PositionKey {
    pub fn new(protocol: &str, chain: Chain, id: &str) -> Self {
        Self {
            protocol: protocol.to_string(),
            chain,
            id: id.to_string(),
        }
    }

    /// The record key of `doc.crypto_positions`:
    /// `<account_id>|<protocol>|<chain>|<id>`. Deterministic, so two devices
    /// that read the same position of the same account hold one record and
    /// the sync has nothing to merge.
    ///
    /// The account is part of it because [`PositionKey::id`] is only unique
    /// within a protocol, not within the vault: an Aave reserve or the stETH
    /// contract is the same id for every followed address that holds it, and
    /// without the account two of them would share one record and overwrite
    /// each other's legs.
    pub fn record_id(&self, account_id: &str) -> String {
        format!("{account_id}|{}", self.record_tail())
    }

    /// What the record key was before the account joined it:
    /// `<protocol>|<chain>|<id>`. Only for recognising a record an older
    /// build wrote ([`record_id_prefixes`]); nothing is written under it.
    pub fn legacy_record_id(&self) -> String {
        self.record_tail()
    }

    fn record_tail(&self) -> String {
        format!("{}|{}|{}", self.protocol, chain_slug(self.chain), self.id)
    }

    /// What the operations of this position are found under, and what the
    /// screen links a row to (spec D5).
    pub fn tracking(&self) -> TrackingRef {
        TrackingRef::Position {
            protocol: self.protocol.clone(),
            chain: self.chain,
            id: self.id.clone(),
        }
    }
}

/// The two prefixes a stored position's record id can start with, current
/// first: `<account_id>|<protocol>|<chain>|` and the older
/// `<protocol>|<chain>|`. What follows is the protocol's own id, which may
/// itself contain a `|`.
pub fn record_id_prefixes(record: &CryptoPosition) -> (String, String) {
    let legacy = format!("{}|{}|", record.protocol, chain_slug(record.chain));
    (format!("{}|{legacy}", record.account_id), legacy)
}

/// What sort of position it is. This is what decides whether impermanent
/// loss against holding means anything at all.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CryptoPositionKind")]
pub enum PositionKind {
    /// Two or more assets in a pool, rebalanced by the market: the one case
    /// where holding is a different outcome, and so the one case with an
    /// impermanent loss.
    Pool,
    /// Supplied and borrowed: the value is the collateral minus the debt, and
    /// there is nothing to compare a one-sided position against.
    #[default]
    Lending,
    /// One asset whose balance or rate grows (stETH, wstETH). One-sided too.
    Staking,
}

impl PositionKind {
    /// Whether an impermanent loss against holding is a number for this kind.
    /// A one-sided position has nothing to compare against, so the screen
    /// shows a dash rather than a zero (decision of the D-stage mockups).
    pub fn compares_with_holding(self) -> bool {
        self == PositionKind::Pool
    }

    /// Whether the protocol keeps the fees it earned apart from the
    /// position's principal. Only a pool does (Uniswap v3 and v4 report the
    /// owed fees outside the legs); lending and staking accrue inside the
    /// balance itself, which is why their uncollected fees are a real zero
    /// rather than an unknown on a device that has not read the chain.
    pub fn keeps_fees_apart(self) -> bool {
        self == PositionKind::Pool
    }
}

/// Whether the position is still open.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "CryptoPositionStatus")]
pub enum PositionStatus {
    #[default]
    Open,
    /// Everything was taken out. Its figures are frozen — every operation of
    /// it is in the past — and it adds nothing to the portfolio's value, but
    /// it stays in the result of a period it was open in.
    Closed,
}

/// What the legs say the position is: more than one asset supplied is a pool,
/// a borrowed leg makes it lending, one supplied asset is staking.
///
/// Used for a position a device only has the stored legs of; a live reading
/// says it itself, because only the adapter knows a single-sided pool from a
/// supply.
pub fn kind_of(legs: &[CryptoLeg]) -> PositionKind {
    if legs.iter().any(|leg| leg.direction == CryptoDirection::Out) {
        return PositionKind::Lending;
    }
    let supplied = legs
        .iter()
        .filter(|leg| leg.direction == CryptoDirection::In)
        .count();
    if supplied > 1 {
        PositionKind::Pool
    } else {
        PositionKind::Staking
    }
}

/// Whether nothing is left in the position: every leg is gone or zero.
pub fn is_closed(legs: &[CryptoLeg]) -> bool {
    legs.iter().all(|leg| leg.amount.is_zero())
}

/// What one protocol's discovery said about an account on one network, once
/// it *succeeded*: which of the positions it still sees.
///
/// This is the only thing that lets the vault tell "the protocol no longer
/// reports this position" from "the pass did not manage to ask". A protocol
/// that refused outright has no review at all, and a position whose own read
/// refused stays in [`ProtocolReview::kept`] — a refusal says nothing about
/// the position, so it must not be taken for a withdrawal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolReview {
    /// [`super::ProtocolAdapter::protocol`].
    pub protocol: String,
    pub chain: Chain,
    /// Record ids ([`PositionKey::record_id`], with this account) of the positions that are not
    /// gone: read, or found and not readable right now.
    pub kept: BTreeSet<String>,
    /// When the pass took the review (RFC3339 UTC): a position closed by it
    /// is stamped with this.
    pub read_at: String,
}

/// One position as an adapter read it: quantities and facts, no money.
///
/// Built with every field named (there is no `Default`), so a new adapter
/// cannot forget [`PositionReading::approximate`] — the screen never decides
/// approximation on its own, it only shows what the adapter said.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionReading {
    pub key: PositionKey,
    /// The followed account that holds it.
    pub account_id: String,
    /// What the protocol calls it, for the list.
    pub name: String,
    pub kind: PositionKind,
    pub status: PositionStatus,
    /// What the position holds, principal only: a supply is
    /// [`CryptoDirection::In`], a debt [`CryptoDirection::Out`]. Fees the
    /// protocol keeps apart from the principal are not here, they are in
    /// [`PositionReading::fees`].
    pub legs: Vec<CryptoLeg>,
    pub fees: FeeReading,
    /// The tokens this protocol handed over as the receipt for the position:
    /// an aToken, stETH, the position NFT. They sit in the wallet and the
    /// node reports them, so they are named here to be left out of the
    /// capital — the money is already counted as the position
    /// (`service::cryptotrack::positions`).
    pub receipts: Vec<CryptoAsset>,
    /// The numbers are an approximation and the screen must say so — a
    /// Uniswap v4 pool behind a hook, for instance. It comes from the
    /// adapter together with the numbers and is never guessed later.
    pub approximate: bool,
    /// When the position was opened (RFC3339 UTC), when the protocol knows.
    pub opened_at: Option<String>,
    /// When it was closed; `None` while it is open.
    pub closed_at: Option<String>,
    /// When this reading was taken (RFC3339 UTC).
    pub read_at: String,
}

impl PositionReading {
    /// The record key of this reading: the position of this account.
    pub fn record_id(&self) -> String {
        self.key.record_id(&self.account_id)
    }

    /// The durable part of the reading: the record `doc.crypto_positions`
    /// keeps. Nothing derived goes in — no value, no fee, no rate.
    pub fn stored(&self) -> CryptoPosition {
        CryptoPosition {
            id: self.record_id(),
            account_id: self.account_id.clone(),
            chain: self.key.chain,
            protocol: self.key.protocol.clone(),
            name: self.name.clone(),
            legs: self.legs.clone(),
            receipts: self.receipts.clone(),
            updated_at: self.read_at.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::cryptotrack::TokenId;

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect("a test amount parses")
    }

    fn leg(direction: CryptoDirection, quantity: &str) -> CryptoLeg {
        CryptoLeg {
            direction,
            asset: CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xdead")),
            amount: amount(quantity),
            counterparty: None,
        }
    }

    #[test]
    fn a_key_is_one_record_and_one_tracking_reference() {
        let key = PositionKey::new("uniswap-v3", Chain::Arbitrum, "123456");
        assert_eq!(key.record_id("cacc-1"), "cacc-1|uniswap-v3|arbitrum|123456");
        assert_eq!(key.legacy_record_id(), "uniswap-v3|arbitrum|123456");
        assert_ne!(key.record_id("cacc-1"), key.record_id("cacc-2"));
        assert_eq!(
            key.tracking(),
            TrackingRef::Position {
                protocol: "uniswap-v3".into(),
                chain: Chain::Arbitrum,
                id: "123456".into(),
            }
        );
    }

    #[test]
    fn the_legs_say_what_kind_of_position_it_is() {
        let pool = [
            leg(CryptoDirection::In, "1"),
            leg(CryptoDirection::In, "2000"),
        ];
        assert_eq!(kind_of(&pool), PositionKind::Pool);
        assert!(kind_of(&pool).compares_with_holding());

        let debt = [
            leg(CryptoDirection::In, "10000"),
            leg(CryptoDirection::Out, "4000"),
        ];
        assert_eq!(kind_of(&debt), PositionKind::Lending);
        assert!(
            !kind_of(&debt).compares_with_holding(),
            "a one-sided position has nothing to compare against"
        );

        let staked = [leg(CryptoDirection::In, "5")];
        assert_eq!(kind_of(&staked), PositionKind::Staking);
        assert!(!kind_of(&staked).compares_with_holding());
    }

    #[test]
    fn a_position_with_nothing_left_in_it_is_closed() {
        assert!(is_closed(&[]));
        assert!(is_closed(&[leg(CryptoDirection::In, "0")]));
        assert!(!is_closed(&[leg(CryptoDirection::In, "0.000001")]));
    }

    #[test]
    fn the_stored_record_keeps_the_legs_and_nothing_derived() {
        let reading = PositionReading {
            key: PositionKey::new("aave-v3", Chain::Ethereum, "0xbeef"),
            account_id: "cacc-1".into(),
            name: "USDC supply".into(),
            kind: PositionKind::Lending,
            status: PositionStatus::Open,
            legs: vec![leg(CryptoDirection::In, "1000")],
            fees: FeeReading::unknown(),
            receipts: vec![CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xfeed"))],
            approximate: false,
            opened_at: Some("2026-11-01T00:00:00Z".into()),
            closed_at: None,
            read_at: "2026-11-16T10:00:00Z".into(),
        };
        let stored = reading.stored();
        assert_eq!(stored.id, "cacc-1|aave-v3|ethereum|0xbeef");
        assert_eq!(stored.account_id, "cacc-1");
        assert_eq!(stored.protocol, "aave-v3");
        assert_eq!(stored.legs, reading.legs);
        assert_eq!(
            stored.receipts, reading.receipts,
            "the receipt tokens this adapter saw travel with the record: no \
             later reading can tell which they were"
        );
        assert_eq!(stored.updated_at, "2026-11-16T10:00:00Z");
        // The stored shape has exactly the fields E0 gave it plus the
        // additive `receipts` of wave 1 (`#[serde(default)]`, so an older
        // vault still opens): a *derived* figure that got in here would be a
        // vault schema change for nothing.
        let json = serde_json::to_value(&stored).expect("the record serializes");
        let mut keys: Vec<&str> = json
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "account_id",
                "chain",
                "id",
                "legs",
                "name",
                "protocol",
                "receipts",
                "updated_at"
            ]
        );
    }
}
