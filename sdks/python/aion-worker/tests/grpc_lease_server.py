"""An in-process gRPC `WorkerProtocol` server that enforces the connection lease.

Why a real server and not a session fake: the defects this harness exists for
(#26, #50) both live BELOW the session fakes the rest of the suite uses. #26 is
about which frames actually reach the wire while the worker is idle, and #50 is
about what a real `ServerToWorker` oneof does to
`GrpcWorkerSession._receive_from_stream`. A fake session that hands the loop
already-decoded events cannot see either: it never encodes a frame and never
decodes one. So this harness speaks the committed wire contract over a real
`grpc.aio` channel, against the real `GrpcWorkerSession`, and models the two
server behaviours under test:

* the **connection lease** — every inbound frame refreshes it
  (`aion-server/src/api/worker_grpc.rs` `refresh_connection_lease`), a heartbeat
  with neither task id set is accepted as a pure lease beat
  (`worker_grpc.rs:425-431`), and a lease that goes a whole window without any
  frame expires the worker and ends its stream;
* the **transport liveness probe** (#197) — `LivenessPing` frames pushed from
  the server's own timer, whose echoed sequences are the only evidence the
  server-to-worker leg carries.

It is deliberately NOT a re-implementation of the server's dispatch, retry, or
selection logic: it models exactly the two mechanisms whose worker-side half is
being fixed, so a green test here means the worker holds up its end of those
two contracts.
"""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from typing import Any, cast

import grpc

from aion_worker import (
    ActivityDispatcher,
    GrpcWorkerSession,
    ReconnectConfig,
    WorkerConfig,
    connect_register_replay_and_serve,
)
from aion_worker.proto import common_pb2, worker_pb2, worker_pb2_grpc

REGISTER_DEADLINE_SECONDS = 5.0
"""How long a connection may stay open without sending `RegisterWorker`.

Not a lease value: the lease only starts once the worker is registered, so a
connection that never registers would otherwise hold a servicer coroutine open
for the whole test session. A test that hangs here has a worker that never sent
its registration, which is a failure worth surfacing as one.
"""


@dataclass
class Connection:
    """Everything one `StreamWorker` invocation observed, for assertions.

    One instance per accepted stream: the COUNT of these is the reconnect-churn
    measurement (#26 makes a healthy idle worker keep exactly one), and the
    contents are the per-connection evidence.
    """

    registrations: list[list[str]] = field(default_factory=list)
    inbound: list[str] = field(default_factory=list)
    """Every inbound oneof name in arrival order."""
    connection_beats: int = 0
    """Heartbeats with NEITHER task id set — the connection-level lease beat."""
    task_beats: list[tuple[str, int]] = field(default_factory=list)
    """(workflow uuid, activity sequence position) for task-scoped heartbeats."""
    liveness_answers: list[int] = field(default_factory=list)
    """Echoed ping sequences, in arrival order."""
    results: list[tuple[str, int]] = field(default_factory=list)
    """(workflow uuid, activity sequence position) for reported outcomes."""
    lease_expired: bool = False
    """Set when this connection was ended because its lease ran out."""
    registered: asyncio.Event = field(default_factory=asyncio.Event)


class LeaseEnforcingWorkerProtocol(worker_pb2_grpc.WorkerProtocolServicer):
    """A `WorkerProtocol` service that expires an unbeaten connection lease."""

    def __init__(
        self,
        heartbeat_window_ms: int,
        ping_every_seconds: float | None = None,
        tick_seconds: float = 0.005,
        task_to_dispatch: worker_pb2.ActivityTask | None = None,
        cancel_after_seconds: float | None = None,
    ) -> None:
        self.heartbeat_window_ms = heartbeat_window_ms
        self.window_seconds = heartbeat_window_ms / 1000.0
        self.ping_every_seconds = ping_every_seconds
        self.tick_seconds = tick_seconds
        self.task_to_dispatch = task_to_dispatch
        self.cancel_after_seconds = cancel_after_seconds
        self.connections: list[Connection] = []
        self.pings_sent: list[int] = []
        self.cancels_sent: list[int] = []
        self._next_ping_sequence = 0

    async def StreamWorker(  # noqa: N802 — the generated servicer's method name.
        self,
        request_iterator: AsyncIterator[worker_pb2.WorkerToServer],
        context: Any,
    ) -> AsyncIterator[worker_pb2.ServerToWorker]:
        del context
        connection = Connection()
        self.connections.append(connection)
        loop = asyncio.get_running_loop()
        lease = _Lease(last_frame_at=loop.time())
        reader = asyncio.create_task(self._read_inbound(request_iterator, connection, lease))
        try:
            try:
                await asyncio.wait_for(connection.registered.wait(), timeout=REGISTER_DEADLINE_SECONDS)
            except asyncio.TimeoutError:
                return
            yield worker_pb2.ServerToWorker(
                register_ack=worker_pb2.RegisterAck(
                    worker_id=len(self.connections),
                    namespace="default",
                    heartbeat_window_ms=self.heartbeat_window_ms,
                )
            )
            if self.task_to_dispatch is not None:
                yield worker_pb2.ServerToWorker(task=self.task_to_dispatch)
            last_ping_at = loop.time()
            dispatched_at = loop.time()
            cancel_pending = self.cancel_after_seconds is not None and self.task_to_dispatch is not None
            while True:
                await asyncio.sleep(self.tick_seconds)
                if reader.done():
                    # The worker half-closed or its send side failed; the
                    # reader surfaces the reason. Nothing left to lease.
                    reader.result()
                    return
                now = loop.time()
                if now - lease.last_frame_at > self.window_seconds:
                    # Mirrors the server: an idle connection lease that expires
                    # deregisters the worker and ends its stream.
                    connection.lease_expired = True
                    return
                if (
                    cancel_pending
                    and self.cancel_after_seconds is not None
                    and self.task_to_dispatch is not None
                    and now - dispatched_at >= self.cancel_after_seconds
                ):
                    cancel_pending = False
                    task = self.task_to_dispatch
                    self.cancels_sent.append(task.activity_id.sequence_position)
                    yield worker_pb2.ServerToWorker(
                        cancel_activity=worker_pb2.CancelActivity(
                            workflow_id=task.workflow_id,
                            activity_id=task.activity_id,
                        )
                    )
                if self.ping_every_seconds is not None and now - last_ping_at >= self.ping_every_seconds:
                    last_ping_at = now
                    self._next_ping_sequence += 1
                    self.pings_sent.append(self._next_ping_sequence)
                    yield worker_pb2.ServerToWorker(
                        liveness_ping=worker_pb2.LivenessPing(
                            liveness_ping=self._next_ping_sequence,
                            silence_window_ms=self.heartbeat_window_ms,
                        )
                    )
        finally:
            reader.cancel()

    async def _read_inbound(
        self,
        request_iterator: AsyncIterator[worker_pb2.WorkerToServer],
        connection: Connection,
        lease: _Lease,
    ) -> None:
        loop = asyncio.get_running_loop()
        async for message in request_iterator:
            # The server refreshes the connection lease on EVERY inbound frame,
            # whatever it carries.
            lease.last_frame_at = loop.time()
            which = message.WhichOneof("message")
            if which is None:
                continue
            connection.inbound.append(which)
            if which == "register":
                connection.registrations.append(list(message.register.activity_types))
                connection.registered.set()
            elif which == "heartbeat":
                heartbeat = message.heartbeat
                if not heartbeat.HasField("workflow_id") and not heartbeat.HasField("activity_id"):
                    connection.connection_beats += 1
                else:
                    connection.task_beats.append(
                        (heartbeat.workflow_id.uuid, heartbeat.activity_id.sequence_position)
                    )
            elif which == "liveness_answer":
                connection.liveness_answers.append(message.liveness_answer.liveness_ping)
            elif which == "result":
                result = message.result
                connection.results.append((result.workflow_id.uuid, result.activity_id.sequence_position))


@dataclass
class _Lease:
    """The connection lease's single mutable fact, shared reader-to-timer."""

    last_frame_at: float


class RunningLeaseServer:
    """A started `LeaseEnforcingWorkerProtocol` bound to a loopback port."""

    def __init__(self, service: LeaseEnforcingWorkerProtocol, server: Any, endpoint: str) -> None:
        self.service = service
        self._server = server
        self.endpoint = endpoint

    async def stop(self) -> None:
        await self._server.stop(grace=None)


async def start_lease_server(service: LeaseEnforcingWorkerProtocol) -> RunningLeaseServer:
    """Start ``service`` on an ephemeral loopback port and return its endpoint."""

    server = cast(Any, grpc.aio).server()
    worker_pb2_grpc.add_WorkerProtocolServicer_to_server(service, server)
    port = server.add_insecure_port("127.0.0.1:0")
    await server.start()
    return RunningLeaseServer(service, server, f"127.0.0.1:{port}")


@asynccontextmanager
async def running_worker(
    service: LeaseEnforcingWorkerProtocol,
    dispatcher: ActivityDispatcher,
    descriptors: list[worker_pb2.ActivityDescriptor],
    release: asyncio.Event | None = None,
    identity: str = "wire-worker",
) -> AsyncIterator[asyncio.Task[None]]:
    """Run a real `GrpcWorkerSession` worker against ``service`` for the block.

    Assertions belong INSIDE the block: the evidence these tests read — a task
    still in flight, a connection not yet redialled — is only true while the
    worker is running.

    The teardown is ordered so that no failure can wedge the suite or leak a
    listening server. ``release`` (a handler's hold) is set FIRST, because the
    serve loop's final drain waits on in-flight handlers and a red run would
    otherwise deadlock there; the server is stopped in an outer ``finally`` so
    an exception escaping the worker run still closes the port.
    """

    server = await start_lease_server(service)
    try:
        worker_config = WorkerConfig(
            endpoint=server.endpoint,
            task_queue="default",
            identity=identity,
            max_concurrency=1,
            reconnect=ReconnectConfig(initial_backoff_seconds=0.01, max_backoff_seconds=1.0, max_attempts=20),
        )
        shutdown = asyncio.Event()

        async def connect() -> GrpcWorkerSession:
            return await GrpcWorkerSession.connect(worker_config)

        running = asyncio.create_task(
            connect_register_replay_and_serve(
                worker_config,
                connect,
                dispatcher,
                descriptors,
                shutdown=shutdown,
            )
        )
        try:
            yield running
        finally:
            if release is not None:
                release.set()
            shutdown.set()
            await running
    finally:
        await server.stop()


def dispatchable_task(
    workflow_uuid: str,
    sequence_position: int,
    activity_type: str,
    payload_bytes: bytes = b"{}",
) -> worker_pb2.ActivityTask:
    """A fully-formed `ActivityTask`: every field the worker refuses to default."""

    return worker_pb2.ActivityTask(
        workflow_id=common_pb2.WorkflowId(uuid=workflow_uuid),
        activity_id=common_pb2.ActivityId(sequence_position=sequence_position),
        run_id=common_pb2.RunId(uuid=f"run-{workflow_uuid}"),
        activity_type=activity_type,
        input=common_pb2.Payload(content_type="application/json", bytes=payload_bytes),
        attempt=1,
        completion_token=f"generation-{sequence_position}",
        idempotency_key=f"effect-{workflow_uuid}-{sequence_position}",
    )
