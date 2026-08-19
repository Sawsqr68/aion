# Review pass 2 — aion #244 fix leg (branch 244-child-visibility, tip 0328e67a2)

Reviewer: fresh Opus-grade pass dispatched by Waffles, 2026-08-16.
NEW-B1's structural claim independently re-verified at the bytes by Waffles before routing
(live.rs: `selected_run_seen` set/checked only in the WorkflowStarted arm at 129-143; child
discovery 145-167 and observe_attempt 168 ungated).

---

# VERDICT: DO-NOT-LAND

One new blocker, introduced by the B4 remediation itself, plus one smaller new defect and two cosmetic notes. Everything the prior review named — B1–B6, I1–I8, all 11 lower items, and both commit-chain requirements — is genuinely discharged at the bytes. This is close; the remaining work is narrow and well-defined.

**Clarification of record:** the file named to me as the prior review (`gate-logs/244-child-visibility/OPUS-REVIEW.md`, 707 lines) is the round-one FAIL review (tip dd5073bf2, findings F1–F19). The review this remediation answers — DO-NOT-LAND, B1–B6/I1–I8/11 lower, range 051b547a6..b0214c0ef — lives at `.worktrees/244-child-visibility/gate-logs/244-child-visibility/OPUS-REVIEW-20260816.md` and verbatim in `gate-logs/244-fix-leg/input.json`. I read both in full and verified against the B/I review, which is what the fix-leg charter mandated.

## Append-only honesty — VERIFIED

- `git -C <wt> rev-list --count b0214c0ef..0328e67a2` → **25**, `--merges --count` → 0, `--first-parent | wc -l` → 25 (fully linear).
- `git merge-base b0214c0ef 0328e67a2` → **b0214c0ef** — the reviewed base is an ancestor of the tip, which is the strong proof no reviewed commit was rewritten.
- `git status --porcelain` → only untracked `gate-logs/244-child-visibility/` (disclosed, preserved). `git diff --check` → exit 0.
- Branch ref and HEAD both resolve to `0328e67a24384d8adf895b82c542360afd89a2b1`. Author timestamps monotonic 04:15:10 → 05:43:37 +1000.

## Blockers B1–B6

**B1 (tail prints `null`) — DISCHARGED, red-first authentic.** RED `2648cf5bd`: adds a shipping-binary e2e asserting empty stdout; quoted red `left: "null\n" / right: ""` matches the assertion exactly and matches the pre-fix path (tail returns `Ok(Value::Null)`, `main.rs` piped every client command through `print_json`). GREEN `001dcb3ae` changes **production** `main.rs` only — routes `Tail` around `print_json` on the AWL precedent the review itself cited — the test is untouched. Lint follow-up `54c1bb083` extracts `run_client_command` (main.rs:836-853 at HEAD) without suppressions; main.rs net **shrank** 2771→2739. The test later evolved (`4421d4234`) to assert backfill content present and no `null` line — still pins B1.

**B2 (false "No activity transcript stream recorded" on failed read) — DISCHARGED.** RED `4c3e7f658` is an honest seam-red: the shipped `LiveLegsSection` had no injectable-reader seam, so the test failed on the real `useNamespace` hook — quoted red matches. GREEN `dca160b4d`: `LegsSection.tsx` at HEAD has a four-state `LegHistoryRead` (`disabled|error|loading|success`, lines 91-95), `role="alert"` error rendering with the cause (189-193), distinct loading (186-188), and the empty-transcript sentence gated on `history.state === 'success' && leg.runId !== null` (209-211). Pinned against the **shipped** container with injected readers (`legs.test.tsx:28-55`).

**B3 (pending child never goes live) — DISCHARGED, with a named decision.** `useChildren.ts:17-22,39`: `pendingChildRefetchInterval` returns 1000 while any child has `runId === null`, else `false`; wired to `refetchInterval`; documented as a responsiveness/load decision. This is a poll, and the console's own toolkit has a workflow-events socket the CLI's `monitor_pending` equivalent could use — I rule the 1 Hz bounded, self-extinguishing poll **acceptable for this slice as a named decision** (it is real data, terminates when the run starts, and is disclosed in code, commit, and report), with socket-first pending-follow named as legitimate follow-up work, not waved away.

**B4 (attach race loses events) — DISCHARGED FOR ITS PIN, BUT SEE NEW-B1.** RED `ef143fa1c` (red-first authentic): 3-line fixture strengthening requiring `per_workflow/resume_from_seq == 1` on the parent socket; pre-fix code subscribes live-only (`subscribe_workflow`) so the fixture refuses attach and the existing e2e fails with exactly the quoted "tail did not attach its parent workflow socket". GREEN `cff44b588` changes **production** `tail/live.rs` only: `subscribe_workflow_from(&id, NonZeroU64::MIN)` (line 113 at HEAD), `known_children` seeded from the HTTP projection (39-51), idempotent `observe_attempt` retained, continued-as-new guard now waits for the selected run (`selected_run_seen`, 129-143). The e2e drives the shipping binary and passes only with the durable-cursor subscribe. The **payload-event half of the generation claim is not implemented** — that is NEW-B1 below.

**B5 (false truncation notice; `last` semantics) — DISCHARGED.** RED `0e92c6ee6` is a compile-error red (`backfill_notice` doesn't exist yet) — weaker than a behavioral red, noted, but honest about what it is and the pin's assertions are behavioral (requires durable-seq range, forbids "N of M events" and "server's published transcript window limit"). GREEN `84df28eab` deletes the false notice and produces the seq-range notice from `first_store_seq` (tail.rs:351-368 at HEAD). Docs `23dcd1644` land at both surfaces and I verified them against the server bytes: `transcripts.rs:167-178` applies `last` unbounded and clamps only `limit` to `1..2000` — exactly what API.md:66 and the README now say. Ruling on the choice below. One gap: NEW-2.

**B6 (unknown run answers 200 empty) — DISCHARGED, red-first authentic.** RED `7fedcd090`: `unknown_parent_run_is_not_reported_as_an_empty_child_list` asserts 404; quoted red `left: 200 / right: 404` is exactly what pre-fix `run_segment`'s `&[]` fallback produces. GREEN `79e2d63fe` changes **production** `children.rs` only: explicit `run_id` validated against recorded `WorkflowStarted` events, `not_found` when absent (children.rs:102-111 at HEAD). Anti-leak intact — the parent gate has already run before this answer.

## Important findings I1–I8

- **I1 — DISCHARGED.** `7fedcd090` adds `explicit_and_implicit_parent_runs_project_different_generations`: two-generation parent, different children per run; explicit old run → old child, implicit latest → new child (children_tests.rs:122+). The word "run" in this API is now pinned.
- **I2 — DISCHARGED.** Dead `LegsSection`/`LegView` fixture wrapper deleted; only `LiveLegsSection` is exported and it is what `WorkflowDetailView` renders and what the test renders with injected `LiveLegReaders` (`4c3e7f658` + `dca160b4d`).
- **I3 — DISCHARGED.** `1a79146ce`: module doc states the attribution-lag ambiguity (children.rs:13-16); `NotFound` logged via `tracing::warn!` with parent+child ids before omission (156-165); denial stays silent; `c9a34d82e` extracts `is_omitted_child_error` (NamespaceDenied only) and pins that backend errors propagate.
- **I4 — DISCHARGED.** `1a79146ce`: dedupe before I/O (`seen` HashSet), `join_all` concurrency with discovery order preserved, doc extended to cover the console's cumulative O(N²) refresh cost (children.rs:27-32). `0768add28` splits `project_candidates` out of the handler to stay Clippy-clean without suppression.
- **I5 — DISCHARGED.** `LAG_RECONNECT_DELAY = 250ms`, documented, applied at live.rs:391 (`cff44b588`).
- **I6 — DISCHARGED.** Console: `f64fd01d9` asserts exact body + `AbortSignal` object identity. CLI: e2e fixtures now `assert_eq!` the exact `/workflows/children`, `/workflows/transcripts`, and `/workflows/transcript` bodies including `"last": 2_000` (tail_follow_e2e.rs:295-334; `00d8b0519`, `4421d4234`).
- **I7 — DISCHARGED.** `4421d4234` makes fixtures return a real stream + backfill event; the no-follow e2e asserts `retained-backfill-parent` reaches stdout; `844644232` pins label fallback for absent/empty display names; `0328e67a2` routes the backfill sort through `order_backfill` and pins ordering. (The interleave pin is a unit pin on the production helper, not an end-to-end two-stream interleave — honest, adequate, noted.)
- **I8 — DISCHARGED.** `92bef500c` adds `client-normalize.test.ts` pinning `requireWorkflowStatus('Paused')`.

## Lower items (all 11) — ALL DISCHARGED at the bytes

header contexts incl. token-safe bearer message (`84df28eab`); `TAIL_OUTPUT_CHANNEL_CAPACITY` named+documented (`cff44b588`); `Number.isSafeInteger` activity-id validation with `role="alert"` (LegsSection.tsx:253-258/194-198); `LifecycleOutcome` typed union + exhaustive switch, no default (types.ts:20, LegsSection.tsx:274-291); transcript subscription only for the current attempt (`subscribe={attempt.state === 'current'}`, target null otherwise); abort signal threaded to `fetch` with identity test; `nullableString` accepts `''` with caller fallback pinned; epoch fallbacks removed (`from_timestamp_nanos` / `ok_or`); socket-fixture silent exits replaced with operation-specific `eprintln!` diagnostics (e2e:344-349,400-409,413-428); `current_attempt` terminal + mismatch + duplicate-child + error-propagation coverage (`a7985694a`, `c9a34d82e`); `mod.rs:6` children inventory line. Commit-chain requirements: `06b87a9cf` supplies the extraction rationale, `bc46a9729` records the decoder red-pin relocation **and** corrects the head commit's overclaimed console error channel. Both append-only, as charged.

## Scope, standards

- Full diff `b0214c0ef..0328e67a2`: 17 files, +869/−240, every file a slice surface (CLI tail + e2e, server children route + tests + mod doc, console legs/children hooks and tests, README/API.md). **Nothing out of scope.** No Cargo.toml/lock churn.
- Suppression scan over all added lines (`#[allow`/`#[expect`/`#[ignore`/`cfg(any())`/biome-ignore/eslint-disable/ts-ignore/ts-expect-error/todo!/unimplemented!/dbg!): **zero hits**. Unwrap/expect/panic/`let _ =`/`unwrap_or_default` in added Rust: **zero hits** (the one RED-commit `.expect` in a test was itself remediated to `ok_or` before the tip). File-length law held (tail.rs 520 raw lines includes its `mod tests` from ~445; code well under 500).
- Constants added: 256 (documented, backpressure-correct), 250ms (review-demanded delay, documented), 1s pending poll (documented decision), 2000 (CLI-owned request bound aligned to the server's published `limit` clamp constant, documented at every surface). All named; none silent.

## Ruling (a) — server `last` unclamped, CLI-owned 2000 bound: **RIGHT CALL, and honest at every surface**

Clamping `last` server-side would be an API behavior change silently truncating other callers — outside this slice and itself the "arbitrary cap" house law warns against. The prior review offered exactly two branches; the builder took "stop describing the CLI's choice as a server window" and completed it everywhere I checked against the bytes: `transcripts.rs:167-178` (the ground truth), `API.md:66` ("`limit` is clamped to 1..2000; `last` is currently unbounded by the server, so callers … must choose and send their own bound; `head_seq` is the next durable `store_seq`, not a retained-event count"), `aion-cli/README.md:108-111` ("CLI-owned bound, not a server guarantee"), and the runtime notice itself ("the CLI's 2000-event request bound"). No surface anywhere still states a server window. Whether an unbounded server `last` should exist at all is a legitimate server-owned design question — for a future slice, on record, not a landing condition here.

## Ruling (b) — stopped builder battery + independent server-run gate: **SUFFICIENT**

The builder's honesty is exemplary — the stopped run is disclosed and not claimed. The independent gate (`gate-logs/244-fix-leg/`) covers strictly more than the stopped run would have: 11 legs (js-deps ×2, fmt, clippy workspace **and** per-crate, cli/server/client full suites `--no-fail-fast`, console check/typecheck/test) all `EXIT=0` read from the log bytes, plus the full standing battery — and critically the battery log records `HEAD 0328e67a2` at line 2 and the built server banner stamps commit `0328e67a24…` — the gate provably ran at the delivered tip, not a stale ref. Battery: 22/23 green; sole red `gates:costly-announced` (`parked_timer_osproc_kill9_failover_e2e` announced nothing) — the known #251 announce-race, touching parked-timer failover scripts this diff never goes near. Coverage claim stands.

## NEW FINDINGS

**NEW-B1 — BLOCKER — replayed earlier-generation events are attributed to the selected run.** `crates/aion-cli/src/tail/live.rs:145-168` (`monitor_source`). The B4 fix replays every source from seq 1, and the `selected_run_seen` gate exists **only** inside the `WorkflowStarted` arm (its 3 uses are all at lines 112-143). Every non-`WorkflowStarted` event replayed from generations **before** the selected run falls through to child discovery and `observe_attempt` ungated — and `EventEnvelope` (event.rs:15-25) carries no run id, so positional gating is the only available mechanism and it is not applied. Concrete consequences on any multi-generation workflow (a first-class domain concept — `WorkflowContinuedAsNew`, `run_segment`, and the I1 fixture itself constructs exactly such a parent): (1) an old generation's `ChildWorkflowStarted` is not in `known_children` (the HTTP projection is run-scoped — that is B6/I1's whole point), so the tail announces and follows children that are *not legs of this run*, contradicting the server projection on the same screen; (2) old-generation `ActivityStarted` with attempts above the seeded `current` prints a false "superseded" line and spawns a transcript socket at coordinates (selected run + old attempt) that don't exist; (3) the poisoned `current` map then suppresses genuine later attempts of the selected run (`observe_attempt`'s `prior >= attempt` skip, live.rs:267-270). The delivery report's sentence "Continued-as-new handling now ignores older replayed generations until the selected run is observed" is true only of the generation *markers*, not the generations' payload events — a claim without a matching diff for its second half. Pre-fix live-only subscription never saw old generations, so this is remediation-introduced. Fix shape is small: skip child-discovery and `observe_attempt` in `monitor_source` while `!selected_run_seen` (history is seq-ordered, so everything before the selected run's `WorkflowStarted` is prior-generation by construction), plus a red pin with a two-generation fixture — the server tests already show exactly how to build one.

**NEW-2 — must fix with NEW-B1 (small) — a fully-empty backfill with a non-zero head is silent.** `tail.rs:355-368`: `backfill_notice` returns `None` when `events` is empty (`first_store_seq?`), so a stream whose retained records vanish entirely between `/workflows/transcripts` enumeration and the fetch (retention trim race) prints nothing — a quiet path on the exact surface B5 was reopened for. The pre-remediation code noticed this case (with wrong words); the remediation's truthful notice doesn't fire at all. Narrow window, but "every quiet path says why it is quiet" — emit "no retained events; head at durable seq N; earlier records trimmed by retention" when `events.is_empty() && head_seq > 0`.

**NOTE (cosmetic, not code):** commit bodies of `84df28eab`, `1a79146ce`, `a7985694a`, `c9a34d82e`, `92bef500c`, `f64fd01d9`, `00d8b0519`, `4421d4234`, `844644232`, `54c1cb083`-family contain literal `\n\n` escape sequences instead of newlines — a builder tooling artifact. Content is intact and honest; the next leg's commits should not repeat it.

**NOTE (named accepted risk):** the console's 1 Hz pending-child poll (B3) — accepted for this slice as documented above; socket-first pending-follow is real follow-up work, on the record.

## Commands executed (all read-only; git via `-C`, never cd)

- `git -C <wt> log --oneline b0214c0ef..0328e67a2` / `rev-list --count` (25) / `--merges --count` (0) / `--first-parent | wc -l` (25) / `merge-base` (b0214c0ef) / `status --porcelain` (untracked gate-logs only) / `diff --check` (**exit 0**) / `rev-parse HEAD 244-child-visibility` (identical shas) — all exit 0.
- `git -C <wt> diff --stat b0214c0ef..0328e67a2`; suppression grep over added lines (**no hits**, grep exit 1 = not found); unwrap/expect/panic/`let _ =` grep over added crate lines (**no hits**).
- `git -C <wt> show` on all 25 commits' relevant diffs/bodies (individually cited above).
- Gate-log reads: 11 leg logs each ending `EXIT=0`; `gate-full-battery.log` ending `1 of 23 gates failed. EXIT=1`, sole FAIL `gates:costly-announced`, `HEAD 0328e67a2` recorded in-log.
- No tests or builds executed by me: every discharge was verifiable at the bytes, and NEW-B1 is a source-level finding proven by the match structure of `monitor_source` (the same class of proof the round-one review used for F1).

## What must happen before this lands

1. NEW-B1: gate `observe_attempt` and parent child-discovery on `selected_run_seen` in `monitor_source`, red-pinned with a two-generation fixture, committed red-first, append-only.
2. NEW-2: the empty-backfill/non-zero-head notice, pinned in `backfill_notice`'s test.
3. Re-run the touched-crate suites and the battery legs the fix invalidates (aion-cli suite + e2e at minimum); fresh Opus pass on the (small) delta.

Everything else is done, and done properly — this remediation leg is the strongest work this branch has seen; the verdict is DO-NOT-LAND only because a real silent-wrong-answer path shipped inside the B4 cure, and this slice's entire reason for existing is that streams must not lie quietly.
