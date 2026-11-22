//! The crate table of `crates/core/src` as data: modules form the target crates of the crate split
//! (spec crate-split, D8 and "Interfaces"), and an explicit list says which crate may name which.
//! An import `A -> B` is legal when both sit in the same crate or B's crate is in A's `may_use` list.

/// One target crate: the top-level modules it takes and the crates it may depend on.
pub struct Group {
    pub name: &'static str,
    pub modules: &'static [&'static str],
    pub may_use: &'static [&'static str],
}

/// The table. `lib` stands for the root items of `lib.rs` (`VAULT_*`). `kernel` is one module here and
/// counts as vault (its `Books` part moves to money in T13: that edge is an allow entry until then).
/// `macros` has no module of its own (a new proc-macro crate), so it never appears in this table.
pub const GROUPS: &[Group] = &[
    Group {
        name: "base",
        modules: &[
            "amount",
            "error",
            "time",
            "calendar",
            "calc",
            "limits",
            "routes",
            "secret",
            "utilities_formula",
            "lib",
        ],
        may_use: &[],
    },
    Group {
        name: "crypto",
        modules: &["crypto"],
        may_use: &["base"],
    },
    Group {
        name: "model",
        modules: &["model", "model_tests"],
        may_use: &["base", "crypto"],
    },
    Group {
        name: "net",
        modules: &[
            "pricing",
            "chain",
            "exchange",
            "datasource",
            "market",
            "updates",
            "external_url",
        ],
        may_use: &["base", "model"],
    },
    Group {
        name: "vault",
        modules: &["storage", "vault_io", "jsonstore", "kernel", "app_dirs"],
        may_use: &["base", "crypto", "model"],
    },
    Group {
        name: "sync",
        modules: &["sync"],
        may_use: &["base", "crypto", "model"],
    },
    // Test support: compiled only under `cfg(test)` or the `testkit` feature.
    Group {
        name: "testkit",
        modules: &["testkit"],
        may_use: &["base", "model"],
    },
    // `txinfo` is a re-export of `money::txinfo` until Ф7 removes the old path (core-strangler T27).
    Group {
        name: "money",
        modules: &["money", "txinfo"],
        may_use: &["base", "model"],
    },
    Group {
        name: "household",
        modules: &["household"],
        may_use: &["base", "model", "money"],
    },
    // Workflow sees only model and base (data through the `WorkflowHost` port); planning must not use it.
    Group {
        name: "workflow",
        modules: &["workflow"],
        may_use: &["base", "model"],
    },
    Group {
        name: "planning",
        modules: &["planning"],
        may_use: &["base", "model", "money", "household"],
    },
    Group {
        name: "reports",
        modules: &["reports"],
        may_use: &["base", "model", "money", "household", "planning"],
    },
    Group {
        name: "cryptotrack",
        modules: &["cryptotrack"],
        may_use: &["base", "model", "money"],
    },
    // The IPC contract crate `mwt-contract` sits on top of the domain crates (spec fact 2); nothing below may
    // name it. No module of `crates/core/src` belongs to it any more (`ipc_contract.rs` is the facade shim), so
    // the group only records which crates the contract may depend on (checked by `cargo tree`, T24).
    Group {
        name: "contract",
        modules: &[],
        may_use: &[
            "base",
            "model",
            "money",
            "household",
            "workflow",
            "planning",
            "reports",
            "cryptotrack",
            "sync",
            "crypto",
            "vault",
            "net",
        ],
    },
    // The facade (`trade-core`): scenarios, the service shim, sandbox and config; it uses every crate.
    Group {
        name: "facade",
        modules: &[
            "ipc_contract",
            "service",
            "usecases",
            "sandbox",
            "appconfig",
            "vault_facade_tests",
            "sync_facade_tests",
            "money_facade_tests",
            "household_facade_tests",
            "workflow_facade_tests",
            "planning_facade_tests",
            "reports_facade_tests",
            "crypto_facade_tests",
        ],
        may_use: &[
            "base",
            "crypto",
            "model",
            "net",
            "vault",
            "sync",
            "money",
            "household",
            "workflow",
            "planning",
            "reports",
            "cryptotrack",
            "contract",
        ],
    },
];

pub fn group_of(module: &str) -> Option<&'static Group> {
    GROUPS.iter().find(|group| group.modules.contains(&module))
}

/// Whether `from` may name `to`; a module outside the table is never legal.
pub fn is_legal(from: &str, to: &str) -> bool {
    match (group_of(from), group_of(to)) {
        (Some(user), Some(used)) => user.name == used.name || user.may_use.contains(&used.name),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AllowEntry, Edge, domain, evaluate};
    use super::*;

    /// The domain crates the contract may sit on top of.
    const DOMAIN: &[&str] = &[
        "money",
        "household",
        "workflow",
        "planning",
        "reports",
        "cryptotrack",
    ];

    #[test]
    fn every_allowed_crate_is_in_the_table() {
        for group in GROUPS {
            for used in group.may_use {
                assert!(GROUPS.iter().any(|other| other.name == *used), "{used}");
            }
        }
    }

    #[test]
    fn an_allowed_edge_is_legal() {
        assert!(is_legal("household", "money"));
        assert!(is_legal("planning", "household"));
        assert!(is_legal("household", "household"));
        assert!(is_legal("usecases", "ipc_contract"));
        assert!(is_legal("sync", "crypto"));
    }

    #[test]
    fn money_may_not_use_net() {
        assert!(!is_legal("money", "pricing"));
        assert!(!is_legal("money", "exchange"));
        assert!(!is_legal("money", "household"));
    }

    #[test]
    fn contract_sits_above_the_domain() {
        for domain_module in DOMAIN {
            assert!(is_legal("ipc_contract", domain_module), "{domain_module}");
            assert!(!is_legal(domain_module, "ipc_contract"), "{domain_module}");
        }
        let contract = GROUPS
            .iter()
            .find(|group| group.name == "contract")
            .unwrap();
        assert_eq!(
            contract.may_use,
            [
                "base",
                "model",
                "money",
                "household",
                "workflow",
                "planning",
                "reports",
                "cryptotrack",
                "sync",
                "crypto",
                "vault",
                "net"
            ]
        );
    }

    #[test]
    fn the_contract_shim_is_part_of_the_facade() {
        assert!(is_legal("ipc_contract", "appconfig"));
        assert!(is_legal("ipc_contract", "service"));
    }

    #[test]
    fn planning_may_not_use_workflow_and_domain_may_not_use_infra() {
        assert!(!is_legal("planning", "workflow"));
        assert!(!is_legal("cryptotrack", "exchange"));
        assert!(!is_legal("workflow", "kernel"));
    }

    #[test]
    fn nothing_below_the_facade_names_it_or_testkit() {
        assert!(!is_legal("reports", "usecases"));
        assert!(!is_legal("usecases", "testkit"));
        assert!(is_legal("testkit", "model"));
    }

    #[test]
    fn a_stale_allow_entry_fails() {
        let allow = [AllowEntry {
            from: "money/x.rs".to_string(),
            to: "pricing".to_string(),
            until: "T13".to_string(),
            reason: "test".to_string(),
        }];
        let allowed_edge = Edge {
            from_file: "money/x.rs".to_string(),
            line: 1,
            from_module: "money".to_string(),
            to_module: "model".to_string(),
        };
        let report = evaluate(&[allowed_edge], &allow, domain::Findings::default());
        assert_eq!(report.stale.len(), 1);
        assert_eq!(report.failures(), 1);
    }
}
