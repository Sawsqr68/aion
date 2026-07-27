# Red-then-green evidence — build/worker-handshake

The `.v4` worker-contract handshake: one domain-separated identity
(`WORKER_CONTRACT_DOMAIN`) binding canonical BEAMs, the v3 routing/timeout
vector, and the canonical contract encoding; field-level registration
refusals at the server seam; structural start admission (typed
`ContractIdentity` / `NoQueueDeclaration`); no grandfathering of pre-`.v4`
packages. Build by the lane (envelope `claude-dev-lane-handshake.v4XsfV`),
review, reviewer fixes, and the battery tail at Vesper Lynd's hands,
2026-07-27.

## Red — behavioral: a mismatched worker registration was ACCEPTED

`gate-logs/registration-refusal-red.log` (+ `.exit` = 101), captured by the
lane before the server-side check existed:

```
test mismatched_worker_is_refused_with_field_level_diff ... FAILED
---- mismatched_worker_is_refused_with_field_level_diff stdout ----
Error: "mismatched registration was accepted"
```

The strong form: the pre-fix server accepted a worker whose declared
contract disagreed with the deployed package — the exact lie the handshake
exists to make impossible.

## Green — the handshake

`gate-logs/registration-refusal-green.log` (+ `.exit` = 0):

```
test matching_worker_receives_register_ack ... ok
test mismatched_worker_is_refused_with_field_level_diff ... ok
```

The refusal is field-level (the diff names the disagreeing declaration) and
typed end to end. Core pins: no grandfathering
(`pre_v4_exact_version_is_refused_before_history_or_registration` — a
pre-`.v4` identity decodes wholly-uncommitted), encoding determinism
(`contract_tests.rs`: same contract, permuted declarations → identical
hash).

## Three reviewer-found defects (all from the battery the builder skipped)

1. **LAW-1 — timeout-arming authority from identity alone.**
   `Package::has_declared_timeout()` was a pure `has_contract_identity()`
   check: every `.v4` package claimed timeout-arming authority whether or
   not a timeout was ever authored. Caught by
   `unauthored_timeout_is_absent_and_authored_timeout_is_declared` in
   `aion-awl-package` — a crate the build lane never ran. Fixed to the
   value conjunct: `manifest.timeout.is_some() && has_contract_identity(…)`.
2. **Embedded-refusal class — the queue-service check refused engines with
   no queues.** `NoQueueDeclaration` admission killed every embedded-engine
   suite whose fixture carried a legacy manifest — including
   `activity_retry_e2e`, the proof that provider flakes cost retries, not
   runs (~16 suites, hidden behind fail-fast). Fix: the `ActivityServing`
   declaration on the catalog — `QueueRouted` is the fail-closed default
   and the server posture (refusals unchanged); `InProcess` is a written
   opt-in (`EngineBuilder::in_process_activity_serving()`) for engines
   whose configured dispatcher fulfils every activity directly, where no
   queue exists to be unserved. Pins:
   `in_process_serving_admits_unscoped_activities_past_admission`,
   `in_process_serving_still_refuses_pre_v4_identity` (the identity floor
   is unconditional in both postures), and — the ratified residual-cost
   pin — `in_process_dispatch_miss_is_typed_recorded_and_fails_the_run`
   (a wrong `InProcess` claim surfaces as a typed, journaled, run-failing
   dispatch miss; completion proves no hang).
3. **Registration hard-refused ALL workers on engine-less server states.**
   The gRPC contract check required the global engine accessor, which
   legitimately fails on states built from parts (`authorization_only`
   resolver) — every registration refused with an internal error, the
   worker SDK's retry budget exhausted silently, and the dispatcher then
   waited forever at INFO. Presented as a 3.5-hour hang in
   `heartbeat_sweeper_e2e` (`select_worker_or_wait` park; stack samples
   captured, and independently captured from outside by Waffles during the
   same morning's incident triage — two captures, one wedge). Fix mirrors
   the lane's own liminal path: no engine → no catalog → no deployed
   contract can exist to contradict the worker → admit with a loud warn;
   the strict check is untouched on engine-backed states. Live pin: the
   formerly-hung test, green in seconds. The admission-vs-validity boundary
   pin: `no_catalog_registration_still_refuses_malformed_descriptor`
   (declaration validity needs no catalog; the parse refusal runs before
   engine acquisition).

## The timeout awakening (behavioral change, ruled fixture-by-fixture)

`.v4` packaging wakes the previously-inert `timeout_seconds` in every
legacy manifest: declared deadlines now really arm (`TimerStarted` at
start, `TimerCancelled` at completion) — the lane's intent, and correct.
Fallout and rulings:

* **The authoring surface forced the lie**: `timeout_seconds` was a
  REQUIRED descriptor field, so "no timeout" was unauthorable and every
  package carried boilerplate. Now optional — absent decodes to `None`
  (no buried default), declared zero refuses. Pin:
  `absent_timeout_seconds_declares_no_timeout`.
* **Fixtures now declare what they MEAN**: unmeant boilerplate stripped
  (`examples/hello-world`, `fixtures/sleep_query`, `fixtures/nested_chain`,
  the shared in-code test manifests); meant declarations untouched. This
  also makes `no_declared_timeout_arms_nothing`'s premise true.
* **Synthesized-history identities**: two suites hand-computed the
  beams-only pre-`.v4` hash for recorded package pins; recovery correctly
  refused them ("pinned version not loaded"). `fixture_version` now
  recomputes the contract-bound identity through the same package
  construction the loads use.
* **bc4 differential**: both backends arm identically; the only
  divergences were the run-uuid embedded in `deadline:<run-id>` timer
  names, which the trail normalizer now aliases through the same
  first-appearance run-id table (decision 11 preserved — identity aliasing
  only, never shape). Park-summary goldens updated to expect the deadline
  timer with the aliased identity.
* **The park/drain e2e** stays on the STRICT queue-routed path: its
  fixture now carries a real queue-scoped `PackageContract`
  (`with_contract`, the same mechanism a checked AWL deploy uses) and its
  worker advertises the matching typed census — no exemption.

## The ruled fork — legacy Gleam agent scaffold (no grandfathering)

The `agent` template's implicit activities (`act`, `scout`, `verify`)
cannot be scoped by a legacy manifest, so its deployed package refuses at
start with `NO_QUEUE_DECLARATION` — a known out-of-box break until the
template-AWL-migration lane. Ratified with the mint-time-loudness
condition, both built:

* `new_agent_e2e.rs` PINS the refusal
  (`scaffolded_agent_deploys_but_start_is_refused_unscoped`: deploy
  succeeds, start refuses with the typed message naming the unscoped
  activities and the fix, graceful drain still exits 0).
* `aion new agent` warns unmissably at mint (stderr + `warning` field in
  the JSON report) naming the AWL successor — pinned by
  `agent_scaffold_warns_it_will_be_refused_at_start` and re-asserted
  through the real binary inside the e2e.

## Battery record

* Lane's own runs: touched-crate suites + clippy `-D warnings` clean
  (gate-logs/), but no workspace battery — the gap all three defects hid
  behind.
* `gate-logs/test-workspace-reviewer2.log` / `-reviewer3.log` /
  `-reviewer5.log` / `-reviewer6.log`: intermediate fail-fast runs during
  the tail hunt (historical; each one's failure is resolved above).
* Triage run (no-fail-fast, scratchpad `battery-triage.log`):
  **triage-only — mixed tree** (edits landed mid-run); its role was
  enumerating the tail, not proving green.
* One flake disclosed, not waved: `gate-logs/test-workspace-reviewer4.log`
  aborted on `recovered_reconcile_failure_after_publication_runs_observed_abort`
  (a startup-recovery liveness race in an area this lane never touched).
  Five isolated runs green, full `aion-rs --lib` rerun green 578/578
  (`gate-logs/test-aion-lib-reviewer-rerun.log`); it passed in every other
  battery including the citable one. Filed as a flake observation, not
  fixed here.
* **The citable battery**: `gate-logs/test-workspace-reviewer7.log` —
  plain `cargo test --workspace` at the exact committed tree, exit 0,
  **214 suites, 0 failures**. Clippy `-D warnings` exit 0 at the same tree
  (`gate-logs/cargo-clippy-reviewer-final3.log`); `cargo fmt` clean.
