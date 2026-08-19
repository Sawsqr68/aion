# #266 Defect-B lane — the superseded attempt gets its record

**Worktree:** `.worktrees/supersession-record`, branch `supersession-record`, base main cec80a8b4.
**Ruling (Waffles 17:38Z 08-16, GO 21:06Z):** adoption records
`ActivityFailed{attempt, reason: server-death}` BEFORE re-dispatch; terminality
= the action's own retry policy. Digest rides #264 to Tom; reshape on his word
expected. This is the ATTEMPT-level record — NOT the run-level list-supersession
lane (BRIEF-SUPERSESSION-DRAFT.md, separate, at Waffles' desk).

## The defect, at the bytes

Recovery replay of an in-flight activity resolves `ResumeLive`
(`resolver.rs`; cursor Exhausted on a dangling Scheduled/Started trail). The
dispatch site `nif_activity_dispatch.rs:244` continues the attempt trail:
`attempt = next_delivery_attempt(history, activity_id).max(call.attempt)`
(`nif_activity_retry.rs:363` — latest recorded attempt N → N+1). It records
`ActivityStarted{attempt: N+1}` and dispatches. **Attempt N is left dangling
forever: no ActivityFailed, no terminal of any kind.** Pulse-6's five parked
Scheduled+Started pairs are five wild specimens. An honest projection reads
Running for an attempt that died with its server.

## The model to mirror (live retry loop, nif_activity_retry_dispatch.rs)

- A failed live attempt is recorded durably as a NON-terminal (Retryable)
  `ActivityFailed` (record_retry_event), then the loop backs off and
  re-dispatches attempt+1. History walks past non-terminal failures to the
  eventual terminal.
- Exhausted budget (`failure_stand_down`: policy present && attempt >=
  max_attempts → stand down) delivers the failure to the workflow; the workflow
  thread records the terminal `ActivityFailed`.
- The resolver already treats a dangling RETRYABLE failure as ResumeLive
  (resolver.rs:645 comment, the #197 crash-mid-backoff shape) — so writing the
  supersession record keeps every later replay resolving identically. The
  record is CONVERGENT with existing machinery, not a new class.

## Fix shape

In `dispatch_activity_with_context`'s ResumeLive arm (nif_activity_dispatch.rs),
BEFORE `record_started` for attempt N+1:

1. Dangling predicate: `latest_recorded_attempt(history, id) == Some(N)` AND
   attempt N carries a Started/Scheduled with no ActivityFailed/Completed/
   Cancelled at attempt >= N (i.e. the trail's newest event for the ordinal is
   non-terminal). If no prior attempt (fresh ordinal), nothing to record.
   NOTE: if attempt N's newest record IS a retryable ActivityFailed (the #197
   crash-mid-backoff shape), the failure record already exists — do NOT write a
   second one. The predicate must distinguish dangling-Started from
   already-failed-retryable. Idempotence for OUR record comes from the same
   test: once written, attempt N is no longer dangling.
2. Record `ActivityFailed{activity_id, attempt: N, error: Retryable,
   message: server-death supersession reason (canonical string constant)}`
   through the recorder (single-writer invariant — same seam record_started
   uses).
3. Terminality: consult `retry_policy_from_config(&call.config)`. If policy
   present and N >= max_attempts (death during the final attempt), the
   supersession failure is the LAST allowed: follow the live exhaustion shape —
   the workflow receives the failure (terminal ActivityFailed recorded by the
   workflow thread), NO re-dispatch. Investigate exact delivery path before
   building this arm: mirroring failure_stand_down + deliver via the same
   mechanism the recorded-failure resolve path uses.
4. `next_delivery_attempt` already yields N+1 whether the trail ends in
   Started{N} or Failed{N} (it takes max attempt + 1) — re-dispatch numbering
   unchanged by the new record.

## Red-first ordering

Test 1 (unit, engine crate): history with WorkflowStarted + Scheduled +
Started{attempt:1}, replay → re-dispatch; assert history now carries
ActivityFailed{attempt:1, retryable, server-death reason} BEFORE
Started{attempt:2}. RED today (no such record).
Test 2 (control): fresh ordinal replay records NO ActivityFailed.
Test 3 (idempotence/#197 shape): trail ending in retryable ActivityFailed{1}
→ re-dispatch writes NO second failure record for attempt 1.
Test 4 (exhaustion): policy max_attempts=1, dangling Started{1} → supersession
failure is terminal per policy; no dispatch of attempt 2; workflow observes
the failure.
E2e candidate: extend recovery_declared_body_e2e (the #266 rig) — corpse with
in-flight attempt, post-recovery history carries the supersession record.

## Open questions (answer in-tree before building arm 3)

- ✅ ANSWERED (07:2x 08-17): exhaustion needs NO new machinery. Live-path
  exhaustion shape = retry loop records non-terminal Failed{N}, then delivers
  via `runtime.deliver_activity_failure_message_with_attempt(workflow_pid,
  correlation, reason, Some(N))`; the WORKFLOW THREAD records the terminal
  ActivityFailed on receipt (nif_activity_retry_dispatch.rs:41-59 + ~458
  comment). Recovery-site exhaustion arm mirrors it exactly: record
  supersession Failed{N}, skip the dispatcher, deliver the failure message.
  VERIFY IN-TREE while coding: how the delivered reason string is classified
  terminal-vs-retryable on the workflow thread (the server-death constant must
  land terminal on this delivery path — check is_retryable_reason's consumers).
- Endorsement (Waffles 21:11Z): plan ratified as-written; Scheduled-no-Started
  = state the shape, test the resolver's behavior at it, never invent a
  record; his re-gate fires on my closeout.
- Canonical reason string: house has is_worker_lost_reason / is_parked_reason
  vocabularies (nif_activity_retry.rs) — the new reason must be its own
  canonical constant, checked against those predicates' behavior (must NOT
  accidentally match is_retryable_reason arms in a way that changes class).
- Whether ActivityScheduled-only (no Started) dangling attempts exist in the
  wild population (park-before-start) and carry an attempt number at all —
  Scheduled has NO attempt field (see event.rs); dangling-Scheduled ordinals
  have latest_recorded_attempt == None → predicate treats as fresh — confirm
  that is the honest shape (nothing started, nothing to fail) and state it in
  the delivery report.

## Gates

Standard battery (t4-battery.sh shape): scoped tests, test -p aion-server,
test -p aion-rs, clippy --workspace, clippy -p aion-server, all -D warnings,
per-leg load, manifest exit codes. Red logs kept. #85 attribution protocol
stands. Bindings regen if any ts-rs surface moves (none expected — Event is
already generated? check).
