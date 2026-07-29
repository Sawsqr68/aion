"""Reusable transport helpers for the Aion Python client."""

from __future__ import annotations

import importlib
import re
from collections.abc import Sequence
from types import ModuleType
from typing import Any, Protocol
from urllib.parse import urlparse

from .errors import InvalidArgument, ServerError
from .proto import common_pb2

MappingMetadata = tuple[tuple[str, str], ...]

# The prefix protobuf requires on every `WorkflowStatus` member because enum
# names share one namespace per package. Stripped from the decoded name so the
# SDK reports the language-neutral status spelling (`RUNNING`, `PAUSED`) that the
# shared conformance contract uses, rather than the protobuf-internal one.
_WORKFLOW_STATUS_PREFIX = "WORKFLOW_STATUS_"


class WorkflowTransport(Protocol):
    """Protocol implemented by real/stub reusable workflow transports."""

    async def start_workflow(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def signal(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def query(self, request: Any, metadata: MappingMetadata, timeout: float | None = None) -> Any: ...
    async def cancel(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def reopen(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def pause(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def resume(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def list_workflows(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def describe_workflow(self, request: Any, metadata: MappingMetadata) -> Any: ...
    async def close(self) -> None: ...


class GrpcWorkflowTransport:
    """gRPC transport backed by generated ``aion`` protobuf stubs."""

    def __init__(self, endpoint: str, *, tls: Any) -> None:
        try:
            grpc = importlib.import_module("grpc")
            workflow_pb2_grpc = importlib.import_module("aion_client.proto.workflow_pb2_grpc")
        except ImportError as exc:
            raise InvalidArgument(
                "grpcio and the packaged aion_client.proto stubs are required for the default transport"
            ) from exc
        self._grpc: ModuleType = grpc
        self._workflow_pb2_grpc: ModuleType = workflow_pb2_grpc
        self._channel = self._build_channel(endpoint, tls)
        self._stub = workflow_pb2_grpc.WorkflowServiceStub(self._channel)

    @staticmethod
    def message(name: str) -> Any:
        """Create a generated AW workflow protobuf message by name."""

        try:
            workflow_pb2 = importlib.import_module("aion_client.proto.workflow_pb2")
        except ImportError as exc:
            raise InvalidArgument("packaged aion_client.proto.workflow_pb2 module is required") from exc
        try:
            cls = getattr(workflow_pb2, name)
        except AttributeError as exc:
            raise InvalidArgument(f"generated proto message {name} is unavailable") from exc
        return cls()

    async def connect(self) -> None:
        """Await channel readiness when the gRPC runtime exposes it."""

        channel_ready = getattr(self._channel, "channel_ready", None)
        if callable(channel_ready):
            await channel_ready()

    async def start_workflow(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.StartWorkflow(request, metadata=metadata)

    async def signal(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.Signal(request, metadata=metadata)

    async def query(self, request: Any, metadata: MappingMetadata, timeout: float | None = None) -> Any:
        return await self._stub.Query(request, metadata=metadata, timeout=timeout)

    async def cancel(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.Cancel(request, metadata=metadata)

    async def reopen(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.Reopen(request, metadata=metadata)

    async def pause(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.Pause(request, metadata=metadata)

    async def resume(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.Resume(request, metadata=metadata)

    async def list_workflows(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.ListWorkflows(request, metadata=metadata)

    async def describe_workflow(self, request: Any, metadata: MappingMetadata) -> Any:
        return await self._stub.DescribeWorkflow(request, metadata=metadata)

    async def close(self) -> None:
        await self._channel.close()

    def _build_channel(self, endpoint: str, tls: Any) -> Any:
        target = grpc_target(endpoint)
        if tls.enabled:
            credentials = self._grpc.ssl_channel_credentials(
                root_certificates=read_optional(tls.ca_file),
                private_key=read_optional(tls.client_key_file),
                certificate_chain=read_optional(tls.client_cert_file),
            )
            options: list[tuple[str, str]] = []
            if tls.server_name is not None:
                options.append(("grpc.ssl_target_name_override", tls.server_name))
            return self._grpc.aio.secure_channel(target, credentials, options=options)
        return self._grpc.aio.insecure_channel(target)


def metadata(
    auth: str | None,
    *,
    subject: str | None = None,
    namespaces: Sequence[str] | None = None,
) -> MappingMetadata:
    """Build AW caller-identity metadata: the bearer credential plus the
    development-mode identity headers (``x-aion-subject``,
    ``x-aion-namespaces``) the server's caller extraction reads."""

    entries: list[tuple[str, str]] = []
    if auth is not None:
        token = auth.removeprefix("Bearer ")
        entries.append(("authorization", f"Bearer {token}"))
    if subject is not None:
        entries.append(("x-aion-subject", subject))
    if namespaces is not None:
        entries.append(("x-aion-namespaces", ",".join(namespaces)))
    return tuple(entries)


def workflow_status_name(value: Any) -> str:
    """Decode a wire ``WorkflowStatus`` into its language-neutral name.

    Reopen, pause, and resume each return the run's PROJECTED status, which is
    the whole point of their having a response body rather than being an empty
    ack: the caller learns the run is live again, or held. So an undecodable
    status is a failed operation, not a detail to shrug at — every branch here
    either produces a real status name or raises.

    Protobuf enum fields read back as ``int``, resolved through the generated
    descriptor so no name table is maintained by hand. A ``str`` is accepted for
    non-gRPC transports and test doubles, in any of the three spellings in
    circulation — the protobuf member name, that name without its prefix, or
    aion-core's serde PascalCase — and is checked against the descriptor rather
    than trusted. Naive upper-casing would turn ``TimedOut`` into ``TIMEDOUT``,
    which is not a status and would be reported as though it were.

    Raises:
        ServerError: the status is unspecified (code 0 — the server declined to
            report the projected status), unknown to the committed contract, or
            of a type that carries no status at all.
    """

    if isinstance(value, str):
        name = _status_name_from_text(value)
    elif isinstance(value, int) and not isinstance(value, bool):
        try:
            name = str(common_pb2.WorkflowStatus.Name(value)).removeprefix(_WORKFLOW_STATUS_PREFIX)
        except ValueError as exc:
            raise ServerError(
                f"server returned workflow status {value}, which is not a value in this "
                "SDK's committed wire contract; the stubs are older than the server "
                "(regenerate with `python -m proto_codegen`)"
            ) from exc
    else:
        raise ServerError(f"server returned a workflow status of type {type(value).__name__}, which carries no status")
    if not name or name == "UNSPECIFIED":
        raise ServerError(
            "server returned an unspecified workflow status; the projected status is "
            "the point of this response and cannot be assumed"
        )
    return name


def _status_name_from_text(value: str) -> str:
    """Resolve a textual status to a declared enum member, or refuse it.

    Both candidate spellings are checked against the generated descriptor because
    a status name that merely LOOKS plausible is worse than an error: it would be
    handed to the caller as a status the contract does not define.
    """

    direct = value.strip().upper().removeprefix(_WORKFLOW_STATUS_PREFIX)
    # PascalCase (aion-core's serde spelling) split on each capital: TimedOut ->
    # TIMED_OUT, ContinuedAsNew -> CONTINUED_AS_NEW.
    pascal = re.sub(r"(?<!^)(?=[A-Z])", "_", value.strip()).upper()
    for candidate in (direct, pascal):
        if f"{_WORKFLOW_STATUS_PREFIX}{candidate}" in common_pb2.WorkflowStatus.keys():
            return candidate
    raise ServerError(
        f"server returned workflow status {value!r}, which is not a status in this SDK's "
        "committed wire contract in any recognised spelling"
    )


def grpc_target(endpoint: str) -> str:
    """Translate URL-style endpoints to the host:port gRPC target."""

    parsed = urlparse(endpoint)
    if parsed.scheme in {"http", "https"} and parsed.netloc:
        return parsed.netloc
    return endpoint.removeprefix("grpc://").removeprefix("grpcs://")


def read_optional(path: str | None) -> bytes | None:
    """Read optional TLS material from disk."""

    if path is None:
        return None
    with open(path, "rb") as file:
        return file.read()
