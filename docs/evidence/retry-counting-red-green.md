# Red-then-green evidence — build/retry-counting

Lane: R4, retry-count enforcement. `retry N` in AWL declares **N further
attempts after the first** — N+1 total executions. The engine did N total.

Toolchain stamped on every gate log and on this file:
`rustc 1.97.1 (8bab26f4f 2026-07-14)`, `cargo 1.97.1 (c980f4866 2026-06-30)`,
`gleam 1.17.0` (Erlang/OTP 29 box). No `rust-toolchain.toml` edit.

## The contract of record

- `docs/design/aion-authoring/awl/AWL-COMPLETION-COLD-READ.md` §2.3: "The
  reference defines it as 'N further attempts after the first' and the grammar
  keeps the word `retry` honest to that."
- `crates/aion-package/src/contract.rs` — `RetryContract::Every::count` and
  `RetryContract::Backoff::count` are both documented "Number of further
  attempts after the first". The engine contradicted its own contract's
  documentation.

`docs/design/aion-authoring/awl/AWL-2-SPEC.md` now states the rule in the
worker-declaration section (it previously said nothing), and the open finding
in `AWL-2-BUILD-PLAN.md` is annotated RESOLVED for the counting half only —
the invented `2.0` backoff multiplier named in the same finding is untouched
and stays open.

## Where the meaning lives

The engine's dispatch loop is convention-free and stayed **byte-identical**:

```
crates/aion/src/runtime/nif_activity_retry_dispatch.rs:138
    Some(policy) if is_retryable_reason(reason) && attempt < policy.max_attempts => None,
```

The convention is owned by the two places a declared count becomes the SDK's
total-attempt budget — one per lowering. Both were fixed, and only those:

| lowering | site | change |
| --- | --- | --- |
| reference (emitted Gleam) | `crates/aion-awl/src/emitter/stmts.rs` — `retry_policy` | `max_attempts: {count}` → `max_attempts: {total_attempts(count)}` |
| direct (MIR → `.beam`) | `crates/aion-awl/src/mir/lower/activity.rs` — `build_retry_policy` | `Value::Int(count)` → `Value::Int(total_attempts(count))` |

`total_attempts(count) = count.saturating_add(1)`, present in both files.
Saturating rather than wrapping: a wrapped budget of `0` is decoded by
`retry_policy_from_config` as "no policy at all" (`nif_activity_retry.rs:195`),
which would silently delete an author's retries. On the direct side the
saturated value still fails the existing `i64::try_from` guard, so an absurd
count refuses at lowering instead of emitting an unrepresentable budget.

## Red

Committed as `03a838aa`, before any source change.

`gate-logs/retry-counting-red-e2e.{log,exit}` — exit **101**:

```
running 1 test
test retry_three_runs_the_action_four_times_on_both_backends ... FAILED

thread '…' panicked at crates/aion/tests/awl_retry_counting_e2e.rs:262:5:
assertion `left == right` failed: `retry 3` must buy 3 FURTHER attempts after
the first — 4 executions on the reference backend
  left: 3
 right: 4
```

`gate-logs/retry-counting-red-awl.{log,exit}` — exit **101**, three of four
pins red at the mapping sites themselves:

```
reference_emitter_maps_three_further_attempts_to_four_total
  left: "activity.RetryPolicy(max_attempts: 3"
 right: "activity.RetryPolicy(max_attempts: 4"

direct_lowering_maps_three_further_attempts_to_four_total
  `retry 3 every 1s` must lower to a four-attempt budget:
  v4 = record(retry_policy, [3, v3])

zero_further_attempts_is_one_total_execution
  left: "activity.RetryPolicy(max_attempts: 0"
 right: "activity.RetryPolicy(max_attempts: 1"

declared_count_is_the_identity_and_does_not_move ... ok   ← green by design
```

The e2e red exhibit (`crates/aion/tests/awl_retry_counting_e2e.rs`) is a real
engine run, not a unit assertion: one `retry 3 every 1s` AWL workflow with an
always-retryably-failing action, packaged and started on an embedded
`EngineBuilder`, counting dispatcher deliveries. It runs **both** productions —
the emitted-Gleam reference package, and the MIR `select()` bytes spliced in as
the entry module over that same package's SDK closure (the BC-4 splice
discipline, with the no-op-splice and lost-entry preconditions asserted). A
divergence can therefore only be a backend difference.

## Green

`retry 3` now runs the action four times on both backends; `retry 0` is one
execution and no retry; both retry dialects (`every D`, `backoff D..D`) agree.

## `retry 0`

The grammar admits it (`parser/workers.rs:296` takes any `Integer`; no checker
rule refuses zero), so it was pinned rather than invented away: `retry 0` →
`max_attempts: 1` → exactly one execution, no retry. Observable behaviour is
unchanged from before the fix — the old mapping produced `max_attempts: 0`,
which `decode_policy` refuses (`nif_activity_retry.rs:193-197`) and the engine
treats as an absent policy, also one execution. What changed is that the
declaration is now *stated* rather than reached by an accidental refusal path.

## Contract identity — unchanged, pinned

`crates/aion-awl/tests/retry_counting.rs::declared_count_is_the_identity_and_does_not_move`
compiles the `retry 3 every 1s` document and compares
`PackageContract::canonical_bytes()` against the committed golden
`crates/aion-awl/tests/contract-goldens/retry_three_every_1s.hex`, captured
**before** the mapping change (it is in the red commit and passed there) and
byte-identical after it.

That is the correct outcome and not an accident of the encoding:
`compile.rs::retry_contract` copies the declared `count` verbatim into
`RetryContract`, and `ActionContract::canonical_bytes` (`contract.rs:199-221`)
encodes that same `count`. The number the author wrote is the number identity
commits to; only its runtime translation moved.

The *package* content hash does change, because the entry `.beam` bytes change
— `content_hash_with_contract` (`hash.rs:139-150`) digests beams alongside the
contract. That is the honest reading: the code changed, so the artifact is a
new artifact.

## Versioned semantics — answered at the bytes

**Already-deployed packages keep the old count. Nothing re-derives dispatch
config at run time.** Verified along the whole chain, not assumed:

1. The number is compiled INTO the workflow's `.beam`. Both lowerings emit it
   as a literal in the entry module (`activity.RetryPolicy(max_attempts: K, …)`
   in emitted Gleam; `record(retry_policy, [K, …])` in the MIR the direct
   backend selects to bytes).
2. At dispatch time the **BEAM** builds the config JSON from that in-beam
   value: `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:199-228`
   (`config/2`) calls `retry_config` at `:242-252`, which writes
   `#("max_attempts", json.int(attempts))` straight from the record.
3. Rust receives that string as a NIF argument and never reconstructs it:
   `crates/aion/src/runtime/nif_activity_dispatch.rs:151` decodes `config` from
   `args[2]`, carries it through `ActivityCall::config` (`:180`) onto
   `ActivityDispatch::config` (`:269`), which
   `crates/aion/src/activity/bridge.rs:53-54` documents as "the JSON strings
   the Gleam SDK sends through the NIF".
4. `retry_policy_from_config` (`nif_activity_retry.rs:176`) is the only reader,
   and it parses that same string.
5. Nothing outside `aion-package`/`aion-awl` ever reads `RetryContract`: a
   tree-wide grep finds it only in `aion-package/src/{lib,contract,
   contract_tests}.rs` and `aion-awl/src/compile.rs`. Neither `aion` nor
   `aion-server` derives dispatch config from the contract or the manifest.

So this is **version-pinned behaviour: the package you deployed is the package
you run.** A deployed `retry 3` package keeps 3 total executions until it is
rebuilt from source; rebuilding it yields 4 and a new package identity (the
beams changed). There is no run-time re-derivation seam that would flip old
packages, and none was added.

Operator consequence worth stating plainly: after this lands, a `retry N`
workflow silently changes attempt count *at its next deploy*, not at upgrade.
That is the right boundary but it is a real behaviour change at redeploy, and
it belongs in the release note for the version that carries it.

## Old-meaning siblings flipped in the fix commit

Every artifact that encoded `retry N` → N total. All are MIR goldens
(`crates/aion-awl/tests/mir-goldens/…`), regenerated with `AWL_BC2_BLESS=1`;
each diff is exactly one `record(retry_policy, [N, …])` → `[N+1, …]`:

| golden | declared in the fixture | budget `[N → N+1]` |
| --- | --- | --- |
| `declarations/valid/declarations_combined.mir` | `retry 2`, `retry 4`, `retry 3` | `[2 → 3]`, `[4 → 5]`, `[3 → 4]` |
| `declarations/valid/spawn_detached.mir` | `retry 2` | `[2 → 3]` |
| `declarations/valid/timeout_inside_retry.mir` | `retry 4` | `[4 → 5]` |
| `declarations/valid/worker_action_config_lines.mir` | `retry 3` | `[3 → 4]` |
| `declarations/valid/worker_retry_backoff.mir` | `retry 5` | `[5 → 6]` |
| `declarations/valid/workers_multiple.mir` | `retry 2` | `[2 → 3]` |
| `header-types/valid/combined.mir` | `retry 2` | `[2 → 3]` |
| `loop-outcomes/valid/loop_counting_until_max.mir` | `retry 3` | `[3 → 4]` |
| `schema-doors/valid/mixed_doors.mir` | `retry 2` | `[2 → 3]` |

Eleven sites across nine files. The paired `.gleam_types.hex` sidecars did not
move (retry is not in the type surface).

**No test assertion anywhere in the workspace encoded the old meaning.** Searched
and cleared, with the reason each is untouched:

- `crates/aion/tests/activity_retry_e2e.rs` — named in the brief as a likely
  sibling; it is **not** one. It exercises `examples/retry-policy`, a
  hand-written Gleam SDK workflow that declares
  `activity.RetryPolicy(max_attempts: 3)` directly
  (`examples/retry-policy/src/retry_policy.gleam:52,65`). That is the SDK's
  total-attempt surface, whose meaning this lane does not touch. Its "3
  deliveries then exhaust" pins remain correct.
- `crates/aion/src/runtime/nif_activity_dispatch_tests/retry_outcomes.rs` and
  `retry_support.rs` — drive the dispatch loop from hand-written config JSON
  (`{"retry":{"max_attempts":3,…}}`), i.e. the SDK surface again. Untouched.
- `crates/aion/src/runtime/nif_activity_retry.rs` decode tests — same.
- `crates/aion-awl/tests/operators_and_literals.rs:257` — lexes
  `retry 2 every 30s`; asserts tokens, not counts.
- `crates/aion-awl/tests/runtime_codecs/fork_generality.rs:158` — asserts the
  merged config *contains* `max_attempts`, not its value.
- `crates/aion-worker`, `crates/aion-server` `max_attempts` hits are all
  `reconnect.max_attempts` / `outbox.max_attempts` — unrelated surfaces.

## Knowingly stale, deliberately not rewritten

`docs/design/aion-authoring/awl/exam/playtest/emitted.gleam` and
`docs/design/aion-authoring/awl/exam/workbench/sitting.gleam` are dated
transcripts of emitter output captured during the AWL exam. They now differ
from what the emitter produces. No test reads them; they are historical
records of a session, and editing them would falsify the record.

`docs/design/fanout-observability/awl-fork-generality-spec.md:54` cites
`src/emitter/stmts.rs:141-156`, a line range this lane shifted by a few lines.
Left alone (design-doc line citations drift constantly and chasing them is not
this lane's job) — flagged here so nobody reads it as a live anchor.

## Owed to the landing owner — R5 collision

The R5 advisory lane (`build/advisory-class`, at review) touches
`crates/aion-awl/src/emitter/stmts.rs` and its exhibit declares
`advisory, retry 2 every 1s`, asserting heartbeat delivery counts under the OLD
meaning. When both lanes land, that exhibit's expected deliveries move **2 → 3**.
This lane did not check out, read, or edit that branch. Sequence it.

## Battery

Run at the finished lane tree, in the lane worktree, default target dir.

| leg | command | exit | result |
| --- | --- | --- | --- |
| preflight | `df -h .` | 0 | 85Gi available |
| fmt | `cargo fmt --all --check` | **0** | clean |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **0** | clean |
| tests | `cargo test --workspace --no-fail-fast` | **0** | **218 suites, 2962 passed, 0 failed, 6 ignored** |

Logs and exit codes: `gate-logs/retry-counting-{fmt,clippy,tests}.{log,exit}`;
each log header stamps the tree SHA, `rustc`/`cargo`/`gleam` versions, UTC
timestamp, and the `df` line.

The 6 ignored are the tree's pre-existing named `#[ignore]`s, each carrying its
reason (OS-process failover suites, the hex.pm-downloading dev-pipeline
template build, the norn harness). None was added or touched by this lane.

### One flake, disclosed

An earlier sibling-discovery sweep on this same tree failed
`outbox_e2e::outbox_crash_recovery_rearms_stranded_done_row_and_finishes` with
`no outbox row staged for ordinal 0` — a timing-sensitive read of staged outbox
rows under full-workspace parallel load
(`crates/aion/tests/outbox_e2e.rs:325-334`). It passed 3/3 in isolation
immediately afterwards and passed in the final battery above. It touches no AWL
and no retry declaration — the suite drives the hand-written
`aion_outbox_fixture.erl` — so it is not this lane's, but it is recorded here
rather than quietly dropped.
