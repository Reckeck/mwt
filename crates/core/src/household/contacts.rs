//! Contacts: who money moves to and from, their saved accounts, turnover and
//! personal debts (plan §5, §4.8).
//!
//! Pure part of the area: the checks behind each write and the reads over a
//! document or a read context. The writes are the scenarios in
//! `usecases::household::contacts`.

use std::collections::{BTreeMap, HashSet};

use crate::Amount;
use crate::error::{Error, Result};
use crate::household::shared_events::view::{shared_debts, shared_tx_links};
use crate::model::VaultDoc;
use crate::model::contact::{Contact, ContactAccount, ContactKind, DebtTerms};
use crate::model::store::AssetRef;
use crate::model::transaction::{AccountRef, DebtEffect, LegRole, Transaction};
use crate::money::books::Books;
use crate::money::tree::root_of;
use crate::time::secs_or_now;

/// Money moved with one contact in one asset.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TurnoverRow {
    pub asset: AssetRef,
    pub sent: Amount,
    pub received: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ContactTurnover {
    pub contact_id: String,
    pub account_id: Option<String>,
    pub base_currency: String,
    pub rows: Vec<TurnoverRow>,
    /// Sums at the price stamped on each transaction; unpriced legs count as zero.
    pub sent_base: Amount,
    pub received_base: Amount,
    pub tx_count: u32,
}

/// Personal debt with one contact in one asset: what they owe me and what I owe them.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ContactBalance {
    pub contact_id: String,
    pub asset: AssetRef,
    pub receivable: Amount,
    pub payable: Amount,
}

/// One row of the contacts table: activity and debts in the base currency.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ContactSummary {
    pub contact_id: String,
    /// Operations with the contact; internal moves do not count.
    pub tx_count: u32,
    pub last_tx_at: Option<String>,
    /// Signed from the user's side (negative: money went to the contact).
    pub last_amount: Option<Amount>,
    pub last_asset: Option<String>,
    /// What they owe me, in the base currency.
    pub receivable_base: Amount,
    /// What I owe them (positive), in the base currency.
    pub payable_base: Amount,
    /// A debt currency had no rate: it is left out of the base sums.
    pub missing_rate: bool,
}

/// What the UI sends to create or edit a contact. Accounts without an id are new.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ContactDraft {
    pub name: String,
    pub kind: ContactKind,
    pub note: String,
    pub accounts: Vec<ContactAccount>,
    /// Directory group ("Family", «P2P»); empty = grouped by kind.
    #[serde(default)]
    pub group: String,
}

/// Due date and note for a debt with a contact in one asset.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct DebtTermsDraft {
    pub contact_id: String,
    pub asset: String,
    pub due_at: Option<String>,
    pub note: String,
}

/// What the shared events add to the personal debts. They live in the
/// not-yet-moved events area, so the scenario reads them and passes them in.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EventDebts {
    /// The transfers an event made: its balance already counts them.
    pub tx_ids: HashSet<String>,
    pub debts: Vec<EventDebt>,
}

/// One event's debt with one contact; positive means the contact owes me.
#[derive(Debug, Clone, PartialEq)]
pub struct EventDebt {
    pub contact_id: String,
    pub currency: String,
    pub amount: Amount,
}

/// Personal debts from debt transfers made up to `secs`, plus what the
/// shared events stood at then.
pub fn contact_balances_until(books: &Books<'_>, secs: i64) -> Vec<ContactBalance> {
    let doc = books.doc();
    balances_until(doc, secs, &event_debts(doc, secs))
}

/// The shared events' part of the debts as of `secs`.
pub fn event_debts(doc: &VaultDoc, secs: i64) -> EventDebts {
    EventDebts {
        tx_ids: shared_tx_links(doc)
            .into_iter()
            .map(|link| link.tx_id)
            .collect(),
        debts: shared_debts(doc, secs)
            .into_iter()
            .map(|debt| EventDebt {
                contact_id: debt.contact_id,
                currency: debt.currency,
                amount: debt.amount,
            })
            .collect(),
    }
}

/// The fields `update_contact` writes, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactEdit {
    pub name: String,
    pub kind: ContactKind,
    pub note: String,
    pub accounts: Vec<ContactAccount>,
    pub group: String,
}

/// What `set_debt_terms` will write: the terms of one debt in one asset are
/// replaced by this due date and note, or cleared when both are empty.
#[derive(Debug, Clone, PartialEq)]
pub struct DebtTermsPlan {
    pub contact_id: String,
    /// Upper-case ticker of the debt's asset.
    pub asset: String,
    pub due_at: Option<String>,
    pub note: String,
}

impl DebtTermsPlan {
    /// Neither a date nor a note: the entry is removed, not replaced.
    pub fn clears(&self) -> bool {
        self.due_at.is_none() && self.note.is_empty()
    }
}

/// The contact with this id, or `Missing`.
pub fn require_contact<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc Contact> {
    doc.contacts
        .iter()
        .find(|contact| contact.id == id)
        .ok_or_else(|| Error::Missing(format!("contact {id}")))
}

/// Whether any transaction or store points at the contact (or one account of it).
pub fn contact_in_use(doc: &VaultDoc, contact_id: &str, account_id: Option<&str>) -> bool {
    let named_by_tx = doc
        .transactions
        .iter()
        .any(|tx| involves(tx, contact_id, account_id));
    let named_by_store = account_id.is_none()
        && doc
            .stores
            .iter()
            .any(|store| store.contact_id.as_deref() == Some(contact_id));
    // A guest of a shared event who is linked to the contact (not to one
    // of its accounts) counts as a use too.
    let named_by_event = account_id.is_none()
        && doc.shared_events.iter().any(|event| {
            event
                .participants
                .iter()
                .any(|person| person.contact_id.as_deref() == Some(contact_id))
        });
    named_by_tx || named_by_store || named_by_event
}

/// Trim the accounts, refuse one without a value and give a new one (no id
/// yet) an id and mark it usable.
pub fn clean_accounts(accounts: Vec<ContactAccount>) -> Result<Vec<ContactAccount>> {
    accounts
        .into_iter()
        .map(|mut account| {
            account.value = account.value.trim().to_string();
            account.label = account.label.trim().to_string();
            if account.value.is_empty() {
                return Err(Error::InvalidInput("an account needs a value".into()));
            }
            // A new account (no id yet) is always usable.
            if account.id.trim().is_empty() {
                account.id = uuid::Uuid::new_v4().to_string();
                account.active = true;
            }
            account.group = account
                .group
                .map(|group| group.trim().to_string())
                .filter(|group| !group.is_empty());
            account.currency = account
                .currency
                .map(|currency| currency.trim().to_lowercase())
                .filter(|currency| !currency.is_empty());
            Ok(account)
        })
        .collect()
}

fn checked_name(draft: &ContactDraft) -> Result<String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err(Error::InvalidInput("a contact needs a name".into()));
    }
    Ok(name)
}

/// The contact a draft creates.
pub fn plan_create_contact(draft: ContactDraft, created_at: &str) -> Result<Contact> {
    let name = checked_name(&draft)?;
    Ok(Contact {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        kind: draft.kind,
        accounts: clean_accounts(draft.accounts)?,
        note: draft.note.trim().to_string(),
        archived: false,
        created_at: created_at.to_string(),
        group: draft.group.trim().to_string(),
    })
}

/// The new fields of a contact. An account that disappeared from the draft
/// but is used by a transaction is kept, deactivated.
pub fn plan_update_contact(doc: &VaultDoc, id: &str, draft: ContactDraft) -> Result<ContactEdit> {
    let name = checked_name(&draft)?;
    let mut accounts = clean_accounts(draft.accounts)?;
    for old in &require_contact(doc, id)?.accounts {
        if !accounts.iter().any(|account| account.id == old.id)
            && contact_in_use(doc, id, Some(&old.id))
        {
            accounts.push(ContactAccount {
                active: false,
                ..old.clone()
            });
        }
    }
    Ok(ContactEdit {
        name,
        kind: draft.kind,
        note: draft.note.trim().to_string(),
        accounts,
        group: draft.group.trim().to_string(),
    })
}

/// Only a contact nothing refers to can be deleted; otherwise archive it.
pub fn plan_delete_contact(doc: &VaultDoc, id: &str) -> Result<()> {
    if contact_in_use(doc, id, None) {
        return Err(Error::ContactInUse);
    }
    require_contact(doc, id).map(|_| ())
}

/// The ids of the debt terms a contact has: they go with the contact.
pub fn debt_terms_of(doc: &VaultDoc, contact_id: &str) -> Vec<String> {
    doc.debt_terms
        .iter()
        .filter(|terms| terms.contact_id == contact_id)
        .map(|terms| terms.id.clone())
        .collect()
}

/// The ids of the terms of one contact's debt in one asset (the asset in
/// upper case), which a new entry replaces.
pub fn debt_terms_replaced(doc: &VaultDoc, contact_id: &str, asset: &str) -> Vec<String> {
    doc.debt_terms
        .iter()
        .filter(|terms| terms.contact_id == contact_id && terms.asset == asset)
        .map(|terms| terms.id.clone())
        .collect()
}

/// The checked terms a draft sets.
pub fn plan_debt_terms(doc: &VaultDoc, draft: DebtTermsDraft) -> Result<DebtTermsPlan> {
    require_contact(doc, &draft.contact_id)?;
    let asset = draft.asset.trim().to_uppercase();
    if asset.is_empty() {
        return Err(Error::InvalidInput("the debt needs an asset".into()));
    }
    let due_at = draft
        .due_at
        .map(|due| due.trim().to_string())
        .filter(|due| !due.is_empty());
    if let Some(due) = &due_at
        && chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d").is_err()
    {
        return Err(Error::InvalidInput("the due date is YYYY-MM-DD".into()));
    }
    Ok(DebtTermsPlan {
        contact_id: draft.contact_id,
        asset,
        due_at,
        note: draft.note.trim().to_string(),
    })
}

/// The debt terms of a contact (all of them without one).
pub fn debt_terms(doc: &VaultDoc, contact_id: Option<&str>) -> Vec<DebtTerms> {
    doc.debt_terms
        .iter()
        .filter(|terms| contact_id.is_none_or(|id| terms.contact_id == id))
        .cloned()
        .collect()
}

/// Whether the transaction involves the contact (and, if given, that account of it).
pub fn involves(tx: &Transaction, contact_id: &str, account_id: Option<&str>) -> bool {
    let by_account = tx.accounts().into_iter().any(|account| match account {
        AccountRef::Contact {
            contact_id: id,
            account_id: used,
        } => id == contact_id && account_id.is_none_or(|wanted| used.as_deref() == Some(wanted)),
        _ => false,
    });
    by_account || (account_id.is_none() && tx.meta().contact_id.as_deref() == Some(contact_id))
}

/// What was sent to and received from a contact, per asset. Internal
/// moves never count: they did not leave the user's hands.
pub fn turnover(
    books: &Books<'_>,
    contact_id: &str,
    account_id: Option<&str>,
) -> Result<ContactTurnover> {
    let doc = books.doc();
    require_contact(doc, contact_id)?;
    let (base, _) = books.base_and_fx();
    let book = books.rate_book();
    let mut rows: BTreeMap<String, TurnoverRow> = BTreeMap::new();
    let mut out = ContactTurnover {
        contact_id: contact_id.to_string(),
        account_id: account_id.map(str::to_string),
        base_currency: base.to_string(),
        ..ContactTurnover::default()
    };
    for tx in &doc.transactions {
        if !involves(tx, contact_id, account_id) || tx.is_internal(|id| root_of(doc, id)) {
            continue;
        }
        out.tx_count += 1;
        for leg in tx
            .legs()
            .into_iter()
            .filter(|leg| leg.role == LegRole::Principal)
        {
            let row = rows.entry(leg.asset.key()).or_insert_with(|| TurnoverRow {
                asset: leg.asset.clone(),
                ..TurnoverRow::default()
            });
            let value = leg.amount.abs()
                * book.leg_price_in_base(&leg, base, secs_or_now(&tx.meta().timestamp));
            if leg.amount.is_negative() {
                row.sent += leg.amount.abs();
                out.sent_base += value;
            } else {
                row.received += leg.amount;
                out.received_base += value;
            }
        }
    }
    out.rows = rows.into_values().collect();
    Ok(out)
}

/// One summary per contact: operation count, the latest operation and the
/// debts converted to the base currency with the cached rates (no network).
/// `events` are the shared events' part of the debts, as of now.
pub fn summaries(books: &Books<'_>, events: &EventDebts) -> Vec<ContactSummary> {
    let doc = books.doc();
    let (base, _) = books.base_and_fx();
    let book = books.rate_book();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut out: BTreeMap<String, ContactSummary> = doc
        .contacts
        .iter()
        .map(|contact| {
            (
                contact.id.clone(),
                ContactSummary {
                    contact_id: contact.id.clone(),
                    ..ContactSummary::default()
                },
            )
        })
        .collect();
    let mut latest: BTreeMap<String, i64> = BTreeMap::new();
    for tx in &doc.transactions {
        if tx.is_internal(|id| root_of(doc, id)) {
            continue;
        }
        let secs = secs_or_now(&tx.meta().timestamp);
        for (contact_id, summary) in out.iter_mut() {
            if !involves(tx, contact_id, None) {
                continue;
            }
            summary.tx_count += 1;
            if latest.get(contact_id).is_some_and(|seen| *seen > secs) {
                continue;
            }
            latest.insert(contact_id.clone(), secs);
            summary.last_tx_at = Some(tx.meta().timestamp.clone());
            let leg = tx
                .legs()
                .into_iter()
                .find(|leg| leg.role == LegRole::Principal);
            summary.last_amount = leg.as_ref().map(|leg| leg.amount);
            summary.last_asset = leg.map(|leg| leg.asset.symbol);
        }
    }
    for balance in balances_until(doc, i64::MAX, events) {
        let Some(summary) = out.get_mut(&balance.contact_id) else {
            continue;
        };
        let symbol = &balance.asset.symbol;
        for (sum, value) in [
            (&mut summary.receivable_base, balance.receivable),
            (&mut summary.payable_base, balance.payable),
        ] {
            if value.is_zero() {
                continue;
            }
            match book.convert(value, symbol, base, now) {
                Some(converted) => *sum += converted,
                None => summary.missing_rate = true,
            }
        }
    }
    out.into_values().collect()
}

/// Personal debts from debt transfers made up to `secs`, plus what the
/// shared events stood at then (`events`, as of `secs`). The transfers an
/// event made are left out of the first part: the event's balance already
/// counts them.
pub fn balances_until(doc: &VaultDoc, secs: i64, events: &EventDebts) -> Vec<ContactBalance> {
    let mut balances: BTreeMap<(String, String), ContactBalance> = BTreeMap::new();
    for tx in doc
        .transactions
        .iter()
        .filter(|tx| secs_or_now(&tx.meta().timestamp) <= secs)
    {
        let Transaction::Transfer(inner) = tx else {
            continue;
        };
        let Some(effect) = inner.debt else {
            continue;
        };
        if events.tx_ids.contains(&inner.meta.id) {
            continue;
        }
        let Some(contact_id) = inner.from.contact_id().or(inner.to.contact_id()) else {
            continue;
        };
        let entry = balances
            .entry((contact_id.to_string(), inner.asset.key()))
            .or_insert_with(|| ContactBalance {
                contact_id: contact_id.to_string(),
                asset: inner.asset.clone(),
                ..ContactBalance::default()
            });
        match effect {
            DebtEffect::Lend => entry.receivable += inner.amount,
            DebtEffect::Collect => entry.receivable -= inner.amount,
            DebtEffect::Borrow => entry.payable += inner.amount,
            DebtEffect::Repay => entry.payable -= inner.amount,
        }
    }
    for debt in &events.debts {
        let code = debt.currency.to_uppercase();
        let asset = AssetRef::fiat(&code, &code);
        let entry = balances
            .entry((debt.contact_id.clone(), asset.key()))
            .or_insert_with(|| ContactBalance {
                contact_id: debt.contact_id.clone(),
                asset,
                ..ContactBalance::default()
            });
        if debt.amount.is_positive() {
            entry.receivable += debt.amount;
        } else {
            entry.payable += -debt.amount;
        }
    }
    balances
        .into_values()
        .filter(|balance| !balance.receivable.is_zero() || !balance.payable.is_zero())
        .collect()
}
