//! Envelopes: earmarking parts of a store's balance without moving money.
//!
//! Invariant on write: the envelopes of one `(store, asset)` never hold more
//! than the store has. Spending can still push the balance under what is
//! earmarked; such envelopes are reported as short, never silently cut.
//!
//! Pure part of the area: reads and decisions over a document or a read
//! context. The writes are the scenarios in `usecases::household::envelopes`.

use crate::Amount;
use crate::error::{Error, Result, WalletRule};
use crate::kernel::books::Books;
use crate::model::ids::StoreId;
use crate::model::planning::Envelope;
use crate::model::store::{AssetRef, Store};
use crate::model::vault::VaultDoc;

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

/// What `fund_goal` will do: the asset the goal's currency is held in, what of
/// the amount is free to take, and the goal's envelope there if it exists.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalFunding {
    pub goal_name: String,
    pub asset: AssetRef,
    pub take: Amount,
    pub envelope_id: Option<String>,
}

/// Sum earmarked by every envelope of one asset in one store.
pub fn earmarked(doc: &VaultDoc, store_id: &StoreId, asset_key: &str) -> Amount {
    doc.envelopes
        .iter()
        .filter(|envelope| {
            envelope.store_id == store_id.as_str() && envelope.asset.key() == asset_key
        })
        .fold(Amount::ZERO, |sum, envelope| sum + envelope.allocated)
}

/// Balance of the asset in the store that no envelope claims.
pub fn free_balance(doc: &VaultDoc, store_id: &StoreId, asset: &AssetRef) -> Amount {
    let key = asset.key();
    let held = doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == store_id.as_str() && holding.asset.key() == key)
        .fold(Amount::ZERO, |sum, holding| sum + holding.quantity);
    held - earmarked(doc, store_id, &key)
}

/// Envelopes of one store, or of every store with `None`.
pub fn envelopes(doc: &VaultDoc, store_id: Option<StoreId>) -> Vec<EnvelopeView> {
    doc.envelopes
        .iter()
        .filter(|envelope| {
            store_id
                .as_ref()
                .is_none_or(|id| envelope.store_id == id.as_str())
        })
        .map(|envelope| {
            let free = free_balance(doc, &StoreId::from(&envelope.store_id), &envelope.asset);
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

/// The part of a draft check that needs nothing but the draft.
pub fn check_draft_name(draft: &EnvelopeDraft) -> Result<()> {
    if draft.name.trim().is_empty() {
        return Err(Error::InvalidInput("an envelope needs a name".into()));
    }
    Ok(())
}

/// The part of a draft check that reads the draft's (live) store and the goals.
pub fn check_draft_against(doc: &VaultDoc, store: &Store, draft: &EnvelopeDraft) -> Result<()> {
    if !store.binding.admits(&draft.asset) {
        return Err(Error::WalletRule(WalletRule::BindingMismatch));
    }
    if draft.target.is_some_and(|target| !target.is_positive()) {
        return Err(Error::InvalidInput("the target must be positive".into()));
    }
    if let Some(goal_id) = &draft.goal_id
        && !doc.goals.iter().any(|goal| &goal.id == goal_id)
    {
        return Err(Error::Missing(format!("goal {goal_id}")));
    }
    Ok(())
}

/// A new envelope holding nothing.
pub fn new_envelope(draft: EnvelopeDraft, id: String, created_at: String) -> Envelope {
    Envelope {
        id,
        store_id: draft.store_id,
        name: draft.name.trim().to_string(),
        asset: draft.asset,
        allocated: Amount::ZERO,
        moves: Vec::new(),
        target: draft.target,
        goal_id: draft.goal_id,
        created_at,
    }
}

fn find_envelope<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc Envelope> {
    doc.envelopes
        .iter()
        .find(|envelope| envelope.id == id)
        .ok_or_else(|| Error::Missing("envelope".into()))
}

/// Rename, retarget or relink an envelope: the envelope to write. Its store
/// and asset are fixed once money is earmarked.
pub fn plan_update_envelope(doc: &VaultDoc, id: &str, draft: EnvelopeDraft) -> Result<Envelope> {
    let mut envelope = find_envelope(doc, id)?.clone();
    let moved = envelope.store_id != draft.store_id || envelope.asset.key() != draft.asset.key();
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
    Ok(envelope)
}

/// Earmark more (`amount > 0`, taken from the free balance) or release
/// (`amount < 0`, back to free). Releasing below zero is refused.
pub fn plan_fund_envelope(doc: &VaultDoc, id: &str, amount: Amount) -> Result<()> {
    let envelope = find_envelope(doc, id)?;
    let store_id = StoreId::from(&envelope.store_id);
    if amount.is_positive() {
        let free = free_balance(doc, &store_id, &envelope.asset);
        if free < amount {
            return Err(Error::InsufficientFunds(format!(
                "only {free} {} is free in this wallet",
                envelope.asset.symbol
            )));
        }
    }
    if (envelope.allocated + amount).is_negative() {
        return Err(Error::InvalidInput(
            "the envelope holds less than that".into(),
        ));
    }
    Ok(())
}

/// Move earmarked money between two envelopes of the same store and asset.
pub fn plan_move_between_envelopes(
    doc: &VaultDoc,
    from: &str,
    to: &str,
    amount: Amount,
) -> Result<()> {
    if !amount.is_positive() {
        return Err(Error::InvalidInput("the amount must be positive".into()));
    }
    let (source, target) = (find_envelope(doc, from)?, find_envelope(doc, to)?);
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
    Ok(())
}

/// Put aside up to `amount` of the goal's currency in `store_id` for the
/// goal: only the free balance is taken (`take` is zero when nothing is free).
pub fn plan_fund_goal(
    doc: &VaultDoc,
    goal_id: &str,
    store_id: &StoreId,
    amount: Amount,
) -> Result<GoalFunding> {
    if !amount.is_positive() {
        return Err(Error::InvalidInput("the amount must be positive".into()));
    }
    let goal = doc
        .goals
        .iter()
        .find(|goal| goal.id == goal_id)
        .ok_or_else(|| Error::Missing(format!("goal {goal_id}")))?;
    let asset = doc
        .holdings
        .iter()
        .find(|holding| {
            holding.store_id == store_id.as_str()
                && holding.asset.symbol.eq_ignore_ascii_case(&goal.currency)
        })
        .map(|holding| holding.asset.clone())
        .ok_or_else(|| Error::InvalidInput(format!("the wallet holds no {}", goal.currency)))?;
    let take = amount.min(free_balance(doc, store_id, &asset));
    let envelope_id = doc
        .envelopes
        .iter()
        .find(|envelope| {
            envelope.store_id == store_id.as_str()
                && envelope.asset.key() == asset.key()
                && envelope.goal_id.as_deref() == Some(goal_id)
        })
        .map(|envelope| envelope.id.clone());
    Ok(GoalFunding {
        goal_name: goal.name.clone(),
        asset,
        take,
        envelope_id,
    })
}

/// Value of the envelopes linked to a goal, in `currency` at `now`.
pub fn envelope_saved_for_goal(
    books: &Books<'_>,
    goal_id: &str,
    currency: &str,
    now: i64,
) -> Option<Amount> {
    let linked: Vec<&Envelope> = books
        .doc()
        .envelopes
        .iter()
        .filter(|envelope| envelope.goal_id.as_deref() == Some(goal_id))
        .collect();
    if linked.is_empty() {
        return None;
    }
    let book = books.rate_book();
    let total = linked.iter().fold(Amount::ZERO, |sum, envelope| {
        let usd = envelope.allocated * books.price_usd(&envelope.asset.key());
        sum + book
            .convert(usd, "usd", currency, now)
            .unwrap_or(Amount::ZERO)
    });
    Some(total)
}

/// Give every envelope its opening entry and re-derive `allocated` from
/// the journal (on open, and after a sync brought records in). True when
/// something changed.
pub fn settle_envelopes(doc: &mut VaultDoc) -> bool {
    let mut changed = false;
    for envelope in &mut doc.envelopes {
        changed |= envelope.settle_moves();
    }
    changed
}
