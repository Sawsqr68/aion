# Fleet dispatch brief — the durable gate battery runs the repository's own declared legs

2026-08-20, Waffles (author), **revision 3** — carries Vesper's scope ruling
(topic ablative/stack/aion/gates, decision 6d7be40f, 09:03:46Z): **scope word
GIVEN** — and **Artemis's grill: COMPLETE, ACCEPT (10:46:01Z, grilled against
docs main f633179 = current text). The fork CLOSES; nothing goes to Tom.**
Her four required corrections are folded below at their sites (authority
sentence; `undeclared` status; selector-perturbation arm + zero-legs-is-a-
refusal; stable-shape-now). Build FULLY UNBLOCKED.
Venue for the build: fleet dev queue, clone of aion —
**additive, builds NOW in parallel; it does not ride Vesper's landing order.**
Landing: Vesper's hands after her chain (corrected order: line 28 → #83 →
#94 — #83 first, list_runs is dead estate-wide). **Ruled names: workflow
`repo_battery`, task queue `lane_repo_battery`** (matching `lane_gates`).
**Ruled venue shape: WORKER DOCUMENT, not an SDK crate** (paper Open Q1
closed): the ONE command stays reviewable text at a sha; `gate_worker.awl`'s
probe shape is the precedent; and a crate brings a `Cargo.lock` that rots —
Vesper measured sixteen out-of-workspace worker projects pinning beamr 0.6.1
through 0.18.2 while the root moved to 0.19.3. A document has no lock to rot.

## The settled design this implements

`docs/design/gate-battery-workflow-v2-20260818.md` (this repo), at r5. The
destination is §10.5's, verbatim: *"the venue running the house runner against
the repository's own committed config, with this workflow contributing only
what it genuinely adds: pinned provisioning, the completion proof, the
population census, and the durable receipt. **No second leg list anywhere in
the system.**"* The receipt scope law (§10.2) and validated-instrument scope
law (§10.3) are ruled estate-wide and bind every requirement below.

**Substrate (corrected rev 2, Vesper's cite):** aion block 28 is LANDED —
**main `c7fa6adba` (PR #102), change commit `bb1d54049`, gate log
`06bd154a9`; `gates.json` declares 24 legs there.** It makes the aion house
runner exactly the shape §10.5 wants: `gates.json` is the ONE list;
`scripts/battery.sh` executes exactly what it declares ("the cure is
subtraction, not a checker" — its own header); and
`scripts/gates/verify-declaration.sh` is the declared-vs-executed refusal
instrument that "holds no list of its own" (names + counts + exactly one
EXECUTED_LEGS line; its red path is runnable via `AION_BATTERY_SELF_TEST=1`).
Builder note: `scripts/gates/lib.sh` is bash — it uses process substitution
and must never be invoked or sourced under `sh` (measured 08:36Z; Vesper's
POSIX-source fix rides separately on `fix/gates-lib-posix-source`).
The workflow's job is to INVOKE that machinery durably, never to reimplement
any of it. §10.5's own indictment of the previous cut: "declared-versus-run is
already enforced as a refusal — this workflow simply never invoked it."

## The collision — RULED (Vesper, decision 6d7be40f): reconciliation accepted, vocabulary corrected

Wave-1 codification (`workflows/gate/gate.awl`) carries a CLOSED leg
vocabulary: whole-cloth commands, "a caller can never choose which program
runs", per Tom's 2026-08-18 order. §10.5's destination runs a script
committed in the subject repo. **Vesper accepts the reconciliation and
refuses only the word "sealed": recording a sha is a WITNESS, not a seal.**
A seal REFUSES on mismatch; what slice 2 does records the sha256 into the
receipt and proceeds — it tells you afterwards which bytes ran. And it
cannot be a seal: the instrument lives inside the tree it measures, so there
is no sha the operator can pin the subject repo against that the subject
repo cannot also change — pinning the expected sha in the operator's
document is the two-lists problem wearing a different hat. **Binding rule
for the builder: say witness / record / attest — never seal or pin —
wherever the receipt's shas are described, in this brief and in the
receipt's field docs.** Trust sits where it actually sits: the subject
repo's review and its landing gate. Why accepted anyway (her measurement,
this week): block 28's justification was `gates.json` declaring 7 legs while
`battery.sh` hardcoded 23, neither a superset — two lists drift silently. A
whole-cloth leg vocabulary in the operator's document recreates that one
layer up against 24 legs. The self-gating hazard is PRE-EXISTING (every aion
gate run already uses the subject branch's own runner), so no new trust is
asked for and the question does not go to Tom. **Artemis's required
correction (R1), folded verbatim in force: "no new trust" is true of the
CODE and false of the VENUE — pre-existing, the hazard sat inside aion's own
CI under aion's authority; this workflow moves it to a fleet venue that may
hold credentials, network reach, and neighbours. State plainly, here and in
the receipt's field docs: A BATTERY RUNS WITH THE VENUE'S AUTHORITY, AND
NAMING A REF IS THEREFORE AN AUTHORITY-BEARING ACT.** Artemis's acceptance
reasoning, for the record: the caller chooses a REF, not a COMMAND; a ref is
reviewable and a second list is not; no third option satisfies both orders. Addition, hers: keep the
recorded shas in a stable shape and place every run so they are COMPARABLE;
the drift check across successive receipts is the named follow-on, out of
scope for this slice. Artemis's grill remains outstanding: if she refuses
the reconciliation, the fork goes to Tom regardless.

## Slice 1 — the `repo_battery` workflow: provision, invoke, receipt

New workflow document pair (**ruled names: workflow `repo_battery`, queue
`lane_repo_battery`**) in aion `workflows/`, deliberately NOT an edit to
wave-1 `gate.awl`
(closed vocabulary stays closed) nor to `gates.awl`/`repo_gates.awl` (v1
stays in service; F3 parallel-run rule `workflows/gates/README.md:300-311` is
the cut-over instrument, per repo, later — not this slice).

Flow: `provision` (clone/worktree the subject repo at a caller-named ref into
a RUN-OWNED tree under the venue's workspace root — never a person's checkout,
§9.5's measured reason) → `battery` (one whole-cloth command: run the tree's
own `scripts/battery.sh`, exit-zero-always, rc + log path + contended flag as
data — the `gate_worker.awl` probe shape) → `verify` (run the tree's own
`scripts/gates/verify-declaration.sh` against the battery log; its refusal IS
the scope instrument) → `receipt` (typed report; see slice 2) → `teardown`
(run-owned tree only, root-guarded, the investigate `teardown.sh` precedent).

Requirements carried from the design with their teeth:
- **Verdict is data** (§9.3 ruling): red routes to SUCCESS; `Failed` is
  reserved for could-not-measure. Only `unproven` routes to failure.
  **G2 (required placement): state this inversion AND its reason in the
  receipt type's own field documentation** — the status answers "did I
  measure?", the receipt answers "what did I measure?" — because a red gate
  reporting SUCCESS is exactly the shape a later reader tidies away, and
  that reader will be holding the field docs, not the design paper.
- **No retry on battery legs; workflow timeout is the one duration** (§2).
- **No child workflows anywhere** (§5; cancellation contract abandons
  children). Every long leg under process-group containment; a cancelled leg
  reports CANCELLED MEASUREMENT, never a result.
- **fmt is never a gate** (STANDING ORDER 9) — note aion's `gates.json`
  declares `rust:fmt`; the workflow runs what the repo declares (the repo's
  list is the contract; if the estate wants aion's fmt leg gone, that is a
  diff to aion's `gates.json` under its own review, not a workflow opinion).

## Slice 2 — the receipt attests its own scope, nothing wider (§10.2 as code)

The terminal payload carries: `head_commit`, tree path, ref requested,
`gates_json_sha256`, `battery_sha256` (+ `scripts/gates/*` shas) — all
WITNESSED, never described as sealed (ruling above), in a stable shape and
order every run so successive receipts are comparable — **R4 (Artemis,
required): this stable-shape-and-order requirement is LOAD-BEARING NOW, at
full strength, even though the drift check that consumes it is out of
scope: comparability cannot be retrofitted onto receipts already written,
so slice 1 ships witnesses to the ordering discipline on the strength of a
consumer that does not exist yet. A builder must not read "drift check out
of scope" as licence to drop the ordering** —
`legs_declared` (from the provisioned tree's `gates.json`, NEVER from the
venue — proof 8 sits on this invariant's path),
`legs_executed` (from the battery's own EXECUTED_LEGS line), per-leg rc,
the verify-declaration verdict, and `waivers` (per-run, each naming leg +
reason + who ruled it; a green-with-waivers receipt lists its waived legs —
the denominator travels beside the verdict; a waiver missing its author is
refused at input validation, §10.4 precedent).
- A declared leg that did not run → `unproven` — not red, not green.
- **No absolute paths in a declared leg** — admission check at dispatch
  keyed on the PATH INSIDE THE COMMAND, never the leg's name (§10.2: ten of
  thirteen estate gate lists carry such a leg under differing names).
- A repo with no committed `gates.json` → typed refusal `undeclared`, named
  reason, no verdict. (§10.5 flagged this as policy; RULED here by Waffles as
  the dispatch default, reversible at Tom's word: no contract, no green — a
  repository buys receipts by declaring its gates. Grill BACKED it,
  uncontested.) **R2 (Artemis, required): `undeclared` carries an EXPLICIT
  status, documented in the same field docs G2 requires — RULED: workflow
  status SUCCESS (the workflow measured the tree and found no contract; a
  determinate census result, not a could-not-measure), receipt verdict the
  typed literal `undeclared`, which consumers MUST treat as not-a-verdict:
  never green, never red, never pass-by-omission. A refusal with no
  assigned status resolves to the runtime's default, and a repo with no
  `gates.json` must never be able to read as a pass.**
- **R3 (Artemis, required): a zero-leg declaration is a TYPED REFUSAL,
  never a value.** A census returns empty when its selector is dead, and
  that is byte-identical to the thing being absent; zero legs is a value
  that would compare, render, and pass proofs intact. `legs_declared`
  resolving to zero legs → typed refusal naming the selector path, no
  verdict.
- Venue pre-checks recorded, not assumed (§9.5): the repo's
  `rust-toolchain.toml` pin vs the venue's matching toolchain (with targets),
  PATH order (rustup shim first), and — Vesper's landing note, 2026-08-20 —
  the fresh-tree hydration step the repo's own docs name (for aion:
  `bun install --frozen-lockfile`; untracked `node_modules` otherwise reads
  as five red legs that look like code failures).
- **(Amendment 2026-08-20, rev: original worked example withdrawn by its
  author same hour — the runner self-resolves the variable via a sourced
  file her census's pathspec missed; the practice stands on its own
  merits.)** Every leg's declared environment block is confirmed SET in the
  environment about to fire, before dispatch — `gates.json` declares
  requirements precisely so this is one read, not detective work — and the
  effective environment is RECORDED in the receipt beside the frozen HEAD.
  **(Artemis refinement, folded: "confirmed set" is weaker than the
  reasoning — a variable set to a DIFFERENT value than the leg's scope
  assumes is the silent scope change itself, and passes a set-check.
  Confirm and record the VALUE for declared non-secret variables; presence
  plus a digest for anything secret-shaped — the pre-check as strong as
  the record.)**
  Reason: a gate's scope is set by its environment as much as by its
  declaration. Standing true receipt for the class: the estate's runtime
  env-gated tests (a leg whose tests read a `*_TEST_URL`-style variable and
  skip when unset measures a DIFFERENT scope with and without it, while
  reporting the same green). Receipts whose environments do not travel with
  their verdicts are not comparable — pass counts compare verdicts, not
  what was asked.

## Slice 3 — red-first proofs, positive control load-bearing (§10.4's law)

Prove before first real use, each as a committed receipt in the lane:
1. Green end-to-end against a purpose-built positive-control fixture whose
   committed `gates.json` declares exactly what runs (green must be REACHABLE
   through the new path — "the positive control is the load-bearing half").
2. A deliberate red leg → run completes, verdict red, finding carried,
   status SUCCESS.
3. verify-declaration refusal (a declared leg suppressed) → `unproven`.
4. Missing `gates.json` → `undeclared` refusal, no verdict.
5. Absolute-path leg → refused at admission, at authoring time.
6. Malformed waiver (no author) → refused at input.
7. Mid-battery cancel → CANCELLED MEASUREMENT, no `GateResult` from the
   killed leg, teardown obligations surfaced not assumed.
8. **(G1, REQUIRED by Vesper's ruling; three arms per the topic follow-ups,
   Cally 09:07Z)** — the never-from-the-venue invariant, whose violation is
   invisible in a green run. Three distinct discriminators, all required:
   (a) POSITIVE (Vesper): venue copy of `gates.json` differs from the
   provisioned tree's — assert the receipt reports the TREE's. Answers
   "does it read the right one".
   (b) NEGATIVE (Athena's arm): prove the guard still REFUSES a wrong
   input — deleting a guard passes every positive arm identically.
   Answers "does the assertion still run". **R3 extension (Artemis,
   required): perturb the SELECTOR too, not only the guard — break the
   path by which `legs_declared` is read and require a typed refusal, not
   an empty census. A dead selector returns empty, byte-identical to
   absence; zero legs is a value that passes arms (a) and (c) intact.
   Confirmed by two independent instances of the failure mode measured on
   the estate the same day.**
   (c) SUBSTITUTION: run the proof with the VENUE ITSELF a git worktree,
   where path-derived and repo-derived resolution stop agreeing
   (`--show-toplevel` names the worktree, refs live in the repo — measured
   at frame: repo-relative paths "break in every worktree and off-box
   clone, which are precisely the venues batteries run in"). Answers "is
   it right for the right reason, or agreeing by accident of layout" —
   the one arm that survives both others passing.

## Out of scope, named

R3 repair loops (aion#34 shape — its own brief after this lands); schedules
(manual start only, the v1 ruling stands); replacing v1 by count (F3
parallel-run governs cut-over per repo, a later lane); any edit to wave-1
`gate.awl`'s closed vocabulary; generalizing beyond the first subject repo
(aion is the first customer because block 28 just made its declaration
trustworthy — beamr and the rest follow the same shape only after F3 says so).

## Rulings ledger (rev 2)

1. **Vesper — RULED 09:03:46Z** (decision 6d7be40f, folded throughout):
   scope word GIVEN; witness-not-seal vocabulary REQUIRED; worker document
   over SDK crate; names `repo_battery`/`lane_repo_battery`; additive —
   builds now in parallel, lands after her chain (line 28 → #83 → #94);
   G1 = proof 8 required; G2 = inversion into field docs; G3 `undeclared`
   backed (reversible at Tom's word); G4 fmt tension is hers to settle
   separately — build neither around it nor waiting on it; G5 hydration
   becomes a declared rc-3 refusal on aion's side, never auto-install (an
   instrument must not mutate the venue it measures).
2. **Artemis — DISCHARGED 10:46:01Z: ACCEPT** (grilled at docs main
   f633179). Reconciliation accepted — the caller chooses a REF not a
   COMMAND, a ref is reviewable and a second list is not, no third option
   satisfies both orders. Four required corrections folded at their sites
   (R1 authority sentence; R2 `undeclared` status; R3 selector arm +
   zero-legs refusal; R4 stable-shape-now) + env-value refinement.
   Endorsements not to be softened by a builder: G4 fmt resolution (a
   suppressed leg = a second list one item long); proof-8 arm (c) is the
   one schedule pressure will try to drop — keep all three (redundancy
   pays only when copies differ in POSITION); witness-not-seal in
   Vesper's form. G3 `undeclared` BACKED uncontested. She holds no
   further gate; builder questions on the settled design go to her,
   answered inside the hour, without reopening the grill.
3. **Tom — DOES NOT FIRE** (item 2 accepted; fork closed).

Citations were read at aion `d8b53d08c`/`29361eae` (rev 1) and corrected to
the landed main **`c7fa6adba`** (rev 2, substrate section). The builder
re-verifies every citation at the landed main before relying on a line.
