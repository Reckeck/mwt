//! What the frozen conversion of the old Bybit queue took from the live crypto module, copied.
//!
//! Frozen conversions do not follow live code: these are the helpers as they were when the v4
//! transition was cut off (the exchange-coin encoding, the event of a staged record, the
//! checked decimal math, the id of an event). A later change of the live module must not change
//! what an old file converts to, so nothing here names `cryptotrack` or `exchange`.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoDirection, CryptoFee, CryptoLeg, CryptoTx, CryptoTxKind,
    TokenId, TrackingRef, chain_slug,
};
use crate::model::vault::{ACCOUNT_EARN, ACCOUNT_UNIFIED, ImportKind, PendingImport};

pub(super) use crate::time::date_ts;

/// The checked decimal math of the crypto engine.
pub(super) mod math {
    use crate::Amount;

    pub(crate) fn mul(left: Amount, right: Amount) -> Option<Amount> {
        left.0.checked_mul(right.0).map(Amount::new)
    }

    pub(crate) fn add(left: Amount, right: Amount) -> Option<Amount> {
        left.0.checked_add(right.0).map(Amount::new)
    }

    pub(crate) fn div(left: Amount, right: Amount) -> Option<Amount> {
        if right.is_zero() {
            return None;
        }
        left.0.checked_div(right.0).map(Amount::new)
    }
}

/// The `YYYY-MM-DD` of an RFC3339 moment; the text itself when it is not one.
pub(super) fn day(time: &str) -> String {
    let time = time.trim();
    let date = time.get(..10).unwrap_or("");
    let shaped = date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit());
    if shaped {
        date.to_owned()
    } else {
        time.to_owned()
    }
}

fn present(value: &str, what: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::InvalidInput(format!(
            "crypto: {what} must not be empty"
        )));
    }
    Ok(())
}

/// Refuse an account that names nothing.
pub(super) fn validate_account(account: &CryptoAccount) -> Result<()> {
    present(&account.id, "an account id")?;
    present(&account.name, "an account name")?;
    present(&account.address, "an address")
}

fn validate_event(event: &CryptoTx) -> Result<()> {
    present(&event.id, "an operation id")?;
    present(&event.account_id, "an operation's account")?;
    present(&event.time, "an operation's time")?;
    match &event.tracking {
        TrackingRef::ChainTx { hash, .. } => present(hash, "a transaction hash")?,
        TrackingRef::Address { address, .. } => present(address, "an address")?,
        TrackingRef::Position { protocol, id, .. } => {
            present(protocol, "a protocol")?;
            present(id, "a position id")?;
        }
        TrackingRef::ExchangeHistory { exchange, exec_id } => {
            present(exchange, "an exchange")?;
            present(exec_id, "a fill id")?;
        }
    }
    for leg in &event.legs {
        if let CryptoAsset::Token(token) = &leg.asset {
            present(&token.contract, "a token contract")?;
        }
    }
    Ok(())
}

fn event_id(account_id: &str, tracking: &TrackingRef) -> String {
    match tracking {
        TrackingRef::ChainTx { chain, hash } => {
            format!("cev:{account_id}:{}:{hash}", chain_slug(*chain))
        }
        TrackingRef::ExchangeHistory { exchange, exec_id } => {
            format!("cev:{account_id}:{exchange}:{exec_id}")
        }
        TrackingRef::Position {
            protocol,
            chain,
            id,
        } => format!("cev:{account_id}:{}:{protocol}:{id}", chain_slug(*chain)),
        TrackingRef::Address { chain, address } => {
            format!("cev:{account_id}:{}:{address}", chain_slug(*chain))
        }
    }
}

/// The Bybit source of the crypto module, as the conversion used it.
pub(super) mod bybit {
    use super::*;

    pub(crate) const SOURCE_ID: &str = "bybit";
    const EXCHANGE: &str = "bybit";
    /// The pseudo-network an exchange balance lives on.
    pub(crate) const EXCHANGE_CHAIN: Chain = Chain::Fiat;
    /// The `decimals` an exchange coin's token record carries.
    pub(crate) const COIN_DECIMALS: u8 = 8;

    /// The module's asset for an exchange ticker (uppercased, on the fiat pseudo-network).
    fn exchange_asset(symbol: &str) -> CryptoAsset {
        CryptoAsset::Token(TokenId::new(
            EXCHANGE_CHAIN,
            &symbol.trim().to_ascii_uppercase(),
        ))
    }

    /// The ticker of an asset [`exchange_asset`] built, else `None`.
    pub(crate) fn asset_ticker(asset: &CryptoAsset) -> Option<&str> {
        match asset {
            CryptoAsset::Token(token) if token.chain == EXCHANGE_CHAIN => Some(&token.contract),
            _ => None,
        }
    }

    fn is_usd_coin(symbol: &str) -> bool {
        crate::model::coins::is_usd_stablecoin(&format!(
            "asset:{}",
            symbol.trim().to_ascii_uppercase()
        ))
    }

    pub(crate) fn account_handle(store_id: &str) -> String {
        format!("{EXCHANGE}:{store_id}")
    }

    pub(crate) fn account_id_for(store_id: &str) -> String {
        account_handle(store_id)
    }

    /// The `exec_id` an event was read under, when it came from this exchange.
    pub(crate) fn exec_id_of(event: &CryptoTx) -> Option<&str> {
        match &event.tracking {
            TrackingRef::ExchangeHistory { exchange, exec_id } if exchange == EXCHANGE => {
                Some(exec_id.as_str())
            }
            _ => None,
        }
    }

    /// The operation a Bybit record becomes, or `None` when the record cannot be read.
    pub(crate) fn event_from_pending(account_id: &str, imp: &PendingImport) -> Option<CryptoTx> {
        let exec_id = imp.exec_id.trim();
        if exec_id.is_empty() || account_id.trim().is_empty() || imp.occurred_at.trim().is_empty() {
            return None;
        }
        let tracking = TrackingRef::ExchangeHistory {
            exchange: EXCHANGE.to_owned(),
            exec_id: exec_id.to_owned(),
        };
        let handle = account_id;
        let (kind, legs, from, to) = match imp.kind {
            ImportKind::Fill => {
                let sold = leg(CryptoDirection::Out, &imp.asset_sold, imp.qty_sold, None)?;
                let bought = leg(CryptoDirection::In, &imp.asset_bought, imp.qty_bought, None)?;
                (
                    CryptoTxKind::Swap,
                    vec![sold, bought],
                    String::new(),
                    ACCOUNT_UNIFIED.to_owned(),
                )
            }
            ImportKind::Transfer => {
                let out = leg(
                    CryptoDirection::Out,
                    &imp.asset_sold,
                    imp.qty_sold,
                    Some(handle),
                )?;
                let into = leg(
                    CryptoDirection::In,
                    &imp.asset_bought,
                    imp.qty_bought,
                    Some(handle),
                )?;
                (
                    CryptoTxKind::Transfer,
                    vec![out, into],
                    imp.from_account.clone().unwrap_or_default(),
                    imp.to_account.clone().unwrap_or_default(),
                )
            }
            ImportKind::EarnIncome => {
                let paid = leg(CryptoDirection::In, &imp.asset_bought, imp.qty_bought, None)?;
                (
                    CryptoTxKind::Claim,
                    vec![paid],
                    String::new(),
                    ACCOUNT_EARN.to_owned(),
                )
            }
        };
        let event = CryptoTx {
            id: event_id(account_id, &tracking),
            chain: EXCHANGE_CHAIN,
            account_id: account_id.to_owned(),
            time: imp.occurred_at.trim().to_owned(),
            block: None,
            from,
            to,
            legs,
            gas: fee_of(imp),
            protocol: Some(EXCHANGE.to_owned()),
            method: None,
            kind,
            raw: None,
            tracking,
        };
        validate_event(&event).ok()?;
        Some(event)
    }

    fn leg(
        direction: CryptoDirection,
        symbol: &str,
        amount: Amount,
        counterparty: Option<&str>,
    ) -> Option<CryptoLeg> {
        let symbol = symbol.trim();
        if symbol.is_empty() || !amount.is_positive() {
            return None;
        }
        Some(CryptoLeg {
            direction,
            asset: exchange_asset(symbol),
            amount,
            counterparty: counterparty.map(str::to_owned),
        })
    }

    fn fee_of(imp: &PendingImport) -> Option<CryptoFee> {
        let paid = imp.fee.abs();
        let symbol = imp.fee_asset.trim();
        (!paid.is_zero() && !symbol.is_empty()).then(|| CryptoFee {
            asset: exchange_asset(symbol),
            amount: paid,
        })
    }

    /// What one unit of an asset was worth in USD according to the record itself.
    pub(crate) fn implied_usd(imp: &PendingImport) -> Vec<(CryptoAsset, Amount)> {
        let one = Amount::new(rust_decimal::Decimal::ONE);
        let mut out = Vec::new();
        let mut note = |symbol: &str, unit: Amount| {
            let symbol = symbol.trim();
            if !symbol.is_empty() && unit.is_positive() {
                out.push((exchange_asset(symbol), unit));
            }
        };
        for symbol in [imp.asset_bought.as_str(), imp.asset_sold.as_str()] {
            if is_usd_coin(symbol) {
                note(symbol, one);
            }
        }
        if imp.kind == ImportKind::Fill && imp.price.is_positive() {
            let buying = imp.side.eq_ignore_ascii_case("buy");
            let (base, quote) = if buying {
                (imp.asset_bought.as_str(), imp.asset_sold.as_str())
            } else {
                (imp.asset_sold.as_str(), imp.asset_bought.as_str())
            };
            if is_usd_coin(quote) && !is_usd_coin(base) {
                note(base, imp.price);
            }
        }
        out
    }
}
