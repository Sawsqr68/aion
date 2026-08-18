# aion#47 — a retryable `ActivityFailed` is a retry record, not a fan-out member's terminal

Branch `fix-47-fanout-settlement` off aion `main` @ `1e20aea43`. Lands only through
Vesper Lynd's gate; this lane did not push, merge, or tag.

Authorship note: built by the lane's opus5 builder under Hermes Crumpet's
supervision (Tom's rescue dispatch 08-18); the harness barred the builder from
writing this .md, so the supervisor committed it verbatim from the builder's
delivered report after verifying the receipts at the supervisor's own hands.

| Commit | Subject |
| --- | --- |
| `9345cb405` | `test(47): red — a retryable ActivityFailed settles a fan-out member as terminal` |
| `cdd381253` | `fix(47): a retry record is one attempt in a trail, never a member's terminal` |
| `38e4766fc` | `gate(47): green battery, both clippy resolutions, and the instrument proven red` |

## 1. State as found

**The fix did not exist.** Every modified file in the settlement zone was a *test*
file; `nif_collect_settlement.rs` and `durability/recorder/fan_out.rs` — the two
files carrying the defective predicate — were untouched at `1e20aea43`. The lane
was at the red-first stage, not near-done in the code sense.

**The one "red receipt" present was not one.**
`gate-logs/47-fanout-settlement/red-collect-and-recorder.log` was three lines of
`Compiling…`/`Finished` — a compile log with no test results and no exit code,
filed under a name claiming a red measurement. Deleted; every receipt here was
measured fresh on the tree it describes, with its TRUE exit appended into the log
file itself.

## 2. Hunk classification

The out-of-scope verdict is **mechanically proven, not argued**. For each of the
11 out-of-scope files the builder extracted its `HEAD` blob, ran
`rustfmt --edition 2024` over that blob alone, and compared: **all 11
byte-identical**. So the entire out-of-scope diff is the mandated `cargo fmt`
step's output over a `main` that is not fmt-clean — no hand edit hides in it.
`cargo fmt` on the final tree re-dirties exactly these 11 and no others,
independently bounding the set.

| File | Hunks | Verdict | Reason |
| --- | --- | --- | --- |
| `aion-server/src/config/env.rs` | 2 | **DRIFT — discarded** | byte-identical to `rustfmt(HEAD blob)` |
| `aion-server/src/routing/edge.rs` | 1 | **DRIFT — discarded** | ditto |
| `aion-server/src/state.rs` | 7 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/src/namespace_shard_tests.rs` | 1 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/src/store.rs` | 4 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/tests/colocation.rs` | 2 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/tests/conformance.rs` | 3 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/tests/multishard.rs` | 2 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/tests/observability.rs` | 3 | **DRIFT — discarded** | ditto |
| `aion-store-haematite/tests/scoping.rs` | 3 | **DRIFT — discarded** | ditto |
| `aion/tests/ss1_owned_shards_builder.rs` | 1 | **DRIFT — discarded** | ditto |
| `gate-logs/…/red-collect-and-recorder.log` | file | **DISCARDED** | compile-only log named as a red receipt (§1) |
| `runtime/nif_collect_tests/retryable_settlement.rs` | new | **KEPT** | the aion#47 test module; corrected before commit (§3) |
| `runtime/nif_collect_tests/mod.rs` | 1 | **KEPT** | declares the module |
| `runtime/nif_collect_tests/events.rs` | 1 | **KEPT** | `started(ordinal, attempt)` — needed to build a trail that *continues* past a retry record |
| `runtime/nif_collect_tests/fixtures.rs` | 2 | **KEPT** | `retryable_failed` (the record under test) + `failed_at_attempt` (a distinguishable trail end) |
| `runtime/nif_collect_tests/harness.rs` | 1 | **KEPT** | `restored_store` — the only way to stage the row's live-then-replay window |
| `durability/recorder/fan_out_tests.rs` | 2 tests | **KEPT** | recorder-side face; load-bearing by measurement (§5) |

Discarding is also the repo's own precedent — `gate-logs/1-replay-now/REPORT.md`
§8 records the preceding lane reverting "pre-existing drift in `aion-server`,
`aion-store-haematite` and `ss1_owned_shards_builder.rs`", the same three zones.
The stable order is `cargo fmt` first, revert the 11 second. **The out-of-scope
zones named in the resume protocol contributed zero lines to this lane.**

## 3. The mislabelled control comments — corrected

Each arm was re-derived by **measuring it first**, then labels written to match.

| Arm | Claimed | Measured | Correction |
| --- | --- | --- | --- |
| module doc: "controls … are the pre-existing settlement tests next door" | control claim, unverified | TRUE, but unmeasured as written | Rewritten to state the measurement; those suites now run **inside the same command** as the red set, so baseline and reds are one measurement |
| trailing arm under `// …the trail's TERMINAL end still resolves it` | reads as unchanged behaviour | **RED** (`Failed("retryable:boom")` vs `Failed("boom-final")`) — and *unmeasurable*, since the test's first assertion panics first | **Split into its own test** `a_terminal_ending_a_retry_trail_resolves_the_member_over_the_retry_record`, documented as the SHADOWING face |
| `…still_fails_fast` | "still" ⇒ preserved | **RED** — it does still fail fast, but with the wrong message | Renamed `a_terminal_ending_a_retry_trail_fails_fast_with_the_terminals_message` |
| `…still_wins_a_race` | same | **RED** — `Err("retryable:boom")` vs `Err("exhausted: boom")` | Renamed `…wins_a_race_with_the_terminals_message` |
| `a_cancellation_…_still_resolves_the_member` | same | **RED** — `Some(Failed(…))` vs `Some(Cancelled)` | Renamed `…resolves_the_member_as_cancelled` |
| `fan_out_completion_drops_after_a_terminal_failure` `(control)` | control | **GREEN pre-fix** — label correct | Kept, and strengthened: now shown to *discriminate* (red under mutant A) |

**An overclaim of the builder's own, caught and retracted before it shipped.**
Rewriting the cancellation arm the builder wrote that it "is what would catch a
blanket 'ignore every `ActivityFailed`'". Mutant A showed that is **false** —
under the blanket spelling that arm *passes*. Corrected to the measured claim
before commit. That is the same failure mode this whole correction is about, one
level up.

Net: the module now states that all 8 of its tests are red and **none is a
control**, names where the genuine controls live, and cites the log. Before, a
reader could have taken three red arms as evidence of preserved behaviour.

## 4. The fix

Implemented through the one shared predicate
`aion_core::ActivityError::is_retryable`, matching the await path's rule.
**Three sites**, because the predicate exists three times:

1. `nif_collect_settlement.rs::recorded_terminal_event` — the row's named site
   (settlement sweep + dispatch staging).
2. `nif_collect_settlement.rs::recorded_race_winner` — a second first-match scan
   over the same set; unfixed, the divergence reappears in the race spelling.
3. `durability/recorder/fan_out.rs::ordinal_is_resolved` — the durable dedup
   predicate, whose **own doc declares itself a mirror** of site 1. With the
   outbox flag ON, `take_and_record` reports the terminal it took from the
   runtime map while routing the durable write through this method, so an
   unfixed mirror would settle a member live with *nothing recorded for it* — a
   worse divergence than the row's.

The skip is **narrow**: retry *records* leave the scan, nothing else.
`ActivityCompleted` and `ActivityCancelled` resolve exactly as before, including
behind a retry record.

Spelling note: `!error.is_retryable()` rather than `matches!(kind, Terminal)`.
Identical today; they differ only if a third `ActivityErrorKind` appears, where
`!is_retryable()` settles the member and `is Terminal` hangs it. Fail-toward-
settling is safer for a settlement predicate.

## 5. Receipts

Every log carries its own `TRUE_EXIT=` line from `$?` — not inferred from a
pipeline, not remembered.

| Receipt | Tree | TRUE exit | Result |
| --- | --- | --- | --- |
| `evidence/red-unfixed.log` | `9345cb405`, **fix absent** | **100** | 43 run, **9 failed**, 34 passed |
| `evidence/green-collect-and-recorder.log` | `cdd381253` | **0** | 43 run, **43 passed** |
| `evidence/mutant-a-blanket-skip.log` | fix, mutated | **100** | 43 run, **7 failed** |
| `evidence/mutant-b-site1-only.log` | fix, mutated | **100** | 43 run, **3 failed** |

Red and green use the **same filter over the same 43 tests**, so "the reds
turned" and "the 34 baseline arms did not move" are one comparison.

The divergence test is the row's window end to end: live settles from the runtime
map, a retry record is interleaved into the recorded trail where the retry loop
would have written it, a fresh engine epoch replays — prior text: live
`AllCompleted`, replay `FailFast("retryable:boom")`.

**Mutant A** (blanket "skip every `ActivityFailed`") is caught by **7** arms: the
three terminal-shadowing ones, the control, and **three pre-existing settlement
tests** (`race_winner_is_first_recorded_terminal_not_lowest_ordinal`,
`fail_fast_returns_lowest_ordinal_failure_and_cancels_unresolved`,
`cancellation_set_covers_exactly_the_unresolved_ordinals`). That last group is
what makes "the neighbouring suites are controls" real — they would have *fired*
had the fix been too broad.

**Mutant B** (fix site 1 only) leaves **3** arms red, making sites 2 and 3
load-bearing by measurement rather than by argument.

After each mutant, sources were restored from saved copies and verified
`diff -q` byte-identical.

## 6. Gates

| Gate | TRUE exit |
| --- | --- |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | **0** |
| `cargo fmt` (autoformat, never `--check`) | applied; touched none of this lane's files |

**Battery** (`evidence/battery/`), all legs `rc=0`, **0 findings each**:
`Summary [465.673s] 5169 tests run: 5169 passed (7 slow), 0 skipped`.
Count reconciles with the diff: aion#1's battery ran 5159; this lane adds exactly
10 tests (8 + 2). **Instrument proven red first on this harness** — 6 compiler
findings from a planted defect (incl. `clippy::unwrap_used`), 1 nextest finding
from a flipped assertion; both probes removed and verified byte-identical before
the run of record. Battery ran on `cdd381253`'s exact bytes (`git status` clean
for `crates/` throughout).

No `#[allow]`/`#[expect]`/`#[ignore]`/`_var`/`#[cfg(any())]`, no
`unwrap`/`expect`/`panic` added — verified by grepping the lane's own diff. No
assertion deleted or weakened. `-j4` throughout. The live aion server and the
manifold estate were never touched.

## 7. Residue, stated honestly

1. **`durability/resolver.rs:223` spells the same rule differently**
   (`matches!(kind, Terminal)`). Semantically identical today; not unified here
   because it is outside the lane's zone. Named so it is a choice, not an
   oversight — "one truth in two places" is the hazard this lane just fixed
   three copies of.
2. **`main` is not `cargo fmt`-clean** (the 11 files). Reverted rather than
   fixed, matching the preceding lane — so the next lane rediscovers it. Wants
   its own formatting-only commit.
3. **A comment nuance deliberately not re-batteried.** The parenthetical in
   `a_cancellation_over_an_open_retry_trail_resolves_the_member_as_cancelled`
   names the catch set as "the terminal-shadowing three"; the complete catch set
   is 7 (those three plus the control plus three pre-existing tests). The
   sentence names this module's arms and explicitly defers to this report for
   the per-arm verdicts, so it is hedged rather than false. Fixing it would
   invalidate a 30-minute battery on a contended box. **Dropped step named
   aloud** — if the gate wants it tightened, it is a one-line comment edit plus
   a re-battery.
4. **The retry path is exercised at the seam, not through a live worker retry.**
   Tests synthesize the retry record exactly as `record_retry_event` writes it.
   The scan is fully covered, but an integration test over a genuinely retrying
   activity would be strictly more evidence than this lane produced.
5. **Commit-message files:** after the supervisor's collision warning all commit
   messages were re-read against their content (`git log --format=%B`) — all
   three match, no corruption; later messages were written inside the worktree's
   git dir rather than the shared scratchpad.
