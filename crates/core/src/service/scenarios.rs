//! Saved investment calculations (plan A): CRUD, «Из данных» and goals that
//! follow a scenario.

use std::collections::HashSet;

use chrono::{Months, NaiveDate};

use crate::Amount;
use crate::calc::invest::{self, CapFreq, ContributionStage, Freq, InvestInput};
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::ipc_contract::GoalInput;
use crate::model::deposit::Capitalization;
use crate::model::planning::{CalcScenario, ScenarioSource};
use crate::model::store::StoreScope;
use crate::model::vault::now_iso;

use super::UnlockedVault;

/// Periods «Из данных» can average income over.
pub const INCOME_MONTHS: [u8; 3] = [3, 6, 12];
/// Importance of a goal made from a scenario, the middle of 1–5.
const SCENARIO_GOAL_PRIORITY: u8 = 3;

/// What the user saves: `id = None` makes a new scenario.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CalcScenarioDraft {
    pub id: Option<String>,
    pub name: String,
    pub input: InvestInput,
    pub source: Option<ScenarioSource>,
    pub prefilled_at: Option<String>,
}

/// A deleted scenario and the goals it was unlinked from: enough to undo.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CalcScenarioDeleted {
    pub scenario: CalcScenario,
    pub goal_ids: Vec<String>,
}

/// Numbers «Из данных» puts into the calculator, with where they came from.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ScenarioPrefill {
    /// Value of the source wallets in the scenario currency.
    pub initial: Amount,
    /// The deposit rate of the largest source wallet that has one.
    pub rate_pct: Option<Amount>,
    pub capitalization: Option<CapFreq>,
    /// That deposit pays interest out instead of compounding it.
    pub pays_out: bool,
    pub rate_store_id: Option<String>,
    /// `income × income_pct / 100` in the scenario currency.
    pub monthly_contribution: Option<Amount>,
    /// Average monthly income in the scenario currency.
    pub income_avg: Option<Amount>,
    /// Finished months the average really took; fewer than asked when the
    /// history is short.
    pub income_months_used: u32,
    pub prefilled_at: String,
}

/// A stage's contribution spread over a month.
fn per_month(stage: &ContributionStage) -> Amount {
    match stage.freq {
        Freq::Monthly => stage.amount,
        Freq::Quarterly => stage.amount / Amount::from(3),
        Freq::Yearly => stage.amount / Amount::from(12),
    }
}

/// The balance a scenario plans for `elapsed` months after its start.
pub fn planned_balance(input: &InvestInput, elapsed: u32) -> Result<Amount> {
    if elapsed == 0 {
        return Ok(input.initial);
    }
    let result = invest::simulate(input)?;
    let months: Vec<_> = result.years.iter().flat_map(|year| &year.months).collect();
    let index = (elapsed as usize).min(months.len());
    Ok(months
        .get(index.wrapping_sub(1))
        .map_or(input.initial, |month| month.balance))
}

impl UnlockedVault {
    /// Saved scenarios, the last changed first.
    pub fn calc_scenarios(&self) -> Vec<CalcScenario> {
        let mut out = self.doc.calc_scenarios.clone();
        out.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        out
    }

    pub(crate) fn find_calc_scenario(&self, id: &str) -> Option<&CalcScenario> {
        self.doc
            .calc_scenarios
            .iter()
            .find(|scenario| scenario.id == id)
    }

    fn check_scenario_source(&self, source: &ScenarioSource) -> Result<()> {
        if !INCOME_MONTHS.contains(&source.income_months) {
            return Err(Error::InvalidInput(
                "income period is 3, 6 or 12 months".into(),
            ));
        }
        if source.income_pct.is_negative() || source.income_pct > Amount::from(100) {
            return Err(Error::InvalidInput("the share of income is 0–100%".into()));
        }
        match source
            .scope
            .store_ids
            .iter()
            .find(|id| self.find_store(id).is_none())
        {
            Some(id) => Err(Error::Missing(format!("store {id}"))),
            None => Ok(()),
        }
    }

    pub fn save_calc_scenario(&mut self, draft: CalcScenarioDraft) -> Result<CalcScenario> {
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::InvalidInput("name the scenario".into()));
        }
        invest::simulate(&draft.input)?;
        if let Some(source) = &draft.source {
            self.check_scenario_source(source)?;
        }
        let now = now_iso();
        let saved = match draft.id.as_deref() {
            Some(id) => {
                let scenario = self
                    .doc
                    .calc_scenarios
                    .iter_mut()
                    .find(|scenario| scenario.id == id)
                    .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
                scenario.name = name;
                scenario.input = draft.input;
                scenario.source = draft.source;
                scenario.prefilled_at = draft.prefilled_at;
                scenario.updated_at = now;
                scenario.clone()
            }
            None => {
                let scenario = CalcScenario {
                    id: uuid::Uuid::new_v4().to_string(),
                    name,
                    input: draft.input,
                    source: draft.source,
                    prefilled_at: draft.prefilled_at,
                    created_at: now.clone(),
                    updated_at: now,
                };
                self.doc.calc_scenarios.push(scenario.clone());
                scenario
            }
        };
        self.persist()?;
        Ok(saved)
    }

    /// Delete a scenario; goals made from it stay and stop following it.
    pub fn delete_calc_scenario(&mut self, id: &str) -> Result<CalcScenarioDeleted> {
        let index = self
            .doc
            .calc_scenarios
            .iter()
            .position(|scenario| scenario.id == id)
            .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
        let scenario = self.doc.calc_scenarios.remove(index);
        let mut goal_ids = Vec::new();
        for goal in &mut self.doc.goals {
            if goal.scenario_id.as_deref() == Some(id) {
                goal.scenario_id = None;
                goal_ids.push(goal.id.clone());
            }
        }
        self.persist()?;
        Ok(CalcScenarioDeleted { scenario, goal_ids })
    }

    /// Undo [`Self::delete_calc_scenario`]: the scenario and its goal links.
    pub fn restore_calc_scenario(&mut self, deleted: CalcScenarioDeleted) -> Result<CalcScenario> {
        let id = deleted.scenario.id.clone();
        if self.find_calc_scenario(&id).is_some() {
            return Err(Error::InvalidInput(format!("scenario {id} already exists")));
        }
        self.doc.calc_scenarios.push(deleted.scenario.clone());
        let goals: HashSet<&String> = deleted.goal_ids.iter().collect();
        for goal in &mut self.doc.goals {
            if goals.contains(&goal.id) && goal.scenario_id.is_none() {
                goal.scenario_id = Some(id.clone());
            }
        }
        self.persist()?;
        Ok(deleted.scenario)
    }

    /// Start amount, rate and contribution from the user's data (plan A,
    /// decisions 7 and 9). Nothing is saved.
    pub fn scenario_prefill(
        &self,
        source: &ScenarioSource,
        currency: &str,
        now: i64,
        zone: &str,
    ) -> Result<ScenarioPrefill> {
        self.check_scenario_source(source)?;
        let cal = Calendar::new(zone);
        let book = self.rate_book();
        let values = self.store_values_usd();
        let scope = self.resolve_scope(&source.scope);
        let initial_usd = scope
            .iter()
            .filter_map(|id| values.get(id))
            .fold(Amount::ZERO, |sum, value| sum + *value);
        let initial = book
            .convert(initial_usd, "usd", currency, now)
            .unwrap_or(Amount::ZERO)
            .round(2);

        let today = cal.date_of(now).format("%Y-%m-%d").to_string();
        let deposit = scope
            .iter()
            .filter_map(|id| {
                let store = self.find_store(id)?;
                let terms = store.deposit.as_ref()?;
                let rate = terms.rate_on(&today)?;
                let value = values.get(id).copied().unwrap_or_default();
                Some((value, store.id.clone(), rate, terms.capitalization))
            })
            .max_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)));
        let (rate_store_id, rate_pct, capitalization, pays_out) = match deposit {
            Some((_, id, rate, cap)) => {
                let (freq, pays_out) = match cap {
                    Capitalization::Daily => (CapFreq::Daily, false),
                    Capitalization::Monthly => (CapFreq::Monthly, false),
                    Capitalization::AtEnd => (CapFreq::AtEnd, false),
                    Capitalization::None => (CapFreq::Monthly, true),
                };
                (Some(id), Some(rate), Some(freq), pays_out)
            }
            None => (None, None, None, false),
        };

        let average = self.avg_month(source.income_months as usize, now, zone);
        let income_avg = average.as_ref().and_then(|avg| {
            book.convert(avg.income, &avg.currency, currency, now)
                .map(|value| value.round(2))
        });
        let monthly_contribution =
            income_avg.map(|income| (income * source.income_pct / Amount::from(100)).round(2));
        Ok(ScenarioPrefill {
            initial,
            rate_pct,
            capitalization,
            pays_out,
            rate_store_id,
            monthly_contribution,
            income_avg,
            income_months_used: average.map_or(0, |avg| avg.months),
            prefilled_at: calendar::iso(now),
        })
    }

    /// A goal to reach what the scenario ends with, for the goal form to
    /// fill in (plan A, decision 12). Nothing is saved.
    pub fn goal_from_scenario(&self, id: &str, now: i64, zone: &str) -> Result<GoalInput> {
        let scenario = self
            .find_calc_scenario(id)
            .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
        let result = invest::simulate(&scenario.input)?;
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let deadline = today
            .checked_add_months(Months::new(scenario.input.term_months))
            .unwrap_or(today);
        let at_midnight = |date: NaiveDate| calendar::iso(cal.at_local(date, 0));
        let first_stage = scenario
            .input
            .stages
            .iter()
            .min_by_key(|stage| stage.from_month);
        Ok(GoalInput {
            name: scenario.name.clone(),
            active: true,
            target_amount: result.final_nominal,
            currency: scenario.input.currency.trim().to_lowercase(),
            starts_at: at_midnight(today),
            deadline: Some(at_midnight(deadline)),
            priority: SCENARIO_GOAL_PRIORITY,
            monthly_commitment: first_stage.map(|stage| per_month(stage).round(2)),
            scope: scenario
                .source
                .as_ref()
                .map_or_else(StoreScope::default, |source| source.scope.clone()),
            scenario_id: Some(scenario.id.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::invest::{TaxRule, Timing};
    use crate::model::deposit::{DepositTerms, RatePeriod};
    use crate::model::store::{Binding, StoreKind, SubType};
    use crate::model::transaction::{AccountRef, Transaction};
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;
    use crate::service::{EntityKind, EntityRef, RelationKind, StoreExtras};

    fn input(initial: &str, monthly: &str, months: u32) -> InvestInput {
        InvestInput {
            currency: "usd".into(),
            start: "2026-01".into(),
            initial: amount(initial),
            rate_pct: amount("12"),
            term_months: months,
            capitalization: CapFreq::Monthly,
            stages: vec![ContributionStage {
                from_month: 1,
                amount: amount(monthly),
                freq: Freq::Monthly,
                timing: Timing::End,
            }],
            indexation_pct: Amount::ZERO,
            inflation_pct: Amount::ZERO,
            tax: TaxRule::default(),
            withdrawal: None,
        }
    }

    fn draft(name: &str) -> CalcScenarioDraft {
        CalcScenarioDraft {
            id: None,
            name: name.into(),
            input: input("1000", "100", 12),
            source: None,
            prefilled_at: None,
        }
    }

    fn deposit_store(vault: &mut UnlockedVault, name: &str) -> String {
        let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
        let extras = StoreExtras {
            sub_type: Some(SubType::Deposit),
            binding: Some(Binding::Asset(usd())),
            ..StoreExtras::default()
        };
        vault
            .create_store_with(name, StoreKind::Bank, "#000", "", Some(bank), extras)
            .unwrap()
            .id
    }

    fn usd_transfer(from: &str, to: &str, raw: &str) -> Transaction {
        let from = AccountRef::Store(from.into());
        transfer_between(from, AccountRef::Store(to.into()), usd(), raw)
    }

    fn dated(mut tx: Transaction, ts: &str) -> Transaction {
        at(&mut tx, ts);
        tx
    }

    #[test]
    fn scenarios_are_saved_updated_and_checked() {
        let (_dir, mut vault) = empty_vault();
        assert!(vault.save_calc_scenario(draft("  ")).is_err());
        let mut broken = draft("Broken");
        broken.input.initial = amount("-1");
        assert!(vault.save_calc_scenario(broken).is_err());
        let mut odd = draft("Odd period");
        odd.source = Some(ScenarioSource {
            scope: StoreScope::default(),
            income_months: 5,
            income_pct: amount("10"),
        });
        assert!(vault.save_calc_scenario(odd).is_err());

        let saved = vault.save_calc_scenario(draft(" Pension ")).unwrap();
        assert_eq!(saved.name, "Pension");
        let mut edit = draft("Pension 2");
        edit.id = Some(saved.id.clone());
        let updated = vault.save_calc_scenario(edit).unwrap();
        assert_eq!(updated.id, saved.id);
        assert_eq!(updated.created_at, saved.created_at);
        assert_eq!(vault.calc_scenarios().len(), 1);
        assert_eq!(vault.calc_scenarios()[0].name, "Pension 2");

        let mut missing = draft("Missing");
        missing.id = Some("nope".into());
        assert!(vault.save_calc_scenario(missing).is_err());
    }

    #[test]
    fn deleting_a_scenario_unlinks_its_goals_and_undo_links_them_back() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let scenario = vault.save_calc_scenario(draft("Flat")).unwrap();
        let mut goal = vault
            .goal_from_scenario(
                &scenario.id,
                unix_secs("2026-03-10T10:00:00Z").unwrap(),
                "UTC",
            )
            .unwrap();
        goal.scope = StoreScope {
            store_ids: vec![card],
            include_children: false,
        };
        let goal = vault.create_goal(goal.into()).unwrap();
        assert_eq!(goal.scenario_id.as_deref(), Some(scenario.id.as_str()));

        let deleted = vault.delete_calc_scenario(&scenario.id).unwrap();
        assert_eq!(deleted.goal_ids, vec![goal.id.clone()]);
        assert!(vault.calc_scenarios().is_empty());
        assert_eq!(vault.goals()[0].scenario_id, None);
        assert!(vault.delete_calc_scenario(&scenario.id).is_err());

        vault.restore_calc_scenario(deleted.clone()).unwrap();
        assert_eq!(vault.calc_scenarios().len(), 1);
        assert_eq!(
            vault.goals()[0].scenario_id.as_deref(),
            Some(scenario.id.as_str())
        );
        assert!(vault.restore_calc_scenario(deleted).is_err());
    }

    #[test]
    fn a_goal_with_an_unknown_scenario_is_refused() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let scenario = vault.save_calc_scenario(draft("Flat")).unwrap();
        let mut goal = vault
            .goal_from_scenario(
                &scenario.id,
                unix_secs("2026-03-10T10:00:00Z").unwrap(),
                "UTC",
            )
            .unwrap();
        goal.scope.store_ids = vec![card];
        goal.scenario_id = Some("gone".into());
        assert!(vault.create_goal(goal.into()).is_err());
    }

    #[test]
    fn goal_from_scenario_aims_at_the_final_amount_over_the_term() {
        let (_dir, mut vault) = empty_vault();
        let mut quarterly = draft("Car");
        quarterly.input.stages[0].freq = Freq::Quarterly;
        quarterly.input.stages[0].amount = amount("300");
        let scenario = vault.save_calc_scenario(quarterly.clone()).unwrap();
        let goal = vault
            .goal_from_scenario(
                &scenario.id,
                unix_secs("2026-03-10T10:00:00Z").unwrap(),
                "UTC",
            )
            .unwrap();
        let expected = invest::simulate(&quarterly.input).unwrap().final_nominal;
        assert_eq!(goal.target_amount, expected);
        assert_eq!(goal.name, "Car");
        assert_eq!(goal.starts_at, "2026-03-10T00:00:00Z");
        assert_eq!(goal.deadline.as_deref(), Some("2027-03-10T00:00:00Z"));
        assert_eq!(goal.monthly_commitment, Some(amount("100")));
        assert_eq!(goal.scenario_id, Some(scenario.id));
        assert!(vault.goal_from_scenario("nope", 0, "UTC").is_err());
    }

    #[test]
    fn prefill_takes_wallet_value_deposit_terms_and_a_share_of_income() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let deposit = deposit_store(&mut vault, "Deposit");
        let small = deposit_store(&mut vault, "Small deposit");
        for (target, rate, cap) in [
            (&deposit, "16", Capitalization::Daily),
            (&small, "20", Capitalization::None),
        ] {
            vault
                .set_deposit_terms(
                    target,
                    Some(DepositTerms {
                        rates: vec![RatePeriod {
                            from: "2026-01-01".into(),
                            rate: amount(rate),
                        }],
                        capitalization: cap,
                        opened_at: None,
                        term_months: None,
                        auto_accrue: false,
                        pds: None,
                    }),
                )
                .unwrap();
        }
        for (month, sum) in [("01", "1000"), ("02", "2000"), ("03", "3000")] {
            let ts = format!("2026-{month}-05T10:00:00Z");
            vault
                .add_transaction(dated(income(&card, sum, "salary"), &ts))
                .unwrap();
        }
        vault
            .add_transaction(dated(
                usd_transfer(&card, &deposit, "1500"),
                "2026-03-06T10:00:00Z",
            ))
            .unwrap();
        vault
            .add_transaction(dated(
                usd_transfer(&card, &small, "500"),
                "2026-03-06T10:00:00Z",
            ))
            .unwrap();

        let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
        let source = ScenarioSource {
            scope: StoreScope {
                store_ids: vec![deposit.clone(), small],
                include_children: false,
            },
            income_months: 3,
            income_pct: amount("10"),
        };
        let prefill = vault.scenario_prefill(&source, "usd", now, "UTC").unwrap();
        assert_eq!(prefill.initial, amount("2000"));
        // The larger deposit gives the rate.
        assert_eq!(prefill.rate_store_id, Some(deposit));
        assert_eq!(prefill.rate_pct, Some(amount("16")));
        assert_eq!(prefill.capitalization, Some(CapFreq::Daily));
        assert!(!prefill.pays_out);
        assert_eq!(prefill.income_avg, Some(amount("2000")));
        assert_eq!(prefill.income_months_used, 3);
        assert_eq!(prefill.monthly_contribution, Some(amount("200")));
        assert_eq!(prefill.prefilled_at, "2026-04-10T10:00:00Z");
    }

    #[test]
    fn prefill_without_history_or_deposits_gives_only_the_value() {
        let (_dir, mut vault) = empty_vault();
        let source = ScenarioSource {
            scope: StoreScope::default(),
            income_months: 12,
            income_pct: amount("10"),
        };
        let prefill = vault
            .scenario_prefill(
                &source,
                "usd",
                unix_secs("2026-04-10T10:00:00Z").unwrap(),
                "UTC",
            )
            .unwrap();
        assert_eq!(prefill.initial, Amount::ZERO);
        assert_eq!(prefill.rate_pct, None);
        assert_eq!(prefill.monthly_contribution, None);
        assert_eq!(prefill.income_months_used, 0);
        let mut bad = source.clone();
        bad.income_pct = amount("101");
        assert!(vault.scenario_prefill(&bad, "usd", 0, "UTC").is_err());
        bad.income_pct = amount("10");
        bad.scope.store_ids = vec!["gone".into()];
        assert!(vault.scenario_prefill(&bad, "usd", 0, "UTC").is_err());
        let _ = &mut vault;
    }

    #[test]
    fn a_goal_shows_what_its_scenario_planned_by_now() {
        let (_dir, mut vault) = empty_vault();
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let scenario = vault.save_calc_scenario(draft("Flat")).unwrap();
        let mut goal = vault
            .goal_from_scenario(
                &scenario.id,
                unix_secs("2026-03-10T10:00:00Z").unwrap(),
                "UTC",
            )
            .unwrap();
        goal.scope.store_ids = vec![card];
        let goal = vault.create_goal(goal.into()).unwrap();
        let groups = vault.relations(&EntityRef::new(EntityKind::Scenario, &scenario.id));
        assert_eq!(groups[0].kind, RelationKind::Goals);
        assert_eq!(groups[0].items[0].target.id, goal.id);
        let back = vault.relations(&EntityRef::new(EntityKind::Goal, &goal.id));
        assert!(
            back.iter()
                .any(|group| group.kind == RelationKind::Scenarios)
        );
        let now = unix_secs("2026-05-20T10:00:00Z").unwrap();
        let progress = vault.goal_progress(now, "UTC");
        let expected = planned_balance(&scenario.input, 2).unwrap();
        assert_eq!(progress[0].planned_now, Some(expected.round(2)));
    }

    #[test]
    fn planned_balance_follows_the_simulation_and_stops_at_the_end() {
        let plan = input("1000", "100", 3);
        let result = invest::simulate(&plan).unwrap();
        let months: Vec<_> = result.years.iter().flat_map(|year| &year.months).collect();
        assert_eq!(planned_balance(&plan, 0).unwrap(), amount("1000"));
        assert_eq!(planned_balance(&plan, 2).unwrap(), months[1].balance);
        assert_eq!(planned_balance(&plan, 40).unwrap(), result.final_nominal);
    }
}
