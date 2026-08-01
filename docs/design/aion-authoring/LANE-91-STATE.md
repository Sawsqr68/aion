# LANE 91 — LEG 1 state (AWL source as a first-class archive entry)

Implementer lane, working directly on `main` in `/Users/tom/Developer/ablative/stack/aion`.
No worktree, no branch, no commit — the seat verifies and lands.

## Position

- **Status:** FINAL for this lane. Implementation complete in the working tree
  (uncommitted, as briefed). All six mutation arms run and restored. Gates run
  and recorded below; one red, attributed and declared unresolved. No further
  cargo invocations after the seat's disk directive.
- **Last update:** 2026-08-01, on seat directive to close out.
- **Tree:** working changes sit on top of `19485286` — HEAD moved under this
  lane (session started at `087ada72`); the other lane's AWL-MIR work and the
  `deploys green` console fix landed mid-flight. Every gate above ran against
  the current HEAD with this lane's changes applied.

## Entry-naming design (chosen)

Two entry families under one `awl/` prefix, both rooted at the DOCUMENT'S
DIRECTORY (`schema_root` — the same root the checker resolves `schema(...)`
imports against):

```
awl/document/<original filename>       exactly one entry, e.g. awl/document/dev_brief.awl
awl/schema/<document-relative path>    zero or more, e.g. awl/schema/schemas/brief.schema.json
```

Why this and not the alternatives:

- **Not `src/`** — `src/` entries are keyed `<name>.gleam` (`builder.rs:172`);
  AWL under a `.gleam` name lies about content type and destroys the
  "`src/` empty ⇒ AWL-origin" signal.
- **Two families, not one flat `awl/` tree** — a single flat tree would need a
  heuristic ("the entry ending `.awl` is the document") to tell the document
  from its imports. `awl/document/` is a declaration; the split makes the
  document unambiguous with no extension sniffing, and refuses a second
  document entry rather than picking one.
- **Full filename kept, extension included** — the read path strips only the
  family prefix and validates the remainder with `is_safe_logical_name`, so
  `dev_brief.awl` and `schemas/brief.schema.json` round-trip byte- and
  name-identical. Schema paths keep their nesting because leg 2 must stage them
  back at exactly the relative paths the document's `schema("…")` lines name;
  flattening would break the re-check.
- **Rooted at the document's directory** — the document is always at the root of
  that directory, so its entry carries a bare filename with no directory part,
  and every schema path is relative to the same root. That is exactly the layout
  `stage_schema_imports` (`aion-server/src/awl/handlers.rs:220-267`) builds when
  it stages a workspace document for a check, so leg 2 can reuse the shape.

## Milestones

- [x] Scout read; surfaces mapped (`package.rs`, `builder.rs`, `hash.rs`,
      `assemble.rs`, `prepare.rs`, callers).
- [x] `AwlSource` type + `Package` field + read/write round-trip.
- [x] Assembler embeds document + schema imports.
- [x] Rider stated (hash doc, builder format doc) + pin tests.
- [x] False comment at `assemble.rs:80-83` replaced.
- [x] Tests (round-trip, identity pin, tolerance) written; `-p aion-package`
      exit 0, `-p aion-awl-package` exit 0.
- [x] Format-doc statement in `docs/design/aion-package/DESIGN.md` + `design.json`.
- [x] Mutation arms A-F: each watched RED, restored from the scratchpad
      snapshot, restore verified green AND by sha against BASELINE.sha.
- [x] `cargo fmt --all` (run, not checked — estate rule).
- [x] `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
- [x] `cargo test -p aion-package` → exit 0 (309 + 10 further binaries, 0 failed).
- [x] `cargo test -p aion-awl-package` → exit 0 (8 binaries, 0 failed).
- [x] `cargo test -p aion-awl --lib` → exit 0 (117 passed).
- [x] `cargo test -p aion-rs --test awl_bc4_differential` → exit 0, 29 passed,
      `COMPILED 104 module(s) under real \`gleam build\` in 1 attempt(s)`.
- [~] `cargo test -p aion-rs` → **exit 101, and the run is INCOMPLETE**.
      Cargo runs targets in order and stops at the first failing one, so only
      **9 of aion-rs's 58 test targets executed** (lib + 8 integration
      binaries, ending at `tests/awl_bc4_differential.rs`). 710 passed,
      1 failed, 0 ignored across what ran; **the remaining ~49 integration
      binaries NEVER RAN and are declared unrun, not green.** The single
      failure is `covered::covered_set_is_byte_identical`:
      `test result: FAILED. 28 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 74.56s`
      — see "The one red" below. A `--no-fail-fast` re-run to reach the rest
      was NOT launched (seat disk directive).

Gate ordering note: `cargo fmt --all` and `cargo clippy --workspace
--all-targets -- -D warnings` were both run BEFORE the seat's cancellation
directive arrived, and clippy exited 0 against the FINAL Rust tree (no Rust
source was edited after that clippy run — only this state file). Per the seat
directive they were not re-run afterwards, and the seat runs both at its own
hand during landing verification: **clippy/fmt re-run cancelled by seat
directive (disk band), seat runs them at landing.**

## The one red — `covered::covered_set_is_byte_identical`

```
DIVERGENCE dag-fork/valid/fork_named_homogeneous event[6] /data/activity_id: reference=0 direct=1
DIVERGENCE dag-fork/valid/fork_named_homogeneous event[7] /data/activity_id: reference=1 direct=0
```

Two facts, both measured, no inference beyond them:

- The SAME test binary, on the SAME tree with every change in this lane
  already applied, ran alone earlier in this lane and passed:
  `cargo test -p aion-rs --test awl_bc4_differential` → exit 0,
  `test result: ok. 29 passed; 0 failed; …; finished in 68.98s`, with
  `COMPILED 104 module(s)`. The red appeared only inside the full
  `-p aion-rs` run, where ~90 test binaries execute concurrently.
- The divergence is a SWAP of two `activity_id` values between two events of
  one named-fork fixture — an ordering difference between the reference and
  direct backends, on a surface this lane does not touch (no MIR, no
  lowering, no goldens, no scheduling).

A confirming standalone re-run was LAUNCHED and then STOPPED unfinished to
honour the seat's disk directive, so the contention hypothesis is
**supported by the earlier standalone pass but NOT independently confirmed**.
Declared unresolved for the seat, not explained away.

## Deviations to flag to the seat

1. **`compile_and_assemble_awl` gained a third parameter** (`document_filename`).
   The seam had no way to learn the document's real filename, and the ruling
   requires the ORIGINAL filename to be preserved. ~25 call sites updated,
   including the standalone `examples/staged-rounds/worker` crate (not a
   workspace member — its `cargo check` was NOT run here).
2. **Refusal-direction widenings** (flagged, not asked, per the board law):
   - a `schema("…")` import that resolves ABOVE the document's own directory
     (`../…`, absolute) now REFUSES at assembly instead of deploying. Such a
     document compiled and deployed before this change; it cannot be archived
     without lying about provenance. `./x.json` is normalised, not refused.
   - an archive entry under `awl/` that names no defined family is REFUSED at
     load (previously any unknown entry was skipped). Skipping would lose the
     bytes at the next persistence rewrite.
   - `awl/schema/…` entries with no `awl/document/…` entry are REFUSED.
3. **Pre-existing finding, out of scope:** `manifest.entry_function` is NOT
   bound into any package identity — a mutation that rewrote it left the
   content hash unchanged. Noticed while building mutation arm C; nothing in
   this leg depends on it.
4. **`docs/design/aion-package/DESIGN.md` was hand-edited** alongside
   `design.json` because `render-cluster.py` CANNOT render this cluster:
   `TypeError: cannot use 'dict' as a dict key` at `render-cluster.py:106`
   (pre-existing, reproduced on the unmodified tree).
