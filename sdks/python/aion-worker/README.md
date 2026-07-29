# aion-worker

Python remote-worker SDK for registering out-of-process Aion activities and serving them from an Aion server task queue. Status: in progress/hardening; install from this checkout until a release is published for your target environment.

## Install

```sh
python -m pip install -e sdks/python/aion-worker
```

## Serve a script

The fastest path: a file with decorated handlers, and a command pointed at it.
No entry point, no `WorkerConfig` literal, no `asyncio.run`.

```python
# classify.py
from dataclasses import dataclass

from aion_worker import activity


@dataclass
class Request:
    path: str


@dataclass
class Facts:
    width: int
    height: int


@activity(name="images.classify")
def classify(input: Request) -> Facts: ...
```

```sh
aion-worker classify.py \
  --endpoint 127.0.0.1:50051 \
  --task-queue python_box \
  --identity classify-worker-1 \
  --concurrency 4 \
  --reconnect-initial-backoff 0.1 \
  --reconnect-max-backoff 5 \
  --reconnect-max-attempts 10
```

`python -m aion_worker classify.py ...` is the same command; both run
`aion_worker.host:main`.

### Why the type hints matter

Both sides of a handler are annotated, and neither annotation is decoration:

- The **input** hint is what the payload is decoded INTO. An activity's input
  arrives as a JSON object keyed by parameter name — an AWL action
  `classify(path: String)` dispatches `{"path": "..."}` — so a single-parameter
  handler takes one dataclass holding those keys, not a bare `str`. A payload
  that does not fit is a terminal failure naming the field, never a silent
  coercion.
- The **return** hint is what the worker ADVERTISES it produces. The server
  checks that advertisement covariantly against the deployed contract, so a
  handler annotated `-> dict` cannot describe what it emits and is refused by
  any contract-bearing queue. A return type that proves nothing at all
  (`-> Any`) is refused locally, at startup, naming the activity — a message
  you can act on instead of a remote schema diff.

Supported annotations: `@dataclass` types (nested, and the recommended shape
for anything structured), the JSON scalars, `list[T]`, `tuple[T, ...]`,
`dict[str, V]`, `Optional[T]`, unions, `Literal[...]`, and string-valued
enums. A dataclass return is encoded field by field; you do not call
`asdict` yourself.

### Check the action surface without a server

```sh
aion-worker classify.py --check
```

prints the exact `input_schema`/`output_schema` this worker would advertise.
Those descriptors are the whole admission decision on a contract-bearing
queue, so this turns a remote `FailedPrecondition` into something you can read
at your desk.

### Connection settings

Every setting is supplied by a flag or by the named environment variable —
the flag wins, and nothing is invented. A missing setting is reported by name,
with all of them listed at once.

| Flag | Environment variable | Meaning |
| --- | --- | --- |
| `--endpoint` | `AION_WORKER_ENDPOINT` | gRPC target, for example `127.0.0.1:50051` |
| `--task-queue` | `AION_TASK_QUEUE` | pool within each namespace; for AWL this is the `worker` block's name |
| `--identity` | `AION_WORKER_IDENTITY` | stable worker-build identity |
| `--concurrency` | `AION_WORKER_CONCURRENCY` | maximum activities served at once |
| `--reconnect-initial-backoff` | `AION_RECONNECT_INITIAL_BACKOFF_SECONDS` | seconds before the first reconnect |
| `--reconnect-max-backoff` | `AION_RECONNECT_MAX_BACKOFF_SECONDS` | reconnect backoff cap |
| `--reconnect-max-attempts` | `AION_RECONNECT_MAX_ATTEMPTS` | cumulative session-drop budget |
| `--token` | `AION_TOKEN` | bearer token; when both are absent the development header paths apply |

`--namespace` (repeatable), `--subject`, and `--node` have documented library
defaults (`default`, `worker`, and this machine's hostname) and may be omitted.

### Loading arbitrary code by path

The script is imported, so module-level code runs — that is how `@activity`
registers. Three consequences, handled deliberately:

- It is loaded under a private module name no `import` statement can spell, so
  a script called `json.py` never becomes the `json` everything else resolves.
- Its directory is **appended** to `sys.path`, never prepended: a sibling
  helper module stays importable while a sibling named after a standard-library
  module can never win over the real one.
- `__name__` is not `"__main__"`, so a script's own
  `if __name__ == "__main__":` block does not fire. The host serves the
  handlers; it does not run the script's standalone entry point.

## Library worker

When a script is not the right shape — several modules, an existing service, a
custom executor — construct the worker directly. Registration derives and sends
the typed action surface either way.

```python
import asyncio
from dataclasses import dataclass

from aion_worker import ReconnectConfig, Worker, WorkerConfig, activity


@dataclass
class Greeting:
    name: str


@dataclass
class Greeted:
    message: str


@activity(name="examples.greet")
def greet(input: Greeting) -> Greeted:
    return Greeted(message=f"hello, {input.name}")


async def main() -> None:
    config = WorkerConfig(
        endpoint="127.0.0.1:50051",
        task_queue="default",
        identity="python-worker-1",
        max_concurrency=8,
        reconnect=ReconnectConfig(
            initial_backoff_seconds=0.1,
            max_backoff_seconds=5.0,
            max_attempts=10,
        ),
        # namespaces is a SET, and every one must be authorized or the whole
        # registration is denied. It is also what goes in `x-aion-namespaces`.
        namespaces=("default",),
    )

    await Worker(config).run()


asyncio.run(main())
```

`Worker(...)` derives the descriptors at construction, so a handler that cannot
advertise what it returns fails before the process dials.

## Failure classification

- Raise `TerminalError` for something a retry cannot change. Subclassing works
  — `class UnreadableImage(TerminalError)` classifies every raise site at once.
- Raise `RetryableError` to say so explicitly.
- Anything else that escapes a handler is reported **retryable**, so a genuinely
  transient failure (a busy volume, a flaky socket) does not have to be
  classified to be retried correctly.
- A payload that does not fit the handler's input type, and a return value that
  cannot be encoded, are both **terminal**: neither changes on a retry.

## Example

`examples/python-worker/` in the main repository holds a complete single-file
worker (`image_worker.py`) and the AWL workflow it serves
(`image_report.awl`).

See the main Aion repository at <https://github.com/ablative-io/aion>.

## Regenerating the protobuf stubs

The gRPC stubs under `aion_worker/proto/` are generated from the shared wire
contract at `crates/aion-proto-generated/proto/` (the hatch build hook in
`build_proto.py` runs the same generation at package build time). After a
wire-contract change, regenerate and commit the refreshed stubs:

```bash
cd sdks/python/aion-worker
uv run --extra dev --isolated -- python -c "
from pathlib import Path
from build_proto import generate_proto_stubs
generate_proto_stubs(Path('.').resolve())
"
```
