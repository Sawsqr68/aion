# 📍 SEAT RESUME — 2026-08-01

Supersedes the resume markers inside `CANCEL-NEVER-ALIVE-RUN-117C.md`, whose lane is
closed. Read this first.

## Tree state

**`origin/main` = `4b431af0`. Everything is pushed. Working tree clean of crate code.**

| commit | what |
|---|---|
| `2eb515c0` | #117(c) — a run that was never alive can be cancelled |
| `e86378b2` | #117 surface — `GET /workflows/unrecoverable` |
| `c11e18a5` | docs — the exclusion is process-local, and where that puts the first live case |
| `6f18949f` | #123 — `GET /build`, a server can say what code it is |
| `a06a714b` | docs — what code Tom's live server actually runs, bounded to 76 commits |
| `9e77ba65` | docs — this resume marker |
| `4b431af0` | docs — the author/committer control, and which way the bound errs |

Untracked and pre-existing, not mine to sweep: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `examples/remote-gates/worker/Cargo.lock` (modified).

## Gates, as run

```
cargo clippy --workspace --all-targets -- -D warnings   EXIT 0
cargo fmt --all                                         EXIT 0
cargo test --workspace --no-fail-fast                   EXIT 0
                    3950 passed / 0 failed / 243 result lines
```

## ✅ Closed this session

- **#117** — all three pieces. The fact is kept (`7c9a2329`), the run can be cancelled
  (`2eb515c0`), the id can be found (`e86378b2`). Waffles ENDORSED the workflow-scope
  widening and minted a board law from it.
- **#123** — `GET /build`. Filed and closed the same session, from a measurement.
- **#44** and **#45** — closed BY MEASUREMENT, not by work. Both were built and gated and
  left `in_progress`. `aion awl emit --target beam` exists with three tests; an
  AWL-authored `timeout` is a real armed deadline through the whole chain
  (`document.rs` → `compile.rs` → `assemble.rs` → `contract.rs` → catalog →
  `start.rs:187` → `deadline.rs`). A stale board entry reads exactly like a current one.

## 🔴 The one thing genuinely waiting

**The first live case of #117(c) — run `0756ecd5`, per requirement (4).**

It must run INSIDE the live server's own process (the exclusion is process-local — see
`CANCEL-NEVER-ALIVE-RUN-117C.md`). Not from a CLI, not from a second engine, however
convenient the store on disk makes it look.

Two locks, neither mine:
1. **Tom's word** on restarting his server (#108).
2. **Acceptance of a binary carrying (c)** — the accepted one is #58-era and does not.

Cally carries the dependency in Tom's digest.

## Board after this

| item | note |
|---|---|
| **#88** | still needs its (a)/(b) ruling — the floor I tried was not option-independent, reverted at `9ccee6f9` |
| **#85** | open investigation; `sqlite3_errmsg` connection-level-state lead REFUTED, cause unidentified; evidence in `evidence/85-outbox-flake/` |
| **#71** | GitHub Actions is DISABLED on the repo — a decision for Tom, not a build |
| **#76** | a behaviour-changing dependency feature can arrive with no diff and nothing detects it — same family as everything below, unbuilt |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, the NIF-state P0 |

## 🔬 Method laws earned tonight — these are the durable part

**Waffles, board-wide:** *a guard may always widen beyond its ruling in the **refusal**
direction without returning for a word; only permissive deviations come back.* Flag it
anyway — the flag never blocks.

**Waffles, board-wide:** *pin the INVARIANT, never the schedule.* "600 isolated iterations
green, failed under load" is what a schedule-pin looks like right before it ships.

**Identity is content, never a label.** Three costumes, all measured:
- an artefact's identity is its **inode/hash**, never its path — `lsof`'s path column is a
  record of where a file *used to be*;
- a **status code cannot prove an endpoint exists** — the SPA catch-all answers
  `200 text/html` for any unmatched path, so probe the BODY (`jq -e .commit`), never
  `%{http_code}`;
- **shape is an enumeration; the value is the invariant.** Measured, not argued: four
  tests asserting a commit's shape (40 hex, known dirty states, parseable timestamp) ALL
  PASSED a deliberately fabricated hash. Only the test comparing against an independent
  `git rev-parse HEAD` failed. An instrument built to catch a wrong label, four-fifths of
  which could not tell a wrong label from a right one.

**A correct number with the wrong NOUN.** Athena measured a 36 GiB build tree honestly and
called it *fifteen runs accumulating*; it was mostly ONE build's **footprint**. They render
identically in a `du` and only the second predicts the next build. Measuring more carefully
never catches this — only asking what KIND of quantity it is.

**A vacuous ABSENCE check looks like SUCCESS.** Athena's, caught inside her own
verification of the `target/debug` clear — the night's exact family, committed in the
act of verifying it. A wrong path prefix made `ls -d "$A/debug"` test a path that had
never existed; it answered `No such file or directory`, which is *exactly what a correct
"debug is gone" result looks like*. True, empty, confirming. What caught it was the
positive arm: the same bad prefix hit `du "$A/release"`, where absence is a **broken
command** rather than an answer.

> The absence-check went quiet and confirming; the presence-check went loud and broken.
> — Athena

⇒ **Never verify a deletion alone.** Every "X is gone" gets a sibling "Y is still here"
built from the SAME prefix — the positive arm is the only one whose failure is loud, so
it is the one that proves the negative arm was pointed at the right place at all. It
generalises off the filesystem: *no rows matched*, *no such key*, *no error logged*,
*the route 404s* all pass vacuously against the wrong database, namespace, or host. Same
reversal as the non-vacuity guard one layer out: there an empty baseline satisfies
equality, here an absent root satisfies absence.

**One-writer is structural within a process and an ALARM across processes.** The registry
holds no persistence and rebuilds empty at boot. Any second process takes a terminal-writer
reservation trivially. `SequenceConflict` is the only cross-process defence, and an alarm is
never a load-bearing component.

## 🔴 Box rules confirmed the hard way

- ✅ **`target/debug` was CLEARED at the end of this session** (2026-08-01) and the clear
  is **VERIFIED AT ATHENA'S HAND, on her second pass** (see the vacuous-absence law
  above — her first pass was empty). Free went 48 → **85.73 GiB**; `target/release`
  intact to the byte (1,427,144 KiB, `f748956c…`), the `.aion-accepted-binary-20260731/`
  ledger copy untouched at the same sha, and nothing collateral swept. A cold ~40 GiB
  build from here lands at **45.73 GiB — clear of the floor by 20.7.** So the next cold
  build has room — but measure it again anyway, because that is how this one was caught.
- 🔴 **Athena's measurement that forced it:** from 48 GiB free with a 37 GiB debug tree, a
  cold full-workspace build would have landed at **~8.8 GiB — far below the hard floor of
  25.** The box could not have absorbed one. Do not assume headroom; measure it.
- A cold full-workspace build + battery costs **~25–37 GiB** and needs **~40 GiB free**.
  Name it to Athena BEFORE starting; she grants, **you execute** (she is read-only by
  standing constraint, and if she both deletes and verifies nobody independent has
  checked). Pin `target/release` size + `aion` sha256 at your own hand first.
- The reclaim is exactly `rm -rf <repo>/target/debug`. ⛔ **Never bare `cargo clean`** —
  it takes `target/release`.
- `target/release/aion` and `.aion-accepted-binary-20260731/aion` are byte-identical
  (`f748956c…`) and are the restart path. Never sweep either.
- **Do not go near pid 99844** beyond read-only probes.
