<!-- EXTRACTION: final verdict message of the review-24 subagent, extracted
     2026-08-08 from session transcript agent-a6f61b568b0350116.jsonl (full JSONL not committed).
     The extraction is the last text block of the transcript, verbatim. -->

Tree restored and verified. Writing my verdict.

## (1) Verdict: **REFUSED** — 0 blocking, 0 major, 2 minor

All five R23 findings are genuinely fixed, the core is proven unchanged, and the measurement is re-proven live at my own hand. The lane is one clause from landing; nothing structural is wrong. I refuse only because this chain treats comment truth as a correctness property, and one sentence introduced *by the B1 fix itself* is over-wide in the same way as two claims this lane already retracted.

## (2) Findings

**MINOR 1 — `crates/aion/src/runtime/nif_continue_as_new.rs:345-346`: "sparing loses nothing" contradicts the same doc comment's own earlier paragraph.**

Exact claim: *"a kill here would race the pid-enders and usurp the deregister-only owners mid-teardown. The terminal is already durable either way, so sparing loses nothing."*

Exact code/context it contradicts, same comment, lines 303-306: *"Leaving it alive after a landed terminal is strictly worse than leaving it alive before one: it writes timers, activity records, children, and signals into a history that is already closed, concurrently with the successor… two writers on one history, which is load-bearing invariant 3."*

I verified the window is real: `lifecycle/continue_as_new.rs` records the terminal at :93, starts the successor at :111, and removes the predecessor's handle only at :150, with no `cancel_pid`. So between :93 and :150 a spared predecessor is registered, resolvable, *and* carries a durable terminal — exactly the "strictly worse" state. Sparing therefore does lose something (it forgoes cutting that window short); what makes it right is that a kill cannot be taken safely here, not that the loss is zero. Note this is the same species of over-wide claim the lane already retracted twice — `api.rs`'s "refusing cost nothing that was not already lost" and the "spared because it is recoverable" justification. Left as written, it could suppress a legitimate future re-examination of this seam.

Remedy (one clause): replace "so sparing loses nothing" with e.g. "so sparing costs no durability — what it does not close is the window named just above, and that residual is accepted because no kill here can be taken safely."

**MINOR 2 — `docs/design/aion-authoring/REVIEW-20-FINDINGS.md:400-401` and :411: the m1 defect recurs in miniature in the new gates section.**

The header reads *"POST-R23-FIX GATES (2026-08-07 22:36–23:20 local, transcript scratchpad/r23-fix-gates.transcript…)"*, and :411 lists suite attempt 2 beneath it. But `r23-fix-gates.transcript` ends at suite attempt 1 (EXIT 101) + shas + DONE, written 22:43:28; `r23f-suite-2.log` has mtime 23:22:04 — outside the transcript *and* outside the stated window. This is structurally what m1 was raised for. Mitigating: the header also names "logs r23f-*.log", and :411 states attempt 2's entry load (7.85) and no-foreign-cargo at its own hand, so the quiet property is not misrepresented.

Remedy: extend the window to 23:22 and add "attempt 2 was a manual re-run outside the harness" — the same clause m1 already earned at :338.

## (3) Verified at my own hand vs taken on trust

**Verified (with how):**
- All five pinned shas match, before and after my work; final `git status` identical to session start.
- **The core is unchanged — decisively.** I found R23's exact pinned baseline (`8431b572…`) preserved as `scratchpad/r23-nif.bak` and diffed it against the current file: the *only* delta is the doc paragraph at 327-346. Zero functional change to `record_continuation`, `outcome_must_end_the_process`, or the gate.
- `engine/api.rs`'s entire diff vs HEAD is doc-comment — no code change at all. `lib.rs`/`store_faults.rs` unchanged this pass.
- **B1, every mechanism claim:** `completion.rs:212` really carries the "Five ordinary terminal paths" paragraph and characterises them as removers; the partition is exhaustive and correct (5 = 2 pid-enders + 3 deregister-only). `terminate::cancel` `cancel_pid`s at :139 then removes at :154; the deadline handler `cancel_pid`s at :343 and its code literally says "6. Deregister LAST" at :380; `terminate::complete`/`fail` (:64/:96) only `registry.remove`, no `cancel_pid` anywhere in the file but :139; `continue_as_new` never touches the pid and orders terminal→successor→remove as claimed.
- "complete/fail test-only": all 8 in-repo call sites are inside `#[cfg(test)]` (api.rs tests start :777, sites at 1316-1676; terminate.rs tests start :190, sites :301/:473). I checked the census bound — they are `pub` and re-exported from `pub mod lifecycle`, so they *are* public surface, but no caller exists anywhere in `crates/`, `sdks/`, or `gleam/`. The doc's hedge "presently reachable only from tests" is accurate.
- **No stale claim survives:** "three writers" and "ends the process at its own hand" appear only at :331 as deliberate provenance naming the predecessor's error (which B1 required); no other site, and no surviving copy of the old enumeration or the old "recoverable" justification.
- **m3:** the only short lines in both paragraphs are paragraph terminators, not mid-sentence breaks (measured by line length, not eye).
- **m4:** exactly as specified — `torn_down: Cell<bool>` :646, `shutdown()` sets the flag *before* attempting (:700 then :701) and propagates via `?`, `Drop` :655 early-returns when set and reports on stderr :661. Orchestrator calls `harness.shutdown()?` at :1015; the only two `runtime.shutdown()` calls are inside the harness. Double-shutdown is impossible on both the success and failure paths. **Zero bypasses in the whole diff** — no `#[allow]`, `#[expect]`, `#[deny]`, `#[ignore]`, `cfg(any())`, `.unwrap()`, or `.expect()`.
- **Suite evidence reconciled** (my own laws demand this): 60 verdict lines = 58 binaries (1 lib + exactly the 57 integration files on disk) + 2 doctest result lines. No binary vanished, so the 1130/0 tally is not hiding a crashed binary. Attempt 1's single failure is `outbox_happy_path_completes_with_n_terminals_in_order` with `Backend("SQLite failure: bad parameter or other API misuse")` — the #85 signature, in a file the lane does not touch.
- **All gate evidence ran against the pinned bytes:** every source mtime is ≤ 22:18:32, before the 22:35+ gates and the 23:22 suite.
- Supporting arithmetic: `fail_appends_after` short-circuits so the skip budget is consumed without touching the refusal budget; `take_failure` uses `checked_sub(1)`; `register_live_run` makes 1 append for `None` and 2 for `Outstanding`, so CONTROL 2's `(1,1)` and TREATMENT A's `(3,1)` are both exactly right; `retire_run_deadline` appends `TimerCancelled` only when an outstanding deadline exists, so TREATMENT B's witness genuinely discriminates.

**Taken on trust:** R23's per-claim list for the pre-fix bytes (its remit, and its core conclusion I independently corroborated by mutation); the #85 and #149 flake attributions as known-on-main; the historical r22 battery load readings. I could not byte-diff `nif_continue_as_new_tests.rs` against R23's pin (no copy survives) — I closed that gap by mutation instead.

## (4) Gates and mutations I re-ran

Box sequencing honored throughout: I found another seat's `cargo test --workspace` (manifold worktree) live and held ~9 minutes until it cleared, never touching it. My first waiter had a self-match bug — its own command line contained the literal `cargo test --workspace`, so `pgrep -f` matched itself and it could never exit; I stopped it and re-armed on `pgrep -x cargo` after confirming that actuator sees the foreign run and not itself.

| Gate / mutation | Result | 1-min load |
|---|---|---|
| Module baseline `-p aion-rs --lib runtime::nif_continue_as_new` | EXIT 0, **5/0** | 26.3 (decay tail) |
| **Mutation: TerminalOnly arm-skip** on the live gate | **EXIT 101 — RED**, killed solely by `the_nif_gate_ends_a_live_process…` at `nif_continue_as_new_tests.rs:860` (TREATMENT A), diagnosis `terminal in history = true`, `timer retired = false`, `unspent append failures = 0` | 19.5 |
| Restore | **byte-identical**, `5c36b302…`, `diff` clean vs backup — restored by writing bytes, not `mv` | — |
| Module re-run on rebuilt restored tree | EXIT 0, **5/0** | 12.1 |
| `cargo fmt --check` | EXIT 0, empty output | — |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | EXIT 0, real work (`Checking aion-rs` present — m2's rule applied; I touched the file to force it) | 10.6 |
| `cargo clippy --workspace --all-targets -- -D warnings` | EXIT 0, 5 `Checking` lines, no warnings | 10.4 |

The mutation is the load-bearing result: it proves the measurement survived the `GateHarness` decomposition and the m4 teardown rework — the gate still genuinely consumes `Ok(TerminalOnly)`, and the fault arithmetic is confirmed live, not merely reasoned about.
