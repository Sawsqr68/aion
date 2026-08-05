# Seat resume — marker AC, 5 August 2026

Supersedes marker AB. Latest letter wins.

## What is live right now

**The fresh-machine setup test is RUNNING** at
`/Users/tom/aion-freshtest-20260805/` (moved out of the scratchpad — a store
may not live under `/private/tmp`, see below). Log: `run.log`. Harness:
`harness.sh`. A persistent Monitor watches the log.

It is currently in `cargo install --path .../crates/aion-cli` from the isolated
clone — a ~25 minute build. Phases still unmeasured after it: deploy, worker
build, worker registration.

**Guards holding, all verified this session:**
- Tom's server (pid 85467, ports 8080 / 50051 / 50061) untouched and answering.
- Isolated ports 18080 / 18051 / 18061; harness refuses if they collide or are busy.
- `CARGO_INSTALL_ROOT` redirected — his `~/.cargo/bin/aion` never overwritten.
- Homebrew deliberately off the test PATH.

**Cleanup owed when the test is done:** `rm -rf /Users/tom/aion-freshtest-20260805`.
It is ~tens of GiB of build output in Tom's home. Do not leave it there.

## What the test found (committed, pushed — `6b6ad6c2`)

Full record: `docs/evidence/setup-fresh-machine-2026-08-05.md`.

1. **`setup.sh` could not finish on any machine running the published CLI it
   tells you to install — including Tom's own.** It requires `workspace_root`
   from the startup banner; that field landed in `fe702945` (#139 leg A) after
   `v0.11.0` was published; `cargo install aion-cli` gets 0.11.0. Verified three
   ways, including that Tom's live server's banner lacks it.
   **Fixed by derivation, not by a version bump**: the script now installs the
   CLI from the checkout it already has, so script and server come from one
   tree and cannot skew again.
2. **The server-start wait burned 120s and reported no cause** while the server
   had exited one second in with a precise one — and the worker loop in the
   same file had always known to check for a dead process. Fixed; 120s with no
   cause became 2s with the cause; controlled both ways.

**Trap worth remembering:** `git merge-base --is-ancestor` answered "no" about
a commit in a `--depth 1` clone, because the object is not there. The content
check (the clone's own `run.rs`) was the truth. A shallow clone lies to a graph
query and tells the truth to a diff.

## Owed to Tom

1. **Tell him the setup finding.** He asked "will it work when they run it over
   their side — have we got a clear command?" The honest answer was **no**, and
   I should give him the whole thing once the run completes: what broke, that
   it would have broken on his machine too, and that it now works — not a
   partial. Plain language, four-part.
2. **The docs-site shape is written and committed** —
   `docs/design/aion-authoring/DOCS-SITE-SHAPE.md`. This discharges "come back
   with a shape". Silence-default: nothing gets built, but I do the example
   gate anyway.
3. **Never publish artifacts for Tom.** Deliverables are local files.
   The deck is `docs/authoring/awl-deck.html` — `open` it.

## The documentation example gate (piece 1 of the docs shape)

`docs/design/aion-authoring/DOC-EXAMPLE-GATE-FINDINGS.md`, committed `0978e8c2`.

44 AWL blocks. **3 check clean; 1 is a complete document that deliberately does
not check and whose prose says so; 40 are fragments and are NOT YET VERIFIED.**
Do not let "no false claims found" stand as a result — 40 of 44 are unmeasured.

Found on the way: `gated_item` and `gated_review` each exist in *both*
`AWL-REFERENCE.md` and `AWL-AUTHORING.md`, and **the copies differ**.

Next: per-class wrappers for the 40, an illustrative-block mark that requires a
reason, and reconcile the duplicated examples to one copy.

## Standing constraints (unchanged)

- 🔴 Worker rebuilds frozen estate-wide until the #139 live cut lands. The
  isolated build is in a separate tree on separate ports and cannot reach Tom's
  server; that is why it is allowed.
- 🔴 beamr JIT blocker with Artemis — keep long workflows off.
- 🔴 Task #142: merge `fix/worker-shell-dial-and-audibility` when the #139 cut
  clears, **then run the operator's real command** — the unit pin does not.
- ⛔ Never `git add -A` in this tree. ⛔ Never bare `cargo clean`.
- Commit messages with backticks go through `git commit -F -`.
- Do not edit `CLAUDE.md` — the proposal sits with Waffles.

## Commits this session

`10d00d44` docs-site shape · `0978e8c2` doc-example gate findings ·
`6b6ad6c2` setup.sh version-skew + startup-failure fixes with evidence.
All pushed. Tree clean.
