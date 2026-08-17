//! Same-execution single-flight coordination for liminal agent redeliveries.

use aion_core::WorkflowId;
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

/// Stable identity shared by every delivery attempt for one logical activity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AgentExecutionKey {
    workflow_id: WorkflowId,
    ordinal: u64,
}

impl AgentExecutionKey {
    pub(crate) const fn new(workflow_id: WorkflowId, ordinal: u64) -> Self {
        Self {
            workflow_id,
            ordinal,
        }
    }
}

/// Followers waiting for the leader execution's single terminal outcome.
pub(crate) struct AgentFlights<T> {
    followers: DashMap<AgentExecutionKey, Vec<T>>,
}

impl<T> Default for AgentFlights<T> {
    fn default() -> Self {
        Self {
            followers: DashMap::new(),
        }
    }
}

impl<T> AgentFlights<T> {
    /// Join an existing execution or reserve the key and return its leader value.
    pub(crate) fn join_or_lead(&self, key: AgentExecutionKey, delivery: T) -> Result<(), T> {
        match self.followers.entry(key) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(delivery);
                Ok(())
            }
            Entry::Vacant(entry) => {
                entry.insert(Vec::new());
                Err(delivery)
            }
        }
    }

    /// End the execution and return every redelivery awaiting its outcome.
    pub(crate) fn finish(&self, key: &AgentExecutionKey) -> Vec<T> {
        self.followers
            .remove(key)
            .map_or_else(Vec::new, |(_, followers)| followers)
    }
}

#[cfg(test)]
mod tests {
    use aion_core::WorkflowId;

    use super::{AgentExecutionKey, AgentFlights};

    #[test]
    fn same_execution_joins_and_foreign_execution_leads() {
        let flights = AgentFlights::default();
        let workflow = WorkflowId::new_v4();
        let own = AgentExecutionKey::new(workflow.clone(), 1);
        let foreign = AgentExecutionKey::new(workflow, 2);

        assert_eq!(flights.join_or_lead(own.clone(), "leader"), Err("leader"));
        assert_eq!(flights.join_or_lead(own.clone(), "redelivery"), Ok(()));
        assert_eq!(
            flights.join_or_lead(foreign.clone(), "foreign leader"),
            Err("foreign leader")
        );
        assert_eq!(flights.finish(&own), vec!["redelivery"]);
        assert_eq!(flights.finish(&foreign), Vec::<&str>::new());
    }
}
