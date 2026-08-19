# Manifold #210 delivery report

**Worktree:** `/Users/tom/Developer/ablative/apps/manifold/.worktrees/m210-attach-backpressure`  
**Branch:** `m210-attach-backpressure`  
**Base:** `bb213fdd60ef1d31d06a9095251e641e246d1054`  
**Final HEAD:** `d9a92df`  
**State:** clean; not pushed and not merged. The live estate, servers, workers, and `~/.aion` were not touched.

## 1. Protocol contract findings

The lockfile resolves:

- `liminal-protocol` **0.7.0**
- crates.io checksum `a7d32aac53e4692c4e5d7a25c17870e675b31511fb0a5c38a9c518f27a54a2b0`

### Who emits `ObserverProgressed`

The broker/server emits `ServerPush::ObserverProgressed` on the **participant connection stream**. It is connection-scoped to the connection whose recovery handshake armed the refusal.

It carries:

- `conversation_id`
- the exact `refused_epoch`
- current `observer_progress`

Relevant resolved-source locations:

- `wire/request.rs:134-168`: `ObserverRefusal` and `ObserverRecoveryHandshake`
- `wire/response.rs:828-923`: refusal epoch/progress and operation-specific `ObserverBackpressure`
- `wire/response.rs:1707-1727`: accepted recovery status, including `armed` and `progressed`
- `wire/push.rs:153-172`: `ObserverProgressed` and its connection-scoped delivery
- `lifecycle/observer_recovery.rs:274-323`: authoritative observer-progress advance fires an installed equal-epoch arm

`ObserverProgressed` is not the direct correlated reply to the refused attach or admission. The client first sends `ObserverRecoveryHandshake`. If the accepted status says the epoch is already progressed, it may retry immediately. Otherwise the handshake atomically arms the equal epoch, and the later authoritative observer-progress advance generates the push.

### Can manifold legally drive it?

Yes.

The feared deadlock cycle was:

1. manifold parks waiting for `ObserverProgressed`;
2. the push is delivered on the same participant stream;
3. no code drains that stream because the exchange owner is blocked elsewhere;
4. the wake can never be observed.

The implemented shape closes that cycle by waiting through the SDK’s event-driven `receive_within` door on the same exchange owner. It does not poll or infer progress. Unrelated pushes received during the handshake or parked wait are retained and returned to the normal mailbox caller.

Observer progress itself remains server-owned and can only advance from an authoritative acknowledgement or binding-ending transition. Manifold does not guess or synthesize it.

### What bounds the wait?

The protocol carries **no elapsed-time or retry bound**.

The implementation therefore introduces an all-or-nothing deployment budget with no defaults:

- `MANIFOLD_OBSERVER_RECOVERY_WAIT_MS`
- `MANIFOLD_OBSERVER_RECOVERY_MAX_ATTEMPTS`

If recovery is required and either value is absent, zero, malformed, or only one member of the pair is configured, recovery refuses rather than inventing a constant.

The wait value bounds one event-driven parked wait. The attempt value bounds repeated refusal/handshake/retry cycles.

## 2. K9 attribution duty

I ran the mailbox suite alone on the untouched base commit:

```text
cargo test -p manifold-node --test mailbox
```

Evidence: `/tmp/manifold-210-base-mailbox.log`

- Started: `2026-08-15T14:58:41Z`
- Base commit: `bb213fdd60ef1d31d06a9095251e641e246d1054`
- Load before: `57.43 34.04 27.87`
- Load after: `51.44 26.97 22.12`
- Duration: `1444.74s`
- Result: **142 passed, 3 failed, 0 ignored**

Failures:

- `session_door::a_command_that_ignores_the_hangup_is_reported_as_an_unattachable_survivor`
- `session_door::killing_the_kernel_hangs_up_the_session_that_was_created_first`
- `write_door::a_partial_bootstrap_reconciles_each_missing_seat_on_the_next_boot`

The partial-bootstrap failure contained `MarkerSettlementBackpressure`, not `ObserverBackpressure`.

### Attribution verdict

**None of those three failures is attributed to #210.**

The earlier 26 K9 reds also remain **not attributed**:

- the supplied K9 log contained no observer-backpressure text;
- the solo base run did not reproduce the 26-failure distribution;
- it produced no `ObserverBackpressure` face.

No causal claim is made for attach_client, face, registry, chair, manifest, or session_door failures without a matching fixture or log.

## 3. Red-first evidence

### Contract/attribution commit

`8946d05 docs(mailbox): record #210 contract and base attribution`

Recorded the resolved protocol contract and honest base-suite result before implementation.

### Committed red

`8dd0870 test(mailbox): pin #210 observer recovery red`

The commit message quotes the red and load:

```text
error[E0599]: no variant named `Backpressured` found for enum `AttachOutcome`
error: could not compile `manifold-node` (lib test) due to 1 previous error
```

Load: `14.76 17.85 19.30`.

The fixture constructs the exact broker attach-backpressure response with refusal epoch `41` and requires that the exchange seam preserve it as a recoverable, epoch-bearing outcome instead of returning the old terminal bailout.

### Green implementation

`14a71e6 fix(mailbox): drive observer backpressure recovery`

The original red fixture passes after implementation.

A second regression pins the bounded terminal refusal’s typed fields:

- final refused epoch `43`
- attempts `3`
- recovery sequence `[41, 42, 43]`

### Attempt-count honesty correction

`d9a92df fix(mailbox): count only recovery handshakes sent`

Corrected the bound accounting so `attempts` counts handshakes actually sent. A terminal backpressure epoch is retained in the recovery sequence without falsely claiming that another handshake was attempted.

## 4. File-by-file changes

### `crates/manifold-node/src/mailbox/recovery.rs`

New shared recovery implementation:

- extracts exact conversation and epoch from every `ObserverBackpressure` operation family;
- sends `ObserverRecoveryHandshake`;
- validates the single request-correlated recovery status;
- retries immediately for `progressed`;
- waits through `receive_within` for an armed, matching `ObserverProgressed`;
- preserves unrelated pushes;
- retries the original byte-identical request;
- enforces deployment wait and attempt bounds;
- returns typed `ObserverRecoveryRefusal` with epoch, actual handshake count, sequence, and reason;
- retains `NeverSent` typing for SDK-local refusals.

### `mailbox/member.rs`

- Removed the “named unimplemented arm” documentation.
- Enrollment now uses the shared recovering round trip.
- Attach now uses the local-refusal-preserving recovery wrapper.
- Added `AttachOutcome::Backpressured` for direct response-seam classification.
- Removed the two explicit terminal observer-backpressure bailouts.
- Reduced the file to **489 lines**.

### `mailbox/driver/submit.rs`

Record admission now uses the same shared recovering round trip. The request remains borrowed and cloned unchanged, preserving its payload, generation, and attempt token across retries.

### `mailbox/driver/fate.rs`

`ObserverBackpressure` is no longer classified as `Fate::Refused`. The shared exchange must consume it before record-fate classification; a leaked refusal is treated as an unreadable internal protocol mismatch rather than spending staged bytes.

### `mailbox/driver/inbound.rs`

Updated stale reporting that claimed manifold did not drive observer recovery. A late unmatched wake is now reported as having no currently parked matching recovery wait.

### `mailbox/exchange.rs`

Made the existing `unsent` constructor visible within the mailbox module so the recovery wrapper preserves the established typed `NeverSent` semantics.

### `mailbox/refusal.rs`

Extracted `CapacityRefusal` into a focused module to satisfy the 500-line floor without changing its public behavior.

### `mailbox/member_tests.rs`

Moved member classification tests out of `member.rs`, including the committed red/green epoch fixture.

### `mailbox/mod.rs`

Registered the recovery, refusal, and extracted test modules.

### `docs/evidence/MANIFOLD-210-contract-and-base.md`

Records:

- resolved dependency contract;
- exact wait-cycle analysis;
- base mailbox attribution;
- deployment value discussion;
- commits and red/green evidence;
- verification actually performed.

## 5. Permitted choices

- **Environment-backed deployment values:** selected because the protocol provides no bound and the mailbox call sites do not currently carry a deployment configuration object. Both values are mandatory together and have no default.
- **SDK `receive_within`:** selected because it is explicitly the caller-bounded, event-driven participant receive door. No sleep or wall-clock polling loop was introduced.
- **One shared wrapper:** enrollment, credential attach, and record admission all pass through the same recovery implementation, preventing three divergent interpretations.
- **Push preservation:** every nonmatching push remains caller-owned rather than being discarded while recovery is parked.
- **Actual handshakes as the attempt count:** the terminal repeated refusal is included in the epoch sequence but does not inflate the number of handshakes claimed.

## 6. Verification performed

Final gates:

```text
cargo test -p manifold-node --lib
```

- **867 passed**
- 0 failed
- 0 ignored
- 32.06s
- Evidence: `/tmp/manifold-210-unit-green-final2.log`

```text
cargo clippy -p manifold-node --all-targets -- -D warnings
```

- Passed
- No lint bypasses or allowances added
- Evidence: `/tmp/manifold-210-clippy-final2.log`

```text
cargo fmt --all
```

- Passed

`git diff --check` passed, and the final worktree is clean.

## 7. Disclosed non-goals and verification limits

- The full mailbox integration suite was **not rerun after implementation**. Its base run took approximately 24 minutes; only the required base attribution run and the final library/clippy gates above are claimed.
- No live broker, server, worker, running estate, or `~/.aion` state was modified.
- No push or merge was performed.
- The committed red/green exchange fixture pins lawful epoch preservation at the response seam. The final tests also pin bounded typed-refusal contents. A separate scripted transport transcript asserting every emitted handshake frame, parked receive, and final accepted retry was not added; no claim is made that such a test was run.
