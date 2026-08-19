# Delivery report — aion #244 child visibility

**Branch:** `244-child-visibility`  
**Reviewed base:** `b0214c0ef`  
**Delivered tip:** `0328e67a24384d8adf895b82c542360afd89a2b1`  
**Append-only remediation:** 25 new commits; no rebase, merge, push, or history rewrite.

Tracked files are clean and `git diff --check` passes. The pre-existing untracked `gate-logs/244-child-visibility/` directory was preserved and not committed. No test `aion` process or other process started by this work remains running.

## Blockers

### B1 — trailing `null` from `aion tail`

- **RED:** `2648cf5bd` — shipping-binary test demonstrated stdout was exactly `"null\n"`.
- **GREEN:** `001dcb3ae` — routed `tail` around generic `print_json`, following the existing AWL reporting-contract precedent.
- **Lint follow-up:** `54c1bb083` — extracted client-command rendering from `main` so the exemption remains below Clippy’s function-length limit without suppression.

Tail owns its human transcript output and emits no terminal JSON `null`.

### B2 — console presented history failures as “No activity transcript”

- **RED:** `4c3e7f658` — replaced the presentation-only fixture with a test seam against the shipped `LiveLegsSection`.
- **GREEN:** `dca160b4d` — added distinct loading and `role="alert"` error states. The “No activity transcript stream recorded” message now appears only after a successful history read.

### B3 — pending child never became live after page load

- **RED:** `4c3e7f658` — pinned the absence of a pending-child refresh policy.
- **GREEN:** `dca160b4d` — pending rows refetch once per second until `run_id` appears.

The one-second interval is a named, documented responsiveness/load decision rather than an implicit query-library default.

### B4 — CLI attach race silently lost events

- **RED:** `ef143fa1c` — the real-binary e2e failed once the fixture required `resume_from_seq = 1`.
- **GREEN:** `cff44b588` — workflow monitors now subscribe from the durable cursor origin, seed already-discovered children/current attempts, and replay attach-window events idempotently.
- Continued-as-new handling now ignores older replayed generations until the selected run is observed, then announces only a later generation.

### B5 — false truncation/window notice

- **RED:** `0e92c6ee6` — pinned that the notice must not treat `head_seq` as an event count or claim a server-published tail window.
- **GREEN:** `84df28eab` — notice generation now uses durable sequence ranges and attributes unavailable earlier records to retention or the CLI’s explicit 2,000-event request bound.
- **Docs:** `23dcd1644` — documents that:
  - `last` is currently unbounded server-side;
  - `limit` alone is clamped to `1..2000`;
  - `head_seq` is the next durable sequence, not an event count;
  - 2,000 is a CLI-owned choice, not a server guarantee.

I chose not to change server `last` semantics in this slice. The truthful fix preserves the existing API and explicitly bounds the CLI request.

### B6 — unknown parent run returned truthful-looking empty success

- **RED:** `7fedcd090` — captured `200` versus expected `404`.
- **GREEN:** `79e2d63fe` — validates explicit parent runs against recorded `WorkflowStarted` events and returns `not_found` when absent.

## Important findings

### I1 — run scoping was vacuously tested

- `7fedcd090` adds a two-generation parent fixture with different children.
- Explicit old-run and implicit latest-run requests now demonstrably project different generations.

### I2 — tests targeted an unshipped component

- `4c3e7f658` removed the dead exported `LegsSection` fixture wrapper.
- `dca160b4d` tests the shipped `LiveLegsSection` with injected history/live readers, including fallback wiring, live status, loading, and error rendering.

### I3 — attribution lag conflated with denial

- `1a79146ce`:
  - documents that `NotFound` may represent lagging attribution even though the child is named in durable parent history;
  - logs that case internally with parent and child IDs;
  - retains silent wire omission for anti-leak behavior;
  - keeps namespace denial silent.
- `c9a34d82e` pins that backend/non-authorization errors remain on the propagation path.

### I4 — sequential N+1 reads and undocumented quadratic refresh cost

- `1a79146ce`:
  - deduplicates child IDs before I/O;
  - performs child authorization and whole-history reads concurrently with `join_all`;
  - preserves parent-history discovery order in the response;
  - documents whole-history cost and the console’s possible cumulative O(N²) refresh traffic.
- `0768add28` extracted this projection from the route handler to remain Clippy-clean without bypasses.

### I5 — tight reconnect storm after lag

- `cff44b588` adds a 250 ms delay before lag reconnection.

### I6 — consumer request bodies were untested

- **Console:** `f64fd01d9` asserts the exact `namespace`, `workflow_id`, and `run_id` body and verifies the same `AbortSignal` reaches `fetch`.
- **CLI:** `00d8b0519` asserts exact `/workflows/children` and `/workflows/transcripts` bodies.
- `4421d4234` additionally asserts the exact retained-transcript request, including activity, attempt, run, and CLI-owned `last: 2000`.

### I7 — backfill/interleave paths lacked coverage

- `4421d4234` drives the shipping binary through real stream enumeration and retained HTTP backfill while retaining the post-attach child-live e2e.
- `844644232` pins workflow-type fallback for absent or empty display names.
- `0328e67a2` routes retained rows through the production ordering helper and pins parent/child interleaving by emitted timestamp.
- Existing pending-child and supersession coverage remains in the remediation suite.

### I8 — `Paused` normalization had no pin

- `92bef500c` explicitly tests that shared workflow-status normalization accepts `Paused`.

## Lower-severity findings

- **Header parse context:** `84df28eab`
  - distinct context for `x-aion-subject` and `x-aion-namespaces`;
  - authorization context names the bearer header without exposing the token.
- **Undiscussed `mpsc` capacity:** `cff44b588`
  - named `TAIL_OUTPUT_CHANNEL_CAPACITY`;
  - documented why 256 bounds burst memory while awaited sends preserve output.
- **Unsafe activity ID conversion:** `dca160b4d`
  - validates finite safe non-negative integers before transcript targeting;
  - invalid IDs render an alert instead of `Activity NaN`.
- **Silent lifecycle default:** `dca160b4d`
  - `LifecycleOutcome` is typed and handled exhaustively with no default branch.
- **Socket per superseded attempt:** `dca160b4d`
  - only the current attempt receives a transcript target/subscription.
- **Abort signal not forwarded:** `f64fd01d9`
  - signal reaches `ChildrenReadClient.listChildren` and `fetch`;
  - request test verifies object identity.
- **Empty display name rejected:** `f64fd01d9`
  - decoder accepts `""`;
  - console/CLI caller fallback remains available.
- **Timestamp epoch fallbacks:** `00d8b0519`, finalized in `54c1bb083`
  - removed `unwrap_or_default`;
  - fixed fixtures use chrono’s infallible nanosecond constructor.
- **Silent socket-fixture encode/decode exits:** `00d8b0519`
  - failures now emit operation-specific diagnostics rather than masquerading as attach failures.
- **`current_attempt` terminal coverage:** `a7985694a`
  - covers completed, failed, cancelled, and mismatched-attempt behavior.
- **Duplicate child and error propagation:** `a7985694a`, `c9a34d82e`
  - duplicate starts project once in first-discovery position;
  - non-authorization errors are not silently omitted.
- **HTTP module documentation:** `a7985694a`
  - adds `children` to the module-layout inventory.

## Scope and commit-chain requirements

- **Test extraction rationale:** `06b87a9cf`
  - records why the faithful WebSocket/router test extraction is retained despite not being required by the configured line rule.
- **RED-pin relocation:** `bc46a9729`
  - records that `ebb0d6b70`’s decoder moved from CLI live code to `aion-client::transport::transcript_ws::decode_frame`;
  - also records that the later console remediation supplies the per-leg history error channel absent from the original head commit.
- No reviewed commit was amended or rewritten.

## Verification performed

### Passed

- `bun install --frozen-lockfile`
  - 504 installs / 608 packages checked; no changes.
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy -p aion-cli -p aion-server -p aion-client --all-targets -- -D warnings`
- `cargo clippy -p aion-cli --all-targets -- -D warnings`
- `cargo clippy -p aion-server --all-targets -- -D warnings`
- `cargo test -p aion-client --no-fail-fast`
  - 72 library tests, conformance, error-mapping, and doctests passed.
- `cargo test -p aion-server --no-fail-fast`
  - 1,122 library tests plus all integration groups and doctests passed.
- `cargo test -p aion-cli --no-fail-fast`
  - full suite passed.
- `cargo test -p aion-cli --test tail_follow_e2e -- --nocapture`
  - 2 passed.
- All blocker-specific RED/GREEN commands recorded in their commit bodies.
- Ops console:
  - `bun run check`
  - `bun run typecheck`
  - `bun test`
  - all exited zero. Biome reports two existing cognitive-complexity warnings in unrelated update/doc-adjacency files, but no check errors.
- Final `git diff --check`
- Final orphan-process audit: none.

### Full standing battery

`./scripts/battery.sh` was started against final HEAD `0328e67a2`.

Observed before the reporting deadline:

- `rust:fmt` — exit 0
- `rust:clippy` — exit 0
- `rust:test` — still compiling/running the workspace

The full battery had not reached a terminal result by the reporting deadline, so I stopped it rather than leave a background process running. **I do not claim a full standing-battery pass.** No failure had been reported when it was stopped. The directly requested touched-crate suites, both Clippy resolutions, formatting, dependency, and complete console gates were independently run and passed.

## Disclosed non-goals / preserved state

- No server API behavior was changed to clamp `last`; documentation and CLI behavior were made truthful instead.
- No unrelated pre-existing console cognitive-complexity warnings were refactored.
- No reviewed commits were rewritten.
- No push, merge, or rebase was performed.
- Pre-existing untracked review/gate logs remain untouched.