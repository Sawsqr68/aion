# Red-then-green evidence — dev/retry-fencing

Captured at the reviewer's hands (Vesper Lynd), 2026-07-25. The build lane's
own red-iteration logs were discarded before its report (a gate miss, noted
in review); the red below was reproduced post-hoc with the stash technique
against the committed tree's parent.

## Red — absence: the completion-fencing surface did not exist

All implementation changes stashed (server, worker, proto, engine src), the
new tests kept, `cargo test -p aion-server --all-features --no-run`:

```
error[E0560]: struct `ActivityResult` has no field named `completion_token`
error[E0609]: no field `completion_token` on type
              `&aion_proto::aion_proto_generated::ActivityTask`
8 compile errors; exit 101
```

Labeled as what it is: an absence red — pre-fix, no wire field, no fence, no
token existed; a stale worker's completion had nothing to be checked against.
(The behavioral form — stale completion accepted live — is not expressible
against the old surface without the new wire fields; the ground-truth mapping
in the lane's envelope records the unfenced path at file level.)

## Green — the fencing

`CompletionFences` (aion-server/src/worker/envelope.rs): every live dispatch
issues a fresh opaque `CompletionToken` generation; workers echo it verbatim
(including reconnect re-reports); the server atomically accepts only the
current generation before workflow delivery; stale, duplicate, missing-token,
malformed, pre-recovery, and failed-delivery paths are typed rejections.
Idempotency keys: SHA-256, scoped by workflow + run + activity site,
attempt-independent — pinned by
`idempotency_key_is_attempt_independent_and_site_run_scoped`. Recovery pin:
`a_pre_recovery_generation_is_rejected_after_recovery` (process-incarnation
fencing — recovery mints a new incarnation, all prior generations refuse).
Exactly-once: `accepted_generation_is_consumed_exactly_once`. Stale retry:
`issuing_a_retry_rejects_the_stale_generation`, plus the integration pin
`stale_worker_completion_after_heartbeat_loss_*` at the server seam.

Touched-crate suites, clippy `-D warnings`, fmt — all exit 0 (lane's
envelope). Final full battery run by the reviewer on the finished tree:
`cargo test --workspace` exit 0, 213 suites ok
(reviewer log `retry-workspace-battery.log`).
