//! Contacts: who money moves to and from, their saved accounts, turnover and
//! personal debts (plan §5, §4.8).

use std::collections::BTreeMap;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::contact::{Contact, ContactAccount, ContactKind};
use crate::model::store::AssetRef;
use crate::model::transaction::{AccountRef, DebtEffect, LegRole, Transaction};
use crate::model::vault::now_iso;

use super::UnlockedVault;
use crate::pricing::history::secs_or_now;

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

/// What the UI sends to create or edit a contact. Accounts without an id are new.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ContactDraft {
    pub name: String,
    pub kind: ContactKind,
    pub note: String,
    pub accounts: Vec<ContactAccount>,
    /// Directory group («Семья», «P2P»); empty = grouped by kind.
    #[serde(default)]
    pub group: String,
}

impl UnlockedVault {
    fn find_contact_mut(&mut self, id: &str) -> Result<&mut Contact> {
        self.doc
            .contacts
            .iter_mut()
            .find(|contact| contact.id == id)
            .ok_or_else(|| Error::Missing(format!("contact {id}")))
    }

    /// Whether any transaction or store points at the contact (or one account of it).
    fn contact_in_use(&self, contact_id: &str, account_id: Option<&str>) -> bool {
        let named_by_tx = self.doc.transactions.iter().any(|tx| {
            if account_id.is_none() && tx.meta().contact_id.as_deref() == Some(contact_id) {
                return true;
            }
            tx.accounts().into_iter().any(|account| match account {
                AccountRef::Contact {
                    contact_id: id,
                    account_id: used,
                } => {
                    id == contact_id
                        && account_id.is_none_or(|wanted| used.as_deref() == Some(wanted))
                }
                _ => false,
            })
        });
        let named_by_store = account_id.is_none()
            && self
                .doc
                .stores
                .iter()
                .any(|store| store.contact_id.as_deref() == Some(contact_id));
        named_by_tx || named_by_store
    }

    fn clean_accounts(accounts: Vec<ContactAccount>) -> Result<Vec<ContactAccount>> {
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

    pub fn create_contact(&mut self, draft: ContactDraft) -> Result<Contact> {
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::InvalidInput("a contact needs a name".into()));
        }
        let contact = Contact {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            kind: draft.kind,
            accounts: Self::clean_accounts(draft.accounts)?,
            note: draft.note.trim().to_string(),
            archived: false,
            created_at: now_iso(),
            group: draft.group.trim().to_string(),
        };
        self.doc.contacts.push(contact.clone());
        self.persist()?;
        Ok(contact)
    }

    /// Replace a contact's fields and accounts. An account that disappeared
    /// from the draft but is used by a transaction is kept, deactivated.
    pub fn update_contact(&mut self, id: &str, draft: ContactDraft) -> Result<Contact> {
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::InvalidInput("a contact needs a name".into()));
        }
        let mut accounts = Self::clean_accounts(draft.accounts)?;
        let previous = self
            .doc
            .contacts
            .iter()
            .find(|contact| contact.id == id)
            .ok_or_else(|| Error::Missing(format!("contact {id}")))?
            .accounts
            .clone();
        for old in previous {
            if !accounts.iter().any(|account| account.id == old.id)
                && self.contact_in_use(id, Some(&old.id))
            {
                accounts.push(ContactAccount {
                    active: false,
                    ..old
                });
            }
        }
        let contact = self.find_contact_mut(id)?;
        contact.name = name;
        contact.kind = draft.kind;
        contact.note = draft.note.trim().to_string();
        contact.accounts = accounts;
        contact.group = draft.group.trim().to_string();
        let out = contact.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn archive_contact(&mut self, id: &str, archived: bool) -> Result<()> {
        self.find_contact_mut(id)?.archived = archived;
        self.persist()
    }

    /// Only a contact nothing refers to can be deleted; otherwise archive it.
    pub fn delete_contact(&mut self, id: &str) -> Result<()> {
        if self.contact_in_use(id, None) {
            return Err(Error::InvalidInput(
                "the contact is used by transactions or wallets; archive it instead".into(),
            ));
        }
        let before = self.doc.contacts.len();
        self.doc.contacts.retain(|contact| contact.id != id);
        if before == self.doc.contacts.len() {
            return Err(Error::Missing(format!("contact {id}")));
        }
        self.persist()
    }

    /// Whether the transaction involves the contact (and, if given, that account of it).
    fn involves(tx: &Transaction, contact_id: &str, account_id: Option<&str>) -> bool {
        let by_account = tx.accounts().into_iter().any(|account| match account {
            AccountRef::Contact {
                contact_id: id,
                account_id: used,
            } => {
                id == contact_id && account_id.is_none_or(|wanted| used.as_deref() == Some(wanted))
            }
            _ => false,
        });
        by_account || (account_id.is_none() && tx.meta().contact_id.as_deref() == Some(contact_id))
    }

    /// What was sent to and received from a contact, per asset. Internal
    /// moves never count: they did not leave the user's hands.
    pub fn contact_turnover(
        &self,
        contact_id: &str,
        account_id: Option<&str>,
    ) -> Result<ContactTurnover> {
        if !self
            .doc
            .contacts
            .iter()
            .any(|contact| contact.id == contact_id)
        {
            return Err(Error::Missing(format!("contact {contact_id}")));
        }
        let (base, _) = self.base_and_fx();
        let book = self.rate_book();
        let mut rows: BTreeMap<String, TurnoverRow> = BTreeMap::new();
        let mut out = ContactTurnover {
            contact_id: contact_id.to_string(),
            account_id: account_id.map(str::to_string),
            base_currency: base.clone(),
            ..ContactTurnover::default()
        };
        for tx in &self.doc.transactions {
            if !Self::involves(tx, contact_id, account_id) || tx.is_internal(|id| self.root_of(id))
            {
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
                    * book.leg_price_in_base(&leg, &base, secs_or_now(&tx.meta().timestamp));
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

    /// Personal debts per contact and asset, from transfers marked as debts only.
    pub fn contact_balances(&self) -> Vec<ContactBalance> {
        self.contact_balances_until(i64::MAX)
    }

    /// Personal debts from debt transfers made up to `secs`.
    pub(crate) fn contact_balances_until(&self, secs: i64) -> Vec<ContactBalance> {
        let mut balances: BTreeMap<(String, String), ContactBalance> = BTreeMap::new();
        for tx in self
            .doc
            .transactions
            .iter()
            .filter(|tx| crate::pricing::history::secs_or_now(&tx.meta().timestamp) <= secs)
        {
            let Transaction::Transfer(inner) = tx else {
                continue;
            };
            let Some(effect) = inner.debt else {
                continue;
            };
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
        balances
            .into_values()
            .filter(|balance| !balance.receivable.is_zero() || !balance.payable.is_zero())
            .collect()
    }
}

/// Due date and note for a debt with a contact in one asset.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct DebtTermsDraft {
    pub contact_id: String,
    pub asset: String,
    pub due_at: Option<String>,
    pub note: String,
}

impl UnlockedVault {
    pub fn debt_terms(&self, contact_id: Option<&str>) -> Vec<crate::model::contact::DebtTerms> {
        self.doc
            .debt_terms
            .iter()
            .filter(|terms| contact_id.is_none_or(|id| terms.contact_id == id))
            .cloned()
            .collect()
    }

    /// Set (or replace) the terms of one contact's debt in one asset; an
    /// empty date and note removes them.
    pub fn set_debt_terms(
        &mut self,
        draft: DebtTermsDraft,
    ) -> Result<Option<crate::model::contact::DebtTerms>> {
        if !self
            .doc
            .contacts
            .iter()
            .any(|contact| contact.id == draft.contact_id)
        {
            return Err(Error::Missing(format!("contact {}", draft.contact_id)));
        }
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
        let note = draft.note.trim().to_string();
        self.doc
            .debt_terms
            .retain(|terms| !(terms.contact_id == draft.contact_id && terms.asset == asset));
        let out = if due_at.is_none() && note.is_empty() {
            None
        } else {
            let terms = crate::model::contact::DebtTerms {
                id: uuid::Uuid::new_v4().to_string(),
                contact_id: draft.contact_id,
                asset,
                due_at,
                note,
                created_at: crate::model::vault::now_iso(),
            };
            self.doc.debt_terms.push(terms.clone());
            Some(terms)
        };
        self.persist()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::contact::DetailKind;
    use crate::model::store::StoreKind;

    fn ann(vault: &mut UnlockedVault) -> Contact {
        vault
            .create_contact(ContactDraft {
                group: String::new(),
                name: "Ann".into(),
                kind: ContactKind::Person,
                note: String::new(),
                accounts: vec![ContactAccount {
                    value: "DE00 1234".into(),
                    kind: DetailKind::Iban,
                    ..ContactAccount::default()
                }],
            })
            .unwrap()
    }

    fn with_debt(tx: Transaction, effect: DebtEffect) -> Transaction {
        match tx {
            Transaction::Transfer(mut inner) => {
                inner.debt = Some(effect);
                Transaction::Transfer(inner)
            }
            other => other,
        }
    }

    #[test]
    fn debts_add_up_per_contact_and_count_in_net_worth() {
        let (_dir, mut vault) = setup();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let store = vault.doc.stores[1].id.clone();
        let contact = ann(&mut vault);
        let mine = || AccountRef::Store(store.clone());
        let theirs = || AccountRef::Contact {
            contact_id: contact.id.clone(),
            account_id: None,
        };
        vault
            .add_transaction(income(&store, "10000", "salary"))
            .unwrap();
        for (from, to, raw, effect) in [
            (mine(), theirs(), "5000", DebtEffect::Lend),
            (theirs(), mine(), "2000", DebtEffect::Collect),
            (theirs(), mine(), "700", DebtEffect::Borrow),
            (mine(), theirs(), "200", DebtEffect::Repay),
        ] {
            vault
                .add_transaction(with_debt(transfer_between(from, to, usd(), raw), effect))
                .unwrap();
        }
        let balances = vault.contact_balances();
        assert_eq!(balances.len(), 1);
        assert_eq!(balances[0].receivable, amount("3000"));
        assert_eq!(balances[0].payable, amount("500"));

        let worth = vault.net_worth();
        // 10000 − 5000 + 2000 + 700 − 200 on the exchange
        assert_eq!(worth.financial, amount("7500"));
        assert_eq!(worth.receivables, amount("3000"));
        assert_eq!(worth.payables, amount("500"));
        assert_eq!(worth.net, amount("10000"));
        // a debt is neither income nor spending
        for tx in &vault.doc.transactions[1..] {
            assert_eq!(vault.classify_flow(tx), super::super::FlowClass::Ignored);
        }
    }

    #[test]
    fn turnover_counts_sent_and_received_but_not_internal_moves() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let sub = vault
            .create_store("Sub", StoreKind::Exchange, "", "", Some(store.clone()))
            .unwrap()
            .id;
        let contact = ann(&mut vault);
        let account = contact.accounts[0].id.clone();
        let theirs = |account_id: Option<String>| AccountRef::Contact {
            contact_id: contact.id.clone(),
            account_id,
        };
        vault.add_transaction(income(&store, "1000", "in")).unwrap();
        vault
            .add_transaction(transfer_between(
                AccountRef::Store(store.clone()),
                theirs(Some(account.clone())),
                usd(),
                "300",
            ))
            .unwrap();
        vault
            .add_transaction(transfer_between(
                theirs(None),
                AccountRef::Store(store.clone()),
                usd(),
                "50",
            ))
            .unwrap();
        // internal, but tagged with the contact: still not turnover
        let mut internal = transfer_between(
            AccountRef::Store(store.clone()),
            AccountRef::Store(sub),
            usd(),
            "10",
        );
        internal.meta_mut().contact_id = Some(contact.id.clone());
        vault.add_transaction(internal).unwrap();

        let all = vault.contact_turnover(&contact.id, None).unwrap();
        assert_eq!(all.tx_count, 2);
        assert_eq!(all.rows[0].sent, amount("300"));
        assert_eq!(all.rows[0].received, amount("50"));
        let one = vault.contact_turnover(&contact.id, Some(&account)).unwrap();
        assert_eq!(one.tx_count, 1);
    }

    #[test]
    fn used_contacts_are_archived_and_used_accounts_deactivated() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let contact = ann(&mut vault);
        let account = contact.accounts[0].clone();
        vault
            .add_transaction(transfer_between(
                AccountRef::Store(store),
                AccountRef::Contact {
                    contact_id: contact.id.clone(),
                    account_id: Some(account.id.clone()),
                },
                usd(),
                "1",
            ))
            .unwrap();
        assert!(vault.delete_contact(&contact.id).is_err());
        let updated = vault
            .update_contact(
                &contact.id,
                ContactDraft {
                    name: "Ann B.".into(),
                    ..ContactDraft::default()
                },
            )
            .unwrap();
        assert_eq!(updated.accounts.len(), 1);
        assert!(!updated.accounts[0].active);
        vault.archive_contact(&contact.id, true).unwrap();

        let unused = vault
            .create_contact(ContactDraft {
                name: "Bob".into(),
                ..ContactDraft::default()
            })
            .unwrap();
        vault.delete_contact(&unused.id).unwrap();
    }

    #[test]
    fn debt_terms_are_set_replaced_and_cleared() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let friend = vault
            .create_contact(ContactDraft {
                name: "Friend".into(),
                ..ContactDraft::default()
            })
            .unwrap();
        let draft = |due: Option<&str>, note: &str| DebtTermsDraft {
            contact_id: friend.id.clone(),
            asset: "usd".into(),
            due_at: due.map(str::to_string),
            note: note.into(),
        };
        assert!(vault.set_debt_terms(draft(Some("31.12.2026"), "")).is_err());
        vault
            .set_debt_terms(draft(Some("2026-12-31"), "for the car"))
            .unwrap();
        vault.set_debt_terms(draft(Some("2027-01-15"), "")).unwrap();
        let terms = vault.debt_terms(Some(&friend.id));
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].asset, "USD");
        assert_eq!(terms[0].due_at.as_deref(), Some("2027-01-15"));
        assert!(vault.set_debt_terms(draft(None, "")).unwrap().is_none());
        assert!(vault.debt_terms(None).is_empty());
    }
}
