//! Envelopes: earmarking parts of a store's balance without moving money.
//!
//! Invariant on write: the envelopes of one `(store, asset)` never hold more
//! than the store has. Spending can still push the balance under what is
//! earmarked; such envelopes are reported as short, never silently cut.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::planning::Envelope;
use crate::model::store::AssetRef;
use crate::model::vault::now_iso;
use crate::service::UnlockedVault;

/// What creating or editing an envelope takes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct EnvelopeDraft {
    pub store_id: String,
    pub name: String,
    pub asset: AssetRef,
    pub target: Option<Amount>,
    pub goal_id: Option<String>,
}

/// An envelope with what its store can still give it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct EnvelopeView {
    pub envelope: Envelope,
    /// Balance of the asset in the store not earmarked by any envelope; may
    /// be negative when spending ate into the envelopes.
    pub free: Amount,
    /// `allocated / target` in percent, when there is a target.
    pub pct: Option<f64>,
    /// The store holds less than its envelopes earmark.
    pub short: bool,
}

impl UnlockedVault {
    /// Sum earmarked by every envelope of one asset in one store.
    pub fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount {
        self.doc
            .envelopes
            .iter()
            .filter(|envelope| envelope.store_id == store_id && envelope.asset.key() == asset_key)
            .fold(Amount::ZERO, |sum, envelope| sum + envelope.allocated)
    }

    /// Balance of the asset in the store that no envelope claims.
    pub fn free_balance(&self, store_id: &str, asset: &AssetRef) -> Amount {
        let key = asset.key();
        let held = self
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == store_id && holding.asset.key() == key)
            .fold(Amount::ZERO, |sum, holding| sum + holding.quantity);
        held - self.earmarked(store_id, &key)
    }

    /// Envelopes of one store, or of every store with `None`.
    pub fn envelopes(&self, store_id: Option<&str>) -> Vec<EnvelopeView> {
        self.doc
            .envelopes
            .iter()
            .filter(|envelope| store_id.is_none_or(|id| envelope.store_id == id))
            .map(|envelope| {
                let free = self.free_balance(&envelope.store_id, &envelope.asset);
                let pct = envelope
                    .target
                    .filter(|target| !target.is_zero())
                    .map(|target| (envelope.allocated / target).to_f64() * 100.0);
                EnvelopeView {
                    envelope: envelope.clone(),
                    free,
                    pct,
                    short: free.is_negative(),
                }
            })
            .collect()
    }

    fn check_envelope_draft(&self, draft: &EnvelopeDraft) -> Result<()> {
        if draft.name.trim().is_empty() {
            return Err(Error::InvalidInput("an envelope needs a name".into()));
        }
        let store = self.live_store(&draft.store_id)?;
        if !store.binding.admits(&draft.asset) {
            return Err(Error::WalletRule(crate::error::WalletRule::BindingMismatch));
        }
        if draft.target.is_some_and(|target| !target.is_positive()) {
            return Err(Error::InvalidInput("the target must be positive".into()));
        }
        if let Some(goal_id) = &draft.goal_id
            && !self.doc.goals.iter().any(|goal| &goal.id == goal_id)
        {
            return Err(Error::Missing(format!("goal {goal_id}")));
        }
        Ok(())
    }

    pub fn create_envelope(&mut self, draft: EnvelopeDraft) -> Result<Envelope> {
        self.check_envelope_draft(&draft)?;
        let envelope = Envelope {
            id: uuid::Uuid::new_v4().to_string(),
            store_id: draft.store_id,
            name: draft.name.trim().to_string(),
            asset: draft.asset,
            allocated: Amount::ZERO,
            target: draft.target,
            goal_id: draft.goal_id,
            created_at: now_iso(),
        };
        self.doc.envelopes.push(envelope.clone());
        self.persist()?;
        Ok(envelope)
    }

    /// Rename, retarget or relink an envelope. Its store and asset are fixed
    /// once money is earmarked.
    pub fn update_envelope(&mut self, id: &str, draft: EnvelopeDraft) -> Result<Envelope> {
        self.check_envelope_draft(&draft)?;
        let envelope = self.envelope_mut(id)?;
        let moved =
            envelope.store_id != draft.store_id || envelope.asset.key() != draft.asset.key();
        if moved && !envelope.allocated.is_zero() {
            return Err(Error::InvalidInput(
                "empty the envelope before moving it to another wallet or asset".into(),
            ));
        }
        envelope.store_id = draft.store_id;
        envelope.asset = draft.asset;
        envelope.name = draft.name.trim().to_string();
        envelope.target = draft.target;
        envelope.goal_id = draft.goal_id;
        let out = envelope.clone();
        self.persist()?;
        Ok(out)
    }

    /// Earmark more (`amount > 0`, taken from the free balance) or release
    /// (`amount < 0`, back to free). Releasing below zero is refused.
    pub fn fund_envelope(&mut self, id: &str, amount: Amount) -> Result<Envelope> {
        let envelope = self
            .doc
            .envelopes
            .iter()
            .find(|envelope| envelope.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing("envelope".into()))?;
        if amount.is_positive() && self.free_balance(&envelope.store_id, &envelope.asset) < amount {
            return Err(Error::InsufficientFunds(format!(
                "only {} {} is free in this wallet",
                self.free_balance(&envelope.store_id, &envelope.asset),
                envelope.asset.symbol
            )));
        }
        if (envelope.allocated + amount).is_negative() {
            return Err(Error::InvalidInput(
                "the envelope holds less than that".into(),
            ));
        }
        let target = self.envelope_mut(id)?;
        target.allocated += amount;
        let out = target.clone();
        self.persist()?;
        Ok(out)
    }

    /// Move earmarked money between two envelopes of the same store and asset.
    pub fn move_between_envelopes(&mut self, from: &str, to: &str, amount: Amount) -> Result<()> {
        if !amount.is_positive() {
            return Err(Error::InvalidInput("the amount must be positive".into()));
        }
        let find = |id: &str| {
            self.doc
                .envelopes
                .iter()
                .find(|envelope| envelope.id == id)
                .cloned()
                .ok_or_else(|| Error::Missing("envelope".into()))
        };
        let (source, target) = (find(from)?, find(to)?);
        if source.store_id != target.store_id || source.asset.key() != target.asset.key() {
            return Err(Error::InvalidInput(
                "envelopes must share the wallet and the asset".into(),
            ));
        }
        if source.allocated < amount {
            return Err(Error::InvalidInput(
                "the envelope holds less than that".into(),
            ));
        }
        self.envelope_mut(from)?.allocated -= amount;
        self.envelope_mut(to)?.allocated += amount;
        self.persist()
    }

    /// Delete an envelope; whatever it held simply becomes free again.
    pub fn delete_envelope(&mut self, id: &str) -> Result<()> {
        let before = self.doc.envelopes.len();
        self.doc.envelopes.retain(|envelope| envelope.id != id);
        if self.doc.envelopes.len() == before {
            return Err(Error::Missing("envelope".into()));
        }
        self.persist()
    }

    /// Value of the envelopes linked to a goal, in `currency` at `now`.
    pub(crate) fn envelope_saved_for_goal(
        &self,
        goal_id: &str,
        currency: &str,
        now: i64,
    ) -> Option<Amount> {
        let linked: Vec<&Envelope> = self
            .doc
            .envelopes
            .iter()
            .filter(|envelope| envelope.goal_id.as_deref() == Some(goal_id))
            .collect();
        if linked.is_empty() {
            return None;
        }
        let book = self.rate_book();
        let total = linked.iter().fold(Amount::ZERO, |sum, envelope| {
            let usd = envelope.allocated * self.price_usd_for(&envelope.asset.key());
            sum + book
                .convert(usd, "usd", currency, now)
                .unwrap_or(Amount::ZERO)
        });
        Some(total)
    }

    fn envelope_mut(&mut self, id: &str) -> Result<&mut Envelope> {
        self.doc
            .envelopes
            .iter_mut()
            .find(|envelope| envelope.id == id)
            .ok_or_else(|| Error::Missing("envelope".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::service::test_support::{amount, empty_vault, income, payment, store, usd};

    fn draft(store_id: &str, name: &str) -> EnvelopeDraft {
        EnvelopeDraft {
            store_id: store_id.into(),
            name: name.into(),
            asset: usd(),
            target: Some(amount("100")),
            goal_id: None,
        }
    }

    #[test]
    fn envelopes_never_earmark_more_than_the_store_has() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        vault.add_transaction(income(&cash, "100", "pay")).unwrap();
        let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
        let tax = vault.create_envelope(draft(&cash, "Tax")).unwrap();

        vault.fund_envelope(&trip.id, amount("70")).unwrap();
        assert_eq!(vault.free_balance(&cash, &usd()), amount("30"));
        assert!(vault.fund_envelope(&tax.id, amount("40")).is_err());
        vault
            .move_between_envelopes(&trip.id, &tax.id, amount("20"))
            .unwrap();
        assert_eq!(vault.earmarked(&cash, &usd().key()), amount("70"));
        assert!(vault.fund_envelope(&tax.id, amount("-30")).is_err());

        // spending into the envelopes is allowed but reported
        vault.add_transaction(payment(&cash, "50", "food")).unwrap();
        let views = vault.envelopes(Some(&cash));
        assert!(views.iter().all(|view| view.short));
        assert_eq!(views[0].free, amount("-20"));

        vault.delete_envelope(&trip.id).unwrap();
        assert_eq!(vault.free_balance(&cash, &usd()), amount("30"));
    }

    #[test]
    fn a_bound_store_only_takes_envelopes_of_its_asset() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = store(&mut vault, "Card", StoreKind::Bank, Some(&bank));
        let mut eur = draft(&card, "Euro");
        eur.asset = AssetRef::fiat("EUR", "Euro");
        assert!(vault.create_envelope(eur).is_err());
        vault.create_envelope(draft(&card, "Usd")).unwrap();
    }
}
