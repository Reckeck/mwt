//! Pointing a fiat record at a crypto operation and back (spec E13).
//!
//! The pointer is [`CryptoOverride::fiat_tx_id`]. It is a visible reference
//! and nothing else: nothing here touches `flow_id`, a chain, a cost basis or
//! a sum, and nothing recomputes the holdings. The chain of money is written
//! by `crypto_set_override` (spec E10), and only there. Both ends may vanish
//! later (a deleted record, a wiped module); reading a pointer whose end is
//! gone gives `None` for that end, never an error.

use chrono::NaiveDate;

use super::super::UnlockedVault;
use crate::Amount;
use crate::cryptotrack::engine::{ids, overrides};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::links::{
    CryptoLinkCandidatesInput, CryptoLinkInput, CryptoLinkOption, CryptoPair, CryptoPairFiat,
    CryptoPairsInput, CryptoUnlinkInput,
};
use crate::ipc_contract::cryptotrack::operations::{CryptoOpRow, CryptoOperationDetailInput};
use crate::model::cryptotrack::CryptoOverride;
use crate::model::transaction::LegRole;
use crate::model::vault::now_iso;

/// How far apart the two sides may be dated, in days.
const WINDOW_DAYS: i64 = 3;
/// How far the fiat sum may be from the coin's value, in percent.
const TOLERANCE_PERCENT: u32 = 5;
/// The most suggestions offered.
const OPTION_CAP: usize = 40;

fn day_of(time: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(time.get(..10)?, "%Y-%m-%d").ok()
}

impl UnlockedVault {
    /// Crypto operations that look like the same movement of money as a fiat
    /// record: close in time, the ones whose value fits first. Only a
    /// suggestion, nothing is written.
    pub fn crypto_link_candidates(
        &self,
        input: &CryptoLinkCandidatesInput,
    ) -> Vec<CryptoLinkOption> {
        let Some(day) = day_of(&input.time) else {
            return Vec::new();
        };
        let fixes = overrides::index(&self.doc.crypto_overrides);
        let mut found: Vec<(u32, &str)> = Vec::new();
        for event in &self.doc.crypto_events {
            if ids::adjustment(&event.id).is_some()
                || fixes.get(event.id.as_str()).is_some_and(|fix| fix.hidden)
            {
                continue;
            }
            let Some(other) = day_of(&event.time) else {
                continue;
            };
            let days = (other - day).num_days().unsigned_abs();
            if days as i64 > WINDOW_DAYS {
                continue;
            }
            found.push((days as u32, event.id.as_str()));
        }
        let mut options: Vec<CryptoLinkOption> = found
            .into_iter()
            .filter_map(|(days, id)| {
                let row = self.crypto_row_of(id)?;
                let fits = self.value_fits(input, &row);
                Some(CryptoLinkOption { row, days, fits })
            })
            .collect();
        options.sort_by(|left, right| {
            right
                .fits
                .cmp(&left.fits)
                .then(left.days.cmp(&right.days))
                .then(right.row.time.cmp(&left.row.time))
                .then(left.row.id.cmp(&right.row.id))
        });
        options.truncate(OPTION_CAP);
        options
    }

    fn crypto_row_of(&self, event_id: &str) -> Option<CryptoOpRow> {
        self.crypto_operation_detail(&CryptoOperationDetailInput {
            id: event_id.to_string(),
            with_candidates: false,
        })
        .ok()
        .map(|detail| detail.row)
    }

    fn value_fits(&self, input: &CryptoLinkCandidatesInput, row: &CryptoOpRow) -> bool {
        let (Some(amount), Some(currency), Some(value)) =
            (input.amount, input.currency.as_deref(), row.value_usd)
        else {
            return false;
        };
        if value.is_zero() {
            return false;
        }
        self.convert_at(amount.abs(), currency, "usd", &input.time)
            .is_some_and(|usd| {
                (usd - value).abs() * Amount::from(100) <= value * Amount::from(TOLERANCE_PERCENT)
            })
    }

    /// Point a crypto operation at a fiat record. Both must exist now.
    pub fn crypto_link_fiat(&mut self, input: &CryptoLinkInput) -> Result<()> {
        if !self
            .doc
            .crypto_events
            .iter()
            .any(|event| event.id == input.event_id)
        {
            return Err(Error::NotFound(format!("operation {}", input.event_id)));
        }
        if !self
            .doc
            .transactions
            .iter()
            .any(|tx| tx.meta().id == input.fiat_tx_id)
        {
            return Err(Error::NotFound(format!("transaction {}", input.fiat_tx_id)));
        }
        let now = now_iso();
        let stands = overrides::index(&self.doc.crypto_overrides)
            .get(input.event_id.as_str())
            .map(|fix| fix.id.clone());
        let slot = stands.and_then(|id| {
            self.doc
                .crypto_overrides
                .iter()
                .position(|fix| fix.id == id)
        });
        match slot {
            Some(index) => {
                let fix = &mut self.doc.crypto_overrides[index];
                fix.fiat_tx_id = Some(input.fiat_tx_id.clone());
                fix.updated_at = now;
            }
            None => self.doc.crypto_overrides.push(CryptoOverride {
                id: format!("cov:{}", input.event_id),
                event_id: input.event_id.clone(),
                kind: None,
                comment: None,
                categories: Vec::new(),
                flow_id: None,
                contact_id: None,
                not_exchange: false,
                hidden: false,
                fiat_tx_id: Some(input.fiat_tx_id.clone()),
                updated_at: now,
            }),
        }
        self.persist()
    }

    /// Remove the pointer. Nothing to remove is not an error.
    pub fn crypto_unlink_fiat(&mut self, input: &CryptoUnlinkInput) -> Result<()> {
        let stands = overrides::index(&self.doc.crypto_overrides)
            .get(input.event_id.as_str())
            .filter(|fix| fix.fiat_tx_id.is_some())
            .map(|fix| fix.id.clone());
        let Some(id) = stands else {
            return Ok(());
        };
        if let Some(fix) = self
            .doc
            .crypto_overrides
            .iter_mut()
            .find(|fix| fix.id == id)
        {
            fix.fiat_tx_id = None;
            fix.updated_at = now_iso();
        }
        self.persist()
    }

    /// The pointers of one operation or one fiat record, both ends resolved.
    /// A vault without crypto data has none, so the fiat form shows nothing.
    pub fn crypto_pairs(&self, input: &CryptoPairsInput) -> Vec<CryptoPair> {
        overrides::index(&self.doc.crypto_overrides)
            .values()
            .filter_map(|fix| {
                let fiat_id = fix.fiat_tx_id.as_deref()?;
                if input
                    .event_id
                    .as_deref()
                    .is_some_and(|id| id != fix.event_id)
                    || input.fiat_tx_id.as_deref().is_some_and(|id| id != fiat_id)
                {
                    return None;
                }
                Some(CryptoPair {
                    event_id: fix.event_id.clone(),
                    fiat_tx_id: fiat_id.to_string(),
                    event: self.crypto_row_of(&fix.event_id),
                    fiat: self.pair_fiat(fiat_id),
                })
            })
            .collect()
    }

    fn pair_fiat(&self, id: &str) -> Option<CryptoPairFiat> {
        let tx = self.doc.transactions.iter().find(|tx| tx.meta().id == id)?;
        let legs = tx.legs();
        let leg = legs
            .iter()
            .find(|leg| leg.role == LegRole::Principal)
            .or_else(|| legs.first())?;
        let meta = tx.meta();
        Some(CryptoPairFiat {
            id: meta.id.clone(),
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg, CryptoToken, CryptoTx,
        CryptoTxKind, TokenId, TrackingRef,
    };
    use crate::model::store::StoreKind;
    use crate::service::test_support::{amount, at, empty_vault, payment, store};

    const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";

    fn event(id: &str, time: &str) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: time.into(),
            block: Some(1),
            from: "0xaaaa".into(),
            to: "0xbbbb".into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDT)),
                amount: amount("500"),
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

    /// A vault with two operations and one fiat payment near them.
    fn setup() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.doc.crypto_accounts.push(CryptoAccount::new(
            "acc-1",
            "Main",
            Chain::Ethereum,
            "0xbbbb",
            "2026-01-01T00:00:00Z",
        ));
        vault.doc.crypto_tokens.push(CryptoToken::new(
            TokenId::new(Chain::Ethereum, USDT),
            "USDT",
            6,
        ));
        vault.doc.crypto_events = vec![
            event("near", "2026-11-11T14:20:00Z"),
            event("far", "2026-10-01T10:00:00Z"),
        ];
        let bank = store(&mut vault, "Bank", StoreKind::Cash, None);
        let mut spend = payment(&bank, "500", "to the exchanger");
        at(&mut spend, "2026-11-11T09:00:00Z");
        let id = vault.add_transaction(spend).unwrap().meta().id.clone();
        (dir, vault, id)
    }

    fn link(vault: &mut UnlockedVault, event_id: &str, fiat_tx_id: &str) {
        vault
            .crypto_link_fiat(&CryptoLinkInput {
                event_id: event_id.into(),
                fiat_tx_id: fiat_tx_id.into(),
            })
            .unwrap();
    }

    #[test]
    fn candidates_are_the_operations_close_in_time_and_write_nothing() {
        let (_dir, vault, _id) = setup();
        let before = vault.doc.crypto_overrides.clone();
        let options = vault.crypto_link_candidates(&CryptoLinkCandidatesInput {
            time: "2026-11-11T09:00:00Z".into(),
            ..Default::default()
        });
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].row.id, "near");
        assert_eq!(options[0].days, 0);
        assert_eq!(
            vault.doc.crypto_overrides, before,
            "a suggestion writes nothing"
        );
    }

    #[test]
    fn a_pointer_is_visible_from_both_sides_and_can_be_removed() {
        let (_dir, mut vault, id) = setup();
        link(&mut vault, "near", &id);
        for input in [
            CryptoPairsInput {
                event_id: Some("near".into()),
                ..Default::default()
            },
            CryptoPairsInput {
                fiat_tx_id: Some(id.clone()),
                ..Default::default()
            },
        ] {
            let pairs = vault.crypto_pairs(&input);
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].event.as_ref().unwrap().id, "near");
            assert_eq!(pairs[0].fiat.as_ref().unwrap().id, id);
        }
        vault
            .crypto_unlink_fiat(&CryptoUnlinkInput {
                event_id: "near".into(),
            })
            .unwrap();
        assert!(vault.crypto_pairs(&CryptoPairsInput::default()).is_empty());
        // Removing what is not there is not an error.
        vault
            .crypto_unlink_fiat(&CryptoUnlinkInput {
                event_id: "near".into(),
            })
            .unwrap();
    }

    #[test]
    fn a_missing_target_is_a_calm_empty_end_not_an_error() {
        let (_dir, mut vault, id) = setup();
        link(&mut vault, "near", &id);
        vault.delete_transaction(&id).unwrap();
        let pairs = vault.crypto_pairs(&CryptoPairsInput::default());
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].fiat.is_none());
        assert!(pairs[0].event.is_some());
        vault.doc.crypto_events.clear();
        let pairs = vault.crypto_pairs(&CryptoPairsInput::default());
        assert!(pairs[0].event.is_none() && pairs[0].fiat.is_none());
        // A pointer cannot be made to something that is not there.
        assert!(
            vault
                .crypto_link_fiat(&CryptoLinkInput {
                    event_id: "near".into(),
                    fiat_tx_id: id,
                })
                .is_err()
        );
    }

    #[test]
    fn a_pointer_changes_no_amount_no_total_and_no_chain() {
        let (_dir, mut vault, id) = setup();
        let at_time = "2026-11-12T00:00:00Z";
        let snapshot = |vault: &UnlockedVault| {
            (
                format!("{:?}", vault.crypto_totals(at_time)),
                serde_json::to_string(&vault.portfolio_summary()).unwrap(),
                // Holding ids are fresh on every recompute; what they hold is not.
                vault
                    .doc
                    .holdings
                    .iter()
                    .map(|held| {
                        format!(
                            "{}|{}|{}|{:?}",
                            held.store_id, held.asset.symbol, held.quantity, held.avg_cost
                        )
                    })
                    .collect::<Vec<_>>(),
                serde_json::to_string(&vault.doc.transactions).unwrap(),
                serde_json::to_string(&vault.doc.crypto_events).unwrap(),
            )
        };
        let without = snapshot(&vault);
        link(&mut vault, "near", &id);
        assert_eq!(snapshot(&vault), without);
        let fix = &vault.doc.crypto_overrides[0];
        assert_eq!(fix.flow_id, None, "a pointer joins no chain");
        assert!(vault.doc.transactions[0].meta().flow_id.is_none());
        // The same after a recompute from scratch.
        vault.crypto_sync_holdings().unwrap();
        assert_eq!(snapshot(&vault), without);
    }

    /// The pointer must stay in a gated, wiped collection: if someone moves it
    /// onto a fiat record it would travel past the level-3 gate.
    #[test]
    fn the_pointer_only_lives_in_a_gated_collection() {
        let doc = serde_json::to_value(crate::model::fixtures::full_doc()).unwrap();
        let mut holders = Vec::new();
        for (name, value) in doc.as_object().unwrap() {
            if value.to_string().contains("\"fiat_tx_id\"") {
                holders.push(name.clone());
            }
        }
        assert_eq!(holders, vec!["crypto_overrides".to_string()]);
        for name in holders {
            assert!(crate::sync::crypto_gate::is_crypto_collection(&name));
        }
    }

    /// A vault written before `CryptoOverride::fiat_tx_id` opens in both
    /// payload formats (JSON, and MessagePack with field names), keeps its
    /// correction, and the pointer reads as unset. The field is additive, so
    /// no format bump is needed (rule 5).
    #[test]
    fn a_vault_written_before_the_field_opens_in_json_and_pack() {
        use crate::model::vault::VaultDoc;
        let doc = crate::model::fixtures::full_doc();
        assert!(
            doc.crypto_overrides[0].fiat_tx_id.is_some(),
            "the fixture fills it"
        );
        let mut json: serde_json::Value = serde_json::to_value(&doc).unwrap();
        for fix in json["crypto_overrides"].as_array_mut().unwrap() {
            fix.as_object_mut().unwrap().remove("fiat_tx_id");
        }
        let from_json: VaultDoc =
            serde_json::from_slice(&serde_json::to_vec(&json).unwrap()).unwrap();
        let from_pack: VaultDoc =
            rmp_serde::from_slice(&rmp_serde::to_vec_named(&json).unwrap()).unwrap();
        for out in [from_json, from_pack] {
            assert_eq!(out.crypto_overrides.len(), doc.crypto_overrides.len());
            assert_eq!(out.crypto_overrides[0].fiat_tx_id, None);
            assert_eq!(
                out.crypto_overrides[0].flow_id,
                doc.crypto_overrides[0].flow_id
            );
            assert_eq!(out.contacts, doc.contacts);
        }
    }
}
