# repo_battery — the plan

**Run:** `repo-battery-flight1`. **Rig commit:** `856b2a863`, base = landed main `cdde313b3`.
**Legs:** exactly ONE, id `build`, brief at `.fleet/legs/build.md`.
**Gate entry:** `.fleet/gate-entry.sh` (already committed at the rig commit; not the builder's to edit).
**Brief:** `.fleet/BRIEF.md`, revision 3 — rulings ledger settled, Vesper's scope ruling GIVEN,
Artemis's grill DISCHARGED/ACCEPT with corrections R1–R4 folded at their sites.

The brief's slices 1, 2 and 3 are **sequential PHASES of the one leg**, in that order, never
parallel work items: the proofs of phase 3 consume the workflow that phases 1 and 2 build.
`.fleet/legs/capture.md` and `.fleet/reports/capture.md` are a **prior run's** artifacts
(`fleet-hardening-1`, inherited through the rig base) and are not this run's work.

---

## 0. Citation re-verification — every brief citation, read at THIS tree

The brief's own rule binds this plan: it was written against `c7fa6adba` and main has moved to
`cdde313b3`. Every line below was re-read here before anything in this plan leaned on it.

| Brief claim | Verified at this tree | Verdict |
|---|---|---|
| `gates.json` is the ONE list, 24 legs | `gates.json`, 24 entries, keys `name`/`kind`/`cmd`/`why` | ✅ **24** |
| "the cure is subtraction, not a checker" — its own header | `scripts/battery.sh:42-45` | ✅ verbatim |
| battery executes exactly what `gates.json` declares | `scripts/battery.sh:169-179` (reader), `:226-239` (loop) | ✅ |
| exactly one `EXECUTED_LEGS` line | `scripts/battery.sh:282`; enforced at `scripts/gates/verify-declaration.sh:102-119` | ✅ |
| verify-declaration "holds no list of its own" | `scripts/gates/verify-declaration.sh:6-18, 93` | ✅ |
| its red path runs via `AION_BATTERY_SELF_TEST=1` | `scripts/battery.sh:196-199`; `verify-declaration.sh:38-44` | ✅ |
| `scripts/gates/lib.sh` is bash, process substitution | `scripts/gates/lib.sh:1` (`#!/usr/bin/env bash`), `:91` (`done < <(type -ap python3 …)`) | ✅ |
| wave-1 closed leg vocabulary in `gate.awl` | `workflows/gate/gate.awl:8-15` | ✅ |
| "verdict is data" already law in wave 1 | `workflows/gate/gate.awl:28-31` | ✅ |
| "fmt is never a gate" (STANDING ORDER 9) | `workflows/gate/gate.awl:40-42` | ✅ |
| aion's `gates.json` nevertheless declares `rust:fmt` | `gates.json` leg 0, `cmd: cargo fmt --all -- --check` | ✅ (G4 tension real) |
| `gate_worker.awl`'s probe shape is the precedent | `workflows/gate/gate_worker.awl:13-16` (exit-zero-always), `:18-24` (contention at source), `:36-58` | ✅ |
| F3 parallel-run rule at `workflows/gates/README.md:300-311` | `:300` is the heading, `:302-311` the rule | ✅ **exact** |
| manual start only, v1 ruling stands | `workflows/gates/README.md:290-293` | ✅ |
| the investigate `teardown.sh` root-guard precedent | `workflows/investigate/bin/teardown.sh:14-28` | ✅ |
| ruled queue name matches `lane_gates` | `workflows/gates/worker/src/main.rs:32` (`const TASK_QUEUE: &str = "lane_gates"`) | ✅ |
| **`docs/design/gate-battery-workflow-v2-20260818.md` "(this repo), at r5"** | **absent from the working tree AND from every ref** (`git log --all --` returns nothing) | ❌ **NOT PRESENT** |

### The one citation that does not verify, and what follows from it

**The design paper is not in this repository, at this tree or on any ref.** `find`, `git log --all`
and a `docs/design/` listing all agree. The brief's parenthetical "(this repo)" is wrong at
`cdde313b3`; the paper lives in the estate docs repo, which this venue does not carry — the same
place `workflows/investigate/investigate.awl:7-9` puts *its* design paper.

Consequence, stated rather than papered over: **§2, §5, §9.3, §9.5, §10.2, §10.3, §10.4 and §10.5
cannot be re-read here.** Their content is not lost, because `.fleet/BRIEF.md` quotes or states
every load-bearing one inline — §10.5 verbatim, the §10.2/§10.3 scope laws by name, §9.3's
verdict-is-data ruling, §9.5's run-owned-tree reason, §10.4's positive-control law. **Ruling: at
this venue `.fleet/BRIEF.md` rev 3 is the operative authority, and a section number is a pointer to
the brief's own restatement, never to text the builder is expected to fetch.** Nothing in this plan
or the leg brief depends on unquoted paper text. The builder must not invent a §-reading; if a
question turns on paper text the brief does not quote, it goes to Artemis (rulings ledger item 2 —
"answered inside the hour, without reopening the grill"), not to a guess.

---

## 1. The venue, measured

Everything here was measured on this box at plan time, not assumed.

| Fact | Measurement |
|---|---|
| `aion` binary | `/home/aion/.cargo/bin/aion`, built 2026-08-18 |
| `cargo` / `rustc` | `/home/aion/.cargo/bin/…`; `rust-toolchain.toml` pins `1.97.1` + rustfmt, clippy |
| `bash`, `git`, `python3`, `jq` | present (`/usr/bin/…`) |
| `bun`, `node` | present, at the **transient** path `/tmp/bun-node-0d9b296af/` |
| `gleam` | **ABSENT** |
| `uv` | **ABSENT** |
| `tools/gates/lib/estate-root.sh` | **ABSENT** anywhere on the walk from the repo root to `/` |
| `ABLATIVE_ESTATE_ROOT` | **unset** |
| this tree | a plain clone, **not** a git worktree (`--git-dir` == `--git-common-dir` == `.git`) |
| live aion server / seated worker | **none, and none may be started** (see §1.2) |

### 1.1 What the venue census means

aion's own 24-leg battery **cannot go green here** and never could: `gleam:*` (3 legs) and
`python:*` (2 legs) hit `require_tool` (`scripts/gates/lib.sh:40-52`) and return 127; `estate-guard`
returns rc 3 by its own declared refusal (`gates.json` leg 23) because the estate root is absent.
That is a **venue fact, not a code verdict** — which is exactly why `.fleet/gate-entry.sh` names its
scope in its own header and runs fmt + workspace clippy + workspace tests only.

This is also *why the positive control is buildable here anyway*: the brief already requires
"a purpose-built positive-control fixture whose committed `gates.json` declares exactly what runs".
It does not require aion's battery to be green at this venue, and it must not be read that way.

### 1.2 Venue honesty ruling — what phase 3 can and cannot EXECUTE here

Running the workflow end-to-end needs `aion server` (HTTP + gRPC + WebSocket listeners,
`aion server --help`) and a seated `aion worker awl`. This run is forbidden to touch the live
estate — no network services, no databases — so **no server is started at this venue and no live
dispatch happens here.**

**Typed refusal, recorded:** `venue_lacks_engine` — *this box carries the `aion` binary and the full
shell toolchain, but this run may start no listener, so no workflow instance is created, no activity
is dispatched, and no engine-side routing or cancellation is observed at this venue.*

That refusal does **not** license a dropped proof, a faked receipt, or a "we'll see at landing"
shrug. Two things follow, both binding:

1. **Everything that can be executed here, is** — and the receipt is committed now.
2. **What cannot be executed here ships as a committed, runnable artifact** — a proof script the
   landing desk runs against a live server with one command — and its **execution receipt is OWED
   at the landing desk**, named as owed, per proof, in `.fleet/reports/build.md`.

Measured, this splits far better than it first looks, because the `aion` CLI has two offline seams:

- **`aion worker awl --check <doc>`** prints the worker's exact advertised surface as JSON —
  every action's name, input/output JSON Schema, and **the literal command string it would run** —
  and **exits 0 without connecting** (measured against `workflows/gate/gate_worker.awl`). A proof
  harness can therefore read the committed command out of the document and execute **those exact
  bytes**, with parameters substituted, without any server. That is not a copy of the body; it is
  the body.
- **`aion awl emit --target beam`** compiles "through the SAME `aion_awl::compile` seam the server's
  `POST /awl/deploy` path uses" (its own `--help`) and writes a `<out>.beam.json` sidecar carrying
  the derived `output_schema`. Measured on a prototype: **per-field `///` docs travel into that
  sidecar as JSON-Schema `description` strings**, and the `required` array is in **declaration
  order**. So G2/R1/R2's field docs and R4's field order are *machine-assertable offline*.

What genuinely needs the engine is the **routing and cancellation** half: that a red battery lands on
a SUCCESS route, that `undeclared` lands on a SUCCESS route, that a mid-battery cancel yields
CANCELLED MEASUREMENT. Two of those three are also assertable *structurally* offline, from the
emitted Gleam, which lowers a success route to `Ok(<Outcome>(…))` and a failure route to
`Error(awl_error.AwlOutcomeFailure(…))` (measured on a prototype). A structural assertion is weaker
than an execution and is labelled as such wherever it is used — it is never called a receipt.

Full per-proof allocation is the matrix in §6.

---

## 2. Language findings — every construct prototyped against the real binary

Prototyped in `/tmp/awlproto` with `/home/aion/.cargo/bin/aion`, all checking green. These are
facts, not preferences, and the leg brief prescribes only constructs that appear here.

- **L1 — per-field `///` docs are first-class AWL.** `FieldDecl` carries `pub docs: Vec<DocLine>`
  (`crates/aion-awl/src/ast/document.rs:329-335`); the parser attaches them
  (`crates/aion-awl/src/parser/types.rs:188,235`) and names fields explicitly in its own diagnostic:
  "`///` doc lines attach to declarations (types, fields, actions, steps)"
  (`crates/aion-awl/src/parser/statements.rs:25`). **G2, R1 and R2 can therefore be satisfied
  literally, in the receipt type's own field documentation, as required — not in a header comment.**
- **L2 — an action may declare `timeout` with NO `retry` clause.** Checks green. Retry is
  declaration-owned, and a call site "may pin `node` and `timeout` only — `retry` stays on the
  declaration" (`crates/aion-awl/src/checker/calls.rs:98-104`). **The no-retry law is expressible by
  omission, and is not defeasible at a call site.**
- **L3 — a typed refusal may route to SUCCESS.** `outcome undeclared: type Undeclared, route success`
  checks green, and lowers to `Ok(UndeclaredOutcome(…))`. **R2 is directly expressible.**
- **L4 — string and empty-list literals are legal route arguments.** `route undeclared(verdict:
  "undeclared", …)` and `script_witnesses: []` both check green. **The typed not-a-verdict literal
  needs no runtime composition** (AWL composes no strings at runtime).
- **L5 — list-of-record inputs and list-of-record fields check green.** `input waivers: [Waiver]`,
  `legs: [LegRecord]`, `script_witnesses: [ScriptWitness]`.
- **L6 — `sequence` iterates an ACTIVITY-BOUND list, not only an input.** `sequence name in
  c.legs_declared` where `c` is an activity result checks green (5 steps). Available, but §4.3 rules
  it *out* for the census on purpose.
- **L7 — the combinator vocabulary is `filter`, `map`, `sort`, `count`, `any`, `all`**
  (`crates/aion-awl/src/ast/steps.rs:234-247`; `crates/aion-awl/src/checker/stages.rs:1-3`), and a
  comparison predicate is legal in one: `waivers |> any(.ruled_by == "") -> anonymous` checks green,
  as does `c.legs_declared |> count -> n` with `when n == 0`. **Proof 6's waiver refusal and R3's
  zero-leg refusal are both expressible in the workflow, at input, before anything is provisioned.**
- **L8 — the receipt's serialized field order IS its declaration order.** The emitter writes
  `receipt_to_json` as `json.object([#("zeta", …), #("alpha", …), #("mid_field", …)])` in
  declaration order, measured with a deliberately non-alphabetical type. The beam sidecar's
  `required` array is likewise in declaration order (the `properties` *object* is alphabetised, so
  `required` is the order witness, not `properties`). **R4 is satisfied by declaring the fields in
  the ruled order, and it is mechanically checkable.**
- **L9 — `aion awl fmt` has no `--check` mode.** Its own help says so: "the printer is the single
  formatter, one rendering, so there is no `--check` mode." It rewrites in place. **It must not be
  run as an acceptance step; `aion awl check` is the acceptance.**
- **L10 — worker-document bodies admit no shell variables, so logic beyond one command lives in a
  script file.** Not a preference — the stated language constraint and the standing first-party
  precedent: `workflows/investigate/investigate_worker.awl:7-13`, whose actions are
  `run json "sh /home/aion/fleet/bin/<op>.sh $params"` (`:28`), with the scripts' source of truth in
  that folder's `bin/`. See §4.2 for the ruling this grounds.

---

## 3. The substrate, measured — five things the builder must not re-derive

Measured here at this tree, with real runs, in `/tmp/fixproof`.

**S1 — the source closure of `scripts/battery.sh` is exactly four files.** `battery.sh:66` sources
`scripts/gates/lib.sh`; `lib.sh:243` sources `scripts/battery-costly-announcements.sh`; the
estate-root walk (`battery.sh:98-104`) is conditional and finds nothing here. Both `battery.sh:56`
and `verify-declaration.sh:48` derive `REPO_ROOT` from `BASH_SOURCE` — **script location, not git**.
So the closure is `scripts/battery.sh`, `scripts/gates/lib.sh`, `scripts/gates/verify-declaration.sh`,
`scripts/battery-costly-announcements.sh`, and a tree holding those four plus a `gates.json` is a
runnable subject repo. **Measured: it is.** A two-leg fixture built exactly that way ran green
(battery rc 0, `EXECUTED_LEGS fixture:reads-tree fixture:true`) and verify-declaration returned
`VERDICT: exact match`, rc 0.

**S2 — the `sh` hazard is real, loud, and early.** `sh ./scripts/battery.sh` → **rc 2**, dying at
`battery.sh: 56: Bad substitution` (`${BASH_SOURCE[0]}`) and then
`battery.sh: 66: .: cannot open //scripts/gates/lib.sh`. It never reaches the declaration read and
emits no `EXECUTED_LEGS`. The brief's rule is confirmed with a file and a line: **invoke these
scripts so their bash shebang is honoured, or invoke them as `bash <script>` — never `sh <script>`.**

**S3 — a suppressed leg leaves the battery GREEN and only verify-declaration catches it.** Measured:
`AION_BATTERY_ONLY="fixture:true" ./scripts/battery.sh` → **rc 0**, `0 of 1 gates failed`; the same
log through `verify-declaration.sh` → **rc 1**, `🔴 DECLARED BUT NOT EXECUTED — the population
shrank: fixture:reads-tree`. This is the `unproven` discriminator, and it is why §10.5's indictment
bites: *the battery's own exit code cannot see this.* Proof 3 has its instrument, already committed
in the subject repo, and the workflow's job is to **invoke** it.

**S4 — a dead selector and a real drift are INDISTINGUISHABLE at the instrument.** Measured: a
`gates.json` with `"legs": []` run through `verify-declaration.sh` exits **1** with "the declaration
lists no legs"; a genuine declared-but-not-executed drift also exits **1**. Same code, different
worlds. This is R3's collision, measured at this tree rather than argued — and it fixes *where* the
discrimination must live: **in the workflow's own census, as a typed refusal naming the selector
path, never inferred from an exit code.**

**S5 — `gates.json` declares environment as NAMES ONLY, and exactly one leg does.** Leg 23
`estate-guard` carries `"environment": ["ABLATIVE_ESTATE_ROOT"]`; the other 23 carry none. So
Artemis's refinement — *confirm and record the VALUE, not merely that it is set* — is satisfiable:
the name comes from the declaration, the value from the environment about to fire. Also measured:
**no leg in aion's `gates.json` carries a literal absolute path.** `estate-guard`'s
`"$ABLATIVE_ESTATE_ROOT/tools/gates/hooks/check_must_be_claimed.sh"` is **variable-rooted, not
absolute**, and a checker that reds it would red aion's own honest declaration. §4.5 states the rule
precisely so the builder does not over-refuse.

---

## 4. Design decisions, each ruled here so the builder does not re-open them

### 4.1 The documents

New folder `workflows/repo-battery/`, per `workflows/README.md:45` ("Each directory carries its own
README with the inputs it takes"):

| Path | What |
|---|---|
| `workflows/repo-battery/repo_battery.awl` | the workflow document — **ruled name `repo_battery`** |
| `workflows/repo-battery/repo_battery_worker.awl` | the worker document — **ruled queue `lane_repo_battery`** |
| `workflows/repo-battery/bin/*.sh` | the operation scripts (§4.2) |
| `workflows/repo-battery/README.md` | inputs, how to run, what a green does not prove |
| `workflows/repo-battery/proofs/` | phase-3 fixtures, harness, and receipts |

**No edit to `workflows/gate/gate.awl`, `workflows/gates/gates.awl`, or
`workflows/gates/repo_gates.awl`.** All three verified present and untouched at this tree. v1 stays
in service; F3 (`workflows/gates/README.md:300-311`) governs cut-over per repo, in a later lane.

### 4.2 Worker document + committed scripts — RULED, with its reason

**Ruling: the mechanical logic lives in committed scripts under `workflows/repo-battery/bin/`,
invoked by the worker document's bodies.** Not an SDK crate (Vesper's ruling: a crate brings a
`Cargo.lock` that rots — sixteen out-of-workspace worker projects pinning beamr 0.6.1 through 0.18.2
while the root moved to 0.19.3). Not 400-character single-quoted `sh -c` bodies carrying python
either.

The reason is a language constraint, not taste, and the repo states it:
`workflows/investigate/investigate_worker.awl:7-13` — *"worker-document run bodies admit no shell
variables (an AWL parameter is the only `$name` allowed), so any logic beyond one command lives in a
script file"*. `investigate_ops` is a live first-party workflow built exactly this way.

Three things make this consistent with Vesper's "the ONE command stays reviewable text at a sha":

1. A committed script **is** reviewable text at a sha, and its sha256 is witnessed into the receipt.
2. The whole-cloth / no-runtime-path law is `gate.awl`'s closed vocabulary (`gate.awl:8-15`), which
   this work is explicitly forbidden to edit and does not inherit.
3. This workflow *already* resolves a path at run time by design — the provisioned tree's own
   `scripts/battery.sh`. That is the reconciliation Vesper accepted and Artemis discharged. A rule
   the design has deliberately set aside at its centre cannot be reimposed at its edge.

**Named for the landing desk as a planner's call**, so it is reviewed rather than assumed.

### 4.3 The census is ONE activity, and that is deliberate

L6 proves the workflow *could* iterate `legs_declared` with `sequence`. It must not. The declaration
is read **once**, in one script, at one site: `bin/census-declaration.sh`.

Reason, and it is R3's: the selector-perturbation arm needs a **single nameable selector site** to
perturb. A selector smeared across a worker body, a workflow pipe chain and a step guard has three
places to be right and no address to break. One site, one `file:line`, one arm that can aim at it.

The battery does not need iteration anyway: `scripts/battery.sh` runs the whole declared list itself
(`:226-239`) and reports per-leg rc in its manifest (`:264-274`). The workflow's job is to invoke,
census, witness and report — never to re-run a list.

### 4.4 The flow

`admit` → `provision` → `precheck` → `battery` → `verify` → `receipt` → `teardown`.

- **`admit`** — input validation, before anything is provisioned: a waiver missing its author is
  refused here (`waivers |> any(.ruled_by == "")`, L7). Costs nothing; provisions nothing.
- **`provision`** — clone/worktree the subject repo at the caller-named ref into a **run-owned tree**
  under the venue's workspace root, never a person's checkout. Refusals are data, and a leftover
  destination refuses rather than being deleted — `workflows/investigate/bin/clone.sh:26` is the
  precedent, verbatim in register.
- **`precheck`** — venue pre-checks **recorded, not assumed**: toolchain pin vs venue toolchain (with
  targets), PATH order (rustup shim first), hydration state (G5: a declared refusal, **never an
  auto-install** — an instrument must not mutate the venue it measures), and the per-leg declared
  environment confirmed **by value** (§4.6).
- **`battery`** — ONE whole-cloth command: run **the tree's own** `scripts/battery.sh`,
  exit-zero-always, rc + log path + contended flag as data — the `gate_worker.awl:13-24` probe shape.
  **No retry** (L2).
- **`verify`** — run **the tree's own** `scripts/gates/verify-declaration.sh` against the battery
  log. Its refusal IS the scope instrument (S3). Never reimplemented; the workflow holds no list.
- **`receipt`** — the terminal typed report (§5).
- **`teardown`** — run-owned tree only, root-guarded in the same script that does the `rm`, never a
  separate command: `workflows/investigate/bin/teardown.sh:3-6,14-28`. Teardown obligations are
  **surfaced, not assumed**, on every route that owns a tree.

**No child workflows anywhere** — the cancellation contract abandons children. Every long leg under
process-group containment; a cancelled leg reports **CANCELLED MEASUREMENT**, never a result.
**The workflow timeout is the one duration**; no per-leg retry ladder.

### 4.5 The absolute-path admission rule, stated precisely

Refuse at admission a declared leg whose **command** contains a **literal absolute path** — a `/`
beginning a path token, not preceded by a word character or `$`. Keyed on the **path inside the
command**, never on the leg's name (§10.2: ten of thirteen estate gate lists carry such a leg under
differing names).

**A variable-rooted path is NOT an absolute path.** `"$ABLATIVE_ESTATE_ROOT/tools/…"` is a declared-
environment leg (aion's own leg 23) and is handled by the environment pre-check, not by this rule.
Measured (§S5): a rule that reds it would red aion's honest declaration on its first real run, which
is how a correct instrument gets switched off.

### 4.6 The environment pre-check, at Artemis's strength

Not "confirmed set". For each name in a leg's declared `environment` block: **confirm and record the
VALUE** for non-secret variables; **presence plus a digest** for anything secret-shaped. The reason
is the correction's own: a variable set to a *different* value than the leg's scope assumes is the
silent scope change itself, and it passes a set-check. The effective environment is **recorded in
the receipt beside the frozen HEAD**, because receipts whose environments do not travel with their
verdicts are not comparable — pass counts compare verdicts, not what was asked.

Standing true receipt for the class, carried into the field docs: a leg whose tests read a
`*_TEST_URL`-style variable and skip when unset measures a **different scope** with and without it,
while reporting the same green.

---

## 5. The receipt — R4 at full strength, now

The terminal payload's fields are declared in **exactly this order**, and the order is load-bearing
(L8: declaration order is the serialization order, every run):

1. `subject_repo` · 2. `ref_requested` · 3. `head_commit` · 4. `tree_path` ·
5. `gates_json_sha256` · 6. `battery_sha256` · 7. `script_witnesses` (`[ScriptWitness]`, sorted by
`path`) · 8. `venue` · 9. `environment` (`[EnvRecord]`, in declaration order per leg) ·
10. `legs_declared` · 11. `legs_executed` · 12. `legs` (`[LegRecord]`: name, rc, status) ·
13. `verify_verdict` · 14. `waivers` · 15. `verdict`.

**R4, verbatim and binding: this stable-shape-and-order requirement is LOAD-BEARING NOW, at full
strength, even though the drift check that consumes it is out of scope. Comparability cannot be
retrofitted onto receipts already written, so this phase ships witnesses to the ordering discipline
on the strength of a consumer that does not exist yet. A builder must not read "drift check out of
scope" as licence to drop the ordering.**

**Vocabulary, binding everywhere shas are described — in this plan, in both documents, in the field
docs, in the README, in the reports: witness / record / attest. NEVER seal, NEVER pin.** A seal
REFUSES on mismatch; this records the sha256 and proceeds — it tells you afterwards which bytes ran.
And it cannot be a seal: the instrument lives inside the tree it measures, so there is no sha the
operator can pin the subject repo against that the subject repo cannot also change. Pinning an
expected sha in the operator's document is the two-lists problem wearing a different hat. Trust sits
where it actually sits: the subject repo's review and its landing gate.
*(Note: `gate_worker.awl:1-6` and `investigate_worker.awl:13` say "sealed" — pre-existing text,
not this work's to edit and not this work's precedent.)*

### The three sentences that must appear in the receipt type's own field docs

Not in the `//!` header. In the **field documentation**, which L1 proves is a first-class AWL
construct and which the beam sidecar carries as JSON-Schema `description` (§1.2) — because the
reader who tidies this away will be holding the field docs, not the design paper.

- **G2 — verdict inversion, on `verdict`.** A red gate routes to workflow SUCCESS; `Failed` is
  reserved for could-not-measure; only `unproven` routes to failure. *With its reason:* the status
  answers "did I measure?", the receipt answers "what did I measure?" — and a red gate reporting
  SUCCESS is exactly the shape a later reader tidies away.
- **R1 — the authority sentence, on the fields that name a ref and a venue.** "No new trust" is true
  of the CODE and false of the VENUE: pre-existing, the self-gating hazard sat inside aion's own CI
  under aion's authority; this workflow moves it to a fleet venue that may hold credentials, network
  reach, and neighbours. Stated plainly: **A BATTERY RUNS WITH THE VENUE'S AUTHORITY, AND NAMING A
  REF IS THEREFORE AN AUTHORITY-BEARING ACT.**
- **R2 — `undeclared`'s explicit status, in the same field docs.** Workflow status **SUCCESS** (the
  workflow measured the tree and found no contract — a determinate census result, not a
  could-not-measure); receipt verdict the typed literal **`undeclared`**, which consumers **MUST**
  treat as **not-a-verdict**: never green, never red, never pass-by-omission. A refusal with no
  assigned status resolves to the runtime's default, and a repo with no `gates.json` must never be
  able to read as a pass.

Plus **R3**: `legs_declared` resolving to zero legs → **typed refusal naming the selector path**, no
verdict. A census returns empty when its selector is dead, and that is byte-identical to the thing
being absent; zero legs is a value that would compare, render, and pass proofs intact (S4 measured
this at the instrument level).

And: a declared leg that did not run → **`unproven`** — not red, not green. Waivers are per-run, each
naming leg + reason + who ruled it; a green-with-waivers receipt lists its waived legs, because the
denominator travels beside the verdict.

**`legs_declared` comes from the provisioned tree's `gates.json`, NEVER from the venue.** Proof 8
sits on this invariant's path.

---

## 6. Requirement → phase map, with the venue split

Phases are sequential. Nothing in phase 3 starts before phase 2 lands.

### Phase 1 — the workflow: provision, invoke, receipt

| # | Requirement | Anchor |
|---|---|---|
| P1.1 | `workflows/repo-battery/` created; new document pair; **no edit** to `gate.awl`/`gates.awl`/`repo_gates.awl` | brief §slice 1 |
| P1.2 | Names: workflow `repo_battery`, queue `lane_repo_battery` | ruled; shape matches `workflows/gates/worker/src/main.rs:32` |
| P1.3 | Flow `admit → provision → precheck → battery → verify → receipt → teardown` | §4.4 |
| P1.4 | Battery leg invokes **the tree's own** `scripts/battery.sh`; never reimplemented | `scripts/battery.sh:42-45` |
| P1.5 | Verify leg invokes **the tree's own** `scripts/gates/verify-declaration.sh`; never reimplemented | `verify-declaration.sh:6-18` |
| P1.6 | **Never `sh <script>`** — bash shebang honoured or `bash <script>` explicitly | S2, measured rc 2 |
| P1.7 | Exit-zero-always, refusal as typed data on every anticipated face | `gate_worker.awl:13-16` |
| P1.8 | **No retry on battery legs**; workflow timeout is the one duration | L2 |
| P1.9 | **No child workflows anywhere**; process-group containment; cancelled leg = CANCELLED MEASUREMENT | brief §slice 1 |
| P1.10 | Run-owned tree under the venue workspace root; never a person's checkout | `clone.sh:14-34` |
| P1.11 | Root-guarded teardown, guard in the same script as the `rm`; obligations surfaced | `teardown.sh:3-6,14-28` |
| P1.12 | **fmt is never a gate** — and the workflow runs what the repo declares, including aion's `rust:fmt`. G4 is Vesper's to settle separately: build neither around it nor waiting on it | `gate.awl:40-42`; `gates.json` leg 0 |
| P1.13 | Manual start only — no schedule, no cadence | `workflows/gates/README.md:290-293` |
| P1.14 | `README.md` for the folder; `//!` header states what it measures and what that does not prove | `workflows/README.md:45,56-59` |

### Phase 2 — the receipt attests its own scope, nothing wider

| # | Requirement | Anchor |
|---|---|---|
| P2.1 | **R4** — the 15 fields in the ruled order, at full strength now | §5, L8 |
| P2.2 | Shas **witnessed**; witness/record/attest vocabulary throughout; never seal, never pin | ruling |
| P2.3 | **G2** verdict inversion + reason in the `verdict` field's own docs | §5, L1 |
| P2.4 | **R1** authority sentence in the ref/venue field docs | §5, L1 |
| P2.5 | **R2** `undeclared` = workflow SUCCESS + typed not-a-verdict literal, documented in the same field docs | §5, L3, L4 |
| P2.6 | **R3** zero-leg census = typed refusal **naming the selector path**, no verdict | §5, S4, L7 |
| P2.7 | Declared-but-not-run → `unproven`; only `unproven` routes to workflow failure | brief §slice 2 |
| P2.8 | Absolute-path admission check keyed on the path **inside the command**; variable-rooted paths exempt | §4.5, S5 |
| P2.9 | Missing `gates.json` → typed refusal `undeclared`, named reason, no verdict | §5 |
| P2.10 | Waivers: leg + reason + **who ruled it**; anonymous waiver refused at input | L7 |
| P2.11 | Venue pre-checks recorded not assumed: toolchain pin + targets, PATH order, hydration | brief §slice 2 |
| P2.12 | **G5** hydration is a declared rc-3-style refusal, **never auto-install** | ruling 1(G5) |
| P2.13 | Environment confirmed and recorded **by value** (digest if secret-shaped), beside the frozen HEAD | §4.6, S5 |
| P2.14 | `legs_declared` from the **provisioned tree**, never the venue; ONE selector site | §4.3 |

### Phase 3 — red-first proofs, positive control load-bearing

Each proof is a committed receipt in the lane. **E** = executed at this venue, receipt committed now.
**O** = owed at the landing desk: committed runnable artifact, typed refusal `venue_lacks_engine`
recorded, execution receipt named as owed. **E+O** = the measurement half executes here, the engine
routing half is owed.

| # | Proof | Class | Instrument |
|---|---|---|---|
| 1 | Green end-to-end vs purpose-built positive-control fixture whose committed `gates.json` declares exactly what runs — **"the positive control is the load-bearing half"** | **E+O** | fixture built per S1; measured green here already |
| 2 | Deliberate red leg → run completes, verdict red, finding carried, **status SUCCESS** | **E+O** | red fixture; status half structural offline, executed at landing |
| 3 | verify-declaration refusal (a declared leg suppressed) → `unproven` | **E+O** | S3, measured: battery rc 0, verify rc 1 |
| 4 | Missing `gates.json` → `undeclared` refusal, no verdict, **SUCCESS** | **E+O** | no-declaration fixture |
| 5 | Absolute-path leg → refused at admission | **E** | admission is a body; no engine needed |
| 6 | Malformed waiver (no author) → refused at input | **O** | workflow-level (L7); structural offline, executed at landing |
| 7 | Mid-battery cancel → CANCELLED MEASUREMENT, no `GateResult` from the killed leg, teardown obligations surfaced not assumed | **E+O** | process-group containment measurable at shell level here; engine cancellation owed |
| **8a** | **POSITIVE** — venue copy of `gates.json` differs from the provisioned tree's; assert the receipt reports the **TREE's**. *Does it read the right one?* | **E** | plant a divergent `gates.json` at the venue |
| **8b** | **NEGATIVE + R3 selector extension** — prove the guard still **REFUSES** a wrong input (deleting a guard passes every positive arm identically), **and perturb the SELECTOR**: break the path by which `legs_declared` is read and require a **typed refusal, not an empty census**. *Does the assertion still run?* | **E** | one selector site (§4.3) gives the arm an address |
| **8c** | **SUBSTITUTION** — run the proof with the VENUE ITSELF a git worktree, where path-derived and repo-derived resolution stop agreeing. *Is it right for the right reason, or agreeing by accident of layout?* | **E** | measured here: see below |

**All three proof-8 arms execute at this venue.** That matters, because arm (c) is the one schedule
pressure will try to drop and Artemis's endorsement is not to be softened by a builder — *redundancy
pays only when copies differ in POSITION*. **Its premise is measured here, not cited:** inside a git
worktree, `git rev-parse --show-toplevel` → the worktree, `--git-common-dir` → the original repo's
`.git`, and a `dirname $0/..` script-derived root → the worktree. Path-derived and repo-derived
resolution genuinely diverge. (`scripts/battery.sh:56` and `verify-declaration.sh:48` are
`BASH_SOURCE`-derived and therefore worktree-correct; the hazard is in the *workflow's own*
provisioning and census code, which is exactly what arm (c) aims at.)

**Fixtures are BUILT, never committed as a second copy of the runner.** A committed builder script
creates each fixture repo from its own committed `gates.json` plus copies of the four closure files
(S1) taken **from the tree under test**, recording the source sha256 of each. Committing a second
`battery.sh` under `proofs/` would be a second runner that rots — the exact defect this whole design
exists to remove. Fixtures needed: `green` (positive control), `red`, `absolute-path`,
`no-declaration`, `zero-legs`; proof 3 reuses `green` with `AION_BATTERY_ONLY`, 8a reuses `green`
with a divergent venue copy, 8c reuses `green` provisioned as a worktree.

**Perturbations are applied to a scratch COPY at proof time, never to the committed tree**, and the
harness records the source sha of what it copied. Red evidence lands as **its own commit**, per the
hard walls.

---

## 7. Out of scope — the brief's own list, restated so the builder cannot drift into it

Copied from `.fleet/BRIEF.md` "Out of scope, named". **None of these is this leg's work, in any
phase, however tempting it looks once the workflow exists:**

- **R3 repair loops** (aion#34 shape — its own brief after this lands).
- **Schedules** — manual start only; the v1 ruling stands.
- **Replacing v1 by count** — F3 parallel-run governs cut-over per repo, a later lane.
- **Any edit to wave-1 `gate.awl`'s closed vocabulary.**
- **Generalizing beyond the first subject repo** — aion is the first customer because block 28 just
  made its declaration trustworthy; beamr and the rest follow the same shape only after F3 says so.

Two more, from the rulings ledger, that a builder will mistake for in-scope work:

- **The drift check across successive receipts** is the named follow-on and is **out of scope** —
  but R4's ordering discipline it consumes is **in scope at full strength now** (§5). Out-of-scope
  consumer, in-scope discipline. Do not collapse those.
- **G4, the fmt tension** — aion's `gates.json` declares `rust:fmt` while STANDING ORDER 9 says fmt
  is never a gate. **Hers to settle separately. Build neither around it nor waiting on it.** The
  workflow runs what the repo declares; if the estate wants aion's fmt leg gone, that is a diff to
  aion's `gates.json` under its own review, not a workflow opinion. Artemis's endorsement, not to be
  softened: a suppressed leg is a second list one item long.

---

## 8. Hard walls — binding on every commit of this leg

1. **No lint suppressions in any spelling** — `#[allow]`, `#[expect]`, `#[deny]`, `_var` renames,
   `#[cfg(any())]` dead-code hiding. If clippy fires, fix the code.
2. **No `#[ignore]` on tests.** A test needing a runtime gates it at runtime: read an env var, emit a
   `tracing::info!` skip line, return `Ok(())`.
3. **No file over 500 lines of code** (excluding tests, comments, whitespace).
4. **`mod.rs` re-exports only** — `pub mod` declarations and re-exports, logic in named files.
5. **`thiserror` for library errors**; `anyhow` only in the binary.
6. **Never `.unwrap()` or `.expect()` in library code.** Lock poison handled explicitly, mapped to a
   typed error.
7. **Red evidence as separate commits.**
8. **Do not touch the live estate** — no network services, no databases. Env-gated tests skip where
   their env is absent.
9. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean before any
   commit.

---

## 9. Acceptance

1. `aion awl check workflows/repo-battery/repo_battery.awl` → `ok:` , exit 0.
2. `aion awl check workflows/repo-battery/repo_battery_worker.awl` → `ok: … (worker
   \`lane_repo_battery\`, N actions)`, exit 0.
3. `aion worker awl --check workflows/repo-battery/repo_battery_worker.awl` → exit 0, and its JSON
   surface committed as evidence (the advertised contract and the literal command per action).
4. `aion awl emit --target beam` clean, and the sidecar asserted: the receipt's `required` array is
   the 15 fields **in the §5 order** (R4), and the G2/R1/R2 sentences are present as field
   `description`s (L1, §1.2). This assertion is a committed script, not a reading.
5. `bash -n` clean on every `workflows/repo-battery/bin/*.sh` and every proofs script.
6. `sh .fleet/gate-entry.sh` green (fmt + workspace clippy + workspace tests — the venue scope it
   names in its own header; the full 24-leg battery is the landing desk's).
7. Every **E** and **E+O** proof executed, its verbatim output committed under
   `workflows/repo-battery/proofs/receipts/`, and summarised in `.fleet/reports/build.md`.
8. Every **O** proof committed as a runnable artifact with its one-command invocation documented,
   its `venue_lacks_engine` refusal recorded, and its execution receipt **named as owed** at the
   landing desk. **No faked receipt. No silently dropped proof.**
9. `aion awl fmt` **not** run (L9 — no `--check` mode; it rewrites in place).

---

## 10. What this does not prove

- **No live dispatch happened at this venue** (§1.2). The workflow's engine-side behaviour — routing
  a red to SUCCESS, routing `undeclared` to SUCCESS, refusing an anonymous waiver at input, and
  CANCELLED MEASUREMENT under a real cancel — is proven **structurally** here and **executably** at
  the landing desk. Structural is labelled structural everywhere it appears; it is never called a
  receipt.
- **aion's own 24-leg battery has never been green at this venue and cannot be** (§1.1): `gleam` and
  `uv` are absent and the estate root is absent. The positive control is a purpose-built fixture, as
  the brief requires, and a green from it is a green of *that* declaration.
- **verify-declaration compares NAMES.** Its own header says so (`verify-declaration.sh:33-36`): a
  leg named `rust:clippy` whose command is `true` passes it. This workflow inherits that limit
  exactly and must not be described as closing it.
- **The receipt witnesses; it does not seal.** It tells you afterwards which bytes ran. It cannot
  refuse on mismatch, and the reason is structural, not an omission (§5).
- **The design paper could not be re-read** (§0). The brief's restatement is the operative authority
  at this venue; that is a recorded venue fact, not a silently accepted citation.
