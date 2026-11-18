//! Rules every transaction must satisfy before it is stored (plan §4.5,
//! V1–V14), and the automatic categorization that runs right before them.
//! Free functions over the vault document: the same rules for every entry
//! (spec core-strangler, D2).

use std::collections::HashSet;

use crate::Amount;
use crate::error::{Error, Result, WalletRule};
use crate::model::category::CategoryGroup;
use crate::model::store::{Store, StoreKind};
use crate::model::transaction::{
    AccountRef, AdjustmentTx, DebtEffect, FuturesAction, KindDetails, LegRole, Transaction, TxKind,
    allowed_kinds,
};
use crate::model::vault::{VaultDoc, iso_to_millis};

/// How far into the future a timestamp may point (V11): a day of slack for
/// time zones and a clock that runs a little fast.
const MAX_FUTURE_MS: i64 = 24 * 60 * 60 * 1000;

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidInput(message.into())
}

fn positive(amount: Amount, what: &str) -> Result<()> {
    if amount.is_positive() {
        Ok(())
    } else {
        Err(invalid(format!("{what} must be positive")))
    }
}

/// Trim, lowercase and deduplicate category keys, keeping their order.
pub fn normalize_categories(tx: &mut Transaction) {
    let mut seen = HashSet::new();
    let categories = &mut tx.meta_mut().categories;
    for raw in categories.iter_mut() {
        *raw = raw.trim().to_lowercase();
    }
    categories.retain(|key| !key.is_empty() && seen.insert(key.clone()));
}

/// Fill empty categories from the first matching active rule (plan §8.1):
/// a case-insensitive substring of the comment or of a contact's name.
pub fn apply_categorization_rules(doc: &VaultDoc, tx: &mut Transaction) {
    if !tx.meta().categories.is_empty() || tx.kind() == TxKind::Adjustment {
        return;
    }
    let mut haystacks = vec![tx.meta().comment.to_lowercase()];
    for contact_id in tx.contact_ids() {
        if let Some(contact) = doc.contacts.iter().find(|contact| contact.id == contact_id) {
            haystacks.push(contact.name.to_lowercase());
        }
    }
    let mut rules: Vec<_> = doc
        .categorization_rules
        .iter()
        .filter(|rule| rule.active && !rule.pattern.trim().is_empty())
        .collect();
    rules.sort_by_key(|rule| rule.order);
    let matched = rules.into_iter().find(|rule| {
        let needle = rule.pattern.trim().to_lowercase();
        haystacks.iter().any(|text| text.contains(&needle))
    });
    if let Some(rule) = matched
        && doc
            .categories
            .iter()
            .any(|category| category.key == rule.category && !category.archived)
    {
        tx.meta_mut().categories = vec![rule.category.clone()];
    }
}

pub fn live_store<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc Store> {
    match doc.stores.iter().find(|store| store.id == id) {
        Some(store) if store.archived => Err(Error::Archived(id.into())),
        Some(store) => Ok(store),
        None => Err(Error::Missing(format!("store {id}"))),
    }
}

/// V16: a wallet the crypto module reads (`Store::crypto_account_id`)
/// takes no hand-made entry. The engine writes its operations itself, so a
/// ledger entry beside them counts the same money twice. The interface
/// hides the actions of such a wallet; this makes the rule hold for every
/// other way in — a transfer from another wallet, an automation, a bulk
/// edit, a balance adjustment.
pub fn refuse_engine_wallet(store: &Store) -> Result<()> {
    if store.crypto_account_id.is_some() {
        return Err(invalid(
            "this wallet is read by the crypto module and takes no hand-made operations",
        ));
    }
    Ok(())
}

/// V7: a contact side names an existing contact, and an active account of it.
pub fn check_account(doc: &VaultDoc, account: &AccountRef) -> Result<()> {
    match account {
        AccountRef::Store(id) => live_store(doc, id).map(|_| ()),
        AccountRef::Contact {
            contact_id,
            account_id,
        } => {
            let contact = doc
                .contacts
                .iter()
                .find(|contact| &contact.id == contact_id)
                .ok_or_else(|| Error::Missing(format!("contact {contact_id}")))?;
            if let Some(account_id) = account_id {
                let account = contact
                    .accounts
                    .iter()
                    .find(|account| &account.id == account_id)
                    .ok_or_else(|| invalid("the account does not belong to this contact"))?;
                if !account.active {
                    return Err(invalid("this account is no longer active"));
                }
            }
            Ok(())
        }
        AccountRef::Raw(text) => {
            if text.trim().is_empty() {
                Err(invalid("the counterparty must not be empty"))
            } else {
                Ok(())
            }
        }
    }
}

/// V8: details only for the store kinds they make sense for.
pub fn check_details(store: &Store, details: &KindDetails, amount: Amount) -> Result<()> {
    if let Some(notes) = &details.cash_breakdown {
        if store.kind != StoreKind::Cash {
            return Err(invalid("a banknote count is only for cash"));
        }
        let too_large = || invalid("the banknote total is too large");
        let mut total = Amount::ZERO;
        for note in notes {
            let count = Amount::new(rust_decimal::Decimal::from(note.count));
            let value = note.denomination.checked_mul(count).ok_or_else(too_large)?;
            total = total.checked_add(value).ok_or_else(too_large)?;
        }
        if !total.within_input_bound() {
            return Err(too_large());
        }
        if total != amount {
            return Err(invalid("the banknotes do not add up to the amount"));
        }
    }
    if details.unit_price.is_some() && store.kind != StoreKind::Property {
        return Err(invalid("a unit price is only for property"));
    }
    if let Some(price) = details.unit_price {
        positive(price, "the unit price")?;
    }
    let crypto = matches!(store.kind, StoreKind::Exchange | StoreKind::Crypto);
    if (details.network.is_some()
        || details
            .tx_hash
            .as_deref()
            .is_some_and(|hash| !hash.is_empty()))
        && !crypto
    {
        return Err(invalid("a network and a hash are only for crypto wallets"));
    }
    Ok(())
}

/// A correction: at least one asset, each once, each one the account may
/// hold; a real balance below zero only on a loan or a credit card.
pub fn check_adjustment(doc: &VaultDoc, tx: &AdjustmentTx) -> Result<()> {
    let store = live_store(doc, &tx.store_id)?;
    if tx.lines.is_empty() {
        return Err(invalid("a correction needs at least one asset"));
    }
    if tx.fee.is_some() {
        return Err(invalid("a correction has no fee"));
    }
    if !tx.meta.categories.is_empty() {
        return Err(invalid("a correction is neither income nor spending"));
    }
    let mut seen = HashSet::new();
    for line in &tx.lines {
        if !seen.insert(line.asset.key()) {
            return Err(invalid(format!("{} is listed twice", line.asset.symbol)));
        }
        if line.after.is_negative() && !store.is_debt() {
            return Err(invalid(format!(
                "{}: a real balance cannot be below zero",
                line.asset.symbol
            )));
        }
        if !store.binding.admits(&line.asset) {
            return Err(Error::WalletRule(WalletRule::BindingMismatch));
        }
        if matches!(store.kind, StoreKind::Cash | StoreKind::Bank) && !line.asset.is_fiat() {
            return Err(invalid("cash and bank accounts hold only fiat"));
        }
    }
    Ok(())
}

/// Every store a transaction is booked on, with the kind it must accept.
pub fn store_sides(tx: &Transaction) -> Vec<&str> {
    match tx {
        Transaction::Topup(inner) => vec![&inner.store_id],
        Transaction::Payment(inner) => vec![&inner.store_id],
        Transaction::Transfer(inner) => [inner.from.store_id(), inner.to.store_id()]
            .into_iter()
            .flatten()
            .collect(),
        Transaction::Convert(inner) => [inner.from.account.store_id(), inner.to.account.store_id()]
            .into_iter()
            .flatten()
            .collect(),
        Transaction::TradeSpot(inner) => vec![&inner.store_id],
        Transaction::TradeFutures(inner) => vec![&inner.store_id],
        Transaction::Defi(inner) => vec![&inner.store_id],
        Transaction::AssetSell(inner) => vec![&inner.store_id],
        Transaction::Adjustment(inner) => vec![&inner.store_id],
    }
}

/// Plan §4.5, V1–V14 (V13 — negative balances — is a warning, not an error),
/// with V11 measured from `now_ms`: the caller owns the clock. The hash shape
/// check (V14) needs `txinfo`, a sibling group, so the scenario adds it.
pub fn validate_transaction_at(doc: &VaultDoc, tx: &Transaction, now_ms: i64) -> Result<()> {
    let kind = tx.kind();

    // V1, V2
    for id in store_sides(tx) {
        let store = live_store(doc, id)?;
        refuse_engine_wallet(store)?;
        if !allowed_kinds(store.kind).contains(&kind) {
            return Err(invalid(format!(
                "a {} is not possible on this kind of wallet",
                kind.tag()
            )));
        }
    }
    if let Transaction::AssetSell(inner) = tx
        && let Some(target) = &inner.proceeds_to
    {
        refuse_engine_wallet(live_store(doc, target)?)?;
    }

    // V3: what each kind of store may hold.
    for leg in tx
        .legs()
        .into_iter()
        .filter(|leg| matches!(leg.role, LegRole::Principal | LegRole::Adjustment))
    {
        let store = live_store(doc, &leg.store_id)?;
        // V15: a sub-account holds only what it is bound to.
        if !store.binding.admits(&leg.asset) {
            return Err(Error::WalletRule(WalletRule::BindingMismatch));
        }
        match store.kind {
            StoreKind::Cash | StoreKind::Bank if !leg.asset.is_fiat() => {
                return Err(invalid("cash and bank accounts hold only fiat"));
            }
            StoreKind::Property if leg.asset.coingecko_id.is_some() || leg.asset.is_fiat() => {
                return Err(invalid("a property wallet holds things, not money"));
            }
            _ => {}
        }
    }

    // V4, V6, V8, V9, V14 — per variant
    match tx {
        Transaction::Topup(inner) => {
            positive(inner.amount, "the amount")?;
            if let Some(AccountRef::Store(_)) = inner.from {
                return Err(invalid("money from your own wallet is a transfer"));
            }
            if let Some(from) = &inner.from {
                check_account(doc, from)?;
            }
            let store = live_store(doc, &inner.store_id)?;
            check_details(store, &inner.details, inner.amount)?;
            if store.kind == StoreKind::Property && inner.details.unit_price.is_none() {
                return Err(invalid("buying property needs a unit price"));
            }
        }
        Transaction::Payment(inner) => {
            positive(inner.amount, "the amount")?;
            if let Some(AccountRef::Store(_)) = inner.to {
                return Err(invalid("money to your own wallet is a transfer"));
            }
            if let Some(to) = &inner.to {
                check_account(doc, to)?;
            }
            let store = live_store(doc, &inner.store_id)?;
            check_details(store, &inner.details, inner.amount)?;
        }
        Transaction::Transfer(inner) => {
            positive(inner.amount, "the amount")?;
            if inner.from.store_id().is_none() && inner.to.store_id().is_none() {
                return Err(invalid("one side of a transfer must be your wallet"));
            }
            if inner.from == inner.to {
                return Err(invalid("a transfer needs two different sides"));
            }
            check_account(doc, &inner.from)?;
            check_account(doc, &inner.to)?;
            if let Some(effect) = inner.debt {
                let (store_side, contact_side) = if effect.is_outgoing() {
                    (&inner.from, &inner.to)
                } else {
                    (&inner.to, &inner.from)
                };
                if store_side.store_id().is_none() || contact_side.contact_id().is_none() {
                    return Err(invalid(match effect {
                        DebtEffect::Lend | DebtEffect::Repay => {
                            "lending or repaying sends money from your wallet to a contact"
                        }
                        DebtEffect::Borrow | DebtEffect::Collect => {
                            "borrowing or collecting brings money from a contact to your wallet"
                        }
                    }));
                }
            }
        }
        Transaction::Convert(inner) => {
            positive(inner.from.amount, "the amount given")?;
            positive(inner.to.amount, "the amount received")?;
            if inner.from.account.store_id().is_none() && inner.to.account.store_id().is_none() {
                return Err(invalid("one side of a conversion must be your wallet"));
            }
            if inner.from.asset.key() == inner.to.asset.key() {
                return Err(invalid("a conversion needs two different assets"));
            }
            if let Some(rate) = inner.rate_override {
                positive(rate, "the rate")?;
            }
            check_account(doc, &inner.from.account)?;
            check_account(doc, &inner.to.account)?;
        }
        Transaction::TradeSpot(inner) => {
            positive(inner.sold.qty, "the amount sold")?;
            positive(inner.bought.qty, "the amount bought")?;
            if inner.sold.asset.key() == inner.bought.asset.key() {
                return Err(invalid("a trade needs two different assets"));
            }
        }
        Transaction::TradeFutures(inner) => {
            positive(inner.quantity, "the quantity")?;
            positive(inner.fill_price, "the price")?;
            if !inner.contract.is_derivative() {
                return Err(invalid("a futures trade needs a contract"));
            }
            if inner.closed_pnl.is_some() && inner.action != FuturesAction::Close {
                return Err(invalid("realized PnL belongs to a closing trade"));
            }
            if let Some(leverage) = inner.leverage {
                positive(leverage, "the leverage")?;
            }
        }
        Transaction::Defi(inner) => positive(inner.amount, "the amount")?,
        Transaction::AssetSell(inner) => {
            positive(inner.quantity, "the quantity")?;
            positive(inner.unit_price, "the price")?;
        }
        Transaction::Adjustment(inner) => check_adjustment(doc, inner)?,
    }
    if let Some(fee) = tx.fee()
        && fee.amount.is_negative()
    {
        return Err(invalid("a fee must not be negative"));
    }

    // Principal legs must not cancel each other out on one (store, asset).
    let principal: Vec<_> = tx
        .legs()
        .into_iter()
        .filter(|leg| leg.role == LegRole::Principal)
        .collect();
    for (index, leg) in principal.iter().enumerate() {
        if principal[..index].iter().any(|earlier| {
            earlier.store_id == leg.store_id && earlier.asset.key() == leg.asset.key()
        }) {
            return Err(invalid("both sides land on the same wallet and asset"));
        }
    }

    check_meta(doc, tx, now_ms)
}

/// V5, V10, V11, V12 — the shared fields.
fn check_meta(doc: &VaultDoc, tx: &Transaction, now_ms: i64) -> Result<()> {
    let meta = tx.meta();
    let kind = tx.kind();

    // V11
    let millis = iso_to_millis(&meta.timestamp);
    if millis == 0 && !meta.timestamp.starts_with("1970-01-01T00:00:00") {
        return Err(invalid("the date is not a valid RFC3339 timestamp"));
    }
    if millis > now_ms + MAX_FUTURE_MS {
        return Err(invalid("the date is too far in the future"));
    }

    // V5
    if matches!(kind, TxKind::Topup | TxKind::Payment) && meta.categories.is_empty() {
        return Err(invalid("choose a category"));
    }

    // V10, V12
    for key in &meta.categories {
        let category = doc
            .categories
            .iter()
            .find(|category| &category.key == key)
            .ok_or_else(|| invalid(format!("unknown category {key}")))?;
        if category.archived {
            return Err(invalid(format!("category {key} is archived")));
        }
        let fits = match kind {
            TxKind::Topup => matches!(
                category.group,
                CategoryGroup::Income | CategoryGroup::Service
            ),
            TxKind::Payment => !matches!(category.group, CategoryGroup::Income),
            _ => true,
        };
        if !fits {
            return Err(invalid(format!(
                "category {key} does not fit a {}",
                kind.tag()
            )));
        }
    }
    if meta
        .flow_id
        .as_deref()
        .is_some_and(|flow| flow.trim().is_empty())
    {
        return Err(invalid("a chain id must not be empty"));
    }
    if let Some(contact_id) = &meta.contact_id
        && !doc.contacts.iter().any(|contact| &contact.id == contact_id)
    {
        return Err(Error::Missing(format!("contact {contact_id}")));
    }
    Ok(())
}
