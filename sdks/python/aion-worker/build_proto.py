"""Hatch build hook that generates AW-owned worker protobuf stubs."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from hatchling.builders.config import BuilderConfig
from hatchling.builders.hooks.plugin.interface import BuildHookInterface

# Where the shared wire contract lives, relative to the repository root.
#
# This is a path into ANOTHER language's tree, so nothing in the Rust build
# breaks when it moves — which is exactly what happened once already: the
# `.proto` files were relocated into `aion-proto-generated` and this hook kept
# pointing at the old directory, leaving the Python worker SDK uninstallable
# with no Rust-side signal at all. The regression guard for that is
# `tests/test_proto_contract_location.py`; if this constant is edited, that
# test is what proves the new value is real.
PROTO_ROOT_PARTS = ("crates", "aion-proto-generated", "proto")

# The stubs generated from that contract. Named here so the anchor file used to
# find the repository root is the same one the generator actually compiles.
PROTO_FILES = ("common.proto", "worker.proto")

# Type stubs that live in the generated directory but are NOT generated.
#
# grpcio-tools emits no `.pyi` for the gRPC service classes, so these are
# hand-maintained alongside generated neighbours — `WorkerProtocolStub`'s TYPE
# is hand-written while its IMPLEMENTATION is generated. That makes them the one
# part of this directory the staleness gate cannot catch by regenerating, since
# there is nothing to regenerate them from. They are named here so the gate can
# exclude them deliberately rather than by accident, and check them against the
# generated module instead — see `tests/test_proto_contract_location.py`.
HAND_WRITTEN_STUBS = ("common_pb2_grpc.pyi", "worker_pb2_grpc.pyi")

# protoc emits flat top-level imports; the stubs ship inside a package, so each
# one has to be rewritten to a relative import or importing the package fails.
_RELATIVE_IMPORT_PATCHES = (
    ("worker_pb2.py", "import common_pb2 as common__pb2", "from . import common_pb2 as common__pb2"),
    (
        "worker_pb2.pyi",
        "import common_pb2 as _common_pb2",
        "from . import common_pb2 as _common_pb2",
    ),
    (
        "worker_pb2_grpc.py",
        "import worker_pb2 as worker__pb2",
        "from . import worker_pb2 as worker__pb2",
    ),
)


class CustomBuildHook(BuildHookInterface[BuilderConfig]):
    """Generate Python gRPC stubs from the shared wire contract before packaging."""

    def initialize(self, version: str, build_data: dict[str, object]) -> None:
        generate_proto_stubs(Path(self.root))


def generate_proto_stubs(root: Path, output: Path | None = None) -> None:
    """Regenerate the gRPC stubs from the repository's `.proto` sources.

    `root` locates the wire contract; `output` is where the stubs are written,
    defaulting to this package's own `aion_worker/proto`. They are separable so
    the staleness gate can regenerate into a scratch directory and compare the
    result against what is committed — a check that is only meaningful if it
    can generate WITHOUT overwriting the very files it is about to inspect.
    """

    proto_root = _find_repo_root(root).joinpath(*PROTO_ROOT_PARTS)
    output = output if output is not None else root / "aion_worker" / "proto"
    output.mkdir(parents=True, exist_ok=True)
    command = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        "-I",
        str(proto_root),
        f"--python_out={output}",
        f"--pyi_out={output}",
        f"--grpc_python_out={output}",
        *(str(proto_root / name) for name in PROTO_FILES),
    ]
    subprocess.run(command, check=True)
    for filename, old, new in _RELATIVE_IMPORT_PATCHES:
        _patch_relative_imports(output / filename, old, new)


def _find_repo_root(root: Path) -> Path:
    """Return the ancestor of `root` that contains the shared wire contract.

    Raises with the exact path that was searched for, because the failure this
    guards against is the contract having MOVED — and a message naming the
    directory it expected is the difference between a one-line fix and an
    afternoon.
    """

    anchor = Path(*PROTO_ROOT_PARTS, PROTO_FILES[-1])
    for candidate in (root, *root.parents):
        if (candidate / anchor).exists():
            return candidate
    raise RuntimeError(
        f"could not locate {anchor.as_posix()} in {root} or any of its parents; "
        "the shared wire contract has moved and PROTO_ROOT_PARTS in "
        "sdks/python/aion-worker/build_proto.py must be updated to match"
    )


def _patch_relative_imports(path: Path, old: str, new: str) -> None:
    """Rewrite one flat protoc import to a package-relative one.

    A patch that silently matched nothing would ship a package that imports
    cleanly here and fails at `import aion_worker` on a user's machine, so a
    miss is an error rather than a no-op.

    The already-patched check has to come FIRST and it has to be the only thing
    that decides. `old` is a strict substring of `new` — "import common_pb2 as
    common__pb2" occurs inside "from . import common_pb2 as common__pb2" — so
    testing for `old` on patched output succeeds and the replacement produces
    `from . from . import ...`, corrupting the file. That is not hypothetical:
    the README instructs a regenerate-and-commit cycle, and a second pass over
    committed output is exactly the documented workflow.
    """

    content = path.read_text()
    if new in content:
        return
    if old not in content:
        raise RuntimeError(
            f"{path} contains neither the expected protoc import {old!r} nor its "
            f"patched form {new!r}; the generator's import style changed and the "
            "rewrite in build_proto.py no longer applies"
        )
    path.write_text(content.replace(old, new))
