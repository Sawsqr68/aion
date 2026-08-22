"""Async Aion workflow client for the caller-side Python SDK."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from types import TracebackType
from typing import Any, TypeVar, cast

from .errors import (
    AionClientError,
    AlreadyExists,
    InvalidArgument,
    QueryFailed,
    ServerError,
    map_query_error,
    raise_mapped,
)
from .handle import WorkflowHandle
from .payload import JSONValue, Payload, assign_payload, decode_payload, ensure_payload, payload_from_wire
from .transport import GrpcWorkflowTransport, MappingMetadata, WorkflowTransport, metadata, workflow_status_name

T = TypeVar("T")

# What a start request is keyed by for SDK-boundary idempotency: the namespace,
# the workflow type, the payload's content type, its bytes, and BOTH routing
# dimensions. The routing fields are part of the key because they change where
# the workflow runs and which queue its activities go to — replaying a cached
# handle for a key reused with a different task queue would silently discard the
# caller's second, different intent and hand back the first workflow as though it
# had honoured it.
#
# `display_name` (#211) is deliberately NOT in here. The fingerprint answers "is
# this the same act?", and a label carries no identity — nothing resolves a
# workflow by name — so two starts differing only in their name are the same act
# and must dedupe to one run rather than conflict. What the exclusion costs is
# reported instead of swallowed: see `StartOutcome.display_name_not_applied`.
# The Rust caller SDK keys the same six values, and the cross-SDK conformance
# contract requires the two to agree on what a key *means*.
StartFingerprint = tuple[str, str, str, bytes, str | None, str | None]


@dataclass(frozen=True, slots=True)
class TLSConfig:
    """TLS options used when creating the reusable transport connection."""

    enabled: bool = True
    ca_file: str | None = None
    client_cert_file: str | None = None
    client_key_file: str | None = None
    server_name: str | None = None


@dataclass(frozen=True, slots=True)
class WorkflowDescription:
    """Decoded describe response: summary, optional history, and the bounded
    describe facts the contract carries alongside them.

    ``run_id`` names the concrete run described. ``history_head_seq`` is the
    run's current history head, usable as a ``read_history`` cursor bound.
    ``terminal_event`` is the run's terminal event envelope when the run has
    reached one, ``None`` while it is still running.
    """

    summary: Any
    history: list[Any]
    run_id: str
    history_head_seq: int
    terminal_event: Any | None


@dataclass(frozen=True, slots=True)
class HistoryPage:
    """One ``read_history`` page: events plus the cursor pair.

    ``next_from_seq`` is the cursor to pass as the next call's ``from_seq``;
    ``head_seq`` is the run's history head at read time, so a caller can tell
    a complete read (``next_from_seq > head_seq``) from a partial one.
    """

    events: list[Any]
    next_from_seq: int
    head_seq: int


@dataclass(frozen=True, slots=True)
class RunOutcome:
    """The run a lifecycle transition acted on, and its projected status.

    Returned by :meth:`Client.reopen`, :meth:`Client.pause`, and
    :meth:`Client.resume`. All three carry exactly this pair — the concrete run
    and the status the transition projects it into (``RUNNING`` after a reopen or
    resume, ``PAUSED`` after a pause) — so one type serves all three rather than
    three identical ones differing only in name.

    ``status`` is the language-neutral status name shared with the other client
    SDKs (``RUNNING``, ``PAUSED``, ``COMPLETED``, ...), decoded from the wire
    enum's own descriptor.
    """

    run_id: str
    status: str


@dataclass(frozen=True, slots=True)
class DisplayNameNotApplied:
    """The report that an idempotent start replay did not apply a display name (#211).

    Carries both sides so the caller can act on the difference: use a different
    idempotency key, accept the standing name, or rename the run through the
    console/HTTP rename surface (`POST /workflows/rename`). Renaming is NOT an
    operation on this SDK in this release, so it is not a remedy this object can
    hand you — it is an operator action taken elsewhere.
    """

    requested: str
    """The display name this start asked for. Never blank and never ``None``:
    the report is raised only for a call that actually requested a name, so
    there is always a name that did not get applied."""

    standing: str | None
    """The display name the matched run was started with and still wears
    (``None`` when that run was started unnamed)."""


@dataclass(frozen=True, slots=True)
class StartOutcome:
    """What :meth:`Client.start` returns: the started (or idempotently replayed)
    run, plus the note that a requested display name was NOT applied.

    The note exists because ``display_name`` is deliberately outside
    :data:`StartFingerprint`: the fingerprint answers "is this the same act?",
    and a label carries no identity, so two starts differing only in name are the
    same act and dedupe to one run. That leaves a second, different name with
    nowhere to go — and dropping it silently would hand the caller a run wearing
    a name they did not ask for while saying nothing. So the drop is REPORTED
    here instead. The replayed run keeps its EXISTING name; nothing is renamed
    behind the caller's back.
    """

    handle: WorkflowHandle
    """Handle to the started run, or to the existing run an idempotent replay
    matched."""

    display_name_not_applied: DisplayNameNotApplied | None
    """Present ONLY when this call REQUESTED a display name, an idempotency-key
    replay matched an existing run, and that run does not already wear the
    requested name — whether it wears a different one or none at all.

    ``None`` on every other path: a first start (which applies the name it asked
    for), a replay that requested the same name, and — the rule this field's
    absence states — any call that requested NO name. A caller who asked for
    nothing had nothing dropped, so there is nothing to report, even when the
    standing run wears a name of its own."""


@dataclass(frozen=True, slots=True)
class _CachedStart:
    """One idempotency-cache entry: the fingerprint the start was keyed by, the
    handle to replay, and the display name that start requested (#211).

    The display name is stored BESIDE the fingerprint, never inside it. Inside,
    it would make two starts differing only in a label two different acts and
    break the dedupe ruling; absent entirely, a replay could not tell the caller
    which name the standing run actually wears. Beside is what lets the replay
    dedupe AND report.
    """

    fingerprint: StartFingerprint
    handle: WorkflowHandle
    display_name: str | None

    def display_name_not_applied(self, requested: str | None) -> DisplayNameNotApplied | None:
        """The report that ``requested`` was not applied, or ``None`` when there
        is nothing to report.

        The rule, exactly: a report is raised when this replay ASKED for a name
        that the standing run does not already wear — whether that run wears a
        different name or no name at all. Both of those callers asked for
        something the replay could not give them, so both are told.

        A replay that asked for NO name gets no report, whatever the standing run
        wears. Nothing was requested, so nothing was dropped, and the report's
        absence is precisely the statement that no requested name went missing
        (see :attr:`StartOutcome.display_name_not_applied`). Raising it there
        would report a drop that never happened and push callers who branch on
        the field into acting on a run they never tried to name.
        """

        if requested is None or requested == self.display_name:
            return None
        return DisplayNameNotApplied(requested=requested, standing=self.display_name)


class Client:
    """Reusable async client for Aion workflow operations.

    Public methods raise branchable subclasses of ``AionClientError`` according
    to the shared taxonomy: NotFound, AlreadyExists, QueryFailed, QueryTimeout,
    Cancelled, Unavailable, Unauthenticated, NamespaceDenied, InvalidArgument,
    and ServerError.
    """

    def __init__(
        self,
        endpoint: str,
        *,
        auth: str | None = None,
        tls: TLSConfig | bool | None = None,
        namespace: str = "default",
        transport: WorkflowTransport | None = None,
        stream_endpoint: str | None = None,
        subject: str | None = None,
        namespaces: Sequence[str] | None = None,
    ) -> None:
        """Create a reusable client connection.

        ``stream_endpoint`` is the full URL of the server's
        ``/events/stream`` WebSocket route (for example
        ``ws://127.0.0.1:8080/events/stream``). There is no default and
        nothing is derived: the gRPC endpoint and the HTTP/WebSocket
        listener are separate addresses, so ``subscribe`` without this
        option raises :class:`InvalidArgument` with a precise message.

        Raises:
            InvalidArgument, Unavailable, Unauthenticated, ServerError,
            Cancelled.
        """

        if not endpoint:
            raise InvalidArgument("endpoint must not be empty")
        if not namespace:
            raise InvalidArgument("namespace must not be empty")
        self.endpoint = endpoint.rstrip("/")
        self.namespace = namespace
        self.auth = auth
        self.subject = subject
        self.namespaces = tuple(namespaces) if namespaces is not None else None
        self.tls = _normalize_tls(tls)
        self._transport = transport or GrpcWorkflowTransport(self.endpoint, tls=self.tls)
        self.stream_endpoint = stream_endpoint
        self._metadata = metadata(auth, subject=subject, namespaces=self.namespaces)
        # SDK-boundary start idempotency (the contract's hard case): the same
        # key retried with an identical request returns the original handle;
        # conflicting reuse raises AlreadyExists. Keyed by idempotency key,
        # fingerprinted over every IDENTIFYING field of the start request (see
        # StartFingerprint), with the requested display name carried alongside
        # so a replay can report a name it did not apply (see _CachedStart).
        self._idempotent_starts: dict[str, _CachedStart] = {}

    @classmethod
    async def connect(
        cls,
        endpoint: str,
        *,
        auth: str | None = None,
        tls: TLSConfig | bool | None = None,
        namespace: str = "default",
        transport: WorkflowTransport | None = None,
        stream_endpoint: str | None = None,
        subject: str | None = None,
        namespaces: Sequence[str] | None = None,
    ) -> Client:
        """Create a client and validate the reusable connection if supported.

        Raises:
            InvalidArgument, Unavailable, Unauthenticated, ServerError,
            Cancelled.
        """

        client = cls(
            endpoint,
            auth=auth,
            tls=tls,
            namespace=namespace,
            transport=transport,
            stream_endpoint=stream_endpoint,
            subject=subject,
            namespaces=namespaces,
        )
        connect = getattr(client._transport, "connect", None)
        if callable(connect):
            try:
                await connect()
            except BaseException as exc:
                if isinstance(exc, AionClientError):
                    raise
                raise_mapped(exc)
        return client

    async def __aenter__(self) -> Client:
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        await self.close()

    async def close(self) -> None:
        """Close the reusable transport.

        Raises:
            Unavailable, ServerError, Cancelled.
        """

        try:
            await self._transport.close()
        except BaseException as exc:
            if isinstance(exc, AionClientError):
                raise
            raise_mapped(exc)

    async def start(
        self,
        workflow_type: str,
        input: JSONValue | None = None,
        *,
        raw: bytes | None = None,
        content_type: str | None = None,
        namespace: str | None = None,
        idempotency_key: str | None = None,
        routing_key: str | None = None,
        task_queue: str | None = None,
        display_name: str | None = None,
    ) -> StartOutcome:
        """Start a workflow and return a :class:`StartOutcome` bound to the run.

        ``routing_key`` steers the start to the owner of
        ``shard_for(routing_key)``, forwarding there when the dialed node is not
        the owner; omitting it keeps the default unsteered placement.
        ``task_queue`` sets the queue this workflow's activities default to,
        recorded durably on the start so it survives replay and failover;
        omitting it keeps the namespace's default queue.

        ``display_name`` is the operator-facing name for the workflow this start
        creates (#211), recorded durably on the start. It is a LABEL over the
        UUID identity, never an address: no SDK call resolves a workflow by name,
        and two workflows may wear the same one. Omitting it starts the workflow
        unnamed, which shows its bare UUID. It is trimmed before it is sent,
        because the server trims it before recording it — so the name this SDK
        remembers is the name the workflow actually wears, and surrounding
        whitespace is never mistaken for a different name.

        You name the run you are starting, but the label READS BACK per
        workflow: the recorded attribute carries no run id, so every reader folds
        it over the whole history, last write wins. A continue-as-new successor
        of this run inherits the name, and describing any run of the workflow
        returns whatever name was recorded most recently.

        None of the three accepts an empty or whitespace-only string; pass
        ``None`` to mean "no selection" / "no name". For ``routing_key`` and
        ``task_queue`` the server reads a blank as "not selected" and falls back
        to the default, so a caller who believed they had chosen one would
        silently get neither. For ``display_name`` the server REFUSES a
        present-but-blank name with ``invalid_input`` rather than reading it as
        unnamed — only ABSENCE starts a run unnamed.

        The return is a :class:`StartOutcome` rather than a bare handle so an
        idempotent replay can REPORT a display name it did not apply instead of
        dropping it in silence — see
        :attr:`StartOutcome.display_name_not_applied`.

        Raises:
            AlreadyExists, Unauthenticated, NamespaceDenied, Unavailable,
            InvalidArgument, ServerError, Cancelled.
        """

        if not workflow_type:
            raise InvalidArgument("workflow_type must not be empty")
        _reject_blank_selection("routing_key", routing_key)
        _reject_blank_selection("task_queue", task_queue)
        _reject_blank_selection("display_name", display_name)
        # The server trims the name before recording it, so the run wears the
        # TRIMMED string. Trim here too and the SDK's cached "standing name" is
        # the name the run actually has — otherwise a replay comparing `"X"`
        # against a cached `"  X  "` would report a difference the server had
        # already erased. This is not a silent normalization: it is the same
        # trim the server performs, applied where the SDK's own bookkeeping can
        # see it. Blank-after-trim was refused just above, so this can never
        # turn a name into no name. One trimmed value is then what is SENT, what
        # is STORED in the cache entry, and what is COMPARED on a replay — one
        # value, three uses, so they cannot disagree.
        display_name = display_name.strip() if display_name is not None else None
        payload = ensure_payload(input, raw=raw, content_type=content_type)
        fingerprint: StartFingerprint | None = None
        if idempotency_key is not None:
            fingerprint = (
                namespace or self.namespace,
                workflow_type,
                payload.content_type,
                bytes(payload.bytes),
                routing_key,
                task_queue,
            )
            cached = self._idempotent_starts.get(idempotency_key)
            if cached is not None:
                if cached.fingerprint == fingerprint:
                    # The run already exists and keeps the name it already
                    # wears. If this call asked for a different one, say so
                    # rather than quietly handing back a differently-named run.
                    return StartOutcome(
                        handle=cached.handle,
                        display_name_not_applied=cached.display_name_not_applied(display_name),
                    )
                raise _idempotency_conflict()
        request = self._message("StartWorkflowRequest")
        request.namespace = namespace or self.namespace
        request.workflow_type = workflow_type
        assign_payload(request.input, payload)
        # `routing_key`, `task_queue`, and `display_name` are proto3 `optional`,
        # so assigning them is what marks them present. Leaving them unassigned
        # is how "no selection" is expressed on the wire — never an empty string.
        # For `routing_key` and `task_queue` a blank would make the request claim
        # a selection it does not carry; for `display_name` the server refuses a
        # present-but-blank value outright, so absence is the only way to start
        # a run unnamed.
        if routing_key is not None:
            request.routing_key = routing_key
        if task_queue is not None:
            request.task_queue = task_queue
        if display_name is not None:
            request.display_name = display_name
        try:
            response = await self._transport.start_workflow(
                request,
                self._call_metadata(idempotency_key=idempotency_key),
            )
            workflow_id = _id_value(response.workflow_id)
            run_id = _id_value(response.run_id)
        except BaseException as exc:
            if isinstance(exc, AionClientError):
                raise
            raise_mapped(exc, operation="start")
        handle = WorkflowHandle(
            client=self,
            workflow_id=workflow_id,
            run_id=run_id,
            namespace=namespace or self.namespace,
        )
        if idempotency_key is not None and fingerprint is not None:
            recorded = self._idempotent_starts.get(idempotency_key)
            if recorded is not None and recorded.fingerprint != fingerprint:
                raise _idempotency_conflict()
            self._idempotent_starts.setdefault(
                idempotency_key,
                _CachedStart(fingerprint=fingerprint, handle=handle, display_name=display_name),
            )
        # This call reached the server, so whatever name it asked for was applied
        # to the run it started: there is nothing unapplied to report. That holds
        # even when a concurrent start won the cache race above — the handle
        # returned here is THIS start's own run, wearing THIS call's own name.
        return StartOutcome(handle=handle, display_name_not_applied=None)

    async def signal(
        self,
        workflow_id: str,
        signal_name: str,
        payload: JSONValue | None = None,
        *,
        raw: bytes | None = None,
        content_type: str | None = None,
        run_id: str | None = None,
        namespace: str | None = None,
    ) -> None:
        """Send a signal to a workflow run or latest run.

        Raises:
            NotFound, Unauthenticated, NamespaceDenied, Unavailable,
            InvalidArgument, ServerError, Cancelled.
        """

        if not workflow_id or not signal_name:
            raise InvalidArgument("workflow_id and signal_name must not be empty")
        request = self._targeted_message("SignalRequest", namespace, workflow_id, run_id)
        request.signal_name = signal_name
        assign_payload(request.payload, ensure_payload(payload, raw=raw, content_type=content_type))
        await self._call("signal", self._transport.signal(request, self._metadata))

    async def query(
        self,
        workflow_id: str,
        query_name: str,
        payload: JSONValue | None = None,
        *,
        raw: bytes | None = None,
        content_type: str | None = None,
        run_id: str | None = None,
        namespace: str | None = None,
        target_type: type[T] | None = None,
        timeout: float | None = None,
    ) -> T | JSONValue | bytes:
        """Run a synchronous workflow query and decode its result.

        Query arguments passed here are validated and then DROPPED: this SDK
        does not populate ``QueryRequest.arguments``, so the server substitutes
        the canonical JSON ``null`` document and the handler is called with no
        arguments. The wire field exists — it is tag 5 on ``QueryRequest`` and
        the regenerated stubs carry it — so this is a gap in this client, not in
        the contract, and it is tracked as one rather than described as a
        contract limitation. Until it is closed, do not pass ``payload``/``raw``
        expecting the handler to see them.

        Raises:
            NotFound, QueryFailed, QueryTimeout, Unauthenticated,
            NamespaceDenied, Unavailable, InvalidArgument, ServerError,
            Cancelled.
        """

        if not workflow_id or not query_name:
            raise InvalidArgument("workflow_id and query_name must not be empty")
        if payload is not None or raw is not None:
            ensure_payload(payload, raw=raw, content_type=content_type)
        request = self._targeted_message("QueryRequest", namespace, workflow_id, run_id)
        request.query_name = query_name
        try:
            response = await self._transport.query(request, self._metadata, timeout=timeout)
            outcome = _which_oneof(response, "outcome")
            if outcome == "error":
                raise map_query_error(response.error)
            if outcome != "result":
                raise QueryFailed("query response did not contain a result")
            return decode_payload(payload_from_wire(response.result), target_type)
        except BaseException as exc:
            if isinstance(exc, AionClientError):
                raise
            raise_mapped(exc, operation="query")

    async def cancel(
        self,
        workflow_id: str,
        *,
        run_id: str | None = None,
        reason: str = "",
        namespace: str | None = None,
    ) -> None:
        """Request cooperative cancellation of a workflow run or latest run.

        Raises:
            NotFound, Unauthenticated, NamespaceDenied, Unavailable,
            InvalidArgument, ServerError, Cancelled.
        """

        request = self._targeted_message("CancelRequest", namespace, workflow_id, run_id)
        request.reason = reason
        await self._call("cancel", self._transport.cancel(request, self._metadata))

    async def reopen(
        self,
        workflow_id: str,
        *,
        run_id: str | None = None,
        namespace: str | None = None,
    ) -> RunOutcome:
        """Reopen a reopenable terminal run and return it with its new status.

        Targets the latest run unless ``run_id`` names one. Unlike ``cancel``,
        this is not an empty ack: the response carries the run that is live again
        and its projected status (``RUNNING``), because "the request was accepted"
        would not tell the caller whether the run actually reopened.

        Raises:
            NotFound: no such workflow or run.
            InvalidArgument: the run is not in a reopenable state.
            Unauthenticated, NamespaceDenied, Unavailable, ServerError,
            Cancelled.
        """

        request = self._targeted_message("ReopenRequest", namespace, workflow_id, run_id)
        response = await self._call("reopen", self._transport.reopen(request, self._metadata))
        return _run_outcome(response)

    async def pause(
        self,
        workflow_id: str,
        *,
        run_id: str | None = None,
        reason: str = "",
        namespace: str | None = None,
    ) -> RunOutcome:
        """Hold new activity dispatch for a live run and return its new status.

        Targets the latest run unless ``run_id`` names one. The hold is durable,
        so it survives failover; ``Paused`` is a non-terminal status superseded by
        a resume. ``reason`` is recorded for the operator and may be empty.

        Raises:
            NotFound: no such workflow or run.
            InvalidArgument: the run is not ``Running``.
            Unauthenticated, NamespaceDenied, Unavailable, ServerError,
            Cancelled.
        """

        request = self._targeted_message("PauseRequest", namespace, workflow_id, run_id)
        request.reason = reason
        response = await self._call("pause", self._transport.pause(request, self._metadata))
        return _run_outcome(response)

    async def resume(
        self,
        workflow_id: str,
        *,
        run_id: str | None = None,
        namespace: str | None = None,
    ) -> RunOutcome:
        """Release a paused run's dispatch hold and return its new status.

        Targets the latest run unless ``run_id`` names one.

        Raises:
            NotFound: no such workflow or run.
            InvalidArgument: the run is not ``Paused``.
            Unauthenticated, NamespaceDenied, Unavailable, ServerError,
            Cancelled.
        """

        request = self._targeted_message("ResumeRequest", namespace, workflow_id, run_id)
        response = await self._call("resume", self._transport.resume(request, self._metadata))
        return _run_outcome(response)

    async def list(self, *, namespace: str | None = None, workflow_filter: Any | None = None) -> list[Any]:
        """List workflow summaries in the namespace.

        Raises:
            Unauthenticated, NamespaceDenied, Unavailable, InvalidArgument,
            ServerError, Cancelled.
        """

        request = self._message("ListWorkflowsRequest")
        request.namespace = namespace or self.namespace
        if workflow_filter is not None:
            _assign_wire_envelope(request.filter, namespace or self.namespace, workflow_filter)
        response = await self._call("list", self._transport.list_workflows(request, self._metadata))
        return list(getattr(response, "summaries", []))

    async def describe(
        self,
        workflow_id: str,
        *,
        run_id: str | None = None,
        include_history: bool = False,
        namespace: str | None = None,
    ) -> WorkflowDescription:
        """Describe workflow state and optional event history.

        Raises:
            NotFound, Unauthenticated, NamespaceDenied, Unavailable,
            InvalidArgument, ServerError, Cancelled.
        """

        request = self._targeted_message("DescribeWorkflowRequest", namespace, workflow_id, run_id)
        request.include_history = include_history
        response = await self._call("describe", self._transport.describe_workflow(request, self._metadata))
        terminal_event = response.terminal_event if response.HasField("terminal_event") else None
        return WorkflowDescription(
            summary=response.summary,
            history=list(getattr(response, "history", [])),
            run_id=_id_value(response.run_id),
            history_head_seq=response.history_head_seq,
            terminal_event=terminal_event,
        )

    async def read_history(
        self,
        workflow_id: str,
        *,
        from_seq: int = 0,
        limit: int | None = None,
        namespace: str | None = None,
    ) -> HistoryPage:
        """Read one page of a workflow's event history from ``from_seq``.

        ``limit`` bounds the page when given; when omitted the field is left at
        the wire default and the server applies its own bound. Page through by
        passing each page's ``next_from_seq`` back as ``from_seq`` until it
        exceeds ``head_seq``.

        Raises:
            NotFound, Unauthenticated, NamespaceDenied, Unavailable,
            InvalidArgument, ServerError, Cancelled.
        """

        if not workflow_id:
            raise InvalidArgument("workflow_id must not be empty")
        request = self._message("ReadHistoryRequest")
        request.namespace = namespace or self.namespace
        _set_id(request.workflow_id, workflow_id)
        request.from_seq = from_seq
        if limit is not None:
            request.limit = limit
        response = await self._call("read_history", self._transport.read_history(request, self._metadata))
        return HistoryPage(
            events=list(getattr(response, "events", [])),
            next_from_seq=response.next_from_seq,
            head_seq=response.head_seq,
        )

    def handle(self, workflow_id: str, *, run_id: str | None = None, namespace: str | None = None) -> WorkflowHandle:
        """Construct a bare-ID workflow handle targeting latest or a concrete run.

        Raises:
            InvalidArgument: workflow_id is empty.
        """

        if not workflow_id:
            raise InvalidArgument("workflow_id must not be empty")
        return WorkflowHandle(
            client=self,
            workflow_id=workflow_id,
            run_id=run_id,
            namespace=namespace or self.namespace,
        )

    def _targeted_message(self, name: str, namespace: str | None, workflow_id: str, run_id: str | None) -> Any:
        if not workflow_id:
            raise InvalidArgument("workflow_id must not be empty")
        request = self._message(name)
        request.namespace = namespace or self.namespace
        _set_id(request.workflow_id, workflow_id)
        if run_id is not None:
            _set_id(request.run_id, run_id)
        return request

    def _message(self, name: str) -> Any:
        message_factory = getattr(self._transport, "message", None)
        if callable(message_factory):
            return message_factory(name)
        return GrpcWorkflowTransport.message(name)

    def _call_metadata(self, *, idempotency_key: str | None = None) -> MappingMetadata:
        if idempotency_key is None:
            return self._metadata
        return (*self._metadata, ("x-aion-idempotency-key", idempotency_key))

    async def _call(self, operation: str, awaitable: Any) -> Any:
        try:
            return await awaitable
        except BaseException as exc:
            if isinstance(exc, AionClientError):
                raise
            raise_mapped(exc, operation=operation)


def _idempotency_conflict() -> AlreadyExists:
    """The SDK-boundary idempotency conflict: the same key was reused with a
    different start request."""

    return AlreadyExists(
        "idempotency key was already used by a different start request "
        "(namespace, workflow type, input, routing key, or task queue differ)"
    )


def _reject_blank_selection(field: str, value: str | None) -> None:
    """Refuse a present-but-blank start selection.

    `None` is the only way to say "no selection"; a blank string is a caller
    mistake and is reported as one. The server's own handling of a blank differs
    by field, and refusing here gives every field the same answer before a round
    trip:

    * `routing_key` and `task_queue` — the server trims them and reads a blank
      as "not selected", falling back to the default placement or the
      namespace's default queue. A caller who believed they had chosen one would
      silently get neither, with no error.
    * `display_name` — the server REFUSES a present-but-blank name with
      `invalid_input`; it does NOT read one as unnamed. Absence is how a start
      says "unnamed". Forwarding a blank would only buy a round trip to the same
      refusal.
    """

    if value is not None and not value.strip():
        raise InvalidArgument(
            f"{field} must not be empty or whitespace; pass None to mean "
            f"no {field} explicitly"
        )


def _run_outcome(response: Any) -> RunOutcome:
    """Decode a reopen/pause/resume response into its run and projected status.

    A missing run id is a SERVER fault, not a caller one: the caller asked to
    transition a run and is entitled to know which run moved, so there is nothing
    sensible to return without it and nothing the caller could have done
    differently. (``start`` reports the same shape as ``InvalidArgument`` through
    ``_id_value``; that is its established contract and is left alone here rather
    than changed underneath it.)
    """

    uuid = getattr(getattr(response, "run_id", None), "uuid", None)
    if not isinstance(uuid, str) or not uuid:
        raise ServerError("server returned a lifecycle response with no run id; the transitioned run is unidentifiable")
    return RunOutcome(run_id=uuid, status=workflow_status_name(getattr(response, "status", None)))


def _normalize_tls(tls: TLSConfig | bool | None) -> TLSConfig:
    if tls is None:
        return TLSConfig()
    if isinstance(tls, bool):
        return TLSConfig(enabled=tls)
    return tls


def _id_value(value: Any) -> str:
    uuid = getattr(value, "uuid", None)
    if not isinstance(uuid, str) or not uuid:
        raise InvalidArgument("server returned an empty workflow/run id")
    return uuid


def _set_id(target: Any, value: str) -> None:
    if not value:
        raise InvalidArgument("id value must not be empty")
    target.uuid = value


def _which_oneof(message: Any, name: str) -> str | None:
    which = getattr(message, "WhichOneof", None)
    if callable(which):
        result = which(name)
        return str(result) if result is not None else None
    if getattr(message, "error", None) is not None:
        return "error"
    if getattr(message, "result", None) is not None:
        return "result"
    return None


def _assign_wire_envelope(envelope: Any, namespace: str, value: Any) -> None:
    envelope.namespace = namespace
    if isinstance(value, Payload):
        assign_payload(envelope.payload, value)
    else:
        assign_payload(envelope.payload, ensure_payload(cast(JSONValue, value)))
