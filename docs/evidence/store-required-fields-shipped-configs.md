# Ruling: required-with-no-defaults stands; the sweep was the failure

> **Addendum at landing, 2026-08-24.** Between this record's census and its
> landing, `27bfba8a9` ("fix: retire the writer-lock patience keys — boot
> waits, reporting, indefinitely") retired `store.lock_acquisition_patience_ms`
> and `store.lock_acquisition_retry_cadence_ms`: the keys are now parsed and
> **ignored**, and boot waits for the writer lock indefinitely while reporting
> that it is waiting. The required-no-default set at landing is therefore
> THREE fields, and it is not one uniform set: `observability.max_batch_events`
> and `observability.max_batch_hold_ms` are required on **every** backend (the
> transcript channel is always mounted), while `store.node_cache_budget` is
> required exactly where boot enforces it — the haematite connect seam, the
> only backend with a node cache. That conditionality is load-bearing for the
> census: the landing review caught two memory-backend surfaces this record's
> haematite-shaped pattern was structurally blind to (`ci/smoke-embed.sh` and
> the conformance README's config block, both refusing on the observability
> bounds), and the landing sweeps them too. The census, witness, and ruling
> below are the record of 2026-08-21 and are left as written; the paragraph
> of the ruling that defends the lock fields is superseded by the retirement,
> and the standing rule at the bottom is restated in the same terms as this
> paragraph.

> **Addendum, same day (the witness caught the ruling's own author).** The
> first cut of this branch swept the three `[store]` fields — and the boot
> witness then died on `observability.max_batch_events`, a fourth required
> field this document's census never looked for. The census below was
> originally scoped to the `[store]` section: it swept the CONTAINER the
> defect was found in, not the CLAIM ("every required-no-default field
> reaches every hand-written config"). The corrected census is driven by the
> code — every `is required and has no default` message in
> `crates/aion-server/src/config/defaults.rs` — and the unconditional set is
> FIVE fields: the three `[store]` fields plus `observability.
> max_batch_events` and `observability.max_batch_hold_ms`. (Two more are
> conditionally required — `outbox.reconcile_*` when reconciliation is
> enabled, `authoring.project_root` when `authoring.gleam_path` is set — and
> no shipped config sets either enabler.) The standing rule at the bottom
> gains its corollary: the sweep's floor is the CODE's required set, never
> the section the last failure happened to be in.

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
change. The sweep's population is **every surface that boots a server**,
never the surfaces matching one backend's shape: a requirement that fires on
every backend (the observability batch bounds) is invisible to a
backend-shaped grep, which is exactly how the two memory-backend surfaces in
the landing addendum were missed. The mechanized floor is
`crates/aion-server/src/config/shipped_configs_tests.rs`, which holds the
shipped TOMLs to the boot path's own requirement functions; the manual sweep
above it covers what a test cannot parse.
