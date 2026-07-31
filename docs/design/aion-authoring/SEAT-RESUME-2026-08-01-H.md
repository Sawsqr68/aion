# 📍 SEAT RESUME — 2026-08-01 (H)

**Supersedes G. Read this first.** G is historical and its #90 section is
WRONG — it argued the fix was complete and correct. It was neither.

I am **Vesper Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign
`— Vesper`. **Fable is the MODEL I run on, not my name.**

## State

```
origin/main = f7a08aaf   (PUSHED; local == remote, nothing in flight locally)
crates/ is byte-identical to the pre-#90 state 3bee2840
```

Working tree clean apart from untracked `.aion-*-20260731/` artifact dirs and
these docs.

## 🔴 #90 IS REVERTED — the fix was the defect

Full record: `RETRY-CLASS-HONESTY-90.md` (rewritten; it now says the opposite
of its first version). Task #90 carries the detail and **stays open** — the
operator-facing observation is real, only the fix was wrong.

One-line cause: `ActivityErrorKind` and the `retryable:` message prefix are not
duplicates. `kind` is the replay disposition and on this path is a **hardcoded
constant** (`nif_activity.rs:262`); the prefix is the **Gleam SDK's typed-error
wire format**, parsed by `parse_error` and pattern-matched by workflow code.
The fix rewrote the informative field to agree with the constant one, which made
`error.Retryable` unreachable from any remote activity.

Verified green after revert: `EXIT 0 · 5 targets · UNPAIRED 0 · FAILED 0`.

## Battery of record (ws6)

`WORKSPACE_EXIT=101 · 243 verdicts · targets 240 · UNPAIRED 0 · IGNORED 0 ·
4018 passed · 9 failed.` Eight were #90. **The ninth is open — see below.**
Athena holds `ws6-FINAL.log` (6,349 lines); it outlives this session. My copy
dies with the session.

🔴 **#85 DID NOT FIRE** — 0 occurrences of its signature in the whole log,
against a live control. My prediction ("expect exactly one red, and it will be
#85") was wrong about the count and wrong about the test.

## 🔴 OPEN: task #127 — haematite conformance

```
losing concurrent append should return SequenceConflict for the winning head
  expected: 1   actual: 0
```
`crates/aion-store/src/conformance.rs:291`. haematite is the **default**
durable backend, and by invariant 3 a `SequenceConflict` **is** the
double-writer signal. Prime suspect `51411a41`, which gave haematite "a
generalized read-modify-write whose guard refusals write nothing" (+215 lines).
Still genuinely open: whether the case guarantees a real race or merely hopes
for one — if the appends serialize, 0 is correct and the ASSERTION is wrong.
An Opus diagnostician was dispatched read-only; **if no verdict is in this
transcript, re-dispatch it.**

Note: the working tree is CLEAN. Earlier notes calling these files
"uncommitted-modified" came from a stale session-start snapshot — refuted.

## Method laws earned (the expensive ones)

**🔴 WHEN TWO FIELDS DISAGREE, ESTABLISH WHICH ONE CARRIES INFORMATION BEFORE
CORRECTING EITHER.** One may be a hardcoded constant; harmonising toward it
destroys the only signal while looking like a consistency fix.

**🔴 A STRING THAT CROSSES A LANGUAGE BOUNDARY IS A WIRE FORMAT.** Sweeping Rust
consumers was necessary and not sufficient — the decisive consumer was Gleam.
A fix's search space follows the value across every boundary it crosses.

**🔴 A CONTRACT CAN LOOK LIKE A STALE PIN** — the mirror of "a defect can have a
test defending it". Three tests said "verbatim" and I judged them probably
stale. They pinned the contract. A bulk expectation-update would have shipped
an estate-wide regression **with its own gate rewritten to endorse it.**

**🔴 A MUTATION PROOF SHOWS A CHANGE IS WIRED IN. IT CANNOT SHOW IT IS CORRECT.**
Five arms all measured against the premise instead of testing it.

**🔴 A ROUGH-EDGE COMMENT IS A LOAD-BEARING SPECIFICATION.** The violated
constraint was written in the adjacent test file and read only afterwards.

**A THRESHOLD TEST MUST RE-MEASURE AT THE MOMENT IT DECIDES** (Athena's, taken):
`df -g` truncates, so it understates by at most 0.99 GiB and cannot explain a
25-vs-28.70 gap — staleness can.

Prior laws stand in `assert-the-invariant-not-the-enumeration.md`.

## Standing orders

- **Delegate; do not implement in my own context.** Subagents on **Opus**,
  **2–3 concurrent max** (budget: the shared 5h window went 33%→83% in half an
  hour). Seat's job = dispatch, then verify at the sources.
- ⚠️ **NO WORKTREES** (Tom). File-mutating agents run one at a time in the main
  tree; parallelism is for read-only work.
- ⛔ **Never bare `cargo clean`** — it takes `target/release`, Tom's #108
  restart path. Reclaim is `rm -rf target/debug` ONLY, absence check paired with
  a survival check under the same prefix.
- **#108 pin:** `target/release/aion` the BINARY = 58,245,680 bytes, sha256
  `f748956ca23289a3376d196219214bb9c903f083fffb0093cbbd573413ef4d8a`;
  `target/release` the DIRECTORY = 1,427,144 KiB. Different things — they read
  as corruption the wrong way round. Tom's server pid 99844 runs from it.
- Box is shared by ≥5 seats. Free space is never "mine". Declare detached runs
  to Athena (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`) before the process
  exists.

## Board

| item | note |
|---|---|
| **#90** | reverted, pushed, **stays open** — observation real, fix wrong; §5 of the record has the corrected design |
| **#127** | haematite conformance — the live unknown |
| **#85** | root cause verified in libsql 0.9.30 source (double-close UAF); did NOT fire in ws6; decision for Waffles/Cally, E1/E2 discriminators unrun |
| **#34** | corroborated — `nif_activity_in_vm.rs` has zero retry symbols in 266 lines |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, NIF-state P0 |
