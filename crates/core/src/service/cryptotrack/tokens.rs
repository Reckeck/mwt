//! Operations on the tokens the module has met (spec E2).
//!
//! Receiving events calls [`UnlockedVault::crypto_register_token`]; it adds
//! what it has not met and refreshes the text of what it has, and never
//! touches the user's verdict (a separate correction, see
//! `cryptotrack::tokens`), so meeting the same token again changes nothing
//! the user decided.

use super::super::UnlockedVault;
use crate::Amount;
use crate::cryptotrack::protocols::LegToken;
use crate::cryptotrack::tokens::text::{has_look_alikes, sanitize_name, sanitize_symbol, skeleton};
use crate::cryptotrack::tokens::{self, TokenStatus, known};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::tokens::{TokenView, TokenWarning};
use crate::model::cryptotrack::{CryptoAsset, CryptoToken, TokenId, TokenVerdict};
use crate::model::vault::{VaultDoc, now_iso};

/// Whether an asset takes part in the totals: the native coin always, a token
/// when `cryptotrack::tokens::counts_in_totals` says so. The one home of this
/// rule for the present capital, the fold and the snapshots.
pub(super) fn counts_asset(doc: &VaultDoc, asset: &CryptoAsset) -> bool {
    match asset {
        CryptoAsset::Native { .. } => true,
        CryptoAsset::Token(token) => tokens::counts_in_totals(doc, token),
    }
}

impl UnlockedVault {
    /// Note a token met in an event. A new contract starts in quarantine
    /// (unless it is on the built-in list); a known record gets its text
    /// refreshed, sanitized. Returns the status after the call.
    pub fn crypto_register_token(
        &mut self,
        token: &TokenId,
        symbol: &str,
        name: &str,
        decimals: u8,
    ) -> Result<TokenStatus> {
        let (status, changed) = self.register_token_unsaved(token, symbol, name, decimals)?;
        self.transact(|tx| {
            tx.touch_if(changed);
            Ok(status)
        })
    }

    /// [`Self::crypto_register_token`] without the save: the caller owns it
    /// and saves once for however many tokens it noted (a batch can meet
    /// hundreds of junk tokens, and each save is a full encryption under the
    /// vault's lock). Returns the status and whether the document changed.
    pub(super) fn register_token_unsaved(
        &mut self,
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
        let existing = self
            .doc
            .crypto_tokens
            .iter_mut()
            .find(|record| record.id() == *token);
        let changed = match existing {
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
                self.doc.crypto_tokens.push(record);
                true
            }
        };
        Ok((tokens::status(&self.doc, token), changed))
    }

    /// Register a leg token of a position read by a built-in adapter as
    /// trusted: a record without a verdict becomes `Confirmed` (unless the
    /// built-in list already knows the contract), with the text and decimals
    /// the adapter read. A record that carries a verdict is never changed, and
    /// real text is never overwritten by an unknown symbol or decimals.
    /// Returns whether the document changed; the caller saves.
    pub(super) fn trust_adapter_token(&mut self, leg: &LegToken) -> Result<bool> {
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
        let existing = self
            .doc
            .crypto_tokens
            .iter_mut()
            .find(|record| record.id() == leg.token);
        let Some(record) = existing else {
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
                record.verdict_at = Some(now_iso());
            }
            record.validate()?;
            self.doc.crypto_tokens.push(record);
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
            record.verdict_at = Some(now_iso());
            changed = true;
        }
        Ok(changed)
    }

    /// The tokens met so far, each with its status, its basis and what is
    /// suspicious about its text.
    pub fn crypto_tokens(&self) -> Vec<TokenView> {
        self.doc
            .crypto_tokens
            .iter()
            .map(|record| {
                let id = record.id();
                let status = tokens::status(&self.doc, &id);
                let mut warnings = Vec::new();
                if has_look_alikes(&record.symbol) || has_look_alikes(&record.name) {
                    warnings.push(TokenWarning::LookAlike);
                }
                let imitates =
                    known::symbol_is_known(&skeleton(&record.symbol)) && !known::is_known(&id);
                if imitates {
                    warnings.push(TokenWarning::ImitatesKnown);
                }
                TokenView {
                    chain: record.chain,
                    contract: record.contract.clone(),
                    symbol: record.symbol.clone(),
                    name: record.name.clone(),
                    decimals: record.decimals,
                    status,
                    basis_usd: tokens::basis(&self.doc, &id),
                    warnings,
                    decided_at: record.verdict_at.clone(),
                }
            })
            .collect()
    }

    fn record_mut(&mut self, token: &TokenId) -> Result<&mut CryptoToken> {
        self.doc
            .crypto_tokens
            .iter_mut()
            .find(|record| record.id() == *token)
            .ok_or_else(|| Error::NotFound(format!("token {}", token.key())))
    }

    fn decide(&mut self, token: &TokenId, verdict: Option<TokenVerdict>) -> Result<TokenStatus> {
        let at = verdict.map(|_| now_iso());
        let record = self.record_mut(token)?;
        // The date moves only with a real change: deciding the same thing again keeps the first date.
        if record.verdict != verdict {
            record.verdict = verdict;
            record.verdict_at = at;
        }
        // A verdict decides whether the token counts at all, so the holdings
        // the capital is read from have to be re-derived (spec E8).
        self.crypto_sync_holdings()?;
        Ok(tokens::status(&self.doc, token))
    }

    /// Take a token into the totals.
    pub fn crypto_confirm_token(&mut self, token: &TokenId) -> Result<TokenStatus> {
        self.decide(token, Some(TokenVerdict::Confirmed))
    }

    /// Hide a token: no list, no total, no price request.
    pub fn crypto_reject_token(&mut self, token: &TokenId) -> Result<TokenStatus> {
        self.decide(token, Some(TokenVerdict::Rejected))
    }

    /// Take back the user's verdict on a token: it is judged by the built-in
    /// list or the quarantine again, so a confirmed token leaves the totals
    /// and a hidden one comes back to the quarantine list. Clearing a token
    /// that has no verdict changes nothing.
    pub fn crypto_clear_token_verdict(&mut self, token: &TokenId) -> Result<TokenStatus> {
        self.decide(token, None)
    }

    /// Set (or, with `None`, clear) what the old purchases of this token
    /// cost, USD per unit.
    pub fn crypto_set_token_basis(
        &mut self,
        token: &TokenId,
        basis_usd: Option<Amount>,
    ) -> Result<()> {
        if basis_usd.is_some_and(|amount| amount.is_negative()) {
            return Err(Error::InvalidInput(
                "crypto: a cost basis cannot be negative".into(),
            ));
        }
        self.record_mut(token)?.basis_usd = basis_usd;
        // The declared cost basis is what a holding's `avg_cost` carries.
        self.crypto_sync_holdings()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::empty_vault;
    use super::*;
    use crate::model::chain::Chain;

    const REAL_USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const SCAM: &str = "0x00000000000000000000000000000000deadbeef";

    fn eth(contract: &str) -> TokenId {
        TokenId::new(Chain::Ethereum, contract)
    }

    #[test]
    fn a_scam_usdc_with_a_foreign_contract_stays_out_of_the_totals() {
        let (_dir, mut vault) = empty_vault();
        let real = vault
            .crypto_register_token(&eth(REAL_USDC), "USDC", "USD Coin", 6)
            .unwrap();
        let scam = vault
            .crypto_register_token(&eth(SCAM), "USDC", "USD Coin", 6)
            .unwrap();
        assert_eq!(real, TokenStatus::Known);
        assert_eq!(scam, TokenStatus::Quarantined);
        assert!(tokens::counts_in_totals(&vault.doc, &eth(REAL_USDC)));
        assert!(!tokens::counts_in_totals(&vault.doc, &eth(SCAM)));
        let view = vault.crypto_tokens();
        let scam_view = view.iter().find(|row| row.contract == SCAM).unwrap();
        assert!(scam_view.warnings.contains(&TokenWarning::ImitatesKnown));
    }

    #[test]
    fn taking_back_a_recognition_returns_the_token_to_quarantine_and_out_of_the_capital() {
        use crate::model::cryptotrack::{
            CryptoAsset, CryptoDirection, CryptoLeg, CryptoTx, CryptoTxKind, TrackingRef,
        };
        use crate::model::store::StoreKind;
        let (_dir, mut vault) = empty_vault();
        let wallet = vault
            .create_store("Cold EVM", StoreKind::Crypto, "#000", "", None)
            .unwrap();
        let mut account = crate::model::cryptotrack::CryptoAccount::new(
            "cacc-1",
            "Cold EVM",
            Chain::Ethereum,
            "0x7a3f19c4d2b85e6f0a9c41b7ce3a0d8e2f4b9bc1",
            "2026-01-01T00:00:00Z",
        );
        account.source = None;
        vault.doc.crypto_accounts.push(account);
        vault
            .doc
            .stores
            .iter_mut()
            .find(|store| store.id == wallet.id)
            .unwrap()
            .crypto_account_id = Some("cacc-1".into());
        let token = eth(SCAM);
        vault
            .crypto_register_token(&token, "PEPE2", "Pepe 2", 18)
            .unwrap();
        vault.doc.crypto_events.push(CryptoTx {
            id: "ev-1".into(),
            chain: Chain::Ethereum,
            account_id: "cacc-1".into(),
            time: "2026-02-01T10:00:00Z".into(),
            block: None,
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Token(token.clone()),
                amount: Amount::parse("5").unwrap(),
                counterparty: None,
            }],
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: "0xfeed".into(),
            },
        });
        let held = |vault: &crate::service::UnlockedVault| {
            vault
                .doc
                .holdings
                .iter()
                .any(|holding| holding.store_id == wallet.id)
        };
        assert!(!held(&vault), "quarantined: not in the capital");
        assert_eq!(
            vault.crypto_confirm_token(&token).unwrap(),
            TokenStatus::Confirmed
        );
        assert!(held(&vault), "recognised: in the capital at once");
        assert_eq!(
            vault.crypto_clear_token_verdict(&token).unwrap(),
            TokenStatus::Quarantined
        );
        assert!(!held(&vault), "taken back: out of the capital again");
        assert!(
            vault.doc.crypto_tokens[0].verdict_at.is_none(),
            "the date goes with the verdict"
        );
        // A hidden token comes back to the quarantine list the same way.
        vault.crypto_reject_token(&token).unwrap();
        assert_eq!(
            vault.crypto_clear_token_verdict(&token).unwrap(),
            TokenStatus::Quarantined
        );
    }

    #[test]
    fn an_evm_address_in_another_case_is_one_token_and_solana_keeps_case() {
        let (_dir, mut vault) = empty_vault();
        vault
            .crypto_register_token(
                &eth(&SCAM.to_uppercase().replace("0X", "0x")),
                "AAA",
                "",
                18,
            )
            .unwrap();
        vault
            .crypto_register_token(&eth(SCAM), "AAA", "", 18)
            .unwrap();
        assert_eq!(vault.doc.crypto_tokens.len(), 1);

        let mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let sol = |contract: &str| TokenId::new(Chain::Solana, contract);
        vault
            .crypto_register_token(&sol(mint), "USDC", "", 6)
            .unwrap();
        vault
            .crypto_register_token(&sol(&mint.to_lowercase()), "USDC", "", 6)
            .unwrap();
        let solana = vault
            .doc
            .crypto_tokens
            .iter()
            .filter(|record| record.chain == Chain::Solana)
            .count();
        assert_eq!(solana, 2);
        assert_eq!(tokens::status(&vault.doc, &sol(mint)), TokenStatus::Known);
        assert_eq!(
            tokens::status(&vault.doc, &sol(&mint.to_lowercase())),
            TokenStatus::Quarantined
        );
    }

    #[test]
    fn the_users_decision_survives_receiving_the_events_again() {
        let (_dir, mut vault) = empty_vault();
        let airdrop = eth(SCAM);
        vault
            .crypto_register_token(&airdrop, "AIR", "Airdrop", 18)
            .unwrap();
        vault.crypto_confirm_token(&airdrop).unwrap();
        let basis = Amount::parse("0.5").unwrap();
        vault.crypto_set_token_basis(&airdrop, Some(basis)).unwrap();
        // The same token arrives again, with new text: a refetch.
        vault
            .crypto_register_token(&airdrop, "AIR2", "Renamed", 9)
            .unwrap();
        assert_eq!(vault.doc.crypto_tokens.len(), 1);
        assert_eq!(vault.doc.crypto_tokens[0].symbol, "AIR2");
        assert_eq!(tokens::status(&vault.doc, &airdrop), TokenStatus::Confirmed);
        assert_eq!(tokens::basis(&vault.doc, &airdrop), Some(basis));
        assert!(tokens::counts_in_totals(&vault.doc, &airdrop));

        vault.crypto_reject_token(&airdrop).unwrap();
        vault
            .crypto_register_token(&airdrop, "AIR", "Airdrop", 18)
            .unwrap();
        assert_eq!(tokens::status(&vault.doc, &airdrop), TokenStatus::Rejected);
        assert!(!tokens::counts_in_totals(&vault.doc, &airdrop));
    }

    #[test]
    fn rejecting_a_known_token_wins_over_the_list() {
        let (_dir, mut vault) = empty_vault();
        let usdc = eth(REAL_USDC);
        vault.crypto_register_token(&usdc, "USDC", "", 6).unwrap();
        vault.crypto_reject_token(&usdc).unwrap();
        assert_eq!(tokens::status(&vault.doc, &usdc), TokenStatus::Rejected);
    }

    #[test]
    fn the_basis_is_an_attribute_that_can_be_set_and_cleared() {
        let (_dir, mut vault) = empty_vault();
        let token = eth(SCAM);
        vault.crypto_register_token(&token, "AIR", "", 18).unwrap();
        let price = Amount::parse("1.25").unwrap();
        vault.crypto_set_token_basis(&token, Some(price)).unwrap();
        assert_eq!(tokens::basis(&vault.doc, &token), Some(price));
        assert!(
            vault
                .crypto_set_token_basis(&token, Some(Amount::parse("-1").unwrap()))
                .is_err()
        );
        vault.crypto_set_token_basis(&token, None).unwrap();
        assert_eq!(tokens::basis(&vault.doc, &token), None);
        // A basis is not a verdict: the token is still quarantined.
        assert_eq!(tokens::status(&vault.doc, &token), TokenStatus::Quarantined);
    }

    #[test]
    fn deciding_on_an_unmet_token_is_refused() {
        let (_dir, mut vault) = empty_vault();
        assert!(matches!(
            vault.crypto_confirm_token(&eth(SCAM)),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn a_hostile_name_is_cleaned_before_it_is_stored() {
        let (_dir, mut vault) = empty_vault();
        let hostile = format!("Free\u{202E} https://evil.example {}", "x".repeat(300));
        vault
            .crypto_register_token(&eth(SCAM), "US\u{200B}DC", &hostile, 18)
            .unwrap();
        let record = &vault.doc.crypto_tokens[0];
        assert_eq!(record.symbol, "USDC");
        assert!(!record.name.contains("evil") && !record.name.contains('\u{202E}'));
        assert!(record.name.chars().count() <= 64);
    }
}
