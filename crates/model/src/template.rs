//! Operation templates and the default rules they and categories carry
//! (spec op-period 5.0-5.2): the period rule «за что» and the income rule
//! «Куда раскладывать». Data only; resolution lives in `mwt-base::calendar`
//! (period) and `mwt-planning` (distribution).

use serde::{Deserialize, Serialize};

use crate::store::AssetRef;
use crate::transaction::{DistKind, DistTarget, PeriodKind};
use mwt_base::Amount;

/// Which half of a month a custom period rule picks (spec D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum RuleHalf {
    First,
    Second,
}

/// A default «за что» for an operation, resolved from its money date (spec D1, D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PeriodRule {
    PrevMonth,
    /// Days 16..last of the previous month.
    PrevHalfSecond,
    /// Days 1..15 of the money date's month.
    ThisHalfFirst,
    ThisMonth,
    NextMonth,
    Custom {
        /// `month`, `half`, `quarter` or `year`; `range` is not a rule kind.
        kind: PeriodKind,
        /// Units from the unit of the money date (half-months for `half`).
        offset: i8,
        /// For `half`: which half; none keeps the half the date is in.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        half: Option<RuleHalf>,
    },
}

/// What to do when the payout differs from the expected amount (spec D9).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum OnOtherAmount {
    /// Sum rows keep their amount, percent rows follow the payout, the remainder absorbs the rest.
    #[default]
    ShrinkRemainder,
    /// Every sum row is scaled by actual / expected.
    ShrinkAll,
    /// The core does not distribute; the automation asks first.
    Ask,
}

/// One row of a distribution rule: what is asked and where it goes (spec S6, D7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DistributionRow {
    pub kind: DistKind,
    pub target: DistTarget,
}

/// «Куда раскладывать»: the rows applied to a regular income (spec D6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DistributionRule {
    pub rows: Vec<DistributionRow>,
    #[serde(default)]
    pub on_other_amount: OnOtherAmount,
    /// Least balance left on the card after the rows (spec D10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub keep_on_card_min: Option<Amount>,
}

/// The kind of operation a template makes: topup or payment only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum TemplateKind {
    Income,
    Expense,
}

/// A saved operation shape with its period rule and, for an income, its
/// distribution rule (spec 5.0). Store, asset and amount only prefill quick-add.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct OperationTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
    pub created_at: String,
    pub kind: TemplateKind,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub store_id: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub asset: Option<AssetRef>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub amount: Option<Amount>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub contact_id: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub period_rule: Option<PeriodRule>,
    /// Income only (spec T2).
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub distribution: Option<DistributionRule>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> OperationTemplate {
        OperationTemplate {
            id: "t1".into(),
            name: "Salary".into(),
            archived: false,
            created_at: "2026-11-24T00:00:00Z".into(),
            kind: TemplateKind::Income,
            store_id: Some("card".into()),
            asset: None,
            amount: Some(Amount::parse("100000").unwrap()),
            categories: vec!["salary".into()],
            comment: String::new(),
            contact_id: None,
            period_rule: Some(PeriodRule::Custom {
                kind: PeriodKind::Half,
                offset: -1,
                half: Some(RuleHalf::Second),
            }),
            distribution: Some(DistributionRule {
                rows: vec![DistributionRow {
                    kind: DistKind::Remainder,
                    target: DistTarget::None,
                }],
                on_other_amount: OnOtherAmount::ShrinkAll,
                keep_on_card_min: Some(Amount::parse("500").unwrap()),
            }),
        }
    }

    /// T1: a template survives JSON and MessagePack.
    #[test]
    fn t1_template_round_trips_in_json_and_messagepack() {
        let value = template();
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<OperationTemplate>(&json).unwrap(),
            value
        );
        let packed = rmp_serde::to_vec_named(&value).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<OperationTemplate>(&packed).unwrap(),
            value
        );
    }

    /// D1: the five presets and `custom` keep a plain snake_case form.
    #[test]
    fn d1_period_rule_forms_are_plain() {
        assert_eq!(
            serde_json::to_string(&PeriodRule::PrevHalfSecond).unwrap(),
            "\"prev_half_second\""
        );
        let custom = serde_json::to_string(&PeriodRule::Custom {
            kind: PeriodKind::Quarter,
            offset: -2,
            half: None,
        })
        .unwrap();
        assert_eq!(custom, r#"{"custom":{"kind":"quarter","offset":-2}}"#);
    }

    /// D9: a rule without `on_other_amount` reads as `shrink_remainder`.
    #[test]
    fn d9_default_on_other_amount_is_shrink_remainder() {
        let rule: DistributionRule = serde_json::from_str(r#"{"rows":[]}"#).unwrap();
        assert_eq!(rule.on_other_amount, OnOtherAmount::ShrinkRemainder);
        assert_eq!(rule.keep_on_card_min, None);
    }
}
