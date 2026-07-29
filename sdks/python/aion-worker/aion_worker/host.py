"""Script host: serve a decorated `.py` file as an Aion worker.

A single-file script that does real work — measure an image, reshape a CSV,
call a model — should not have to carry an asyncio entry point, a
`WorkerConfig` literal, and a `Worker(...).run()` incantation. This module is
that boilerplate, once, so a file containing only handlers and `@activity` can
be served by pointing a command at it::

    python -m aion_worker script.py --endpoint 127.0.0.1:50051 --task-queue q ...

Flag vocabulary matches `aion worker` in the Rust CLI (`--endpoint`,
`--namespace`, `--subject`, `--token`) so the two read as one product. Every
connection value comes from a flag or an environment variable and NONE is
invented: a missing setting is reported by name, never guessed.
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
import importlib.util
import json
import logging
import os
import re
import signal
import sys
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType

from .activity import ActivityRegistrationError, ActivityRegistry, default_registry
from .session import (
    DEFAULT_WORKER_NAMESPACE,
    DEFAULT_WORKER_SUBJECT,
    ReconnectConfig,
    TransportCredentials,
    WorkerConfig,
    default_node,
)
from .worker import Worker

__all__ = ["ScriptLoadError", "build_worker_config", "load_activity_script", "main"]

logger = logging.getLogger("aion_worker.host")

# Every connection setting, its flag, and the environment variable that supplies
# it when the flag is absent. The variable names are the ones the rest of the
# product already uses (`aion-package`'s generated Rust and Python workers read
# exactly these), so a deployment that already runs an Aion worker can run this
# one with the same environment.
_ENDPOINT_ENV = "AION_WORKER_ENDPOINT"
_TASK_QUEUE_ENV = "AION_TASK_QUEUE"
_IDENTITY_ENV = "AION_WORKER_IDENTITY"
_CONCURRENCY_ENV = "AION_WORKER_CONCURRENCY"
_INITIAL_BACKOFF_ENV = "AION_RECONNECT_INITIAL_BACKOFF_SECONDS"
_MAX_BACKOFF_ENV = "AION_RECONNECT_MAX_BACKOFF_SECONDS"
_MAX_ATTEMPTS_ENV = "AION_RECONNECT_MAX_ATTEMPTS"
_TOKEN_ENV = "AION_TOKEN"


class ScriptLoadError(RuntimeError):
    """The activity script could not be loaded, or registered no activities."""


class ConfigurationError(RuntimeError):
    """A required connection setting has no source, or has an unusable value."""


@dataclass(frozen=True)
class _Missing:
    """One setting with no value, named by both the ways it could be supplied."""

    flag: str
    variable: str


def main(argv: list[str] | None = None) -> int:
    """Load an activity script and serve it. Returns a process exit code."""

    parser = _build_parser()
    arguments = parser.parse_args(argv)
    logging.basicConfig(
        level=getattr(logging, arguments.log_level.upper()),
        format="%(asctime)s %(levelname)s %(name)s %(message)s",
    )
    try:
        registry = load_activity_script(Path(arguments.script))
    except ScriptLoadError as error:
        print(f"aion-worker: {error}", file=sys.stderr)
        return 2

    if arguments.check:
        try:
            _print_action_surface(registry)
        except ActivityRegistrationError as error:
            print(f"aion-worker: {error}", file=sys.stderr)
            return 2
        return 0

    try:
        config = build_worker_config(arguments)
    except ConfigurationError as error:
        print(f"aion-worker: {error}", file=sys.stderr)
        return 2

    try:
        worker = Worker(config, registry=registry)
    except (ActivityRegistrationError, ValueError) as error:
        # Descriptor derivation happens here: a handler whose return type proves
        # nothing is refused BEFORE the process dials, naming the activity.
        print(f"aion-worker: {error}", file=sys.stderr)
        return 2
    logger.info(
        "Serving %s on task queue %r in namespaces %s as %r",
        ", ".join(worker.activity_types()),
        config.task_queue,
        ",".join(config.namespaces),
        config.identity,
    )
    return asyncio.run(_serve(worker))


async def _serve(worker: Worker) -> int:
    """Run the worker until it finishes or a termination signal drains it."""

    shutdown = asyncio.Event()
    loop = asyncio.get_running_loop()
    installed = []
    for name in ("SIGINT", "SIGTERM"):
        received = getattr(signal, name, None)
        if received is None:
            continue
        try:
            loop.add_signal_handler(received, _request_shutdown, shutdown, name)
        except (NotImplementedError, RuntimeError):
            # Platforms without signal handlers on the event loop (Windows
            # proactor) simply do not get graceful drain; the worker still
            # exits on the interpreter's own KeyboardInterrupt.
            logger.debug("signal %s cannot be handled on this event loop", name)
            continue
        installed.append(received)
    try:
        await worker.run(shutdown=shutdown)
    except Exception as error:  # noqa: BLE001 - the process boundary reports, it does not swallow
        logger.error("worker stopped: %s", error, exc_info=True)
        return 1
    finally:
        for received in installed:
            with contextlib.suppress(NotImplementedError, RuntimeError):
                loop.remove_signal_handler(received)
    return 0


def _request_shutdown(shutdown: asyncio.Event, name: str) -> None:
    logger.info("%s received; draining in-flight activities before exit", name)
    shutdown.set()


def _print_action_surface(registry: ActivityRegistry) -> None:
    """Print what this script would advertise, without connecting to anything.

    The descriptors are the whole admission decision on a contract-bearing
    queue, so being able to see them before dialling turns a remote
    `FailedPrecondition` into something checkable at the desk.
    """

    surface = [
        {
            "name": descriptor.name,
            "input_schema": json.loads(descriptor.input_schema_json),
            "output_schema": json.loads(descriptor.output_schema_json),
        }
        for descriptor in registry.activity_descriptors()
    ]
    print(json.dumps(surface, indent=2))


# --------------------------------------------------------------------------- #
# configuration
# --------------------------------------------------------------------------- #


def build_worker_config(arguments: argparse.Namespace) -> WorkerConfig:
    """Assemble a :class:`WorkerConfig` from flags and environment.

    Flags win over the environment. Nothing is defaulted that the product does
    not already default: `namespaces`, `subject`, and `node` fall through to the
    library's own documented values, and every remaining setting must be
    supplied. All missing settings are reported together — a worker started
    from a shell should need one correction, not seven.

    Raises:
        ConfigurationError: naming every setting that has no source.
    """

    missing: list[_Missing] = []
    endpoint = _resolve(arguments.endpoint, _ENDPOINT_ENV, "--endpoint", missing)
    task_queue = _resolve(arguments.task_queue, _TASK_QUEUE_ENV, "--task-queue", missing)
    identity = _resolve(arguments.identity, _IDENTITY_ENV, "--identity", missing)
    concurrency = _resolve(arguments.concurrency, _CONCURRENCY_ENV, "--concurrency", missing)
    initial_backoff = _resolve(
        arguments.reconnect_initial_backoff,
        _INITIAL_BACKOFF_ENV,
        "--reconnect-initial-backoff",
        missing,
    )
    max_backoff = _resolve(arguments.reconnect_max_backoff, _MAX_BACKOFF_ENV, "--reconnect-max-backoff", missing)
    max_attempts = _resolve(arguments.reconnect_max_attempts, _MAX_ATTEMPTS_ENV, "--reconnect-max-attempts", missing)
    if missing:
        listed = "\n".join(f"  {item.flag} or ${item.variable}" for item in _sorted_missing(missing))
        raise ConfigurationError(
            f"these connection settings have no value; supply each as a flag or an environment variable:\n{listed}"
        )

    reconnect = ReconnectConfig(
        initial_backoff_seconds=_positive_float(initial_backoff, "--reconnect-initial-backoff", _INITIAL_BACKOFF_ENV),
        max_backoff_seconds=_positive_float(max_backoff, "--reconnect-max-backoff", _MAX_BACKOFF_ENV),
        max_attempts=_positive_int(max_attempts, "--reconnect-max-attempts", _MAX_ATTEMPTS_ENV),
    )
    # `namespaces`, `subject`, and `node` are the three settings with documented
    # library defaults, so an unsupplied one falls through to the dataclass
    # rather than being decided here.
    namespaces = tuple(arguments.namespace) if arguments.namespace else (DEFAULT_WORKER_NAMESPACE,)
    try:
        return WorkerConfig(
            endpoint=str(endpoint),
            task_queue=str(task_queue),
            identity=str(identity),
            max_concurrency=_positive_int(concurrency, "--concurrency", _CONCURRENCY_ENV),
            reconnect=reconnect,
            transport_credentials=_transport_credentials(arguments.token),
            namespaces=namespaces,
            node=arguments.node if arguments.node is not None else default_node(),
            subject=arguments.subject,
        )
    except ValueError as error:
        raise ConfigurationError(str(error)) from error


def _sorted_missing(missing: list[_Missing]) -> list[_Missing]:
    """Order missing settings by flag so the report is stable across runs."""

    return sorted(missing, key=lambda item: item.flag)


def _transport_credentials(token: str | None) -> TransportCredentials | None:
    """Build bearer metadata, matching how the Rust client sends a token.

    The flag overrides `AION_TOKEN`; when both are absent no `authorization`
    metadata is sent at all and the server's development header paths apply —
    the same rule `aion worker` documents.
    """

    resolved = token if token is not None else os.environ.get(_TOKEN_ENV)
    if not resolved:
        return None
    return TransportCredentials(metadata=(("authorization", f"Bearer {resolved}"),))


def _resolve(flag_value: str | None, variable: str, flag: str, missing: list[_Missing]) -> str | None:
    if flag_value is not None:
        return flag_value
    value = os.environ.get(variable)
    if value is None or not value.strip():
        missing.append(_Missing(flag=flag, variable=variable))
        return None
    return value


def _positive_int(raw: str | None, flag: str, variable: str) -> int:
    if raw is None:
        raise ConfigurationError(f"{flag} / ${variable} has no value")
    try:
        value = int(raw)
    except ValueError:
        raise ConfigurationError(f"{flag} / ${variable} must be a whole number, got {raw!r}") from None
    if value <= 0:
        raise ConfigurationError(f"{flag} / ${variable} must be greater than zero, got {value}")
    return value


def _positive_float(raw: str | None, flag: str, variable: str) -> float:
    if raw is None:
        raise ConfigurationError(f"{flag} / ${variable} has no value")
    try:
        value = float(raw)
    except ValueError:
        raise ConfigurationError(f"{flag} / ${variable} must be a number, got {raw!r}") from None
    if value <= 0:
        raise ConfigurationError(f"{flag} / ${variable} must be greater than zero, got {value}")
    return value


# --------------------------------------------------------------------------- #
# script loading
# --------------------------------------------------------------------------- #


def load_activity_script(script: Path) -> ActivityRegistry:
    """Import an activity script and return the registry it populated.

    Loading arbitrary code by path has three sharp edges, each handled
    deliberately rather than hopefully:

    * **Module-name shadowing.** A script called `struct.py` or `json.py` must
      not become the `struct` or `json` every later import resolves to. It is
      loaded under a private, prefixed module name that no `import` statement
      can spell, so its filename is irrelevant to the import system.
    * **`sys.path` pollution.** The script's directory is APPENDED, never
      prepended, so a sibling module stays importable while a sibling named
      after a standard-library module can never win over the real one.
    * **Import side effects.** Executing the file IS the registration
      mechanism, so module-level code runs. `__name__` is not `"__main__"`,
      which means a script's own `if __name__ == "__main__":` block does NOT
      fire — the host serves the handlers, it does not run the script's
      standalone entry point.

    Raises:
        ScriptLoadError: the file is missing, failed to import, or registered
            no activities.
    """

    resolved = script.expanduser().resolve()
    if not resolved.exists():
        raise ScriptLoadError(f"no such file: {resolved}")
    if not resolved.is_file():
        raise ScriptLoadError(f"not a file: {resolved}")

    before = set(default_registry.activity_types())
    module_name = _private_module_name(resolved)
    directory = str(resolved.parent)
    if directory not in sys.path:
        sys.path.append(directory)

    _import_module_from_path(module_name, resolved)

    after = set(default_registry.activity_types())
    if not after - before:
        raise ScriptLoadError(
            f"{resolved} registered no activities. Decorate a handler with "
            "`@activity(name=...)` from aion_worker — the decorator is what makes a "
            "function servable."
        )
    return default_registry


def _import_module_from_path(module_name: str, path: Path) -> ModuleType:
    specification = importlib.util.spec_from_file_location(module_name, path)
    if specification is None or specification.loader is None:
        raise ScriptLoadError(f"{path} is not an importable Python module")
    module = importlib.util.module_from_spec(specification)
    # Registered BEFORE execution: dataclasses and `typing.get_type_hints`
    # resolve a module's own annotations through `sys.modules`, and with
    # `from __future__ import annotations` — which the handler scripts use —
    # every annotation is a string that must resolve later.
    sys.modules[module_name] = module
    try:
        specification.loader.exec_module(module)
    except Exception as error:
        # A partially executed module left in sys.modules would let a retry see
        # half-initialised state.
        sys.modules.pop(module_name, None)
        raise ScriptLoadError(f"{path} raised while importing: {error!r}") from error
    return module


def _private_module_name(path: Path) -> str:
    """Build a module name no `import` statement can produce.

    The leading underscore and prefix keep it out of every namespace a user
    could reach for, which is what makes a script named after a standard-library
    module harmless.
    """

    stem = re.sub(r"\W", "_", path.stem)
    return f"_aion_worker_script_{stem}"


# --------------------------------------------------------------------------- #
# argument parsing
# --------------------------------------------------------------------------- #


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="aion-worker",
        description="Serve the @activity-decorated handlers in a Python script as an Aion worker.",
        epilog=(
            "Every connection setting may be given as a flag or as the named environment "
            "variable; the flag wins. Nothing is defaulted that you have not been told about."
        ),
    )
    parser.add_argument("script", help="Python file containing @activity-decorated handlers")
    parser.add_argument(
        "--endpoint",
        help=f"gRPC server target, for example 127.0.0.1:50051 (env {_ENDPOINT_ENV})",
    )
    parser.add_argument(
        "--task-queue",
        help=(
            "pool/flavour selector within each namespace; for an AWL workflow this is the "
            f"`worker` block's name (env {_TASK_QUEUE_ENV})"
        ),
    )
    parser.add_argument(
        "--identity",
        help=f"stable worker-build identity used to correlate compatibility decisions (env {_IDENTITY_ENV})",
    )
    parser.add_argument(
        "--concurrency",
        help=f"maximum activities served at once (env {_CONCURRENCY_ENV})",
    )
    parser.add_argument(
        "--reconnect-initial-backoff",
        help=f"seconds before the first reconnect attempt (env {_INITIAL_BACKOFF_ENV})",
    )
    parser.add_argument(
        "--reconnect-max-backoff",
        help=f"reconnect backoff cap in seconds (env {_MAX_BACKOFF_ENV})",
    )
    parser.add_argument(
        "--reconnect-max-attempts",
        help=f"cumulative session-drop budget before the run fails (env {_MAX_ATTEMPTS_ENV})",
    )
    parser.add_argument(
        "--namespace",
        action="append",
        help=(
            "correctness/isolation boundary to register into. Repeat to register into several; "
            "every one must be authorized or the whole registration is denied. "
            "Defaults to the SDK's `default`."
        ),
    )
    parser.add_argument(
        "--node",
        help="locality affinity advertised at registration (defaults to this machine's hostname)",
    )
    parser.add_argument(
        "--subject",
        default=DEFAULT_WORKER_SUBJECT,
        help="caller subject metadata sent to the server",
    )
    parser.add_argument(
        "--token",
        help=(
            f"bearer token for authenticated servers; overrides ${_TOKEN_ENV}. When both are "
            "absent the development header paths apply."
        ),
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help=("load the script, print the typed action surface it would advertise, and exit without connecting"),
    )
    parser.add_argument(
        "--log-level",
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
        help="logging verbosity",
    )
    return parser
