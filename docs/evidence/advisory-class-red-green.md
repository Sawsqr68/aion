# Red-then-green evidence — build/advisory-class

RUNTIME-OPERATIONS.md R5, the advisory action class: "Side-channel actions
(heartbeats, notifications) declare `advisory`: retry exhaustion emits a
visible warning event on the run and surfaces in the console; it never faults
the step." Built on `build/advisory-class` off aion main `28eb1eec`, in the
`.worktrees/advisory-class` worktree, 2026-07-29.

Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`.

## The failure R5 exists to kill

`examples/remediation-packet/awl/remediation_packet.awl` puts five
meridian-heartbeat `run_command` calls on its critical path. A meridian
outage therefore sinks an otherwise healthy packet run — the run fails
because the announcement failed.

## Red — a meridian outage sinks a healthy run

`gate-logs/advisory-red-exhibit.log` (+ `.exit` = 101), committed at
`65973b59` before any fix. Two DIRECT-compiled AWL documents differing in
exactly one token, both run against a real engine with an outage dispatcher
whose `send_heartbeat` always fails retryably and whose `do_work` always
succeeds:

```
test non_advisory_exhaustion_still_fails_the_run ... ok
test advisory_exhaustion_warns_and_the_run_completes ... FAILED
```

The failing test asserted the desired post-build behaviour; it failed because
`advisory` was not yet a config key, so the document did not parse. The
passing test is the control and the regression pin for the untouched
non-advisory posture: the same outage faults the step, the run fails with the
last reason verbatim, the real work is never dispatched.

## Green — the class

`gate-logs/advisory-green-tests.log`, `crates/aion/tests/advisory_class_e2e.rs`:

```
test advisory_exhaustion_warns_and_the_run_completes ... ok
test advisory_exhaustion_replays_from_history_without_divergence ... ok
test non_advisory_exhaustion_still_fails_the_run ... ok
```

The first test is R5 in one run, live against a real engine: the advisory
heartbeat exhausts its declared budget (2 deliveries), records its honest
terminal `ActivityFailed` with the verbatim last reason, records an
`ActivityAdvisoryExhausted` warning on the run, does NOT fault the calling
step, and the real work still runs and produces the outcome payload. The
third is the same outage without the marker: still a failed run, still no
warning, still zero real work — byte-identical to today.

### The four seams

**Language.** `advisory` is a config-line key on an action DECLARATION,
carrying its own span. Lexer keyword tables, the parser's soft-keyword
allowlist (a `retry`-shaped contextual keyword — it must stay usable as an
ordinary name), the config-block lookahead, the config-key loop with its
duplicate refusal, both "expected a config key" strings, and the printer's
fixed-order render, where `advisory` LEADS. Corpus: one valid fixture in the
BC-2 covered ratchet with a committed MIR golden, four invalid fixtures with
sidecars.

**Checker.** Two refusals, both from "an advisory action never faults the
step": its call may not bind a result, and it may not be a pipe stage —
in both positions a value is consumed, and on the failing path there is no
value. Plus, exactly like `retry`, `advisory` is declaration-only: a call
site may pin `node`/`timeout` only.

**Identity.** `ActionContract.advisory` is committed into the `.v4` package
hash, because flipping an action to advisory changes what the package
promises a caller. The canonical record appends a single marker byte after
the retry block ONLY when the flag is true. Three pins are the encoding's
acceptance floor:

- `advisory_free_contracts_keep_their_pre_advisory_v4_identity` — a golden
  hash MEASURED on the pre-advisory tree (`55a07922`, before the field
  existed) and asserted against the post-change encoder. They agree
  byte-for-byte, so "the class mints no redeploy wave" is checked, not
  claimed.
- `advisory_is_identity_bearing` — advisory=true changes bytes and hash.
- `advisory_identity_is_declaration_order_independent` — the same advisory
  action reached through two permutations hashes equal.

The ceiling is written at the encoding site: this consumes
`ActionContract`'s ONE trailing-append slot, because a second optional tail
would not be injective. Any future field must always-encode under a bumped
identity domain.

**Engine.** Two halves, deliberately split:

- The COMPILER routes an advisory call around the fault arm, in BOTH
  lowerings — the direct/MIR path (`mir/lower/activity.rs`: `workflow.run`
  runs, then no `map_activity_error` and no `TryBind`; visible in the
  committed golden diff) and the Gleam emitter (`emitter/stmts.rs`:
  `let _ = … |> workflow.run` instead of `result.try`). A third site the
  brief did not name: `emitter/steps.rs::layer_calls` returns `None` for an
  advisory member, because an advisory call is not a layer-completion
  dependency and must not join a `workflow.all` whose purpose is to gate the
  layer on every member's outcome. `nif_activity_await.rs` is untouched.
- The ENGINE supplies the visibility. At the dispatch seam the declared class
  is read off the package contract THIS RUN IS PINNED TO
  (`nif_activity_advisory::declared_advisory`, keyed by
  `(workflow_type, loaded_version)`), stamped onto `ActivityDispatch`, and
  when the retry loop's attempt budget is spent it records
  `Event::ActivityAdvisoryExhausted` on the run alongside the activity's
  terminal `ActivityFailed`. The failure is never erased — only declawed.

"Retry exhaustion" is read as the attempt budget being SPENT, whatever its
size: an advisory action with no declared retry has a budget of one and its
single failure exhausts it. The warning never fires for a parked dispatch or
a settled-elsewhere stand-down — neither is a failure, and both must record
nothing.

### Why the class is read from the contract, not the dispatch wire

The brief's sketch had advisory ride the dispatch `config` JSON "exactly like
retry/timeout". That JSON is built by the Gleam SDK
(`gleam/aion_flow/src/aion/internal/activity_dispatch.gleam::config`), and
the SDK ships as a **vendored BEAM closure**
(`crates/aion-awl-package/sdk-closure/`, 46 modules embedded by
`include_bytes!`). A no-op run of `scripts/regen-awl-sdk-closure.sh` on this
box rewrote all 24 `aion_flow` beams — the committed closure is not
byte-reproducible here — so any SDK-side field would have dragged a
whole-closure repin, unrelated to and ungated by this lane, into it.

The contract is the better authority anyway: `advisory` is declaration-owned
and committed into `.v4` identity, so the contract is where it already lives.
Reading it there also means one call site cannot forge the class, and
resolution by the run's recorded version pin means a hot redeploy never
retroactively reclasses an in-flight run's activities. Every absence
(no catalog, no such loaded workflow, a pre-`.v4` package, an undeclared
activity) reads as NOT advisory — the loud, pre-R5 posture, never the quiet
one.

### Replay determinism

`advisory_exhaustion_replays_from_history_without_divergence` walks the
recorded history through `durability::replay_inspect::inspect_run` and pins
two things: the run replays from history alone with no divergence, and the
advisory ordinal still resolves to its recorded terminal `ActivityFailed`.

That pin is what the ORDERING choice needs. The warning is recorded by the
dispatch-side retry loop just before the failure is delivered, so it lands
BEFORE the terminal the workflow thread records on receipt. Recorded after,
it would be the last event of the ordinal's matched range and replay would
read the outcome off a warning instead of off the failure.

### Console

`Event::ActivityAdvisoryExhausted` is surfaced end to end: every
compile-forced match got a deliberate arm (status projection returns `None`
— the warning says a side channel failed, not that the run did; the stream
selector projects `Running`; the store's `event_kind`, the durability
cursor, `replay_inspect` and the resolver name it; `continue_as_new`'s
pending-work scan ignores it, because the activity's own terminal is what
clears it). ts-rs regenerated; the console's closed variant set, family
classification (`activity`, existing `EventIcon` tone), and activity-lane
projection carry it, with an `advisory-exhausted` lane status distinct from
`failed` and a summary that says the step was not faulted.

## NOT built, and why — R5's `on failure` requirement

R5's second sentence: "Non-advisory actions require an `on failure` edge
wherever a declared retry can exhaust (Bundle A §2.3)." That checker rule was
built, measured, and BACKED OUT. It is unlandable today, and this is the
finding, not an omission.

The direct (MIR) backend — the production compile path, `aion_awl::compile`
via `compile_and_assemble_awl` — REFUSES `on failure` outright:

```rust
// crates/aion-awl/src/mir/lower/flow.rs:299
if step.on_failure.is_some() {
    return Err(LowerError::unsupported("on failure", step.name_span));
}
```

So the rule demands, on exactly the steps that carry a retry budget, a
construct the production compiler cannot lower. Measured with the rule live,
over every `.awl` in the tree via `aion awl check`: **17 files refused**,
including the live `examples/remediation-packet/*.awl` workflows and the
flagship `dev_brief`. Six of the refused fixtures are BC-2 covered-ratchet
members (`declarations_combined`, `spawn_detached`, `timeout_inside_retry`,
`worker_action_config_lines`, `workers_multiple`,
`loop_counting_until_max`) — adding `on failure` to them would have regressed
them covered → deferred, degrading the ratchet to satisfy a checker rule.

The rule needs `on failure` lowering in the direct backend first. That is its
own lane. Note also that the rule as scouted only reaches CALL statements;
`worker_retry_backoff.awl` shows a retrying action used as a PIPE stage, so
the eventual rule must cover both positions.

## Out of scope — flagged, not fixed

- **tree-sitter-awl grammar** (cross-repo): `advisory` is not in the editor
  grammar. Follow-up owed in that repo.
- **R4 retry-count off-by-one**: unchanged and untouched. `retry N` still
  means N TOTAL attempts in the engine (the exhibit's `retry 2` delivers
  twice) while Bundle A and `RetryContract`'s doc say N FURTHER attempts.
- **Advisory failures that arrive by another path.** The warning is recorded
  at the retry loop's terminal, which is the exhaustion locus. A terminal
  `ActivityFailed` recorded by the workflow thread's `with_timeout` scope
  expiry (`nif_activity_await.rs:125`) does not emit it. Faithful to "retry
  exhaustion"; worth a ruling if the class should also warn on scope expiry.
- **Console runtime verification**: `apps/aion-ops-console` has no
  `node_modules` in this worktree, so the full `bun test` / `tsc --noEmit`
  suites cannot run here. The timeline and variant suites — the ones that own
  the closed event set — were run directly and pass (19/19); the rest fail on
  module resolution, unrelated to this change.

## Battery

At the branch tip, in the lane worktree, logs and exit codes in `gate-logs/`
(`advisory-green-*`).

| Leg | Command | Exit |
| --- | --- | --- |
| preflight | `df -h .` | 0 |
| fmt | `cargo fmt --all --check` | 0 |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| tests | `cargo test --workspace --no-fail-fast` | see `advisory-green-tests.exit` |
