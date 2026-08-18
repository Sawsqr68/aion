"""#50 the liveness ping — and every other frame the server actually sends.

Since #197 the server probes every gRPC-delivered worker over its own task
stream and withdraws dispatch eligibility from any worker that does not answer.
`decode_server_message` had no `liveness_ping` branch, so the oneof reached the
`unsupported server-to-worker message` fall-through — raised out of the
`_receive_from_stream` async generator, which does not merely leave the ping
unanswered, it TEARS THE SESSION DOWN. The worker redials, is pinged again,
tears down again, and its probation never clears.

`cancel_activity` is the same fall-through and the same shape: the server sends
it (`aion-server/src/worker/activity_cancel.rs` `ask_worker_to_stop`), the
serve loop already knows how to deliver it to a running handler
(`loop.py` `_handle_control_event`), and only the decoder was missing — so
cooperative cancellation reached this SDK as a session teardown too, and the
existing `ActivityCancelled` machinery was reachable only from tests.

The answer is a RUNTIME obligation. No handler is consulted and no concurrency
permit is taken, mirroring `crates/aion-worker/src/runtime/loop_.rs`
`answer_ping`: the server is asking whether it can REACH this worker, and a
worker that is busy is reachable.
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import cast

import pytest
from ar007_fakes import (
    FakeGrpcStream,
    FakeSession,
    RecordingDispatcher,
    activity_id,
    config,
    register_ack_frame,
    task,
    workflow_id,
)
from grpc_lease_server import LeaseEnforcingWorkerProtocol, dispatchable_task, running_worker

import aion_worker
from aion_worker import (
    ActivityCancelled,
    ActivityDescriptor,
    ActivityExecutionContext,
    ActivityTask,
    Completed,
    DispatchOutcome,
    GrpcWorkerSession,
    encode_json_payload,
    serve,
)
from aion_worker.proto import worker_pb2
from aion_worker.session import decode_server_message

PING_WINDOW_MS = 30_000


def liveness_ping_frame(sequence: int, silence_window_ms: int = PING_WINDOW_MS) -> worker_pb2.ServerToWorker:
    return worker_pb2.ServerToWorker(
        liveness_ping=worker_pb2.LivenessPing(liveness_ping=sequence, silence_window_ms=silence_window_ms)
    )


def cancel_activity_frame(sequence_position: int) -> worker_pb2.ServerToWorker:
    return worker_pb2.ServerToWorker(
        cancel_activity=worker_pb2.CancelActivity(
            workflow_id=workflow_id(),
            activity_id=activity_id(sequence_position),
        )
    )


def test_a_liveness_ping_decodes_into_a_typed_event() -> None:
    """The sequence and the server's silence window both survive the decode."""

    event = decode_server_message(liveness_ping_frame(sequence=41, silence_window_ms=7_000))

    assert isinstance(event, aion_worker.LivenessPingReceived), f"a liveness ping decoded as {event!r}"
    assert event.sequence == 41
    assert event.silence_window_ms == 7_000


def test_a_cancel_activity_decodes_into_the_cancellation_event() -> None:
    """The server sends this frame; the serve loop already handles the event.

    Only the decoder was missing, so a cooperative cancellation arrived as an
    unsupported oneof and ended the session.
    """

    event = decode_server_message(cancel_activity_frame(sequence_position=5))

    assert isinstance(event, ActivityCancelled), f"a cancel_activity decoded as {event!r}"
    assert event.workflow_id.uuid == "workflow-1"
    assert event.activity_id.sequence_position == 5


async def test_a_liveness_ping_does_not_end_the_receive_stream() -> None:
    """The teardown itself: the ping must not raise out of the generator.

    A `TransportError` escaping `_receive_from_stream` is what the SDK's own
    reconnect machinery classifies as a retryable session drop, so the failure
    mode is invisible as an error and visible only as endless redialling.
    """

    session = GrpcWorkerSession(config())
    session._outbound = asyncio.Queue(maxsize=16)
    session._stream = FakeGrpcStream(
        frames=[
            liveness_ping_frame(sequence=1),
            liveness_ping_frame(sequence=2),
        ]
    )

    received = [event async for event in session.receive_tasks()]

    assert [event.sequence for event in received if isinstance(event, aion_worker.LivenessPingReceived)] == [1, 2], (
        f"the receive stream did not carry both pings through: {received!r}"
    )


async def test_the_session_answers_a_ping_with_the_echoed_sequence() -> None:
    """`LivenessAnswer.liveness_ping` echoes the asked sequence verbatim.

    A stale or invented sequence proves nothing: the server correlates the
    answer against the ping it actually sent.
    """

    session = GrpcWorkerSession(config())
    session._outbound = asyncio.Queue(maxsize=16)
    session._stream = FakeGrpcStream(frames=[register_ack_frame()])

    await session.answer_liveness_ping(9_001)

    outbound = session._outbound
    assert outbound is not None
    frame = outbound.get_nowait()
    assert frame is not None
    assert frame.WhichOneof("message") == "liveness_answer"
    assert frame.liveness_answer.liveness_ping == 9_001


@dataclass
class AnsweringFakeSession(FakeSession):
    """Fake session recording the runtime's liveness answers."""

    answers: list[int] = field(default_factory=list)

    async def answer_liveness_ping(self, sequence: int) -> None:
        self.answers.append(sequence)

    async def send_connection_heartbeat(self) -> None:
        return None


async def test_the_serve_loop_answers_the_ping_without_consulting_the_dispatcher() -> None:
    """The runtime answers; action code is never asked and never could be.

    The dispatcher is asserted untouched because "answered by the runtime" and
    "answered by whatever happened to be running" are different facts, and only
    the first survives an idle worker.
    """

    session = AnsweringFakeSession()
    dispatcher = RecordingDispatcher()
    await session.push(aion_worker.LivenessPingReceived(sequence=17, silence_window_ms=PING_WINDOW_MS))
    await session.push(aion_worker.LivenessPingReceived(sequence=18, silence_window_ms=PING_WINDOW_MS))
    await session.finish()

    end = await serve(config(max_concurrency=1), session, dispatcher)

    assert isinstance(end, aion_worker.StreamFinished)
    assert session.answers == [17, 18]
    assert dispatcher.started == 0, "the liveness ping reached action code"


async def test_a_worker_at_full_concurrency_still_answers() -> None:
    """No concurrency permit is taken: reachability is not work.

    A worker saturated with activities is still reachable, and a probe that
    could not tell the difference would withdraw eligibility from exactly the
    workers doing the most.
    """

    session = AnsweringFakeSession()
    release = asyncio.Event()
    dispatcher = RecordingDispatcher(release)
    await session.push(task(3))
    await session.push(aion_worker.LivenessPingReceived(sequence=55, silence_window_ms=PING_WINDOW_MS))

    serving = asyncio.create_task(serve(config(max_concurrency=1), session, dispatcher))
    try:
        loop = asyncio.get_running_loop()
        deadline = loop.time() + 5.0
        while not session.answers:
            assert not serving.done(), "serve ended before the ping was answered"
            assert loop.time() < deadline, "a busy worker never answered the liveness ping"
            await asyncio.sleep(0.005)
        assert session.answers == [55]
        assert dispatcher.active == 1, "the test did not actually hold the worker at full concurrency"
    finally:
        release.set()
        await session.finish()
        await serving


async def test_an_unsupported_oneof_still_refuses_loudly() -> None:
    """Decoding stays closed: an unknown frame is still a refusal, by name.

    Adding branches must not turn the decoder permissive — a frame this SDK
    genuinely cannot honour is a protocol mismatch an operator has to see.
    """

    with pytest.raises(aion_worker.TransportError, match="RegisterAck"):
        decode_server_message(register_ack_frame())

    with pytest.raises(aion_worker.TransportError, match="no payload set"):
        decode_server_message(worker_pb2.ServerToWorker())


PROBE_DESCRIPTORS = [ActivityDescriptor(name="probe", input_schema_json="true", output_schema_json="true")]


class _HeldDispatcher:
    """Holds its activity in flight until released or cancelled.

    Both exits are needed by the wire tests below: the probe tests want a task
    genuinely occupying the worker's only concurrency slot while the server
    pings it, and the cancellation test wants the handler to observe the
    cooperative cancellation the server sent. Whichever arrives first ends the
    handler, so no test can wedge the serve loop's final drain.
    """

    def __init__(self) -> None:
        self.release = asyncio.Event()
        self.cancellation_seen = asyncio.Event()
        self.started = 0

    def activity_types(self) -> list[str]:
        return ["probe"]

    async def dispatch(self, task: ActivityTask, context: ActivityExecutionContext) -> DispatchOutcome:
        self.started += 1
        cancelled = asyncio.ensure_future(context.cancelled())
        released = asyncio.ensure_future(self.release.wait())
        try:
            waits = cast(set[asyncio.Future[object]], {cancelled, released})
            done, _pending = await asyncio.wait(waits, return_when=asyncio.FIRST_COMPLETED)
            if cancelled in done:
                self.cancellation_seen.set()
        finally:
            cancelled.cancel()
            released.cancel()
        return Completed(encode_json_payload(task.activity_type))


async def _await_answers(service: LeaseEnforcingWorkerProtocol, at_least: int, timeout_seconds: float) -> None:
    loop = asyncio.get_running_loop()
    deadline = loop.time() + timeout_seconds
    while True:
        answered = sum(len(connection.liveness_answers) for connection in service.connections)
        if answered >= at_least:
            return
        if loop.time() >= deadline:
            raise AssertionError(
                f"{answered} of {len(service.pings_sent)} liveness pings were answered across "
                f"{len(service.connections)} connection(s) within {timeout_seconds}s"
            )
        await asyncio.sleep(0.005)


async def test_a_probed_worker_answers_every_round_and_keeps_one_connection() -> None:
    """The acceptance shape of #50, over a real gRPC stream.

    Three probe rounds answered on one connection is the operator-visible
    difference between a worker that clears its probation and one that redials
    forever without ever being dispatchable.
    """

    service = LeaseEnforcingWorkerProtocol(heartbeat_window_ms=2_000, ping_every_seconds=0.1)
    dispatcher = _HeldDispatcher()
    async with running_worker(service, dispatcher, PROBE_DESCRIPTORS, dispatcher.release) as running:
        await _await_answers(service, at_least=3, timeout_seconds=5.0)
        assert not running.done(), "the worker run ended while the probe was under observation"
        assert len(service.connections) == 1, (
            f"{len(service.connections)} connections were accepted while probing; a worker that "
            "answers the ping never redials"
        )
        connection = service.connections[0]
        answered = list(connection.liveness_answers)
        assert answered == service.pings_sent[: len(answered)], (
            f"answers {answered} did not echo the pings {service.pings_sent} in order"
        )
        assert dispatcher.started == 0, "the probe reached action code"


async def test_a_busy_worker_answers_the_probe_over_the_wire() -> None:
    """Dispatch eligibility must not depend on the worker being idle.

    The worker's single concurrency slot is occupied for the whole observation,
    so an answer that needed a permit could not be sent.
    """

    service = LeaseEnforcingWorkerProtocol(
        heartbeat_window_ms=2_000,
        ping_every_seconds=0.1,
        task_to_dispatch=dispatchable_task("workflow-probe", 1, "probe"),
    )
    dispatcher = _HeldDispatcher()
    async with running_worker(service, dispatcher, PROBE_DESCRIPTORS, dispatcher.release) as running:
        await _await_answers(service, at_least=3, timeout_seconds=5.0)
        assert not running.done(), "the worker run ended while the probe was under observation"
        assert len(service.connections) == 1, "a probed busy worker redialled"
        connection = service.connections[0]
        assert dispatcher.started == 1, "the harness never got its task in flight"
        assert connection.results == [], "the held activity reported a result; it was not in flight"
        assert connection.task_beats, "the in-flight activity was never heartbeaten"


async def test_a_cancel_activity_reaches_the_handler_over_the_wire() -> None:
    """The cancellation the decoder used to drop, end to end.

    Proves the frame is decoded, delivered to the in-flight handler's
    cancellation handle, and does not end the session.
    """

    service = LeaseEnforcingWorkerProtocol(
        heartbeat_window_ms=2_000,
        task_to_dispatch=dispatchable_task("workflow-1", 1, "probe"),
        cancel_after_seconds=0.1,
    )
    dispatcher = _HeldDispatcher()
    async with running_worker(service, dispatcher, PROBE_DESCRIPTORS, dispatcher.release) as running:
        await asyncio.wait_for(dispatcher.cancellation_seen.wait(), timeout=5.0)
        assert not running.done(), "the cancellation ended the worker run"
        assert len(service.connections) == 1, "the cancellation frame caused a redial"
        assert service.cancels_sent == [1]
