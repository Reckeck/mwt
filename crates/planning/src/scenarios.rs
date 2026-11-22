//! Saved investment calculations (plan A): the rules of CRUD and "From your data".
//! The scenarios that write them, and the goal a scenario makes, live in
//! `usecases::planning::scenarios`.

use mwt_base::Amount;
use mwt_base::calc::invest::{self, CapFreq, ContributionStage, Freq, InvestInput};
use mwt_base::calendar::{self, Calendar};
use mwt_base::error::{Error, Result};
use mwt_model::Capitalization;
use mwt_model::VaultDoc;
use mwt_model::{CalcScenario, ScenarioSource};
use mwt_money::books::Books;

use super::budget::income_basis;
use super::goals::{resolve_scope, store_values_usd};

/// Periods "From your data" can average income over.
pub const INCOME_MONTHS: [u8; 3] = [3, 6, 12];
/// Importance of a goal made from a scenario, the middle of 1–5.
pub const SCENARIO_GOAL_PRIORITY: u8 = 3;

/// What the user saves: `id = None` makes a new scenario.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CalcScenarioDraft {
    pub id: Option<String>,
    pub name: String,
    pub input: InvestInput,
    pub source: Option<ScenarioSource>,
    pub prefilled_at: Option<String>,
}

/// A deleted scenario and the goals it was unlinked from: enough to undo.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CalcScenarioDeleted {
    pub scenario: CalcScenario,
    pub goal_ids: Vec<String>,
}

/// Numbers "From your data" puts into the calculator, with where they came from.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// The income is the hand-entered one: history is shorter than three
    /// finished months.
    pub income_manual: bool,
    pub prefilled_at: String,
}

/// A stage's contribution spread over a month.
pub fn per_month(stage: &ContributionStage) -> Amount {
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

/// Saved scenarios, the last changed first.
pub fn calc_scenarios(doc: &VaultDoc) -> Vec<CalcScenario> {
    let mut out = doc.calc_scenarios.clone();
    out.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    out
}

pub fn find_calc_scenario<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc CalcScenario> {
    doc.calc_scenarios.iter().find(|scenario| scenario.id == id)
}

pub fn check_scenario_source(doc: &VaultDoc, source: &ScenarioSource) -> Result<()> {
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
        .find(|id| !doc.stores.iter().any(|store| &store.id == *id))
    {
        Some(id) => Err(Error::Missing(format!("store {id}"))),
        None => Ok(()),
    }
}

/// The scenario a save leaves behind: the edited one, or a new one named
/// `fresh_id`. Nothing is written; `now` stamps the change.
pub fn plan_save_calc_scenario(
    doc: &VaultDoc,
    draft: CalcScenarioDraft,
    fresh_id: String,
    now: &str,
) -> Result<CalcScenario> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err(Error::InvalidInput("name the scenario".into()));
    }
    invest::simulate(&draft.input)?;
    if let Some(source) = &draft.source {
        check_scenario_source(doc, source)?;
    }
    Ok(match draft.id.as_deref() {
        Some(id) => {
            let mut scenario = find_calc_scenario(doc, id)
                .cloned()
                .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
            scenario.name = name;
            scenario.input = draft.input;
            scenario.source = draft.source;
            scenario.prefilled_at = draft.prefilled_at;
            scenario.updated_at = now.to_string();
            scenario
        }
        None => CalcScenario {
            id: fresh_id,
            name,
            input: draft.input,
            source: draft.source,
            prefilled_at: draft.prefilled_at,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        },
    })
}

/// What deleting a scenario takes with it: the scenario and the goals made
/// from it, which stay and stop following it.
pub fn plan_delete_calc_scenario(doc: &VaultDoc, id: &str) -> Result<CalcScenarioDeleted> {
    let scenario = find_calc_scenario(doc, id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("scenario {id}")))?;
    let goal_ids = doc
        .goals
        .iter()
        .filter(|goal| goal.scenario_id.as_deref() == Some(id))
        .map(|goal| goal.id.clone())
        .collect();
    Ok(CalcScenarioDeleted { scenario, goal_ids })
}

/// The goals a restored scenario is linked back to: those of the deleted
/// list that still exist and follow no other scenario now.
pub fn plan_restore_calc_scenario(
    doc: &VaultDoc,
    deleted: &CalcScenarioDeleted,
) -> Result<Vec<String>> {
    let id = &deleted.scenario.id;
    if find_calc_scenario(doc, id).is_some() {
        return Err(Error::InvalidInput(format!("scenario {id} already exists")));
    }
    Ok(doc
        .goals
        .iter()
        .filter(|goal| deleted.goal_ids.contains(&goal.id) && goal.scenario_id.is_none())
        .map(|goal| goal.id.clone())
        .collect())
}

/// Start amount, rate and contribution from the user's data (plan A,
/// decisions 7 and 9). Nothing is saved.
pub fn scenario_prefill(
    books: &Books<'_>,
    source: &ScenarioSource,
    currency: &str,
    now: i64,
    zone: &str,
) -> Result<ScenarioPrefill> {
    let doc = books.doc();
    check_scenario_source(doc, source)?;
    let cal = Calendar::new(zone);
    let book = books.rate_book();
    let values = store_values_usd(books);
    let scope = resolve_scope(books, &source.scope);
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
            let store = doc.stores.iter().find(|store| &store.id == id)?;
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

    let average = income_basis(books, source.income_months as usize, now, zone);
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
        income_manual: average.as_ref().is_some_and(|avg| avg.months == 0),
        income_months_used: average.map_or(0, |avg| avg.months),
        prefilled_at: calendar::iso(now),
    })
}
