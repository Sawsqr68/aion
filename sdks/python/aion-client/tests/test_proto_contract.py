"""The committed protobuf stubs must stay identical to what the contract generates.

The stubs under `aion_client/proto` are generated from `.proto` files that live
in the Rust tree, and nothing in the Rust build references this package. The only
thing that had ever asked them to agree was a paragraph of prose in
`aion_client/proto/__init__.py`, and nobody performs a ritual to discover a
problem they do not know they have. So they drifted, silently, and the caller SDK
lost:

  * `StartWorkflowRequest.routing_key` and `.task_queue` — a Python caller could
    not target a shard or a task queue when starting a workflow;
  * the entire `Reopen` / `Pause` / `Resume` triple, request and response
    messages and RPCs alike;
  * `MintNamespace`, `MintNamespaceResponse`, and `NamespaceMintOrigin`;
  * `WORKFLOW_STATUS_PAUSED` and four `WireErrorCode` members.

None of it broke anything. proto3 is additive-tolerant, so every call the SDK
already made stayed valid and every test stayed green; what was lost was reach,
and reach fails quietly. That is the shape this file exists to make impossible.

The checks below are deliberately layered, because each one covers a way the
previous one can be true and still useless:

  * `test_the_committed_stubs_match_the_current_wire_contract` is the drift gate;
  * `test_the_generator_is_reproducible` keeps the drift gate trustworthy, since
    a gate that fails at random gets disabled and a disabled gate is exactly how
    this rot happened;
  * `test_every_generated_stub_declares_the_same_generator_version` and
    `test_the_package_requires_the_runtimes_its_generated_code_demands` turn the
    two ways a byte diff can be a false alarm — a version skew, an understated
    dependency floor — into their own named failures;
  * the hand-written-file tests cover what regeneration structurally cannot: a
    file with no prior generation to diff against.
"""

from __future__ import annotations

import filecmp
import re
from importlib import metadata
from pathlib import Path

import pytest

from proto_codegen import (
    HAND_WRITTEN_FILES,
    PROTO_FILES,
    PROTO_ROOT_PARTS,
    _find_repo_root,
    _patch_relative_imports,
    gencode_version,
    generate_proto_stubs,
    grpc_generated_version,
    version_tuple,
)

SDK_ROOT = Path(__file__).resolve().parents[1]
COMMITTED_STUBS = SDK_ROOT / "aion_client" / "proto"

# The filename shapes protoc emits. Used only to tell "looks generated" from
# "plainly is not"; the authoritative check that a file really came from protoc is
# the regenerate-and-compare test, which does not consult this at all.
_GENERATED_SUFFIXES = ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")

def _committed_source_names() -> list[str]:
    """Committed file names in the stub directory.

    Files only. Importing the package leaves a `__pycache__` DIRECTORY behind, so
    the file filter excludes it without needing a name-based exclusion — and no
    name-based exclusion is wanted here, since every exclusion is a place a real
    file could hide.
    """

    return sorted(path.name for path in COMMITTED_STUBS.iterdir() if path.is_file())


def test_the_declared_proto_root_actually_exists() -> None:
    """The directory the generator compiles from must be a real directory."""

    proto_root = _find_repo_root(SDK_ROOT).joinpath(*PROTO_ROOT_PARTS)
    assert proto_root.is_dir(), (
        f"{proto_root} is not a directory; the shared wire contract has moved and "
        "PROTO_ROOT_PARTS in proto_codegen.py is stale"
    )


@pytest.mark.parametrize("name", PROTO_FILES)
def test_every_compiled_proto_file_is_present(name: str) -> None:
    """Each file the generator passes to protoc must exist, not just the anchor."""

    proto_root = _find_repo_root(SDK_ROOT).joinpath(*PROTO_ROOT_PARTS)
    assert (proto_root / name).is_file(), f"proto_codegen.py compiles {name}, but it is not in {proto_root}"


def test_a_missing_contract_names_the_path_it_looked_for(tmp_path: Path) -> None:
    """The failure must be diagnosable from its own message.

    A bare "could not locate" sends the reader hunting. The message has to name
    the path, because the whole failure mode is that the path changed.
    """

    with pytest.raises(RuntimeError) as raised:
        _find_repo_root(tmp_path)
    message = str(raised.value)
    assert "/".join(PROTO_ROOT_PARTS) in message
    assert "proto_codegen.py" in message


def test_the_committed_stubs_match_the_current_wire_contract(tmp_path: Path) -> None:
    """Regenerate the stubs and require them to be identical to what is committed.

    A byte comparison is the right strictness, and the weaker alternative is what
    caused the damage. "Do the fields we use still exist?" was effectively the
    standard in force here, and it was satisfied throughout: `namespace`,
    `workflow_type`, and `input` never moved. Meanwhile two routing fields, three
    whole RPCs, and an entire namespace-minting surface came and went unnoticed,
    because a field the SDK does not yet send is invisible to a check that only
    looks at the fields it does.

    If this fails, the fix is to regenerate, commit, and then teach the SDK
    whatever changed. It is never to relax the comparison.
    """

    regenerated = tmp_path / "proto"
    generate_proto_stubs(SDK_ROOT, output=regenerated)

    committed = _committed_source_names()
    produced = sorted(path.name for path in regenerated.iterdir())
    generated_names = [name for name in committed if name not in HAND_WRITTEN_FILES]
    assert generated_names == produced, (
        f"the set of generated stub files changed: committed {generated_names}, "
        f"regenerated {produced}. If protoc started or stopped emitting a file, "
        "PROTO_FILES or HAND_WRITTEN_FILES in proto_codegen.py is out of date — "
        "do not paper over it by widening the exclusion set."
    )

    mismatched = [
        name for name in generated_names if not filecmp.cmp(COMMITTED_STUBS / name, regenerated / name, shallow=False)
    ]
    assert not mismatched, (
        f"the committed protobuf stubs are stale: {mismatched} differ from what "
        f"{'/'.join(PROTO_ROOT_PARTS)} generates today. The SDK is describing an "
        "older wire contract than the server speaks, which loses reach silently "
        "rather than failing. Regenerate with `uv run --extra dev python -m "
        "proto_codegen`, commit the result, then extend the SDK for whatever the "
        "contract gained."
    )


def test_the_generator_is_reproducible(tmp_path: Path) -> None:
    """Two runs of the generator must agree, or the drift gate above is noise.

    A gate that fails intermittently gets disabled, and a disabled gate is how
    the stubs went stale in the first place — so the gate must not be able to
    become the thing it replaced. If protoc ever starts emitting a timestamp or a
    nondeterministic ordering, this fails first and names the reason instead of
    the drift test failing mysteriously on an unrelated change.

    Running the generator twice also exercises the import rewrite against
    already-rewritten output, which is the case that corrupts a file into
    `from . from . import ...` when the already-patched check is not tested first.
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


def test_every_generated_stub_declares_the_same_generator_version() -> None:
    """One generator vintage across the whole directory.

    Regenerating a single file — a plausible thing to do when chasing one message
    that looks wrong — leaves the directory straddling two toolchains. The drift
    gate then fails on the files that were NOT touched, and reports them as stale
    against a contract they match perfectly, because the difference is a version
    banner. That is a different diagnosis and a different fix, so it gets its own
    named failure.

    Not hypothetical in kind: the committed stubs were produced by grpcio-tools
    1.81.1 (gencode 6.33.5) while the toolchain `uv.lock` pins is 1.83.0, so the
    first honest byte comparison here was always going to report a version
    difference alongside the real contract drift.
    """

    gencode = {path.name: gencode_version(path.read_text(encoding="utf-8")) for path in _pb2_modules()}
    assert len(set(gencode.values())) == 1, (
        f"the committed stubs carry mixed protobuf gencode versions {gencode}; they were "
        "generated by different toolchains and must be regenerated in one pass"
    )

    grpc_versions = {path.name: grpc_generated_version(path.read_text(encoding="utf-8")) for path in _grpc_modules()}
    assert len(set(grpc_versions.values())) == 1, (
        f"the committed stubs carry mixed grpcio plugin versions {grpc_versions}; they were "
        "generated by different toolchains and must be regenerated in one pass"
    )


def test_the_package_requires_the_runtimes_its_generated_code_demands() -> None:
    """Declared dependency floors must not be lower than the stubs enforce.

    Each generated `_pb2_grpc.py` raises `RuntimeError` on import when the
    installed grpcio is older than the plugin that produced it, and each `_pb2.py`
    calls `ValidateProtobufRuntimeVersion`. A package whose declared floor sits
    below those installs cleanly and then explodes on `import aion_client` — the
    worst failure shape available, since resolution succeeded and the user has no
    reason to suspect the dependency. `grpcio>=1.62` against stubs demanding
    1.81.1 was exactly that, live, before this test existed.
    """

    required_grpcio = max(
        (version_tuple(grpc_generated_version(path.read_text(encoding="utf-8"))) for path in _grpc_modules()),
    )
    required_protobuf = max(
        (version_tuple(gencode_version(path.read_text(encoding="utf-8"))) for path in _pb2_modules()),
    )

    for name, required in (("grpcio", required_grpcio), ("protobuf", required_protobuf)):
        declared = version_tuple(_declared_lower_bound(name))
        assert declared >= required, (
            f"aion-client declares {name}>={'.'.join(str(part) for part in declared)} but its "
            f"committed stubs demand {name}>={'.'.join(str(part) for part in required)}; the "
            "package installs on a runtime its own generated code refuses at import time. "
            "Raise the floor in pyproject.toml to match the generator."
        )


def test_the_hand_written_files_are_the_only_ungenerated_ones() -> None:
    """Every committed file is either protoc output or a declared exception.

    A stale generated file is caught by regenerating and diffing. A HAND-WRITTEN
    file dropped into a generated directory cannot be: there is no prior
    generation to diff it against, so regeneration says nothing about it and it
    becomes a forgery of provenance — code that reads as machine-generated,
    reviewed as machine-generated, and answerable to nothing. The allow-list is
    the mechanism that makes such a file a deliberate, named decision instead of
    an accident, so it has to be exhaustive.
    """

    declared = set(HAND_WRITTEN_FILES)
    committed = set(_committed_source_names())
    undeclared = sorted(name for name in committed - declared if not name.endswith(_GENERATED_SUFFIXES))
    assert not undeclared, (
        f"{undeclared} sit in the generated stub directory but are neither protoc "
        "output nor listed in proto_codegen.HAND_WRITTEN_FILES. Either they belong "
        "in aion_client/ proper, or they must be named in HAND_WRITTEN_FILES and "
        "given a behavioural check here — an unlisted hand-written file in a "
        "generated directory is invisible to every staleness check there is."
    )
    missing = sorted(declared - committed)
    assert not missing, (
        f"proto_codegen.HAND_WRITTEN_FILES names {missing}, which are not in "
        f"{COMMITTED_STUBS}; the allow-list has outlived the files it excuses and "
        "is now excusing nothing while looking like it excuses something"
    )


def test_the_hand_written_package_init_reexports_every_generated_module() -> None:
    """The one file regeneration cannot check, checked against what it describes.

    `aion_client/proto/__init__.py` is hand-written, so a generated module could
    be added and left unexported indefinitely — importable by full path, absent
    from the package surface, and silent about it. It is checked against the
    directory it fronts rather than against a copy of itself, and it is IMPORTED
    rather than grepped: a name in `__all__` that does not resolve is the failure
    this is for.
    """

    from aion_client import proto

    generated_modules = sorted(path.stem for path in _pb2_modules() + _grpc_modules())
    assert sorted(proto.__all__) == generated_modules, (
        f"aion_client/proto/__init__.py exports {sorted(proto.__all__)} but the directory "
        f"contains {generated_modules}; a generated module is missing from the package surface"
    )
    for name in proto.__all__:
        module = getattr(proto, name, None)
        assert module is not None, f"aion_client.proto.__all__ names {name!r}, which the package does not bind"
        assert getattr(module, "DESCRIPTOR", None) is not None or hasattr(module, "GRPC_GENERATED_VERSION"), (
            f"aion_client.proto.{name} is neither a protobuf message module nor a gRPC service module; "
            "the re-export points at something that is not a generated stub"
        )


def test_an_import_patch_that_matches_nothing_is_an_error(tmp_path: Path) -> None:
    """A silent no-op here ships a package that fails on the user's machine."""

    target = tmp_path / "workflow_pb2.py"
    target.write_text("import something_else as other\n", encoding="utf-8")
    with pytest.raises(RuntimeError) as raised:
        _patch_relative_imports(target, "import common_pb2", "from . import common_pb2")
    assert "no longer applies" in str(raised.value)


def test_patching_already_patched_output_is_accepted(tmp_path: Path) -> None:
    """Re-running the generator over patched output must be a no-op, not a corruption.

    `old` is a strict substring of `new`, so an implementation that tests for
    `old` first rewrites patched output into `from . from . import ...`. The
    documented workflow regenerates over committed files, so this is the ordinary
    path rather than an edge case.
    """

    target = tmp_path / "workflow_pb2.py"
    already = "from . import common_pb2 as common__pb2\n"
    target.write_text(already, encoding="utf-8")
    _patch_relative_imports(target, "import common_pb2 as common__pb2", "from . import common_pb2 as common__pb2")
    assert target.read_text(encoding="utf-8") == already


def test_the_stubs_carry_the_start_display_name_field_at_its_contract_tag() -> None:
    """`StartWorkflowRequest.display_name` must be present, optional, and tag 6 (#211).

    The byte-comparison gate above already fails on stale stubs, but it fails as
    an anonymous diff. This names the one field the SDK now depends on and pins
    its TAG, because a tag is what the wire actually carries: a field renumbered
    on either side keeps compiling, keeps every byte comparison against ITS OWN
    generator honest, and silently writes a display name into whatever field 6
    has become. Presence (`optional`) is checked too, since that is what lets
    "unnamed" be absence rather than an empty string.
    """

    from aion_client.proto import workflow_pb2

    field = workflow_pb2.StartWorkflowRequest.DESCRIPTOR.fields_by_name.get("display_name")
    assert field is not None, (
        "StartWorkflowRequest has no display_name field; the committed stubs predate #211. "
        "Regenerate with `uv run --extra dev python -m proto_codegen`."
    )
    assert field.number == 6, f"display_name is tag {field.number}, but the wire contract assigns it 6"
    assert field.has_presence, "display_name must be a proto3 `optional`, so an unnamed run is absence, not a blank"


def test_the_stubs_carry_the_rename_rpc_and_its_messages() -> None:
    """The `Rename` RPC and both its messages must exist (#211).

    This SDK exposes NO rename operation — renaming is a console/HTTP action in
    this release (`POST /workflows/rename`), not an SDK or CLI verb — so there is
    no caller reach to lose. The pin exists for a different and sharper reason:
    precisely BECAUSE nothing in `aion_client` touches these descriptors, nothing
    in the SDK's own test suite would notice if a regeneration dropped them.
    Contrast `StartWorkflowRequest.display_name` above, which `Client.start`
    assigns directly: lose that and the client tests break loudly on their own.
    `Rename` has no such witness, so this file is its only one.

    What it adds over the byte-comparison gate is the same thing the
    `display_name` pin adds — that gate fails as an ANONYMOUS diff, while this
    names the missing RPC and the command that restores it.

    Checked through the service descriptor rather than by module attribute, so
    the RPC itself is proven, not merely two message types that happen to be
    named for it.
    """

    from aion_client.proto import workflow_pb2

    service = workflow_pb2.DESCRIPTOR.services_by_name["WorkflowService"]
    method = service.FindMethodByName("Rename")
    assert method is not None, (
        "WorkflowService has no Rename RPC; the committed stubs predate #211. "
        "Regenerate with `uv run --extra dev python -m proto_codegen`."
    )
    assert method.input_type.name == "RenameRequest"
    assert method.output_type.name == "RenameResponse"

    request_fields = workflow_pb2.RenameRequest.DESCRIPTOR.fields_by_name
    assert set(request_fields) == {"namespace", "workflow_id", "run_id", "display_name"}
    response_fields = workflow_pb2.RenameResponse.DESCRIPTOR.fields_by_name
    assert set(response_fields) == {"run_id", "display_name"}


def test_a_generated_file_without_a_version_banner_is_refused(tmp_path: Path) -> None:
    """Version extraction must not invent an answer for input it cannot read."""

    with pytest.raises(RuntimeError, match="Protobuf Python Version"):
        gencode_version("# not protoc output\n")
    with pytest.raises(RuntimeError, match="GRPC_GENERATED_VERSION"):
        grpc_generated_version("# not protoc output\n")
    with pytest.raises(RuntimeError, match="dotted numeric version"):
        version_tuple("1.83.0rc1")


def _pb2_modules() -> list[Path]:
    return sorted(path for path in COMMITTED_STUBS.glob("*_pb2.py"))


def _grpc_modules() -> list[Path]:
    return sorted(COMMITTED_STUBS.glob("*_pb2_grpc.py"))


def _declared_lower_bound(distribution_name: str) -> str:
    """Return the `>=` floor this package declares for one runtime dependency.

    Read from the INSTALLED distribution metadata rather than by parsing
    `pyproject.toml`, because the metadata is what a user's resolver actually
    honours — and because `tomllib` does not exist on the oldest Python this
    package supports, so parsing the source would mean adding a dependency to
    check a dependency.

    Raises:
        RuntimeError: the requirement is absent or is not a simple `>=` bound.
            Both cases mean the floor cannot be compared, and a test that cannot
            compare must fail rather than pass.
    """

    requirements = metadata.requires("aion-client") or ()
    pattern = re.compile(rf"^{re.escape(distribution_name)}\s*(?P<specifier>[^;]*)$")
    for requirement in requirements:
        matched = pattern.match(requirement.strip())
        if matched is None:
            continue
        specifier = matched.group("specifier").strip()
        bound = re.fullmatch(r">=\s*(?P<version>[0-9][0-9.]*)", specifier)
        if bound is None:
            raise RuntimeError(
                f"aion-client declares {distribution_name!r} as {specifier!r}; this check "
                "understands a single `>=` floor only, and guessing at a compound "
                "specifier would compare the wrong bound"
            )
        return bound.group("version")
    raise RuntimeError(
        f"aion-client declares no requirement on {distribution_name!r}, but its committed "
        f"stubs import it; either the dependency was dropped from pyproject.toml or the "
        f"installed metadata is stale (reinstall with `uv sync --extra dev`). Declared: {sorted(requirements)}"
    )
