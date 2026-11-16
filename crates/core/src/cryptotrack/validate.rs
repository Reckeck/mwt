//! What the crypto module refuses to store, whichever way the data arrives.
//!
//! An event whose hash or address is empty cannot be refreshed, deduplicated
//! or shown, and a record whose key is empty cannot even be synced: it must
//! not reach the document. The rules live here once, so the three ways in —
//! a source's answer being applied, a sync batch being received, and the raw
//! JSON editor saving a document — all refuse the same thing.
//!
//! [`validate_section`] is the whole-document check those last two use; the
//! `validate` methods are what an `apply` calls per record.

use crate::error::{Error, Result};
use crate::model::cryptotrack::{
    CryptoAccount, CryptoCursor, CryptoLeg, CryptoOverride, CryptoPosition, CryptoSnapshot,
    CryptoToken, CryptoTx, CryptoWipe, TrackingRef,
};
use crate::model::vault::VaultDoc;

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidInput(message.into())
}

/// A field that identifies a record must not be empty or blank.
fn present(value: &str, what: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(invalid(format!("crypto: {what} must not be empty")));
    }
    Ok(())
}

impl TrackingRef {
    /// Refuse an origin that names nothing: an empty hash, address, position
    /// or fill id cannot be fetched again or deduplicated.
    pub fn validate(&self) -> Result<()> {
        match self {
            TrackingRef::ChainTx { hash, .. } => present(hash, "a transaction hash"),
            TrackingRef::Address { address, .. } => present(address, "an address"),
            TrackingRef::Position { protocol, id, .. } => {
                present(protocol, "a protocol")?;
                present(id, "a position id")
            }
            TrackingRef::ExchangeHistory { exchange, exec_id } => {
                present(exchange, "an exchange")?;
                present(exec_id, "a fill id")
            }
        }
    }
}

impl CryptoLeg {
    pub fn validate(&self) -> Result<()> {
        if let crate::model::cryptotrack::CryptoAsset::Token(token) = &self.asset {
            present(&token.contract, "a token contract")?;
        }
        Ok(())
    }
}

impl CryptoTx {
    /// Refuse an operation that cannot be identified or re-read.
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "an operation id")?;
        present(&self.account_id, "an operation's account")?;
        present(&self.time, "an operation's time")?;
        self.tracking.validate()?;
        for leg in &self.legs {
            leg.validate()?;
        }
        Ok(())
    }
}

impl CryptoAccount {
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "an account id")?;
        present(&self.name, "an account name")?;
        present(&self.address, "an address")
    }
}

impl CryptoToken {
    pub fn validate(&self) -> Result<()> {
        present(&self.contract, "a token contract")?;
        present(&self.symbol, "a token symbol")
    }
}

impl CryptoOverride {
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "a correction id")?;
        present(&self.event_id, "the operation a correction belongs to")
    }
}

impl CryptoPosition {
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "a position id")?;
        present(&self.account_id, "a position's account")?;
        present(&self.protocol, "a protocol")?;
        for leg in &self.legs {
            leg.validate()?;
        }
        Ok(())
    }
}

impl CryptoSnapshot {
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "a snapshot id")?;
        present(&self.at, "a snapshot's time")
    }
}

impl CryptoCursor {
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "a cursor id")?;
        present(&self.source, "a cursor's source")?;
        present(&self.account_id, "a cursor's account")
    }
}

impl CryptoWipe {
    pub fn validate(&self) -> Result<()> {
        present(&self.id, "a wipe id")?;
        present(&self.at, "a wipe's time")
    }
}

/// Every crypto record of a whole document.
///
/// Used where a document arrives at once instead of record by record: a sync
/// batch being received and the raw JSON editor saving.
pub fn validate_section(doc: &VaultDoc) -> Result<()> {
    for account in &doc.crypto_accounts {
        account.validate()?;
    }
    for event in &doc.crypto_events {
        event.validate()?;
    }
    for token in &doc.crypto_tokens {
        token.validate()?;
    }
    for correction in &doc.crypto_overrides {
        correction.validate()?;
    }
    for position in &doc.crypto_positions {
        position.validate()?;
    }
    for snapshot in &doc.crypto_snapshots {
        snapshot.validate()?;
    }
    for cursor in &doc.crypto_cursors {
        cursor.validate()?;
    }
    for wipe in &doc.crypto_wiped_at {
        wipe.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{CryptoAsset, CryptoDirection, CryptoTxKind, TokenId};

    fn event(tracking: TrackingRef) -> CryptoTx {
        CryptoTx {
            id: "ev-1".into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: "2026-11-10T10:00:00Z".into(),
            block: None,
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: Vec::new(),
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking,
        }
    }

    fn chain_tx(hash: &str) -> TrackingRef {
        TrackingRef::ChainTx {
            chain: Chain::Ethereum,
            hash: hash.into(),
        }
    }

    #[test]
    fn an_empty_hash_is_refused() {
        let err = event(chain_tx(" ")).validate().expect_err("an empty hash");
        assert!(matches!(err, Error::InvalidInput(_)), "got {err:?}");
        assert_eq!(err.code(), "INVALID_INPUT");
        assert!(event(chain_tx("0xdead")).validate().is_ok());
    }

    #[test]
    fn an_empty_address_or_fill_is_refused() {
        for tracking in [
            TrackingRef::Address {
                chain: Chain::Ethereum,
                address: String::new(),
            },
            TrackingRef::ExchangeHistory {
                exchange: "bybit".into(),
                exec_id: String::new(),
            },
            TrackingRef::Position {
                protocol: "aave-v3".into(),
                chain: Chain::Ethereum,
                id: String::new(),
            },
        ] {
            assert!(tracking.validate().is_err(), "{tracking:?}");
        }
    }

    #[test]
    fn an_operation_without_an_account_or_a_time_is_refused() {
        let mut orphan = event(chain_tx("0xdead"));
        orphan.account_id = String::new();
        assert!(orphan.validate().is_err());

        let mut timeless = event(chain_tx("0xdead"));
        timeless.time = String::new();
        assert!(timeless.validate().is_err());
    }

    #[test]
    fn a_leg_without_a_contract_is_refused() {
        let mut swap = event(chain_tx("0xdead"));
        swap.legs = vec![CryptoLeg {
            direction: CryptoDirection::In,
            asset: CryptoAsset::Token(TokenId {
                chain: Chain::Ethereum,
                contract: String::new(),
            }),
            amount: crate::Amount::parse("1").unwrap(),
            counterparty: None,
        }];
        assert!(swap.validate().is_err());
    }

    #[test]
    fn a_whole_document_is_checked_record_by_record() {
        let mut doc = VaultDoc::default();
        assert!(validate_section(&doc).is_ok());
        doc.crypto_events.push(event(chain_tx("")));
        assert!(validate_section(&doc).is_err());
    }
}
