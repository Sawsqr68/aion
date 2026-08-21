# Ruling: required-with-no-defaults stands; the sweep was the failure

**Date:** 2026-08-21. **Author:** Vesper Lynd (delegated by Waffles,
2026-08-21). **Trigger:** the first armed repo battery caught four e2e config
templates refusing to boot (exit 2) because three `[store]` fields were added
as required-with-no-defaults and the templates were never swept; this document
records the ruling on the law itself and the shipped-surface sweep that
followed.

## The question

`store.node_cache_budget`, `store.lock_acquisition_patience_ms`, and
`store.lock_acquisition_retry_cadence_ms` are required for the haematite
backend and have **no defaults** — a config without them stops the server at
startup (exit 2) with a typed message naming the field
(`crates/aion-server/src/config/defaults.rs`,
`STORE_NODE_CACHE_BUDGET_REQUIRED` / `STORE_LOCK_ACQUISITION_REQUIRED`).
When that requirement landed, every shipped config that hand-writes a
`[store]` section silently stopped booting. Is the law wrong, or was the
landing incomplete?

## The ruling

**The law stands.** A cache ceiling measured in bytes is a deployment
decision — a node is not a fixed-size thing, and a cache bounded only by
entry count is unbounded in bytes. Lock patience and retry cadence encode how
a node behaves under contention for the store lock; a silently invented
default would decide failover timing on the operator's behalf. Both are
exactly the class of value the estate's no-assumed-defaults rule exists for:
the refusal message teaches the operator what they are deciding, and
`"unlimited"` exists so that choosing the unbounded behaviour is visible in
the file and in every diff, instead of being the invisible consequence of
omission.

**The failure was the sweep.** The front door was done right — the server's
first-run scaffold (`crates/aion-server/templates/first-run-config.toml`) and
the `aion new` project template (`crates/aion-cli/templates/shared/aion.toml`)
both carry all three fields with the full teaching comments, and their
identity is pinned by a test. What was never swept is every place that
hand-writes its own `[store]` section instead of using the templates. One
truth in N places had drifted; each copy is a boot that dies at the door.

## The census (this branch closes every row)

| Surface | State found | Fields missing |
|---|---|---|
| `crates/aion-server/templates/first-run-config.toml` | correct | none |
| `crates/aion-cli/templates/shared/aion.toml` | correct | none |
| `dev-config.toml` (repo root) | **broken** | all three |
| `examples/agent-dev/demo-config.toml` | **broken** | all three |
| `examples/incident-triage/demo-config.toml` | **broken** | all three |
| `docs/authoring/tutorial-examples/aion.toml` (+ generated `first-workflow.html`) | **broken** | all three |
| `docs/GETTING-STARTED.md` (prose + TOML block) | partial | both lock fields |
| `docs/operations/operations.md` reference `[store]` block | **missing** | all three |
| `scripts/demo/lib.sh` (cluster demo heredoc) | **broken** | all three |
| `scripts/mp-failover-spike.sh` (spike heredoc) | **broken** | all three |
| e2e test templates (4 files) | fixed on `fix/skip-family-unproven` | — |

Census method: `git grep -l 'backend = "haematite"'` across the tree minus
`crates/` (the templates inside `crates/` were verified separately), then
each hit read for the three field names. The e2e templates are counted for
completeness; their cure rides the skip-family branch, not this one.

## Witness

The refusal is not theoretical. The shipped tutorial config, run against the
installed `aion 0.18.1` binary (2026-08-21):

```
exit=2
configuration error: store.node_cache_budget is required and has no default: …
```

A reader following `docs/GETTING-STARTED.md` or the first-workflow tutorial
verbatim could not boot the server they had just been taught to configure.
The cure witness (patched config boots to `/health`) requires a binary built
from a commit carrying the lock fields in `StoreConfig`; the installed
release binary predates them and refuses the field names as unknown — that
witness is recorded in the branch's landing note, run against the tree's own
binary.

## Why the demos never caught it

None of the broken surfaces is executed by any gate: the tutorial deck's
build gates check AWL documents and CLI verbs but never boot a server; the
example RUNBOOKs are operator-driven; the demo scripts are run by hand. A
skip that announces nothing looks exactly like a pass — the same disease the
armed battery was built to cure, wearing config instead of tests.

## The standing rule

A change that adds a required field to the server config MUST sweep every
hand-written `[store]` (or other affected section) in the tree — templates,
examples, docs prose, generated docs sources, and scripts — in the same
change. The census command above is the sweep's floor, not its ceiling.
