# Status truth delivery report

Verified code HEAD before adding this report: `8f0b796e5e9cd774c11da85e3c542028d3b6296d`. The containing report commit is the branch HEAD reported at delivery.

## R1 — live census: complete

`census.json` and `CENSUS.md` contain all 143 rows observed from the live `default` namespace using only the permitted read-only list/history POSTs. The run observed 80 Completed, 56 Failed, 5 Cancelled, and 2 Running; all 143 displayed statuses matched full history at this observation. The two named former F2 specimens are included and their full histories contain `WorkflowFailed` at sequences 604 and 605.

## R2 — mechanism audit: complete

`MECHANISM.md` records the list, describe, visibility-store, startup reconciliation, lifecycle projection, and mutable-cache paths with file:line evidence. It corrects the initial F2 premise: the banked histories stopped at the endpoint's 500-event default window. No writer of terminal status without a terminal event was proven. The genuine historical F1 evidence remains: stale durable visibility rows could survive until reconciliation.

## R3 — status truth fix: substantially complete

- `POST /workflows/list` now reconciles the durable visibility cache from full recorded history before filtering and serving rows.
- A terminal history cannot be served as Running; pinned by the stale-Running regression.
- A mid-flight history cannot be served as Failed merely because a divergent visibility row says Failed; the list response and stored cache are corrected to Running.
- No new status field or compatibility shim was added.
- The false-terminal/cache-correction test was added after the shared reconciliation fix rather than as a failing commit before it. This is a red-first ordering gap. The original demand for a literal proven F2 writer was based on truncated evidence and is not claimed as completed.
- Recovery's existing terminal recording path was not rewritten because the full-history evidence did not establish the alleged side-channel terminal writer.

## R4 — honest failure reason: complete

For opaque `workflow N returned error` terminal messages, projection now extracts the recorded structured detail message, removes the activity wrapper, and serves its first line. Non-opaque multiline terminal messages are likewise bounded to their first line. The server-list regression uses the known `the declared command `zsh` exited 1` error and is red-first (`b9287ae83` before `3ee6d09d9`). Full detail remains in event history.

## R5 — verification

Passed at final code state:

- `cargo fmt --all -- --check` (after formatting commit)
- `cargo clippy -p aion-core -p aion-server --all-targets -- -D warnings`
- focused server stale-Running/status+reason regression
- focused server false-terminal/cache-correction regression
- existing `aion-core` failure-projection tests were compiled as part of the server test dependency build
- clean worktree before this report was written

No task-specific supplied gate script was present at the named status-front paths, so no claim is made that an unseen gate script ran. A full workspace test was not completed in the available run window; targeted changed-crate lint and focused regressions passed.

## Commit ordering

1. `a3b1d82c6` docs(status): record live workflow census
2. `854a8f821` docs(status): explain visibility status mechanism
3. `6711fde16` test(status): pin stale running list projection
4. `4f705b66a` test(status): pin list endpoint projection
5. `8196c86dd` fix(status): reconcile list rows from history
6. `b9287ae83` test(status): pin honest list failure reason
7. `3ee6d09d9` fix(status): surface recorded terminal error
8. `d4b685b94` test(status): pin false terminal cache correction
9. `8f0b796e5` style(status): format projection changes

## Known gaps

- The false-terminal/divergent-cache test does not precede the reconciliation fix in history.
- No separate recovery mutation was made because the supposed F2 specimens were shown to contain terminal events beyond the truncated 500-event samples.
- Full workspace tests and an external/supplied status-front gate script were not run; only the verification listed above is claimed.

## BRIEF-STATUS-FIX2 addendum — stopped on fixture/oracle conflict

Diagnostic HEAD `e790ada74` reproduces the named solo red in 0.05s (`fix2-solo-red.log`, exit 101). The printed-key run in `fix2-key-diagnostic.log` proves the two raw visibility rows are not duplicate projections of the fixture:

- `(00000000-a10a-0000-0000-000000000004, 00000000-a10a-0000-0000-000000000005)` is the history-backed engine-internal `aion.schedule_coordinator` row seeded and reconciled during `shared_engine()` startup.
- `(00000000-0000-0000-0000-000000000001, 00000000-0000-0000-0000-00000000000a)` is the fixture row. Its planted `Failed` value is corrected in place to `Running` under the same key; Recorder did not mint a divergent run identity.

The HTTP list serves exactly one user row because internal workflows are explicitly filtered at the user-facing boundary. The final failing assertion instead counts the unfiltered shared store, where the repository contract requires internal and user executions to coexist. Both surviving keys have authoritative histories. Consequently, a correct stale-orphan removal pass would retain both and could not make the byte-identical oracle green.

Per R2's explicit stop branch, no mechanism or fixture mutation was made. The general mechanical fix remains an authoritative-key reconcile sweep plus a key-specific visibility removal seam; structural prevention is unavailable without redesigning the intentionally separate mutable projection. That fix is **not done** because this test does not plant an unaccounted key. No R4 battery-green claim is made; the mandatory test is still red. No presentation/supersession work, reason-extraction change, new refusal surface, live-server write, daemon, alternate target directory, or push was performed. Full evidence, the 4-call reconciliation census with positive control, and non-claims are in `FIX2-STOP-REPORT.md` and `fix2-writer-census.log`.

## BRIEF-STATUS-FIX2-AMEND delivery addendum

Amendment commit: `63141eef2518dbade920f0691f3860a0030a4205` (`test(status): scope raw-store oracle to fixture key`).

The final raw-store leg of `http_list_refuses_terminal_status_without_terminal_history` now selects the fixture row by `(workflow_id, run_id)`, fails loudly if that key is absent, and asserts that row remains `Running` with no failure reason or close time. The store-read comment states the shared-store population constraint from `internal_workflow.rs`: engine-internal executions such as the schedule coordinator legitimately coexist with the fixture. The HTTP assertions and every other test remain unchanged.

Diff-stat against `e53758842` is committed below; it proves no production source changed and that only the specified test file plus this delivery report moved:

```text
 crates/aion-server/src/api/http/workflows.rs  | 12 +++++++++---
 gate-logs/status-front-run/DELIVERY-REPORT.md | 14 ++++++++++++++
 2 files changed, 23 insertions(+), 3 deletions(-)
```
