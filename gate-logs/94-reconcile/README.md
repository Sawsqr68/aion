# What the logs in this directory are

Four battery runs live here and **they are not four attempts at the same claim.**
Only the last is #94's verdict. Read this before citing any of them.

| log | HEAD | what it is |
|---|---|---|
| `battery-20260820.log` | pre-rebase | pre-block-28 runner, 23 legs, 7 FAIL — **none of them the code**. Cited by #54/#97/#98/#100. |
| `battery-20260820-rebased-ABORTED.log` | `40fd2958c` | stopped by me at 11 min. No verdict lines. |
| `battery-20260820-tests-pinned-SUPERSEDED.log` | `ba56311f6` | stopped by me at 3 min. No verdict lines. |
| **`battery-20260820-on-main.log`** | **`2ee53285d`** | **the gate.** Base `dfea2ec83`. |

Only the last commit still exists: the two stops were each followed by a change
to the branch, and the second by a rebase, so the first three HEADs above have
been rewritten away. They are recorded as history, not as things to check out.

## `battery-20260820.log` — SUPERSEDED. Not #94's verdict.

**23 legs, 7 FAIL.** It is kept because it is cited evidence, not because it
says anything about this fix. **None of its seven failures were the code.**

- It ran on the **pre-block-28 runner**: 23 hardcoded legs, no `estate-guard`,
  and a `gates:costly-announced` that fires FALSE. That runner no longer exists
  at the landing venue — block 28 replaced it (`bb1d54049`, landed
  `c7fa6adba`), and `gates:costly-announced` going green on the cured runner
  against byte-identical test source is the controlled proof that its red here
  was the grep anchor, not the suite.
- `rust:test` exit 101 was **aion#54's known shard-actor timeout flake, under
  contention I supplied myself** by running a `cargo check` while the battery
  was live. Base rate is **2/24 on base == 2/24 on branch** — it is never
  attributable to a branch. I self-reported the contention on that issue.
- `console:typecheck`, `console:test`, `ts-worker:proto-drift` (and the rest of
  that family) failed as *"cannot find module"* because this was a **fresh
  worktree with no `node_modules`** — the workspace is a bun workspace and
  `node_modules` is untracked at the repo root and under
  `apps/aion-ops-console/`. That reads as broken code and is not.

**Cited by:** aion#54 (base-rate evidence), #97, #98, #100. Do not delete it,
and do not read it as a verdict on `fix/94-reconcile-run-scope`.

## The two runs I stopped on purpose

**Both stops have the same reason**, and it is not that anything failed. A gate
attests a `(commit, parent)` pair. In each case the commit being measured was
about to stop being the one that lands, so the run's green could never have been
spent — finishing either would have bought nothing and cost hours.

### `battery-20260820-rebased-ABORTED.log` — stopped at 11 minutes

Reached leg 3 of 24, no verdict lines written. While it ran I found that
`EngineError::RunNotInHistory` — the error path the fix's own commit message
calls "the point" — had **no test at any of its three surfaces**: the raise site
in `Registry::reconcile`, and both `aion-server` mappings. Reverting the guard to
the pre-fix body passed the entire tree. Closing that gap changes the branch.

Kept because it carries evidence: `disk-trace.txt` beside it is this box's disk
and load trajectory under a **cold** `cargo test --workspace` — 112 → 84 GiB in
six minutes, load peaking at 60 on 10 cores. That is what sized the headroom for
the runs after it, and the 29 GiB `target/` the stop left behind is why they are
warm.

### `battery-20260820-tests-pinned-SUPERSEDED.log` — stopped at 3 minutes

Reached leg 3 of 24, no verdict lines written. #83 merged while it ran and main
moved from `c7fa6adba` to `dfea2ec83`, so the parent this run's attestation named
was no longer main's tip.

**The re-anchor was owed, not ceremonial.** The delta carries `convert.rs`,
`mcp/tools/list.rs`, and a **new integration test target**
(`crates/aion-server/tests/mcp_surface_e2e.rs`) in a crate this change also
touches — so `--all-targets` clippy and `cargo test --workspace` can both see the
difference. Measured before deciding, rather than waved through as unrelated.

## `battery-20260820-on-main.log` — **the gate**

Frozen HEAD **`2ee53285d`**, based on **`dfea2ec83`** (#83's merge). That commit
is what `FROZEN-HEAD.txt` names and what the log stamps itself, so the run names
its own input rather than relying on this file.

Run on the **block-28 runner** — `gates.json` is the one list, and the runner
executes exactly what it declares. Venue and trajectory beside it in
`VENUE-NOTES.md` and `disk-trace-run4.txt`.

Deps were installed with `bun install --frozen-lockfile` earlier and untouched
since — `node_modules` is untracked and neither stop cleared it — so the console
and ts-worker legs measure code rather than a missing toolchain. A future cut
should make that a declared rc-3 refusal naming the command, instead of five
failures that look like defects.

### Verdict — 24 declared, 24 executed, 24 PASS

Ended 2026-08-20 20:52:40 +1000, 48 minutes. HEAD was **unchanged** at
`2ee53285d` when the run ended, and the log stamps that sha nine times.

```
bash scripts/gates/verify-declaration.sh gate-logs/94-reconcile/battery-20260820-on-main.log
  declared 24 legs / executed 24 legs
  VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

That verifier says every declared leg **ran**; it does not say any leg tests what
its name suggests. The claims below were read off the log directly.

**The four tests this branch adds all RAN, in both test legs** — eight `... ok`
lines, read individually rather than trusting a `test result: ok` (a filter
matching nothing prints `ok` at rc 0):

| test | `rust:test` | `rust:test-all-features` |
|---|---|---|
| `registry::table::tests::reconcile_scopes_the_projection_to_the_named_run` | 3292 | 10709 |
| `registry::table::tests::reconcile_still_sees_the_successor_as_running` | 3293 | 10710 |
| `registry::table::tests::reconcile_refuses_a_run_absent_from_the_history` | 3291 | 10708 |
| `error::tests::run_not_in_history_maps_to_typed_backend_on_both_halves` | 5187 | 12603 |

**628 `test result:` lines, every one `0 failed`; no `FAILED` and no `panicked`
anywhere in 16,329 lines.**

#### Two accounted-for discrepancies, recorded because each has a failure mode that looks identical

**1. 630 `running N test(s)` headers against 628 result lines.** Two headers with
no result line is the crashed-test-binary signature — a binary that dies mid-run
emits no verdict. It is not that here. `run_server_outbox_e2e.rs` re-execs its
own binary as a child with a filter (`1 passed; 0 failed; … 3 filtered out`), and
the child's stdout interleaves with the parent's; both test legs do it once, which
is the delta of exactly 2. Decisive rather than plausible: a crashed child could
not hide, because the **parent test asserts on the child's outcome** and every
parent test passed. Announced 11,335 / passed 11,333 closes on the same 2.

⚠️ The first count was wrong for a reason worth keeping: `grep -c '^running
[0-9]* tests?*$'` is a **basic** regular expression, where `?` is a literal
character — so it demanded the plural and silently dropped every `running 1
test`. It reported 501 against 628, a far more alarming gap than the real one. A
malformed pattern fails into a plausible number, not an error.

**2. Two `SKIP (PROVES NOTHING)` lines**, one per test leg, inside
`run_server_outbox_e2e.rs`. The test **runs and passes**; the line announces the
one thing it does not establish (live reconciliation of a stale claim, which
needs a haematite concurrent seam with routed reads, or a server-side claim-aging
seam — aion#35, on Tom's docket). It is pre-existing and untouched by this
branch: the diff `dfea2ec83..2ee53285d` is five files, none of them that test.
It is recorded here so nobody later reads a declared omission as coverage.

#### `estate-guard` ran and passed — a prediction I withdrew before the result

I predicted this leg would refuse rc 3 for a missing `ABLATIVE_ESTATE_ROOT`. It
exited 0. The prediction came from grepping `scripts/` and `gates.json`, which do
not contain the export — `scripts/battery.sh` walks up for
`tools/gates/lib/estate-root.sh` and **sources** it, and that file exports the
variable. Withdrawn at 20:48, while the battery was still running, and no
standalone re-run is owed. Full account in `CORRECTIONS.md`.

---

**Naming note.** `battery-20260820-rebased.log` was renamed to
`…-rebased-ABORTED.log`, and `battery-20260820-tests-pinned.log` to
`…-tests-pinned-SUPERSEDED.log`, when each was stopped. Anything citing an old
name means the corresponding stopped run.

---

**Why this file exists.** The first log is a plausible-looking red with a full
evidence trail, sitting in the directory named for the fix it does not indict —
anyone finding it later would reasonably read seven failures as a verdict. The
two stopped runs have the opposite hazard: they are truncated, and a truncated
log with no verdict lines reads as a crash. A superseded artefact that is kept
must say what it is, beside itself; the alternative is deleting evidence three
issues cite, or leaving a deliberate decision looking like a failure.
