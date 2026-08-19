# Lane charter: write-amplification piece D — envelope-delta persistence

You are fixing one named, measured defect. The forensics document appended
below this charter is the mechanism of record — read it first, especially
§1's per-event-type tables and §3.

## Scope — piece D ONLY

Three full provider response envelopes per LLM turn (`response.created` /
`response.in_progress` / `response.completed`, ~82KB each, identical
instructions block every time) are persisted into the activity-transcript
stream — 91% of transcript bytes. Fix at the event-shaping layer: persist
the full envelope ONCE per turn and deltas for the rest. Events stay
events; history semantics do not change; what changes is what the emitter
puts in them.

## READER FIRST — mandatory ordering inside this lane

This is a FORMAT change to the transcript stream and mixed history will
exist forever. The reader arm lands FIRST: transcript
rendering/reconstruction proves it renders old-format streams, delta
streams, AND a mixed stream — with the old-format render behavior pinned
as a fixture BEFORE you change anything (capture the pre-change rendering
of a real fixture stream; that pinned output is the equivalence oracle).
Only after the reader proof is green does the writer flip. You may not
green the writer against a reader you changed in the same motion.

Requirements:
1. Reader: reconstruction of a turn from (one envelope + deltas) renders
   byte-identically (or provably-equivalently, stated) to the old
   three-envelope form. Old, new, and mixed streams all proven.
2. Writer: the emission layer persists one full envelope per turn, deltas
   for in_progress/completed (the identical instructions block never
   repeats).
3. RED FIRST: per-event-type byte table of a fixture turn — red today
   (3 × full envelope bytes), green with the delta shape. Red output
   quoted in the commit message.
4. Replay/resume paths that consume the transcript stream are part of the
   reader proof, not just the console view.

## Zone wall — violation = STOP, not merge

Your files: the event-emission layer (`crates/aion-integration-norn`,
`crates/aion-integrations` where Raw provider envelopes are shaped) and
the transcript reconstruction/rendering layer. You may NOT touch
`crates/aion-server/src/activity_publisher.rs` or
`crates/aion-store-haematite/src/store.rs` — a parallel lane owns those;
the store returns events, it does not interpret them. If your fix
genuinely needs those files, STOP, write REPORT.md explaining why the
zone split fails, and end. Do not merge two lanes' work with your own
hands.

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
  clippy again `-p aion-integration-norn -p aion-integrations` (feature
  resolution differs per-crate vs workspace), and
  `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --workspace --all-targets --cargo-quiet --message-format libtest-json-plus --no-fail-fast`.
  The battery runs EVERYTHING and reports everything; never stop at the
  first red binary. Exit codes recorded in REPORT.md.
- No allocation instrumentation without an explicit disk-budget line.
- REPORT.md at the worktree root before you finish, whatever the outcome:
  what you did, what you proved (with the red/green evidence and the
  reader-first ordering receipts), and an explicit non-claim section for
  anything unproven. Silence is not an option.
