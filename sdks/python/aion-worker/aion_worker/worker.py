"""Worker object + run()."""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Mapping
from concurrent.futures import Executor

from .activity import ActivityRegistry, clone_registry, default_registry, registry_from_mapping
from .loop import connect_register_replay_and_serve, serve
from .reconnect import UnackedResultTracker
from .session import ActivityDescriptor, GrpcWorkerSession, WorkerConfig, WorkerSession


class Worker:
    """Author-facing worker that serves registered activities."""

    def __init__(
        self,
        config: WorkerConfig,
        *,
        registry: ActivityRegistry | None = None,
        activities: Mapping[str, Callable[..., object]] | None = None,
        executor: Executor | None = None,
    ) -> None:
        self.config = config
        if registry is not None and activities is not None:
            raise ValueError("provide either registry or activities, not both")
        if activities is not None:
            self._registry = registry_from_mapping(activities, executor=executor)
        else:
            source = registry if registry is not None else default_registry
            self._registry = clone_registry(source, executor=executor)
        self._activity_types = list(self._registry.activity_types())
        if not self._activity_types:
            raise ValueError("worker must register at least one activity")
        # Derived HERE, before any network I/O, so a handler that cannot
        # advertise what it returns fails at Worker construction rather than
        # part-way through the reconnect cycle against a live server.
        self._descriptors = self._registry.activity_descriptors()
        self._tracker = UnackedResultTracker()

    def activity_types(self) -> list[str]:
        """Return activity type names served by this worker."""

        return list(self._activity_types)

    def activity_descriptors(self) -> list[ActivityDescriptor]:
        """Return the typed action surface this worker advertises at registration."""

        return list(self._descriptors)

    def available_handlers(self) -> list[str]:
        """Return handler names available for registration."""

        return self.activity_types()

    async def run(self, *, shutdown: asyncio.Event | None = None) -> None:
        """Connect to the engine and serve until shutdown, denial, or budget exhaustion.

        Clean server-side stream closes reconnect through the bounded drop
        budget (which resets once a session proves healthy) rather than
        ending the run; see :class:`aion_worker.ReconnectConfig`.
        """

        await connect_register_replay_and_serve(
            self.config,
            self._connect,
            self._registry,
            self._descriptors,
            self._tracker,
            shutdown=shutdown,
        )

    async def run_with_session(self, session: WorkerSession, *, shutdown: asyncio.Event | None = None) -> None:
        """Test seam: serve an already constructed session with graceful drain."""

        await session.handshake(self.config)
        await session.register(self._activity_types, self._descriptors, self.available_handlers())
        await serve(self.config, session, self._registry, self._tracker, shutdown=shutdown)

    async def _connect(self) -> WorkerSession:
        return await GrpcWorkerSession.connect(self.config)
