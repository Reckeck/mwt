//! Shim of the spending figures of shared events: moved to `crate::money::shared_ratios`,
//! `crate::household::shared_events::analytics` and `crate::usecases::household::shared_events`
//! (spec core-strangler, D1).

pub(crate) use crate::money::shared_ratios::SharedExtra;

use crate::calendar::Calendar;
use crate::money::rates::RateBook;

use super::super::UnlockedVault;
use super::super::budget::Flow;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Spending that has no payment of mine behind it: my part of other
        /// people's purchases and what I wrote off, over entries the caller already
        /// collected (`Books` keeps them, so a request collects them once).
        pub(crate) fn extra_flows_from(&self, extras: &[SharedExtra], book: &RateBook, cal: &Calendar, currency: &str, from: i64, to: i64) -> Vec<Flow>
            => crate::usecases::household::shared_events::extra_flows_from @doc;
    }
    // forward:end
}
