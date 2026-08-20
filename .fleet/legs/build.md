# Leg `build` — the `repo_battery` workflow, its receipt, and its proofs

**Run:** `repo-battery-flight1`. **This is the run's ONLY leg.** It owns the entire delivery.
**Tree:** aion at rig commit `856b2a863`, base = landed main `cdde313b3`.
**Plan:** `.fleet/plan.md` (read it; this brief is self-contained but the plan carries the full
citation ledger and the measurements).
**Gate entry:** `.fleet/gate-entry.sh` — already committed, **not yours to edit**.
**Your report:** `.fleet/reports/build.md` (create it; `.fleet/reports/capture.md` is a prior run's).

The brief's slices 1, 2 and 3 are **three sequential PHASES of this one leg**, in that order.
They are never parallel work items: **the proofs of phase 3 consume the workflow that phases 1 and
2 build.** Do not start phase 3 before phase 2 lands.

---

## 0. Read this first — four facts that will save you a day

1. **The design paper `docs/design/gate-battery-workflow-v2-20260818.md` is NOT in this repo**, at
   this tree or on any ref. Checked. `.fleet/BRIEF.md` rev 3 is the operative authority here, and it
   quotes or states every load-bearing section inline. **Do not invent a §-reading.** If a question
   turns on paper text the brief does not quote, it goes to **Artemis** (rulings ledger item 2 —
   she answers inside the hour, without reopening the grill), never to a guess.
2. **You may start no server and no listener.** No network services, no databases, no touching the
   live estate. See §6 for exactly which proofs that costs you and what you owe instead.
3. **Everything cited in this brief was re-verified at THIS tree**, because the brief was written
   against `c7fa6adba`. The same rule binds you: re-read any line before you lean on it.
4. **Vocabulary is binding: witness / record / attest. NEVER seal, NEVER pin.** Everywhere a
   recorded sha is described — in both documents, in the receipt's field docs, in the README, in
   your report. A seal REFUSES on mismatch; this **records** the sha256 and proceeds — it tells you
   afterwards which bytes ran. It *cannot* be a seal: the instrument lives inside the tree it
   measures, so there is no sha the operator can pin the subject repo against that the subject repo
   cannot also change. Pinning an expected sha in the operator's document is the two-lists problem
   wearing a different hat. (`gate_worker.awl:1-6` and `investigate_worker.awl:13` say "sealed" —
   pre-existing text, not yours to edit and not your precedent.)

---

## 1. What you are building

New folder `workflows/repo-battery/`:

| Path | What |
|---|---|
| `repo_battery.awl` | the workflow document — **ruled name `repo_battery`** |
| `repo_battery_worker.awl` | the worker document — **ruled queue `lane_repo_battery`** |
| `bin/*.sh` | the operation scripts |
| `README.md` | inputs, how to run, what a green does not prove |
| `proofs/` | fixtures, builder, harness, receipts |

**Flow:** `admit` → `provision` → `precheck` → `battery` → `verify` → `receipt` → `teardown`.

**FORBIDDEN — no edit, in any phase, for any reason:**
`workflows/gate/gate.awl`, `workflows/gates/gates.awl`, `workflows/gates/repo_gates.awl`.
Wave-1's closed leg vocabulary stays closed; v1 stays in service. F3 parallel-run
(`workflows/gates/README.md:300-311`) is the cut-over instrument, per repo, in a later lane.

**The workflow INVOKES the repo's committed scripts. It never reimplements them.**
`scripts/battery.sh` and `scripts/gates/verify-declaration.sh` are the instruments. §10.5's whole
indictment of the previous cut was: *"declared-versus-run is already enforced as a refusal — this
workflow simply never invoked it."* You are the invocation. You hold no leg list.

---

## 2. The substrate, measured at this tree — do not re-derive it

**S1 — `scripts/battery.sh`'s source closure is exactly four files.**
`battery.sh:66` sources `scripts/gates/lib.sh`; `lib.sh:243` sources
`scripts/battery-costly-announcements.sh`; the estate-root walk (`battery.sh:98-104`) is conditional
and finds nothing at this venue. Both `battery.sh:56` and `verify-declaration.sh:48` derive
`REPO_ROOT` from `BASH_SOURCE` — **script location, not git.** So a tree holding
`scripts/battery.sh`, `scripts/gates/lib.sh`, `scripts/gates/verify-declaration.sh`,
`scripts/battery-costly-announcements.sh` plus a `gates.json` is a runnable subject repo.
**Measured: it is.** A two-leg fixture built that way ran green (battery rc 0,
`EXECUTED_LEGS fixture:reads-tree fixture:true`) and verify-declaration returned
`VERDICT: exact match`, rc 0.

**S2 — `scripts/gates/lib.sh` is bash and must NEVER run under `sh`.**
`lib.sh:1` is `#!/usr/bin/env bash`; `lib.sh:91` uses process substitution
(`done < <(type -ap python3 2>/dev/null)`). Measured: `sh ./scripts/battery.sh` → **rc 2**, dying at
`battery.sh: 56: Bad substitution` (`${BASH_SOURCE[0]}`) then
`battery.sh: 66: .: cannot open //scripts/gates/lib.sh`. It never reaches the declaration read and
emits no `EXECUTED_LEGS`. **Rule: invoke these scripts so their bash shebang is honoured, or invoke
them as `bash <script>` explicitly. Never `sh <script>`.** Note that
`workflows/investigate/investigate_worker.awl:28` uses `sh <script>` — those are `#!/bin/sh`
scripts. Yours are not. Do not copy that half of the precedent.

**S3 — a suppressed leg leaves the battery GREEN; only verify-declaration catches it.**
Measured: `AION_BATTERY_ONLY="fixture:true" ./scripts/battery.sh` → **rc 0**, `0 of 1 gates failed`.
The same log through `verify-declaration.sh` → **rc 1**,
`🔴 DECLARED BUT NOT EXECUTED — the population shrank: fixture:reads-tree`.
This is your `unproven` discriminator, already committed in the subject repo. Proof 3's instrument
exists; your job is to invoke it and route on it.

**S4 — a dead selector and a real drift are INDISTINGUISHABLE at the instrument.**
Measured: `gates.json` with `"legs": []` → `verify-declaration.sh` exits **1**
("the declaration lists no legs"); a genuine declared-but-not-executed drift also exits **1**. Same
code, different worlds. **Therefore the discrimination MUST live in your census, as a typed refusal
naming the selector path — never inferred from an exit code.** This is R3, measured.

**S5 — `gates.json` declares environment as NAMES ONLY, and exactly one leg does.**
24 legs; leg 23 `estate-guard` carries `"environment": ["ABLATIVE_ESTATE_ROOT"]`; the other 23 carry
none. And: **no leg carries a literal absolute path.** `estate-guard`'s
`"$ABLATIVE_ESTATE_ROOT/tools/gates/hooks/check_must_be_claimed.sh"` is **variable-rooted, not
absolute**. A checker that reds it would red aion's own honest declaration on its first real run —
which is how a correct instrument gets switched off. See §5.4.

**S6 — the venue.** `aion` at `/home/aion/.cargo/bin/aion`; `cargo`/`rustc` per
`rust-toolchain.toml` (`1.97.1`, rustfmt + clippy); `bash`, `git`, `python3`, `jq` present; `bun`
and `node` present at the **transient** path `/tmp/bun-node-0d9b296af/`; **`gleam` ABSENT**, **`uv`
ABSENT**; `tools/gates/lib/estate-root.sh` absent anywhere on the walk to `/`;
`ABLATIVE_ESTATE_ROOT` unset; this tree is a plain clone, **not** a worktree.
**Consequence: aion's own 24-leg battery cannot go green here** (3 gleam legs + 2 python legs → 127
via `require_tool`, `lib.sh:40-52`; `estate-guard` → rc 3 by its own declared refusal). That is a
**venue fact, not a code verdict**, and it is not a defect to fix. Your positive control is a
purpose-built fixture, which is what the brief requires.

---

## 3. The AWL language surface — every construct below was prototyped and checks green

Prototyped in `/tmp/awlproto` against `/home/aion/.cargo/bin/aion`. **Use these; they work.**

- **L1 — per-field `///` docs are first-class.** `FieldDecl` carries `pub docs: Vec<DocLine>`
  (`crates/aion-awl/src/ast/document.rs:329-335`); the parser attaches them
  (`crates/aion-awl/src/parser/types.rs:188,235`) and names fields in its own diagnostic:
  "`///` doc lines attach to declarations (types, fields, actions, steps)"
  (`crates/aion-awl/src/parser/statements.rs:25`).
  **G2, R1 and R2 go in the receipt type's own FIELD docs — not in the `//!` header.**
- **L2 — an action may declare `timeout` with NO `retry` clause.** Checks green. And retry is
  declaration-owned: a call site "may pin `node` and `timeout` only — `retry` stays on the
  declaration" (`crates/aion-awl/src/checker/calls.rs:98-104`). **The no-retry law is expressed by
  omission and cannot be defeated at a call site.**
- **L3 — a typed refusal may route to SUCCESS.** `outcome undeclared: type Undeclared, route success`
  checks green and lowers to `Ok(UndeclaredOutcome(…))`.
- **L4 — string and empty-list literals are legal route arguments.**
  `route undeclared(verdict: "undeclared", …)` and `script_witnesses: []` both check green. AWL
  composes no strings at runtime, so the typed literal is a literal.
- **L5 — list-of-record inputs and fields check green:** `input waivers: [Waiver]`,
  `legs: [LegRecord]`, `script_witnesses: [ScriptWitness]`.
- **L6 — `sequence` can iterate an activity-bound list** (`sequence name in c.legs_declared`).
  Available. **You must not use it for the census** — see §5.3.
- **L7 — combinators are `filter`, `map`, `sort`, `count`, `any`, `all`**
  (`crates/aion-awl/src/ast/steps.rs:234-247`), and a comparison predicate is legal:
  `waivers |> any(.ruled_by == "") -> anonymous` checks green, as does
  `c.legs_declared |> count -> n` with `when n == 0`. **Proof 6's waiver refusal and R3's zero-leg
  refusal are both expressible in the workflow, at input, before anything is provisioned.**
- **L8 — the receipt's serialized field order IS its declaration order.** Measured with a
  deliberately non-alphabetical type: the emitter writes `receipt_to_json` as
  `json.object([#("zeta",…), #("alpha",…), #("mid_field",…)])`, declaration order. The beam
  sidecar's `required` array is likewise declaration order — the `properties` *object* is
  alphabetised, so **`required` is the order witness, not `properties`.** This is how R4 is checked.
- **L9 — `aion awl fmt` has NO `--check` mode** (its own help: "the printer is the single formatter,
  one rendering, so there is no `--check` mode"). It rewrites in place. **Do not run it.**
  `aion awl check` is the acceptance.
- **L10 — worker-document bodies admit no shell variables, so logic beyond one command lives in a
  script file.** The repo states it: `workflows/investigate/investigate_worker.awl:7-13`. This is the
  language constraint that grounds §5.2.

**Working skeleton, checked green** (`ok: … (2 steps)`), as your starting shape:

```awl
workflow probe_shape
  timeout 6h
  input subject_ref: String
  outcome reported:   type Receipt,    route success
  outcome undeclared: type Undeclared, route success
  outcome unmeasured: type Unmeasured, route failure

type Receipt {
  /// The head commit of the provisioned tree, WITNESSED at provision.
  head_commit: String,
  /// VERDICT INVERSION: a red battery is a SUCCESSFUL measurement.
  verdict: String,
}

worker battery_ops
  /// No retry clause at all — battery legs never retry.
  action provision(subject_ref: String) -> Probe
    timeout 30m
```

---

## 4. Phase 1 — the workflow: provision, invoke, receipt

**P1.1** Create `workflows/repo-battery/`. New document pair. **No edit** to `gate.awl`,
`gates.awl`, `repo_gates.awl`.

**P1.2** Names, ruled and not yours to vary: workflow `repo_battery`, queue `lane_repo_battery`.
The shape matches the existing `lane_gates` (`workflows/gates/worker/src/main.rs:32`).

**P1.3 The flow.**

- **`admit`** — input validation, **before anything is provisioned**. A waiver missing its author is
  refused here (L7). Provisions nothing, costs nothing.
- **`provision`** — clone/worktree the subject repo at the **caller-named ref** into a **RUN-OWNED
  tree** under the venue's workspace root — **never a person's checkout** (§9.5's measured reason).
  Refusals are data; a leftover destination **refuses** rather than being deleted —
  `workflows/investigate/bin/clone.sh:26` is the register, verbatim: *"a leftover tree from a
  collided dispatch, the dispatcher's to inspect"*.
- **`precheck`** — venue pre-checks **recorded, not assumed**. See §5.5.
- **`battery`** — ONE whole-cloth command: run **the tree's own** `scripts/battery.sh`,
  exit-zero-always, **rc + log path + contended flag as data** — the `gate_worker.awl:13-24` probe
  shape. **No retry** (L2).
- **`verify`** — run **the tree's own** `scripts/gates/verify-declaration.sh` against the battery
  log. **Its refusal IS the scope instrument** (S3). Never reimplemented.
- **`receipt`** — the terminal typed report (§5).
- **`teardown`** — **run-owned tree only**, root-guarded, with the guard in the **same script that
  does the `rm`, never a separate command**: `workflows/investigate/bin/teardown.sh:3-6` states the
  law — *"Deletion inherits every adjective of its justification"* — and `:14-28` implements it.
  Teardown obligations are **surfaced, not assumed**, on every route that owns a tree.

**P1.4 / P1.5 — invoke, never reimplement.** `scripts/battery.sh:42-45` is the reason, in its own
header: *"🔴 THE CURE IS SUBTRACTION, NOT A CHECKER. A drift-checker would be a third copy of the
same list."* `verify-declaration.sh:6-18` says the same of itself: it *"holds no list of its own"*.
Your workflow holds no list either.

**P1.6 — never `sh <script>`** (S2).

**P1.7 — exit-zero-always, refusal as typed data on every anticipated face.** `gate_worker.awl:13-16`
states why: *"a nonzero exit is a retryable-then-terminal activity failure and the workflow must
ROUTE on these faces."* Genuine breakage (unspawnable script, dead disk) may exit nonzero; a red
suite, a missing tree, an unreadable log may not.

**P1.8 — no retry on battery legs; the workflow timeout is the one duration.** No per-leg retry
ladder anywhere.

**P1.9 — no child workflows anywhere.** The cancellation contract abandons children. Every long leg
under **process-group containment**; a cancelled leg reports **CANCELLED MEASUREMENT**, never a
result.

**P1.10 / P1.11** — run-owned tree; root-guarded teardown; obligations surfaced.

**P1.12 — fmt is never a gate** (`gate.awl:40-42`, STANDING ORDER 9) — **and** aion's `gates.json`
declares `rust:fmt` (leg 0, `cargo fmt --all -- --check`). The workflow **runs what the repo
declares**: the repo's list is the contract. If the estate wants aion's fmt leg gone, that is a diff
to aion's `gates.json` under its own review, not a workflow opinion. **G4 is Vesper's to settle
separately — build neither around it nor waiting on it.** Do not add an fmt-suppression path;
Artemis's endorsement, not to be softened: *a suppressed leg is a second list one item long.*

**P1.13 — manual start only.** No cadence, no CI trigger, no schedule
(`workflows/gates/README.md:290-293`).

**P1.14 — the folder carries its own README**, and the `//!` header states what the workflow
measures **and what that does not prove** — `workflows/README.md:56-59`: *"In the `//!` header, not
only in the README. The document travels; the README does not."*

---

## 5. Phase 2 — the receipt attests its own scope, nothing wider

### 5.1 The receipt's fields, in exactly this order

**R4 is load-bearing NOW, at full strength.** L8 proves declaration order is serialization order, so
the order below **is** the wire order, every run:

1. `subject_repo` · 2. `ref_requested` · 3. `head_commit` · 4. `tree_path` ·
5. `gates_json_sha256` · 6. `battery_sha256` · 7. `script_witnesses` (`[ScriptWitness]`, sorted by
`path`) · 8. `venue` · 9. `environment` (`[EnvRecord]`, declaration order per leg) ·
10. `legs_declared` · 11. `legs_executed` · 12. `legs` (`[LegRecord]`: name, rc, status) ·
13. `verify_verdict` · 14. `waivers` · 15. `verdict`.

> **R4, verbatim and binding:** *this stable-shape-and-order requirement is LOAD-BEARING NOW, at full
> strength, even though the drift check that consumes it is out of scope: comparability cannot be
> retrofitted onto receipts already written, so this phase ships witnesses to the ordering discipline
> on the strength of a consumer that does not exist yet. **A builder must not read "drift check out
> of scope" as licence to drop the ordering.***

`legs_executed` comes from the battery's own `EXECUTED_LEGS` line (`scripts/battery.sh:282`), which
`verify-declaration.sh:102-119` requires to appear **exactly once** in the log.

### 5.2 Where the logic lives — RULED, do not re-open

**The mechanical logic goes in committed scripts under `workflows/repo-battery/bin/`, invoked by the
worker document's bodies.** Not an SDK crate (Vesper: a crate brings a `Cargo.lock` that rots — she
measured sixteen out-of-workspace worker projects pinning beamr 0.6.1 through 0.18.2 while the root
moved to 0.19.3; a document has no lock to rot). Not 400-character single-quoted `sh -c` bodies
carrying python either.

The reason is the language constraint at L10, stated in this repo:
`workflows/investigate/investigate_worker.awl:7-13`. `investigate_ops` is a live first-party workflow
built exactly this way. A committed script **is** reviewable text at a sha, and its sha256 is
**witnessed** into the receipt. This is named in `.fleet/plan.md` §4.2 as a planner's call, for the
landing desk to review — do not quietly change it either way.

### 5.3 ONE census site — this is R3's foundation

The provisioned tree's `gates.json` is read **once**, in **one** script, at **one** site:
`bin/census-declaration.sh`. **Do not** smear the selector across a worker body, a workflow pipe
chain and a step guard, and **do not** use `sequence` to iterate `legs_declared` even though L6
proves you could.

The reason is proof 8b: the selector-perturbation arm needs a **single nameable selector site with a
`file:line` address** to aim at. Three sites is three places to be right and no address to break.
And the battery does not need iteration: `scripts/battery.sh:226-239` runs the whole declared list
itself and reports per-leg rc in its manifest (`:264-274`).

### 5.4 The absolute-path admission rule, stated precisely

Refuse **at admission** a declared leg whose **command** contains a **literal absolute path** — a `/`
beginning a path token, not preceded by a word character or `$`. Keyed on the **PATH INSIDE THE
COMMAND, never the leg's name** (§10.2: ten of thirteen estate gate lists carry such a leg under
differing names).

**A variable-rooted path is NOT an absolute path.** `"$ABLATIVE_ESTATE_ROOT/tools/…"` is aion's own
leg 23 and is handled by the environment pre-check (§5.5), not by this rule. S5 measured this: a rule
that reds it would red aion's honest declaration on its first real run.

### 5.5 Venue pre-checks — recorded, not assumed

- The repo's `rust-toolchain.toml` pin vs the venue's matching toolchain, **with targets**.
- **PATH order** — rustup shim first.
- **Hydration**: the fresh-tree step the repo's own docs name (for aion:
  `bun install --frozen-lockfile`) — untracked `node_modules` otherwise reads as **five red legs that
  look like code failures**. **G5, ruled: this is a declared rc-3-style refusal on aion's side, NEVER
  an auto-install. An instrument must not mutate the venue it measures.**
- **Environment, at Artemis's strength — NOT "confirmed set".** For each name in a leg's declared
  `environment` block: **confirm and record the VALUE** for non-secret variables; **presence plus a
  digest** for anything secret-shaped. The reason is the correction's own: *a variable set to a
  DIFFERENT value than the leg's scope assumes is the silent scope change itself, and it passes a
  set-check.* The effective environment is **recorded in the receipt beside the frozen HEAD**.
  Reason to carry into the field docs: a gate's scope is set by its environment as much as by its
  declaration; the standing true receipt for the class is the estate's runtime env-gated tests — a
  leg whose tests read a `*_TEST_URL`-style variable and skip when unset measures a **different
  scope** with and without it, while reporting the same green. Receipts whose environments do not
  travel with their verdicts are not comparable: pass counts compare verdicts, not what was asked.

### 5.6 The four sentences that MUST appear in the receipt type's own FIELD docs

Not in the `//!` header. In the **field documentation** (L1) — because the reader who tidies this
away will be holding the field docs, not the design paper. These are machine-readable in the beam
sidecar as JSON-Schema `description` strings, and §7.4 asserts them there.

- **G2 — verdict inversion, on `verdict`, with its reason.** A red gate routes to workflow
  **SUCCESS**; `Failed` is reserved for **could-not-measure**; only `unproven` routes to failure.
  The reason, stated: **the status answers "did I measure?", the receipt answers "what did I
  measure?"** — because a red gate reporting SUCCESS is exactly the shape a later reader tidies away.
- **R1 — the authority sentence, on the fields that name a ref and a venue.** "No new trust" is true
  of the CODE and false of the VENUE: pre-existing, the self-gating hazard sat inside aion's own CI
  under aion's authority; this workflow moves it to a fleet venue that may hold credentials, network
  reach, and neighbours. State it plainly, in these words:
  **A BATTERY RUNS WITH THE VENUE'S AUTHORITY, AND NAMING A REF IS THEREFORE AN AUTHORITY-BEARING
  ACT.**
- **R2 — `undeclared`'s explicit status, in the same field docs.** Workflow status **SUCCESS** — the
  workflow measured the tree and found no contract; a **determinate census result**, not a
  could-not-measure. Receipt verdict is the typed literal **`undeclared`**, which consumers **MUST**
  treat as **not-a-verdict**: never green, never red, never pass-by-omission. State the reason: *a
  refusal with no assigned status resolves to the runtime's default, and a repo with no `gates.json`
  must never be able to read as a pass.* (Policy ruled by Waffles as the dispatch default,
  reversible at Tom's word: **no contract, no green** — a repository buys receipts by declaring its
  gates. Grill BACKED it, uncontested.)
- **R3 — the zero-leg refusal.** `legs_declared` resolving to zero legs → **typed refusal naming the
  selector path**, no verdict. State the reason: *a census returns empty when its selector is dead,
  and that is byte-identical to the thing being absent; zero legs is a value that would compare,
  render, and pass proofs intact.* S4 measured the collision at the instrument level.

Also in the receipt, by field:

- A declared leg that did not run → **`unproven`** — not red, not green. Only `unproven` routes the
  workflow to failure.
- **Waivers** are per-run, each naming **leg + reason + who ruled it**. A green-with-waivers receipt
  **lists its waived legs** — the denominator travels beside the verdict. A waiver missing its author
  is **refused at input validation** (§10.4 precedent).
- **`legs_declared` comes from the provisioned tree's `gates.json`, NEVER from the venue.** Proof 8
  sits on this invariant's path.

---

## 6. Phase 3 — red-first proofs, positive control load-bearing

Each proof is a committed receipt in the lane, under
`workflows/repo-battery/proofs/receipts/<n>-<name>.md`, holding the **verbatim** command and output.

### 6.1 The venue split — read this before you plan a single proof

You may start no server and no listener, so **no live dispatch happens at this venue**. Record that
as the typed refusal **`venue_lacks_engine`**: *this box carries the `aion` binary and the full shell
toolchain, but this run may start no listener, so no workflow instance is created, no activity is
dispatched, and no engine-side routing or cancellation is observed here.*

**That refusal licenses nothing.** It does not license a dropped proof, a faked receipt, or a
"we'll see at landing" shrug. Two rules:

1. **Everything executable here IS executed here**, and its receipt is committed now.
2. **What cannot be executed here ships as a committed, runnable artifact** — a proof script the
   landing desk runs against a live server with **one documented command** — with its
   `venue_lacks_engine` refusal recorded and its **execution receipt named as OWED** in
   `.fleet/reports/build.md`, per proof.

The split is better than it looks, because the `aion` CLI has two offline seams — **use both:**

- **`aion worker awl --check <doc>`** prints the worker's exact advertised surface as JSON: every
  action's name, input/output JSON Schema, and **the literal command string it would run** — and
  **exits 0 without connecting** (measured against `workflows/gate/gate_worker.awl`). Your proof
  harness reads the committed command out of that JSON and executes **those exact bytes** with
  parameters substituted. That is not a copy of the body; it **is** the body. Build the harness this
  way — do not hand-transcribe bodies into the harness.
- **`aion awl emit --target beam --output <out>`** compiles "through the SAME `aion_awl::compile`
  seam the server's `POST /awl/deploy` path uses" (its own `--help`) and writes `<out>.beam.json`
  carrying the derived `output_schema`. Per-field `///` docs travel there as `description` strings
  and `required` is in declaration order (L1, L8).

The emitted Gleam also lowers a success route to `Ok(<Outcome>(…))` and a failure route to
`Error(awl_error.AwlOutcomeFailure(…))`, so routing can be asserted **structurally** offline.
**Label a structural assertion as structural everywhere it appears. Never call it a receipt.**

### 6.2 The proof matrix

**E** = executed here, receipt committed now. **O** = owed at the landing desk.
**E+O** = measurement half here, engine-routing half owed.

| # | Proof | Class |
|---|---|---|
| 1 | **Green end-to-end** against a purpose-built positive-control fixture whose committed `gates.json` declares exactly what runs. **"The positive control is the load-bearing half"** — green must be REACHABLE through the new path. | **E+O** |
| 2 | A deliberate red leg → run completes, verdict red, finding carried, **status SUCCESS**. | **E+O** |
| 3 | verify-declaration refusal (a declared leg suppressed) → **`unproven`**. | **E+O** |
| 4 | Missing `gates.json` → **`undeclared`** refusal, no verdict, status SUCCESS. | **E+O** |
| 5 | Absolute-path leg → **refused at admission**. | **E** |
| 6 | Malformed waiver (no author) → **refused at input**. | **O** |
| 7 | Mid-battery cancel → **CANCELLED MEASUREMENT**, no `GateResult` from the killed leg, teardown obligations **surfaced not assumed**. | **E+O** |
| **8a** | **POSITIVE** (Vesper) — venue copy of `gates.json` differs from the provisioned tree's; **assert the receipt reports the TREE's.** *Does it read the right one?* | **E** |
| **8b** | **NEGATIVE** (Athena) + **R3 selector extension** (Artemis) — see below. *Does the assertion still run?* | **E** |
| **8c** | **SUBSTITUTION** — run the proof with the **VENUE ITSELF a git worktree**. *Is it right for the right reason, or agreeing by accident of layout?* | **E** |

Proof 3's instrument is measured and ready: `AION_BATTERY_ONLY` (`scripts/battery.sh:219-255`)
suppresses a declared leg, battery still exits **0**, verify-declaration exits **1** naming the
missing leg (S3). Proof 7's process-group containment is measurable at the shell level here even
though the engine cancellation is owed — measure what you can and say which half you measured.

### 6.3 Proof 8 — all three arms are REQUIRED and all three execute here

**8b, in full.** Two distinct perturbations, both required:
- **The guard.** Prove the guard still **REFUSES** a wrong input — *deleting a guard passes every
  positive arm identically.*
- **The selector (R3 extension, Artemis, required).** *Perturb the SELECTOR too, not only the guard:
  break the path by which `legs_declared` is read and require a **typed refusal, not an empty
  census**. A dead selector returns empty, byte-identical to absence; zero legs is a value that
  passes arms (a) and (c) intact. Confirmed by two independent instances of the failure mode measured
  on the estate the same day.* §5.3's one-site rule exists to give this arm an address. S4 measured
  the collision.

**8c, and why you will be tempted to drop it.** Artemis's endorsement is **not to be softened by a
builder**: arm (c) is *the one schedule pressure will try to drop* — keep all three, because
**redundancy pays only when copies differ in POSITION**. It is also the one arm that survives both
others passing.

**Its premise is measured at this tree, not cited.** Inside a git worktree:
`git rev-parse --show-toplevel` → the **worktree**; `git rev-parse --git-common-dir` → the
**original repo's** `.git`; a `dirname $0/..` script-derived root → the **worktree**. Path-derived
and repo-derived resolution genuinely diverge — repo-relative paths *"break in every worktree and
off-box clone, which are precisely the venues batteries run in"*.
Note: `scripts/battery.sh:56` and `verify-declaration.sh:48` are `BASH_SOURCE`-derived and therefore
**already worktree-correct**. The hazard is in **your** provisioning and census code. That is what
arm (c) aims at.

### 6.4 Fixtures — BUILT, never a committed second copy of the runner

A committed builder script creates each fixture repo from **its own committed `gates.json`** plus
copies of the four closure files (S1) taken **from the tree under test**, recording each source
sha256. **Committing a second `battery.sh` under `proofs/` would be a second runner that rots — the
exact defect this whole design exists to remove.**

Fixtures: `green` (positive control — legs that pass with bash + coreutils only, **never** cargo /
gleam / uv, per S6), `red` (one leg `false`), `absolute-path`, `no-declaration`, `zero-legs`.
Proof 3 reuses `green` with `AION_BATTERY_ONLY`; 8a reuses `green` with a divergent venue copy
planted; 8c reuses `green` provisioned as a worktree.

**Perturbations are applied to a scratch COPY at proof time, never to the committed tree**, and the
harness records the source sha of what it copied.

---

## 7. Acceptance — every item run, output committed

1. `aion awl check workflows/repo-battery/repo_battery.awl` → `ok:`, exit 0.
2. `aion awl check workflows/repo-battery/repo_battery_worker.awl` → `ok: … (worker
   \`lane_repo_battery\`, N actions)`, exit 0.
3. `aion worker awl --check workflows/repo-battery/repo_battery_worker.awl` → exit 0, JSON surface
   committed as evidence.
4. `aion awl emit --target beam` clean, **and a committed script asserts the sidecar**: the receipt's
   `required` array is the 15 fields **in the §5.1 order** (R4), and the **G2 / R1 / R2 / R3
   sentences are present as field `description`s** (L1). An assertion in code, not a reading.
5. `bash -n` clean on every `workflows/repo-battery/bin/*.sh` and every proofs script.
6. `sh .fleet/gate-entry.sh` green — fmt + workspace clippy + workspace tests, the venue scope it
   names in its own header. The full 24-leg battery is the landing desk's (S6).
7. Every **E** and **E+O** proof executed, verbatim output committed under
   `workflows/repo-battery/proofs/receipts/`, summarised in `.fleet/reports/build.md`.
8. Every **O** proof committed as a runnable artifact with its one-command invocation documented, its
   `venue_lacks_engine` refusal recorded, and its execution receipt **named as OWED** at the landing
   desk. **No faked receipt. No silently dropped proof.**
9. `aion awl fmt` **not** run (L9).
10. `.fleet/reports/build.md` states, in its own words, **what this does not prove** — at minimum:
    no live dispatch here; aion's 24-leg battery cannot be green at this venue; verify-declaration
    compares **NAMES** only (`verify-declaration.sh:33-36`: a leg named `rust:clippy` whose command
    is `true` passes it) and this workflow inherits that limit exactly; the receipt **witnesses**, it
    does not seal.

---

## 8. Out of scope — do not drift into any of these

The brief's own named list, restated. **None of it is this leg's work, in any phase, however tempting
it looks once the workflow exists:**

- **R3 repair loops** (aion#34 shape — its own brief after this lands).
- **Schedules** — manual start only; the v1 ruling stands.
- **Replacing v1 by count** — F3 parallel-run governs cut-over per repo, a later lane.
- **Any edit to wave-1 `gate.awl`'s closed vocabulary.**
- **Generalizing beyond the first subject repo** — aion is the first customer because block 28 just
  made its declaration trustworthy; beamr and the rest follow the same shape only after F3 says so.

Two more that will read as in-scope and are not:

- **The drift check across successive receipts** is the named follow-on, **out of scope** — but R4's
  ordering discipline that it consumes is **in scope at full strength now** (§5.1). Out-of-scope
  consumer, in-scope discipline. Do not collapse those.
- **G4, the fmt tension** — hers to settle separately. Build neither around it nor waiting on it
  (P1.12).

---

## 9. Hard walls — binding on every commit

1. **No lint suppressions in any spelling** — `#[allow]`, `#[expect]`, `#[deny]`, `_var` renames,
   `#[cfg(any())]` dead-code hiding. If clippy fires, **fix the code**.
2. **No `#[ignore]` on tests.** A test needing a runtime gates it **at runtime**: read an env var,
   emit a `tracing::info!` skip line, return `Ok(())`.
3. **No file over 500 lines of code** (excluding tests, comments, whitespace).
4. **`mod.rs` re-exports only.**
5. **`thiserror` for library errors**; `anyhow` only in the binary.
6. **Never `.unwrap()` or `.expect()` in library code.** Lock poison handled explicitly, mapped to a
   typed error.
7. **Red evidence as separate commits.** Each red arm's evidence lands as its own commit.
8. **Do not touch the live estate** — no network services, no databases. Env-gated tests skip where
   their env is absent.
9. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean before any
   commit.
10. **Witness / record / attest. Never seal, never pin.**

---

## 10. Where to ask

Builder questions on the settled design go to **Artemis** — answered inside the hour, without
reopening the grill (rulings ledger item 2). She holds no further gate. **Tom does not fire**; the
fork is closed. Landing is Vesper's hands after her chain (line 28 → #83 → #94). This build is
**additive and builds now in parallel; it does not ride her landing order.**
