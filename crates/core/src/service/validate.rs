//! Rules every transaction must satisfy before it is stored (plan §4.5,
//! V1–V14), and the automatic categorization that runs right before them.

use std::collections::HashSet;

use crate::Amount;
use crate::error::{Error, Result, WalletRule};
use crate::model::category::CategoryGroup;
use crate::model::store::{Store, StoreKind};
use crate::model::transaction::{
    AccountRef, DebtEffect, FuturesAction, KindDetails, LegRole, Transaction, TxKind, allowed_kinds,
};
use crate::model::vault::iso_to_millis;
use crate::txinfo::{HashShape, hash_shape};

use super::UnlockedVault;

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

impl UnlockedVault {
    /// Trim, lowercase and deduplicate category keys, keeping their order.
    pub(crate) fn normalize_categories(tx: &mut Transaction) {
        let mut seen = HashSet::new();
        let categories = &mut tx.meta_mut().categories;
        for raw in categories.iter_mut() {
            *raw = raw.trim().to_lowercase();
        }
        categories.retain(|key| !key.is_empty() && seen.insert(key.clone()));
    }

    /// Fill empty categories from the first matching active rule (plan §8.1):
    /// a case-insensitive substring of the comment or of a contact's name.
    pub(crate) fn apply_categorization_rules(&self, tx: &mut Transaction) {
        if !tx.meta().categories.is_empty() {
            return;
        }
        let mut haystacks = vec![tx.meta().comment.to_lowercase()];
        for contact_id in tx.contact_ids() {
            if let Some(contact) = self
                .doc
                .contacts
                .iter()
                .find(|contact| contact.id == contact_id)
            {
                haystacks.push(contact.name.to_lowercase());
            }
        }
        let mut rules: Vec<_> = self
            .doc
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
            && self
                .doc
                .categories
                .iter()
                .any(|category| category.key == rule.category && !category.archived)
        {
            tx.meta_mut().categories = vec![rule.category.clone()];
        }
    }

    pub(crate) fn live_store(&self, id: &str) -> Result<&Store> {
        match self.doc.stores.iter().find(|store| store.id == id) {
            Some(store) if store.archived => Err(Error::Archived(id.into())),
            Some(store) => Ok(store),
            None => Err(Error::Missing(format!("store {id}"))),
        }
    }

    /// V7: a contact side names an existing contact, and an active account of it.
    fn check_account(&self, account: &AccountRef) -> Result<()> {
        match account {
            AccountRef::Store(id) => self.live_store(id).map(|_| ()),
            AccountRef::Contact {
                contact_id,
                account_id,
            } => {
                let contact = self
                    .doc
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
    fn check_details(store: &Store, details: &KindDetails, amount: Amount) -> Result<()> {
        if let Some(notes) = &details.cash_breakdown {
            if store.kind != StoreKind::Cash {
                return Err(invalid("a banknote count is only for cash"));
            }
            let total = notes.iter().fold(Amount::ZERO, |acc, note| {
                acc + note.denomination * Amount::new(rust_decimal::Decimal::from(note.count))
            });
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

    /// Every store a transaction is booked on, with the kind it must accept.
    fn store_sides(tx: &Transaction) -> Vec<&str> {
        match tx {
            Transaction::Topup(inner) => vec![&inner.store_id],
            Transaction::Payment(inner) => vec![&inner.store_id],
            Transaction::Transfer(inner) => [inner.from.store_id(), inner.to.store_id()]
                .into_iter()
                .flatten()
                .collect(),
            Transaction::Convert(inner) => {
                [inner.from.account.store_id(), inner.to.account.store_id()]
                    .into_iter()
                    .flatten()
                    .collect()
            }
            Transaction::TradeSpot(inner) => vec![&inner.store_id],
            Transaction::TradeFutures(inner) => vec![&inner.store_id],
            Transaction::Defi(inner) => vec![&inner.store_id],
            Transaction::AssetSell(inner) => vec![&inner.store_id],
        }
    }

    /// Plan §4.5, V1–V14 (V13 — negative balances — is a warning, not an error).
    pub(crate) fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        let kind = tx.kind();

        // V1, V2
        for id in Self::store_sides(tx) {
            let store = self.live_store(id)?;
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
            self.live_store(target)?;
        }

        // V3: what each kind of store may hold.
        for leg in tx
            .legs()
            .into_iter()
            .filter(|leg| leg.role == LegRole::Principal)
        {
            let store = self.live_store(&leg.store_id)?;
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
                    self.check_account(from)?;
                }
                let store = self.live_store(&inner.store_id)?;
                Self::check_details(store, &inner.details, inner.amount)?;
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
                    self.check_account(to)?;
                }
                let store = self.live_store(&inner.store_id)?;
                Self::check_details(store, &inner.details, inner.amount)?;
            }
            Transaction::Transfer(inner) => {
                positive(inner.amount, "the amount")?;
                if inner.from.store_id().is_none() && inner.to.store_id().is_none() {
                    return Err(invalid("one side of a transfer must be your wallet"));
                }
                if inner.from == inner.to {
                    return Err(invalid("a transfer needs two different sides"));
                }
                self.check_account(&inner.from)?;
                self.check_account(&inner.to)?;
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
                if inner.from.account.store_id().is_none() && inner.to.account.store_id().is_none()
                {
                    return Err(invalid("one side of a conversion must be your wallet"));
                }
                if inner.from.asset.key() == inner.to.asset.key() {
                    return Err(invalid("a conversion needs two different assets"));
                }
                if let Some(rate) = inner.rate_override {
                    positive(rate, "the rate")?;
                }
                self.check_account(&inner.from.account)?;
                self.check_account(&inner.to.account)?;
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

        self.check_meta(tx)?;
        self.check_on_chain_refs(tx)
    }

    /// V5, V10, V11, V12 — the shared fields.
    fn check_meta(&self, tx: &Transaction) -> Result<()> {
        let meta = tx.meta();
        let kind = tx.kind();

        // V11
        let millis = iso_to_millis(&meta.timestamp);
        if millis == 0 && !meta.timestamp.starts_with("1970-01-01T00:00:00") {
            return Err(invalid("the date is not a valid RFC3339 timestamp"));
        }
        let now = iso_to_millis(&crate::model::vault::now_iso());
        if millis > now + MAX_FUTURE_MS {
            return Err(invalid("the date is too far in the future"));
        }

        // V5
        if matches!(kind, TxKind::Topup | TxKind::Payment) && meta.categories.is_empty() {
            return Err(invalid("choose a category"));
        }

        // V10, V12
        for key in &meta.categories {
            let category = self
                .doc
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
            && !self
                .doc
                .contacts
                .iter()
                .any(|contact| &contact.id == contact_id)
        {
            return Err(Error::Missing(format!("contact {contact_id}")));
        }
        Ok(())
    }

    /// Shape-check every on-chain hash a transaction claims. A hash whose
    /// network differs from the assets it moved would open the explorer on
    /// somebody else's transaction, so both are refused.
    fn check_on_chain_refs(&self, tx: &Transaction) -> Result<()> {
        let chains = tx.asset_chains();
        for (network, hash) in tx.on_chain_refs() {
            let Some(hash) = hash.filter(|raw| !raw.is_empty()) else {
                continue;
            };
            if hash_shape(hash) == HashShape::Malformed {
                return Err(Error::TxHashMalformed(hash.to_string()));
            }
            if let Some(network) = network
                && chains.iter().flatten().any(|chain| chain != &network)
            {
                return Err(Error::TxHashNetworkMismatch(format!(
                    "network {network:?} does not match the assets moved"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::model::chain::Chain;
    use crate::model::contact::{ContactAccount, ContactKind, DetailKind};
    use crate::model::store::{AssetRef, StoreKind};
    use crate::model::transaction::*;
    use crate::service::ContactDraft;

    fn rejected(result: crate::Result<Transaction>) -> String {
        match result {
            Err(err) => err.to_string(),
            Ok(tx) => panic!("expected a rejection, got {}", tx.tag()),
        }
    }

    /// V2: a kind the store does not accept.
    #[test]
    fn a_kind_must_fit_the_store() {
        let (_dir, mut vault) = setup();
        let cold = vault.doc.stores[2].id.clone();
        assert!(
            rejected(vault.add_transaction(trade(&cold, "1", "1", "x"))).contains("not possible")
        );
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        assert!(rejected(vault.add_transaction(payment(&flat, "1", "x"))).contains("not possible"));
    }

    /// V3: cash and bank accounts hold fiat only.
    #[test]
    fn a_bank_holds_only_fiat() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        assert!(
            rejected(vault.add_transaction(income_of(&bank, btc(), "1", "1", "x")))
                .contains("only fiat")
        );
        vault.add_transaction(income(&bank, "1", "x")).unwrap();
    }

    /// V5, V12: topups and payments need a category of a matching group.
    #[test]
    fn categories_must_fit_the_verb() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut bare = payment(&store, "1", "x");
        bare.meta_mut().categories.clear();
        assert!(rejected(vault.add_transaction(bare)).contains("choose a category"));

        let mut wrong = income(&store, "1", "x");
        wrong.meta_mut().categories = vec!["groceries".into()];
        assert!(rejected(vault.add_transaction(wrong)).contains("does not fit"));

        vault.archive_category("groceries", true).unwrap();
        assert!(rejected(vault.add_transaction(payment(&store, "1", "x"))).contains("archived"));
    }

    /// V6: a transfer needs one of the user's stores and two different sides.
    #[test]
    fn transfers_need_a_store_and_two_sides() {
        let (_dir, mut vault) = setup();
        let raw = |text: &str| AccountRef::Raw(text.into());
        assert!(
            rejected(vault.add_transaction(transfer_between(raw("a"), raw("b"), usd(), "1")))
                .contains("must be your wallet")
        );
        let store = vault.doc.stores[1].id.clone();
        let mut convert = Transaction::Convert(ConvertTx {
            meta: meta("fx", &[]),
            from: ConvertSide {
                account: AccountRef::Store(store.clone()),
                asset: usd(),
                amount: amount("1"),
                price: PriceStamp::default(),
            },
            to: ConvertSide {
                account: raw("exchanger"),
                asset: usd(),
                amount: amount("1"),
                price: PriceStamp::default(),
            },
            fee: None,
            rate_override: None,
        });
        assert!(rejected(vault.add_transaction(convert.clone())).contains("two different assets"));
        if let Transaction::Convert(inner) = &mut convert {
            inner.to.asset = AssetRef::fiat("EUR", "Euro");
        }
        vault.add_transaction(convert).unwrap();
    }

    /// V7: a contact account must belong to the contact and be active.
    #[test]
    fn contact_accounts_are_checked() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let contact = vault
            .create_contact(ContactDraft {
                name: "Mom".into(),
                kind: ContactKind::Person,
                note: String::new(),
                accounts: vec![ContactAccount {
                    value: "4276 0000".into(),
                    kind: DetailKind::Card,
                    ..ContactAccount::default()
                }],
            })
            .unwrap();
        let account_id = contact.accounts[0].id.clone();
        let to = |account: &str| AccountRef::Contact {
            contact_id: contact.id.clone(),
            account_id: Some(account.into()),
        };
        let send =
            |to: AccountRef| transfer_between(AccountRef::Store(store.clone()), to, usd(), "5");
        assert!(rejected(vault.add_transaction(send(to("other")))).contains("does not belong"));
        vault.add_transaction(send(to(&account_id))).unwrap();
    }

    /// V8: details only where they make sense, banknotes must add up.
    #[test]
    fn kind_details_are_checked() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
        let Transaction::Topup(mut topup) = income(&cash, "300", "found") else {
            unreachable!()
        };
        topup.details.cash_breakdown = Some(vec![CashNote {
            denomination: amount("100"),
            count: 2,
        }]);
        assert!(
            rejected(vault.add_transaction(Transaction::Topup(topup.clone()))).contains("add up")
        );
        topup.details.cash_breakdown = Some(vec![CashNote {
            denomination: amount("100"),
            count: 3,
        }]);
        vault
            .add_transaction(Transaction::Topup(topup.clone()))
            .unwrap();
        topup.details.network = Some(Chain::Bitcoin);
        assert!(
            rejected(vault.add_transaction(Transaction::Topup(topup))).contains("crypto wallets")
        );
    }

    /// V9: realized PnL only on a close; leverage positive.
    #[test]
    fn futures_fields_are_checked() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let futures = |action: FuturesAction, pnl: Option<&str>| {
            Transaction::TradeFutures(TradeFuturesTx {
                meta: meta("perp", &[]),
                store_id: store.clone(),
                contract: AssetRef::perp("bybit", "BTCUSDT"),
                settle: AssetRef::fiat("USDT", "Tether"),
                direction: FuturesDirection::Long,
                action,
                quantity: amount("1"),
                fill_price: amount("100"),
                leverage: Some(amount("10")),
                fee: None,
                closed_pnl: pnl.map(amount),
                order_id: None,
                category: FuturesCategory::Linear,
                price: PriceStamp::default(),
            })
        };
        assert!(
            rejected(vault.add_transaction(futures(FuturesAction::Open, Some("5"))))
                .contains("closing")
        );
        vault
            .add_transaction(futures(FuturesAction::Open, None))
            .unwrap();
        vault
            .add_transaction(futures(FuturesAction::Close, Some("5")))
            .unwrap();
    }

    /// V11: a date too far ahead, or not a date at all.
    #[test]
    fn dates_are_checked() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut later = income(&store, "1", "x");
        at(&mut later, "2999-01-01T00:00:00Z");
        assert!(rejected(vault.add_transaction(later)).contains("future"));
        let mut garbage = income(&store, "1", "x");
        at(&mut garbage, "yesterday");
        assert!(rejected(vault.add_transaction(garbage)).contains("RFC3339"));
    }

    /// V14: the debt direction must match the sides.
    #[test]
    fn debt_direction_must_match_the_sides() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let contact = vault
            .create_contact(ContactDraft {
                name: "Ann".into(),
                ..ContactDraft::default()
            })
            .unwrap();
        let ann = AccountRef::Contact {
            contact_id: contact.id,
            account_id: None,
        };
        let mut lend = transfer_between(ann.clone(), AccountRef::Store(store.clone()), usd(), "10");
        if let Transaction::Transfer(inner) = &mut lend {
            inner.debt = Some(DebtEffect::Lend);
        }
        assert!(rejected(vault.add_transaction(lend)).contains("lending"));
        let mut borrow = transfer_between(ann, AccountRef::Store(store), usd(), "10");
        if let Transaction::Transfer(inner) = &mut borrow {
            inner.debt = Some(DebtEffect::Borrow);
        }
        vault.add_transaction(borrow).unwrap();
    }

    /// §8.1: the first matching active rule fills empty categories only.
    #[test]
    fn rules_fill_empty_categories_in_order() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let late = vault
            .create_categorization_rule("такси", "transport")
            .unwrap();
        vault.create_categorization_rule("ТАКСИ", "taxi").unwrap();
        // the second rule should run first
        let ids: Vec<String> = vault
            .categorization_rules()
            .into_iter()
            .rev()
            .map(|rule| rule.id)
            .collect();
        vault.reorder_categorization_rules(&ids).unwrap();

        let mut ride = payment(&store, "300", "Яндекс Такси до дома");
        ride.meta_mut().categories.clear();
        let stored = vault.add_transaction(ride).unwrap();
        assert_eq!(stored.meta().categories, vec!["taxi"]);

        // a category the user chose is never replaced
        let chosen = vault
            .add_transaction(payment(&store, "300", "такси"))
            .unwrap();
        assert_eq!(chosen.meta().categories, vec!["groceries"]);

        // an inactive rule does nothing
        vault
            .update_categorization_rule(&late.id, "такси", "transport", false)
            .unwrap();
        assert_eq!(vault.categorization_rules().len(), 2);
    }
}
