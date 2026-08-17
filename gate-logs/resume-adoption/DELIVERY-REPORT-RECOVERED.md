# Delivery report — recovered from norn session ad933f99 (builder pid 10147)
# last assistant message at 2026-08-17T07:15:27.276282Z (builder exit 07:15:34Z)

# Delivery report — **INCOMPLETE / NOT DISPATCH-READY**

I did **not** complete the approved seven-leg acceptance package. The branch contains 11 committed red/green changes and is clean except for the pre-existing Norn execution marker, but several mandatory full-path e2e proofs remain absent.

I am explicitly **not claiming the lane is ready to land**.

## Repository state

- Worktree: `/Users/tom/Developer/ablative/stack/aion/.worktrees/resume-adoption`
- Branch: `resume-adoption`
- Base: `168baa5f4` (`v0.17.0`)
- Branch: 11 commits ahead of `origin/main`
- Pushes/merges: **none**
- Live estate, `~/.aion`, `aion-data/`, live workers, and `.worktrees/libsql-retirement`: **not touched**
- Existing untracked `.aion-held-execution.json`: **left untouched**; it identifies this Norn session.
- Isolated Aion server/AION_HOME/ports: **none allocated**, because the required server-level e2e suite was not reached.

## Commit trail

```text
bbce645ac fix(36): resume waits for recovered readiness without false timeout
e350538ad test(36): recovered resume has no false-error deadline — red
2e03375ae fix(36): liminal redeliveries join the live agent outcome
28bca0388 test(36): specify same-execution single-flight coordination
0724c75fe test(36): agent redelivery single-flights by execution — red
d02da35a2 fix(36): startup adopts dangling activities from paused histories
6540c7c29 test(36): paused dangling activities enter adoption — red
85755d737 fix(36): spawn gate adopts its own live execution
74c249d8c test(36): self occupancy adopts, foreign occupancy refuses — red
b73246ddc fix(36): exhaustion records name the spent budget
836ef5a88 test(36): exhaustion record must name exhaustion — red
```

## File-by-file changes

### `crates/aion-integration-norn/src/spawn_guard.rs`

- Compares the existing marker’s `(workflow_id, activity_id)` with the incoming `AgentRunSpec`.
- A live foreign holder remains an `Occupied` refusal.
- A live holder with the same workflow/activity returns an adoption claim and allows the configured Norn reattach invocation to proceed.
- The existing marker remains intact if the replacement spawn fails; successful binding atomically replaces it with the new child incarnation.

### `crates/aion-integration-norn/src/harness.rs`

- Converted the old collision fixture into a true **foreign-workflow negative control**.
- Added a self-occupancy adoption test.
- The test proves the self path enters a replacement invocation rather than returning `Occupied`.
- It does **not** prove Norn session-ID continuity or that a real orphaned Norn session was resumed.

### `crates/aion-worker/src/runtime/agent_flights.rs`

- Added a concurrent single-flight registry keyed by workflow ID and activity ordinal.
- First delivery becomes leader.
- Same-execution redeliveries become followers.
- A different activity remains an independent leader.
- Leader completion drains the followers.

### `crates/aion-worker/src/runtime/liminal.rs`

- Wired `AgentFlights` into `LiminalActivityWorker`.
- Same `(workflow, activity)` redeliveries no longer spawn another harness session in the same worker process.
- Followers receive the leader execution’s terminal outcome with their own correlation IDs and completion tokens.
- Foreign activities remain independent executions.

### `crates/aion-worker/src/runtime/mod.rs`

- Declares the liminal single-flight module.

### `crates/aion-worker/Cargo.toml`
### `Cargo.lock`

- Added the workspace `dashmap` dependency for non-poisoning concurrent flight state.

### `crates/aion/src/engine/startup.rs`

- Startup recovery now considers the union of `list_active()` and `list_paused()`, deduplicated.
- Ordinary paused workflows remain non-resident.
- A paused history with a dangling `ActivityStarted` attempt is admitted to recovery so its activity can reach the adoption path.
- Added history classification for attempts with no matching completed, failed, or cancelled record.

**Important limitation:** this does not yet prove that the replay path omits `superseded:server-death` when the live holder is adopted. No adoption event was added or asserted.

### `crates/aion/src/engine/startup_tests.rs`

- Added the paused/dangling-activity recovery-classification pin.
- Preserved the existing control that an ordinary paused workflow is not made resident.

### `crates/aion/src/runtime/nif_activity_retry_dispatch.rs`

- Retry-budget exhaustion now reports:

```text
exhausted: spent 3 of 3 attempts; last refusal: retryable:reset three
```

- Non-exhausted retryable failures and genuinely non-retryable failures retain their prior behavior.

### `crates/aion/src/runtime/nif_activity_dispatch_tests/retry_outcomes.rs`

- Updated the exhaustion test to require honest exhaustion wording, attempts spent, budget, and last refusal.

### `crates/aion/src/runtime/handle/readiness.rs`

- Removed the fixed recovered-pending-await deadline.
- Resume/reopen now waits until either:
  - the recovered process reaches its pending await, or
  - the recovered process exits.
- This chooses the charter’s permitted **“wait out the recovery fence”** shape rather than “exit 0 with recovery pending.”

## Red → green evidence

### Exhaustion record

**Red commit:** `836ef5a88`

Focused red:

```text
budget exhaustion must name itself, its spent budget, and the last refusal:
RetryLoopOutcome {
    attempt: 3,
    terminal: Failed("retryable:reset three")
}
EXIT=101
```

**Green commit:** `b73246ddc`

Verified:

```text
runtime::nif_activity_dispatch::tests::retry_outcomes::
exhausted_retry_budget_names_exhaustion_and_last_refusal ... ok
```

### Spawn-gate self-adoption and foreign control

**Red commit:** `74c249d8c`

Focused red:

```text
harness::tests::a_live_self_occupancy_is_adopted ... FAILED

Occupied {
  detail: "spawn refused: live sibling pid 16749 holds this tree
           (workflow 8685562f-02b9-47c4-9c98-f507646f881d,
            activity activity:1)"
}
EXIT=101
```

**Green commit:** `85755d737`

Verified:

```text
harness::tests::a_live_self_occupancy_is_adopted ... ok
harness::tests::a_foreign_live_sibling_collision_is_retryable_not_terminal ... ok
```

### Paused dangling-activity recovery classification

**Red commit:** `6540c7c29`

Focused red:

```text
error[E0425]: cannot find function `history_needs_activity_adoption`
EXIT=101
```

**Green commit:** `d02da35a2`

Verified:

```text
paused_history_with_a_dangling_activity_requires_adoption ... ok
a_paused_run_forced_into_the_active_list_is_never_made_resident ... ok
```

### In-process redelivery single-flight

**Red commit:** `0724c75fe`

Focused red:

```text
error[E0432]: unresolved imports
`super::AgentExecutionKey`, `super::AgentFlights`
EXIT=101
```

**Contract commit:** `28bca0388`  
**Production wiring commit:** `2e03375ae`

Verified:

```text
runtime::agent_flights::tests::
same_execution_joins_and_foreign_execution_leads ... ok
```

Strict verification:

```text
cargo clippy -p aion-worker \
  --features liminal-transport \
  --all-targets -- -D warnings
```

passed.

### False-error resume rider

**Red commit:** `e350538ad`

Focused red:

```text
error[E0432]: unresolved import `super::pending_await_deadline`
EXIT=101
```

**Green commit:** `bbce645ac`

Verified:

```text
runtime::handle::readiness::tests::
recovered_pending_await_has_no_false_error_deadline ... ok
```

Strict verification:

```text
cargo clippy -p aion-rs --lib -- -D warnings
```

passed.

## Acceptance-leg disposition

| Acceptance leg | Disposition |
|---|---|
| Issue #36 verbatim: live builder → pause → restart → resume → completes with one session | **NOT PROVEN** — no isolated server-level reproduction was built or run |
| Foreign sibling remains refused | **Proven at Norn harness unit/integration boundary** |
| Redelivery into same live worker joins one outcome, with no second harness session | **Implementation and coordination test present; full two-wire-dispatch e2e not run** |
| Cross-process replacement worker reattaches surviving Norn session with session-ID continuity | **EXPLICIT NON-CLAIM** |
| Paused-at-boot history has no supersession and records adoption | **NOT PROVEN** — paused histories enter recovery, but no history assertion proves absence of `superseded:server-death`, and no adoption event was added |
| Exhaustion record names exhaustion | **Proven by focused red/green test** |
| Resume-under-load exits 0 and never reports the 400ms false error | **Partially proven** — the fixed deadline was removed and strict tests passed; no scripted CLI-under-load e2e was run |

## Explicit cross-process non-claim

The required cross-process fixture was **not built**.

The code currently permits a same-identity marker holder to enter the configured Norn invocation, which is the necessary gate behavior for `resume-if-exists`, but I did not run:

1. a real Norn agent,
2. kill its worker while leaving the agent alive,
3. start a replacement worker,
4. redeliver the same workflow/activity,
5. capture the pre/post Norn session ID,
6. prove session continuity and no second session.

Therefore I make **no claim** that the cross-process arm is accepted.

## Permitted choices made

- **Resume rider:** chose to wait out the real recovery fence rather than return “recovery pending.” This removes the arbitrary fixed deadline and preserves the requirement that timer rearming occurs only after the process has actually committed its await.
- **Concurrent flight state:** used `DashMap` instead of a poisonable standard mutex. No lint bypasses or poison suppression were introduced.
- **Flight key:** used logical `(workflow_id, activity ordinal)` rather than attempt, matching the approved self-occupancy identity and allowing later attempt/redelivery envelopes to join the same logical activity.
- **Paused enumeration:** ordinary paused histories remain non-resident; only paused histories with a genuinely dangling activity attempt enter recovery.

## Disclosed non-goals / uncompleted work

- No real isolated Aion server was started.
- No worktree-local `AION_HOME` was created.
- No ports were allocated.
- No issue #36 verbatim reproduction was run.
- No real Norn session-ID continuity test was run.
- No adoption event was introduced or asserted.
- No proof was made that `superseded:server-death` is absent for an adopted live holder.
- No scripted `aion resume` CLI-under-load test was run.
- No full workspace test or workspace-wide clippy pass was run after the final commits.
- No Opus review was performed.
- No push or merge was performed.

## Verification actually performed

Passed:

```text
cargo test -p aion-integration-norn
cargo test -p aion-rs --lib
cargo test -p aion-worker --features liminal-transport \
  same_execution_joins_and_foreign_execution_leads

cargo clippy -p aion-integration-norn -p aion-rs --lib -- -D warnings
cargo clippy -p aion-worker --features liminal-transport \
  --all-targets -- -D warnings
cargo clippy -p aion-rs --lib -- -D warnings

cargo fmt --check
```

Focused positive and negative controls listed above also passed.

## Final disposition

**Blocked, not deliverable as the approved lane.**

The committed work provides useful implementation slices for self-occupancy classification, same-worker single-flight joining, paused-history recovery enumeration, honest exhaustion records, and removal of the false readiness deadline. It does **not** satisfy the charter’s mandatory server-level and cross-process acceptance evidence, and it does not yet prove truthful no-supersession history for a live adopted holder.