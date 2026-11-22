//! Today's net worth assembled from the holdings, the baskets and the contact
//! debts: the one source of the current capital, read by the capital
//! scenario and by the networth report's current point.

use std::collections::HashMap;

use crate::Amount;
use crate::household::contacts::contact_balances_until;
use crate::model::store::{LiabilityKind, StoreKind};
use crate::money::books::Books;
use crate::money::worth::{ClassShare, LiabilityShare, NetWorth};
use crate::planning::allocation::{Slot, slot_resolver};

/// The bound that takes every contact balance, however late.
const ALL_BALANCES: i64 = i64::MAX;

/// Today's valuation plus the part of `financial` held in crypto wallets
/// and exchanges. The one source of the current capital: the networth
/// report's current point is taken from here too.
pub(crate) fn net_worth_with_crypto(books: &Books<'_>) -> (NetWorth, Amount) {
    let doc = books.doc();
    let (base, _) = books.base_and_fx();
    let kinds: HashMap<&str, StoreKind> = doc
        .stores
        .iter()
        .map(|store| (store.id.as_str(), store.kind))
        .collect();
    let loan_kinds: HashMap<&str, LiabilityKind> = doc
        .stores
        .iter()
        .map(|store| {
            let kind = store
                .liability
                .as_ref()
                .map_or(LiabilityKind::Other, |terms| terms.kind);
            (store.id.as_str(), kind)
        })
        .collect();
    let mut out = NetWorth {
        base_currency: base.to_string(),
        ..NetWorth::default()
    };
    let mut crypto = Amount::ZERO;
    let mut loans: HashMap<&str, Amount> = HashMap::new();
    let slot_of = slot_resolver(books.doc());
    let mut by_class: HashMap<Option<String>, Amount> = HashMap::new();
    for holding in doc
        .holdings
        .iter()
        .filter(|holding| !holding.asset.is_derivative())
    {
        let value = holding.quantity * books.price_base(&holding.asset.key());
        match kinds.get(holding.store_id.as_str()) {
            Some(StoreKind::Property | StoreKind::Loyalty) => out.non_financial += value,
            Some(StoreKind::Liability) => {
                *loans.entry(holding.store_id.as_str()).or_default() += value
            }
            other => {
                out.financial += value;
                if matches!(other, Some(StoreKind::Crypto | StoreKind::Exchange)) {
                    crypto += value;
                }
                match slot_of(holding) {
                    Slot::Fund => out.financial_fund += value,
                    Slot::Class(class) => *by_class.entry(class).or_default() += value,
                }
            }
        }
    }
    let mut by_kind: Vec<LiabilityShare> = Vec::new();
    for (store_id, value) in loans {
        if value.is_negative() {
            out.liabilities += value.abs();
            let kind = loan_kinds
                .get(store_id)
                .copied()
                .unwrap_or(LiabilityKind::Other);
            match by_kind.iter_mut().find(|share| share.kind == kind) {
                Some(share) => share.amount += value.abs(),
                None => by_kind.push(LiabilityShare {
                    kind,
                    amount: value.abs(),
                }),
            }
        } else {
            out.financial += value;
            *by_class.entry(None).or_default() += value;
        }
    }
    for balance in contact_balances_until(books, ALL_BALANCES) {
        let price = books.price_base(&balance.asset.key());
        out.receivables += balance.receivable.max(Amount::ZERO) * price;
        out.payables += balance.payable.max(Amount::ZERO) * price;
    }
    by_kind.sort_by(|left, right| {
        (right.amount.cmp(&left.amount)).then((left.kind as u8).cmp(&(right.kind as u8)))
    });
    out.liabilities_by_kind = by_kind;
    let mut classes: Vec<ClassShare> = by_class
        .into_iter()
        .filter(|(_, amount)| !amount.is_zero())
        .map(|(class_id, amount)| ClassShare { class_id, amount })
        .collect();
    classes.sort_by(|left, right| {
        (right.amount.cmp(&left.amount)).then(left.class_id.cmp(&right.class_id))
    });
    out.financial_by_class = classes;
    out.net = out.financial + out.non_financial + out.receivables - out.liabilities - out.payables;
    (out, crypto)
}
