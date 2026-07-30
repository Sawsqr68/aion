# Fleet + lanes: LIVE STATE (resume point)

## 🔴 RESUME HERE — 2026-07-31 midday. main `e16b5962`.

### PICK UP HERE, IN THIS ORDER

1. **PUSH FIRST.** `e16b5962` (the `agent`-marker WIP) may be unpushed. Check
   `git status` and `git log origin/main..HEAD` before anything else.
2. **FINISH #98, the `agent` marker.** `e16b5962` is HALF DONE and labelled so.
   Done: keyword, `ConfigLine.agent: Option<Span>`, parser (with duplicate
   refusal), canonical printer. aion-awl green, round trip holds. **NOT done,
   and it must not be called landed until both are:**
   - the checker must REFUSE `agent` on an action that is not arity-1
     `String -> String`. Until then the marker is accepted anywhere and means
     nothing, which is the dormant-surface shape the change exists to remove.
   - delete `AGENT_ACTIVITY_TYPES` at `examples/agent-dev/worker/src/main.rs:50`
     (also used at :106 and :255). That deletion is the whole payoff.
   Design is ruled in `RULING-AGENT-ACTION-WIRE-FORM.md`.
3. **Then #97-B**: bare optional scalars never narrow. The `narrow` slot exists
   at `checker/exprs.rs:14-23`; `blocks.rs:102` and `walk.rs:291` pass `None`,
   so it never reaches a statement body.

### Verification standing at this point

- `cargo test --workspace` **exit 0, 240 suites, 3818 passed, 0 failed** (run
  at `bfaa42b9`-era tree, before the dispatch and agent-marker commits).
- `cargo clippy --workspace --all-targets -D warnings` exit 0.
- `cargo test -p aion-server` was mid-run at the compaction — 26 suites green,
  0 failed, NOT complete. **Re-run it; the dispatch change is not fully gated
  until it finishes.**
- aion-awl green after the agent-marker WIP.

### Coordination

- **Callisto** may take the #95/#97-A boundary lane (`16ba5157`, preserved,
  NOT verified). She stood down on 2026-07-30 because accepting it would settle
  an open question of Tom's by doing it. If she starts: she is in
  `aion-awl`/`aion-package`/the signal boundary, so **tell her to rebase** —
  I added `schema_is_usable` to `aion-package/src/admission.rs` and the
  `agent` marker to `aion-awl`.
- **ONE FULL AION BATTERY AT A TIME ON THIS BOX.** Coordinate before starting.
- **I owe Cally the disk measurement unprompted** whenever it moves materially.
  She named me as the party owing the clearing observation rather than polling.

---

## Earlier 2026-07-31 — main `4ea6e510`


### 🔴 READ THIS FIRST: the box, and Tom's server

- **Disk was at 96% (43Gi free of 926Gi) with `aion/target` at 78G.** Cleared
  `target/debug` only — now **112Gi free at 88%**, target back to ~6G.
- **Tom's live server (pid 99844) was running from a DELETED inode.** `lsof`
  said it was executing `target/release/aion`; `ls` said no such file. Something
  had removed `target/release` while a process was executing out of it, so the
  estate's authoring server was irreplaceable. **A spare binary now exists**
  (rebuilt 09:13, `aion 0.10.0`, verified by running it). The running image is
  58,087,280 bytes and the on-disk file is 57,939,600 — DIFFERENT FILES, which
  is the proof the running one is still the old inode. It is now safe to
  restart. See #108.
- **Law worth keeping:** a derived artefact is safe to delete only if nothing is
  currently executing it. "Derived" is exactly the property that makes people
  skip the check, because derived reads as reproducible and reproducible reads
  as disposable. It is reproducible. It is not disposable while it is running.
  `cargo clean` would NOT have freed the inode, would NOT have helped, and
  **would have looked like it worked**.
- **Never `git add -A` on this tree.** It sweeps `.worktrees/` (an embedded git
  repo) into the commit. Now gitignored.

### Manifold conditions — 4 of 5 discharged

| # | Condition | State |
|---|---|---|
| 1 | AWL end to end, scoped contract under queue-routed serving | ✅ verified LIVE: `default` available, 1 connected worker, `assistant` route-active |
| 2 | Definition iteration | ✅ answered (unload already landed) + visibility built `1a2551de`. Live: 6 resident versions / 5 types / 1 superseded |
| 3 | Empty-queue dispatch visible | ✅ BUILT `4ea6e510` — visibility half only, wait still unbounded |
| 4 | Cancel contract written | ✅ `c5260d39` — `docs/CANCELLATION-CONTRACT.md` |
| 5 | beamr JIT pushed + tagged | ⏳ Artemis' `4e94cec`, unpushed, not mine |

**Condition 3, what it actually was:** the R1 queue-service taxonomy landed for
the DIRECT dispatch path and never reached the second one. `dispatch_to_node`
(`aion-server/src/worker/dispatch.rs`), production-wired as the gRPC leg of the
outbox dispatcher, was the pre-R1 loop verbatim. Three surfaces read "nothing
to see" simultaneously: `/queues/unserved` empty, `dispatch_parked` FALSE for a
parked dispatch, and the outbox row stuck `claimed` where dead-letter and
redrive could not see it because `dispatch()` never returned. Fixed by
factoring `observe_selection_miss` OUT of the selection wait so both loops call
one implementation. **The wait is still unbounded on purpose — bounding it is
Tom's decision and implementing it would have been a way of making it.**

### Corrections I made to my own record today

- **#101 was WRONG.** Versions CAN be unloaded. Only a type's ROUTE-ACTIVE
  version cannot, and there is no route-retire path. Cally had already relayed
  my wrong version to Waffles, who was designing around it.
- **#42/#45 was WRONG.** The authored workflow timeout is NOT wired to nothing.
  It is wired and gated end to end (`crates/aion/tests/workflow_timeout_e2e.rs`).
  The real cause is that a Gleam `workflow.toml` does not opt into the
  timeout-bearing identity, so it reads as not-declared. (Found by Callisto,
  verified by me at the file header.)
- **A limitation nobody re-checks is indistinguishable from a fact.**

### New findings logged today

- **#107** — there is NO parent→children relation. The system cannot answer
  "what are this workflow's children". Bigger than cancel; blocks ADR-004.
- **#106** — a derived schema that cannot compile means input validation is
  silently absent.
- **#108** — the deleted-binary incident.

---

## Earlier today — main `c5260d39`


Four commits landed today, all pushed. Workspace clippy exit 0 throughout.
Nothing is in flight at my hand; one read-only scout is mapping the empty-queue
dispatch path for condition 3.

### THE BUILD LIST IS NO LONGER MINE TO INFER

Waffles STATED what "completely ready" means for Manifold (via Cally, 22:31Z
30 Jul). Five conditions, none of them a feature. Full text in task #105.

| # | Condition | State |
|---|---|---|
| 1 | AWL authoring proven end to end, scoped contract admitted under queue-routed serving | believed discharged (#93/#99 + `ASSISTANT-AWL-LIVE-PROOF.md`) — verify the wording, then close |
| 2 | Definition-iteration story — unload/GC **or** a sanctioned throwaway-server pattern, **my choice** | ✅ ANSWERED: unload is already landed. Visibility half BUILT (`1a2551de`). Prune option NOT built; when it comes it defaults OFF and says so. |
| 3 | Empty-queue dispatch fails loudly or parks VISIBLY (#87) | **NEXT. A build, not a ruling.** Now a release condition. |
| 4 | Cancel semantics stated as a CONTRACT, even if "no cascade" | ✅ DISCHARGED `c5260d39` — `docs/CANCELLATION-CONTRACT.md` |
| 5 | beamr JIT re-entry fix pushed and tagged (#82) | Artemis' `4e94cec` is on beamr main but UNPUSHED, held on Tom's box. Not mine. |

**Gleam (#88) is OFF Manifold's critical path but the estate question is still
Tom's and still OPEN. Cally's instruction, repeated twice and unchanged: do NOT
write the Gleam limitation up as a deliberate boundary. A boundary that becomes
true because nobody answered is not a decision.**

### What landed today

1. **`241b1a06` — the differential scored its own agreement as noise.** The
   harness generated `"x"` for every string witness including one whose schema
   declares `minLength: 3`, so `import_constraints` had been starting with
   input its own contract forbids for the whole life of the harness. When the
   boundary work made enforcement real, BOTH backends refused identically —
   agreement, the one signal the differential exists to produce — and
   `fold_runs` scored it `INFRA: engine run error`. Now: the generator honours
   the constraint keywords; every witness is judged against the schema it was
   generated for; identical refusals are an AGREED-REFUSAL bucket pinned empty,
   and a one-sided or differing refusal is a DIVERGENCE (it was noise before
   too). Package hashes aliased out of the comparison.

2. **`bfaa42b9` — a contract nothing could compile validated nothing,
   silently.** The witness check immediately found two fixtures whose DERIVED
   contract `jsonschema` cannot compile. The engine's answer to a schema it
   cannot compile is to log a warning and admit the payload UNCHECKED
   (`catalog_admission.rs:134-143`) — so those workflows ran with input
   validation off and every signal said fine.
   *Cause one:* a schema door is a complete document rooted at ITS root; the
   deriver pasted it in verbatim as a nested definition, silently changing what
   `#` means, so every internal `$ref` pointed at nothing. The checker had it
   right (`checker/project.rs:272-283`); only the deriver disagreed. Fixed by
   re-rooting a door's internal pointers under wherever the door lands —
   positional, nothing renamed, no collisions.
   *Cause two:* `^{[0-9]+}$` is legal ECMA-262 and rejected by the engine's
   regex engine.
   *The general guard:* `aion awl check` now proves the contract it DERIVES
   compiles, via `aion_package::schema_is_usable` — the same compilation the
   engine performs at admission. Previously `check` was parse-and-typecheck
   only and could say ok over a hollow contract. **This guard has its observed
   red** (two uncompilable fixtures + one fixture restored to a real bucket),
   which satisfies Waffles' new no-observed-red-no-gate-promotion law.

3. **`1a2551de` — deploy now names the versions it superseded.** See the
   correction below.

4. **`c5260d39` — the cancellation contract, written down.**

### 🔴 I KILLED ONE OF MY OWN FINDINGS. #101 WAS WRONG.

I had recorded, and Cally had RELAYED TO WAFFLES, that deploy is a one-way door
with no way to remove a version. **False.** `Engine::unload_workflow_version`
(`crates/aion/src/engine/reload.rs:221-273`) exists, is wired to
`POST /deploy/unload` and `aion unload <type> <hash>`, refuses on five ordered
liveness pins, rolls the catalog back on failure, and deletes the persisted
archive so the version cannot resurrect at restart. It was a RULED design —
decision record #62 D2: manual unload, the PLATFORM owns *when*, deliberately
no auto-GC inside aion.

What is actually true is narrower and still real: **the route-active version of
a type can never be unloaded and there is no route-retire path**, so version
accumulation is bounded by what an operator prunes and TYPE accumulation is not
bounded at all.

The gap was ergonomics, not capability: yesterday's queue-unservable incident
was RESOLVED by that verb, and what cost a morning is that nothing told the
operator it existed. Hence `superseded_versions` on the deploy response.

**Lesson worth keeping: a limitation nobody re-checks is indistinguishable from
a fact, and mine was being designed around.**

### Open, in order

1. **Condition 3 / #87** — empty-queue dispatch. Scout mapping in flight.
2. Condition 1 — verify the wording and close.
3. #98 the `agent` marker (RULED, `47078711`); then #97-B (bare optional
   scalars never narrow; `narrow` slot exists at `checker/exprs.rs:14-23`,
   `blocks.rs:102` and `walk.rs:291` pass `None`).
4. ADR-004 itself (build the parent-close policy) — needs parent→children
   linkage first, which does not exist.
5. Re-run the whole battery; `ts-worker:test` red 3 still undiagnosed
   (`reconnect > completes an unaborted default sleep and leaves no timer
   behind`), and `rust:test` red 1 was OOM under contention, not a defect.

### Process notes

- `git add -A` swept `.worktrees/` (an embedded git repo) into a commit. Caught
  in my own review, reset, re-committed path-scoped. Now gitignored, so the
  mistake is unavailable rather than discouraged.
- Two seats were in this repo at once overnight (Artemis, sanctioned, in an
  isolated worktree). Anyone measuring during that window measured a moving
  tree.

---

## Historical — the state before today

## 🔴🔴 RESUME HERE — 2026-07-31. Both lanes DEAD (usage limit). HEAD `16ba5157`.

**Both Opus lanes were terminated mid-run by the weekly usage limit**, not by
failure. Limit resets **Mon 3 Aug 2pm Melbourne**. Fan-out is constrained until
then; solo work is fine.

### What landed

- **AWL query surface — COMMITTED `d00cb80b`.** `query <name>: <Type>` header
  decl + `answer <name>(…)` statement. `examples/assistant/awl/assistant.awl`
  now answers `assistant_status` with phase/round, re-registered per loop pass.
  #96 closed in substance.
- **Boundary lane (#95 + #97-A) — PRESERVED as `16ba5157`, a WIP commit I made
  because the agent died before committing.** 27 files, incl. new
  `crates/aion-package/src/admission.rs`, `crates/aion/src/signal/admission.rs`,
  `crates/aion/src/loader/catalog_admission.rs`, and a 504-line
  `crates/aion/tests/declared_contract_boundary.rs` carrying the exact ordering
  test the brief demanded. **NOT verified, NOT claimed green.**

### 🔴 THE FIRST WHOLE-REPO BATTERY EVER RUN — 8/11 green, 3685 tests passed

`bash scripts/battery.sh`, log at the scratchpad `battery.log`. Manifest:

```
PASS rust:fmt · rust:clippy · console:typecheck · console:test
PASS python:aion-worker (125) · python:aion-client (124)
PASS ts-worker:proto-drift · ts-worker:typecheck
FAIL rust:test (101) · rust:test-all-features (101) · ts-worker:test (1)
```

**None of the three reds is a product defect. All three diagnosed:**

1. `aion-package --test awl_worker_build` →
   `the_generated_worker_compiles_lints_and_proves_its_advertisement`.
   Cause: **`signal: 9, SIGKILL`** on a dozen dependency crates. The test
   spawns a NESTED cargo build in a temp dir while the battery is itself
   compiling — memory contention, the box OOM-killing rustc. This is the
   contended-red-is-not-red law. Re-run it alone to confirm.
2. `aion-rs --test awl_bc4_differential` → `covered::covered_set_is_byte_identical`.
   Report reads **`0 DIVERGENCES, 1 INFRA`** — the backends AGREE perfectly.
   The INFRA line is `schema-doors/valid/import_constraints`, where BOTH
   backends refused the start with the identical message:
   `/profile/handle: "x" is shorter than 3 characters`.
   **ROOT CAUSE (found, fix NOT yet written):** the witness generator
   `example_with_defs` at `crates/aion/tests/awl_bc4_differential/fixtures.rs:147`
   emits `Value::String("x")` for ANY string and ignores every constraint
   keyword. `profile.schema.json` declares `minLength: 3`, `maxLength: 32`,
   `pattern: ^[a-z][a-z0-9_]*$`, `age` `minimum: 0`/`maximum: 150`. The
   generator's own doc-comment at `fixtures.rs:103-107` CLAIMS it "satisfies
   schema" — it does not, and nothing caught that until the boundary lane made
   start-input validation real. So this red is the boundary lane WORKING.
   **Fix shape, ruled:** (a) honour minLength/maxLength/minimum/maximum/minItems
   in the generator so witnesses are valid and the fixture regains its real
   coverage — a start-refused fixture tests nothing about lowering; (b) assert
   the produced witness against the schema (`jsonschema` is already a workspace
   dep, used by `aion-package`) so an unsatisfiable witness fails LOUDLY at the
   fixture instead of surfacing as infra noise; (c) `fold_runs` at
   `driver.rs:292-311` classifies ANY engine run error as INFRA — a both-sides
   identical refusal is AGREEMENT and should be scored as such, not as noise.
3. `ts-worker:test` → `reconnect > completes an unaborted default sleep and
   leaves no timer behind`. NOT diagnosed. Fake-timer test; could be a real
   reconnect bug or a flake. Owed.

### 🔴 #95 IS ANSWERED: input schema was genuinely UNENFORCED at start

The differential red above is the proof: a fixture ran for months with an input
violating its own declared `minLength`. It now refuses. So the start-boundary
hole was real, and the boundary lane closed it.

### Queued, ruled, ready to dispatch when the limit resets

#98 the `agent` marker (`RULING-AGENT-ACTION-WIRE-FORM.md`), then #97-B (a bare
optional scalar never narrows; the `narrow` machinery is half-built —
`checker/exprs.rs:14-23` carries the slot, `blocks.rs:102` and `walk.rs:291`
pass `None`).

### Answered to Cally 2026-07-31 (full text in the Meridian DM)

**The tharsis/`staged_rounds` flight test: NOT norn, NOT aion — beamr.** Ran
07:10:44→08:06:17 on 30 Jul. Every piece of real work SUCCEEDED (planner, 3×
provision, 3× dev with real commits on three branches, 3× gate, 54–61 KB
results). Died 14 ms after the last gate. beamr 0.16.2's JIT re-enters the
interpreter for any external call from compiled code and BOTH edges build a
fresh `NativeServices::default()` — `jit/runtime.rs:165`,
`jit/runtime_closure.rs:197`, `interpreter/mod.rs:210-217`.
`JitRuntimeContext` (`process/types.rs:269`) carries module/registry/jit_cache
but not services. Trigger: JIT'd F → bytecode G → NIF; a direct `Native` import
deopts and is safe, which is why the first two fork/joins worked. Artemis owns
it; branch `defect/beamr-jit-loses-native-services`. **`replay_driver` is in
the dropped set — that is a durability violation presenting as silence.**

**Open question put to Tom via Cally:** is Gleam a supported authoring surface,
or is AWL the only one? Gleam packages build and deploy cleanly and then cannot
start (#88). Silence-default: AWL is the path, and I write the Gleam limit up
as a deliberate documented boundary rather than a bug.

## 🔴 TWO LANES IN FLIGHT RIGHT NOW — 2026-07-30 ~13:20Z (HISTORICAL). `origin/main` = `7bbe947f`.

Both are Opus agents working DIRECTLY in this tree (no worktrees, per Tom).
They were briefed into disjoint crates and told to stage only their own paths
by name — never `git add -A`. **If you are resuming and they are gone, check
`git status` before doing anything: uncommitted work in `crates/aion-awl` or
`crates/aion`+`crates/aion-server` is theirs, and a `reset --hard` destroys it.**

| lane | brief | crates it owns | task |
|---|---|---|---|
| AWL query surface | `BRIEF-AWL-QUERY-SURFACE.md` | `crates/aion-awl` | #96 |
| Contract at the boundary | `BRIEF-DECLARED-CONTRACT-AT-THE-BOUNDARY.md` | `crates/aion`, `crates/aion-server` | #95 + #97-A |

**Queued behind the AWL lane, ruled and ready to dispatch the moment
`crates/aion-awl` is free:** #98, the `agent` marker on an action declaration
(`RULING-AGENT-ACTION-WIRE-FORM.md`), then #97-B (a bare optional scalar never
narrows, so `T?` is write-only — the `narrow` machinery is half-built, see the
end of the boundary brief).

**Landed by me this session, pushed:** the live restoration (below), the
bindings-gate verification, the console README correction, both briefs and the
ruling, and `scripts/battery.sh`.

**OWED:** one end-to-end run of `scripts/battery.sh`. It has NOT been run whole
— only its self-test (`AION_BATTERY_SELF_TEST=1`, exits 1 correctly), its
`gate_in` mechanism, and the four non-Rust gates by hand (console typecheck 0,
python worker 125 passed, python client 124 passed, ts check:proto matched).
The Rust gates were skipped deliberately: two lanes are mid-build and a red
would have been theirs, not the script's. Run it once the tree is theirs no
longer.

## ✅✅ RESTORATION DONE — 2026-07-30 12:41Z. THE FULL AWL ASSISTANT RAN.

**The restoration sequence banked at `9fb28133` is executed and complete.**
Everything below it that describes the restoration as "staged but NOT done"
is now historical — read this block first.

Queue `default` is **up**: `available: true`, `connected_workers: 1`, worker
`registered liminal worker in-band` at 12:34:48Z with no refusal after it.
The 974 refusals in `~/.aion/server.log` all predate 11:08Z.

**What is deployed and route-active right now** (`GET /deploy/versions`):

| type | hash | source |
|---|---|---|
| `assistant` | `d9774285e352824b` | `examples/assistant/awl/assistant.awl` — the full port |
| `assistant_spike` | `256ceddad674c60c` | the agreeing 3-arg spike |
| `agent_dev` | `8186d26d2e60d702` | unchanged |
| `shell_run` | `b2177c0d64272337` | unchanged |
| `staged_rounds` | `5f0f3bf19ed9b6d2` | unchanged |

Five superseded versions (four `assistant`, one `assistant_spike`) were
unloaded — a stale version is still a reachable contract and still gates
admission, which is what took the queue down in the first place.

**Both documents were deployed through `PUT /awl/documents/{path}` +
`POST /awl/deploy`, i.e. the studio path, not the CLI** — deliberately, because
a CLI deploy leaves the document invisible in the studio (#91). Tom can see
both in the authoring workspace.

**The live proof:** workflow `1b3afbde-823e-4374-8910-7e3fa3a83fae`, 14
events, provision → round 1 → signal → round 2 → signal → `WorkflowCompleted`
with `{"outcome":"ended","disposition":"OperatorEnded","rounds":2, …}` — every
field exact. Round 2's dispatch carried the operator's message verbatim into
the same norn session and the agent answered from memory of round 1.
**Full record with the recorded bytes: `ASSISTANT-AWL-LIVE-PROOF.md`.**

**What it does NOT prove, stated there and repeated here:** the `stalled` and
`capped` budget-exhaustion paths never executed (covered analytically by
`crates/aion-awl/tests/assistant_example.rs`, not live); the degraded-session
resume path never executed (fresh session — the epoch fix is proven by hand,
separately); and the prompt is still JSON-wrapped on the wire (#98).

**Live processes:** server pid 99844 (`./target/release/aion server`, the
20:13 release binary — deliberately NOT rebuilt). Worker pid 35591
(`agent-dev-worker --address 127.0.0.1:50061 --norn-account default
--norn-allow-degraded-session`), logging to `~/.aion/worker-assistant.log`.

**Two new seams found while proving it, both filed:** `/workflows/start`
demands `content_type: "application/json"` while `/workflows/history` returns
`"Json"` for the same payload (read and write disagree, so a payload read off
a run cannot be resubmitted); and history elides payload bytes over a default
limit unless `payload_limit_bytes: 0` is passed, hiding exactly the agent
replies an operator wants to read.

## ✅ PUSHED — 2026-07-30 ~10:10Z. Board clear, nothing in flight.

**`origin/main` is `c443d584`.** 29 commits pushed (`8fb4c153..c443d584`);
the count had been misrecorded as 10, was measured at 26, and grew to 29 with
this turn's three landings. Nothing is uncommitted and no agent is running.

**The battery that backs it, on the exact pushed tree:**
`cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero
diagnostics · `cargo test --workspace` **`CARGO_TEST_EXIT=0`, 238 suites,
3759 passed, 0 failed, 5 ignored** · console `bun run typecheck` exit 0,
`bun test` 580 pass / 0 fail.

⚠️ **Read the exit code, not the summary.** The first attempt at this battery
reported "exit code 0" while a test had actually failed — the command ended in
`echo`, so the captured status was the echo's. The real value was 101. The log
now carries `CARGO_TEST_EXIT=` written by the runner itself. A summarised exit
is never citable.

⚠️ **3759 passed does NOT include the failover proofs.** Five tests are
`#[ignore]`d (task #86) and three of them are the durability evidence: the
cross-node kill-9 failover, the parked-timer hard-kill, and the real-worker
OS-process test. A green battery is silent about them.

**THIS TURN'S LANDINGS:**
`e3c4a3ff` #80 admission + the paused-run unload strand fix ·
`a4a7afde` BC-4 ratchet (my own red, see below) · `c443d584` fmt + count.

**I left main RED and reported the lane green.** `be0112bf` added
`step-bodies/valid/send_signal.awl` without bumping `COVERED_COUNT`, so
`covered_set_is_byte_identical` failed on every run from that commit until
`a4a7afde`. The #80 agent found it, not me. The lesson is not "bump the
ratchet" — it is that a per-crate gate (`cargo test -p aion-awl`, which I ran
and which passed) cannot see a cross-crate ratchet, and I cited it as if it
could.

## 🔴 RESUME HERE — 2026-07-30 ~11:00Z. HEAD = `87aaf8f2`. TWO AGENTS IN FLIGHT.

**Do not edit these paths — an agent owns each:**
- Agent A (assistant AWL port + worker fix): `examples/agent-dev/worker/**`,
  `examples/assistant/**`, `docs/design/aion-authoring/ASSISTANT-AWL-PORT.md`
- Agent B (task #87, engine-side): `crates/aion/**`, `crates/aion-server/**`,
  `crates/aion-core/**`

**THE ANSWER TO "IT JUST STOPPED AGAIN" — now complete, and it is TWO faults,
neither of them in the durable engine.**

1. **The 2 July session was alive the whole time and nobody could tell.** Run
   `f9b062c2-d260-43f9-8374-d3210330f567` sat parked on `assistant_continue`
   for 28 days. A live `assistant_status` query answered instantly:
   `{"phase":"awaiting_operator","round":2}`. The engine held queryable state
   four weeks past its last event. It was never stuck — it was waiting for him.

2. **But it was a zombie.** Signalling it woke it correctly and it died in 11
   seconds: norn refused with *"session '…-assistant' is a fresh-epoch
   projection; explicit resume approval is required"*. The agent-dev worker
   composes `--resume-if-exists` and `--account` and never
   `--allow-degraded-session`, so ANY agent session outliving a provider epoch
   is permanently unresumable through the worker. **Fix proven by hand against
   that exact session id before a line was written** — it resumed WITH ITS
   MEMORY INTACT, naming a concrete item from the 2 July conversation
   unprompted. Full evidence:
   `docs/design/aion-authoring/DURABLE-AGENT-SESSION-EPOCH-FINDING.md` (commit
   `87aaf8f2`), task #89. Worker fix dispatched to Agent A.

   ⚠️ **I killed that run doing it.** It is now `Failed`. It would have died
   identically on his own next message, so the loss bought a diagnosis — but it
   is a real durable run that no longer exists, and the query was free while
   the signal never could be.

3. **Newer sessions hung forever showing `Running`** — task #87, dispatched to
   Agent B. Confirmed at the bytes: `ActivityScheduled` and `ActivityStarted`
   are recorded atomically at DISPATCH with the identical timestamp
   (`10:46:51.332538Z` for both, seq 14/15). `ActivityStarted` means "the
   engine sent it", not "a worker took it". On a queue with no workers that is
   indistinguishable from live work, forever, with no timeout.

**Also filed:** task #90 — a retry-exhausted activity records
`{"kind":"Terminal","message":"retryable:…"}`. The kind flips, the prefix
doesn't. A lie in an event that can never be rewritten.

**SERVER RESTARTED — and it exposed a P0 that is DOWN RIGHT NOW (task #93).**

Old server pid 47610 (started 17:08, running a binary older than the one on
disk) was SIGTERM'd and exited cleanly in 2 seconds — #72 verified on his own
box, not just in tests. New server is **pid 99844**, same invocation, same cwd,
`nohup`+`disown`. WAL recovery clean on every shard (`corrupted_tail=false`,
`replayed_mutations=0`). All 20 workflows, 5 routed versions and both authoring
documents survived.

🔴 **But `agent-dev-worker` can no longer register, so queue `default` has ZERO
workers.** `/awl/workers/availability` → `available:false, connected_workers:0`.
The refusal contradicts itself inside one log record: it lists `assistant` in
`activity_types` and simultaneously reports it `<missing>`. Traced: the log
prints `registration.activity_types` (names, `liminal_transport.rs:1648`) while
admission reads `registration.activities` (typed descriptors,
`liminal_transport.rs:1495-1522` → `compatibility.rs:62-90`). Two different
fields. Full evidence in task #93; dispatched to Agent B as its priority.

**NOT a regression from #80** — the worker registered at 10:24, `assistant_spike`
deployed at 10:37, so its contract had never faced admission. The restart is the
first time it ever did.

⚠️ **Correction to `e819b96a`'s claim.** "AWL drives the agent worker end to end
— LIVE PROVEN" is true at the DISPATCH level and was NEVER exercised at the
ADMISSION level. The run and its reply were real; the contract check was
bypassed by registration ordering.

⚠️ **My cost:** before the restart the worker was registered and the queue
worked. The state was fragile — any reconnect would have done this — but I
triggered it, and it is down now and was not before. No clean workaround:
`assistant_spike` is the only version of its type and is route-active, so
`/deploy/unload` correctly refuses it and there is nothing to re-point to. The
fix is the restoration.

🔴 **`agent-dev-worker` IS NOT RUNNING — I stopped it (task #94).** Refused, it
re-dialled every 500ms forever and the server re-logged the full ~1.5KB refusal
each time: **100KB of server.log per 30 seconds, ~12MB/hour**, 300 of the last
300 lines identical. SIGTERM froze the log instantly, which is the proof. A
CONTRACT refusal is permanent until a human changes something, so retrying it
2/s cannot ever succeed and buries every other line.

**After #93 lands, the restart command is:**
```
./examples/agent-dev/worker/target/release/agent-dev-worker \
    --address 127.0.0.1:50061 --norn-account default
```
(plus whatever `--allow-degraded-session` opt-in Agent A adds, which is what
makes the owner's parked assistant sessions resumable at all — task #89.)

**AGENT A LANDED 4 COMMITS** (`d4a2578b` degraded-session fix · `241c9094`
worker-side prompt composition · `c75e61b3` the full `assistant.awl` ·
`0d7fbb2e` the differences ledger). Zero files under `crates/` — the territory
split held. It is now resumed on two follow-ups: the worker descriptor fix
(below) and a suspected loop defect I sent it.

**#93 IS ANSWERED — the GATE IS RIGHT, the WORKER IS BROKEN.** `agent-dev-worker`
advertises 8 activity NAMES and ZERO typed descriptors: `registry.rs:72-79`
uses plain `register_activity`, whose own doc says registering a handler with
no descriptor "is what makes a worker invisible to contract admission", and
`AgentHarnessConfig::new` sets `activity_descriptors: Vec::new()` with
`main.rs` never calling `.with_activity_descriptors(...)`. The admissible shape
already exists at `crates/aion-cli/src/worker_agent/serve.rs:185`. **Do not
loosen the gate.** Agent B's #93 scope is now the contradictory LOGGING only.

**SUSPECTED DEFECT sent back to Agent A, unresolved:** in `assistant.awl:132-140`,
if the inner nudge loop exits via `max max_nudges` it leaves `nudge.message ==
""` and `nudge.end == false`, so `until round.ended` stays false and the outer
loop dispatches `assistant(prompt: "")` — empty prompts to a real agent until
the round cap. The file's own line 148 says the opposite. **Not confirmed** —
I may be misreading AWL's `max` semantics and asked for evidence either way.

**Four findings filed from the port:** #95 an AWL `input_schema` may not be
enforced at start (a validation hole in EVERY AWL workflow — establish the fact
first) · #96 AWL has no `query` surface at all, so the live-state query that
proved the 28-day run healthy cannot be authored · #97 an undecodable signal
payload KILLS the run, and optionals cannot be read — which forced the control
signal's fields to be REQUIRED, so the documented `{"end": true}` payload is
now invalid and callers need updating · #94 the refusal flood.

⚠️ **I told Agent A "AWL cannot compose strings" as fact and it is FALSE** —
`dev_brief.awl` concatenates runtime bindings in production. The worker-side
composition decision stands on the better reason (the worker already
materializes the resources the prompt describes), but the stated reason was
wrong and it checked rather than designing around it.

## ✅ MY OWN BATTERY, on the pushed tree `dc87e91a` — not a report, my run

```
FMT_EXIT=0                # cargo fmt --all --check
CLIPPY_EXIT=0             # --workspace --all-targets -D warnings
CLIPPY_DIAGNOSTICS=0
CARGO_TEST_EXIT=0         # cargo test --workspace
238 suites · 3785 passed · 0 failed · 5 ignored
```

Up from 3759 at session start. The 5 ignored are still task #86, and three of
them are the failover proofs — a green battery remains silent about those.

🔴 **AND THE BATTERY MISSED A BREAK (task #100).** `cargo test` regenerates the
console's TypeScript through ts-rs. The new `unserved` field landed referencing
`UnservedActivity`, **a type ts-rs never emitted** — it appears exactly once in
the generated file, at the reference. `bun run typecheck` → `TS2304: Cannot find
name 'UnservedActivity'`, `TSC_EXIT=2`, while every Rust gate was green.

Three defects there and the third is the one that matters: generated output is
NOT committed (so the tree is simultaneously stale AND broken, and any
`cargo test` flips it from one to the other), and **`cargo test --workspace` is
structurally blind to the console.** With Actions disabled on this repo, a
cross-language break can be reported green by two agents AND a full battery.
Neither agent ran `--workspace` at all — one ran three `-p` crates, the other
one. Caught only because the battery was re-run at this seat.

## 🔴🔴 RESUME HERE FIRST — 2026-07-30 ~22:20. `origin/main` = `b6853187`, pushed.

**BATTERY GREEN AT MY SEAT on `b6853187`** (my run, not a report):
`FMT_EXIT=0` · `CLIPPY_EXIT=0` / 0 diagnostics · `CARGO_TEST_EXIT=0` ·
**238 suites, 3785 passed, 0 failed, 5 ignored** · console `tsc` 0 ·
`bun test` 580/0 · **and zero TS drift after a full test run** (the thing that
was broken).

**THE RESTORATION IS NOT DONE. Everything is ready to do it. Steps, in order:**

1. **Deploy the updated spike** — `cargo run --bin aion -- deploy
   examples/assistant/awl/assistant_spike.awl`. It now declares the 3-arg
   `assistant_provision(repo_path, run_id, objective)` and agrees with the
   worker. This is what unblocks admission.
2. **Unload the OLD spike hash** `93971008e5511edfec8469271a49e1ab8626f4591eace35a37b987d09a910224`
   via `POST /deploy/unload` — it only becomes unloadable once the new version
   is route-active.
3. **Deploy `examples/assistant/awl/assistant.awl`** (the full port).
4. **Start the worker** — its binary was rebuilt 22:13 WITH the descriptor fix:
   ```
   ./examples/agent-dev/worker/target/release/agent-dev-worker \
       --address 127.0.0.1:50061 --norn-account default
   ```
   Add Agent A's `--norn-allow-degraded-session` opt-in if resuming a parked
   session (task #89).
5. **Verify**: `POST /awl/workers/availability {namespace, task_queue}` must
   report `available: true` with a connected worker, and the server log must
   carry NO `REFUSED liminal worker registration`.
6. **Live-prove** a run — and describe it as a proof of the DEGRADED path until
   #98 (the JSON-wrapped prompt) is settled.

**I did NOT rebuild `target/release/aion`.** The server (pid **99844**) still
runs the 20:13 binary, which predates Agent B's #93 refusal fix. That is
DELIBERATE: Agent B is back in `crates/` with uncommitted edits and a release
build would compile a half-edited tree. Rebuild and restart the server only
once Agent B has landed and the tree is verified clean. **The restoration does
not need it** — the queue was blocked by the worker descriptors and the spike
contract, both now fixed.

**`agent-dev-worker` is NOT running** (I stopped it, task #94).

**Agent B is IN FLIGHT** on the expect-test gate ruling (hole 2 of #100): fail
without writing on any committed-vs-regenerated mismatch, regenerate only under
an explicit env opt-in matching whatever convention already exists — do NOT
invent a third. Plus the same two questions asked of `aion-proto-generated`.

**Tasks filed this session:** #89 durable-session epoch (fix proven) · #90
`retryable:` on a Terminal · #91 CLI deploy invisible in studio · #92 tharsis
second blocker · #93 admission field mismatch (ANSWERED: gate right, worker
broken; fixed) · #94 refusal flood · #95 input_schema maybe unenforced · #96 no
AWL query surface · #97 AWL signal semantics · #98 JSON-wrapped prompt · #99
spike/port conflict (RESOLVED) · #100 cross-language gate hole · #101 deploy is
a one-way door (CONFIRMED).

## 🔴 RESTORATION SEQUENCE — do this in order, nothing else restores the queue

Queue `default` is DOWN for **two independent reasons**, and fixing one alone
changes nothing.

1. **Agent B's work must be committed and its gates re-run on the committed
   tree.** It reported green (clippy 0/0 diagnostics, 1919 passed/0 failed) but
   committed NOTHING — every crates/ change was loose in the working tree. It
   has been sent back to commit. **Verify at the bytes; do not take the report.**
2. **Rebuild `target/release/aion`** so the server carries B's refusal fix.
3. **`assistant_spike` must stop demanding the OLD `assistant_provision`
   signature (task #99).** The port added `objective`; input compatibility is
   contravariant, so a worker requiring it is refused by the spike's
   two-argument contract. With both deployed NEITHER runs. A route-active
   version cannot be unloaded, so the only path is: update or retire the spike →
   deploy it → the old hash becomes unreachable → `/deploy/unload` accepts it.
   **Consider retiring the spike outright** — its job was to prove the seam, the
   port exists, and all it does now is conflict.
4. **Then** deploy `assistant.awl` and restart the worker:
   ```
   ./examples/agent-dev/worker/target/release/agent-dev-worker \
       --address 127.0.0.1:50061 --norn-account default
   ```

## ⚠️ THE "LIVE PROVEN" CLAIM IN `e819b96a` NEEDS TWO QUALIFICATIONS

Both found by Agent A, neither by me. The asparagus run happened and the reply
was real. But:

- **Admission was never exercised** — the worker registered at 10:24, the spike
  deployed at 10:37, and dispatch does not re-check.
- **The prompt arrived JSON-WRAPPED (task #98).** AWL synthesizes an input
  record with no arity-1 unwrap, so `assistant(prompt: String)` puts
  `{"prompt":"..."}` on the wire, and the norn adapter unwraps only a BARE JSON
  string. The agent literally received
  `{"prompt":"You are the aion AWL authoring assistant: …"}` and coped. It is
  degraded, not broken, which is exactly why nothing surfaced it.

It worked *despite* being wrong, and I reported it as proof. Any live proof
taken before #98 is settled is a proof of the DEGRADED path and must be
described that way.

**I was right about the nudge defect** — Agent A confirmed it at the lowering
(`emitter/loops.rs:83-121` computes ONE exit expression used by both the `until`
and the exhaustion arm, so an exhausted inner loop just continues the outer
body) and fixed it in `b7cd6425`. It proved the fix by evaluating the document's
own loop conditions over the whole domain, and **verified by mutation** that
restoring the old condition fails the test.

**Still owed:** the staged-rounds worker (pid 49833) is still an old binary.

**Do not tell Tom any of this as status.** Fix it, prove it live, then tell him
with the working thing attached — he has asked repeatedly not to be given
progress without substance.

**THE OTHER NEXT STEP (Agent A):** port `examples/assistant` to AWL — the full
workflow, not the spike. The seam is PROVEN (see below), so this is now a
straight port rather than a risk. Source of truth for behaviour:
`examples/assistant/src/assistant.gleam` (305 lines), whose module doc states
the whole contract: `assistant_provision -> [ assistant round; await operator ]*`
bounded at `max_rounds = 50`; ONE control signal `assistant_continue` where
`{"message": "..."}` continues and `{"end": true}` ends cleanly as
`OperatorEnded`; an undecodable or blank payload is a TOLERATED no-op nudge
because an operator typo must never kill a live session; a spent budget is the
`RoundCapExhausted` disposition and NOT an error; agent rounds carry NO timeout
by design. Prompt composition lives in `assistant/prompts`. Keep the Gleam
version in place until the AWL one is live-proven.

**✅ THE SEAM IS PROVEN, LIVE, ON TOM'S SERVER.**
`examples/assistant/awl/assistant_spike.awl` — `awl check` clean (2 steps), then

```
aion run … --input '{"objective":"Reply with exactly one word: asparagus","repo_path":""}'
{"outcome":"answered","payload":{"reply":"asparagus",
 "workspace_path":"/Users/tom/.aion/clones/5b86b02e…/repo"}}
```

provision → agent round → typed outcome, through the real agent-dev worker on
the real norn harness. Action shapes mirror what the worker advertises
(`assistant_provision(repo_path, run_id) -> Workspace{path}`,
`assistant(prompt: String) -> String`); `workflow.id` is available in AWL and
MUST be passed as `run_id` or the agent works somewhere other than where it was
provisioned.

**HIS PROCESSES, as I leave them:** server pid 47610 and staged-rounds worker
pid 49833 are STILL THE OLD BINARIES (restart not yet done — it was pending
Tom's word and then overtaken by the investigation). `target/release/aion` IS
now rebuilt (0.10.0, exit 0). A NEW process is running that I started:
`agent-dev-worker --address 127.0.0.1:50061 --norn-account default`, which is
what makes queue `default` servable at all.

**AWAITING TOM** on #88 — recommended option 2+3 (move examples to AWL, stop
claiming the Gleam path is first-class, make it refuse loudly). Option 3 is
DONE and landed (`6cae9127`). Silence-default was to proceed with 2.

## 🔴 LIVE-SERVER INVESTIGATION, 2026-07-30 ~10:30Z — READ BEFORE TOUCHING HIS BOX

Checking whether a restart was safe turned into the answer to Tom's "it just
stopped". Three findings, each with its own file, all pushed:

1. **`ASSISTANT-STUCK-FINDING.md` / task #87.** His assistant stopped on
   6 July. `assistant_provision` was started four times and completed zero
   times over 24 days, re-armed once per server restart, on a queue with zero
   connected workers — while the status read `Running` the whole time. The
   healthy control (workflow `f9b062c2`) is ALSO `Running`, correctly, because
   a long-lived assistant idles awaiting a signal. One status covers "waiting
   for you" and "dead for three weeks". **I misread the healthy one as stuck on
   first look, which is the point.**
   Also the exact #42 receipt: the workflow declares `timeout_seconds =
   604800` (seven days, explained in words in its own comment) and was 17 days
   past it, still `Running`.

2. **The agent-dev worker never passed `--account`** (fixed, `3a14be8f`). It
   composes norn with `--resume-if-exists`, which norn refuses without an
   account alias, so every agent round failed three times while the worker
   logged "connected and registered; serving dispatches". `--norn-account` is
   now REQUIRED — defaulting it would only move the failure. `RUNBOOK.md` and
   `DAVID.md` documented the broken invocation; both corrected. DAVID.md's
   preflight (`norn -p --fast "say ok"`) does not resume a session, so it
   passes while the real invocation fails — a preflight that structurally
   cannot catch the bug it precedes.

3. **`GLEAM-PATH-CANNOT-START-FINDING.md` / task #88 — the big one.** A
   Gleam-authored package with ANY activity cannot start under queue-routed
   serving. `DeclaredActivity` has exactly one field (`activity_type`), so the
   manifest has nowhere to record a queue; `from_manifest` marks every activity
   unscoped; `start_admission` refuses anything unscoped. Only AWL emits a
   scoped contract. `activity.task_queue()` LOOKS like the fix and is not —
   its value never reaches the manifest (verified: rebuilt, redeployed,
   identical refusal). Deploy SUCCEEDS; only start refuses, so a broken package
   is indistinguishable from a working one until used. This contradicts
   CLAUDE.md's "two first-class surfaces".

**PROVEN GOOD, incidentally:** the durable dispatch held a parked activity for
24 days and delivered it the instant a worker connected (attempt 4, completed
in under a second). The waiting machinery is sound; the reporting around it is
not.

**AWAITING TOM:** recommended option 2+3 on #88 (move examples to AWL, stop
claiming the Gleam path is first-class, and make it refuse loudly), which means
authoring the assistant in AWL. Silence-default is to start on it.

**PENDING DECISION FOR TOM — do not take it unilaterally.** His running server
(pid 47610) and worker (pid 49833) are BOTH still old binaries. Everything
landed today — the chmod fix, `send` on the canvas, the scaffold command, the
admission fix — is on disk and pushed but not in the processes he is using. A
restart gives him all of it in one motion and there is no run to interrupt
(tharsis is blocked on beamr regardless). BUT `target/release/aion` was DELETED
by an agent's cargo activity and must be rebuilt first — a long build. The
worker release binary IS already rebuilt (exit 0) and carries both halves of
#79.

**Pushed separately:** branch `defect/beamr-jit-loses-native-services`
(`fd2330b9`) — the #82 repro, the beamr patch, and the standalone pure-beamr
wall. **ARTEMIS HAS CLOSED THEIR GATE ON IT.** Reviewed at their bytes, verified
on beamr `main`, blocker reported cleared to the stack lead so a release slot can
be found. An executor builds red-first against the repro, then their review at
the bytes. They will ping for a real-workload run when there is a candidate.
**NOTHING IS OWED BY ME ON #82.**

**Verified by running it myself, not on report:** `aion awl scaffold` against the
real `examples/staged-rounds/awl/staged_rounds.awl` → exit 0, 11 actions across
5 connections (planner, shell, developer, reviewer, remediation), matching the
hand-written worker exactly. Console: typecheck exit 0, **580 pass / 0 fail**.

---

# (historical) LIVE STATE at 2026-07-30 ~08:00Z

Supersedes the earlier revision of this file. `FLEET-SMOKE-STATE.md` remains the
record of the first smoke flight.

## LIVE PROCESSES — Tom's own server (he ordered the restart at 07:05Z)

- **Server** `./target/release/aion server`, started 07:08Z from the repo root,
  log `~/.aion/server.log`. HTTP **127.0.0.1:8080**, gRPC **50051**, liminal
  worker listener **50061**. Store = repo-local legacy `./aion-data`
  (haematite, 64 shards) — Tom's real state, preserved. Authoring root
  `~/.aion/authoring`. v0.10.0, deploy enabled.
- **Worker** `examples/staged-rounds/worker/target/release/staged-rounds-worker
  --profiles-dir ./profiles --identity tharsis-fleet --norn-account default
  --max-parallel 4`, log `~/.aion/worker.log`. All FIVE connections registered.
- **NEW `~/.aion/config.toml`** is what makes the worker channel exist.
  `store.data_dir` / `authoring.workspace_dir` DELIBERATELY ABSENT so the
  resolver picks the legacy working-dir locations where the real state lives.
- Canvas: `staged_rounds.awl` (302 lines) loaded into the studio, on disk at
  `~/.aion/authoring/staged_rounds.awl`. Console route **/authoring**.

## 🔴🔴 THE THARSIS RUN DIED AT 08:06:17Z — AND THE FAULT IS OURS (task #82)

Read this section first. It supersedes the "IS EXECUTING" section below.

**Every piece of real work succeeded.** Planner, 3x `provision_item`,
3x `dev_item` (real commits on three item branches), 3x `gate_item` — all
`ActivityCompleted`, gate results 54-61 KB each. Then, 14 ms after the last
gate landed, the workflow failed:

```
WorkflowFailed seq 37 @ 2026-07-30T08:06:17.859413Z
  message: "workflow 3 returned error"
  details: {"tag":"AwlActivityFailed",
            "message":"activity engine failure: engine NIF state is not installed for this runtime"}
```

Evidence chain, all verified at my seat:
- `"workflow {pid} returned error"` — `crates/aion/src/runtime/outcome.rs:116`.
  pid 3 = the top-level workflow's beamr process.
- `"engine NIF state is not installed for this runtime"` —
  `crates/aion/src/runtime/nif_state.rs:432`, `ctx.nif_private_data()` was `None`.
- `"activity engine failure: …"` — the AWL error wrapper, so it was hit inside
  an ACTIVITY-dispatching NIF.
- Aion installs the data ONCE: `crates/aion/src/runtime/handle.rs:141`
  (`SchedulerConfig::nif_private_data`).
- beamr 0.16.2 has exactly THREE sites that put it on a `ProcessContext`:
  `interpreter/opcodes/native_call.rs:169` (**gated on `if let Some(svc) =
  ctx.services`**), `interpreter/opcodes/trampoline.rs:166`,
  `scheduler/execution/core.rs:1507`. `native/context/mod.rs:557` and `:616`
  both default it to `None`.
- Failing call site: `plan_reviews(...)` at `staged_rounds.awl:237` — the plain
  activity call right after the THIRD `fork…join` in the `execute` loop.
- JIT correlation, NOT yet a cause: `~/.aion/server.log` "defining function
  funcidNN" at 07:25:52 (44), 08:06:08 (45), **08:06:17.845 (46)**. But
  `grep` of `beamr-0.16.2/src/jit` finds NO NIF call sites at all.

### ✅ SOLVED — it is a BEAMR defect. We are BLOCKED on a beamr release.

beamr 0.16.2's JIT re-enters the interpreter for ANY external call made from
compiled code, and BOTH re-entry edges construct a fresh
`NativeServices::default()` — every facility `None`:

- `jit/runtime.rs:165` `jit_call_interpreted` (external-call edge)
- `jit/runtime_closure.rs:197` `call_interpreted_closure` (closure edge)
- `interpreter/mod.rs:210-217` `run_with_registry` builds the empty bundle

`JitRuntimeContext` (`process/types.rs:269`) carries module/registry/jit_cache
but NOT the services, so those edges have nothing to restore.

**Trigger shape: JIT'd F → bytecode G → NIF.** A direct `Native` import is SAFE
(`jit/runtime.rs:139-145` deopts on `ResolvedImportTarget::Native`). That is
exactly why the two earlier fork/joins and the plain `fold_phase` call worked —
nothing had been compiled yet.

The JIT correlation is now DEMONSTRATED CAUSE, from the log itself: funcid46's
Cranelift IR at `~/.aion/server.log:120` is a one-arity function whose ENTIRE
body is `call fn6(v1, 2141, 5, 1, v0)`, and `fn6`'s signature
`(i64,i64,i64,i64,i64)->(i8,i64)` is precisely
`jit_call_interpreted(process, module_index, import_index, arity, args)`.

🔴 **BLAST RADIUS IS FAR WIDER THAN OUR NIF: 33 FIELDS.** I first reported 8;
Artemis counted the struct. Beyond the obvious ones (`atom_table`,
`spawn_facility`, `link_facility`, `local_send`, `timers`, `ets_facility`,
`supervision_facility`, `replay_driver`) it also drops `bif_registry`,
`capability_audit_sink`, `capability_violation_handler`, the io / file_io /
tcp_io / pg / readiness facilities, `code_management_facility` and
`suspension_registrar`.

### Artemis VERIFIED all of it on beamr `main` (689c092) — NOT fixed forward

Any cut inherits it. Three things they added, two of which change the story:

1. **The replay danger has a line and a mechanism.**
   `interpreter/opcodes/native_call.rs` guards the ExternalIo/Entropy
   interception on
   `… && let Some(svc) = ctx.services && let Some(driver) = &svc.replay_driver`.
   With services empty that guard is FALSE and **the interception is skipped
   entirely** — the call takes the live path. So it is not that replay
   misbehaves; replay interception is CONDITIONAL ON A FIELD THE JIT EDGE
   GUARANTEES IS ABSENT.
2. **There is NO capability bypass.** They went looking specifically so the
   worst reading could be retired. `granted` is computed from
   `process.capabilities()`, never from services, so enforcement is intact
   below a JIT frame: a denied call is still denied. What is lost is
   `capability_audit_sink` (the audit event is never recorded) and the
   embedder's violation handler (falls back to stderr). **A hole in the
   evidence, not in the gate.**
3. `bif_registry: None` reproduces a defect class beamr has ALREADY diagnosed
   in a different consumer — an empty BIF registry makes `erlang:'+'` resolve
   Deferred and then get refused as a guard BIF. Two doors into one hole makes
   this a design question about WHO MAY CONSTRUCT a `NativeServices`, not a
   missing field on a struct.

Agreed landing shape: fold `services` into `JitRuntimeContext::new` at all 10
call sites (8 are tests, passing an explicit empty and saying so) so a
services-less context cannot be reached by defaulting.

⚠️ **beamr 0.16.3 IS published** (Artemis probed it) while this tree still
declares 0.16.2. It does NOT carry this fix. Take the step with the 0.17.0
bump, not before — one thing at a time.

**Repro + patch PUSHED: branch `defect/beamr-jit-loses-native-services`.**
`crates/aion/src/runtime/jit_nif_boundary_tests.rs` is a DIFFERENTIAL — two
tests, identical three-module graph, the ONLY variable is whether compiled code
is in the scheduler's JIT cache; compilation goes through `JitCompiler::compile`
on `Module::function_instructions`, byte-identical to `CompilationJob::run`, no
faked entry. 0.32 seconds. VERIFIED AT MY OWN SEAT: control ok, JIT case FAILED
0 vs 1. `beamr-jit-services.patch` (5 files, ~70 lines) proven on an isolated
0.16.2 copy: pure-beamr repro 1p/1f → **2p/0f**, beamr's own `cargo test --lib`
**1726 passed / 0 failed**, non-regressive.

🔴 **The pin is RED and stays red until beamr ships. It is deliberately NOT on
main** — a red test blocks every gate. Branch only.

**NO AION-SIDE DEFENCE EXISTS in 0.16.2.** Every lever checked: `jit` is a
MISLABELLED optional feature (`scheduler/mod.rs:214` gates the `use` on
`threads`, not `jit`) so it cannot be turned off; `SchedulerServices` offers no
JIT choice; `dirty_cpu_threads: Some(0)` is a non-starter because we register
five production dirty NIFs (`engine_nifs.rs:129-153`); and
`jit_threshold = u32::MAX` is a MITIGATION not a gate — the counter saturates
and still fires — as well as an arbitrary limit. Deliberately NOT applied.

Sent to **Artemis Peach** with the three riders they should own: the mislabelled
`jit` feature, `tune_threshold` (`jit/profiler.rs:203`) having no production
callers, and a dirty NIF below a JIT frame mapping `DirtyCall` to a bare deopt
that discards the partially-executed compiled frame.

🔴 **DO NOT RE-RUN THARSIS before the beamr fix lands.** It dies in the same
place and costs another hour.

**The gates did their job on the way past.** Decoded gate result for
`d1-project-foundations`: `gate scope-fence exited 1` listing `.github/
workflows/ci.yml`, `clippy.toml`, `rust-toolchain.toml`, `rustfmt.toml`,
`spikes/s*/run.sh`. That is the fence catching a real out-of-scope write — the
loop was working, and the item would have gone to the review panel and been
folded back as blocked. The engine died before the panel ran.

## THE RUN AS IT EXECUTED (historical)

Workflow `9da28cf9-5eaa-45b8-9b3d-eccc194dceed`, run
`8fed4511-401c-49dc-b2e7-44d3624dffdf`, started **07:10:44Z**, status `Running`.
Target `/Users/tom/Developer/ablative/dev-ops/tharsis` (which DOES gitignore
`target/`). 5 gates, 4 lenses, max_rounds 8, 15 pointers, 5863-char brief.

The planner landed at **07:25:51** after ~15 min of provider overload, and the
FAN-OUT FIRED: three phase-1 items, three real git worktrees under
`tharsis/.staged-rounds/`, three branches, three dev agents in parallel:
`d1-project-foundations`, `b1-m1-core-model`, `s1-lite-readonly-export`.

Persistent Monitor **bp7y2n4zk** watches step transitions / terminal status /
either process dying. Last event: `steps=2 last=fold_phase`.

DO NOT rebuild or restart the worker mid-run. The live binary predates today's
worker fixes; harmless because tharsis gitignores `target/`.

### Provider-overload finding (Tom challenged this and was right to)

Bare `norn` (nothing of ours in the path) hit the IDENTICAL
`server_is_overloaded` error ~10 times, then punched through in ~50s and wrote
`/tmp/asparagus.md`, exit 0. So the overload is real and independently
confirmed. But the asymmetry is unexplained-but-plausible, NOT verified: a small
request cleared in 50s while the planner's (full brief + 15 docs + big output
schema + a repo to read) took 15 min. Big requests are shed first.

## COMMITTED THIS SESSION (local on main, NOTHING PUSHED)

Everything below the divider that used to say "UNCOMMITTED" is now COMMITTED.
The sections are kept for their reasoning; ignore their staging language.

- `8416af54` docs(fleet) — earlier state + #80/#81.
- `d2b32d84` **#79 item-stage fix** — clippy exit 0, 176 passed / 0 / 0.
- `45fdaaee` docs(fleet) — the pre-compaction bank.
- `be0112bf` **feat(awl): send** — the whole signals lane + 8 aion-server Send
  arms + 3 console sites. aion-awl clippy 0 / **467 passed / 0 / 0**.
- `e78f4b44` **fix(server): #81 chmod** — 8 cases, 3 auto-repaired, 5 still
  refusing by design. clippy 0 / **658 passed / 0 / 0**.
- `93e961f6` **fix(awl): swallowed rollback** — my rider. clippy 0 /
  awl::documents **7 passed / 0 / 0**.
- `b355fc41` docs(fleet) — the #82 evidence chain.
- `5008a747` **fix(staged-rounds): verify_integration rider** — gated in the
  REAL tree this time: clippy 0 / **178 passed / 0 / 0** across 8 suites.
  #79 now CLOSED, both halves.
- `9e544085` + `eff9bd88` docs(fleet) — `THARSIS-RUN-1-POSTMORTEM.md`, what the
  three agents actually built, and the re-run checklist.

**Worker release binary REBUILT** (exit 0). The RUNNING worker pid 49833 still
executes the old code from its own inode and MUST be restarted before run #2.
`target/release/aion` is still MISSING — rebuild it after #82 lands, not before
(the fix changes `crates/aion`, so building now is wasted work).

**Canvas VERIFIED live, not assumed:** `/awl/documents` lists `staged_rounds.awl`;
`/awl/check` returns **0 diagnostics, 12 steps, 11 edges, 32 statement nodes**,
and the `execute` step projects the loop with all four nested parallel forks and
their join bindings in the right order. The server hands the canvas everything
it needs; I cannot verify the browser render from here.

**THREE AGENTS IN FLIGHT:** #82 NIF root-cause (`crates/aion/src/runtime/`,
already scaffolding `jit_nif_boundary_tests`), the worker-scaffold lane
(`crates/aion-package/src/codegen/awl_worker/` — 10 files, all under 500 lines,
and the 135-line `emit_declaration` that was failing workspace clippy is gone,
longest fn now 71), and #80 admission (`aion-server` + `aion-package`).

## UNCOMMITTED, IN THE TREE RIGHT NOW — needs one combined gate then commit

1. **verify_integration rider** (`examples/staged-rounds/worker/src/handlers/
   {verify,gate}.rs`, `tests/shell_gate_activities.rs`). Agent reported clippy
   exit 0 + **178 passed / 0 failed / 0 ignored**, measured in a disposable
   detached worktree because the tree did not build. I verified the BYTES (no
   bypasses, `worktree_status` genuinely deleted with no dangling refs, sizes
   fine) but have NOT gated it in the real tree. HOLD until the tree builds.
2. **AWL signal sender** (`crates/aion-awl/`, whole lane). VERIFIED AT MY SEAT:
   clippy `-p aion-awl` exit 0 zero diagnostics, **467 passed / 0 failed /
   0 ignored** across 42 suites.
3. **MY OWN aion-server + console work** (this is the bit that only exists in
   the working tree — see below).

### What I did by hand for the Send variant (8 Rust sites + 3 console sites)

`Statement::Send` / `StatementKind::Send` broke eight exhaustive matches in
`aion-server`. I did them properly, not mechanically:

- `awl/edit_rename.rs` — NEW `rename_send`: renames BOTH the payload args/value
  AND the target expr. Renaming one and not the other leaves a document that
  parses and no longer compiles.
- `awl/edit_studio.rs` `statement_uses_field` — send-aware over payload AND
  target. Answering `false` would let a rename report "nothing uses this field"
  while a send still names it.
- `awl/projection/build.rs` ×4 — `scan_statement_list` (no nested list),
  `collect_waits` (a send does NOT durably suspend), `collect_activities` (a
  send names a signal, never dispatched to a worker), `collect_child_calls` (a
  send starts no child). Each carries a comment saying WHY the no-op is correct.
- `awl/projection/edges.rs` — not a route edge; control continues, and there is
  no in-document node to draw to.
- `awl/projection/statement_build.rs` + `statement_types.rs` — NEW
  `ProjectionStatementKind::Send`, its own kind so a durable outward effect is
  visible on the canvas rather than collapsed into a neighbour.

**Then the console, which the agent did not flag and which would have broken
Tom's canvas:**
- `apps/aion-ops-console/src/features/authoring/lib/facade-semantic.ts` —
  `projectionStatementKinds` is a RUNTIME allow-list; a document containing a
  send would have thrown `Invalid authoring response` and killed the whole
  canvas. Added `'send'`.
- `.../lib/projection-types.ts` — added `'send'` to the union.
- `.../components/StatementTopology.tsx` — `Record<ProjectionStatementKind,
  string>` label map, caught by tsc exhaustiveness. Added `send: 'Send signal'`.

Verified: `cargo clippy -p aion-server --all-targets -- -D warnings` **exit 0**;
console `bun run typecheck` clean; `bun test` **580 pass / 0 fail** (92 files);
`bun run lint` 9 pre-existing warnings, 0 errors.

## #81 PERMISSIONS — LANDED IN THE TREE (uncommitted), Tom's repeat complaint

Root: `ConfinedDir::require_private_mode` (`filesystem.rs:183`) and
`validate_private_root` (`:525`) refused on `mode & 0o077` telling the operator
to `chmod 700` — while `harden_tree` (`:334`) ALREADY chmods the whole data
tree to 0700/0600. It just ran AFTER the refusal that made it unreachable.

Fix: `ConfinedDir::ensure_private_mode` tightens in place when the server's euid
owns the root, decided entirely against the ALREADY-OPEN descriptor (`fstat` via
`dir_metadata`, `fchmod` via `rustix::fs::fchmod`) so there is no second path
resolution and no TOCTOU window; the applied mode is re-read from the descriptor
because some filesystems accept a chmod and discard it. `ensure_private_home`
now goes through `open_or_create`, so `~/.aion` is CREATED owner-only if absent
(it previously returned `Ok` on NotFound and created nothing).
`filesystem.rs` split: `filesystem/ancestors.rs` holds the ancestor gate — a real
seam, roots we own and may repair vs ancestors we must never touch. 369 + 75 lines.

Still refusing, deliberately, each pinned: world-writable ancestor, foreign-owned
ancestor, macOS ACL granting a foreign principal, symlinked root (target's mode
explicitly left alone), foreign-owned root (its own message now, naming path /
mode / owner uid / server uid — the old one named a `chmod` that would have
failed for the operator too).

Gates: `cargo clippy -p aion-server --all-targets -- -D warnings` exit 0;
`cargo test -p aion-server` **658 passed / 0 failed / 0 ignored**; fmt clean.

`RELEASE-0.9.2.md` — I withdrew the "run `chmod 700 ~/.aion`, that's the privacy
gate doing its job" paragraph. That was the sentence Tom actually read.

## MY OWN RIDER ON TOP (uncommitted): the swallowed rollback

`awl/documents.rs:128` had `if let Ok(workspace) = ConfinedDir::open(root)` and
`let _ = workspace.remove_file(…)` — TWO swallowed errors in the rollback that
removes a document whose first revision failed to store. Cost of the swallow is
not a stray file: the orphan makes every retry of that create fail with `Exists`
for a document that was never created, with nothing anywhere saying why. It also
ran raw sync I/O inline on the async runtime while every other call in the module
goes through `blocking`.

Fixed: new `rollback_created_document` on the blocking pool, NotFound-tolerant
(nothing to undo is not a failure), and a new `DocumentError::CreateRollbackFailed
{ path, reason, rollback }` carrying BOTH causes — plus its own
`DocumentCreateRollbackFailed` HTTP error type, because a caller that cannot tell
it from a plain I/O error cannot explain it to the operator.

Three tests, all passing, incl. the one that pins the ACTUAL defect
(`a_rollback_that_cannot_remove_the_document_reports_it` — chmods the workspace
0500 so `remove_file` fails with PermissionDenied and asserts it propagates;
root-guarded at RUNTIME with a logged skip, never `#[ignore]`).
`awl::documents` **7 passed / 0 failed / 0 ignored**; `cargo clippy -p aion-server
--all-targets -- -D warnings` **exit 0, zero diagnostics**. 249 code lines.

## STILL OUT / BLOCKING THE COMBINED GATE

- **Scaffold agent** (`crates/aion-package/src/codegen/awl_worker/`,
  `aion-cli`) — still running. Its `declaration_rs.rs:168 emit_declaration` is
  135/100 lines and currently FAILS workspace clippy. Not fallout from the
  other lanes.
- **#82 NIF root-cause agent** (`crates/aion/`) — just dispatched. Disjoint from
  the `aion-server` work above.

**SEQUENCE WHEN THEY LAND:** run ONE combined gate over the whole workspace
(`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, console
typecheck+test), then commit the verify_integration rider, the signals lane, and
my Send-arm work together.

## Rulings made this session (recorded so they are not re-litigated)

- **#79**: one staging rule — always stage tracked mods; stage untracked only
  inside the item's `scope_in`; record every exclusion by name including on a
  skip; integration stages tracked-only. Empty `scope_in` refused at the fold
  boundary + planner prompt + schema `minItems: 1`.
- **verify_integration**: fix it the same way. Leaving two different answers to
  one question in one codebase is worse than either answer.
- **Send-arm no-ops**: each is a positive statement about what a send IS, and
  says so in a comment. A send does not suspend, is not an activity, is not a
  route edge, starts no child — and it IS its own projected kind.

## Signals: residue + Phase 2 (from the agent, worth keeping)

Surface built: header `send name: Type` declares outbound; statement
`send progress(item: x) to coordinator_id`. `send`/`to` are NOT reserved words
(the corpus already has `action send(to: String, …)`). Failed send faults the
step and reaches `on failure`. Gleam fixture compiles under real gleam 1.17.0;
MIR golden committed; `aion@signal:send/3` confirmed exported by the sdk-closure
beams.

Irreducible residue: a LIVE target whose document never waits that signal name
(silent — payload recorded, nothing consumes it), or waits it with an
incompatible payload (loud on the RECEIVER's side while the sender looks
healthy). Unresolvable target and terminal target are both LOUD. Also:
at-least-once delivery (`nif_signal.rs:367` routes then records), and an unused
outbound declaration is not flagged because the checker has no warning severity.

Phase 2 is NOT small: a spawn/child handle does not tell the compiler which
SIGNALS the child declares, and `ChildHandle` is opaque in
`gleam/aion_flow/src/aion/child.gleam` with NO id accessor — so it needs a
`gleam/aion_flow` change before AWL can lower a handle-addressed send at all.
Rule on the child-side signal surface FIRST.

Recommendation banked: add `outbound_signals` to `PackageContract` — today a
deployed contract does not state what a workflow SENDS.

## Open tasks: #80 (stale version makes queue unservable — needs a ruling),
#81 (chmod repeat complaint), #34 item 5 (declared terminal outcome reads as a
crash), #24, #30. Cally Ray briefed on #30.
