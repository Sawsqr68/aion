"""Branchable exception taxonomy for the Aion Python client."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from enum import Enum
from typing import Any, NoReturn

from .proto import common_pb2


class ErrorCode(str, Enum):
    """Shared language-neutral client error names."""

    NOT_FOUND = "NotFound"
    ALREADY_EXISTS = "AlreadyExists"
    QUERY_FAILED = "QueryFailed"
    QUERY_TIMEOUT = "QueryTimeout"
    CANCELLED = "Cancelled"
    UNAVAILABLE = "Unavailable"
    UNAUTHENTICATED = "Unauthenticated"
    NAMESPACE_DENIED = "NamespaceDenied"
    INVALID_ARGUMENT = "InvalidArgument"
    SERVER = "Server"


class AionClientError(Exception):
    """Base class for all errors raised by :mod:`aion_client`."""

    code: ErrorCode = ErrorCode.SERVER

    def __init__(self, message: str | None = None, *, detail: str | None = None) -> None:
        self.detail = detail
        super().__init__(message or detail or self.code.value)


class NotFound(AionClientError):
    """The targeted workflow, run, subscription, or entity was not found."""

    code = ErrorCode.NOT_FOUND


class AlreadyExists(AionClientError):
    """A start idempotency key was reused for a conflicting request."""

    code = ErrorCode.ALREADY_EXISTS


class QueryFailed(AionClientError):
    """The workflow query handler ran and returned an application failure."""

    code = ErrorCode.QUERY_FAILED


class QueryTimeout(AionClientError):
    """The query deadline elapsed before a result was available."""

    code = ErrorCode.QUERY_TIMEOUT


class Cancelled(AionClientError):
    """The requested client/server operation was cancelled."""

    code = ErrorCode.CANCELLED


class Unavailable(AionClientError):
    """The server or event stream is temporarily unreachable."""

    code = ErrorCode.UNAVAILABLE


class Unauthenticated(AionClientError):
    """The server rejected or could not validate the caller credential."""

    code = ErrorCode.UNAUTHENTICATED


class NamespaceDenied(AionClientError):
    """The caller has no grant for the requested namespace.

    Raised when the caller's credential is valid but carries no grant for
    the requested namespace. A workflow that does not exist or is owned by
    another namespace surfaces as :class:`NotFound` instead — the response
    is identical in both cases, so existence is never leaked across
    namespaces. Distinct from :class:`Unauthenticated` (credential rejected)
    and not retryable until grants change.
    """

    code = ErrorCode.NAMESPACE_DENIED


class InvalidArgument(AionClientError):
    """The request is syntactically or semantically invalid."""

    code = ErrorCode.INVALID_ARGUMENT


class ServerError(AionClientError):
    """An unexpected server-side failure with preserved diagnostic detail."""

    code = ErrorCode.SERVER


_ERROR_CLASSES: dict[ErrorCode, type[AionClientError]] = {
    ErrorCode.NOT_FOUND: NotFound,
    ErrorCode.ALREADY_EXISTS: AlreadyExists,
    ErrorCode.QUERY_FAILED: QueryFailed,
    ErrorCode.QUERY_TIMEOUT: QueryTimeout,
    ErrorCode.CANCELLED: Cancelled,
    ErrorCode.UNAVAILABLE: Unavailable,
    ErrorCode.UNAUTHENTICATED: Unauthenticated,
    ErrorCode.NAMESPACE_DENIED: NamespaceDenied,
    ErrorCode.INVALID_ARGUMENT: InvalidArgument,
    ErrorCode.SERVER: ServerError,
}

_GRPC_CODE_MAP: dict[str, ErrorCode] = {
    "NOT_FOUND": ErrorCode.NOT_FOUND,
    "ALREADY_EXISTS": ErrorCode.ALREADY_EXISTS,
    "DEADLINE_EXCEEDED": ErrorCode.QUERY_TIMEOUT,
    "CANCELLED": ErrorCode.CANCELLED,
    "UNAVAILABLE": ErrorCode.UNAVAILABLE,
    "UNAUTHENTICATED": ErrorCode.UNAUTHENTICATED,
    "INVALID_ARGUMENT": ErrorCode.INVALID_ARGUMENT,
    "FAILED_PRECONDITION": ErrorCode.INVALID_ARGUMENT,
    "PERMISSION_DENIED": ErrorCode.NAMESPACE_DENIED,
    "RESOURCE_EXHAUSTED": ErrorCode.UNAVAILABLE,
    "INTERNAL": ErrorCode.SERVER,
    "UNKNOWN": ErrorCode.SERVER,
}

# Every `WireErrorCode` the contract defines, keyed by the stable `snake_case`
# code that `aion_proto::WireErrorCode::as_str` documents as the branchable
# spelling. The gRPC wire carries the same values as the protobuf enum's
# SCREAMING_SNAKE names; `_canonical_wire_code` folds either spelling onto these
# keys, so each code appears exactly once instead of twice.
#
# `tests/test_errors.py` requires this table to cover every value in the
# generated `common_pb2.WireErrorCode` descriptor. That gate exists because this
# table's predecessor was a hand-maintained mirror that stopped at code 10:
# `not_owner` and `invalid_state` were added to the contract, arrived as ints
# 13 and 14, matched nothing, and were reported as generic server faults — a
# retryable routing fence indistinguishable from a crashed backend.
_WIRE_CODE_MAP: dict[str, ErrorCode] = {
    # Code 0 exists only because proto3 requires a zero value; the Rust enum has
    # no such variant, so a server sending it is reporting a failure it declined
    # to classify. That is a server fault, not an unrecognized code.
    "unspecified": ErrorCode.SERVER,
    "not_found": ErrorCode.NOT_FOUND,
    "namespace_denied": ErrorCode.NAMESPACE_DENIED,
    # sequence_conflict signals an engine-internal double-writer bug, never an
    # idempotency conflict; it is a server fault on every operation.
    "sequence_conflict": ErrorCode.SERVER,
    "unknown_query": ErrorCode.INVALID_ARGUMENT,
    "query_timeout": ErrorCode.QUERY_TIMEOUT,
    "not_running": ErrorCode.INVALID_ARGUMENT,
    "lagged": ErrorCode.UNAVAILABLE,
    "invalid_input": ErrorCode.INVALID_ARGUMENT,
    "backend": ErrorCode.SERVER,
    "query_failed": ErrorCode.QUERY_FAILED,
    # deploy_denied and version_pinned belong to the operator deploy surface,
    # which the caller contract deliberately excludes, so no caller operation can
    # return them. They are server faults here rather than new taxonomy members —
    # the same call the Rust caller SDK makes.
    "deploy_denied": ErrorCode.SERVER,
    "version_pinned": ErrorCode.SERVER,
    # not_owner is the wrong-shard-owner fence: the request reached a node that
    # does not own the target's shard, and re-resolving the owner and retrying is
    # the correct response. Unavailable is the shared taxonomy's retryable member,
    # so that is where it belongs; Server would tell the caller to stop trying.
    # (The Rust caller SDK gave it a dedicated `NotOwner` class, which the shared
    # ten-code taxonomy this SDK implements with the TypeScript client has no room
    # for; Unavailable is where Rust itself carried it before the split.)
    "not_owner": ErrorCode.UNAVAILABLE,
    # invalid_state means the target exists but is in the wrong state for the
    # request — pausing a run that is not Running, resuming one that is not
    # Paused. Distinct from not_found (absent) and backend (internal failure).
    "invalid_state": ErrorCode.INVALID_ARGUMENT,
}

_START_IDEMPOTENCY_CODES = {"already_exists"}


@dataclass(frozen=True)
class ServerFailure:
    """A normalized server failure value for transports that do not use gRPC."""

    code: str
    message: str
    detail: str | None = None
    operation: str | None = None


def error_from_code(code: ErrorCode, message: str | None = None, *, detail: str | None = None) -> AionClientError:
    """Build the branchable exception for a normalized taxonomy code."""

    return _ERROR_CLASSES[code](message, detail=detail)


def raise_mapped(error: BaseException, *, operation: str | None = None) -> NoReturn:
    """Raise ``map_error(error)``; useful in ``except`` blocks."""

    raise map_error(error, operation=operation) from error


def map_error(
    error: BaseException | ServerFailure | Mapping[str, object], *, operation: str | None = None
) -> AionClientError:
    """Map transport/server failures to the shared Aion client taxonomy.

    Existing :class:`AionClientError` instances are returned unchanged. gRPC
    ``AioRpcError``-style values are detected structurally by a ``code()``
    method, while simple ``ServerFailure`` and mapping values support test and
    non-gRPC transport adapters.
    """

    if isinstance(error, AionClientError):
        return error
    if isinstance(error, TimeoutError):
        if operation == "query":
            return QueryTimeout(str(error) or "query timed out")
        return Unavailable(str(error) or "operation timed out")
    if isinstance(error, OSError):
        return Unavailable(str(error) or "transport unavailable")
    if isinstance(error, ServerFailure):
        return _map_server_failure(error)
    if isinstance(error, Mapping):
        return _map_mapping(error, operation=operation)

    code_attr = getattr(error, "code", None)
    if callable(code_attr):
        grpc_code = code_attr()
        name = getattr(grpc_code, "name", str(grpc_code).split(".")[-1])
        details_attr = getattr(error, "details", None)
        details = details_attr() if callable(details_attr) else str(error)
        return _map_grpc_status(str(name), str(details), operation=operation)

    return ServerError(str(error) or "unexpected client error", detail=str(error) or None)


def map_query_error(error: Any) -> AionClientError:
    """Map a QueryResponse error payload to ``QueryFailed`` or a specific status."""

    if isinstance(error, AionClientError):
        return error
    mapped = _map_wire_like(error, operation="query")
    if isinstance(
        mapped,
        QueryFailed
        | InvalidArgument
        | QueryTimeout
        | NotFound
        | Unauthenticated
        | NamespaceDenied
        | Unavailable
        | Cancelled,
    ):
        return mapped
    if isinstance(mapped, ServerError):
        return mapped
    return QueryFailed(str(mapped) or "query failed")


def _map_mapping(mapping: Mapping[str, object], *, operation: str | None) -> AionClientError:
    code = mapping.get("code") or mapping.get("status") or mapping.get("error_code")
    message = mapping.get("message") or mapping.get("detail")
    return _map_server_failure(
        ServerFailure(
            code=str(code or "backend"),
            message=str(message or code or "server error"),
            detail=str(mapping.get("detail")) if mapping.get("detail") is not None else None,
            operation=operation,
        )
    )


def _map_server_failure(failure: ServerFailure) -> AionClientError:
    return _map_wire_like(failure, operation=failure.operation)


def _map_wire_like(error: Any, *, operation: str | None) -> AionClientError:
    raw_code = getattr(error, "code", None)
    message = str(getattr(error, "message", "") or getattr(error, "detail", "") or raw_code or "server error")
    detail = getattr(error, "detail", None)

    code_name = _canonical_wire_code(_wire_code_name(raw_code))
    if operation == "start" and code_name in _START_IDEMPOTENCY_CODES:
        return AlreadyExists(message, detail=detail)
    mapped = _WIRE_CODE_MAP.get(code_name)
    if mapped is None:
        # A code neither the contract nor this SDK's generated stubs know: the
        # server is newer than the committed wire contract. Server is the only
        # honest classification, but the code is carried into the detail so the
        # operator can see WHICH code could not be classified instead of reading
        # a bare "server error" and having nothing to search for.
        unclassified = f"unrecognized wire error code {code_name!r}"
        detail = f"{detail}; {unclassified}" if detail else unclassified
        mapped = ErrorCode.SERVER
    return error_from_code(mapped, message, detail=detail)


def _wire_code_name(raw_code: Any) -> str:
    """Extract a wire error code's name from any of the shapes it arrives in.

    A protobuf enum field reads back as a plain ``int``, so the numeric form is
    the ordinary gRPC path rather than an exotic one, and it is resolved through
    the GENERATED descriptor. Its predecessor was a hand-copied int-to-name table
    that had to be edited whenever the contract grew a code and — being a copy
    with nothing enforcing it — was not, twice.

    ``None`` or a blank string means the failure carried no code at all, which the
    shared taxonomy treats as a backend fault (an unclassified server-side
    failure), matching how ``_map_mapping`` defaults a code-less mapping. That is
    a different thing from a code this SDK does not RECOGNISE, which is reported
    as such — conflating the two would send a reader off to regenerate stubs that
    are perfectly current.
    """

    if raw_code is None or (isinstance(raw_code, str) and not raw_code.strip()):
        return "backend"
    name = getattr(raw_code, "name", None)
    if isinstance(name, str):
        return name
    if isinstance(raw_code, int):
        try:
            return str(common_pb2.WireErrorCode.Name(raw_code))
        except ValueError:
            # Not a value in the committed contract. Reported verbatim so the
            # unmapped branch above names the number the server actually sent,
            # rather than being folded into `backend` and losing it.
            return f"numeric_{raw_code}"
    return str(raw_code)


def _canonical_wire_code(name: str) -> str:
    """Fold either wire spelling of a code onto its stable ``snake_case`` form.

    The JSON/WebSocket wire sends ``not_owner``; the protobuf descriptor names
    the same value ``WIRE_ERROR_CODE_NOT_OWNER``. Normalising here is what lets
    ``_WIRE_CODE_MAP`` hold one entry per code instead of two, which matters
    because a two-spelling table is a table where half the entries can silently
    go missing.
    """

    return name.lower().removeprefix("wire_error_code_")


def _map_grpc_status(name: str, details: str, *, operation: str | None) -> AionClientError:
    if operation == "start" and name == "ALREADY_EXISTS":
        return AlreadyExists(details)
    if operation == "query" and name == "DEADLINE_EXCEEDED":
        return QueryTimeout(details)
    if operation == "query" and name == "CANCELLED" and details == "Timeout expired":
        # grpc.aio surfaces a caller-supplied per-call `timeout=` expiry as a
        # locally cancelled RPC with this exact detail string, not as
        # DEADLINE_EXCEEDED. The caller's query deadline elapsing is the
        # contract's QueryTimeout, never a caller-intent Cancelled.
        return QueryTimeout(details)
    return error_from_code(_GRPC_CODE_MAP.get(name, ErrorCode.SERVER), details, detail=details)
