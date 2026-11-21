//! Properties of the compound-interest engine.

use proptest::prelude::*;

use super::invest::{
    CapFreq, ContributionStage, Freq, InvestInput, SolveUnknown, Timing, Withdrawal,
    safe_withdrawal, simulate, solve,
};
use crate::Amount;

fn money(max_units: i64) -> impl Strategy<Value = Amount> {
    (0..=max_units * 100).prop_map(|cents| Amount::new(rust_decimal::Decimal::new(cents, 2)))
}

fn cap_freq() -> impl Strategy<Value = CapFreq> {
    prop_oneof![
        Just(CapFreq::Daily),
        Just(CapFreq::Monthly),
        Just(CapFreq::Quarterly),
        Just(CapFreq::Yearly),
        Just(CapFreq::AtEnd),
    ]
}

fn freq() -> impl Strategy<Value = Freq> {
    prop_oneof![
        Just(Freq::Monthly),
        Just(Freq::Quarterly),
        Just(Freq::Yearly)
    ]
}

fn timing() -> impl Strategy<Value = Timing> {
    prop_oneof![Just(Timing::Start), Just(Timing::End)]
}

fn stage() -> impl Strategy<Value = ContributionStage> {
    (1..=120u32, money(10_000), freq(), timing()).prop_map(|(from_month, amount, freq, timing)| {
        ContributionStage {
            from_month,
            amount,
            freq,
            timing,
        }
    })
}

fn withdrawal() -> impl Strategy<Value = Withdrawal> {
    (money(10_000), freq(), 1..=120u32, any::<bool>()).prop_map(
        |(amount, freq, from_month, index_to_inflation)| Withdrawal {
            amount,
            freq,
            from_month,
            index_to_inflation,
        },
    )
}

/// A valid input with a non-negative rate and no withdrawal.
fn growing_input() -> impl Strategy<Value = InvestInput> {
    (
        (money(1_000_000), 0..=3000u32, 0..=240u32, cap_freq()),
        proptest::collection::vec(stage(), 0..=3),
        (0..=1500u32, 0..=1500u32, 0..=100u32, any::<bool>()),
    )
        .prop_map(
            |(
                (initial, rate, term_months, capitalization),
                stages,
                (indexation, inflation, tax, at_end),
            )| {
                let percent = |hundredths: u32| {
                    Amount::new(rust_decimal::Decimal::new(i64::from(hundredths), 2))
                };
                let mut input = InvestInput {
                    start: "2025-01".into(),
                    initial,
                    rate_pct: percent(rate),
                    term_months,
                    capitalization,
                    stages,
                    indexation_pct: percent(indexation),
                    inflation_pct: percent(inflation),
                    ..InvestInput::default()
                };
                input.tax.pct = Amount::new(rust_decimal::Decimal::from(tax));
                if at_end {
                    input.tax.mode = super::invest::TaxMode::AtEnd;
                }
                input
            },
        )
}

fn with_withdrawal() -> impl Strategy<Value = InvestInput> {
    (growing_input(), withdrawal()).prop_map(|(input, withdrawal)| InvestInput {
        withdrawal: Some(withdrawal),
        ..input
    })
}

fn final_nominal(input: &InvestInput) -> Option<Amount> {
    simulate(input).ok().map(|result| result.final_nominal)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn simulate_never_panics_inside_the_bounds(input in with_withdrawal()) {
        prop_assert!(simulate(&input).is_ok());
    }

    #[test]
    fn a_larger_contribution_never_lowers_the_balance(
        input in growing_input(),
        extra in money(1_000),
    ) {
        let mut larger = input.clone();
        for stage in &mut larger.stages {
            stage.amount += extra;
        }
        if let (Some(base), Some(more)) = (final_nominal(&input), final_nominal(&larger)) {
            prop_assert!(more >= base, "{more} < {base}");
        }
    }

    #[test]
    fn a_longer_term_never_lowers_the_balance(input in growing_input(), more in 1..=24u32) {
        let longer = InvestInput { term_months: input.term_months + more, ..input.clone() };
        if let (Some(base), Some(longer)) = (final_nominal(&input), final_nominal(&longer)) {
            prop_assert!(longer >= base, "{longer} < {base}");
        }
    }

    #[test]
    fn solved_contribution_reaches_the_target(input in growing_input(), target in money(10_000_000)) {
        if let Ok(solved) = solve(&input, target, SolveUnknown::Contribution) && solved.reachable {
            let reached = final_nominal(&solved.input).expect("a solved input simulates");
            prop_assert!(reached + Amount::new(rust_decimal::Decimal::new(1, 2)) >= target,
                "{reached} vs {target}");
        }
    }

    #[test]
    fn solved_term_reaches_the_target(input in growing_input(), target in money(10_000_000)) {
        if let Ok(solved) = solve(&input, target, SolveUnknown::Term) && solved.reachable {
            let reached = final_nominal(&solved.input).expect("a solved input simulates");
            prop_assert!(reached + Amount::new(rust_decimal::Decimal::new(1, 2)) >= target,
                "{reached} vs {target}");
        }
    }

    #[test]
    fn safe_withdrawal_is_none_or_non_negative(input in with_withdrawal()) {
        if let Ok(Some(amount)) = safe_withdrawal(&input) {
            prop_assert!(!amount.is_negative());
        }
    }
}
