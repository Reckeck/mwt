//! Shim of the `budget` area: the rules moved to `crate::planning::budget`, the scenarios and the
//! flow queries to `crate::planning::flows` (spec core-strangler, D1).

use crate::Amount;
use crate::model::transaction::Leg;

use super::UnlockedVault;
use crate::money::rates::RateBook;

impl UnlockedVault {
    /// Value of one leg in `currency` at `secs`; `None` without a rate.
    pub(crate) fn leg_value_in(
        book: &RateBook,
        leg: &Leg,
        currency: &str,
        secs: i64,
    ) -> Option<Amount> {
        crate::planning::flows::leg_value_in(book, leg, currency, secs)
    }
}
