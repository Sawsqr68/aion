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
//! ## Which actions go to the agent harness
//!
//! The DOCUMENT decides, through the `agent` marker on the action declaration.
//! `agent` states that the action's one `String` parameter is a prompt and its
//! `String` result is the reply, and the AWL checker refuses the marker on any
//! other shape — so a name in [`AdvertisedSurface::agent_names`] is a name the
//! harness can be handed without re-checking it here.
//!
//! It used to be decided by a four-name array in this binary's `main.rs`. That
//! is the same class of defect as the missing descriptors above: the document
//! could declare an agent round and the worker had no way to know it had, so
//! the two could drift apart silently and only a live dispatch would say so.
//! One name still lives in that array — see `UNDECLARED_AGENT_ACTIVITY_TYPES`
//! — for a package that has no document to declare it in, and it carries the
//! condition for its own deletion.
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
///
/// The document lives inside the server crate because the `aion` binary embeds
/// it too (#139 leg C): one file, read by the server that installs it and by
/// this worker that serves its agent action. A copy beside this crate would be
/// a second truth, and the two would drift.
pub const ASSISTANT_DOCUMENT: &str =
    include_str!("../../../../crates/aion-server/assistant-embed/assistant.awl");

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
    /// The actions the DOCUMENT marks `agent`, ordered by name.
    ///
    /// This is the set routed to the agent harness, and it comes from the
    /// declaration rather than from a list maintained by hand in this binary.
    /// The checker guarantees every name here is arity-1 `String -> String`, so
    /// the harness can rely on the shape without re-checking it.
    pub agent_names: Vec<String>,
}

/// The advertised surface split by HOW each action is served.
///
/// The split is load-bearing, not cosmetic.
/// [`AgentHarnessConfig::with_activity_descriptors`](aion_worker::AgentHarnessConfig::with_activity_descriptors)
/// adds every descriptor name to the agent activity set, so a descriptor handed
/// to the harness is also ROUTED to the harness. Passing the whole surface
/// there would send any plain registry action — a handler that shells to real
/// CLIs — to norn as if it were a prompt. The two halves are therefore
/// advertised through the two different paths that also execute them. (Today
/// the plain side is empty: the document's only non-agent action carries a
/// declared body, which the server runs, so it is never advertised at all.)
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

    /// Split the surface into harness-executed and registry-executed halves,
    /// along the line the DOCUMENT draws with its `agent` markers.
    ///
    /// Everything not marked `agent` is expected to have a typed handler; the
    /// registry verifies that and refuses if one is missing, so an action can
    /// never be advertised by a worker that cannot run it.
    #[must_use]
    pub fn split(&self) -> SurfaceSplit {
        let (agent, plain) = self
            .descriptors
            .iter()
            .cloned()
            .partition(|descriptor| self.agent_names.contains(&descriptor.name));
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
    let served = || worker.actions.iter().filter(|action| action.body.is_none());
    let mut descriptors: Vec<ActivityDescriptor> = served()
        .map(|action| ActivityDescriptor {
            name: action.name.clone(),
            input_schema: action.input_schema.clone(),
            output_schema: action.output_schema.clone(),
        })
        .collect();
    descriptors.sort_by(|left, right| left.name.cmp(&right.name));
    // The routing split comes from the document's own `agent` markers. It is
    // filtered by the SAME `body.is_none()` predicate as the descriptors, so a
    // name can never enter the agent set without the schema that admits it.
    let mut agent_names: Vec<String> = served()
        .filter(|action| action.agent)
        .map(|action| action.name.clone())
        .collect();
    agent_names.sort();
    Ok(AdvertisedSurface {
        task_queue: worker.task_queue.clone(),
        descriptors,
        agent_names,
    })
}

#[cfg(test)]
#[path = "descriptors_tests.rs"]
mod tests;
