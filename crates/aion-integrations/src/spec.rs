//! The neutral run identity handed to a harness at [`crate::AgentHarness::start`].

use aion_core::{ActivityId, Payload, RunId, WorkflowId};

/// The neutral identity + input for one agent run (one activity attempt).
///
/// Carries only what every harness needs to run an attempt: the
/// `(workflow, run, activity, attempt)` key, the activity-type name the engine dispatched, and
/// the input [`Payload`]. It names **no** harness-specific configuration — an adapter holds any
/// harness-specific settings itself (constructed when the [`crate::AgentHarness`] is built),
/// keeping this spec harness-blind.
///
/// # The run axis is what the adapters stamp onto every event
///
/// Each adapter stamps this identity onto every [`aion_core::ActivityEvent`] it emits, and the
/// transcript keyspace is keyed on the same quad. The run therefore has to arrive here from the
/// dispatch that started the attempt; an adapter has no other source for it and must never
/// synthesise one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRunSpec {
    /// The workflow this activity attempt belongs to.
    pub workflow_id: WorkflowId,
    /// The concrete run of that workflow this attempt belongs to. Required: a continue-as-new
    /// chain reuses one `workflow_id` while ordinals and attempts restart per generation.
    pub run_id: RunId,
    /// The activity within the workflow.
    pub activity_id: ActivityId,
    /// The attempt number of the activity being run.
    pub attempt: u32,
    /// The activity-type name the engine dispatched to this run (e.g. `"dev"`).
    ///
    /// Neutral run identity, not harness configuration: it is the same type name the workflow
    /// definition routed on, so an adapter can label or parameterise a run by what the engine
    /// asked for without the spec learning anything harness-specific.
    pub activity_type: String,
    /// The activity input handed to the agent.
    pub input: Payload,
}

impl AgentRunSpec {
    /// Builds a run spec from the neutral run identity and input payload.
    #[must_use]
    pub fn new(
        workflow_id: WorkflowId,
        run_id: RunId,
        activity_id: ActivityId,
        attempt: u32,
        activity_type: impl Into<String>,
        input: Payload,
    ) -> Self {
        Self {
            workflow_id,
            run_id,
            activity_id,
            attempt,
            activity_type: activity_type.into(),
            input,
        }
    }
}

#[cfg(test)]
mod tests {
    use aion_core::{ActivityId, ContentType, Payload, RunId, WorkflowId};

    use super::AgentRunSpec;

    #[test]
    fn spec_carries_run_identity_and_input() {
        let workflow_id = WorkflowId::new_v4();
        let run_id = RunId::new_v4();
        let activity_id = ActivityId::from_sequence_position(4);
        let input = Payload::new(ContentType::Json, b"{}".to_vec());

        let spec = AgentRunSpec::new(
            workflow_id.clone(),
            run_id.clone(),
            activity_id.clone(),
            2,
            "dev",
            input.clone(),
        );

        assert_eq!(spec.workflow_id, workflow_id);
        assert_eq!(spec.run_id, run_id);
        assert_eq!(spec.activity_id, activity_id);
        assert_eq!(spec.attempt, 2);
        assert_eq!(spec.activity_type, "dev");
        assert_eq!(spec.input, input);
    }

    /// Two generations of one continue-as-new chain: same workflow, same
    /// activity ordinal, same attempt number — distinguished only by the run.
    #[test]
    fn two_generations_of_one_chain_are_distinct_specs() {
        let workflow_id = WorkflowId::new_v4();
        let activity_id = ActivityId::from_sequence_position(0);
        let input = Payload::new(ContentType::Json, b"{}".to_vec());
        let generation = |run_id: RunId| {
            AgentRunSpec::new(
                workflow_id.clone(),
                run_id,
                activity_id.clone(),
                1,
                "dev",
                input.clone(),
            )
        };

        let first = generation(RunId::new_v4());
        let second = generation(RunId::new_v4());
        assert_ne!(first, second);
        assert_eq!(first.workflow_id, second.workflow_id);
        assert_eq!(first.activity_id, second.activity_id);
        assert_eq!(first.attempt, second.attempt);
    }
}
