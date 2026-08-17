//! Same-execution single-flight coordination for liminal agent redeliveries.

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

        assert!(!flights.join_if_running(own.clone(), "leader"));
        assert!(flights.join_if_running(own.clone(), "redelivery"));
        assert!(!flights.join_if_running(foreign.clone(), "foreign leader"));
        assert_eq!(flights.finish(&own), vec!["redelivery"]);
        assert_eq!(flights.finish(&foreign), Vec::<&str>::new());
    }
}
