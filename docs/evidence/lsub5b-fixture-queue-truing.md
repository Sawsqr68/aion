# lsub5b fixture queue-truing — the gate reached its kill

Follow-up ruled 2026-07-28 at the namespace-mint-routing landing. That lane
killed the kill-9 failover gate's FIRST blocker and reported the second at the
bytes: 172 starts out of 172 refused `NO_QUEUE_DECLARATION`, because the
package this gate's harness builds declared its activities with no task queue.
This lane trues the FIXTURE.

Base: `main` **7bd0dbb0**. Lane `build/lsub5b-fixture-queue-truing`, worktree
`.worktrees/lsub5b-fixture-queue-truing`, default target dirs, no
`rust-toolchain.toml` edit.

Toolchain stamped in `gate-logs/lsub5b-fixture-queue-truing/stamp.log`:

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
binary: rustc
commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452
commit-date: 2026-07-14
host: aarch64-apple-darwin
release: 1.97.1
LLVM version: 22.1.6

cargo 1.97.1 (c980f4866 2026-06-30)
jq-1.8.1
df /Users/tom volume: 183Gi free at the stamp (80% used)
```

---

## The outcome, first

**Full green.** The start lands, the `kill -9` fires, a survivor adopts the
shard, and the fan-out completes exactly once — **the gate's first green in its
existence**, and its first `kill -9` ever. **Fleet gate 2's discharge rides
this landing.**

## The defect, at this tree's bytes

`write_package_archive` built the `collect_four` archive with
`PackageBuilder::with_source` and no contract. That path derives the record
with `PackageContract::from_manifest`, which by design "never invents a queue"
(`crates/aion-package/src/contract.rs:50-78`) — so the manifest's bare activity
name landed in `unscoped_activities`, and the archive carried no
`WorkerContract` and no `task_queue` at all.

Under the server's fail-closed `ActivityServing::QueueRouted` posture, a
non-empty `unscoped_activities` is a terminal refusal at start admission
(`crates/aion/src/lifecycle/start_admission.rs:37-47`). That refusal is
**EARNED, not a bug**: an unserved queue would otherwise wait silently forever.
The fixture simply predated the queue-service taxonomy and was never trued when
it landed. Production behaves correctly.

Two further things were false in the same fixture, and both are fixed here:

- the manifest declared ONE activity, `fixture_activity`, which
  `collect_four` never schedules and no worker ever served. What it really
  fans out is `fan:0`..`fan:3` (`aion_outbox_fixture.erl`), which is exactly
  what `spike/liminal-fan-worker` registers handlers for.
- the worker registered those handlers with `register_activity`, which derives
  no descriptor — so its registration advertised an EMPTY activity surface.
  Harmless while nothing declared a contract for its queue; fatal the moment
  one did (below).

## The fix

`crates/aion-cli/tests/common/osproc.rs` — the manifest now declares the four
activities the fixture actually schedules, and the archive carries a real
contract built by amending the manifest-derived record rather than replacing
it: the same four names move out of `unscoped_activities` and onto
`WorkerContract { task_queue: "default", … }` — `default` being the queue
`spike/liminal-fan-worker` polls (`WorkerConfig::task_queue`). It goes in
through `PackageBuilder::with_contract`, so `PackageBuilder` stamps the `.v4`
identity over THIS contract and the package still loads through the server's
real boot path. Nothing is bypassed: `Package::contract()`'s
`has_contract_identity` check is satisfied because the identity commits to the
contract, which is the whole point of `.v4`.

The action schemas are derived by `aion_worker::activity_descriptor::<String,
String>` — the same generator the worker's typed registry uses, for the same
Rust types (`collect_four` passes each member the JSON string `"in"`; the
handler returns a `String`). Declaration and advertisement therefore cannot
drift.

The archive the harness now writes, extracted from a real run and committed at
`gate-logs/lsub5b-fixture-queue-truing/fixture-archive/`:

```json
{"input_schema":{"type":"object"},"output_schema":{},
 "workers":[{"task_queue":"default","actions":[
   {"name":"fan:0","input_schema":{"title":"string","type":"string"},
                   "output_schema":{"title":"string","type":"string"}},
   {"name":"fan:1", …}, {"name":"fan:2", …}, {"name":"fan:3", …}]}]}
```

`unscoped_activities` is absent (it serialises only when non-empty), and the
manifest's stamped `.v4` identity moved with the contract:
`4ed167cb…` (the routing lane's red) → `d7af2730…`.

**This is not a new shape.** The in-process sibling
`crates/aion-server/tests/lsub5_xnode_failover_e2e.rs:177-238` already declares
the same four `fan:N` actions on the same `default` queue, with schemas from
the same `activity_descriptor` generator, and its comment already states the
trap: declaring "the manifest's vestigial `fixture_activity` here would refuse
the fan-out worker". The OS-process fixture was simply never brought up to its
in-process sibling. It is now.

## The one file outside the brief's enumerated surface — declared

The brief's surface was `crates/aion-cli/tests/**`, `gate-logs/` and
`docs/evidence/`. One more file is in this diff and the lead should rule on it:

**`spike/liminal-fan-worker/src/main.rs`, one line** —
`register_activity` → `register_activity_with_contract`, plus the doc comment
explaining why. It was not a choice; it was forced, and forced VISIBLY, with a
red run committed as evidence.

With the archive declaring the queue and the worker unchanged, the connect-half
gate — which had been green — went RED:

```text
Error: "worker never registered against the OS-process liminal listener within 30s"
```

`gate-logs/lsub5b-fixture-queue-truing/red-connect-untyped-worker.{log,exit}`
(`.exit` = 101), with the server and worker logs from that run beside it in
`…-nodes/`. The mechanism is exact: registration admission diffs a worker's
advertised surface against every deployed `.v4` contract for the queue it asks
to serve (`aion-server/src/worker/contracts.rs` →
`aion_package::contract_diffs`), and an action the contract declares but the
worker does not advertise is a `WORKER_CONTRACT_MISMATCH`. `register_activity`
(`aion-worker/src/activity.rs:138`) populates no descriptors — only
`register_activity_with_contract` (`:169`) does — so `registration_from`
advertised `activities: []` and all four declared actions read as missing.

The typed form fixed it, and the loop is closed at the bytes: the ONLY change
between the red above and the green below was the worker's advertisement.

The alternative that would have stayed inside the enumerated surface was
declaring the queue with an EMPTY action list — which passes both checks and
proves nothing. That is the "bypass, not truth" the brief warned against, and
it is not what was built. **The whole fixture is now true: the declared queue
is the queue the worker polls, and the declared actions are the actions the
fixture schedules and the worker serves.**

### A silence worth naming (reported, not chased)

A rejected registration is invisible. `liminal-server` answers a refused
`WorkerRegister` with a `Rejected` ack carrying the reason
(`connection/apply.rs:283-306`), but the server logged nothing, and
`aion_worker`'s redial loop (`runtime/liminal_serve.rs`) has no tracing on its
connect path at all — so the worker's log stops after "starting" and the
reason never reaches a human. Both files are in the red evidence above.
Diagnosing this took reading the source; it should have taken reading a log.
**Not fixed here** (production surface, outside this lane).

## Red

Red-first is banked by citation, as the brief rules:
`gate-logs/namespace-mint-routing/acceptance-lsub5b.{log,exit}` on `main`
(`.exit` = 101) — **172 attempts, 1 distinct refusal**, every one:

```text
error[invalid_state]: failed to start workflow: NO_QUEUE_DECLARATION: workflow
`aion_outbox_fixture` version `4ed167cb…` has unscoped activities
fixture_activity; re-deploy from a checked AWL contract
  server error type: NoQueueDeclaration
```

No fresh pre-fix red was run for that half. The intermediate red above
(`red-connect-untyped-worker`) is this lane's own, and was not planned — it was
produced by the fix and is committed because it is the proof for the worker
half.

## Green — the acceptance legs

Both consumer gates of the shared archive helper were run, each ALONE, never
concurrently with the workspace tests leg.

| gate | exit | result |
| --- | --- | --- |
| `lsub5b_osproc_kill9_failover_e2e` | **0** | `1 passed; 0 failed`, 19.44s |
| `lsub5b_osproc_failover_e2e` (connect half) | **0** | `1 passed; 0 failed`, 4.96s |

```text
cargo test -p aion-cli --features haematite-backend,liminal-transport \
  --test lsub5b_osproc_kill9_failover_e2e -- --ignored --nocapture
cargo test -p aion-cli --features haematite-backend,liminal-transport \
  --test lsub5b_osproc_failover_e2e   -- --ignored --nocapture
```

`gate-logs/lsub5b-fixture-queue-truing/acceptance-kill9.{log,exit}` and
`…/acceptance-connect.{log,exit}`.

**The kill-9 gate went green TWICE on this code**, on two separate cluster
boots (19.44s here; 15.22s on the run that first reached it, before the stale
comments were trued). A gate with a seven-specimen zero-pass history deserves
more than one observation before it is called green, so it got two.

What a green MEANS here — every one of these had to hold, and the test fails
loudly on each:

1. `try_start` returned a workflow id (the `RefusalTally` never exhausted
   `START_DEADLINE`);
2. the run was observed `Running` on node 0 BEFORE the kill — if it had raced
   to `Completed` the failover would not have been exercised, and that is an
   explicit failure, not a silent pass;
3. `kill -9 <node0 pid>` was issued and the process reaped;
4. a SURVIVOR's log carried
   `adopted a downed peer's shards (SS-5b auto-failover)` — the load-bearing
   proof the shipped library did the failover, not the test;
5. the run reached `Completed` read over a **survivor's** gRPC, not node 0's;
6. its history carried EXACTLY 4 `ActivityCompleted` terminals — exactly-once
   across the kill, the owner's lost wave plus the survivor's redelivery
   absorbed by `record_fan_out_completion`.

No sleeps gate any of it. Honestly stated: the harness preserves its work dir
**on failure only**, by design, so a GREEN run leaves no per-node logs to
commit — the passing assertions above are the record, and the archive bytes
that unblocked them are committed at `…/fixture-archive/`.

## The stale comments, trued

Ruled to ride the next lane that touches these files — this one.

- `lsub5b_osproc_kill9_failover_e2e.rs` module header: "CURRENT STATUS — RED,
  and it has NEVER been green anywhere" is replaced by the real record — GREEN,
  with BOTH blockers named in order, each with the commit that killed it, and
  the pointer to `docs/evidence/failover-start-path-investigation.md` kept. The
  "do NOT read the scenario below as a proven capability" warning is gone
  because it is no longer true, and the section heading "What it is built to
  prove (not yet proven)" is now "What it proves".
- Its start-loop comment (the "this loop's premise … is FALSE" paragraph) now
  says what is true: both constant refusals are gone, and the retry is
  tolerance for a settling cluster rather than a wait on a refusal that can
  never change.
- **The `RefusalTally` stays** — it is the instrument that named both blockers,
  one after the other, and the header says so.
- `lsub5b_osproc_failover_e2e.rs` gains one true claim it can now make: its
  green also proves the fixture's declared actions and the worker's advertised
  surface agree, and it records that this step is exactly where it went red
  while the worker was untyped.

## Scope wall

IN, and built: the shared archive helper's contract; the manifest's activity
names; the two gate files' stale comments; the one-line worker advertisement
declared above.

OUT, and NOT touched: every production file under `crates/**/src` (the diff
carries none); board #157's post-kill quorum bug; the registration-refusal
silence named above; the `new_agent_e2e` legacy-scaffold `NO_QUEUE_DECLARATION`
pin, which is a DIFFERENT fixture and stays exactly as it is.

## Laws

- No `unwrap` / `expect` / `panic` anywhere in the diff, tests included.
- No `#[allow]` added.
- ≤500 lines: `osproc.rs` 469, `lsub5b_osproc_kill9_failover_e2e.rs` 427,
  `lsub5b_osproc_failover_e2e.rs` 226, the spike's `main.rs` 189.
- No `#[ignore]` added or changed; both gates keep the `#[ignore]` they have
  always had and are run explicitly with `--ignored`.
- No `rust-toolchain.toml` edit. Default target dirs. No cross-repo change.

## The battery

HARNESS R2 bridge shape. `pipefail` on every leg; no `2>/dev/null`, no
`|| true`; every `.exit` is a real captured `$?`, never a summarized exit.

| leg | command | exit | verdict |
| --- | --- | --- | --- |
| df preflight | `df -h /Users/tom` | — | 183Gi free (80% used) |
| fmt | `cargo fmt --all` | 0 | applied, tree clean |
| clippy | `cargo clippy --workspace --all-targets --all-features --message-format=json` | **0** | below |
| tests | `cargo test --workspace --no-fail-fast` | **0** | below |
| acceptance ×2 | the two gates above, each ALONE | **0**, **0** | green |

The clippy and tests legs are the standing-law commands, not nextest (aion's
server-booting e2e suites depend on cargo's binary-level serialization). The
two acceptance legs ran ALONE, sequentially, never concurrently with the
workspace tests leg or with each other.

Battery bytes: the clippy leg was **re-run in full at the frozen final tree**,
after the last stale-comment edit, and it is that run's artifacts that are
committed. The tests leg was NOT re-run for those last two doc-comment lines:
both gate files are `#![cfg(all(unix, feature = "haematite-backend", feature =
"liminal-transport"))]`, so under the feature-less workspace leg they compile
to empty binaries and a doc comment inside them is not observable to that leg
at all. The clippy leg, which DOES build them (`--all-features`), covers those
bytes. That choice is stated here rather than hidden.

`gate-logs/lsub5b-fixture-queue-truing/clippy.json` is the raw stream;
`clippy.verdict.txt` is the jq extraction beside it:

```text
DENOMINATOR compiler-artifact count: 844
error   diagnostics: 0
warning diagnostics: 0
--- any non-empty extraction below this line is a finding ---
```

The empty extraction is green against a **proven nonzero denominator of 844
compiler artifacts from the same stream** — the stream was not empty, so the
absence of findings is a real absence and not a broken filter.

`gate-logs/lsub5b-fixture-queue-truing/tests.{log,exit,verdict.txt}` carries the
workspace suite; its own `test result:` lines are the verdict, summed over all
222 of them:

```text
exit 0 — 222 suites, 3047 passed, 0 failed, 6 ignored
```

The 6 ignored are the pre-existing, individually-reasoned OS-process and
network suites, unchanged by this lane: the two `lsub5b` gates themselves
(run explicitly with `--ignored` as the acceptance legs above),
`osproc_kill9_parked_timer_resumes_and_completes_exactly_once`, the
hex.pm-downloading dev-pipeline template, the real-norn harness, and
`stacked_dev_lands_through_the_real_worker_and_review_signal`. **This lane
neither added nor changed an `#[ignore]`.**

## The unrelated tree change the gate produces, reverted and flagged

Running either gate rebuilds `spike/liminal-fan-worker`, which refreshes its
own `Cargo.lock` from a stale `aion-* 0.8.0` pin to `0.10.0` plus the
transitive additions. That drift is **pre-existing and not this lane's** — no
dependency changed here (the one-line worker edit calls a method the crate
already depended on). It was reverted with
`git checkout -- spike/liminal-fan-worker/Cargo.lock` so the diff carries only
this lane's surface, and it is flagged again: the routing lane flagged the same
file, and someone should land the refresh on its own.

## What is now proven, and what is not

Proven, live, on real OS processes: an unsteered start lands; a `kill -9` of the
shard owner mid-dispatch is survived; a survivor's `ClusterSupervisor` adopts
the dead owner's shard; the worker redials a survivor and re-registers; the
fan-out completes with EXACTLY ONE terminal per ordinal. That is the whole
scenario the gate was written for, exercised end to end for the first time.

Not proven, and not claimed: anything about board #157 beyond this scenario —
this gate kills node 0 with a pinned adopter (`ADOPTER_INDEX`), which is one
shape of failover, not all of them.
