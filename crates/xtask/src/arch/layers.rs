//! The layer table of `crates/core/src` as data: modules form groups, groups form layers.
//! An import `A -> B` is legal when B is in a lower layer or in the same group.

/// One group of modules and the layer it sits in.
pub struct Group {
    pub name: &'static str,
    pub layer: u8,
    pub modules: &'static [&'static str],
}

/// The table. `lib` stands for the root items of `lib.rs` (`VAULT_*`); `ipc_contract` is a use case for now.
pub const GROUPS: &[Group] = &[
    Group {
        name: "base",
        layer: 0,
        modules: &[
            "amount",
            "error",
            "time",
            "calendar",
            "calc",
            "limits",
            "app_dirs",
            "routes",
            "external_url",
            "secret",
            "lib",
        ],
    },
    Group {
        name: "crypto",
        layer: 1,
        modules: &["crypto"],
    },
    Group {
        name: "model",
        layer: 2,
        modules: &["model"],
    },
    Group {
        name: "vault",
        layer: 3,
        modules: &["storage", "vault_io", "jsonstore", "kernel"],
    },
    Group {
        name: "sync",
        layer: 3,
        modules: &["sync"],
    },
    Group {
        name: "net",
        layer: 3,
        modules: &["pricing", "exchange", "datasource", "market", "updates"],
    },
    Group {
        name: "money",
        layer: 4,
        modules: &["money"],
    },
    Group {
        name: "planning",
        layer: 4,
        modules: &["planning"],
    },
    Group {
        name: "cryptotrack",
        layer: 4,
        modules: &["cryptotrack"],
    },
    Group {
        name: "workflow",
        layer: 4,
        modules: &["workflow"],
    },
    Group {
        name: "txinfo",
        layer: 4,
        modules: &["txinfo"],
    },
    Group {
        name: "utilities_formula",
        layer: 4,
        modules: &["utilities_formula"],
    },
    // Scenarios and the service shim sit above the pure areas they call (core-strangler OD1).
    Group {
        name: "usecases",
        layer: 6,
        modules: &[
            "service",
            "sandbox",
            "appconfig",
            "ipc_contract",
            "usecases",
        ],
    },
    Group {
        name: "household",
        layer: 5,
        modules: &["household"],
    },
    Group {
        name: "reports",
        layer: 5,
        modules: &["reports"],
    },
    // Test support on top of everything: compiled only under `cfg(test)` or the `testkit` feature.
    Group {
        name: "testkit",
        layer: 7,
        modules: &["testkit"],
    },
];

pub fn group_of(module: &str) -> Option<&'static Group> {
    GROUPS.iter().find(|group| group.modules.contains(&module))
}

/// Whether `from` may name `to`; a module outside the table is never legal.
pub fn is_legal(from: &str, to: &str) -> bool {
    match (group_of(from), group_of(to)) {
        (Some(user), Some(used)) => used.layer < user.layer || used.name == user.name,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn household_and_reports_are_their_own_groups() {
        assert_eq!(
            group_of("household").map(|group| group.name),
            Some("household")
        );
        assert_eq!(group_of("reports").map(|group| group.name), Some("reports"));
    }

    #[test]
    fn household_may_not_use_service() {
        assert!(!is_legal("household", "service"));
        assert!(!is_legal("reports", "service"));
    }

    #[test]
    fn scenarios_and_the_service_shim_may_use_household_and_reports() {
        assert!(is_legal("usecases", "household"));
        assert!(is_legal("service", "household"));
        assert!(is_legal("usecases", "reports"));
    }

    #[test]
    fn household_may_use_lower_layers_and_itself() {
        assert!(is_legal("household", "money"));
        assert!(is_legal("household", "household"));
    }
}
