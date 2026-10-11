//! Spending and income categories, grouped the way the budget reads them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CategoryGroup {
    Income,
    /// Mandatory monthly costs: rent, utilities, loan payments.
    Fixed,
    /// Everyday spending the daily budget is about.
    #[default]
    Daily,
    Savings,
    /// Bookkeeping (trading, fees, transfers): outside budgets and month totals.
    Service,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Category {
    /// Stable key stored on transactions; never changes.
    pub key: String,
    /// User label; `None` means the built-in translation `categories.default.<key>`.
    pub name: Option<String>,
    pub group: CategoryGroup,
    /// Hidden from pickers; existing transactions keep it.
    pub archived: bool,
}

impl Category {
    pub fn builtin(key: &str, group: CategoryGroup) -> Self {
        Self {
            key: key.to_string(),
            name: None,
            group,
            archived: false,
        }
    }
}

/// Categories a new vault starts with (plan §7.1, after the T—Zh budget table).
pub fn default_categories() -> Vec<Category> {
    use CategoryGroup::*;
    let groups: [(CategoryGroup, &[&str]); 5] = [
        (
            Income,
            &[
                "salary",
                "advance",
                "bonus",
                "freelance",
                "side_job",
                "gift_received",
                "cashback",
                "deposit_interest",
                "income_other",
            ],
        ),
        (
            Fixed,
            &[
                "rent_mortgage",
                "utilities",
                "internet",
                "mobile",
                "loan_payment",
                "loan_interest",
                "installments",
                "taxes",
                "insurance",
                "subscriptions",
            ],
        ),
        (
            Daily,
            &[
                "groceries",
                "food_delivery",
                "transport",
                "taxi",
                "fuel",
                "medicine",
                "pharmacy",
                "therapy",
                "sport",
                "education",
                "clothing",
                "beauty",
                "household_chemicals",
                "pets",
                "family_help",
                "gifts",
                "repair",
                "household",
                "entertainment",
                "travel",
                "other",
            ],
        ),
        (Savings, &["savings"]),
        (
            Service,
            &["trading", "fee", "transfer", "exchange", "opening_balance"],
        ),
    ];
    groups
        .iter()
        .flat_map(|(group, keys)| keys.iter().map(|key| Category::builtin(key, *group)))
        .collect()
}
