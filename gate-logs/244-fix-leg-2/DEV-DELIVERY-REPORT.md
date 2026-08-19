# Delivery report — #244 fix leg 2

**Status: COMPLETE**  
Branch: `244-child-visibility`  
Delivered tip: `e8c1b28bfcb064124d52467aa68b57d7e142919f`

Three append-only, linear commits were added after `0328e67a2`:

1. `fa69076706` — `test(cli): pin selected-generation replay attribution`
2. `b48fb022cc` — `fix(cli): scope replay payloads to the selected run`
3. `e8c1b28bfc` — `refactor(cli): keep replay regression lint-clean`

No merges, rebases, or history rewrites. The merge-base remains `0328e67a24384d8adf895b82c542360afd89a2b1`. Commit bodies contain real newlines.

## NEW-B1 — fixed

### Production change

In `crates/aion-cli/src/tail/live.rs`, `monitor_source` now skips every non-`WorkflowStarted` event until it observes the selected run’s own `WorkflowStarted` marker:

```rust
if !selected_run_seen {
    continue;
}
```

This gate is placed before both:

- parent `ChildWorkflowStarted` discovery/following;
- `observe_attempt`.

Therefore, prior-generation payloads replayed by the durable seq-1 subscription cannot:

- announce or follow an old child;
- emit a false supersession notice;
- open a transcript using selected-run coordinates and an old attempt;
- poison the selected run’s current-attempt map.

Continued-as-new detection after the selected generation begins remains unchanged.

### Red pin

Commit: `fa69076706`

The shipping-binary e2e now sends a seq-ordered, two-generation parent history:

- prior run start;
- prior child start;
- prior activity attempt 9;
- selected run start;
- selected activity attempt 2;
- selected child start.

It verifies that:

- the prior child is neither announced nor followed;
- no `SUPERSEDED→9` line is printed;
- selected-run attempt 2 still attaches and prints live output;
- the selected-generation child is still followed.

Authentic pre-fix failure:

> prior-generation child workflow socket was followed; prior-generation child was announced; prior-generation activity printed a false superseded notice; selected-run attempt 2 was suppressed after prior replay poisoned current attempts; selected-run attempt 2 live transcript was not printed

Captured stdout included:

> `[PriorBuilder 00000000] child start recorded; run not started yet; following for its run`

and:

> `[parent 00000000 activity:activity:7 attempt:1 SUPERSEDED→9] superseded; this stream is no longer working`

The test failed with exit code `101` on the red commit and passes after `b48fb022cc`.

## NEW-2 — fixed

### Production change

In `crates/aion-cli/src/tail.rs`, `backfill_notice` now handles:

- `event_count == 0`
- `first_store_seq == None`
- `head_seq > 0`

by emitting:

> `no retained events; head at durable seq N; earlier records were trimmed by retention`

Other existing notice behavior is unchanged.

### Red pin

Commit: `fa69076706`

Added the focused unit test:

`tail::tests::empty_retained_backfill_reports_its_nonzero_durable_head`

Authentic pre-fix failure:

> `Error: "an empty retained range needs notice"`

The test failed with exit code `101` on the red commit and passes after `b48fb022cc`.

## Implementation choices

- I used one positional gate immediately after the `WorkflowStarted` arm. This directly matches the event model: replay is durable-sequence ordered, and `EventEnvelope` has no run ID for payload-level filtering.
- The fixture uses monotonically increasing parent workflow event sequences across both generations. This ensures it represents an actual durable replay rather than resetting sequence numbers at the generation boundary.
- The e2e checks old-child following at the workflow-socket boundary, rather than depending only on output timing.
- The first workspace Clippy run identified the enlarged regression test as exceeding `clippy::too_many_lines`. No suppression was added. Commit `e8c1b28bfc` extracted stdout collection and outcome assertions into the existing fixture module; both Clippy commands then passed.

## Verification performed

All commands below were run in the requested worktree at the delivered bytes.

- `cargo fmt --all -- --check` — **passed**
- `cargo clippy --workspace --all-targets -- -D warnings` — **passed**
- `cargo clippy -p aion-cli -- -D warnings` — **passed**
- Focused NEW-2 unit test — **1 passed**
- Focused NEW-B1 shipping-binary e2e — **1 passed**
- `cargo test -p aion-cli --no-fail-fast` — **exit 0**
  - Tests requiring `AION_RUN_COSTLY_TESTS=1` emitted their existing `COSTLY_PROOF_SKIPPED` notices; no test failed.
- `cargo test -p aion-cli --test tail_follow_e2e` — **2 passed, 0 failed**
- `git diff --check` — **passed**
- Branch ref and `HEAD` both resolve to `e8c1b28bfcb064124d52467aa68b57d7e142919f`.
- Added chain: **3 commits, 0 merges**, with `0328e67a2` still the merge-base.

Final worktree status contains only the previously disclosed, preserved untracked path:

```text
?? gate-logs/244-child-visibility/
```

No live estate, other worktree, server process, worker process, or `~/.aion` state was touched.