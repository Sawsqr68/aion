# Open-issue triage, 6 August 2026

Tom asked for agents on the issues raised against the repository. Five open
issues were investigated by two Opus subagents under read-only constraints —
no builds, no tests, no processes touched — with every citation re-read at the
tree and every claim date-checked for staleness against `git log -S`. Issue #10
was measured at this seat and is recorded in marker AO.

**Trees investigated:** #9 and #1 at `be82eaf9c`; #7, #6 and #5 at `8dbe25b19`.

## Verdicts

| Issue | Subject | Verdict |
|---|---|---|
| **#10** | getting-started path fails | **PARTLY STALE** — claim 1 still true, claim 2 half-stale, claim 3 is a real trap |
| **#9** | `ActivityFailed.kind` reflects retry disposition | **TRUE, NOT STALE** — and a duplicate of our own reverted #90 |
| **#7** | completion monitor treats transient store errors as terminal | **TRUE, NOT STALE** — no prior internal knowledge |
| **#6** | outbox head-of-line block + serial plain activities | **PARTLY STALE** — half fixed 2026-07-30, half live and deliberate |
| **#5** | AWL has no numeric `reduce`/`sum` | **TRUE, NOT STALE** — and the spec overclaimed |
| **#1** | `workflow.now()` returns history tail, not replay position | **TRUE, NOT STALE** — the line has never been touched |

## #9 — a duplicate of a fix we already reverted

`kind` is a hardcoded constant at both ends of the retry lifecycle:
`Retryable` stamped only by the retry loop
(`nif_activity_retry_dispatch.rs:441`), `Terminal` stamped at the await seam
(`nif_activity.rs:262-268`) and again at the delivery seam
(`handle/delivery.rs:635-641`). The worker's structured kind survives inside
the engine **only** as the `retryable:` string prefix.

The issue understates its own scope — the last attempt of a policy-bearing
retryable failure is *also* recorded `Terminal` — and its proposed direction 1
would break replay: `cursor.rs:324-349` walks past any retryable
`ActivityFailed` looking for a later attempt, and would re-dispatch on every
replay if a final failure were reclassified.

**The important context:** `RETRY-CLASS-HONESTY-90.md` is our own post-mortem
of the same observation. The obvious fix was implemented, broke the estate, and
was reverted at `64aa16c99` — the `retryable:` prefix is the Gleam SDK's wire
format, and rewriting it made `error.Retryable` unreachable from every remote
activity. §5 there recommends the inverse of the issue's option 3: leave
`message` and `kind` alone and put the operator-facing fact in `details`, which
is always `None` today.

⇒ **Reply, don't build.** The behaviour is pinned deliberately at
`activity_retry_e2e.rs:246-255` and that pin is genuine, not a husk.

## #7 — the strongest of the five, and undefended

`completion.rs:77` reads history with `?`. A transient store error propagates
to `start.rs:574`, which logs `tracing::error!` and returns. The terminal event
is never appended; because status is a projection, the run then projects
`Running`. `monitor_process` is `FnOnce` and the exit record is retired
unconditionally (`monitor.rs:347`), so there is no re-arm.

**Qualification the issue does not make:** "forever" holds for the life of the
server process. `repopulate_active_workflows` re-spawns anything projecting
`Running` on restart, which installs a fresh monitor. So the honest severity is
*a single transient store error wedges the run until the node restarts*, with
no operator-visible signal in between.

**All four tests in the module are husks with respect to this defect** — every
one takes its store from `active_workflow()`, which is `InMemoryStore`, whose
`read_history` cannot fail transiently, so the `?` is never taken. All four
also call `handle_process_exit_async` directly, so the swallowing log site at
`start.rs:574` is executed by no test in the crate. Nothing pins the one-shot
behaviour as a contract either, so a fix breaks no deliberate pin.

The precedent the issue names is real and one directory away:
`nif_child_watch.rs:1142` retries through a `FlakyAppendStore` until the
terminal lands. The fault-injection harness exists; the completion monitor has
no equivalent.

⇒ **A genuine build item.** Two things to get right: there is no
`StoreError::is_retryable` (the precedent retries unconditionally, which is
safe because the terminal-from-history re-check makes it idempotent), and the
`WorkflowTimedOut` exception at `completion.rs:94` is a real contract with its
own pin that a naive retry refactor would break.

## #6 — half of it we fixed from the other end

**Finding 1 (global head-of-line block) is STALE.** At the filing tree
`sweep_once` awaited `process_row` per row in sequence, and `push_dispatch`
deliberately has no reply deadline, so one long activity held the dispatcher's
entire loop — no row anywhere on the node was claimed. That is sufficient to
produce every number in the reporter's table, including `deadletters=0`.
`f464129d4` (2026-07-30) replaced the loop with per-row detached tasks. **The
fix was written for a different symptom** (unfenced at-least-once execution)
and does not name this issue: the reporter's hypothesis was independently
correct and is now closed.

But it is **not pinned**. Under `cfg(test)` `sweep_once` joins every spawned
row task before returning, re-imposing the very serialisation that was the
defect, and no pin in `outbox_delivery_contract_e2e.rs` uses more than one
worker. `wedge_shape_regression_0756ecd5` derives its 400 s deadline from
*fully serial* execution, so it is green under total head-of-line blocking and
green under perfect fan-out. Sound pin for exactly-once; blind to throughput.

**Finding 2 is LIVE.** The inline-on-the-serve-loop half was fixed
(`8e5190314`), but the serialisation was kept on purpose, and
`liminal.rs:786-793` says so, citing a measured incident (a gate worker running
N cargo builds filled 37 GiB in 17 minutes). Read the justification both ways:
that is a real reason deferring a real decision, not a limitation dressed as a
contract. It does not contradict the issue — it confirms the diagnosis and
declines on scope. **The live defect is narrower and sharper than reported:
`max_concurrency` is a required config value (`config.rs:377`) that is enforced
on the gRPC path (`loop_.rs:186`) and silently ignored on the liminal plain
path — a setting the operator must supply and that does nothing.**

## #5 — true, and the spec was worse than the issue

Six `CombinatorKind` variants, six lexer keywords, no `Sum`/`Reduce`/`Fold`
anywhere. 58 commits touched `crates/aion-awl` since filing and six touched the
files defining the combinator vocabulary; the language grew signals, queries,
`advisory`, declared bodies and LSP completion, and did not grow a fold.

**The workaround space is empty, not merely awkward** — verified at this seat,
not taken on report. `fork` iterates but has no accumulator (`ForkStmt` has no
such field); `loop` threads a value but has no collection binding; indexing is
literal-only, `Expr::Index` storing `index: u64` with the doc comment
*"Computed indices are unwritable"*. The three are pairwise incompatible.

**`AWL-2-SPEC.md` claimed the opposite** — a mapping row reading *"sequential
fold | `fork … sequential` or `loop`"* — a documented capability that does not
exist, and one that would have caused this issue to be wrongly closed as
already-supported by anyone triaging against the spec. **Corrected in this
commit**, with the reason stated inline.

⇒ `sum` first: a seventh variant, a fixed type rule, no new binding form, and
combinators already replay as pure recomputation. A general `reduce` needs a
two-argument lambda the language has nowhere else. Landing it must bump
`Keyword::ALL`'s `[Self; 57]` in the same change — the length assertion is what
forces those two places to agree.

## #1 — the contract is documented and violated

`nif_context.rs:118` takes `history.last()`, and the context is rebuilt fresh
on every NIF call, so `now()` is a function of history length at call time, not
of replay position. `workflows.md:65-66` promises the opposite: *"it returns
the recorded event timestamp, identical on every replay."*

Small correction to the issue: the value is not fixed at `T_wake` forever, it
is the tail at each call — constant through the replay window, then advancing
again. That does not weaken the claim.

**One husk and one pin defending the defect.**
`replay_sequence_matches_recorded_history_and_seeded_positions` builds both
contexts over the *same* history vector, so the two `now` values are equal by
construction and the assertion cannot fail either way — its model of "replay"
is the one case the defect does not affect. And
`now_returns_last_recorded_event_timestamp_millis` asserts the tail timestamp
explicitly, encoding the defective behaviour as the contract.

The position-stable path exists and is unwired: `Replay::now()` has no
production caller in this repository — though `Replay` and `DeterminismContext`
are both `pub` and re-exported from a published crate, so that census is
bounded at the repository edge and cannot speak for an external SDK consumer.

## What was not established

- **#7's fix has not been designed**, only scoped; the mechanism was read but
  nothing was built or run.
- **#6 Finding 1 was not proven to be the only mechanism live on the
  reporter's box** — the worker's inline serve loop was independently capable
  of blocking at that tree.
- **#6's head-of-line fix has never been verified live**, and as noted it is
  not pinned.
- No issue was replied to on GitHub. Every verdict above is ours, not theirs.
