<!-- EXTRACTION: final verdict message of the review-25 subagent, extracted
     2026-08-08 from session transcript agent-a8dbe3ee8693291ed.jsonl (full JSONL not committed).
     The extraction is the last text block of the transcript, verbatim. -->

## VERDICT: REFUSED — 0 blocking / 0 major / 2 minor

Both minors are in the record file, not the code. **The code half is clear**: R24 fix 1 is genuine, minimal, and every factual claim in it holds. Both remedies are one-line doc edits in an untracked file that sits on no build graph — **no re-gate is required**.

---

### FINDINGS

**MINOR 1 — a gate timestamp that cannot have been observed.**
`docs/design/aion-authoring/REVIEW-20-FINDINGS.md:461`

- Exact claim: `## POST-R24-FIX GATES (2026-08-07 ~23:56 local, detached pid 96934; transcript scratchpad/r24-fix-gates.transcript, logs r24f-*.log): ALL GREEN`
- Exact fact: the four gate artifacts are `r24f-fmt.log` 23:46:55, `r24f-module.log` 23:47:20, `r24f-clippy-p.log` 23:48:09, `r24f-clippy-ws.log` 23:49:30, transcript 23:49:30 (all `-t "%Y-%m-%dT%H:%M:%S%z"`, AEST). The file's own last-write mtime is **23:51:46**. `~23:56` post-dates every artifact *and* the document's own last write, so no reading of it — gate window, or time of writing — can be true. It has the shape of a 4↔5 transposition of 23:46.
- Remedy: replace `~23:56` with `23:46–23:49`.
- Why it counts: this is the attestation for the change being landed, in the section whose immediately preceding entry (R24 minor 2) was itself a fix for time-attribution honesty in this same document.

**MINOR 2 — the R23 window excludes two of the gates it cites.**
`docs/design/aion-authoring/REVIEW-20-FINDINGS.md:400`

- Exact claim: `## POST-R23-FIX GATES (2026-08-07 22:36–23:22 local; harness transcript … logs r23f-*.log)`
- Exact fact: `r23f-fmt.log` 22:35:21 and `r23f-module.log` 22:35:58 both fall before 22:36, and the transcript's *first* step is that fmt gate. The window as stated does not contain the fmt or module gates.
- Remedy: `22:35–23:22`.
- Note: carried over from the pre-R24 text (`22:36–23:20`); R24's fix extended only the end. R24 had the full mtime listing in front of it — including `22:35:21` — and did not correct the start.

---

### VERIFIED AT MY OWN HAND

**Pinned SHAs.** All five exact via `shasum -a 1` (the pins are SHA-1, not SHA-256), and re-verified unchanged at the end of the review. No mismatch on the four "unchanged" files.

**Fix 1 — the delta itself.** `diff -u scratchpad/r24-nif-baseline.bak crates/aion/src/runtime/nif_continue_as_new.rs` (baseline sha1 = `5c36b302…`, the brief's pre-fix pin): **exactly one hunk**, doc-only, `/// terminal is already durable either way, so sparing loses nothing.` replaced by the four-line clause. Nothing else in the file moved.

**(a) Old clause gone.** `grep -rn "loses nothing"` tree-wide: no hit in `nif_continue_as_new.rs`. Remaining hits are pre-existing files outside this diff (`nif_timer_fire.rs:220,290`, `aion-server/src/api/http/unrecoverable.rs:60`, examples, brief JSON) plus the record's own account of the fix at `REVIEW-20-FINDINGS.md:449`.

**(b) The new clause's claims hold.**
- "the still-registered window named just above" — named 3–6 lines above at `nif_continue_as_new.rs:340-343`.
- The window is real and the terminal is durable **before** it opens: `lifecycle/continue_as_new.rs:92-99` (awaited `record_workflow_continued_as_new`) → `:111` (`start_workflow_with_options`) → `:150` (`context.registry.remove`).
- The named residual is *exactly* the residual, not a wider one: this seam is only reachable while the handle is still registered — `NifContext::new` → `resolve_handle_with_birth_wait`, which returns `NifContextError::UnknownProcess` when the registry has no active handle (`nif_context.rs:24-27, :103`). Once the handle is removed the NIF never reaches `HistoryShape` at all.
- "This seam cannot tell which owner recorded the terminal" holds structurally: `HistoryShape` carries only `reason: String` (`durability/error.rs:35-38`), and the terminal-already-recorded refusal is raised at `nif_continue_as_new.rs:479-487` with no owner information attached.
- No new over-wide phrasing anywhere in the diff: scanned every added line for `nothing`/`free`/`costs no`/`no cost`. The only two hits outside the fixed clause are `engine/api.rs:266-275`, which are explicit **retractions** of the old `"refusing cost nothing that was not already lost"` claim, not new ones.

**(c) The R23 B1 fix has not regressed.**
- The enumeration owner exists: `lifecycle/completion.rs:212-216` — "Five ordinary terminal paths" (line-wrapped across `:212/:213`, which is why a single-line grep for the phrase misses it), naming `terminate::complete`, `terminate::fail`, `terminate::cancel`, `continue_as_new`, and the deadline teardown's deregister-LAST step.
- pid-enders: `terminate.rs:139` `context.runtime.cancel_pid(handle.pid())`; `deadline.rs:343` `runtime.cancel_pid(handle.pid())` with `// 6. Deregister LAST.` at `:380`.
- deregister-only: `terminate.rs:64` (complete) and `:96` (fail) are bare `registry.remove` with no `cancel_pid`; `continue_as_new.rs:150` likewise.
- `engine/api.rs:204-209` states the same split and points at `completion.rs` as the single enumeration owner — the two places do not restate the list, so there is nothing to drift.

**Fix 2 — the R23 section.** `r23-fix-gates.transcript` has one suite step: `suite EXIT 101 verdict-lines 39` + `FAILED. 7 passed; 1 failed`, and `DONE` at 22:43:28 → it is attempt 1, as the record now says. `r23f-suite.log`: 39 `^test result` lines; failure is `outbox_happy_path_completes_with_n_terminals_in_order`, `Running tests/outbox_e2e.rs`, `SQLite failure: bad parameter or other API misuse` — matches the record verbatim and is known flake #85. `r23f-suite-2.log` mtime **23:22:04**, 60 `^test result` lines, summing **1130 passed / 0 failed**, zero `^error` lines, doctests completed — so "A MANUAL RE-RUN OUTSIDE THE HARNESS at 23:22" is correct. Loads "5.0–9.6" match the transcript's 5.40/5.69/5.04/7.43/9.60/8.55.

**Fix 2 — the new REVIEW 24 section.** Every gate result matches its artifact: fmt-check EXIT 0 (`r24f-fmt.log` empty), module EXIT 0 with `5 passed; 0 failed` (all five named tests green), clippy-p EXIT 0 with a **real** `Checking aion-rs v0.11.0 (/Users/tom/Developer/ablative/stack/aion/crates/aion)` line, clippy-ws EXIT 0 over five crates. Loads "9.0–11.0" match 9.89/10.23/11.02/9.02. The summary of R24's verdict is accurate against `tasks/a6f61b568b0350116.output` (`REFUSED`, `0 blocking, 0 major, 2 minor`, "one clause from landing; nothing structural is wrong"). Its "CORE PROVEN UNCHANGED by byte-diff … only the predicate doc paragraph differs" also checks out independently: `diff -u r23-nif.bak r24-nif-baseline.bak` = 30 changed lines, **every one a `///`/`//` comment line**, zero non-comment lines.

**The "no suite re-run" justification — judged HONEST.** The delta since the last green 1130/0 suite is exactly (a) one `///` paragraph on a private `const fn`, byte-verified above, containing no code fence and therefore creating no doctest, and (b) the untracked record file, which sits outside every crate directory and which nothing pulls in (`grep -rn "include_str!\|include_bytes!" crates/` → no `docs/` hit). The module test re-ran 5/5 over the rebuilt file and both clippy runs did real work on `aion-rs` — that is the entire executable surface a doc comment can touch (`missing_docs`, rustdoc lints). The stated reason is the true reason.

**Bypass scan.** Added lines grepped for `#[allow` / `#[expect` / `#[deny` / `#[ignore]` / `cfg(any())` / `unwrap()` / `expect("` — zero hits.

### TAKEN ON TRUST
- `Entry: load 10.49, ZERO foreign cargo on the process table` and `detached pid 96934` (pid no longer exists); `load 7.85 at entry` for R23 suite attempt 2. None is recorded in an artifact.
- `EXIT 0` for `r23f-suite-2.log` — no `.exit` file exists. The log's content (60 green verdicts, no `^error`, doctests completed) is *consistent* with EXIT 0 but is not the exit code.
- The #85/#149 flake attributions and the "2 hits in 4 quiet-box runs" count.

### RE-RAN
Nothing. No cargo command was issued — the delta is doc-only and every gate artifact was verified directly against the record. Process table checked at entry (one cargo pid, already gone by the next call) and at exit (`pgrep -x cargo` → none). Load 4.51 at entry, 8.25 at exit; no result here is timing-sensitive. Nothing mutated, committed, pushed, or deleted; `Cargo.toml`/`Cargo.lock` untouched; no servers or processes touched.

### OBSERVATION (not a finding)
Every piece of evidence this record cites — `tasks/a6f61b568b0350116.output`, `scratchpad/r2*-fix-gates.transcript`, `r2*f-*.log` — lives under the ephemeral session directory, not the repo. The record will be committed; its evidence will not survive the seat. This is the pre-existing convention of the whole document (the R23 section does the same, and R23/R24 accepted it), so it is not a defect of this delta — but if the attestation is meant to outlive the session, the transcripts and gate logs need a durable home.
