//! Names of the UI routes the backend links to (notices, the tray). The UI
//! router owns the list; `tests/checks/routes.check.ts` checks each name here
//! against `src/app/providers/router`, so a renamed route fails `mise run check`.

macro_rules! routes {
    ($($const_name:ident = $value:literal,)+) => {
        $(
            #[doc = concat!("The route `", $value, "`.")]
            pub const $const_name: &str = $value;
        )+
        /// Every route name above.
        pub const ALL: &[&str] = &[$($const_name),+];
    };
}

routes! {
    DASHBOARD = "dashboard",
    STORES = "stores",
    TRANSACTIONS = "transactions",
    TRANSACTIONS_CALENDAR = "transactions-calendar",
    MONEY = "money",
    MONEY_HEALTH = "money-health",
    AUTOMATIONS = "automations",
    AUTOMATIONS_JOURNAL = "automations-journal",
    AUTOMATIONS_ALERTS = "automations-alerts",
    AUTOMATIONS_REMINDERS = "automations-reminders",
    AUTOMATIONS_SOURCES = "automations-sources",
    SETTINGS = "settings",
    WALLET = "wallet",
    WORKFLOW_EDIT = "workflow-edit",
}

/// The inflation section of the settings window (`?settings=<group>&section=<id>`).
pub const SETTINGS_INFLATION: &str = "/?settings=accounting&section=inflation";

/// Where a notice leads: a route name, a `name:id` pair an `open:kind:id` card or an absolute path.
pub fn is_known_link(link: &str) -> bool {
    link.starts_with('/')
        || link.starts_with("open:")
        || ALL.contains(&link.split(':').next().unwrap_or(link))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_kebab_case() {
        let mut seen = std::collections::HashSet::new();
        for name in ALL {
            assert!(seen.insert(*name), "{name} is listed twice");
            assert!(name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-'));
        }
    }

    #[test]
    fn links_are_known() {
        assert!(is_known_link(AUTOMATIONS_ALERTS));
        assert!(is_known_link(SETTINGS_INFLATION));
        assert!(is_known_link("open:goal:abc"));
        assert!(!is_known_link("planning-alerts"));
    }
}
