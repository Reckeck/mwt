//! Recurring payments: was the same payment made before, and did it get dearer.

use super::UnlockedVault;
use crate::Amount;
use crate::model::transaction::{AccountRef, PaymentTx, Transaction};
use crate::pricing::history::unix_secs;

/// How far back an earlier payment still counts as the same subscription.
const WINDOW_DAYS: i64 = 45;

/// What the history says about one payment.
#[derive(Debug, Clone, PartialEq)]
pub struct RecurringCheck {
    /// No earlier payment to the same payee in the same category.
    pub new: bool,
    /// The amount is above the previous one.
    pub grew: bool,
    pub previous: Option<Amount>,
    /// Growth against the previous amount, percent; `None` without one.
    pub change_pct: Option<Amount>,
}

/// Who a payment went to: the contact, the «whom» account or the comment.
fn payee(payment: &PaymentTx) -> Option<String> {
    if let Some(contact) = &payment.meta.contact_id {
        return Some(format!("contact:{contact}"));
    }
    match &payment.to {
        Some(AccountRef::Store(id)) => return Some(format!("store:{id}")),
        Some(AccountRef::Contact { contact_id, .. }) => {
            return Some(format!("contact:{contact_id}"));
        }
        Some(AccountRef::Raw(text)) if !text.trim().is_empty() => {
            return Some(format!("raw:{}", text.trim().to_lowercase()));
        }
        _ => {}
    }
    let comment = payment.meta.comment.trim().to_lowercase();
    (!comment.is_empty()).then(|| format!("comment:{comment}"))
}

impl UnlockedVault {
    /// Compares a payment with the last earlier one to the same payee, in the
    /// same first category and asset, within 45 days. Anything but a payment
    /// counts as new.
    pub fn recurring_check(&self, tx: &Transaction) -> RecurringCheck {
        let fresh = RecurringCheck {
            new: true,
            grew: false,
            previous: None,
            change_pct: None,
        };
        let Transaction::Payment(payment) = tx else {
            return fresh;
        };
        let (Some(key), Some(at)) = (payee(payment), unix_secs(&payment.meta.timestamp)) else {
            return fresh;
        };
        let category = payment.meta.categories.first();
        let since = at - WINDOW_DAYS * 86_400;
        let previous = self
            .doc
            .transactions
            .iter()
            .filter_map(|other| match other {
                Transaction::Payment(earlier) => Some(earlier),
                _ => None,
            })
            .filter(|earlier| earlier.meta.id != payment.meta.id)
            .filter(|earlier| earlier.asset == payment.asset)
            .filter(|earlier| earlier.meta.categories.first() == category)
            .filter(|earlier| payee(earlier).as_ref() == Some(&key))
            .filter_map(|earlier| {
                let secs = unix_secs(&earlier.meta.timestamp)?;
                (secs < at && secs >= since).then_some((secs, earlier.amount))
            })
            .max_by_key(|(secs, _)| *secs)
            .map(|(_, amount)| amount);
        let Some(previous) = previous else {
            return fresh;
        };
        let change_pct = (!previous.is_zero()).then(|| {
            ((payment.amount - previous) / previous
                * Amount::new(rust_decimal::Decimal::ONE_HUNDRED))
            .round(2)
        });
        RecurringCheck {
            new: false,
            grew: payment.amount > previous,
            previous: Some(previous),
            change_pct,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_support::{amount, at, empty_vault, payment};

    fn subscription(raw: &str, stamp: &str) -> Transaction {
        let mut tx = payment("s1", raw, "Music+");
        tx.meta_mut().categories = vec!["subscriptions".into()];
        at(&mut tx, stamp);
        tx
    }

    #[test]
    fn first_payment_is_new() {
        let (_dir, vault) = empty_vault();
        let check = vault.recurring_check(&subscription("5", "2026-05-10T10:00:00Z"));
        assert!(check.new);
        assert!(!check.grew);
        assert_eq!(check.previous, None);
    }

    #[test]
    fn dearer_payment_is_seen() {
        let (_dir, mut vault) = empty_vault();
        vault
            .doc
            .transactions
            .push(subscription("5", "2026-04-10T10:00:00Z"));
        let check = vault.recurring_check(&subscription("6", "2026-05-10T10:00:00Z"));
        assert!(!check.new);
        assert!(check.grew);
        assert_eq!(check.previous, Some(amount("5")));
        assert_eq!(check.change_pct, Some(amount("20")));
    }

    #[test]
    fn same_amount_did_not_grow() {
        let (_dir, mut vault) = empty_vault();
        vault
            .doc
            .transactions
            .push(subscription("5", "2026-04-10T10:00:00Z"));
        let check = vault.recurring_check(&subscription("5", "2026-05-10T10:00:00Z"));
        assert!(!check.new);
        assert!(!check.grew);
    }

    #[test]
    fn old_or_other_payee_does_not_count() {
        let (_dir, mut vault) = empty_vault();
        vault
            .doc
            .transactions
            .push(subscription("5", "2026-02-01T10:00:00Z"));
        let mut other = subscription("5", "2026-05-01T10:00:00Z");
        other.meta_mut().comment = "Cinema".into();
        vault.doc.transactions.push(other);
        let check = vault.recurring_check(&subscription("6", "2026-05-10T10:00:00Z"));
        assert!(check.new);
    }
}
