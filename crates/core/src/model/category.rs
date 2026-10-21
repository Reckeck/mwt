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
    /// Whether everyday spending here is a must (plan C1). `None` takes the
    /// default by key ([`DEFAULT_ESSENTIAL`]); only `Daily` reads it, `Fixed`
    /// is always essential.
    #[serde(default)]
    pub essential: Option<bool>,
    /// Whether income here comes from capital rather than work (plan D).
    /// `None` takes the default by key ([`DEFAULT_PASSIVE`]); only `Income`
    /// reads it.
    #[serde(default)]
    pub passive: Option<bool>,
}

impl Category {
    pub fn builtin(key: &str, group: CategoryGroup) -> Self {
        Self {
            key: key.to_string(),
            name: None,
            group,
            archived: false,
            essential: None,
            passive: None,
        }
    }

    /// Mandatory spending: every `Fixed` category, and `Daily` ones flagged
    /// by the user or, without a flag, by [`DEFAULT_ESSENTIAL`].
    pub fn is_essential(&self) -> bool {
        match self.group {
            CategoryGroup::Fixed => true,
            CategoryGroup::Daily => self
                .essential
                .unwrap_or_else(|| DEFAULT_ESSENTIAL.contains(&self.key.as_str())),
            _ => false,
        }
    }

    /// Income from capital: `Income` categories flagged by the user or, without
    /// a flag, by [`DEFAULT_PASSIVE`].
    pub fn is_passive(&self) -> bool {
        self.group == CategoryGroup::Income
            && self.passive.unwrap_or_else(|| is_passive_key(&self.key))
    }
}

/// Income that counts as passive until the user says otherwise (plan
/// decision 22): returns on capital, not cashback or gifts. `earn` is what
/// the Bybit Earn import books.
pub const DEFAULT_PASSIVE: &[&str] = &[
    "deposit_interest",
    "earn",
    "dividends",
    "coupons",
    "staking",
    "rent_income",
];

/// The default passive flag for a key, also for keys with no category record.
pub fn is_passive_key(key: &str) -> bool {
    DEFAULT_PASSIVE.contains(&key)
}

/// Everyday categories that count as mandatory until the user says otherwise
/// (plan decision 18).
pub const DEFAULT_ESSENTIAL: &[&str] = &[
    "groceries",
    "transport",
    "fuel",
    "medicine",
    "pharmacy",
    "household_chemicals",
    "pets",
    "therapy",
    "education",
    "repair",
    "family_help",
];

/// Built-in categories added after vaults already existed, by the revision
/// that brought them. An older vault receives each batch once when opened.
pub const CATEGORY_ADDITIONS: &[(u32, &[&str])] =
    &[(1, &["dividends", "coupons", "staking", "rent_income"])];

/// The latest revision in [`CATEGORY_ADDITIONS`]: what a new vault starts at.
pub const CATEGORIES_REV: u32 = 1;

/// Marks every operation brought in from an exchange, so they can be found
/// and told apart from the ones entered by hand.
pub const EXCHANGE_IMPORT_CATEGORY: &str = "exchange_import";

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
                "dividends",
                "coupons",
                "staking",
                "rent_income",
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
            &[
                "trading",
                "fee",
                "transfer",
                "exchange",
                "opening_balance",
                EXCHANGE_IMPORT_CATEGORY,
            ],
        ),
    ];
    groups
        .iter()
        .flat_map(|(group, keys)| keys.iter().map(|key| Category::builtin(key, *group)))
        .collect()
}
