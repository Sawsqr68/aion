# Lane charter: write-amplification piece A — batched event appends

You are fixing one named, measured defect. The forensics document appended
below this charter is the mechanism of record — read it first, especially
§2 (the write path, file:line) and §3 (the fixture pair).

## Scope — piece A ONLY

`crates/aion-server/src/activity_publisher.rs:183` publishes every agent
telemetry event as its own `append_activity_event`, which reaches
`crates/aion-store-haematite/src/store.rs:1617` as `append_batch` with a
batch of ONE. One event = one tree commit = one whole-leaf rewrite (the
storage-side amplifier is a separate lane; you are removing the
one-commit-per-event multiplier, which is real on its own).

Requirements:
1. Coalesce publisher appends into real batches through a flush seam.
2. Flush-policy values (max batch size, max hold time) are REQUIRED config,
   validated loudly at startup (the shard_count pattern), NO shipped
   default (house law: no assumed defaults). Your REPORT.md proposes
   recommended values with reasoning; you do not hardcode them.
3. The durability contract is stated explicitly in code docs and REPORT.md:
   what an ack to the worker means before and after your change. If
   batching would weaken it, STOP and report — a silent weakening is a
   correctness bug, not a trade-off you may make alone.
4. `expected_seq`/`SequenceConflict` semantics survive batching intact
   (single-writer law: one Recorder path, a SequenceConflict still means a
   double-writer bug, never a batching artifact).
5. RED FIRST: a test on the REAL publisher path — publish N events, count
   store commits/new blobs; red today (N commits), green batched
   (≤ ceil(N/batch)). The red run's output is quoted in the commit message.

## Zone wall — violation = STOP, not merge

Your files: `crates/aion-server/src/activity_publisher.rs`,
`crates/aion-store-haematite/src/store.rs`, their tests, and config
plumbing. You may NOT touch the event-emission layer
(`aion-integration-norn`, `aion-integrations`) or transcript
rendering/reconstruction — a parallel lane owns those. If your fix
genuinely needs those files, STOP, write REPORT.md explaining why the zone
split fails, and end. Do not merge two lanes' work with your own hands.

## Lane rules (standard, from the 36-follow-on charter — binding)

- PLAIN BUILDER. No aion harness anywhere in this lane's execution.
- Isolation walls absolute: never the live estate server, ~/.aion,
  aion-data/, live workers, or any sibling worktree. Any server you need
  runs isolated inside the worktree, built FROM this worktree's tree
  (binary provenance recorded in REPORT.md: path + commit) — never
  ~/.cargo/bin/aion.
- No lint bypasses, no #[allow], no #[ignore], no test skips, no weakened
  assertions.
- Autoformat (`cargo fmt --all`) before committing; there is NO fmt gate.
- Final battery at the final tree, extraction pipeline, ALL of:
  `cargo check --workspace --all-targets --message-format=json --keep-going`,
  `cargo clippy --workspace --all-targets --message-format=json --keep-going -- -D warnings`,
  clippy again `-p aion-server -p aion-store-haematite` (feature-resolution
  differs per-crate vs workspace), and
  `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --workspace --all-targets --cargo-quiet --message-format libtest-json-plus --no-fail-fast`.
  The battery runs EVERYTHING and reports everything; never stop at the
  first red binary. Exit codes recorded in REPORT.md.
- No allocation instrumentation without an explicit disk-budget line.
- REPORT.md at the worktree root before you finish, whatever the outcome:
  what you did, what you proved (with the red/green evidence), the
  durability-contract statement, proposed flush values with reasoning, and
  an explicit non-claim section for anything unproven. Silence is not an
  option.
