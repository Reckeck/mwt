//! Scenarios of the tokens the module has met (spec E2): noting a token, the
//! user's verdict, the declared cost basis and the list the settings show. The
//! rules are in `cryptotrack::service::tokens`.
//!
//! A token is not a record (it is keyed by chain and contract, so there is no
//! `tx.collection` for it), and `Tx::doc_mut` marks a transaction touched even
//! when nothing changed. So a scenario edits the document through the rules and
//! then ends in the one write: `transact(touch_if(changed))` for a noted token,
//! `crypto_sync_holdings` for a verdict or a basis, which re-derives the
//! holdings the capital is read from (spec E8) and saves.

use crate::Amount;
use crate::cryptotrack::protocols::LegToken;
use crate::cryptotrack::service::tokens as rules;
use crate::cryptotrack::tokens::text::{has_look_alikes, skeleton};
use crate::cryptotrack::tokens::{self, TokenStatus, known};
use crate::error::Result;
use crate::ipc_contract::cryptotrack::tokens::{TokenView, TokenWarning};
use crate::model::cryptotrack::{TokenId, TokenVerdict};
use crate::model::vault::{VaultDoc, now_iso};
use crate::service::Session;

/// Note a token met in an event. A new contract starts in quarantine
/// (unless it is on the built-in list); a known record gets its text
/// refreshed, sanitized. Returns the status after the call.
pub fn crypto_register_token(
    vault: &mut Session,
    token: &TokenId,
    symbol: &str,
    name: &str,
    decimals: u8,
) -> Result<TokenStatus> {
    let (status, changed) = register_token_unsaved(vault, token, symbol, name, decimals)?;
    vault.transact(|tx| {
        tx.touch_if(changed);
        Ok(status)
    })
}

/// [`crypto_register_token`] without the save: the caller owns it and saves
/// once for however many tokens it noted (a batch can meet hundreds of junk
/// tokens, and each save is a full encryption under the vault's lock).
/// Returns the status and whether the document changed.
pub(crate) fn register_token_unsaved(
    vault: &mut Session,
    token: &TokenId,
    symbol: &str,
    name: &str,
    decimals: u8,
) -> Result<(TokenStatus, bool)> {
    rules::register_token(&mut vault.doc, token, symbol, name, decimals)
}

/// Register a leg token of a position read by a built-in adapter as trusted
/// (`rules::trust_adapter_token`). Returns whether the document changed; the
/// caller saves.
pub(crate) fn trust_adapter_token(vault: &mut Session, leg: &LegToken) -> Result<bool> {
    rules::trust_adapter_token(&mut vault.doc, leg, &now_iso())
}

/// The tokens met so far, each with its status, its basis and what is
/// suspicious about its text.
pub fn crypto_tokens(doc: &VaultDoc) -> Vec<TokenView> {
    doc.crypto_tokens
        .iter()
        .map(|record| {
            let id = record.id();
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
                status: tokens::status(doc, &id),
                basis_usd: tokens::basis(doc, &id),
                warnings,
                decided_at: record.verdict_at.clone(),
            }
        })
        .collect()
}

/// A verdict decides whether the token counts at all, so the holdings the
/// capital is read from have to be re-derived (spec E8).
fn decide(
    vault: &mut Session,
    token: &TokenId,
    verdict: Option<TokenVerdict>,
) -> Result<TokenStatus> {
    rules::set_verdict(&mut vault.doc, token, verdict, &now_iso())?;
    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(vault)?;
    Ok(tokens::status(&vault.doc, token))
}

/// Take a token into the totals.
pub fn crypto_confirm_token(vault: &mut Session, token: &TokenId) -> Result<TokenStatus> {
    decide(vault, token, Some(TokenVerdict::Confirmed))
}

/// Hide a token: no list, no total, no price request.
pub fn crypto_reject_token(vault: &mut Session, token: &TokenId) -> Result<TokenStatus> {
    decide(vault, token, Some(TokenVerdict::Rejected))
}

/// Take back the user's verdict on a token: it is judged by the built-in
/// list or the quarantine again, so a confirmed token leaves the totals
/// and a hidden one comes back to the quarantine list. Clearing a token
/// that has no verdict changes nothing.
pub fn crypto_clear_token_verdict(vault: &mut Session, token: &TokenId) -> Result<TokenStatus> {
    decide(vault, token, None)
}

/// Set (or, with `None`, clear) what the old purchases of this token
/// cost, USD per unit.
pub fn crypto_set_token_basis(
    vault: &mut Session,
    token: &TokenId,
    basis_usd: Option<Amount>,
) -> Result<()> {
    rules::set_basis(&mut vault.doc, token, basis_usd)?;
    // The declared cost basis is what a holding's `avg_cost` carries.
    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(vault)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::model::chain::Chain;
    use crate::service::test_support::empty_vault;

    const REAL_USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const SCAM: &str = "0x00000000000000000000000000000000deadbeef";

    fn eth(contract: &str) -> TokenId {
        TokenId::new(Chain::Ethereum, contract)
    }

    #[test]
    fn a_scam_usdc_with_a_foreign_contract_stays_out_of_the_totals() {
        let (_dir, mut vault) = empty_vault();
        let real = crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &eth(REAL_USDC),
            "USDC",
            "USD Coin",
            6,
        )
        .unwrap();
        let scam = crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &eth(SCAM),
            "USDC",
            "USD Coin",
            6,
        )
        .unwrap();
        assert_eq!(real, TokenStatus::Known);
        assert_eq!(scam, TokenStatus::Quarantined);
        assert!(tokens::counts_in_totals(&vault.doc, &eth(REAL_USDC)));
        assert!(!tokens::counts_in_totals(&vault.doc, &eth(SCAM)));
        let view = crate::usecases::cryptotrack::tokens::crypto_tokens(&vault.doc);
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
        let wallet = crate::usecases::money::stores::create_store(
            &mut vault,
            "Cold EVM",
            StoreKind::Crypto,
            "#000",
            "",
            None,
        )
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
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &token, "PEPE2", "Pepe 2", 18,
        )
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
        let held = |vault: &crate::service::Session| {
            vault
                .doc
                .holdings
                .iter()
                .any(|holding| holding.store_id == wallet.id)
        };
        assert!(!held(&vault), "quarantined: not in the capital");
        assert_eq!(
            crate::usecases::cryptotrack::tokens::crypto_confirm_token(&mut vault, &token).unwrap(),
            TokenStatus::Confirmed
        );
        assert!(held(&vault), "recognised: in the capital at once");
        assert_eq!(
            crate::usecases::cryptotrack::tokens::crypto_clear_token_verdict(&mut vault, &token)
                .unwrap(),
            TokenStatus::Quarantined
        );
        assert!(!held(&vault), "taken back: out of the capital again");
        assert!(
            vault.doc.crypto_tokens[0].verdict_at.is_none(),
            "the date goes with the verdict"
        );
        // A hidden token comes back to the quarantine list the same way.
        crate::usecases::cryptotrack::tokens::crypto_reject_token(&mut vault, &token).unwrap();
        assert_eq!(
            crate::usecases::cryptotrack::tokens::crypto_clear_token_verdict(&mut vault, &token)
                .unwrap(),
            TokenStatus::Quarantined
        );
    }

    #[test]
    fn an_evm_address_in_another_case_is_one_token_and_solana_keeps_case() {
        let (_dir, mut vault) = empty_vault();
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &eth(&SCAM.to_uppercase().replace("0X", "0x")),
            "AAA",
            "",
            18,
        )
        .unwrap();
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &eth(SCAM),
            "AAA",
            "",
            18,
        )
        .unwrap();
        assert_eq!(vault.doc.crypto_tokens.len(), 1);

        let mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let sol = |contract: &str| TokenId::new(Chain::Solana, contract);
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &sol(mint),
            "USDC",
            "",
            6,
        )
        .unwrap();
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &sol(&mint.to_lowercase()),
            "USDC",
            "",
            6,
        )
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
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &airdrop, "AIR", "Airdrop", 18,
        )
        .unwrap();
        crate::usecases::cryptotrack::tokens::crypto_confirm_token(&mut vault, &airdrop).unwrap();
        let basis = Amount::parse("0.5").unwrap();
        crate::usecases::cryptotrack::tokens::crypto_set_token_basis(
            &mut vault,
            &airdrop,
            Some(basis),
        )
        .unwrap();
        // The same token arrives again, with new text: a refetch.
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &airdrop, "AIR2", "Renamed", 9,
        )
        .unwrap();
        assert_eq!(vault.doc.crypto_tokens.len(), 1);
        assert_eq!(vault.doc.crypto_tokens[0].symbol, "AIR2");
        assert_eq!(tokens::status(&vault.doc, &airdrop), TokenStatus::Confirmed);
        assert_eq!(tokens::basis(&vault.doc, &airdrop), Some(basis));
        assert!(tokens::counts_in_totals(&vault.doc, &airdrop));

        crate::usecases::cryptotrack::tokens::crypto_reject_token(&mut vault, &airdrop).unwrap();
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &airdrop, "AIR", "Airdrop", 18,
        )
        .unwrap();
        assert_eq!(tokens::status(&vault.doc, &airdrop), TokenStatus::Rejected);
        assert!(!tokens::counts_in_totals(&vault.doc, &airdrop));
    }

    #[test]
    fn rejecting_a_known_token_wins_over_the_list() {
        let (_dir, mut vault) = empty_vault();
        let usdc = eth(REAL_USDC);
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &usdc, "USDC", "", 6,
        )
        .unwrap();
        crate::usecases::cryptotrack::tokens::crypto_reject_token(&mut vault, &usdc).unwrap();
        assert_eq!(tokens::status(&vault.doc, &usdc), TokenStatus::Rejected);
    }

    #[test]
    fn the_basis_is_an_attribute_that_can_be_set_and_cleared() {
        let (_dir, mut vault) = empty_vault();
        let token = eth(SCAM);
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault, &token, "AIR", "", 18,
        )
        .unwrap();
        let price = Amount::parse("1.25").unwrap();
        crate::usecases::cryptotrack::tokens::crypto_set_token_basis(
            &mut vault,
            &token,
            Some(price),
        )
        .unwrap();
        assert_eq!(tokens::basis(&vault.doc, &token), Some(price));
        assert!(
            crate::usecases::cryptotrack::tokens::crypto_set_token_basis(
                &mut vault,
                &token,
                Some(Amount::parse("-1").unwrap())
            )
            .is_err()
        );
        crate::usecases::cryptotrack::tokens::crypto_set_token_basis(&mut vault, &token, None)
            .unwrap();
        assert_eq!(tokens::basis(&vault.doc, &token), None);
        // A basis is not a verdict: the token is still quarantined.
        assert_eq!(tokens::status(&vault.doc, &token), TokenStatus::Quarantined);
    }

    #[test]
    fn deciding_on_an_unmet_token_is_refused() {
        let (_dir, mut vault) = empty_vault();
        assert!(matches!(
            crate::usecases::cryptotrack::tokens::crypto_confirm_token(&mut vault, &eth(SCAM)),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn a_hostile_name_is_cleaned_before_it_is_stored() {
        let (_dir, mut vault) = empty_vault();
        let hostile = format!("Free\u{202E} https://evil.example {}", "x".repeat(300));
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &eth(SCAM),
            "US\u{200B}DC",
            &hostile,
            18,
        )
        .unwrap();
        let record = &vault.doc.crypto_tokens[0];
        assert_eq!(record.symbol, "USDC");
        assert!(!record.name.contains("evil") && !record.name.contains('\u{202E}'));
        assert!(record.name.chars().count() <= 64);
    }
}

/// C3: `counts_in_totals` is the only filter a token meets on every path that
/// reads the module: the legs of the fold, the positions, the totals, the
/// price and the list of tokens a node is asked for. One test per path; each
/// shows a quarantined token left out, and the same events counting at once
/// when the user confirms it.
#[cfg(test)]
mod counts_in_totals_pins {
    use super::*;
    use crate::cryptotrack::engine::position_value::held_by_account;
    use crate::cryptotrack::sources::evm::RawAsset;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg, CryptoPosition, CryptoTx,
        CryptoTxKind, TrackingRef,
    };
    use crate::model::store::StoreKind;
    use crate::model::vault::PricePoint;
    use crate::service::test_support::{amount, empty_vault, store};

    const ME: &str = "0xabcd00000000000000000000000000000000ef01";
    const OUTSIDE: &str = "0x2222222222222222222222222222222222222222";
    const STRANGER: &str = "0x3333333333333333333333333333333333333333";
    const ACCOUNT: &str = "cacc-1";

    fn stranger() -> TokenId {
        TokenId::new(Chain::Ethereum, STRANGER)
    }

    /// A followed address with its wallet, and a token nobody confirmed.
    fn vault_with_a_stranger() -> (tempfile::TempDir, Session) {
        let (dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts.push(CryptoAccount::new(
            ACCOUNT,
            "Cold",
            Chain::Ethereum,
            ME,
            "2026-02-01T00:00:00Z",
        ));
        let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
        vault
            .doc
            .stores
            .iter_mut()
            .find(|have| have.id == wallet)
            .expect("the wallet was created")
            .crypto_account_id = Some(ACCOUNT.to_string());
        crate::usecases::cryptotrack::tokens::crypto_register_token(
            &mut vault,
            &stranger(),
            "USDC",
            "USD Coin",
            6,
        )
        .expect("noted");
        (dir, vault)
    }

    fn the_token() -> CryptoAsset {
        CryptoAsset::Token(stranger())
    }

    fn leg(direction: CryptoDirection, quantity: &str) -> CryptoLeg {
        CryptoLeg {
            direction,
            asset: the_token(),
            amount: amount(quantity),
            counterparty: Some(OUTSIDE.to_string()),
        }
    }

    fn arrival(vault: &mut Session, quantity: &str) {
        vault.doc.crypto_events.push(CryptoTx {
            id: "ev-1".into(),
            chain: Chain::Ethereum,
            account_id: ACCOUNT.into(),
            time: "2026-03-05T10:00:00Z".into(),
            block: Some(21_000_000),
            from: OUTSIDE.into(),
            to: ME.into(),
            legs: vec![leg(CryptoDirection::In, quantity)],
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
    }

    fn in_a_position(vault: &mut Session, quantity: &str) {
        vault.doc.crypto_positions.push(CryptoPosition {
            id: "pos-1".into(),
            account_id: ACCOUNT.into(),
            chain: Chain::Ethereum,
            protocol: "aave-v3".into(),
            name: "Supply".into(),
            legs: vec![leg(CryptoDirection::In, quantity)],
            receipts: Vec::new(),
            updated_at: "2026-03-05T10:00:00Z".into(),
        });
    }

    /// Something quoted the token, as a pretender's price would be.
    fn quoted_at(vault: &mut Session, usd: &str) {
        let key =
            crate::cryptotrack::service::totals::crypto_asset_ref(&vault.doc, &the_token()).key();
        vault.doc.price_cache.push(PricePoint {
            asset_key: key,
            value_usd: amount(usd),
            ts: "2026-03-09T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
    }

    fn confirm(vault: &mut Session) {
        crate::usecases::cryptotrack::tokens::crypto_confirm_token(vault, &stranger())
            .expect("decided");
    }

    #[test]
    fn the_legs_of_the_fold_skip_a_quarantined_token() {
        let (_dir, mut vault) = vault_with_a_stranger();
        arrival(&mut vault, "5");
        let key = (ACCOUNT.to_string(), the_token().key());
        assert!(
            !crate::cryptotrack::service::recompute::fold(&vault.books())
                .balances
                .contains_key(&key)
        );
        confirm(&mut vault);
        assert_eq!(
            crate::cryptotrack::service::recompute::fold(&vault.books())
                .balances
                .get(&key),
            Some(&amount("5"))
        );
    }

    #[test]
    fn the_positions_skip_a_quarantined_token_leg() {
        let (_dir, mut vault) = vault_with_a_stranger();
        in_a_position(&mut vault, "5");
        let ledger = crate::cryptotrack::service::recompute::fold(&vault.books());
        let held = |vault: &Session| {
            crate::cryptotrack::service::positions::crypto_position_inputs(&vault.doc)
                .with(|taking| held_by_account(&ledger, taking))
                .iter()
                .any(|row| row.asset == the_token())
        };
        assert!(!held(&vault), "a leg of a quarantined token is not held");
        confirm(&mut vault);
        assert!(held(&vault), "the same leg counts once confirmed");
    }

    #[test]
    fn the_totals_skip_a_quarantined_token_and_list_it_as_left_out() {
        let (_dir, mut vault) = vault_with_a_stranger();
        arrival(&mut vault, "5");
        quoted_at(&mut vault, "1");
        crate::usecases::money::portfolio::recompute_holdings(&mut vault);
        let at = "2026-03-10T00:00:00Z";
        assert!(
            crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), at)
                .assets
                .is_empty()
        );
        assert!(vault.doc.holdings.is_empty());
        let left = crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books());
        assert_eq!(left.len(), 1, "{left:?}");
        assert_eq!(left[0].asset_key, the_token().key());

        confirm(&mut vault);
        assert_eq!(
            crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), at)
                .assets
                .len(),
            1
        );
        assert_eq!(vault.doc.holdings.len(), 1);
        assert!(
            crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books()).is_empty()
        );
    }

    #[test]
    fn a_quarantined_token_has_no_price_even_when_something_quoted_it() {
        let (_dir, mut vault) = vault_with_a_stranger();
        quoted_at(&mut vault, "1");
        assert_eq!(
            crate::cryptotrack::service::positions::price_now_usd(&vault.books(), &the_token()),
            None
        );
        confirm(&mut vault);
        assert_eq!(
            crate::cryptotrack::service::positions::price_now_usd(&vault.books(), &the_token()),
            Some(amount("1"))
        );
    }

    #[test]
    fn a_node_is_not_asked_for_the_balance_of_a_quarantined_token() {
        let (_dir, mut vault) = vault_with_a_stranger();
        let asked = |vault: &Session| {
            crate::usecases::cryptotrack::apply::crypto_sync_targets(&vault.doc)[0].tokens.iter().any(
                |asset| matches!(asset, RawAsset::Token { contract, .. } if contract == STRANGER),
            )
        };
        assert!(!asked(&vault));
        confirm(&mut vault);
        assert!(asked(&vault));
    }
}
