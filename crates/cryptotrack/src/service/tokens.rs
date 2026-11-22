//! Rules on the tokens the module has met (spec E2): what is noted, what the
//! user's verdict and basis change, and whether an asset takes part in the
//! totals.
//!
//! Receiving events notes a token with [`register_token`]; it adds what it has
//! not met and refreshes the text of what it has, and never touches the user's
//! verdict (a separate correction, see `cryptotrack::tokens`), so meeting the
//! same token again changes nothing the user decided. Every function here works
//! on the document it is given and reads no clock: the scenario passes the
//! moment (`usecases::cryptotrack::tokens`).

use crate::protocols::LegToken;
use crate::tokens::text::{sanitize_name, sanitize_symbol};
use crate::tokens::{self, TokenStatus, known};
use crate::validate::Validate;
use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_model::VaultDoc;
use mwt_model::{CryptoAsset, CryptoToken, TokenId, TokenVerdict};

/// Whether an asset takes part in the totals: the native coin always, a token
/// when `cryptotrack::tokens::counts_in_totals` says so. The one home of this
/// rule for the present capital, the fold and the snapshots.
pub fn counts_asset(doc: &VaultDoc, asset: &CryptoAsset) -> bool {
    match asset {
        CryptoAsset::Native { .. } => true,
        CryptoAsset::Token(token) => tokens::counts_in_totals(doc, token),
    }
}

/// Note a token met in an event. A new contract starts in quarantine (unless
/// it is on the built-in list); a known record gets its text refreshed,
/// sanitized. Returns the status afterwards and whether the document changed;
/// the caller saves, once for however many tokens it noted (a batch can meet
/// hundreds of junk tokens, and each save is a full encryption under the
/// vault's lock).
pub fn register_token(
    doc: &mut VaultDoc,
    token: &TokenId,
    symbol: &str,
    name: &str,
    decimals: u8,
) -> Result<(TokenStatus, bool)> {
    let symbol = sanitize_symbol(symbol);
    let name = sanitize_name(name);
    // A symbol that is empty after cleaning still needs something to show.
    let symbol = if symbol.is_empty() {
        "?".to_string()
    } else {
        symbol
    };
    let changed = match find_mut(doc, token) {
        Some(record) => {
            let changed =
                record.symbol != symbol || record.name != name || record.decimals != decimals;
            if changed {
                record.symbol = symbol;
                record.name = name;
                record.decimals = decimals;
            }
            changed
        }
        None => {
            let mut record = CryptoToken::new(token.clone(), &symbol, decimals);
            record.name = name;
            record.validate()?;
            doc.crypto_tokens.push(record);
            true
        }
    };
    Ok((tokens::status(doc, token), changed))
}

/// Register a leg token of a position read by a built-in adapter as trusted:
/// a record without a verdict becomes `Confirmed` at `now` (unless the
/// built-in list already knows the contract), with the text and decimals the
/// adapter read. A record that carries a verdict is never changed, and real
/// text is never overwritten by an unknown symbol or decimals. Returns whether
/// the document changed; the caller saves.
pub fn trust_adapter_token(doc: &mut VaultDoc, leg: &LegToken, now: &str) -> Result<bool> {
    let known = known::is_known(&leg.token);
    let symbol = leg
        .symbol
        .as_deref()
        .map(sanitize_symbol)
        .filter(|text| !text.is_empty());
    let name = leg
        .name
        .as_deref()
        .map(sanitize_name)
        .filter(|text| !text.is_empty());
    let Some(record) = find_mut(doc, &leg.token) else {
        // Nothing to record about a listed token the adapter said nothing of.
        if known && symbol.is_none() && leg.decimals.is_none() {
            return Ok(false);
        }
        // The text: the adapter's, else the built-in list's, else `?`
        // (D-C5). Decimals the adapter did not read fall back to 18.
        let mut record = CryptoToken::new(
            leg.token.clone(),
            symbol
                .as_deref()
                .or_else(|| known::symbol_of(&leg.token))
                .unwrap_or("?"),
            leg.decimals.unwrap_or(18),
        );
        record.name = name.unwrap_or_default();
        if !known {
            record.verdict = Some(TokenVerdict::Confirmed);
            record.verdict_at = Some(now.to_string());
        }
        record.validate()?;
        doc.crypto_tokens.push(record);
        return Ok(true);
    };
    if record.verdict.is_some() {
        return Ok(false);
    }
    let mut changed = false;
    if let Some(symbol) = symbol.filter(|symbol| *symbol != record.symbol) {
        record.symbol = symbol;
        changed = true;
    }
    if let Some(name) = name.filter(|name| *name != record.name) {
        record.name = name;
        changed = true;
    }
    if let Some(decimals) = leg.decimals.filter(|decimals| *decimals != record.decimals) {
        record.decimals = decimals;
        changed = true;
    }
    if !known {
        record.verdict = Some(TokenVerdict::Confirmed);
        record.verdict_at = Some(now.to_string());
        changed = true;
    }
    Ok(changed)
}

/// Set (or, with `None`, take back) the user's verdict on a token met before;
/// `at` dates a real verdict. The date moves only with a real change: deciding
/// the same thing again keeps the first date.
pub fn set_verdict(
    doc: &mut VaultDoc,
    token: &TokenId,
    verdict: Option<TokenVerdict>,
    at: &str,
) -> Result<()> {
    let record = record_mut(doc, token)?;
    if record.verdict != verdict {
        record.verdict = verdict;
        record.verdict_at = verdict.map(|_| at.to_string());
    }
    Ok(())
}

/// Set (or, with `None`, clear) what the old purchases of a token cost, USD
/// per unit. A negative cost is refused.
pub fn set_basis(doc: &mut VaultDoc, token: &TokenId, basis_usd: Option<Amount>) -> Result<()> {
    if basis_usd.is_some_and(|amount| amount.is_negative()) {
        return Err(Error::InvalidInput(
            "crypto: a cost basis cannot be negative".into(),
        ));
    }
    record_mut(doc, token)?.basis_usd = basis_usd;
    Ok(())
}

fn find_mut<'doc>(doc: &'doc mut VaultDoc, token: &TokenId) -> Option<&'doc mut CryptoToken> {
    doc.crypto_tokens
        .iter_mut()
        .find(|record| record.id() == *token)
}

/// The record of a token met before, or `NotFound`.
fn record_mut<'doc>(doc: &'doc mut VaultDoc, token: &TokenId) -> Result<&'doc mut CryptoToken> {
    find_mut(doc, token).ok_or_else(|| Error::NotFound(format!("token {}", token.key())))
}
