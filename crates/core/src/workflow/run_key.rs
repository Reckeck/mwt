//! The run key: a name for one occasion a job runs, the same on every device.
//!
//! A schedule moment, an operation a trigger reacts to and a wait that wakes
//! may all be met by more than one device. Each of them builds the same key
//! from the same facts, records it in what the run posts, and so a second run
//! of a known key is either not started or merged away after a sync.

/// One occasion a workflow job runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKey<'part> {
    /// A moment of a schedule trigger: the UTC instant (RFC3339) that the
    /// group zone picks for it.
    Schedule {
        workflow_id: &'part str,
        trigger_id: &'part str,
        moment: &'part str,
    },
    /// An operation an «operation added» trigger reacts to.
    Transaction {
        workflow_id: &'part str,
        trigger_id: &'part str,
        tx_id: &'part str,
    },
    /// A sleeping run that goes on from `port` of its wait.
    Wait {
        wait_id: &'part str,
        port: &'part str,
    },
}

impl RunKey<'_> {
    /// The key as stored in an operation's origin.
    pub fn build(&self) -> String {
        match self {
            Self::Schedule {
                workflow_id,
                trigger_id,
                moment,
            } => format!("wf:{workflow_id}:{trigger_id}:{moment}"),
            Self::Transaction {
                workflow_id,
                trigger_id,
                tx_id,
            } => format!("tx:{workflow_id}:{trigger_id}:{tx_id}"),
            Self::Wait { wait_id, port } => format!("wait:{wait_id}:{port}"),
        }
    }
}

/// The id of the wait that `node_id` makes in the run named `run_key`: two
/// devices that start the same run make one wait record.
pub fn wait_id(run_key: &str, node_id: &str) -> String {
    format!("{run_key}#{node_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_built_from_facts_only() {
        let key = RunKey::Schedule {
            workflow_id: "wf",
            trigger_id: "s",
            moment: "2026-11-01T09:00:00Z",
        };
        assert_eq!(key.build(), "wf:wf:s:2026-11-01T09:00:00Z");
        assert_eq!(
            RunKey::Wait {
                wait_id: "w",
                port: "out"
            }
            .build(),
            "wait:w:out"
        );
        assert_eq!(wait_id("wf:a:b:c", "node"), "wf:a:b:c#node");
    }
}
