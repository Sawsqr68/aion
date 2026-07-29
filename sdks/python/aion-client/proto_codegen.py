"""Regenerates the caller SDK's protobuf stubs from the shared wire contract.

The stubs under `aion_client/proto` are COMMITTED artifacts generated from
`.proto` files that live in the Rust tree. Nothing in the Rust build references
this file, so the only thing keeping the two in step is
`tests/test_proto_contract.py`, which regenerates into a scratch directory and
byte-compares. Without that gate the stubs silently rot: they did, for long
enough to lose `StartWorkflowRequest.routing_key`, `StartWorkflowRequest.
task_queue`, the whole `Reopen`/`Pause`/`Resume` triple, and `MintNamespace`.

Deliberately NOT a hatchling build hook, unlike `sdks/python/aion-worker/
build_proto.py`
----------------------------------------------------------------------------
The worker SDK wires its generator in as `[tool.hatch.build.hooks.custom]`, so
its stubs are regenerated whenever the package is built. This one is not, for a
reason specific to how this package declares its dependencies.

A build hook regenerates with whatever `grpcio-tools` the isolated build
environment resolves at BUILD time, which is unpinned. The generated stubs then
enforce that toolchain's versions at import: every `_pb2_grpc.py` raises below
its plugin's grpcio version and every `_pb2.py` calls
`ValidateProtobufRuntimeVersion`. This package's `grpcio` and `protobuf` floors
are derived from exactly those stamps (see the comment on `dependencies` in
pyproject.toml), so a hook would let a wheel ship stubs demanding a newer runtime
than the wheel's own metadata requires — resolving cleanly and then failing on
`import aion_client`. That is the precise failure `tests/test_proto_contract.py`
exists to prevent, and a hook would reintroduce it at build time where no test
can see it.

The cost is that installing never regenerates, so the committed stubs must be
correct in the repository. That is what the gate enforces, and it enforces it on
the bytes consumers actually receive.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

# Where the shared wire contract lives, relative to the repository root.
#
# This is a path into ANOTHER language's tree, so nothing in the Rust build
# breaks when it moves. The worker SDK has already been bitten by exactly that:
# the `.proto` files were relocated into `aion-proto-generated` and its build
# hook kept pointing at the old directory. If this constant is edited,
# `tests/test_proto_contract.py` is what proves the new value is real.
PROTO_ROOT_PARTS = ("crates", "aion-proto-generated", "proto")

# The contract files this SDK compiles. `workflow.proto` is the caller-facing
# service; `common.proto` carries the shared identifier/payload/error messages
# it imports and must be compiled too, because the generated `workflow_pb2`
# imports `common_pb2` as a sibling module.
PROTO_FILES = ("common.proto", "workflow.proto")

# Files that live in the generated directory but are NOT generated.
#
# A stale generated file is caught by regenerating and comparing. A HAND-WRITTEN
# file among generated neighbours cannot be: there is nothing to regenerate it
# from, so the diff is silent about it forever. Naming them here lets the gate
# exclude them deliberately rather than by accident, and check each one
# behaviourally against the generated modules it claims to describe — see
# `tests/test_proto_contract.py`.
#
# `__init__.py` is the only one. Unlike the worker SDK this package hand-writes
# no `_pb2_grpc.pyi` service stubs; the gRPC service class is reached through
# `importlib` in `aion_client/transport.py`, which mypy does not follow, so a
# hand-written type stub would describe a module nothing type-checks against.
HAND_WRITTEN_FILES = ("__init__.py",)

# protoc emits flat top-level imports; the stubs ship inside a package, so each
# one has to be rewritten to a relative import or importing the package fails.
_RELATIVE_IMPORT_PATCHES = (
    ("workflow_pb2.py", "import common_pb2 as common__pb2", "from . import common_pb2 as common__pb2"),
    ("workflow_pb2.pyi", "import common_pb2 as _common_pb2", "from . import common_pb2 as _common_pb2"),
    ("workflow_pb2_grpc.py", "import workflow_pb2 as workflow__pb2", "from . import workflow_pb2 as workflow__pb2"),
)

# The protobuf gencode version protoc stamps into every `_pb2.py`, and the
# grpcio version the gRPC plugin stamps into every `_pb2_grpc.py`. Read out of
# the stubs so the gate can report a generator-version skew as a version skew
# instead of as an unexplained byte difference, and so the package's declared
# dependency floors can be checked against what its own generated code demands.
_GENCODE_VERSION_PATTERN = re.compile(r"^# Protobuf Python Version: (\d+\.\d+\.\d+)", re.MULTILINE)
_GRPC_VERSION_PATTERN = re.compile(r"^GRPC_GENERATED_VERSION = '(\d+\.\d+\.\d+)'", re.MULTILINE)


def generate_proto_stubs(root: Path, output: Path | None = None) -> None:
    """Regenerate the gRPC stubs from the repository's `.proto` sources.

    `root` locates the wire contract; `output` is where the stubs are written,
    defaulting to this package's own `aion_client/proto`. They are separable so
    the staleness gate can generate into a scratch directory and compare the
    result against what is committed — a check that is only meaningful if it can
    generate WITHOUT overwriting the very files it is about to inspect.

    Raises:
        RuntimeError: the wire contract could not be located, or an import
            rewrite no longer applies.
        subprocess.CalledProcessError: protoc rejected the contract.
    """

    proto_root = _find_repo_root(root).joinpath(*PROTO_ROOT_PARTS)
    output = output if output is not None else root / "aion_client" / "proto"
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


def gencode_version(source: str) -> str:
    """Return the protobuf gencode version protoc stamped into a `_pb2.py`.

    Raises:
        RuntimeError: the banner is absent, which means the file is not protoc
            output of the shape this SDK compiles and no version claim about it
            can be trusted.
    """

    return _single_version(_GENCODE_VERSION_PATTERN, source, "# Protobuf Python Version:")


def grpc_generated_version(source: str) -> str:
    """Return the grpcio version the gRPC plugin stamped into a `_pb2_grpc.py`.

    This is the floor the generated module enforces at import time: it raises
    `RuntimeError` when the installed grpcio is older. A package declaring a
    lower floor than this installs cleanly and then fails on `import`.

    Raises:
        RuntimeError: the constant is absent.
    """

    return _single_version(_GRPC_VERSION_PATTERN, source, "GRPC_GENERATED_VERSION")


def version_tuple(version: str) -> tuple[int, ...]:
    """Parse a dotted numeric version into a comparable tuple.

    Raises:
        RuntimeError: a component is not an integer. The versions this handles
            are protoc's and grpcio's own stamped constants plus this package's
            declared floors, all of which are plain `major.minor.patch`; a
            pre-release or local segment means an assumption here has broken and
            silently coercing it would compare the wrong things.
    """

    parts = version.split(".")
    if not all(part.isdigit() for part in parts):
        raise RuntimeError(
            f"{version!r} is not a plain dotted numeric version; the generator or "
            "dependency floor now uses a form proto_codegen.version_tuple cannot compare"
        )
    return tuple(int(part) for part in parts)


def _single_version(pattern: re.Pattern[str], source: str, label: str) -> str:
    matches = pattern.findall(source)
    if not matches:
        raise RuntimeError(f"no {label} line found; the file is not the protoc output this SDK generates")
    if len(set(matches)) != 1:
        raise RuntimeError(f"conflicting {label} lines {sorted(set(matches))} in one generated file")
    return str(matches[0])


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
        "sdks/python/aion-client/proto_codegen.py must be updated to match"
    )


def _patch_relative_imports(path: Path, old: str, new: str) -> None:
    """Rewrite one flat protoc import to a package-relative one.

    A patch that silently matched nothing would ship a package that imports
    cleanly here and fails at `import aion_client` on a user's machine, so a
    miss is an error rather than a no-op.

    The already-patched check has to come FIRST and it has to be the only thing
    that decides. `old` is a strict SUBSTRING of `new` — "import common_pb2 as
    common__pb2" occurs inside "from . import common_pb2 as common__pb2" — so
    testing for `old` first succeeds on already-patched output and the
    replacement produces `from . from . import ...`, corrupting the file. That is
    not hypothetical: regenerate-and-commit is the documented workflow, and the
    gate runs the generator twice in a row to prove reproducibility, so a second
    pass over patched output is a routine event rather than an edge case.

    Raises:
        RuntimeError: the file contains neither the flat import nor its patched
            form, meaning protoc's import style changed.
    """

    content = path.read_text(encoding="utf-8")
    if new in content:
        return
    if old not in content:
        raise RuntimeError(
            f"{path} contains neither the expected protoc import {old!r} nor its "
            f"patched form {new!r}; the generator's import style changed and the "
            "rewrite in proto_codegen.py no longer applies"
        )
    path.write_text(content.replace(old, new), encoding="utf-8")


def main() -> int:
    """Regenerate the committed stubs in place.

    Exposed so the documented regeneration ritual is a command rather than a
    paragraph of prose to retype: `python -m proto_codegen` from this directory.
    """

    generate_proto_stubs(Path(__file__).resolve().parent)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
