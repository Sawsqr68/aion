# `fleet_chain` — the staged-flight chain coordinator

Sequential-legs **Resolution D**, from `docs/design/sequential-legs.md`.

An **opt-in** chain runs whole `fleet_dev` flights one stage at a time from
a validated plan block. A sealed verifier reads one declared conclusion
token from each stage's **exact pushed commit** before anything is
accepted, held, admitted, or skipped. **Existing `fleet_dev` is untouched.**

## What is here

| Path | Role |
|---|---|
| `fleet_chain.awl` | the coordinator — a NEW document |
| `fleet_chain_worker.awl` | the chain-specific worker — a NEW document |
| `bin/fleet-chain-lib.sh` | shared guards, EXIT-ZERO-ALWAYS |
| `bin/fleet-chain-plan-check.sh` | boundary 1 — parse complete |
| `bin/fleet-chain-{dispatch,await,verify,accept,admit}.sh` | boundaries 2–6, one recorded fact each |

The decision logic is **not** in these scripts. It lives in
`crates/aion-fleet-plan`, and the scripts only carry external results
across the seam.

## The plan grammar

The planning counsel commits exactly one block in `.fleet/plan.md`:

```fleet-stages-v1
stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto
stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto
stage id=l3 legs=l3-route after=l2 admit=l1.conclusion!=measured-red conclusions=route-ready conclusion_leg=l3-route advance=auto
```

One physical line per stage, keys in the displayed order, ASCII
identifiers, no quoting, no spaces inside comma-separated values, `-` for
no predecessor. The v1 graph is a **single chain**: exactly one root, at
most one predecessor and successor per stage, exactly one terminal.
Parallelism lives **inside** a stage; ordering is **between** flights.

`conclusion_leg` designates the one leg responsible for committing exactly
one declared token, followed by one LF and no other bytes, at
`.fleet/conclusions/<stage-id>`.

## The ordering authority

The **normalized typed graph** returned by a successful `check_plan` — not
any later parse of the working tree — orders the run.

## The exit contract

| Exit | Meaning |
|---|---|
| 0 | plan, briefs and stage chain valid (or: no block, and the legacy entry point owns this plan) |
| 1 | plan or required committed artifact absent/incomplete |
| 3 | committed briefs disagree with dispatch-fixed leg ids |
| 4 | `PLAN GRAPH INVALID`, with a stable cause code |

Exit 4 maps to the **new** `plan_graph_invalid` outcome carrying
`StagePlanRefused`. **It does not widen `plan_mismatch`.**

**A plan with no block reaches the pre-existing code path byte-identically.**
Block validation and exit 4 are reachable only when a block is present at
HEAD. Proven old-script-versus-new-script in
`docs/evidence/r7-legacy-path/`.

## The stage-id charset

`check_plan` refuses any stage or leg id outside `^[a-z0-9][a-z0-9_-]*$`.
`<stage-id>` interpolates into `.fleet/conclusions/<stage-id>`, so
traversal is closed **by construction**, not by filtering.

**This is a derived tightening the design does not yet carry.** §5.2
constrains stage ids only to "unique, nonempty". It is consistent with
§2.1's "ASCII identifiers" and with every design exemplar, and it is
flagged to the design's owner as an addition the design should absorb.

It composes with the existing dispatch guard rather than replacing it:
`csv_guard` (`workflows/fleet-dev/bin/fleet-lib.sh`) already refuses any
`leg_ids_csv` outside `[A-Za-z0-9,-]`, so `_` is refused EARLIER by that
guard while uppercase and a leading `-` are refused here. The stricter
wins per id, and `fleet-lib.sh` is not modified.

## The seven recorded boundaries

Each is one bodiless worker action, so each is a separately recorded
activity result and replay consumes the record:

1. parse complete — `chain_plan_check`
2. child dispatched — `chain_dispatch`
3. child complete — `chain_await`
4. conclusion verified — `chain_verify`
5. acceptance recorded — `chain_accept`
6. skip recorded — `chain_admit`
7. next child dispatched — the next loop iteration

Ordering is fixed: **pushed receipt → verify → accept/hold → admit.** No
hold can approve a stage whose conclusion has not verified.

## The base law

The root is cut from the exact commit resolved from the chain's authored
`base_ref` at chain start. Every leg in a stage is cut from that same exact
stage-base commit. A successor's base is the recorded **accepted result
SHA** — never a mutable branch name, never a fresh `git rev-parse`. A
skipped stage creates no base.

## What the coordinator never does

Resolve a live ref, read a worktree, `git rev-parse` a name, grep prose for
a conclusion, consult time or worker availability, or read the deployed
module to recover a prior decision. Design §6 names each of those a
violation.

## Provisioning

The scripts expect `$FLEET_HOME` (default `/home/aion/fleet`) with
`bin/aion-fleet-plan` and `bin/aion-fleet-chain-step` installed and
executable, and these two bound configuration values exported — they are
**refused, never defaulted**:

* `AION_FLEET_CHAIN_DEADLINE_SECS` — the barrier's total deadline
* `AION_FLEET_CHAIN_POLL_MILLIS` — the barrier's poll interval

A barrier wait that reaches its deadline exits **75**, a distinct timeout
outcome — never a success and never a silent retry.

## Not in scope, by the design's own ruling

No change to `sequence`/`distribute` lowering. No `FleetDevReport` field.
No multi-token judge admission. No arbitrary DAG or multi-parent stage
graph (§3, §10). No landing-policy change — **the landing re-gate stays at
the operator's desk** (§8). Chain promotion accepts a next-stage base; it
is not a landing to main.
