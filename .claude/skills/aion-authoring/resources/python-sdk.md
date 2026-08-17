# Python worker SDK (`sdks/python/aion-worker`)

Install from the checkout: `python -m pip install -e sdks/python/aion-worker`.
A complete worked example lives at `examples/python-worker/`
(`image_worker.py` + the `image_report.awl` document it serves).

## The fast path: a script of decorated handlers

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
  --endpoint 127.0.0.1:50051 --task-queue python_box \
  --identity classify-worker-1 --concurrency 4 \
  --reconnect-initial-backoff 0.1 --reconnect-max-backoff 5 \
  --reconnect-max-attempts 10
```

(`python -m aion_worker classify.py …` is the same command.)

## The type hints are the contract

- The **input** hint is what the payload decodes into. An activity's input
  arrives as a JSON object keyed by parameter name — an AWL action
  `classify(path: String)` dispatches `{"path": "…"}` — so a handler takes
  one dataclass holding those keys, not a bare `str`. A payload that does
  not fit is a **terminal** failure naming the field, never a coercion.
- The **return** hint is what the worker advertises. The server checks it
  against the deployed contract at registration; `-> dict` or `-> Any` is
  refused (the latter locally, at startup, naming the activity).

Supported: nested `@dataclass` types (the recommended shape), JSON scalars,
`list[T]`, `tuple[T, ...]`, `dict[str, V]`, `Optional[T]`, unions,
`Literal[...]`, string-valued enums. Dataclass returns are encoded for you.

**Check the surface without a server**: `aion-worker classify.py --check`
prints the exact `input_schema`/`output_schema` the worker would advertise —
the whole admission decision, readable at your desk instead of as a remote
`FailedPrecondition`.

## Connection settings

Every setting comes from a flag or its named environment variable (flag
wins); nothing is invented, and a missing setting is reported by name with
all of them listed at once: `--endpoint`/`AION_WORKER_ENDPOINT`,
`--task-queue`/`AION_TASK_QUEUE` (for AWL this is the `worker` block's
name), `--identity`, `--concurrency`, the three `--reconnect-*` values, and
`--token`/`AION_TOKEN`. `--namespace`, `--subject`, `--node` have documented
library defaults (`default`, `worker`, hostname).

## Failure classification

- Raise `TerminalError` for what a retry cannot change (subclassing
  classifies every raise site at once).
- Raise `RetryableError` to say so explicitly.
- Any other escaping exception is reported **retryable** — a transient
  failure does not have to be classified to be retried correctly.
- An input that does not fit and a return that cannot be encoded are both
  **terminal**.

## Library form

For multi-module services, construct `WorkerConfig` +
`ReconnectConfig` and `await Worker(config).run()` — descriptors are derived
at construction, so a handler that cannot advertise its return fails before
the process dials. `namespaces` is a set and every one must be authorized or
the whole registration is denied.

## Caller side

`sdks/python/aion-client` is the caller SDK (start/signal/query/await runs
from Python). Its README is the reference; the worker and client are
separate installs.
