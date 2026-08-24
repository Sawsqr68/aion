//! The start surface of the caller SDK: options, outcome, the SDK-boundary
//! idempotency fingerprint, and `Client::start` itself.
//!
//! Split out of `ops.rs` (which the rest of the operations still share) when the
//! display-name work (#211) pushed that file past the house 500-code-line limit.
//! The seam is cohesive rather than arbitrary: everything here answers one
//! question — what a start request is, what comes back from it, and when two
//! starts are the same act.

use aion_core::Payload;
use aion_proto::{ProtoPayload, ProtoStartWorkflowRequest};
use serde::Serialize;

use crate::client::Client;
use crate::error::ClientError;
use crate::handle::WorkflowHandle;
use crate::ops::{decode_required_run_id, decode_required_workflow_id, operation_namespace};
use crate::payload::to_payload;

/// Options accepted by [`Client::start`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartOptions {
    /// Namespace override for this start request.
    pub namespace: Option<String>,
    /// Caller-supplied idempotency key for safe local retry replay.
    ///
    /// The current AW protobuf has not added an idempotency field yet, so this is
    /// enforced at the SDK boundary without inventing a client-owned wire field.
    /// Reusing a key for a different start request returns
    /// [`ClientError::AlreadyExists`].
    pub idempotency_key: Option<String>,
    /// R-4 steered-start routing key. When set, the cluster steers this start to
    /// `shard_for(routing_key)`'s owner (forwarding there when the dialed node is
    /// not the owner). `None` keeps the default unsteered placement.
    pub routing_key: Option<String>,
    /// Optional default task queue for this workflow's activities. When set, the
    /// server records it durably on the start (the namespace × `task_queue`
    /// targeting story); `None` keeps the namespace's default queue.
    pub task_queue: Option<String>,
    /// Optional operator-facing display name for the workflow this start
    /// creates (#211). When set, the server records it durably on the start as
    /// the `aion.display_name` search attribute; `None` starts it unnamed (it
    /// shows its bare UUID).
    ///
    /// A LABEL over the UUID identity, never an address: no SDK call resolves a
    /// workflow by name. It is deliberately NOT part of the idempotency
    /// fingerprint — see [`StartOutcome::display_name_not_applied`] for what
    /// happens when a reused key carries a different name.
    ///
    /// You name the run you are starting, but the label READS BACK per
    /// workflow: the recorded attribute carries no run id, so every reader
    /// folds it over the whole history, last write wins. A continue-as-new
    /// successor of this run inherits the name, and describing any run of the
    /// workflow returns whatever name was recorded most recently.
    pub display_name: Option<String>,
}

/// What [`Client::start`] returns: the started (or idempotently replayed) run,
/// plus the note that a requested display name was NOT applied.
///
/// The note exists because `display_name` is deliberately outside
/// [`StartFingerprint`]: the fingerprint answers "is this the same act?", and a
/// label carries no identity, so two starts differing only in name are the same
/// act and dedupe to one run. That leaves a second, different name with nowhere
/// to go — and dropping it silently would hand the caller a run wearing a name
/// they did not ask for while saying nothing. So the drop is REPORTED here
/// instead. The replayed run keeps its EXISTING name; nothing is renamed behind
/// the caller's back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartOutcome {
    /// Handle to the started run, or to the existing run an idempotent replay
    /// matched.
    pub handle: WorkflowHandle,
    /// Present ONLY when this call REQUESTED a display name, an idempotency-key
    /// replay matched an existing run, and that run does not already wear the
    /// requested name — whether it wears a different one or none at all.
    ///
    /// `None` on every other path: a first start (which applies the name it
    /// asked for), a replay that requested the same name, and — the rule this
    /// field's absence states — any call that requested NO name. A caller who
    /// asked for nothing had nothing dropped, so there is nothing to report,
    /// even when the standing run wears a name of its own.
    pub display_name_not_applied: Option<DisplayNameNotApplied>,
}

/// The report that an idempotent start replay did not apply the display name
/// this call requested (#211).
///
/// Carries both sides so the caller can act on the difference: rename the run
/// deliberately, use a different idempotency key, or accept the standing name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayNameNotApplied {
    /// The display name this start asked for. Never empty and never absent:
    /// the report is raised only for a call that actually requested a name, so
    /// there is always a name that did not get applied.
    pub requested: String,
    /// The display name the CACHED start requested for that run (`None` when
    /// that start asked for no name).
    ///
    /// This is what the first start asked for, not a live read: the cache is
    /// never refreshed, so if the run has since been renamed through the rename
    /// surface, this is the name it was STARTED with rather than the one it now
    /// wears. Read it as "the name this key already stood for", and read the
    /// run itself if you need its current name.
    pub standing: Option<String>,
}

/// What a start request is keyed by for SDK-boundary idempotency: the namespace,
/// the workflow type, the payload's content type and bytes, and BOTH routing
/// dimensions.
///
/// The routing fields are part of the key because they change where the workflow
/// runs and which queue its activities go to. Replaying a cached handle for a key
/// reused with a different `task_queue` would silently discard the caller's
/// second, different intent and hand back the first workflow as though it had
/// honoured it. The Python SDK keys the same six values, and the cross-SDK
/// conformance contract requires the two to agree on what a key *means*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StartFingerprint {
    namespace: String,
    workflow_type: String,
    content_type: aion_core::ContentType,
    bytes: Vec<u8>,
    routing_key: Option<String>,
    task_queue: Option<String>,
    idempotency_key: String,
}

impl StartFingerprint {
    fn new(
        namespace: String,
        workflow_type: String,
        input: &Payload,
        routing_key: Option<String>,
        task_queue: Option<String>,
        idempotency_key: String,
    ) -> Self {
        Self {
            namespace,
            workflow_type,
            content_type: input.content_type().clone(),
            bytes: input.bytes().to_vec(),
            routing_key,
            task_queue,
            idempotency_key,
        }
    }

    pub(crate) fn key(&self) -> &str {
        &self.idempotency_key
    }
}

/// One idempotency-cache entry: the fingerprint the start was keyed by, the
/// handle to replay, and the display name that start requested (#211).
///
/// The display name is stored BESIDE the fingerprint, never inside it. Inside,
/// it would make two starts differing only in a label two different acts and
/// break the pre-approved dedupe ruling; absent entirely, a replay could not
/// tell the caller which name the standing run actually wears. Beside is what
/// lets the replay dedupe AND report.
#[derive(Clone, Debug)]
pub(crate) struct CachedStart {
    fingerprint: StartFingerprint,
    handle: WorkflowHandle,
    display_name: Option<String>,
}

impl CachedStart {
    pub(crate) const fn new(
        fingerprint: StartFingerprint,
        handle: WorkflowHandle,
        display_name: Option<String>,
    ) -> Self {
        Self {
            fingerprint,
            handle,
            display_name,
        }
    }

    pub(crate) const fn fingerprint(&self) -> &StartFingerprint {
        &self.fingerprint
    }

    pub(crate) fn into_handle(self) -> WorkflowHandle {
        self.handle
    }

    /// The report that `requested` was not applied, or `None` when there is
    /// nothing to report.
    ///
    /// The rule, exactly: a report is raised when this replay ASKED for a name
    /// (`requested.is_some()`) that the standing run does not already wear —
    /// whether that run wears a different name or no name at all. Both of those
    /// callers asked for something the replay could not give them, so both are
    /// told.
    ///
    /// A replay that asked for NO name gets no report, whatever the standing
    /// run wears. Nothing was requested, so nothing was dropped, and the
    /// report's absence is precisely the statement that no requested name went
    /// missing — see [`StartOutcome::display_name_not_applied`]. Raising it
    /// there would report a drop that never happened and push callers who
    /// branch on the field into acting on a run they never tried to name.
    pub(crate) fn display_name_not_applied(
        &self,
        requested: Option<&str>,
    ) -> Option<DisplayNameNotApplied> {
        let requested = requested?;
        if self.display_name.as_deref() == Some(requested) {
            return None;
        }
        Some(DisplayNameNotApplied {
            requested: requested.to_owned(),
            standing: self.display_name.clone(),
        })
    }
}

fn validate_start_options(opts: &StartOptions) -> Result<(), ClientError> {
    if opts
        .idempotency_key
        .as_ref()
        .is_some_and(std::string::String::is_empty)
    {
        return Err(ClientError::invalid_argument(
            "idempotency_key must not be empty",
        ));
    }
    // #211: the server REFUSES a present-but-blank display name with
    // `invalid_input` — it does not reinterpret one as "unnamed", the way it
    // reinterprets a blank `task_queue` as "no selection". Refusing here too
    // means a caller who passed `Some("  ")` gets the same answer from the SDK
    // as from raw gRPC, and gets it before a round trip. `None` is how a caller
    // says "no name"; absence is what the server accepts as unnamed.
    if opts
        .display_name
        .as_ref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(ClientError::invalid_argument(
            "display_name must not be blank; omit it to start the run unnamed",
        ));
    }
    Ok(())
}

impl Client {
    /// Starts a workflow and returns the assigned workflow and run identifiers.
    ///
    /// Returns a [`StartOutcome`] rather than a bare handle so an idempotent
    /// replay can report a display name it did NOT apply (#211) instead of
    /// dropping it silently — see [`StartOutcome::display_name_not_applied`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] when transport, server, or response conversion fails.
    pub async fn start(
        &self,
        workflow_type: impl Into<String>,
        input: Payload,
        opts: StartOptions,
    ) -> Result<StartOutcome, ClientError> {
        validate_start_options(&opts)?;
        let idempotency_key = opts.idempotency_key.clone();
        let routing_key = opts.routing_key.clone();
        let task_queue = opts.task_queue.clone();
        // The server trims the name before recording it, so the run wears the
        // TRIMMED string. Trim here too and the SDK's cached "standing name"
        // is the name the run actually has — otherwise a replay comparing
        // `"X"` against a cached `"  X  "` would report a difference the
        // server had already erased. Blank-after-trim was refused above, so
        // this can never turn a name into no name.
        let display_name = opts
            .display_name
            .as_deref()
            .map(str::trim)
            .map(str::to_owned);
        let namespace = operation_namespace(self, opts.namespace);
        let workflow_type = workflow_type.into();
        let fingerprint = idempotency_key.as_ref().map(|key| {
            StartFingerprint::new(
                namespace.clone(),
                workflow_type.clone(),
                &input,
                routing_key.clone(),
                task_queue.clone(),
                key.clone(),
            )
        });
        if let Some(fingerprint) = &fingerprint
            && let Some(cached) = self.cached_start(fingerprint).await?
        {
            // The run already exists and keeps the name it already wears.
            // If this call asked for a different one, say so rather than
            // quietly handing back a differently-named run (#211).
            let not_applied = cached.display_name_not_applied(display_name.as_deref());
            return Ok(StartOutcome {
                handle: cached.into_handle(),
                display_name_not_applied: not_applied,
            });
        }
        let response = self
            .transport
            .start_workflow(ProtoStartWorkflowRequest {
                namespace,
                workflow_type,
                input: Some(ProtoPayload::from(input)),
                routing_key,
                task_queue,
                display_name: display_name.clone(),
            })
            .await?;
        let workflow_id = decode_required_workflow_id(response.workflow_id, "start response")?;
        let run_id = decode_required_run_id(response.run_id, "start response")?;
        let handle = WorkflowHandle::from_ids(self.clone(), workflow_id, run_id);
        if let Some(fingerprint) = fingerprint {
            self.record_start(CachedStart::new(fingerprint, handle.clone(), display_name))
                .await?;
        }
        Ok(StartOutcome {
            handle,
            display_name_not_applied: None,
        })
    }

    /// Starts a workflow after serializing `input` as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when serialization fails, or the
    /// delegated start error otherwise.
    pub async fn start_typed<T>(
        &self,
        workflow_type: impl Into<String>,
        input: &T,
        opts: StartOptions,
    ) -> Result<StartOutcome, ClientError>
    where
        T: Serialize + ?Sized,
    {
        self.start(workflow_type, to_payload(input)?, opts).await
    }
}
