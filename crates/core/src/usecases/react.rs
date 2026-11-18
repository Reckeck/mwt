//! Reactions: what a scenario does to the vault after its own commit because
//! of an event it announced. The bus keeps the observers (a subscriber gets
//! the event only, never the vault); a reaction gets `&mut UnlockedVault` and
//! makes its own write, a second transaction like the second `persist()` of
//! the direct calls today (spec core-strangler, Open question 2).

use crate::error::Error;
use crate::kernel::events::DomainEvent;
use crate::service::UnlockedVault;

use super::workflow::reactions;

/// One reaction. It ignores events it does not know and returns `Ok(())`.
pub type Reaction = fn(&mut UnlockedVault, &DomainEvent) -> Result<(), Error>;

/// A reaction that failed. The commit that announced the event stays.
#[derive(Debug)]
pub struct ReactionFailure {
    pub event: DomainEvent,
    pub error: Error,
}

/// The registered reactions, in the order they run.
pub const REGISTERED: &[Reaction] = &[
    reactions::on_automation_deleted,
    reactions::on_automation_retried,
    reactions::on_goal_paused,
    reactions::on_goal_resumed,
];

/// Run the registered reactions for `events`, after the commit that made them.
pub fn react(vault: &mut UnlockedVault, events: Vec<DomainEvent>) -> Vec<ReactionFailure> {
    run(vault, events, REGISTERED)
}

/// Run `reactions` for each event, in event order and then registration order.
/// A failure is collected and the rest still run: the first commit is neither
/// undone nor hidden by a later one failing.
pub fn run(
    vault: &mut UnlockedVault,
    events: Vec<DomainEvent>,
    reactions: &[Reaction],
) -> Vec<ReactionFailure> {
    let mut failures = Vec::new();
    for event in events {
        for reaction in reactions {
            if let Err(error) = reaction(vault, &event) {
                failures.push(ReactionFailure {
                    event: event.clone(),
                    error,
                });
            }
        }
    }
    failures
}
