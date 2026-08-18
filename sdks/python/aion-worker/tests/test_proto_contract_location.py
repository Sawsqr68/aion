"""The build hook's view of where the wire contract lives must stay true.

This SDK's stubs are generated at package-build time from `.proto` files that
live in the Rust tree. Nothing in the Rust build references this hook, so when
those files were relocated into `aion-proto-generated`, the hook kept pointing
at a directory that no longer existed and `pip install -e sdks/python/aion-worker`
failed for everyone — with no failing Rust test, no failing Python test, and no
signal at all until somebody tried to install it.

These tests are that signal. They fail the moment the contract moves again.
"""

from __future__ import annotations

import filecmp
from pathlib import Path

import pytest

from build_proto import (
    HAND_WRITTEN_STUBS,
    PROTO_FILES,
    PROTO_ROOT_PARTS,
    _find_repo_root,
    _patch_relative_imports,
    generate_proto_stubs,
)

SDK_ROOT = Path(__file__).resolve().parents[1]
COMMITTED_STUBS = SDK_ROOT / "aion_worker" / "proto"


def test_the_declared_proto_root_actually_exists() -> None:
    """The directory the hook compiles from must be a real directory."""

    proto_root = _find_repo_root(SDK_ROOT).joinpath(*PROTO_ROOT_PARTS)
    assert proto_root.is_dir(), (
        f"{proto_root} is not a directory; the shared wire contract has moved and "
        "PROTO_ROOT_PARTS in build_proto.py is stale"
    )


@pytest.mark.parametrize("name", PROTO_FILES)
def test_every_compiled_proto_file_is_present(name: str) -> None:
    """Each file the hook passes to protoc must exist, not just the anchor."""

    proto_root = _find_repo_root(SDK_ROOT).joinpath(*PROTO_ROOT_PARTS)
    assert (proto_root / name).is_file(), f"build_proto.py compiles {name}, but it is not in {proto_root}"


def test_a_missing_contract_names_the_path_it_looked_for(tmp_path: Path) -> None:
    """The failure must be diagnosable from its own message.

    A bare "could not locate" sends the reader hunting. The message has to name
    the path, because the whole failure mode is that the path changed.
    """

    with pytest.raises(RuntimeError) as raised:
        _find_repo_root(tmp_path)
    message = str(raised.value)
    assert "/".join(PROTO_ROOT_PARTS) in message
    assert "build_proto.py" in message


def test_the_committed_stubs_match_the_current_wire_contract(tmp_path: Path) -> None:
    """Regenerate the stubs and require them to be identical to what is committed.

    This is the gate that actually catches the rot. The location tests above
    only prove the contract is still WHERE the hook looks; they pass happily
    while the contract's CONTENT drifts. That is precisely how this SDK died:
    `RegisterWorker` lost `namespace` and gained `namespaces`/`task_queue`/
    `node`/`activities`/`identity`, the committed stubs kept the old shape, and
    every test in this suite went on passing against a protocol the server had
    stopped speaking. The SDK could not register with a live server, and
    nothing anywhere went red.

    A byte comparison is the right strictness. A "does it still have the fields
    we use" check would let a field the SDK does not yet send appear and go
    unnoticed — which is the exact failure, since `activities` was added and
    ignored. If this test fails, the fix is to regenerate and commit, then make
    the SDK speak whatever changed; it is never to relax the comparison.
    """

    regenerated = tmp_path / "proto"
    generate_proto_stubs(SDK_ROOT, output=regenerated)

    committed = sorted(path.name for path in COMMITTED_STUBS.iterdir() if path.is_file())
    produced = sorted(path.name for path in regenerated.iterdir())
    not_generated = {"__init__.py", *HAND_WRITTEN_STUBS}
    generated_names = [name for name in committed if name not in not_generated]
    assert generated_names == produced, (
        "the set of generated stub files changed: committed "
        f"{generated_names}, regenerated {produced}. If protoc started or stopped "
        "emitting a file, PROTO_FILES or HAND_WRITTEN_STUBS in build_proto.py is "
        "out of date — do not paper over it by widening the exclusion set."
    )

    mismatched = [
        name for name in generated_names if not filecmp.cmp(COMMITTED_STUBS / name, regenerated / name, shallow=False)
    ]
    assert not mismatched, (
        f"the committed protobuf stubs are stale: {mismatched} differ from what "
        f"{'/'.join(PROTO_ROOT_PARTS)} generates today. The SDK is speaking an "
        "older wire contract than the server. Regenerate and commit, then update "
        "the SDK for whatever changed — see the regeneration command in README.md."
    )


def test_the_generator_is_reproducible(tmp_path: Path) -> None:
    """Two runs of the generator must agree, or the drift gate above is noise.

    A gate that fails intermittently gets disabled, and a disabled gate is how
    the stubs went stale in the first place. If protoc ever starts emitting a
    timestamp or a nondeterministic ordering, this fails first and names the
    reason, instead of the drift test failing mysteriously on an unrelated
    change.
    """

    runs = []
    for index in range(2):
        target = tmp_path / f"run{index}"
        generate_proto_stubs(SDK_ROOT, output=target)
        runs.append(target)

    unstable = [
        path.name for path in sorted(runs[0].iterdir()) if not filecmp.cmp(path, runs[1] / path.name, shallow=False)
    ]
    assert not unstable, (
        f"protoc output is not reproducible for {unstable}; the staleness gate cannot be trusted until it is"
    )


def test_the_hand_written_service_stub_describes_the_generated_service() -> None:
    """The one part of the stub directory regeneration cannot check.

    grpcio-tools emits no `.pyi` for gRPC service classes, so `WorkerProtocolStub`
    has a hand-written TYPE and a generated IMPLEMENTATION. Nothing forces them
    to agree: if the service or its method were renamed in `worker.proto`, the
    generated module would follow and the hand-written stub would not, leaving
    mypy checking every call site against a fiction while the real call fails at
    runtime. That is the same silent-drift shape as the stale stubs, one level up.

    So it is checked against the thing it claims to describe — the generated
    module — rather than against a copy of itself.
    """

    from aion_worker.proto import worker_pb2_grpc

    stub_source = (COMMITTED_STUBS / "worker_pb2_grpc.pyi").read_text()
    declarations = (
        "WorkerProtocolStub",
        "StreamWorker",
        # The SERVER half of the same hand-written stub. Declared because the
        # SDK's own wire-level tests stand up a real `WorkerProtocol` service
        # to exercise the connection lease and the liveness probe, and an
        # undeclared servicer would be a second silent-drift surface of exactly
        # the shape this test exists to close.
        "WorkerProtocolServicer",
        "add_WorkerProtocolServicer_to_server",
    )
    for declared in declarations:
        assert declared in stub_source, f"the hand-written service stub no longer declares {declared!r}"

    for declared in ("WorkerProtocolStub", "WorkerProtocolServicer", "add_WorkerProtocolServicer_to_server"):
        assert hasattr(worker_pb2_grpc, declared), (
            f"the hand-written stub declares {declared!r} but the generated module does not "
            "define it; the service was renamed in worker.proto and the stub was not updated"
        )

    # Build the generated stub for real. Its `__init__` binds each RPC as an
    # INSTANCE attribute via the channel, so `hasattr` on the class proves
    # nothing and a source grep proves only that a string exists somewhere. A
    # recording channel is the check that actually fails when the RPC is gone.
    class _RecordingChannel:
        def __init__(self) -> None:
            self.methods: list[str] = []

        def stream_stream(self, method: str, *_args: object, **_kwargs: object) -> object:
            self.methods.append(method)
            return object()

    channel = _RecordingChannel()
    stub = worker_pb2_grpc.WorkerProtocolStub(channel)

    assert hasattr(stub, "StreamWorker"), (
        "the hand-written stub declares a StreamWorker method, but the generated "
        "service binds no such RPC; worker.proto renamed or removed it"
    )
    assert any(path.endswith("/StreamWorker") for path in channel.methods), (
        "StreamWorker is not registered as a stream-stream RPC on the generated "
        f"stub; it bound {channel.methods}. The hand-written type stub describes "
        "a bidirectional stream that no longer exists as one."
    )


def test_an_import_patch_that_matches_nothing_is_an_error(tmp_path: Path) -> None:
    """A silent no-op here ships a package that fails on the user's machine."""

    target = tmp_path / "worker_pb2.py"
    target.write_text("import something_else as other\n")
    with pytest.raises(RuntimeError) as raised:
        _patch_relative_imports(target, "import common_pb2", "from . import common_pb2")
    assert "no longer applies" in str(raised.value)


def test_patching_already_patched_output_is_accepted(tmp_path: Path) -> None:
    """Re-running the generator over patched output is not a failure."""

    target = tmp_path / "worker_pb2.py"
    already = "from . import common_pb2 as common__pb2\n"
    target.write_text(already)
    _patch_relative_imports(target, "import common_pb2 as common__pb2", "from . import common_pb2 as common__pb2")
    assert target.read_text() == already
