//! Recurring payments: was the same payment made before, and did it get dearer.

use mwt_base::Amount;
use mwt_base::time::unix_secs;
use mwt_model::VaultDoc;
use mwt_model::{AccountRef, PaymentTx, Transaction};

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

/// Compares a payment with the last earlier one to the same payee, in the
/// same first category and asset, within 45 days. Anything but a payment
/// counts as new.
pub fn recurring_check(doc: &VaultDoc, tx: &Transaction) -> RecurringCheck {
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
    let previous = doc
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
        ((payment.amount - previous) / previous * Amount::new(rust_decimal::Decimal::ONE_HUNDRED))
            .round(2)
    });
    RecurringCheck {
        new: false,
        grew: payment.amount > previous,
        previous: Some(previous),
        change_pct,
    }
}
