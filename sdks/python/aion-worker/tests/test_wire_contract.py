"""The SDK must speak the CURRENT worker protocol, not a remembered one.

Every test here pins a fact the server actually enforces. They exist because
the SDK once passed its whole suite against committed protobuf stubs that no
longer matched the wire: `RegisterWorker.namespace` had become
`namespaces`/`task_queue`/`node`/`activities`/`identity`, `ActivityResult` had
grown a required `run_id` and `completion_token`, and nothing went red. A test
that only checks what the SDK sends against what the SDK believes is not a
gate; these check against the field-level requirements named in the server.
"""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from typing import Any, cast

import pytest
from ar007_fakes import (
    COMPLETION_TOKEN,
    IDEMPOTENCY_KEY,
    FakeGrpcStream,
    FakeSession,
    activity_id,
    config,
    payload,
    register_ack_frame,
    run_id,
    task,
    workflow_id,
)

import aion_worker
from aion_worker import (
    ActivityContext,
    ActivityRegistrationError,
    ActivityRegistry,
    ReconnectConfig,
    Worker,
    WorkerConfig,
)
from aion_worker.proto import common_pb2, worker_pb2

# --------------------------------------------------------------------------- #
# Defect 1 — the registration frame
# --------------------------------------------------------------------------- #


def _register_config() -> WorkerConfig:
    return WorkerConfig(
        endpoint="127.0.0.1:50051",
        task_queue="python_box",
        identity="image-worker-1",
        max_concurrency=2,
        reconnect=ReconnectConfig(initial_backoff_seconds=0.01, max_backoff_seconds=0.02, max_attempts=3),
        namespaces=("payments", "billing"),
        node="host-7",
        subject="worker-a",
    )


async def _captured_register(
    worker_config: WorkerConfig, descriptors: list[worker_pb2.ActivityDescriptor]
) -> worker_pb2.RegisterWorker:
    session = aion_worker.GrpcWorkerSession(worker_config)
    session._outbound = asyncio.Queue(maxsize=16)
    session._stream = FakeGrpcStream(frames=[register_ack_frame(namespace="payments")])
    await session.register(["inspect_image"], descriptors, ["inspect_image"])
    outbound = session._outbound
    assert outbound is not None
    message = await outbound.get()
    assert message is not None
    return message.register


async def test_register_frame_populates_all_six_contract_fields() -> None:
    """Every field the current `RegisterWorker` declares must carry a real value.

    The previous frame sent `namespace=<the task queue>` — a field that no
    longer exists, holding a value from a different routing dimension. Both
    halves of that are checked here: the six fields are present, and
    `task_queue` and `namespaces` are populated from their OWN config values.
    """

    descriptor = worker_pb2.ActivityDescriptor(
        name="inspect_image",
        input_schema_json='{"type": "object"}',
        output_schema_json='{"type": "object"}',
    )
    register = await _captured_register(_register_config(), [descriptor])

    assert list(register.namespaces) == ["payments", "billing"]
    assert list(register.activity_types) == ["inspect_image"]
    assert register.task_queue == "python_box"
    assert register.node == "host-7"
    assert register.identity == "image-worker-1"
    assert list(register.activities) == [descriptor]


async def test_registration_namespaces_match_the_authorization_metadata() -> None:
    """The registration scope and the auth header must be the SAME set.

    The server authorizes from `x-aion-namespaces` before the first frame and
    then indexes the worker under the frame's `namespaces`. If those disagree a
    worker is admitted into a namespace it was never granted, so they are built
    from one config field and compared here rather than trusted.
    """

    worker_config = _register_config()
    register = await _captured_register(worker_config, [])
    metadata = dict(worker_config.grpc_metadata())

    assert metadata["x-aion-namespaces"] == ",".join(register.namespaces)


def test_an_empty_namespace_set_is_refused_rather_than_sent() -> None:
    """A registration authorizing nothing is an author error, not a default."""

    with pytest.raises(ValueError, match="at least one namespace"):
        WorkerConfig(
            endpoint="127.0.0.1:50051",
            task_queue="q",
            identity="i",
            max_concurrency=1,
            reconnect=ReconnectConfig(initial_backoff_seconds=0.01, max_backoff_seconds=0.02, max_attempts=1),
            namespaces=(),
        )


def test_node_defaults_to_a_resolvable_locality() -> None:
    """`node` must never be blank by accident: empty means "unpinned only"."""

    assert aion_worker.default_node()
    assert config().node


# --------------------------------------------------------------------------- #
# Defect 2 — completion identity
# --------------------------------------------------------------------------- #


async def test_run_id_and_completion_token_are_echoed_verbatim_on_a_result() -> None:
    """The server rejects a result missing either, so both must survive the trip.

    `dispatch.rs` refuses "activity result run id is missing" and
    `envelope.rs` refuses a missing completion token, unconditionally. The
    token especially must be echoed BYTE FOR BYTE: it is the server's proof
    that this worker is still the live execution generation, and a worker that
    rebuilt it would be manufacturing its own authority to complete.
    """

    session = FakeSession()
    dispatcher = _EchoDispatcher()
    incoming = task(1, activity_type="echo")
    await session.push(incoming)
    await session.finish()

    await aion_worker.serve(config(), session, dispatcher)

    assert session.reported == [("result", run_id().uuid, COMPLETION_TOKEN)]


async def test_run_id_and_completion_token_are_echoed_on_a_failure() -> None:
    session = FakeSession()
    incoming = task(1, activity_type="boom")
    await session.push(incoming)
    await session.finish()

    await aion_worker.serve(config(), session, _FailingDispatcher())

    assert session.reported == [("failure", run_id().uuid, COMPLETION_TOKEN)]


async def test_a_replayed_report_carries_the_original_completion_identity() -> None:
    """A later session must re-report with the ORIGINAL task's identity.

    Re-deriving a token from the new session would be fabrication, and the
    server rejects a token it did not issue for that generation — so the
    unacked tracker has to carry the identity, not just the payload.
    """

    tracker = aion_worker.UnackedResultTracker()
    tracker.record(
        aion_worker.PendingCompletedReport(
            workflow_id=workflow_id(),
            activity_id=activity_id(4),
            run_id=run_id(),
            completion_token=COMPLETION_TOKEN,
            output=payload(),
        )
    )
    session = FakeSession()

    await aion_worker.re_report_unacked(session, tracker)

    assert session.reported == [("result", run_id().uuid, COMPLETION_TOKEN)]


def test_the_activity_context_exposes_the_run_and_idempotency_key() -> None:
    """A handler needs the effect key to make an outside-world effect once."""

    incoming = task(1).task
    context = ActivityContext(
        workflow_id=incoming.workflow_id,
        activity_id=incoming.activity_id,
        run_id=incoming.run_id,
        attempt=incoming.attempt,
        idempotency_key=incoming.idempotency_key,
        session=FakeSession(),
    )

    assert context.run_id.uuid == run_id().uuid
    assert context.idempotency_key == IDEMPOTENCY_KEY


# --------------------------------------------------------------------------- #
# Defect 2 — malformed inbound tasks are refused, never defaulted
# --------------------------------------------------------------------------- #


def _wire_task(**overrides: object) -> worker_pb2.ActivityTask:
    fields: dict[str, Any] = {
        "workflow_id": common_pb2.WorkflowId(uuid="workflow-1"),
        "activity_id": common_pb2.ActivityId(sequence_position=1),
        "run_id": common_pb2.RunId(uuid="run-1"),
        "activity_type": "inspect_image",
        "input": common_pb2.Payload(content_type="application/json", bytes=b"{}"),
        "attempt": 1,
        "completion_token": "generation-1",
        "idempotency_key": "effect-key",
    }
    fields.update(cast(dict[str, Any], overrides))
    return worker_pb2.ActivityTask(**{k: v for k, v in fields.items() if v is not None})


def test_a_well_formed_wire_task_decodes_every_field() -> None:
    wire = _wire_task()
    wire.labels["brief"] = "IP-001"

    decoded = aion_worker.ActivityTask.from_proto(wire)

    assert decoded.workflow_id.uuid == "workflow-1"
    assert decoded.activity_id.sequence_position == 1
    assert decoded.run_id.uuid == "run-1"
    assert decoded.activity_type == "inspect_image"
    assert decoded.attempt == 1
    assert decoded.completion_token == "generation-1"
    assert decoded.idempotency_key == "effect-key"
    assert dict(decoded.labels) == {"brief": "IP-001"}


@pytest.mark.parametrize(
    ("overrides", "expected"),
    [
        ({"workflow_id": None}, "workflow_id"),
        ({"activity_id": None}, "activity_id"),
        ({"run_id": None}, "run_id"),
        ({"activity_type": ""}, "activity_type"),
        ({"input": None}, "input"),
        ({"completion_token": ""}, "completion_token"),
        ({"idempotency_key": ""}, "idempotency_key"),
    ],
)
def test_a_malformed_wire_task_is_refused_naming_the_field(overrides: dict[str, object], expected: str) -> None:
    """Proto3 gives every scalar a zero default, so absent and empty look alike.

    Accepting the zero would turn a server-side omission into a worker-side
    lie — the worker would run an activity it can never report on. Each field
    is refused by name instead, mirroring `TryFrom<ProtoActivityTask>` in the
    Rust SDK.
    """

    with pytest.raises(aion_worker.TransportError, match=expected):
        aion_worker.ActivityTask.from_proto(_wire_task(**overrides))


def test_attempt_zero_is_refused_because_attempts_are_one_based() -> None:
    """Zero is the proto3 default, which means the producer never stamped it."""

    with pytest.raises(aion_worker.TransportError, match="attempt"):
        aion_worker.ActivityTask.from_proto(_wire_task(attempt=0))


# --------------------------------------------------------------------------- #
# Defect 3 — the typed action surface
# --------------------------------------------------------------------------- #


@dataclass
class InspectImage:
    path: str


@dataclass
class ImageFacts:
    path: str
    format: str
    width: int
    height: int
    bytes: int


def _descriptor_for(handler: object, name: str = "action") -> worker_pb2.ActivityDescriptor:
    registry = ActivityRegistry()
    registry.register(name=name, func=cast(Any, handler))
    return registry.activity_descriptors()[0]


def test_dataclass_hints_produce_the_schema_shape_awl_contracts_carry() -> None:
    """The derived schemas must be structurally comparable to an AWL contract.

    AWL emits `{type: object, properties, required}` (plus `$schema` and
    `additionalProperties`, which the server's normalizer strips and tolerates
    respectively). Matching that shape is what makes admission a match rather
    than a near-miss.
    """

    def inspect_image(input: InspectImage) -> ImageFacts:
        raise AssertionError("registered for its hints; never invoked")

    descriptor = _descriptor_for(inspect_image, "inspect_image")

    assert json.loads(descriptor.input_schema_json) == {
        "type": "object",
        "properties": {"path": {"type": "string"}},
        "required": ["path"],
    }
    assert json.loads(descriptor.output_schema_json) == {
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "format": {"type": "string"},
            "width": {"type": "integer"},
            "height": {"type": "integer"},
            "bytes": {"type": "integer"},
        },
        "required": ["path", "format", "width", "height", "bytes"],
    }


def test_no_derived_schema_closes_the_object() -> None:
    """`additionalProperties: false` is never emitted, and that is deliberate.

    On the INPUT side the worker's schema is the permissive side of the
    server's subset check, so declaring the object closed would refuse
    contracts the handler can in fact serve. The hints do not prove closure
    either way, so claiming it would be claiming more than is known.
    """

    def inspect_image(input: InspectImage) -> ImageFacts:
        raise AssertionError("registered for its hints; never invoked")

    descriptor = _descriptor_for(inspect_image)

    assert "additionalProperties" not in descriptor.input_schema_json
    assert "additionalProperties" not in descriptor.output_schema_json


def test_an_input_hint_that_proves_nothing_advertises_the_permissive_schema() -> None:
    """Input compatibility is contravariant, so "accepts anything" is TRUE here.

    The codec hands an unconstrained hint straight to the handler without
    coercion, so the worker really does accept whatever arrives. Nothing is
    over-claimed, and the server admits it.
    """

    def loose(input: Any) -> ImageFacts:
        raise AssertionError("registered for its hints; never invoked")

    descriptor = _descriptor_for(loose)

    assert json.loads(descriptor.input_schema_json) is True


def test_a_handler_with_no_input_parameter_advertises_the_permissive_schema() -> None:
    def nothing_in() -> ImageFacts:
        raise AssertionError("registered for its hints; never invoked")

    descriptor = _descriptor_for(nothing_in)

    assert json.loads(descriptor.input_schema_json) is True


def test_registration_still_requires_a_return_hint() -> None:
    """An unannotated return is refused at decoration, before any derivation.

    The hints are now load-bearing twice over — advertisement and decoding — so
    the pre-existing demand for them is the first line of the same defence.
    """

    registry = ActivityRegistry()

    def no_return_hint(input: dict[str, object]) -> None:
        return None

    # The missing return annotation IS the case under test, so it is removed at
    # runtime rather than written out: a source-level omission would just be a
    # type error in this file, which proves nothing about the SDK.
    del no_return_hint.__annotations__["return"]

    with pytest.raises(ActivityRegistrationError, match="return type hint"):
        registry.register(name="no_return_hint", func=no_return_hint)


def test_a_return_hint_that_proves_nothing_is_refused_locally_by_name() -> None:
    """Output compatibility is covariant, so the permissive schema is unusable.

    ``true`` would claim the handler may emit anything, which no real contract
    permits — the server would refuse the registration with a schema diff far
    from its cause. ``false`` would slip PAST the subset check, buying
    admission with a lie in the other direction. So the only honest options are
    a real schema or a refusal, and refusing here names the activity and tells
    the author what to change.
    """

    def opaque(input: InspectImage) -> Any:
        raise AssertionError("registered for its hints; never invoked")

    registry = ActivityRegistry()
    registry.register(name="opaque", func=cast(Any, opaque))

    with pytest.raises(ActivityRegistrationError) as raised:
        registry.activity_descriptors()

    message = str(raised.value)
    assert "opaque" in message, "the refusal must name the activity"
    assert "covariant" in message, "the refusal must say why the server would reject it"


def test_one_un_advertisable_return_refuses_the_WHOLE_registration() -> None:
    """A partially true advertisement is worse than none at all.

    The server only checks the actions a deployed contract names, so dropping
    just the un-derivable descriptor would let the worker be ADMITTED on the
    strength of its other activities and then dispatched the one whose contract
    was never checked — the exact "admitted, then mismatched at dispatch"
    failure a missing descriptor is meant to prevent.
    """

    def good(input: InspectImage) -> ImageFacts:
        raise AssertionError("registered for its hints; never invoked")

    def bad(input: InspectImage) -> Any: ...

    registry = ActivityRegistry()
    registry.register(name="good", func=cast(Any, good))
    registry.register(name="bad", func=cast(Any, bad))

    with pytest.raises(ActivityRegistrationError, match="bad"):
        registry.activity_descriptors()


def test_the_refusal_happens_before_the_worker_dials() -> None:
    """A `Worker` must not be constructible if it cannot advertise itself."""

    registry = ActivityRegistry()

    def bad(input: InspectImage) -> Any:
        raise AssertionError("registered for its hints; never invoked")

    registry.register(name="bad", func=cast(Any, bad))

    with pytest.raises(ActivityRegistrationError):
        Worker(config(), registry=registry)


async def test_a_worker_advertises_its_descriptors_at_registration() -> None:
    """The whole point of the derivation: it must reach the register frame."""

    registry = ActivityRegistry()

    def inspect_image(input: InspectImage) -> ImageFacts:
        raise AssertionError("registered for its hints; never invoked")

    registry.register(name="inspect_image", func=cast(Any, inspect_image))
    worker = Worker(config(), registry=registry)
    session = FakeSession()
    await session.finish()

    await worker.run_with_session(session)

    assert [descriptor.name for descriptor in session.advertised[0]] == ["inspect_image"]


# --------------------------------------------------------------------------- #
# dispatch honours the advertisement
# --------------------------------------------------------------------------- #


async def test_a_declared_input_type_is_constructed_not_handed_over_raw() -> None:
    """Advertising `InspectImage` and passing a `dict` would be a broken promise."""

    seen: list[object] = []
    registry = ActivityRegistry()

    @registry.activity(name="inspect_image")
    def inspect_image(input: InspectImage) -> ImageFacts:
        seen.append(input)
        return ImageFacts(path=input.path, format="png", width=1, height=2, bytes=3)

    outcome = await _dispatch(registry, "inspect_image", {"path": "/tmp/red.png"})

    assert isinstance(outcome, aion_worker.Completed)
    assert seen == [InspectImage(path="/tmp/red.png")]
    assert json.loads(outcome.output.bytes) == {
        "path": "/tmp/red.png",
        "format": "png",
        "width": 1,
        "height": 2,
        "bytes": 3,
    }


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ({"pathx": "y"}, "has no field 'pathx'"),
        ({}, "missing required field 'path'"),
        ({"path": 12}, "must be a string"),
        ("just-a-string", "must be an object"),
    ],
)
async def test_a_payload_that_does_not_fit_is_terminal_and_names_the_field(value: object, expected: str) -> None:
    """The payload came off the wire; no retry changes it, so it is terminal.

    And the message names the field, because a `TypeError` raised from inside
    dataclass construction tells an operator nothing about which key was wrong.
    """

    registry = ActivityRegistry()

    @registry.activity(name="inspect_image")
    def inspect_image(input: InspectImage) -> ImageFacts:
        raise AssertionError("the handler must never be reached")

    outcome = await _dispatch(registry, "inspect_image", value)

    assert isinstance(outcome, aion_worker.Failed)
    assert outcome.failure.kind == worker_pb2.ACTIVITY_ERROR_KIND_TERMINAL
    assert expected in outcome.failure.message


async def test_an_unencodable_return_value_is_terminal_not_retryable() -> None:
    """A handler returning something unserialisable is a code defect.

    Retrying re-runs the same code and produces the same unencodable value, so
    classifying it retryable would burn the activity's whole retry budget on a
    bug that no attempt can clear.
    """

    registry = ActivityRegistry()

    @registry.activity(name="leaky")
    def leaky(input: dict[str, object]) -> dict[str, object]:
        return cast("dict[str, object]", {"handle": object()})

    outcome = await _dispatch(registry, "leaky", {})

    assert isinstance(outcome, aion_worker.Failed)
    assert outcome.failure.kind == worker_pb2.ACTIVITY_ERROR_KIND_TERMINAL
    assert "cannot be encoded" in outcome.failure.message


async def test_a_subclass_of_terminal_error_is_classified_terminally() -> None:
    """Classification is by `isinstance`, so authors can classify by type.

    A handler that declares `class UnreadableImage(TerminalError)` and raises
    it everywhere is classifying once, in the type, instead of remembering at
    each raise site.
    """

    class UnreadableImage(aion_worker.TerminalError):
        pass

    registry = ActivityRegistry()

    @registry.activity(name="unreadable")
    def unreadable(input: dict[str, object]) -> dict[str, object]:
        raise UnreadableImage("not an image")

    outcome = await _dispatch(registry, "unreadable", {})

    assert isinstance(outcome, aion_worker.Failed)
    assert outcome.failure.kind == worker_pb2.ACTIVITY_ERROR_KIND_TERMINAL


async def test_an_unclassified_exception_defaults_to_retryable() -> None:
    """An I/O error on a busy volume can succeed on a second attempt.

    Handlers rely on this default to leave genuinely transient failures
    unclassified rather than flattening them into the terminal class.
    """

    registry = ActivityRegistry()

    @registry.activity(name="busy")
    def busy(input: dict[str, object]) -> dict[str, object]:
        raise OSError("volume busy")

    outcome = await _dispatch(registry, "busy", {})

    assert isinstance(outcome, aion_worker.Failed)
    assert outcome.failure.kind == worker_pb2.ACTIVITY_ERROR_KIND_RETRYABLE


async def _dispatch(registry: ActivityRegistry, activity_type: str, value: object) -> aion_worker.DispatchOutcome:
    incoming = aion_worker.ActivityTask(
        workflow_id=workflow_id(),
        activity_id=activity_id(1),
        run_id=run_id(),
        activity_type=activity_type,
        input=common_pb2.Payload(content_type="application/json", bytes=json.dumps(value).encode("utf-8")),
        attempt=1,
        completion_token=COMPLETION_TOKEN,
        idempotency_key=IDEMPOTENCY_KEY,
    )
    context = ActivityContext(
        workflow_id=incoming.workflow_id,
        activity_id=incoming.activity_id,
        run_id=incoming.run_id,
        attempt=incoming.attempt,
        idempotency_key=incoming.idempotency_key,
        session=FakeSession(),
    )
    return await registry.dispatch(incoming, context)


class _EchoDispatcher:
    def activity_types(self) -> list[str]:
        return ["echo"]

    async def dispatch(
        self, incoming: aion_worker.ActivityTask, context: ActivityContext
    ) -> aion_worker.DispatchOutcome:
        del context
        return aion_worker.Completed(incoming.input)


class _FailingDispatcher:
    def activity_types(self) -> list[str]:
        return ["boom"]

    async def dispatch(
        self, incoming: aion_worker.ActivityTask, context: ActivityContext
    ) -> aion_worker.DispatchOutcome:
        del incoming, context
        return aion_worker.Failed(
            worker_pb2.ActivityError(kind=worker_pb2.ACTIVITY_ERROR_KIND_RETRYABLE, message="boom")
        )


async def test_the_codec_enforces_exactly_what_was_advertised_and_no_more() -> None:
    """A permissive advertisement must not be followed by a strict refusal.

    When the input hint proves nothing the worker tells the server "I accept
    whatever arrives". Rejecting a payload at dispatch would break that promise
    in the one direction the server cannot see, so an unenforceable hint passes
    its value straight through instead — the advertisement and the behaviour
    stay the same claim.
    """

    seen: list[object] = []
    registry = ActivityRegistry()

    @registry.activity(name="unenforceable")
    def unenforceable(input: set[int]) -> ImageFacts:
        seen.append(input)
        return ImageFacts(path="p", format="png", width=1, height=2, bytes=3)

    descriptor = registry.activity_descriptors()[0]
    assert json.loads(descriptor.input_schema_json) is True, (
        "a set has no JSON form, so nothing can be advertised about it"
    )

    outcome = await _dispatch(registry, "unenforceable", {"anything": [1, 2]})

    assert isinstance(outcome, aion_worker.Completed)
    assert seen == [{"anything": [1, 2]}], "the raw payload is what was promised"
