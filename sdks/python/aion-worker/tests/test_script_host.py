"""The script host: a decorated `.py` file must be servable as written.

The story this SDK is for is a single-file script that does real work. These
tests cover the two halves of making that safe: loading arbitrary user code by
path without damaging the interpreter it is loaded into, and refusing to guess
any connection setting the operator did not supply.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import pytest

from aion_worker import ScriptLoadError, default_registry, load_activity_script
from aion_worker.host import ConfigurationError, build_worker_config, main

EXAMPLE = Path(__file__).resolve().parents[4] / "examples" / "python-worker" / "image_worker.py"

_DECORATED_SCRIPT = """
from dataclasses import dataclass

from aion_worker import activity


@dataclass
class Request:
    path: str


@dataclass
class Facts:
    width: int


@activity(name="{name}")
def handler(input: Request) -> Facts:
    return Facts(width=len(input.path))
"""


@pytest.fixture(autouse=True)
def _isolate_the_process_global_registry() -> object:
    """Undo registrations and `sys.path`/`sys.modules` edits between tests.

    `@activity` writes to a process-global registry by design — that is what
    lets a script register by merely being imported — so a test that loads a
    script would otherwise leak its activity into every later test.
    """

    registered = dict(default_registry._activities)
    modules = set(sys.modules)
    path = list(sys.path)
    yield None
    default_registry._activities.clear()
    default_registry._activities.update(registered)
    for name in set(sys.modules) - modules:
        del sys.modules[name]
    sys.path[:] = path


def _write(directory: Path, filename: str, name: str = "images.classify") -> Path:
    script = directory / filename
    script.write_text(_DECORATED_SCRIPT.format(name=name))
    return script


# --------------------------------------------------------------------------- #
# loading
# --------------------------------------------------------------------------- #


def test_a_decorated_file_exposes_its_activity_types(tmp_path: Path) -> None:
    """The whole ergonomic claim: decorate, point a command at it, it serves."""

    registry = load_activity_script(_write(tmp_path, "classify.py"))

    assert "images.classify" in list(registry.activity_types())
    descriptors = {d.name: d for d in registry.activity_descriptors()}
    assert json.loads(descriptors["images.classify"].output_schema_json) == {
        "type": "object",
        "properties": {"width": {"type": "integer"}},
        "required": ["width"],
    }


def test_the_shipped_example_loads_and_advertises_its_declared_surface() -> None:
    """The acceptance target itself, served exactly as written.

    `examples/python-worker/image_worker.py` is a real single-file worker whose
    counterpart AWL declares `inspect_image(path: String) -> ImageFacts`. The
    schemas asserted here are what that AWL contract carries, so a drift in
    either direction shows up as a diff instead of as a registration refused by
    a live server.
    """

    registry = load_activity_script(EXAMPLE)

    assert list(registry.activity_types()) == ["inspect_image"]
    descriptor = registry.activity_descriptors()[0]
    assert json.loads(descriptor.input_schema_json) == {
        "type": "object",
        "properties": {"path": {"type": "string"}},
        "required": ["path"],
    }
    assert json.loads(descriptor.output_schema_json) == {
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "format": {"type": "string"},
            "width": {"type": "integer"},
            "height": {"type": "integer"},
            "bytes": {"type": "integer"},
        },
        "required": ["path", "format", "width", "height", "bytes"],
    }


def test_a_script_named_after_a_stdlib_module_cannot_shadow_it(tmp_path: Path) -> None:
    """A worker called `json.py` must not become the `json` everything imports.

    The script is loaded under a private, prefixed module name that no `import`
    statement can spell, so its filename is irrelevant to the import system —
    and its directory is APPENDED to `sys.path`, never prepended, so a sibling
    named after a standard-library module can never win over the real one.
    """

    script = _write(tmp_path, "json.py", name="shadow.check")
    (tmp_path / "struct.py").write_text("raise AssertionError('the decoy was imported')")

    load_activity_script(script)

    assert sys.modules["json"] is json, "the real json module must be untouched"
    assert "struct" not in sys.modules or sys.modules["struct"].__file__ != str(tmp_path / "struct.py")
    import struct  # noqa: PLC0415 - importing here is the assertion

    assert Path(struct.__file__ or "").parent != tmp_path


def test_a_sibling_module_beside_the_script_is_importable(tmp_path: Path) -> None:
    """A script may still grow a helper file next to it."""

    (tmp_path / "helper_module.py").write_text("WIDTH = 41\n")
    script = tmp_path / "uses_helper.py"
    script.write_text(
        "from dataclasses import dataclass\n"
        "from aion_worker import activity\n"
        "import helper_module\n"
        "@dataclass\n"
        "class Facts:\n"
        "    width: int\n"
        "@activity(name='helper.check')\n"
        "def handler(input: dict) -> Facts:\n"
        "    return Facts(width=helper_module.WIDTH)\n"
    )

    registry = load_activity_script(script)

    assert "helper.check" in list(registry.activity_types())


def test_a_scripts_own_main_block_does_not_run(tmp_path: Path) -> None:
    """The host serves the handlers; it does not run the script's entry point.

    `__name__` is the private module name, never `"__main__"`, so a script that
    can also be run standalone does not try to start a second worker while
    being loaded into this one.
    """

    script = tmp_path / "with_main.py"
    script.write_text(
        "from dataclasses import dataclass\n"
        "from aion_worker import activity\n"
        "@dataclass\n"
        "class Facts:\n"
        "    width: int\n"
        "@activity(name='main.check')\n"
        "def handler(input: dict) -> Facts:\n"
        "    return Facts(width=1)\n"
        "if __name__ == '__main__':\n"
        "    raise AssertionError('the script entry point must not fire')\n"
    )

    registry = load_activity_script(script)

    assert "main.check" in list(registry.activity_types())


def test_a_file_with_no_activities_is_refused_with_an_actionable_message(
    tmp_path: Path,
) -> None:
    """Serving nothing is never what the operator meant."""

    script = tmp_path / "empty.py"
    script.write_text("value = 1\n")

    with pytest.raises(ScriptLoadError, match="registered no activities"):
        load_activity_script(script)


def test_a_missing_file_names_the_path_it_looked_for(tmp_path: Path) -> None:
    with pytest.raises(ScriptLoadError, match="no such file"):
        load_activity_script(tmp_path / "absent.py")


def test_a_script_that_raises_while_importing_surfaces_the_cause(tmp_path: Path) -> None:
    """Executing the file IS the registration mechanism, so failures must show."""

    script = tmp_path / "broken.py"
    script.write_text("raise RuntimeError('module-level boom')\n")

    with pytest.raises(ScriptLoadError, match="module-level boom"):
        load_activity_script(script)
    assert "_aion_worker_script_broken" not in sys.modules, "a half-executed module must not be left importable"


# --------------------------------------------------------------------------- #
# configuration
# --------------------------------------------------------------------------- #


def _arguments(**overrides: object) -> argparse.Namespace:
    values: dict[str, object] = {
        "endpoint": "127.0.0.1:50099",
        "task_queue": "python_box",
        "identity": "image-worker-1",
        "concurrency": "4",
        "reconnect_initial_backoff": "0.1",
        "reconnect_max_backoff": "5",
        "reconnect_max_attempts": "10",
        "namespace": None,
        "node": None,
        "subject": "worker",
        "token": None,
    }
    values.update(overrides)
    return argparse.Namespace(**values)


def test_flags_produce_a_complete_worker_config() -> None:
    built = build_worker_config(_arguments(namespace=["payments", "billing"], node="host-7"))

    assert built.endpoint == "127.0.0.1:50099"
    assert built.task_queue == "python_box"
    assert built.identity == "image-worker-1"
    assert built.max_concurrency == 4
    assert built.namespaces == ("payments", "billing")
    assert built.node == "host-7"
    assert built.reconnect.initial_backoff_seconds == 0.1
    assert built.reconnect.max_backoff_seconds == 5
    assert built.reconnect.max_attempts == 10


def test_the_environment_supplies_what_the_flags_omit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The variable names are the ones the rest of the product already reads."""

    monkeypatch.setenv("AION_WORKER_ENDPOINT", "127.0.0.1:50099")
    monkeypatch.setenv("AION_TASK_QUEUE", "python_box")
    monkeypatch.setenv("AION_WORKER_IDENTITY", "from-env")
    monkeypatch.setenv("AION_WORKER_CONCURRENCY", "2")
    monkeypatch.setenv("AION_RECONNECT_INITIAL_BACKOFF_SECONDS", "0.25")
    monkeypatch.setenv("AION_RECONNECT_MAX_BACKOFF_SECONDS", "8")
    monkeypatch.setenv("AION_RECONNECT_MAX_ATTEMPTS", "3")

    built = build_worker_config(
        _arguments(
            endpoint=None,
            task_queue=None,
            identity=None,
            concurrency=None,
            reconnect_initial_backoff=None,
            reconnect_max_backoff=None,
            reconnect_max_attempts=None,
        )
    )

    assert built.identity == "from-env"
    assert built.max_concurrency == 2
    assert built.reconnect.initial_backoff_seconds == 0.25


def test_a_flag_beats_the_environment(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("AION_WORKER_IDENTITY", "from-env")

    built = build_worker_config(_arguments(identity="from-flag"))

    assert built.identity == "from-flag"


def test_every_missing_setting_is_named_in_one_report(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Nothing is guessed, and the report is complete.

    A host that reported one missing value at a time would cost seven
    round-trips to start a worker; naming them together costs one.
    """

    for variable in (
        "AION_WORKER_ENDPOINT",
        "AION_TASK_QUEUE",
        "AION_WORKER_IDENTITY",
        "AION_WORKER_CONCURRENCY",
        "AION_RECONNECT_INITIAL_BACKOFF_SECONDS",
        "AION_RECONNECT_MAX_BACKOFF_SECONDS",
        "AION_RECONNECT_MAX_ATTEMPTS",
    ):
        monkeypatch.delenv(variable, raising=False)

    with pytest.raises(ConfigurationError) as raised:
        build_worker_config(
            _arguments(
                endpoint=None,
                task_queue=None,
                identity=None,
                concurrency=None,
                reconnect_initial_backoff=None,
                reconnect_max_backoff=None,
                reconnect_max_attempts=None,
            )
        )

    message = str(raised.value)
    for flag in (
        "--endpoint",
        "--task-queue",
        "--identity",
        "--concurrency",
        "--reconnect-initial-backoff",
        "--reconnect-max-backoff",
        "--reconnect-max-attempts",
    ):
        assert flag in message, f"{flag} must be named in the missing-settings report"
    assert "AION_WORKER_ENDPOINT" in message, "each setting names its environment variable too"


@pytest.mark.parametrize(
    ("override", "expected"),
    [
        ({"concurrency": "many"}, "whole number"),
        ({"concurrency": "0"}, "greater than zero"),
        ({"reconnect_initial_backoff": "soon"}, "must be a number"),
        ({"reconnect_max_attempts": "-1"}, "greater than zero"),
    ],
)
def test_an_unusable_value_is_reported_rather_than_coerced(override: dict[str, object], expected: str) -> None:
    with pytest.raises(ConfigurationError, match=expected):
        build_worker_config(_arguments(**override))


def test_a_token_becomes_bearer_authorization_metadata() -> None:
    """Matching how the Rust client sends a token: `authorization: Bearer <t>`."""

    built = build_worker_config(_arguments(token="secret-token"))

    assert built.transport_credentials is not None
    assert built.transport_credentials.metadata == (("authorization", "Bearer secret-token"),)
    assert ("authorization", "Bearer secret-token") in built.grpc_metadata()


def test_no_token_sends_no_authorization_metadata(monkeypatch: pytest.MonkeyPatch) -> None:
    """When flag and env are both absent the development header paths apply."""

    monkeypatch.delenv("AION_TOKEN", raising=False)

    built = build_worker_config(_arguments())

    assert built.transport_credentials is None
    assert not any(key == "authorization" for key, _ in built.grpc_metadata())


def test_the_token_flag_overrides_the_environment(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("AION_TOKEN", "from-env")

    built = build_worker_config(_arguments(token="from-flag"))

    assert built.transport_credentials is not None
    assert built.transport_credentials.metadata == (("authorization", "Bearer from-flag"),)


# --------------------------------------------------------------------------- #
# the command
# --------------------------------------------------------------------------- #


def test_check_prints_the_action_surface_without_connecting(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """The descriptors ARE the admission decision, so they must be inspectable.

    Being able to see them before dialling turns a remote `FailedPrecondition`
    schema diff into something checkable at the desk.
    """

    script = _write(tmp_path, "surface.py", name="surface.check")

    assert main([str(script), "--check"]) == 0

    surface = json.loads(capsys.readouterr().out)
    assert [entry["name"] for entry in surface] == ["surface.check"]
    assert surface[0]["output_schema"]["required"] == ["width"]


def test_a_missing_setting_exits_non_zero_without_dialling(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    for variable in ("AION_WORKER_ENDPOINT", "AION_TASK_QUEUE", "AION_WORKER_IDENTITY"):
        monkeypatch.delenv(variable, raising=False)
    script = _write(tmp_path, "needs_config.py", name="config.check")

    assert main([str(script)]) == 2
    assert "--endpoint" in capsys.readouterr().err


def test_an_unadvertisable_return_is_refused_before_the_process_dials(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A local error the author can act on, not a remote schema diff."""

    script = tmp_path / "opaque.py"
    script.write_text(
        "from typing import Any\n"
        "from aion_worker import activity\n"
        "@activity(name='opaque.check')\n"
        "def handler(input: dict) -> Any:\n"
        "    return {}\n"
    )

    assert main([str(script), "--check"]) == 2
    assert "opaque.check" in capsys.readouterr().err


def test_a_script_that_registers_nothing_exits_non_zero(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    script = tmp_path / "bare.py"
    script.write_text("value = 1\n")

    assert main([str(script), "--check"]) == 2
    assert "@activity" in capsys.readouterr().err
