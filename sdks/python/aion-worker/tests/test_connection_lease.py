"""#26 the connection lease: an idle worker must beat, or it is deregistered.

The server advances a CONNECTION-level lease on every inbound frame and expires
a worker that lets a whole heartbeat window pass without one
(`aion-server/src/api/worker_grpc.rs`). Task heartbeats are not enough: a
healthy worker with nothing in flight sends none, so its lease runs out, it is
deregistered, its stream is closed, and it reconnects — every window, forever,
using reconnection as a keepalive.

The Rust runtime's contract (`crates/aion-worker/src/runtime/loop_.rs`
`pump_liveness`) is one empty-id CONNECTION heartbeat per pump tick, sent
BEFORE the per-task beats and sent whether or not anything is in flight. The
server explicitly accepts that frame as a pure lease beat
(`worker_grpc.rs:425-431`). This module holds the Python runtime to the same
contract, at both levels the suite can honestly reach: the frames the pump
emits against a fake, and the reconnect churn against a real gRPC server that
enforces a real lease.

The cadence is DERIVED, never configured: a quarter of the server's
`RegisterAck.heartbeat_window_ms`, so four beats fit in every window the
operator chose and the worker holds no second copy of the operator's number.
"""

from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field

from ar007_fakes import FakeSession, RecordingDispatcher, config, task
from grpc_lease_server import LeaseEnforcingWorkerProtocol, running_worker

from aion_worker import (
    ActivityDescriptor,
    ActivityExecutionContext,
    ActivityTask,
    Completed,
    DispatchOutcome,
    encode_json_payload,
    serve,
)
from aion_worker.proto import common_pb2

WINDOW_SECONDS = 0.5
"""Short enough for a test, long enough that a loaded box's scheduling jitter
cannot masquerade as a missed beat: the derived cadence is a quarter of it, so
a beat has to be late by 375ms — three whole cadences — before the lease it is
holding open could expire."""

WINDOWS_OBSERVED = 3
"""The issue's own acceptance bar: at least three heartbeat windows idle."""


@dataclass
class LeaseFakeSession(FakeSession):
    """Fake session recording BOTH heartbeat forms with their arrival times.

    Times, not just counts: the "a wedged event loop still expires" property is
    about WHEN a beat reached the wire, and a count cannot express it.
    """

    window_seconds: float | None = None
    connection_beats: list[float] = field(default_factory=list)
    task_beats: list[tuple[str, int, common_pb2.Payload | None, float]] = field(default_factory=list)

    def heartbeat_window_seconds(self) -> float | None:
        return self.window_seconds

    async def send_connection_heartbeat(self) -> None:
        self.connection_beats.append(time.monotonic())

    async def send_heartbeat(
        self,
        workflow: common_pb2.WorkflowId,
        activity: common_pb2.ActivityId,
        progress: common_pb2.Payload | None,
    ) -> None:
        self.task_beats.append((workflow.uuid, activity.sequence_position, progress, time.monotonic()))
        await super().send_heartbeat(workflow, activity, progress)


async def _await_count(probe: object, attribute: str, at_least: int, timeout_seconds: float) -> None:
    loop = asyncio.get_running_loop()
    deadline = loop.time() + timeout_seconds
    while len(getattr(probe, attribute)) < at_least:
        if loop.time() >= deadline:
            raise AssertionError(
                f"only {len(getattr(probe, attribute))} of {at_least} {attribute} arrived "
                f"within {timeout_seconds}s"
            )
        await asyncio.sleep(0.005)


async def test_an_idle_worker_beats_its_connection_lease() -> None:
    """Nothing in flight is exactly when the lease beat matters most."""

    window = 0.1
    session = LeaseFakeSession(window_seconds=window)
    dispatcher = RecordingDispatcher()

    serving = asyncio.create_task(serve(config(max_concurrency=1), session, dispatcher))
    try:
        # Four beats span more than a full window at the quarter-window
        # cadence, so an idle worker demonstrably holds its lease open.
        await _await_count(session, "connection_beats", 4, timeout_seconds=5.0)
        assert not serving.done(), "serve ended while the idle pump was under observation"
        assert session.task_beats == [], "an idle worker has no task to beat"
    finally:
        await session.finish()
        await serving


async def test_an_in_flight_worker_beats_the_connection_before_its_tasks() -> None:
    """A busy worker's lease is beaten too — the task beats do not replace it.

    Mirrors `pump_liveness`: the connection beat goes first on every tick, then
    one beat per in-flight activity. A worker running work still has to prove
    the CONNECTION is alive, because that is the lease the server holds.
    """

    window = 0.1
    session = LeaseFakeSession(window_seconds=window)
    release = asyncio.Event()
    dispatcher = RecordingDispatcher(release)
    await session.push(task(7))

    serving = asyncio.create_task(serve(config(max_concurrency=1), session, dispatcher))
    try:
        await _await_count(session, "task_beats", 3, timeout_seconds=5.0)
        assert not serving.done(), "serve ended while the pump was under observation"
        # Every tick sends the connection beat first, so there is never a task
        # beat that was not preceded by one.
        assert len(session.connection_beats) >= len(session.task_beats), (
            f"the connection lease was beaten {len(session.connection_beats)} times against "
            f"{len(session.task_beats)} task beats: the connection beat must lead every tick"
        )
        first_task_beat = session.task_beats[0][3]
        assert session.connection_beats[0] <= first_task_beat, (
            "the first task beat preceded the first connection beat"
        )
        assert all(
            uuid == "workflow-1" and sequence == 7 and progress is None
            for (uuid, sequence, progress, _at) in session.task_beats
        ), f"automatic task beats must target the in-flight task with no progress: {session.task_beats}"
    finally:
        release.set()
        await session.finish()
        await serving


async def test_the_cadence_is_a_quarter_of_the_server_assigned_window() -> None:
    """Derived from the `RegisterAck`, with no knob of its own.

    Measured as an interval rather than read from a constant: a test that
    asserted against the SDK's own arithmetic would pass for any cadence the
    SDK happened to pick. Four beats bound the elapsed time from both sides —
    a pump that beat once per window would take four windows to get here, and
    one that beat continuously would arrive far too early.
    """

    window = 0.4
    session = LeaseFakeSession(window_seconds=window)
    dispatcher = RecordingDispatcher()

    started = time.monotonic()
    serving = asyncio.create_task(serve(config(max_concurrency=1), session, dispatcher))
    try:
        await _await_count(session, "connection_beats", 4, timeout_seconds=10.0)
        elapsed = session.connection_beats[3] - started
        assert elapsed >= window, (
            f"four quarter-window beats took {elapsed:.3f}s, less than the {window}s window: "
            "the cadence is faster than a quarter window"
        )
        # Generous upper bound: scheduling jitter may stretch each tick, but a
        # half-window (or slower) cadence cannot fit four beats in two windows.
        assert elapsed < window * 2, (
            f"four quarter-window beats took {elapsed:.3f}s, more than two {window}s windows: "
            "the cadence is slower than a quarter window"
        )
    finally:
        await session.finish()
        await serving


async def test_a_blocked_event_loop_sends_no_beats_and_is_correctly_expired() -> None:
    """A wedged runtime must still expire — the pump is not a background thread.

    The pump is an asyncio task on the worker's own event loop, so a handler
    that blocks that loop stops the beats, and the server's lease expires the
    worker exactly as it should. This asserts the mechanism that makes that
    true: no beat's timestamp falls inside the blocked interval.
    """

    window = 0.1
    blocked_for = window * 3
    session = LeaseFakeSession(window_seconds=window)
    dispatcher = RecordingDispatcher()

    serving = asyncio.create_task(serve(config(max_concurrency=1), session, dispatcher))
    try:
        await _await_count(session, "connection_beats", 1, timeout_seconds=5.0)
        block_started = time.monotonic()
        # Synchronous sleep: the event loop cannot run the pump task at all.
        time.sleep(blocked_for)
        block_ended = time.monotonic()
        await asyncio.sleep(0)
        during_block = [at for at in session.connection_beats if block_started < at < block_ended]
        assert during_block == [], (
            f"{len(during_block)} beats were sent while the event loop was blocked; a wedged "
            "runtime must go silent so the server's lease can expire it"
        )
    finally:
        await session.finish()
        await serving


async def test_a_session_without_a_server_window_never_beats() -> None:
    """No `RegisterAck` window means no pump — including no connection beat.

    The window is the server's contract; a session that was never given one
    (every plain fake, and any transport that does not register) has no lease
    to hold open and must not invent a cadence for itself.
    """

    session = LeaseFakeSession(window_seconds=None)
    dispatcher = RecordingDispatcher()

    serving = asyncio.create_task(serve(config(max_concurrency=1), session, dispatcher))
    try:
        # Longer than any plausible derived cadence for a real window.
        await asyncio.sleep(0.1)
        assert session.connection_beats == []
        assert session.task_beats == []
    finally:
        await session.finish()
        await serving


class _IdleDispatcher:
    """Advertises one action and is never asked to run it."""

    def activity_types(self) -> list[str]:
        return ["noop"]

    async def dispatch(self, task: ActivityTask, context: ActivityExecutionContext) -> DispatchOutcome:
        del context
        return Completed(encode_json_payload(task.activity_type))


IDLE_DESCRIPTORS = [ActivityDescriptor(name="noop", input_schema_json="true", output_schema_json="true")]


async def test_an_idle_worker_survives_three_windows_on_a_real_stream() -> None:
    """The issue's acceptance criterion, over a real gRPC connection.

    A server that enforces the connection lease deregisters an unbeaten idle
    worker and ends its stream; the SDK's reconnect loop then redials, which is
    the churn #26 reports. Exactly one accepted connection across more than
    three windows is the proof the worker held its own lease open instead of
    leaning on reconnection as a keepalive.
    """

    service = LeaseEnforcingWorkerProtocol(heartbeat_window_ms=int(WINDOW_SECONDS * 1000))
    async with running_worker(service, _IdleDispatcher(), IDLE_DESCRIPTORS) as running:
        await asyncio.sleep(WINDOW_SECONDS * (WINDOWS_OBSERVED + 0.5))
        assert not running.done(), f"the worker run ended early: {running.exception() if running.done() else ''}"
        assert len(service.connections) == 1, (
            f"{len(service.connections)} connections were accepted across "
            f"{WINDOWS_OBSERVED}+ windows; a worker that beats its lease reconnects zero times. "
            f"Lease expiries: {[c.lease_expired for c in service.connections]}"
        )
        connection = service.connections[0]
        assert not connection.lease_expired, "the idle worker's connection lease expired"
        assert connection.connection_beats >= WINDOWS_OBSERVED, (
            f"only {connection.connection_beats} connection beats reached the server across "
            f"{WINDOWS_OBSERVED} windows"
        )
        assert connection.task_beats == [], "an idle worker sent task heartbeats for tasks it does not have"
        assert connection.registrations == [["noop"]]
