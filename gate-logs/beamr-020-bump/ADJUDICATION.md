# beamr 0.20.0 pin-wave landing battery — adjudication

Venue: rocketfish (Arch, rustc 1.94), tree `~/vesper/aion-migrate`, a content
copy of this commit's tree. Same-unit proof: every `crates/**/*.rs` file plus
`Cargo.toml` and `Cargo.lock` (1359 files) hashed byte-identical between the
committed worktree and the battery venue (order-independent sha256 census,
diff exit 0) at adjudication time.

## aion-battery3.log — the standing record (GREEN)

Fired 2026-08-22T23:25:19Z with the invocation criterion met:
`ulimit_n_became=65536` in-header.

- `cargo clippy --workspace --all-targets -j8 -- -D warnings` →
  `CLIPPY_TRUE_EXIT=0`
- `cargo test --workspace --no-fail-fast -j8` → `TEST_TRUE_EXIT=0`

TRUE_EXIT lines are unpiped (`$?` read directly after each command; no
pipeline tail can substitute its own exit).

## aion-battery2-VOID.log — kept as record, VOID as evidence

Same commands, same tree, fired without raising the file-descriptor limit:
the box's soft nfile is 1024 (hard 524288) and the invocation criterion
(`ulimit_n_became` in header) was unmet. `CLIPPY_TRUE_EXIT=0`,
`TEST_TRUE_EXIT=101` with 87 failures carrying one mechanism: 86 panics
verbatim `Too many open files (os error 24)` at tokio Runtime construction,
and the 87th (`run::lsub_prod_xnode_e2e`) failed in the same window with the
same EMFILE surfacing inside its haematite store setup — a measurement taken
in a resource-exhausted world, indicting the instrument, not the tree.
The test leg is therefore an instrument artifact; battery3 is the re-measure
in a proven world.
