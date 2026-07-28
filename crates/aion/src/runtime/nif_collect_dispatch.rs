//! The live `ActivityDispatch` one fan-out member ordinal is issued as.
//!
//! Split from `nif_collect` for the 500-line law; the seam is deliberate —
//! every routing decision a member carries is resolved in ONE place, so it
//! cannot drift from the `ActivityScheduled` recorded for the same ordinal.

use aion_core::ActivityId;

use crate::activity::bridge::ActivityDispatch;

use super::nif_activity_dispatch::FIRST_DELIVERY_ATTEMPT;
use super::nif_collect::{ActivitySpec, CollectDeps};
use super::nif_context::NifContext;

/// The live dispatch for one fan-out member ordinal.
///
/// Every routing decision is resolved at THIS schedule seam so it matches the
/// `ActivityScheduled` recorded for the same ordinal: NSTQ-4 (+#144) task queue
/// (member override > workflow default > the workflow's recorded start-time
/// queue > the named default), NODE-4 node affinity (member pin, else None),
/// and the R5 advisory class read off the contract this run is pinned to — a
/// fan-out member carries the same declaration-owned class as a single
/// dispatch of the same action.
pub(super) fn member_dispatch(
    deps: &CollectDeps,
    context: &NifContext,
    spec: &ActivitySpec,
    ordinal: u64,
    namespace: &str,
    workflow_id: &aion_core::WorkflowId,
    start_time_task_queue: Option<&str>,
) -> ActivityDispatch {
    ActivityDispatch {
        namespace: namespace.to_owned(),
        task_queue: super::nif_activity::resolve_task_queue(&spec.config, start_time_task_queue),
        node: super::nif_activity::resolve_node(&spec.config),
        workflow_id: workflow_id.clone(),
        run_id: context.workflow_handle().run_id().clone(),
        activity_id: ActivityId::from_sequence_position(ordinal),
        name: spec.name.clone(),
        input: spec.input.clone(),
        config: spec.config.clone(),
        attempt: FIRST_DELIVERY_ATTEMPT,
        labels: super::nif_activity::labels_from_config(&spec.config),
        advisory: super::nif_activity_advisory::declared_advisory(
            deps.advisory_catalog.as_deref(),
            &context.workflow_handle(),
            &spec.name,
        ),
    }
}
