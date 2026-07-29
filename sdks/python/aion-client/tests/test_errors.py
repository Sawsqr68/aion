from __future__ import annotations

from dataclasses import dataclass
from types import SimpleNamespace

import pytest

from aion_client import (
    AlreadyExists,
    InvalidArgument,
    NamespaceDenied,
    NotFound,
    QueryFailed,
    QueryTimeout,
    ServerError,
    Unauthenticated,
    Unavailable,
)
from aion_client.errors import (
    _WIRE_CODE_MAP,
    ErrorCode,
    ServerFailure,
    _canonical_wire_code,
    map_error,
    map_query_error,
)
from aion_client.proto import common_pb2


class _FakeRpcError(Exception):
    """Structural stand-in for ``grpc.aio.AioRpcError`` (``code()``/``details()``)."""

    def __init__(self, name: str, details: str) -> None:
        super().__init__(details)
        self._name = name
        self._details = details

    def code(self) -> SimpleNamespace:
        return SimpleNamespace(name=self._name)

    def details(self) -> str:
        return self._details


@dataclass(frozen=True)
class _WireQueryError:
    """Structural stand-in for the AW QueryResponse error payload."""

    code: int | str
    message: str
    detail: str | None = None


def test_maps_not_found_wire_error() -> None:
    error = map_error(ServerFailure(code="not_found", message="missing"))
    assert isinstance(error, NotFound)


def test_maps_start_idempotency_conflict_to_already_exists() -> None:
    error = map_error(ServerFailure(code="already_exists", message="conflict", operation="start"), operation="start")
    assert isinstance(error, AlreadyExists)


def test_sequence_conflict_is_server_fault_even_on_start() -> None:
    error = map_error(ServerFailure(code="sequence_conflict", message="conflict", operation="start"), operation="start")
    assert isinstance(error, ServerError)
    assert not isinstance(error, AlreadyExists)


def test_maps_query_timeout_wire_error() -> None:
    error = map_error(ServerFailure(code="query_timeout", message="deadline", operation="query"), operation="query")
    assert isinstance(error, QueryTimeout)


def test_maps_transport_os_error_to_unavailable() -> None:
    error = map_error(OSError("connection refused"))
    assert isinstance(error, Unavailable)


def test_public_exceptions_are_branchable() -> None:
    with pytest.raises(NotFound):
        raise map_error(ServerFailure(code="not_found", message="missing"))


def test_namespace_denied_code_string() -> None:
    assert ErrorCode.NAMESPACE_DENIED.value == "NamespaceDenied"
    assert NamespaceDenied.code is ErrorCode.NAMESPACE_DENIED


def test_grpc_permission_denied_maps_to_namespace_denied() -> None:
    error = map_error(_FakeRpcError("PERMISSION_DENIED", "namespace 'prod' is not granted"))
    assert isinstance(error, NamespaceDenied)
    assert not isinstance(error, Unauthenticated)
    assert str(error) == "namespace 'prod' is not granted"
    assert error.detail == "namespace 'prod' is not granted"


def test_grpc_unauthenticated_still_maps_to_unauthenticated() -> None:
    error = map_error(_FakeRpcError("UNAUTHENTICATED", "bad token"))
    assert isinstance(error, Unauthenticated)
    assert not isinstance(error, NamespaceDenied)
    assert str(error) == "bad token"


def test_wire_namespace_denied_lowercase_maps_to_namespace_denied() -> None:
    error = map_error(ServerFailure(code="namespace_denied", message="denied", detail="grant missing"))
    assert isinstance(error, NamespaceDenied)
    assert str(error) == "denied"
    assert error.detail == "grant missing"


def test_wire_namespace_denied_screaming_maps_to_namespace_denied() -> None:
    error = map_error(ServerFailure(code="WIRE_ERROR_CODE_NAMESPACE_DENIED", message="denied", detail="grant missing"))
    assert isinstance(error, NamespaceDenied)
    assert str(error) == "denied"
    assert error.detail == "grant missing"


def test_wire_numeric_namespace_denied_maps_to_namespace_denied() -> None:
    error = map_query_error(_WireQueryError(code=2, message="namespace denied", detail="grant missing"))
    assert isinstance(error, NamespaceDenied)
    assert not isinstance(error, QueryFailed)
    assert str(error) == "namespace denied"
    assert error.detail == "grant missing"


def test_map_query_error_never_rewraps_denial_as_query_failed() -> None:
    error = map_query_error(_WireQueryError(code="namespace_denied", message="denied", detail="grant missing"))
    assert isinstance(error, NamespaceDenied)
    assert not isinstance(error, QueryFailed)
    assert str(error) == "denied"
    assert error.detail == "grant missing"


def test_map_query_error_passes_namespace_denied_through_unchanged() -> None:
    original = NamespaceDenied("denied", detail="grant missing")
    assert map_query_error(original) is original


def test_map_error_passes_namespace_denied_through_unchanged() -> None:
    original = NamespaceDenied("denied", detail="grant missing")
    assert map_error(original) is original


def test_wire_invalid_input_lowercase_maps_to_invalid_argument() -> None:
    error = map_error(ServerFailure(code="invalid_input", message="bad input", detail="field x"))
    assert isinstance(error, InvalidArgument)
    assert not isinstance(error, ServerError)
    assert str(error) == "bad input"
    assert error.detail == "field x"


def test_wire_invalid_input_screaming_maps_to_invalid_argument() -> None:
    error = map_error(ServerFailure(code="WIRE_ERROR_CODE_INVALID_INPUT", message="bad input", detail="field x"))
    assert isinstance(error, InvalidArgument)
    assert not isinstance(error, ServerError)
    assert str(error) == "bad input"
    assert error.detail == "field x"


def test_wire_numeric_invalid_input_maps_to_invalid_argument() -> None:
    error = map_query_error(_WireQueryError(code=8, message="bad input", detail="field x"))
    assert isinstance(error, InvalidArgument)
    assert not isinstance(error, ServerError)
    assert str(error) == "bad input"
    assert error.detail == "field x"


def test_wire_numeric_backend_maps_to_server_error() -> None:
    error = map_query_error(_WireQueryError(code=9, message="backend failure", detail="store down"))
    assert isinstance(error, ServerError)
    assert not isinstance(error, InvalidArgument)
    assert str(error) == "backend failure"
    assert error.detail == "store down"


def test_wire_query_failed_lowercase_maps_to_query_failed() -> None:
    error = map_query_error(_WireQueryError(code="query_failed", message="handler raised", detail="boom"))
    assert isinstance(error, QueryFailed)
    assert not isinstance(error, ServerError)
    assert str(error) == "handler raised"
    assert error.detail == "boom"


def test_wire_query_failed_screaming_maps_to_query_failed() -> None:
    error = map_query_error(
        _WireQueryError(code="WIRE_ERROR_CODE_QUERY_FAILED", message="handler raised", detail="boom")
    )
    assert isinstance(error, QueryFailed)
    assert str(error) == "handler raised"
    assert error.detail == "boom"


def test_wire_numeric_query_failed_maps_to_query_failed() -> None:
    # Numeric pin: query_failed is wire enum value 10.
    error = map_query_error(_WireQueryError(code=10, message="handler raised", detail="boom"))
    assert isinstance(error, QueryFailed)
    assert not isinstance(error, ServerError)
    assert str(error) == "handler raised"
    assert error.detail == "boom"


def test_wire_query_failed_maps_to_query_failed_outside_query_operations() -> None:
    # The wire taxonomy is operation-independent: query_failed is QueryFailed
    # wherever it appears, never collapsed to ServerError.
    error = map_error(ServerFailure(code="query_failed", message="handler raised", detail="boom"))
    assert isinstance(error, QueryFailed)
    assert str(error) == "handler raised"
    assert error.detail == "boom"


def test_namespace_denied_is_branchable_and_distinct() -> None:
    with pytest.raises(NamespaceDenied):
        raise map_error(ServerFailure(code="namespace_denied", message="denied"))
    error = map_error(ServerFailure(code="namespace_denied", message="denied"))
    assert not isinstance(error, Unauthenticated)


# --- The wire-code table must not be allowed to fossilize again --------------
#
# Its predecessor was a hand-copied int-to-name mirror of the protobuf enum that
# stopped at value 10. Four codes were added to the contract; `not_owner` (13)
# and `invalid_state` (14) then arrived as ints, matched nothing, and were
# reported as generic server faults — a retryable routing fence made
# indistinguishable from a crashed backend, and a wrong-state refusal reported as
# an internal error. Nothing failed. The tests below are what makes the next
# addition fail instead.


def test_every_wire_error_code_in_the_contract_is_mapped() -> None:
    """Coverage is checked against the generated descriptor, not a copy of itself.

    This is the gate the old numeric mirror never had. A code added to
    `common.proto` and regenerated into the stubs now fails HERE, at the place
    that has to make a taxonomy decision about it, rather than silently landing in
    the server bucket.
    """

    # `.keys()` is the protobuf enum wrapper's own API listing every declared
    # member; it is not a dict, so iterating it directly would yield nothing.
    unmapped = sorted(
        name for name in common_pb2.WireErrorCode.keys() if _canonical_wire_code(name) not in _WIRE_CODE_MAP
    )
    assert not unmapped, (
        f"{unmapped} are values in the committed WireErrorCode contract with no entry in "
        "aion_client.errors._WIRE_CODE_MAP; they would be reported as generic server "
        "faults. Decide their taxonomy member deliberately — do not leave them to the "
        "fallback, which is how not_owner and invalid_state were lost."
    )


@pytest.mark.parametrize(
    ("code", "expected"),
    [
        # The four codes the fossilized table could not see, pinned by the
        # numeric form the gRPC wire actually delivers.
        (11, ServerError),  # deploy_denied — operator surface, not a caller error
        (12, ServerError),  # version_pinned — operator surface, not a caller error
        (13, Unavailable),  # not_owner — retryable shard fence, never "stop trying"
        (14, InvalidArgument),  # invalid_state — target exists, wrong state
    ],
)
def test_the_codes_added_after_the_stubs_froze_map_to_their_taxonomy_member(
    code: int, expected: type[Exception]
) -> None:
    error = map_query_error(_WireQueryError(code=code, message="wire failure", detail="context"))
    assert isinstance(error, expected), f"wire code {code} mapped to {type(error).__name__}"
    assert str(error) == "wire failure"


def test_not_owner_is_retryable_rather_than_a_server_fault() -> None:
    """The distinction the fossil erased, asserted both ways.

    `not_owner` means the request reached a node that does not own the target's
    shard: re-resolve the owner and retry. Classifying it as a server fault tells
    the caller the opposite, which is how a wrong-shard fence turns into a
    diagnosis hunt.
    """

    for code in ("not_owner", "WIRE_ERROR_CODE_NOT_OWNER", 13):
        error = map_query_error(_WireQueryError(code=code, message="not the shard owner"))
        assert isinstance(error, Unavailable), f"{code!r} mapped to {type(error).__name__}"
        assert not isinstance(error, ServerError)


def test_invalid_state_is_a_caller_error_rather_than_a_server_fault() -> None:
    """Pausing a run that is not Running is the caller's mistake, not the store's."""

    # `ServerFailure.code` is the JSON/WebSocket spelling (a string); the numeric
    # form only ever arrives on a protobuf message field, so each spelling is fed
    # in through the shape that actually carries it.
    for code in ("invalid_state", "WIRE_ERROR_CODE_INVALID_STATE"):
        error = map_error(ServerFailure(code=code, message="run is not Running"))
        assert isinstance(error, InvalidArgument), f"{code!r} mapped to {type(error).__name__}"
    numeric = map_query_error(_WireQueryError(code=14, message="run is not Running"))
    assert isinstance(numeric, InvalidArgument), f"numeric 14 mapped to {type(numeric).__name__}"


def test_an_unspecified_wire_code_is_a_server_fault_not_an_unknown_one() -> None:
    """Code 0 is proto3 padding: a failure the server declined to classify.

    It is a real value in the contract, so it must not be reported as
    "unrecognized" — that phrasing means "your stubs are behind the server", which
    would send the reader to regenerate stubs that are already current.
    """

    error = map_query_error(_WireQueryError(code=0, message="unclassified", detail="context"))
    assert isinstance(error, ServerError)
    assert error.detail == "context"
    assert "unrecognized" not in str(error.detail)


def test_a_code_outside_the_committed_contract_names_itself_in_the_detail() -> None:
    """An unclassifiable code must not be laundered into a bare "server error".

    A server newer than these stubs can send a code this SDK has never heard of.
    Server is the only honest classification, but the detail carries the number so
    there is something to search for — the alternative is an operator staring at
    "server error" with no thread to pull.
    """

    beyond = max(common_pb2.WireErrorCode.values()) + 1
    error = map_query_error(_WireQueryError(code=beyond, message="future failure"))
    assert isinstance(error, ServerError)
    assert error.detail is not None
    assert f"numeric_{beyond}" in error.detail
    assert "unrecognized wire error code" in error.detail


def test_an_unclassifiable_code_preserves_the_servers_own_detail() -> None:
    """The added diagnostic must be additive, never a replacement."""

    beyond = max(common_pb2.WireErrorCode.values()) + 1
    error = map_query_error(_WireQueryError(code=beyond, message="future failure", detail="store said so"))
    assert error.detail is not None
    assert error.detail.startswith("store said so")
    assert "unrecognized wire error code" in error.detail


def test_a_failure_with_no_code_at_all_is_a_backend_fault() -> None:
    """No code is not an unknown code; it is an unclassified server failure."""

    error = map_error(ServerFailure(code="", message="something broke"))
    assert isinstance(error, ServerError)
    assert error.detail is None or "unrecognized" not in error.detail
