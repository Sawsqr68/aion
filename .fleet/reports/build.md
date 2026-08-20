# Build report — `repo-battery-flight1`

## Result

Implemented the `repo_battery` workflow and `lane_repo_battery` worker under `workflows/repo-battery/`. The workflow validates waivers before provisioning, creates a run-owned detached tree at the caller-named ref, performs one declaration census, records venue/toolchain/hydration/environment state, invokes the subject tree's own bash battery and declaration verifier, assembles the ordered receipt, and surfaces root-guarded teardown on every route that owns a tree.

The delivery was built sequentially:

1. **Phase 1** — workflow, worker, committed operation scripts, and README: commit `3cda0b84`.
2. **Phase 2** — emitted worker surface, BEAM sidecar, and mechanical receipt-order/field-description assertion: commit `01dabdf2`.
3. **Phase 3** — built fixtures and harness (`917cd7dd`), separate red-evidence commits, positive controls, substitution arms, owed live runner, and final acceptance transcript. Independent review then found runtime-contract gaps; commit `2d5d1e8e` corrected parent-worker environment capture, per-leg environment order, verifier rc classification, partial-clone ownership, cleanup refusal routing, worker-faithful proof execution, and proof 8c's position. Commit `7483fb1b` made that venue-worktree proof repeatable.

Recorded commit ids above are witnesses to the coherent build steps; they attest the reviewed history and do not impose an expected-digest refusal.

## Receipt and scope

The `Receipt` fields are declared and emitted in the ruled 15-field order. `proofs/assert-schema.sh` mechanically checks the sidecar `required` array and the G2/R1/R2/R3 field descriptions. The receipt records:

- the requested repository/ref and frozen HEAD;
- the run-owned tree path;
- `gates.json`, battery, subject-instrument, and workflow-script sha256 witnesses;
- venue and effective declared environment in per-leg declaration order, using values for non-secret names and presence plus digest for secret-shaped names. Because the declared-command executor clears child environment except `PATH`, the scripts read the seated worker parent's initial environment and privately hand the actual runtime values to the battery;
- declared names from the provisioned tree, executed names from the unique `EXECUTED_LEGS` line, per-leg rc/status, verifier output, waivers, and verdict.

A red measured battery routes to workflow success; status answers “did I measure?” and the receipt answers “what did I measure?”. `unproven` alone routes to failure. Missing `gates.json` is the explicit success-status census result `undeclared`, which is not a verdict. A zero-leg or dead-selector result is a typed refusal naming the selector path.

## Proof matrix

| Proof | Local artifact | Result | Live receipt |
|---|---|---|---|
| 1 green positive control | `proofs/receipts/1-green.md` | **E**: green reachable through provision → precheck → battery → verify → receipt | **OWED**: `1-green-engine.md`; run `workflows/repo-battery/proofs/run-live-proof.sh 1 ENGINE_ENDPOINT` |
| 2 deliberate red | `proofs/receipts/2-red.md` | **E**: rc 1 carried as receipt verdict `red`; emitted success route asserted structurally | **OWED**: `2-red-engine.md`; run `.../run-live-proof.sh 2 ENGINE_ENDPOINT` |
| 3 suppressed declared leg | `proofs/receipts/3-unproven.md` | **E**: battery rc 0, verifier names missing leg, receipt `unproven`; failure route asserted structurally | **OWED**: `3-unproven-engine.md`; run `.../run-live-proof.sh 3 ENGINE_ENDPOINT` |
| 4 missing declaration | `proofs/receipts/4-undeclared.md` | **E**: typed `undeclared`; emitted success route asserted structurally | **OWED**: `4-undeclared-engine.md`; run `.../run-live-proof.sh 4 ENGINE_ENDPOINT` |
| 5 literal absolute path | `proofs/receipts/5-absolute-path.md` | **E**: typed `absolute_path` refusal before battery | none |
| 6 malformed waiver | `proofs/receipts/6-malformed-waiver.md` | **O**: runnable artifact committed; no fabricated execution | **OWED**: `6-malformed-waiver-engine.md`; run `.../run-live-proof.sh 6 ENGINE_ENDPOINT` |
| 7 mid-battery cancel | `proofs/receipts/7-cancel.md` | **E**: contained process group reports `CANCELLED MEASUREMENT`, no completed `EXECUTED_LEGS`, guarded teardown recorded | **OWED**: `7-cancel-engine.md`; run `.../run-live-proof.sh 7 ENGINE_ENDPOINT` |
| 8a tree source | `proofs/receipts/8a-tree-source.md` | **E**: divergent venue declaration ignored; census reports provisioned tree | none |
| 8b perturbations | `proofs/receipts/8b-perturbations.md` | **E**: teardown guard refuses outside path; dead selector returns typed `invalid_declaration`, not empty census | none |
| 8c worktree substitution | `proofs/receipts/8c-worktree.md` | **E**: the repo-battery implementation venue itself is an aion git worktree; top-level/common-dir divergence is recorded and the full positive control remains green | none |

Final red evidence was refreshed from the corrected scripts and committed separately: deliberate red `232cbd9f`, unproven `3577f08d`, undeclared refusal `85bd2afd`, absolute-path refusal `17c7486e`, cancellation `42ccb818`, and selector/guard perturbations `05295008`. The corrected venue-worktree evidence is `73a237a5`.

## Typed venue refusal

`venue_lacks_engine`: this box carries the `aion` binary and the full shell toolchain, but this run may start no listener. No workflow instance was created, no activity was dispatched, and no engine-side routing or cancellation was observed here. The landing-desk execution receipts named above are therefore OWED. `proofs/run-live-proof.sh` is the committed one-command runner against an existing deployed engine; it seats the committed worker and never starts a server.

## Acceptance

The final committed acceptance transcript is `workflows/repo-battery/proofs/receipts/acceptance.md`. It records:

- both `aion awl check` commands green;
- `aion worker awl --check` advertising `lane_repo_battery` and six actions;
- clean BEAM emission and the receipt schema assertion;
- `bash -n` across every operation and proof script;
- `.fleet/gate-entry.sh` green (workspace format, clippy, and tests).

`aion awl fmt` was not run.

## What this does not prove

- **No live dispatch occurred here.** The E+O and O engine receipts remain explicitly owed as named above.
- **Aion's own 24-leg battery cannot be green at this venue.** `gleam` and `uv` are absent, `ABLATIVE_ESTATE_ROOT` is unset, and the estate-root helper is absent. That is a venue fact, not a code verdict; the purpose-built green fixture is the positive control.
- **Declaration verification compares names only.** A leg named `rust:clippy` whose command is `true` passes `verify-declaration.sh`; this workflow inherits that exact limit and does not create another leg list.
- **The receipt witnesses and records; it does not refuse on a digest mismatch.** It tells a later reader which bytes and effective scope ran. Trust remains with the subject repository's review and the authority of the venue.
- No successive-receipt drift check, schedule, v1 replacement, or generalization beyond the first subject is included.

## Refusals

No implementation requirement is refused. The only outstanding executions are the explicitly permitted `venue_lacks_engine` live-engine receipts, named individually as OWED above with runnable commands.
