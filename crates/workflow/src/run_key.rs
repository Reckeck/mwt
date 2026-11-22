//! The run key: a name for one occasion a job runs, the same on every device.
//!
//! A schedule moment, an operation a trigger reacts to and a wait that wakes
//! may all be met by more than one device. Each of them builds the same key
//! from the same facts, records it in what the run posts, and so a second run
//! of a known key is either not started or merged away after a sync.

pub use mwt_model::RunKey;

/// The id of the wait that `node_id` makes in the run named `run_key`: two
/// devices that start the same run make one wait record.
pub fn wait_id(run_key: &str, node_id: &str) -> String {
    format!("{run_key}#{node_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_ids_join_the_run_key_and_the_node() {
        assert_eq!(wait_id("wf:a:b:c", "node"), "wf:a:b:c#node");
    }
}
