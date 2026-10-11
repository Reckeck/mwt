//! Automations (plan §7.2): transactions that repeat on a schedule.
//!
//! A planned date fires at 09:00 local time. Each processed date leaves an
//! [`AutomationRun`] — created, rejected for lack of funds (D26), or skipped —
//! and is never retried by itself: the user retries or skips a rejected run.

use std::collections::HashSet;

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, AUTOMATION_HOUR, Calendar, clamp_day, next_month};
use crate::error::{Error, Result};
use crate::model::planning::{
    Automation, AutomationOutcome, AutomationPreset, AutomationRun, MAX_AUTOMATION_RUNS,
    RejectReason, Schedule, TransactionDraft,
};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::{PriceStamp, Transaction, TxOrigin};
use crate::model::vault::now_iso;
use crate::pricing::history::{secs_or_now, unix_secs};

use super::UnlockedVault;

/// Most planned dates one pass creates; older missed dates are recorded as
/// skipped, so a long time away does not flood the history (plan §7.2).
pub const MAX_CATCH_UP: usize = 24;

/// What an automation is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct AutomationDraft {
    pub name: String,
    pub active: bool,
    pub preset: Option<AutomationPreset>,
    pub schedule: Schedule,
    pub starts_at: String,
    pub template: TransactionDraft,
}

/// The outcome of one scheduler pass.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AutomationPass {
    /// Ids of the transactions created.
    pub created: Vec<String>,
    /// Runs rejected in this pass, for the notification.
    pub rejected: Vec<AutomationRun>,
    /// Missed dates beyond the catch-up limit, recorded as skipped.
    pub skipped: usize,
}

/// One planned date that is due.
#[derive(Debug, Clone, PartialEq)]
struct DueRun {
    automation_id: String,
    secs: i64,
}

/// Every planned date of a schedule after `after` (exclusive) up to `until`
/// (inclusive), starting no earlier than `start`.
pub fn planned_dates(
    schedule: &Schedule,
    start: NaiveDate,
    after: Option<NaiveDate>,
    until: NaiveDate,
) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let keep = |date: NaiveDate| date >= start && after.is_none_or(|last| date > last);
    match *schedule {
        Schedule::Monthly { day } => {
            let mut month = calendar::first_of_month(start);
            while month <= until {
                let date = clamp_day(month, day);
                if date <= until && keep(date) {
                    out.push(date);
                }
                month = next_month(month);
            }
        }
        Schedule::Weekly { weekday } => {
            let offset = (i64::from(weekday) - i64::from(start.weekday().num_days_from_monday()))
                .rem_euclid(7);
            let mut date = start + Duration::days(offset);
            while date <= until {
                if keep(date) {
                    out.push(date);
                }
                date += Duration::days(7);
            }
        }
        Schedule::EveryNDays { days } => {
            let step = Duration::days(i64::from(days.max(1)));
            let mut date = start;
            while date <= until {
                if keep(date) {
                    out.push(date);
                }
                date += step;
            }
        }
    }
    out
}

/// A transaction from a template, dated `ts`; prices are left to the stamp.
pub(crate) fn draft_to_transaction(
    draft: &TransactionDraft,
    automation_id: &str,
    planned_at: &str,
    ts: &str,
) -> Transaction {
    let mut tx = match draft.clone() {
        TransactionDraft::Topup(inner) => Transaction::Topup(inner),
        TransactionDraft::Payment(inner) => Transaction::Payment(inner),
        TransactionDraft::Transfer(inner) => Transaction::Transfer(inner),
        TransactionDraft::Convert(inner) => Transaction::Convert(inner),
    };
    let meta = tx.meta_mut();
    meta.id = uuid::Uuid::new_v4().to_string();
    meta.timestamp = ts.to_string();
    meta.origin = TxOrigin::Automation {
        automation_id: automation_id.to_string(),
        planned_at: planned_at.to_string(),
    };
    for (_, stamp) in tx.stamps_mut() {
        *stamp = PriceStamp::default();
    }
    tx
}

/// What a transaction takes from a store of the user's.
pub(super) struct Outflow {
    pub store_id: String,
    pub asset: AssetRef,
    pub amount: Amount,
    pub fee: Option<(AssetRef, Amount)>,
}

/// The store and asset a transaction draws from, the principal it takes and
/// its fee: `None` when nothing leaves a store of the user's.
pub(super) fn outflow(tx: &Transaction) -> Option<Outflow> {
    let fee = tx.fee().map(|fee| (fee.asset.clone(), fee.amount.abs()));
    let (store_id, asset, amount) = match tx {
        Transaction::Payment(inner) => (inner.store_id.clone(), &inner.asset, inner.amount),
        Transaction::Transfer(inner) => (
            inner.from.store_id()?.to_string(),
            &inner.asset,
            inner.amount,
        ),
        Transaction::Convert(inner) => (
            inner.from.account.store_id()?.to_string(),
            &inner.from.asset,
            inner.from.amount,
        ),
        _ => return None,
    };
    Some(Outflow {
        store_id,
        asset: asset.clone(),
        amount: amount.abs(),
        fee,
    })
}

/// Human text of a rejection, for errors and logs; the UI words its own.
pub fn describe_reject(reason: &RejectReason) -> String {
    match reason {
        RejectReason::InsufficientFunds {
            store_id,
            asset,
            required,
            available,
        } => format!(
            "store {store_id}: {available} {sym} of {required} {sym}",
            sym = asset.symbol
        ),
        RejectReason::CreditLimitExceeded {
            store_id,
            limit,
            required,
        } => format!("store {store_id}: credit limit {limit}, needed {required}"),
        RejectReason::Invalid { message } => message.clone(),
    }
}

impl UnlockedVault {
    // -- CRUD ---------------------------------------------------------------

    pub fn automations(&self) -> Vec<Automation> {
        let mut out = self.doc.automations.clone();
        out.sort_by_key(|automation| automation.name.to_lowercase());
        out
    }

    /// Reject a draft that could never run: a bad schedule, or a template
    /// that fails the transaction rules on its own.
    fn check_automation(&self, draft: &AutomationDraft) -> Result<()> {
        if draft.name.trim().is_empty() {
            return Err(Error::InvalidInput("name the automation".into()));
        }
        match draft.schedule {
            Schedule::Monthly { day } if !(1..=31).contains(&day) => {
                return Err(Error::InvalidInput("day of month is 1–31".into()));
            }
            Schedule::Weekly { weekday } if weekday > 6 => {
                return Err(Error::InvalidInput("weekday is 0–6".into()));
            }
            Schedule::EveryNDays { days: 0 } => {
                return Err(Error::InvalidInput("repeat at least every day".into()));
            }
            _ => {}
        }
        if unix_secs(&draft.starts_at).is_none() {
            return Err(Error::InvalidInput("choose a start date".into()));
        }
        let mut probe = draft_to_transaction(&draft.template, "", "", &draft.starts_at);
        self.prepare_transaction(&mut probe)
    }

    pub fn create_automation(&mut self, draft: AutomationDraft) -> Result<Automation> {
        self.check_automation(&draft)?;
        let automation = Automation {
            id: uuid::Uuid::new_v4().to_string(),
            name: draft.name.trim().to_string(),
            active: draft.active,
            preset: draft.preset,
            schedule: draft.schedule,
            starts_at: draft.starts_at,
            template: draft.template,
            last_planned_at: None,
            created_at: now_iso(),
        };
        self.doc.automations.push(automation.clone());
        self.persist()?;
        Ok(automation)
    }

    /// Change what an automation does. Dates already processed stay
    /// processed: the run history deduplicates by planned date.
    pub fn update_automation(&mut self, id: &str, draft: AutomationDraft) -> Result<Automation> {
        self.check_automation(&draft)?;
        let automation = self
            .doc
            .automations
            .iter_mut()
            .find(|automation| automation.id == id)
            .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
        automation.name = draft.name.trim().to_string();
        automation.active = draft.active;
        automation.preset = draft.preset;
        automation.schedule = draft.schedule;
        automation.starts_at = draft.starts_at;
        automation.template = draft.template;
        let out = automation.clone();
        self.persist()?;
        Ok(out)
    }

    /// Remove an automation and its run history. Transactions it created stay.
    pub fn delete_automation(&mut self, id: &str) -> Result<()> {
        let before = self.doc.automations.len();
        self.doc
            .automations
            .retain(|automation| automation.id != id);
        if self.doc.automations.len() == before {
            return Err(Error::Missing(format!("automation {id}")));
        }
        self.doc
            .automation_runs
            .retain(|run| run.automation_id != id);
        self.persist()
    }

    /// Run history, newest planned date first.
    pub fn automation_runs(&self, automation_id: Option<&str>) -> Vec<AutomationRun> {
        let mut out: Vec<AutomationRun> = self
            .doc
            .automation_runs
            .iter()
            .filter(|run| automation_id.is_none_or(|id| run.automation_id == id))
            .cloned()
            .collect();
        out.sort_by(|left, right| right.planned_at.cmp(&left.planned_at));
        out
    }

    // -- funds (D26) --------------------------------------------------------

    /// Quantity of an asset on one store from transactions up to `secs`.
    pub(crate) fn balance_at(&self, store_id: &str, asset_key: &str, secs: i64) -> Amount {
        self.doc
            .transactions
            .iter()
            .filter(|tx| secs_or_now(&tx.meta().timestamp) <= secs)
            .flat_map(|tx| tx.legs())
            .filter(|leg| leg.store_id == store_id && leg.asset.key() == asset_key)
            .fold(Amount::ZERO, |sum, leg| sum + leg.amount)
    }

    /// Whether the source of a transaction can pay for it at `secs`: its
    /// balance (principal and a fee in the same asset; a fee in another asset
    /// separately), or for a credit card its limit. Money coming in, and money
    /// leaving someone else's side, is not checked.
    /// `None` when it can.
    pub(crate) fn funds_shortfall(&self, tx: &Transaction, secs: i64) -> Option<RejectReason> {
        let Outflow {
            store_id,
            asset,
            amount,
            fee,
        } = outflow(tx)?;
        let Some(store) = self.find_store(&store_id) else {
            return Some(RejectReason::Invalid {
                message: format!("store {store_id} not found"),
            });
        };
        let same_asset_fee = fee
            .as_ref()
            .filter(|(fee_asset, _)| fee_asset.key() == asset.key())
            .map_or(Amount::ZERO, |(_, fee_amount)| *fee_amount);
        if store.kind == StoreKind::Liability {
            let limit = store.liability.as_ref().and_then(|terms| terms.limit)?;
            let debt =
                (Amount::ZERO - self.balance_at(&store_id, &asset.key(), secs)).max(Amount::ZERO);
            let required = debt + amount + same_asset_fee;
            return (required > limit).then_some(RejectReason::CreditLimitExceeded {
                store_id,
                limit,
                required,
            });
        }
        let mut needs = vec![(asset.clone(), amount + same_asset_fee)];
        if let Some((fee_asset, fee_amount)) = fee
            && fee_asset.key() != asset.key()
        {
            needs.push((fee_asset, fee_amount));
        }
        for (need_asset, required) in needs {
            let available = self.balance_at(&store_id, &need_asset.key(), secs);
            if available < required {
                return Some(RejectReason::InsufficientFunds {
                    store_id,
                    asset: need_asset,
                    required,
                    available,
                });
            }
        }
        None
    }

    // -- running ------------------------------------------------------------

    /// Planned dates already processed, by run or by the transaction's origin.
    fn processed_dates(&self) -> HashSet<(String, String)> {
        let mut out: HashSet<(String, String)> = self
            .doc
            .automation_runs
            .iter()
            .map(|run| (run.automation_id.clone(), run.planned_at.clone()))
            .collect();
        for tx in &self.doc.transactions {
            if let TxOrigin::Automation {
                automation_id,
                planned_at,
            } = &tx.meta().origin
            {
                out.insert((automation_id.clone(), planned_at.clone()));
            }
        }
        out
    }

    /// Every due planned date of every active automation, oldest first.
    fn due_runs(&self, now: i64, cal: &Calendar) -> Vec<DueRun> {
        let done = self.processed_dates();
        let today = cal.date_of(now);
        let mut out = Vec::new();
        for automation in self.doc.automations.iter().filter(|item| item.active) {
            let Some(start) = cal.date_of_iso(&automation.starts_at) else {
                continue;
            };
            let after = automation
                .last_planned_at
                .as_deref()
                .and_then(|last| cal.date_of_iso(last));
            for date in planned_dates(&automation.schedule, start, after, today) {
                let secs = cal.at_local(date, AUTOMATION_HOUR);
                if secs > now || done.contains(&(automation.id.clone(), calendar::iso(secs))) {
                    continue;
                }
                out.push(DueRun {
                    automation_id: automation.id.clone(),
                    secs,
                });
            }
        }
        out.sort_by_key(|run| run.secs);
        out
    }

    /// Process every due planned date (plan §7.2): strictly by date across
    /// automations, so a salary on the 10th covers rent on the 15th. Funds are
    /// checked at the planned moment; a shortfall records a rejected run and
    /// creates nothing. Only the newest [`MAX_CATCH_UP`] dates are created;
    /// older ones are recorded as skipped.
    pub fn run_due_automations(&mut self, now: i64, zone: &str) -> Result<AutomationPass> {
        let cal = Calendar::new(zone);
        let mut due = self.due_runs(now, &cal);
        let mut pass = AutomationPass::default();
        if due.is_empty() {
            return Ok(pass);
        }
        let overflow = due.len().saturating_sub(MAX_CATCH_UP);
        for run in due.drain(..overflow) {
            self.record_run(&run.automation_id, run.secs, AutomationOutcome::Skipped);
            pass.skipped += 1;
        }
        for run in due {
            let Some(automation) = self
                .doc
                .automations
                .iter()
                .find(|item| item.id == run.automation_id)
                .cloned()
            else {
                continue;
            };
            let planned_at = calendar::iso(run.secs);
            let tx = draft_to_transaction(
                &automation.template,
                &automation.id,
                &planned_at,
                &planned_at,
            );
            let outcome = match self.funds_shortfall(&tx, run.secs) {
                Some(reason) => AutomationOutcome::Rejected { reason },
                None => match self.insert_transaction(tx) {
                    Ok(saved) => {
                        pass.created.push(saved.meta().id.clone());
                        AutomationOutcome::Created {
                            tx_id: saved.meta().id.clone(),
                        }
                    }
                    Err(err) => AutomationOutcome::Rejected {
                        reason: RejectReason::Invalid {
                            message: err.to_string(),
                        },
                    },
                },
            };
            let recorded = self.record_run(&automation.id, run.secs, outcome);
            if matches!(recorded.outcome, AutomationOutcome::Rejected { .. }) {
                pass.rejected.push(recorded);
            }
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(pass)
    }

    /// Validate and append without persisting — a pass writes once at the end.
    fn insert_transaction(&mut self, mut tx: Transaction) -> Result<Transaction> {
        self.prepare_transaction(&mut tx)?;
        self.doc.transactions.push(tx.clone());
        Ok(tx)
    }

    /// Store a run, advance the automation's last processed date, keep the
    /// history bounded.
    fn record_run(
        &mut self,
        automation_id: &str,
        secs: i64,
        outcome: AutomationOutcome,
    ) -> AutomationRun {
        let planned_at = calendar::iso(secs);
        let run = AutomationRun {
            id: uuid::Uuid::new_v4().to_string(),
            automation_id: automation_id.to_string(),
            planned_at: planned_at.clone(),
            processed_at: now_iso(),
            outcome,
        };
        self.doc.automation_runs.push(run.clone());
        if self.doc.automation_runs.len() > MAX_AUTOMATION_RUNS {
            self.doc
                .automation_runs
                .sort_by(|left, right| left.processed_at.cmp(&right.processed_at));
            let extra = self.doc.automation_runs.len() - MAX_AUTOMATION_RUNS;
            self.doc.automation_runs.drain(..extra);
        }
        if let Some(automation) = self
            .doc
            .automations
            .iter_mut()
            .find(|item| item.id == automation_id)
            && automation
                .last_planned_at
                .as_deref()
                .is_none_or(|last| secs_or_now(last) < secs)
        {
            automation.last_planned_at = Some(planned_at);
        }
        run
    }

    /// Create the transaction of an automation now («Создать сейчас»), with
    /// the same funds check as the schedule.
    pub fn run_automation(&mut self, id: &str, now: i64) -> Result<Transaction> {
        let automation = self
            .doc
            .automations
            .iter()
            .find(|item| item.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("automation {id}")))?;
        let planned_at = calendar::iso(now);
        let tx = draft_to_transaction(&automation.template, id, &planned_at, &planned_at);
        let saved = self.create_checked(tx, now)?;
        self.push_created_run(id, &planned_at, &saved);
        self.persist()?;
        Ok(saved)
    }

    /// Retry a rejected or skipped run: funds are checked at the current
    /// moment and the transaction is dated now.
    pub fn retry_automation_run(&mut self, run_id: &str, now: i64) -> Result<Transaction> {
        let run = self
            .doc
            .automation_runs
            .iter()
            .find(|run| run.id == run_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("automation run {run_id}")))?;
        if matches!(run.outcome, AutomationOutcome::Created { .. }) {
            return Err(Error::InvalidInput(
                "this run already created its transaction".into(),
            ));
        }
        let automation = self
            .doc
            .automations
            .iter()
            .find(|item| item.id == run.automation_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("automation {}", run.automation_id)))?;
        let tx = draft_to_transaction(
            &automation.template,
            &automation.id,
            &run.planned_at,
            &calendar::iso(now),
        );
        let saved = self.create_checked(tx, now)?;
        if let Some(stored) = self
            .doc
            .automation_runs
            .iter_mut()
            .find(|stored| stored.id == run_id)
        {
            stored.outcome = AutomationOutcome::Created {
                tx_id: saved.meta().id.clone(),
            };
            stored.processed_at = now_iso();
        }
        self.persist()?;
        Ok(saved)
    }

    /// Dismiss a rejected run.
    pub fn skip_automation_run(&mut self, run_id: &str) -> Result<AutomationRun> {
        let run = self
            .doc
            .automation_runs
            .iter_mut()
            .find(|run| run.id == run_id)
            .ok_or_else(|| Error::Missing(format!("automation run {run_id}")))?;
        if matches!(run.outcome, AutomationOutcome::Created { .. }) {
            return Err(Error::InvalidInput(
                "this run already created its transaction".into(),
            ));
        }
        run.outcome = AutomationOutcome::Skipped;
        run.processed_at = now_iso();
        let out = run.clone();
        self.persist()?;
        Ok(out)
    }

    /// Funds check, then write; a shortfall is an `INSUFFICIENT_FUNDS` error.
    fn create_checked(&mut self, tx: Transaction, now: i64) -> Result<Transaction> {
        if let Some(reason) = self.funds_shortfall(&tx, now) {
            return Err(match reason {
                RejectReason::Invalid { message } => Error::InvalidInput(message),
                other => Error::InsufficientFunds(self.describe_shortfall(&other)),
            });
        }
        let saved = self.insert_transaction(tx)?;
        self.recompute_holdings();
        Ok(saved)
    }

    /// «Card: 500 USD of 800 USD» — with the store's name, for the error text.
    fn describe_shortfall(&self, reason: &RejectReason) -> String {
        let name = |id: &str| {
            self.find_store(id)
                .map_or_else(|| id.to_string(), |store| store.name.clone())
        };
        match reason {
            RejectReason::InsufficientFunds {
                store_id,
                asset,
                required,
                available,
            } => format!(
                "{}: {available} {sym} / {required} {sym}",
                name(store_id),
                sym = asset.symbol
            ),
            RejectReason::CreditLimitExceeded {
                store_id,
                limit,
                required,
            } => format!("{}: {required} > {limit}", name(store_id)),
            RejectReason::Invalid { message } => message.clone(),
        }
    }

    fn push_created_run(&mut self, automation_id: &str, planned_at: &str, saved: &Transaction) {
        self.doc.automation_runs.push(AutomationRun {
            id: uuid::Uuid::new_v4().to_string(),
            automation_id: automation_id.to_string(),
            planned_at: planned_at.to_string(),
            processed_at: now_iso(),
            outcome: AutomationOutcome::Created {
                tx_id: saved.meta().id.clone(),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{LiabilityKind, LiabilityTerms};
    use crate::model::transaction::{AccountRef, PaymentTx, TopupTx};
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn date(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    fn secs(iso: &str) -> i64 {
        unix_secs(iso).unwrap()
    }

    fn salary_draft(store: &str, sum: &str) -> TransactionDraft {
        let Transaction::Topup(inner) = income(store, sum, "salary") else {
            unreachable!()
        };
        TransactionDraft::Topup(TopupTx { ..inner })
    }

    fn rent_draft(store: &str, sum: &str) -> TransactionDraft {
        let Transaction::Payment(mut inner) = payment(store, sum, "rent") else {
            unreachable!()
        };
        inner.meta.categories = vec!["rent_mortgage".into()];
        TransactionDraft::Payment(PaymentTx { ..inner })
    }

    fn monthly(
        vault: &mut UnlockedVault,
        name: &str,
        day: u8,
        template: TransactionDraft,
    ) -> Automation {
        vault
            .create_automation(AutomationDraft {
                name: name.into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template,
            })
            .unwrap()
    }

    fn outcomes(vault: &UnlockedVault, id: &str) -> Vec<&'static str> {
        let mut runs = vault.automation_runs(Some(id));
        runs.reverse();
        runs.iter()
            .map(|run| match run.outcome {
                AutomationOutcome::Created { .. } => "created",
                AutomationOutcome::Rejected { .. } => "rejected",
                AutomationOutcome::Skipped => "skipped",
            })
            .collect()
    }

    #[test]
    fn the_31st_runs_on_the_last_day_of_february() {
        let dates = planned_dates(
            &Schedule::Monthly { day: 31 },
            date("2026-01-01"),
            None,
            date("2026-03-31"),
        );
        assert_eq!(
            dates,
            vec![date("2026-01-31"), date("2026-02-28"), date("2026-03-31")]
        );
        let weekly = planned_dates(
            &Schedule::Weekly { weekday: 0 },
            date("2026-03-04"),
            Some(date("2026-03-09")),
            date("2026-03-23"),
        );
        assert_eq!(weekly, vec![date("2026-03-16"), date("2026-03-23")]);
    }

    #[test]
    fn salary_before_rent_covers_it_and_rent_before_salary_is_rejected() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
        let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));

        let pass = vault
            .run_due_automations(secs("2026-02-20T12:00:00Z"), "UTC")
            .unwrap();
        // Jan 5 rent: nothing yet. Jan 10 salary. Feb 5 rent: 1000 ≥ 800. Feb 10 salary.
        assert_eq!(outcomes(&vault, &rent.id), vec!["rejected", "created"]);
        assert_eq!(outcomes(&vault, &salary.id), vec!["created", "created"]);
        assert_eq!(pass.created.len(), 3);
        assert_eq!(pass.rejected.len(), 1);
        let AutomationOutcome::Rejected {
            reason:
                RejectReason::InsufficientFunds {
                    required,
                    available,
                    ..
                },
        } = &pass.rejected[0].outcome
        else {
            panic!("{:?}", pass.rejected[0]);
        };
        assert_eq!((*required, *available), (amount("800"), amount("0")));
        assert_eq!(holding_of(&vault, &card, &usd()), amount("1200"));

        // The processed dates are not processed again.
        let again = vault
            .run_due_automations(secs("2026-02-20T13:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(again, AutomationPass::default());
    }

    #[test]
    fn the_fee_counts_toward_the_required_amount() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let mut seed = income(&card, "100", "seed");
        at(&mut seed, "2025-12-01T00:00:00Z");
        vault.add_transaction(seed).unwrap();
        let TransactionDraft::Payment(mut bill) = rent_draft(&card, "95") else {
            unreachable!()
        };
        bill.fee = fee_usd("10");
        let automation = monthly(&mut vault, "Bill", 1, TransactionDraft::Payment(bill));
        let pass = vault
            .run_due_automations(secs("2026-01-02T00:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(outcomes(&vault, &automation.id), vec!["rejected"]);
        let AutomationOutcome::Rejected {
            reason: RejectReason::InsufficientFunds { required, .. },
        } = &pass.rejected[0].outcome
        else {
            panic!()
        };
        assert_eq!(*required, amount("105"));
    }

    #[test]
    fn catch_up_creates_at_most_24_and_skips_the_rest() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let daily = vault
            .create_automation(AutomationDraft {
                name: "Pocket money".into(),
                active: true,
                preset: None,
                schedule: Schedule::EveryNDays { days: 1 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: salary_draft(&card, "1"),
            })
            .unwrap();
        let pass = vault
            .run_due_automations(secs("2026-01-30T12:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(pass.created.len(), MAX_CATCH_UP);
        assert_eq!(pass.skipped, 30 - MAX_CATCH_UP);
        assert_eq!(vault.automation_runs(Some(&daily.id)).len(), 30);

        // Even with the processed marker rolled back nothing is created twice.
        vault.doc.automations[0].last_planned_at = None;
        let again = vault
            .run_due_automations(secs("2026-01-30T12:00:00Z"), "UTC")
            .unwrap();
        assert_eq!(again, AutomationPass::default());
    }

    #[test]
    fn credit_card_checks_the_limit_and_without_one_passes() {
        let (_dir, mut vault) = empty_vault();
        let terms = |limit: Option<&str>| LiabilityTerms {
            kind: LiabilityKind::CreditCard,
            rate: None,
            limit: limit.map(amount),
            due_day: None,
            monthly_payment: None,
        };
        let limited = vault
            .create_store_with(
                "Card",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(terms(Some("100"))),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let open = vault
            .create_store_with(
                "Open card",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(terms(None)),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let over = draft_to_transaction(
            &rent_draft(&limited, "150"),
            "a",
            "",
            "2026-01-01T09:00:00Z",
        );
        assert!(matches!(
            vault.funds_shortfall(&over, secs("2026-01-01T09:00:00Z")),
            Some(RejectReason::CreditLimitExceeded { .. })
        ));
        let within =
            draft_to_transaction(&rent_draft(&limited, "90"), "a", "", "2026-01-01T09:00:00Z");
        assert!(
            vault
                .funds_shortfall(&within, secs("2026-01-01T09:00:00Z"))
                .is_none()
        );
        let unlimited =
            draft_to_transaction(&rent_draft(&open, "5000"), "a", "", "2026-01-01T09:00:00Z");
        assert!(
            vault
                .funds_shortfall(&unlimited, secs("2026-01-01T09:00:00Z"))
                .is_none()
        );
    }

    #[test]
    fn incoming_money_and_a_contact_source_are_not_checked() {
        let (_dir, vault) = empty_vault();
        let topup = draft_to_transaction(&salary_draft("x", "100"), "a", "", "");
        assert!(vault.funds_shortfall(&topup, 0).is_none());
        let from_contact = transfer_between(
            AccountRef::Raw("Mom".into()),
            AccountRef::Store("x".into()),
            usd(),
            "100",
        );
        assert!(vault.funds_shortfall(&from_contact, 0).is_none());
    }

    #[test]
    fn retry_checks_the_current_balance_and_skip_dismisses() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));
        vault
            .run_due_automations(secs("2026-02-20T12:00:00Z"), "UTC")
            .unwrap();
        let runs = vault.automation_runs(Some(&rent.id));
        assert_eq!(runs.len(), 2);

        let now = secs("2026-02-21T12:00:00Z");
        let err = vault.retry_automation_run(&runs[0].id, now).unwrap_err();
        assert_eq!(err.code(), "INSUFFICIENT_FUNDS");

        let mut salary = income(&card, "1000", "salary");
        at(&mut salary, "2026-02-21T10:00:00Z");
        vault.add_transaction(salary).unwrap();
        let created = vault.retry_automation_run(&runs[0].id, now).unwrap();
        assert_eq!(created.meta().timestamp, "2026-02-21T12:00:00Z");
        assert!(matches!(
            vault.automation_runs(Some(&rent.id))[0].outcome,
            AutomationOutcome::Created { .. }
        ));

        let skipped = vault.skip_automation_run(&runs[1].id).unwrap();
        assert_eq!(skipped.outcome, AutomationOutcome::Skipped);
    }

    #[test]
    fn the_planned_moment_is_nine_in_the_local_zone() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        monthly(&mut vault, "Salary", 10, salary_draft(&card, "1"));
        // 09:00 in Bangkok on Jan 10 is 02:00 UTC.
        let pass = vault
            .run_due_automations(secs("2026-01-10T02:30:00Z"), "Asia/Bangkok")
            .unwrap();
        assert_eq!(pass.created.len(), 1);
        let tx = &vault.doc.transactions[0];
        assert_eq!(tx.meta().timestamp, "2026-01-10T02:00:00Z");
    }

    #[test]
    fn a_template_that_breaks_the_rules_is_refused() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let TransactionDraft::Payment(mut bill) = rent_draft(&card, "10") else {
            unreachable!()
        };
        bill.meta.categories.clear();
        let err = vault
            .create_automation(AutomationDraft {
                name: "Bill".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 1 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Payment(bill),
            })
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");
    }
}
