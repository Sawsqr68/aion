"""The caller operations the fossilized stubs had made unreachable.

Every test here covers a surface that existed in the wire contract and in the
Rust caller SDK while the Python client could not reach it, because the committed
protobuf stubs predated it: the two start-routing dimensions, and the
reopen/pause/resume lifecycle triple. `test_proto_contract.py` keeps the stubs
current; these keep the SDK actually using what the stubs now describe, which is
the half that regeneration alone does not deliver.

The transport is faked rather than dialled so the request objects can be
inspected. They are REAL generated protobuf messages, though — the fake only
replaces the channel — so a field the SDK sets that the contract does not have
fails here instead of at the far end of a socket.
"""

from __future__ import annotations

from typing import Any

import pytest

from aion_client import (
    AlreadyExists,
    Client,
    DisplayNameNotApplied,
    InvalidArgument,
    RunOutcome,
    ServerError,
)
from aion_client.proto import common_pb2, workflow_pb2
from aion_client.transport import GrpcWorkflowTransport, MappingMetadata


class _RecordingTransport:
    """Captures each request and returns scripted responses.

    Real generated messages are built through the same factory the gRPC transport
    uses, so the messages under test are the shipped ones.
    """

    def __init__(self, **responses: Any) -> None:
        self.requests: list[tuple[str, Any, MappingMetadata]] = []
        self._responses = responses
        self.closed = False

    @staticmethod
    def message(name: str) -> Any:
        return GrpcWorkflowTransport.message(name)

    def _record(self, operation: str, request: Any, metadata: MappingMetadata) -> Any:
        self.requests.append((operation, request, metadata))
        if operation not in self._responses:
            raise AssertionError(f"the test scripted no {operation} response")
        return self._responses[operation]

    async def start_workflow(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("start_workflow", request, metadata)

    async def signal(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("signal", request, metadata)

    async def query(self, request: Any, metadata: MappingMetadata, timeout: float | None = None) -> Any:
        return self._record("query", request, metadata)

    async def cancel(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("cancel", request, metadata)

    async def reopen(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("reopen", request, metadata)

    async def pause(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("pause", request, metadata)

    async def resume(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("resume", request, metadata)

    async def list_workflows(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("list_workflows", request, metadata)

    async def describe_workflow(self, request: Any, metadata: MappingMetadata) -> Any:
        return self._record("describe_workflow", request, metadata)

    async def close(self) -> None:
        self.closed = True


def _start_response(workflow_id: str = "wf-1", run_id: str = "run-1") -> workflow_pb2.StartWorkflowResponse:
    return workflow_pb2.StartWorkflowResponse(
        workflow_id=common_pb2.WorkflowId(uuid=workflow_id),
        run_id=common_pb2.RunId(uuid=run_id),
    )


def _client(transport: _RecordingTransport) -> Client:
    return Client("http://127.0.0.1:50051", namespace="testing", transport=transport)


def _only_request(transport: _RecordingTransport) -> Any:
    assert len(transport.requests) == 1, f"expected exactly one call, got {[name for name, _, _ in transport.requests]}"
    return transport.requests[0][1]


async def test_start_sends_both_routing_dimensions() -> None:
    """The two fields the fossilized stubs did not have."""

    transport = _RecordingTransport(start_workflow=_start_response())
    await _client(transport).start(
        "testing.echo",
        {"message": "hello"},
        routing_key="tenant-42",
        task_queue="reports",
    )
    request = _only_request(transport)
    assert request.routing_key == "tenant-42"
    assert request.task_queue == "reports"
    assert request.HasField("routing_key")
    assert request.HasField("task_queue")


async def test_start_without_routing_leaves_both_fields_absent() -> None:
    """Absent, not empty.

    All three optional start fields are proto3 `optional`, so presence is
    observable on the wire, and the server distinguishes "chose nothing" from
    "chose blank" TODAY: a present-but-blank `display_name` is refused with
    `invalid_input`, so sending `""` for an unnamed start would fail the start
    outright. For `routing_key` and `task_queue` a blank reads as "not
    selected", but it would still make the request claim a selection it is not
    carrying.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    await _client(transport).start("testing.echo", {"message": "hello"})
    request = _only_request(transport)
    assert not request.HasField("routing_key")
    assert not request.HasField("task_queue")
    assert not request.HasField("display_name")


async def test_start_sends_the_display_name_it_was_given() -> None:
    """The operator-facing label reaches the wire, exactly as passed (#211)."""

    transport = _RecordingTransport(start_workflow=_start_response())
    outcome = await _client(transport).start(
        "testing.echo",
        {"message": "hello"},
        display_name="July invoice run",
    )
    request = _only_request(transport)
    assert request.HasField("display_name")
    assert request.display_name == "July invoice run"
    # A first start applies the name it asked for, so there is nothing to report.
    assert outcome.display_name_not_applied is None


@pytest.mark.parametrize("field", ["routing_key", "task_queue", "display_name"])
@pytest.mark.parametrize("blank", ["", "   ", "\t"])
async def test_start_refuses_a_blank_selection(field: str, blank: str) -> None:
    """A blank is never how a caller says "no selection", so it is refused here.

    `routing_key` and `task_queue` are trimmed by the server and read as "not
    selected", so forwarding a blank would hand back the default placement or
    the namespace default queue while every signal said the request had been
    honoured. `display_name` is different: the server REFUSES a present-but-blank
    name with `invalid_input` rather than starting the run unnamed, so
    forwarding one would only buy a round trip to the same refusal. Either way
    `None` is the way to mean "no selection", and the SDK gives all three fields
    the same answer.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    selection: dict[str, Any] = {field: blank}
    with pytest.raises(InvalidArgument, match=f"{field} must not be empty"):
        await _client(transport).start("testing.echo", None, **selection)
    assert transport.requests == [], "a refused start must not reach the transport"


async def test_start_idempotency_key_replays_an_identical_routed_start() -> None:
    transport = _RecordingTransport(start_workflow=_start_response())
    client = _client(transport)
    first = await client.start("testing.echo", {"n": 1}, idempotency_key="k", routing_key="a", task_queue="q")
    second = await client.start("testing.echo", {"n": 1}, idempotency_key="k", routing_key="a", task_queue="q")
    assert first.handle is second.handle
    assert second.display_name_not_applied is None
    assert len(transport.requests) == 1, "an identical retry must not reach the server again"


async def test_start_trims_the_display_name_before_sending_it() -> None:
    """The wire carries the trimmed name, because that is what the run will wear.

    The server trims before recording, so an untrimmed name sent from here would
    leave this SDK remembering a string the run does not have.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    await _client(transport).start("testing.echo", None, display_name="  Nightly settlement  ")
    assert _only_request(transport).display_name == "Nightly settlement"


@pytest.mark.parametrize("replayed_name", ["  Nightly settlement  ", "Nightly settlement", "\tNightly settlement\n"])
async def test_whitespace_around_a_replayed_name_is_not_a_dropped_name(replayed_name: str) -> None:
    """Whitespace the server would have erased must not be reported as a difference.

    Both sides of the replay comparison are trimmed, so `"  X  "` and `"X"` are
    the same name — the run wears `"X"` either way. Reporting that as a name that
    could not be applied would be a false alarm about a drop that never happened,
    and false alarms are how a real report gets ignored.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    client = _client(transport)
    await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name="Nightly settlement")
    replayed = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=replayed_name)

    assert replayed.display_name_not_applied is None
    assert len(transport.requests) == 1


@pytest.mark.parametrize("name", [None, "July invoice run"])
async def test_replaying_a_start_with_the_same_name_reports_nothing(name: str | None) -> None:
    """Same act, same label: nothing was dropped, so nothing is reported.

    The unnamed case is the one worth stating: two nameless starts must not
    manufacture a note about a name neither of them asked for.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    client = _client(transport)
    first = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=name)
    second = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=name)
    assert first.handle is second.handle
    assert first.display_name_not_applied is None
    assert second.display_name_not_applied is None
    assert len(transport.requests) == 1


@pytest.mark.parametrize(
    ("first_name", "second_name"),
    [
        ("July invoice run", "August invoice run"),
        (None, "August invoice run"),
    ],
)
async def test_replaying_a_start_with_a_different_name_returns_the_standing_run_and_says_so(
    first_name: str | None, second_name: str
) -> None:
    """A label carries no identity, so it does not split the idempotency key —
    but the name that did not get applied is REPORTED rather than dropped (#211).

    Two starts differing only in their display name are the same act: the
    fingerprint deliberately excludes the name, so the second dedupes to the
    first run instead of raising `AlreadyExists`. What must never happen is the
    quiet version of that — handing back a run wearing a name the caller did not
    ask for while saying nothing, or renaming the standing run behind their back.
    So the replay returns the EXISTING handle, the existing run keeps the name it
    already wears, and the outcome carries both sides of the difference.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    client = _client(transport)
    first = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=first_name)
    second = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=second_name)

    assert second.handle is first.handle, "a name difference must not start a second run"
    assert len(transport.requests) == 1, "a name difference is the same act; it must not reach the server again"
    assert second.display_name_not_applied == DisplayNameNotApplied(
        requested=second_name,
        standing=first_name,
    )
    # The standing run keeps its own name: nothing was renamed behind the caller.
    #
    # Presence, not the value: `display_name` is proto3 `optional`, so an absent
    # field and an explicitly-blank one both read back as `""`. Comparing the
    # value alone would collapse "started unnamed" into "started with a blank
    # name" — the exact distinction #211 turns on, and the one the server
    # refuses. So the unnamed case asserts ABSENCE on the wire.
    standing_request = transport.requests[0][1]
    if first_name is None:
        assert not standing_request.HasField("display_name")
    else:
        assert standing_request.HasField("display_name")
        assert standing_request.display_name == first_name


@pytest.mark.parametrize(
    ("standing", "requested", "expected"),
    [
        (None, None, None),
        ("Nightly", None, None),
        (None, "Nightly", DisplayNameNotApplied(requested="Nightly", standing=None)),
        ("Nightly", "Weekly", DisplayNameNotApplied(requested="Weekly", standing="Nightly")),
        ("Nightly", "Nightly", None),
    ],
)
async def test_the_unapplied_name_report_fires_only_for_a_replay_that_asked_for_a_name(
    standing: str | None, requested: str | None, expected: DisplayNameNotApplied | None
) -> None:
    """The WHOLE rule, one case per combination of what the replay asked for and
    what the standing run wears (#211).

    The report is raised only when the caller ASKED for a name the standing run
    does not wear. A caller that asked for nothing dropped nothing, so it is told
    nothing — the report's absence is exactly the statement that no requested
    name went missing, and firing it for an empty request would say the opposite
    of the truth. The Rust SDK pins the same five cases.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    client = _client(transport)
    first = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=standing)
    replayed = await client.start("testing.echo", {"n": 1}, idempotency_key="k", display_name=requested)

    assert first.display_name_not_applied is None, "a first start applies the name it asked for"
    assert replayed.handle is first.handle, "the label carries no identity, so the act deduped to one run"
    assert replayed.display_name_not_applied == expected
    assert len(transport.requests) == 1, "the replay must not reach the server again"


@pytest.mark.parametrize(
    ("first", "second"),
    [
        ({"routing_key": "a"}, {"routing_key": "b"}),
        ({"task_queue": "q"}, {"task_queue": "other"}),
        ({"routing_key": "a"}, {}),
        ({"task_queue": "q"}, {}),
    ],
)
async def test_reusing_an_idempotency_key_with_different_routing_conflicts(
    first: dict[str, Any], second: dict[str, Any]
) -> None:
    """Routing is part of the request, so it is part of the idempotency identity.

    Without this, a key reused with a different task queue would return the FIRST
    workflow's handle and report success — the caller's second, different intent
    discarded in silence, with a handle in hand that looks like it was honoured.
    The reach restored by the regenerated stubs would have arrived pre-broken.
    """

    transport = _RecordingTransport(start_workflow=_start_response())
    client = _client(transport)
    await client.start("testing.echo", {"n": 1}, idempotency_key="k", **first)
    with pytest.raises(AlreadyExists, match="routing key, or task queue differ"):
        await client.start("testing.echo", {"n": 1}, idempotency_key="k", **second)


@pytest.mark.parametrize(
    ("operation", "message", "response_type", "projected"),
    [
        ("reopen", "ReopenRequest", workflow_pb2.ReopenResponse, common_pb2.WORKFLOW_STATUS_RUNNING),
        ("pause", "PauseRequest", workflow_pb2.PauseResponse, common_pb2.WORKFLOW_STATUS_PAUSED),
        ("resume", "ResumeRequest", workflow_pb2.ResumeResponse, common_pb2.WORKFLOW_STATUS_RUNNING),
    ],
)
async def test_lifecycle_operations_target_a_run_and_return_its_projected_status(
    operation: str, message: str, response_type: Any, projected: int
) -> None:
    """The RPC triple that was absent from the committed stubs entirely."""

    response = response_type(run_id=common_pb2.RunId(uuid="run-9"), status=projected)
    transport = _RecordingTransport(**{operation: response})
    client = _client(transport)
    outcome = await getattr(client, operation)("wf-1", run_id="run-9")

    request = _only_request(transport)
    assert type(request).__name__ == message
    assert request.namespace == "testing"
    assert request.workflow_id.uuid == "wf-1"
    assert request.run_id.uuid == "run-9"
    assert outcome == RunOutcome(run_id="run-9", status=common_pb2.WorkflowStatus.Name(projected).removeprefix(
        "WORKFLOW_STATUS_"
    ))


async def test_pause_carries_the_operator_reason() -> None:
    transport = _RecordingTransport(
        pause=workflow_pb2.PauseResponse(
            run_id=common_pb2.RunId(uuid="run-9"),
            status=common_pb2.WORKFLOW_STATUS_PAUSED,
        )
    )
    await _client(transport).pause("wf-1", reason="holding for a store migration")
    assert _only_request(transport).reason == "holding for a store migration"


@pytest.mark.parametrize("operation", ["reopen", "pause", "resume"])
async def test_lifecycle_operations_omit_run_id_to_mean_latest(operation: str) -> None:
    """No run id means the latest run, and must not be sent as a blank one."""

    response_type = {
        "reopen": workflow_pb2.ReopenResponse,
        "pause": workflow_pb2.PauseResponse,
        "resume": workflow_pb2.ResumeResponse,
    }[operation]
    transport = _RecordingTransport(
        **{operation: response_type(run_id=common_pb2.RunId(uuid="latest"), status=common_pb2.WORKFLOW_STATUS_RUNNING)}
    )
    await getattr(_client(transport), operation)("wf-1")
    assert not _only_request(transport).HasField("run_id")


@pytest.mark.parametrize("operation", ["reopen", "pause", "resume"])
async def test_lifecycle_operations_refuse_an_empty_workflow_id(operation: str) -> None:
    transport = _RecordingTransport()
    with pytest.raises(InvalidArgument, match="workflow_id must not be empty"):
        await getattr(_client(transport), operation)("")
    assert transport.requests == []


async def test_a_lifecycle_response_without_a_run_id_is_a_server_fault() -> None:
    """The caller asked which run moved; a response that does not say has failed."""

    transport = _RecordingTransport(pause=workflow_pb2.PauseResponse(status=common_pb2.WORKFLOW_STATUS_PAUSED))
    with pytest.raises(ServerError, match="no run id"):
        await _client(transport).pause("wf-1")


async def test_an_unspecified_projected_status_is_a_server_fault() -> None:
    """Status 0 is proto3 padding, not an answer.

    Reporting the projected status is the entire reason these RPCs have response
    bodies instead of being empty acks, so a server that omits it has not answered
    the question — and guessing `RUNNING` would be inventing the one fact the
    caller came for.
    """

    transport = _RecordingTransport(resume=workflow_pb2.ResumeResponse(run_id=common_pb2.RunId(uuid="run-9")))
    with pytest.raises(ServerError, match="unspecified workflow status"):
        await _client(transport).resume("wf-1")


async def test_a_status_newer_than_the_committed_contract_names_the_stubs() -> None:
    """An unknown status must say WHY it is unknown.

    A status value this SDK's stubs do not define means the server is ahead of the
    committed wire contract — the exact condition that went unnoticed for as long
    as it did. The error names the regeneration command rather than reporting an
    anonymous failure.
    """

    unknown = max(common_pb2.WorkflowStatus.values()) + 1
    transport = _RecordingTransport(
        # Assigning an out-of-range value to a proto enum field is rejected by
        # protobuf itself, so the newer-server case is staged with a stand-in
        # response object carrying the raw wire value.
        reopen=_RawStatusResponse(run_id=common_pb2.RunId(uuid="run-9"), status=unknown)
    )
    with pytest.raises(ServerError, match="proto_codegen"):
        await _client(transport).reopen("wf-1")


class _RawStatusResponse:
    """A lifecycle response carrying a status value protobuf would refuse to hold."""

    def __init__(self, run_id: common_pb2.RunId, status: object) -> None:
        self.run_id = run_id
        self.status = status


@pytest.mark.parametrize(
    ("spelling", "expected"),
    [
        # The three textual spellings in circulation, all resolved against the
        # generated descriptor rather than string-munged into something plausible.
        ("WORKFLOW_STATUS_TIMED_OUT", "TIMED_OUT"),
        ("TIMED_OUT", "TIMED_OUT"),
        ("TimedOut", "TIMED_OUT"),
        ("ContinuedAsNew", "CONTINUED_AS_NEW"),
        ("Paused", "PAUSED"),
    ],
)
async def test_a_textual_status_is_resolved_in_every_spelling(spelling: str, expected: str) -> None:
    """Naive upper-casing turns `TimedOut` into `TIMEDOUT`, which is not a status.

    A non-gRPC transport or a test double may carry the decoded name rather than
    the enum value. Reporting a mangled name as a real status would be worse than
    failing, so each spelling is checked against the contract.
    """

    transport = _RecordingTransport(reopen=_RawStatusResponse(common_pb2.RunId(uuid="run-9"), spelling))
    assert (await _client(transport).reopen("wf-1")).status == expected


async def test_a_textual_status_outside_the_contract_is_refused() -> None:
    """A plausible-looking name that the contract does not define is not a status."""

    transport = _RecordingTransport(reopen=_RawStatusResponse(common_pb2.RunId(uuid="run-9"), "TIMEDOUT"))
    with pytest.raises(ServerError, match="not a status in this SDK's committed wire contract"):
        await _client(transport).reopen("wf-1")


async def test_a_status_of_the_wrong_type_is_refused() -> None:
    """A response whose status field is absent carries no status at all."""

    transport = _RecordingTransport(reopen=_RawStatusResponse(common_pb2.RunId(uuid="run-9"), None))
    with pytest.raises(ServerError, match="carries no status"):
        await _client(transport).reopen("wf-1")


async def test_handle_bound_lifecycle_operations_target_the_handle_s_run() -> None:
    """The handle forms must not silently widen to the latest run."""

    transport = _RecordingTransport(
        start_workflow=_start_response(workflow_id="wf-1", run_id="run-1"),
        pause=workflow_pb2.PauseResponse(
            run_id=common_pb2.RunId(uuid="run-1"),
            status=common_pb2.WORKFLOW_STATUS_PAUSED,
        ),
        resume=workflow_pb2.ResumeResponse(
            run_id=common_pb2.RunId(uuid="run-1"),
            status=common_pb2.WORKFLOW_STATUS_RUNNING,
        ),
        reopen=workflow_pb2.ReopenResponse(
            run_id=common_pb2.RunId(uuid="run-1"),
            status=common_pb2.WORKFLOW_STATUS_RUNNING,
        ),
    )
    client = _client(transport)
    handle = (await client.start("testing.echo", None)).handle

    assert (await handle.pause(reason="held")).status == "PAUSED"
    assert (await handle.resume()).status == "RUNNING"
    assert (await handle.reopen()).status == "RUNNING"

    for operation, request, _ in transport.requests[1:]:
        assert request.workflow_id.uuid == "wf-1", operation
        assert request.run_id.uuid == "run-1", operation
        assert request.namespace == "testing", operation
