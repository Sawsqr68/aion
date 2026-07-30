//! The typed activity surface this worker ADVERTISES at registration.
//!
//! ## The defect this closes
//!
//! A worker registration carries two independent things: activity-type NAMES
//! (`activity_types`) and typed descriptors (`activities`). Contract admission
//! reads ONLY the descriptors. Nothing synthesizes one from the other.
//!
//! This worker advertised 8 names and ZERO descriptors, because
//! `ActivityRegistry::register_activity` inserts a handler without a
//! descriptor and `AgentHarnessConfig::new` starts with an empty descriptor
//! list. Every action of every deployed contract on its queue therefore came
//! back `advertised: <missing>` and the whole connection was refused — while
//! the refusal WARN logged the NAME list, which did contain them, so the log
//! read as a contradiction and the worker looked healthy.
//!
//! The gate was right. The worker was not.
//!
//! ## Where the schemas come from — and why not from here
//!
//! They are DERIVED, at startup, from the `.awl` document that declares them.
//! The document is embedded with `include_str!` and compiled with the same
//! `aion_awl::compile` the `aion worker agent` verb uses, and the descriptors
//! are the compiled contract's own `input_schema` / `output_schema`
//! (`crates/aion-cli/src/worker_surface.rs:158-220` is the reference shape).
//!
//! Hand-writing them here would have re-created the bug in a new form. A
//! hand-written schema is a claim about a contract it cannot see, and it drifts
//! the moment the document changes — silently, because a worker whose schemas
//! merely DIFFER is refused with the same message as one that advertises
//! nothing. Deriving from the document makes the advertisement correct by
//! construction: change `assistant.awl` and the advertisement changes with it.
//!
//! This is the third time this one worker has shipped a registration that did
//! not match the contract it claimed to serve — `--account` missing from the
//! norn invocation, `--allow-degraded-session` missing, and now the descriptors.
//! All three were "the composed thing does not match its contract", and none of
//! them failed until a live dispatch. That is why the surface is derived rather
//! than declared, and why `descriptors_tests.rs` runs the real
//! `aion_package::contract_diffs` gate rather than a proxy for it.
//!
//! ## What is NOT advertised, and why that is not an omission
//!
//! The `agent_dev` workflow's own activities (`provision`, `gate`, `land`,
//! `scout`, `dev`, `review`) are served by this binary but are NOT advertised.
//! No deployable contract declares them: `agent_dev` is a Gleam-authored
//! package, and a Gleam package cannot be started at all under queue-routed
//! serving (`docs/design/aion-authoring/GLEAM-PATH-CANNOT-START-FINDING.md`),
//! so it can never become a reachable contract to be held against this worker.
//! Advertising a schema for an action no contract declares would mean inventing
//! one — the exact thing this module exists to stop. Their handlers stay
//! registered; if `agent_dev` ever gains an `.awl` document, its descriptors
//! come from that document, here, the same way.

use aion_awl::CompileError;
use aion_package::ActivityDescriptor;

/// The `assistant` session document — the contract this worker serves.
///
/// Embedded rather than read from disk: a worker that resolved its own
/// contract from a path could be started against a different document than the
/// one that was deployed, and would advertise a surface nobody asked for.
pub const ASSISTANT_DOCUMENT: &str = include_str!("../../../assistant/awl/assistant.awl");

/// Failure to derive the advertised surface from the embedded document.
#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    /// The embedded document does not compile.
    #[error("the embedded assistant document does not compile: {source}")]
    Compile {
        /// The compiler's diagnosis.
        #[source]
        source: Box<CompileError>,
    },
    /// The compiled contract declares no worker block, so there is no queue.
    #[error(
        "the embedded assistant document declares no worker block, so this worker would \
         advertise nothing and every queue carrying a deployed contract would refuse it"
    )]
    NoWorker,
}

/// The surface this worker advertises: the queue and its typed descriptors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvertisedSurface {
    /// The task queue the document declares — the worker's queue.
    pub task_queue: String,
    /// The typed descriptors, ordered by name so two runs advertise identically.
    pub descriptors: Vec<ActivityDescriptor>,
}

/// The advertised surface split by HOW each action is served.
///
/// The split is load-bearing, not cosmetic.
/// [`AgentHarnessConfig::with_activity_descriptors`](aion_worker::AgentHarnessConfig::with_activity_descriptors)
/// adds every descriptor name to the agent activity set, so a descriptor handed
/// to the harness is also ROUTED to the harness. Passing the whole surface
/// there would send `assistant_provision` — a plain registry handler that
/// clones a workspace — to norn as if it were a prompt. The two halves are
/// therefore advertised through the two different paths that also execute them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceSplit {
    /// Descriptors for actions the agent harness executes.
    pub agent: Vec<ActivityDescriptor>,
    /// Descriptors for actions a typed registry handler executes.
    pub plain: Vec<ActivityDescriptor>,
}

impl AdvertisedSurface {
    /// The advertised action names, derived from the descriptors so a name can
    /// never be announced without the schema that makes it admissible. This is
    /// the invariant the old registration violated.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.descriptors
            .iter()
            .map(|descriptor| descriptor.name.clone())
            .collect()
    }

    /// Split the surface into harness-executed and registry-executed halves.
    ///
    /// `agent_types` is the set this binary routes through the agent harness.
    /// Everything else in the document is expected to have a typed handler; the
    /// registry verifies that and refuses if one is missing, so an action can
    /// never be advertised by a worker that cannot run it.
    #[must_use]
    pub fn split(&self, agent_types: &[&str]) -> SurfaceSplit {
        let (agent, plain) = self
            .descriptors
            .iter()
            .cloned()
            .partition(|descriptor| agent_types.contains(&descriptor.name.as_str()));
        SurfaceSplit { agent, plain }
    }
}

/// Derive the advertised surface from the embedded `assistant.awl`.
///
/// Actions carrying a declared BODY are excluded: the server runs a declared
/// body itself, so no worker serves it and a worker claiming it would shadow
/// the declared implementation.
///
/// # Errors
///
/// [`SurfaceError`] when the embedded document does not compile or declares no
/// worker block. Both are refused at startup rather than at a remote
/// registration, because a refusal that happens on the far side of a socket is
/// a refusal the operator has to go looking for.
pub fn advertised_surface() -> Result<AdvertisedSurface, SurfaceError> {
    // The document declares no schema doors, so it needs no import root; the
    // worker's own directory is the honest answer for one.
    let compiled =
        aion_awl::compile(ASSISTANT_DOCUMENT, std::path::Path::new(".")).map_err(|source| {
            SurfaceError::Compile {
                source: Box::new(source),
            }
        })?;
    let worker = compiled
        .contract
        .workers
        .first()
        .ok_or(SurfaceError::NoWorker)?;
    let mut descriptors: Vec<ActivityDescriptor> = worker
        .actions
        .iter()
        .filter(|action| action.body.is_none())
        .map(|action| ActivityDescriptor {
            name: action.name.clone(),
            input_schema: action.input_schema.clone(),
            output_schema: action.output_schema.clone(),
        })
        .collect();
    descriptors.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(AdvertisedSurface {
        task_queue: worker.task_queue.clone(),
        descriptors,
    })
}

#[cfg(test)]
#[path = "descriptors_tests.rs"]
mod tests;
