# BRIEF — #91 leg 2: the read-only deployed surface

**Dispatched 2026-08-01 by Vesper (seat). Base: origin/main = c6c66add
(leg 1 = 678ab99c).** Athena's disk clearance covers EXACTLY: (1) aion-server
+ dependents, incremental, warm tree; (2) the ops-console bun/vite build.
NOTHING ELSE — no workspace-wide cargo anything, no clippy, no fmt --all, no
non-workspace builds (examples/ is out of scope). Violating this is a lane
failure regardless of code quality.

## Objective

A CLI-deployed AWL workflow becomes VISIBLE in the studio: the operator opens
the console and can see, for any deployed AWL-origin package version, the
authored document and its schemas — read-only. This is the leg Tom actually
sees. Leg 1 (landed, 678ab99c) put the source in the archive; leg 2 surfaces
it.

Read first: docs/design/aion-authoring/SCOUT-91-STUDIO-VISIBILITY.md (the
scout's file:line map + seat recommendation) and LANE-91-STATE.md (leg 1's
evidence). The ruled design (Waffles + Cally):

1. **Server**: a read-only endpoint surface on the authoring/studio API that
   lists deployed package versions carrying AWL provenance and serves the
   archived document text + schema files for a given version. Source of
   truth is `Package::awl()` (the `AwlSource` landed in leg 1) read from the
   deployed package — never the operator's workspace, never a re-compile.
2. **Console**: a deployed-documents view in the studio — the operator can
   select a deployed AWL workflow version and see its source rendered in the
   same editor/canvas surfaces used for workspace documents, UNAMBIGUOUSLY
   read-only (no edit, no save, no format actions reachable; visually marked
   as the deployed artifact, identified by workflow type + content-hash
   version).
3. **Deploy NEVER auto-writes the operator's workspace** — ruled, killed by
   construction. The deployed surface is display-only. No "copy to
   workspace" in this leg unless it is a deliberate, operator-initiated
   action you choose to add — and if you add it, it must refuse to
   overwrite an existing file. When in doubt, display-only.
4. A package WITHOUT archived source (pre-leg-1 deploys, Gleam-authored
   packages) is a first-class state: the listing says so honestly
   ("no archived source" — absence stated, not hidden, not errored).

## Constraints (house law — violations are defects)

- Read-only means read-only: the new server surface performs NO writes of
  any kind (no workspace mutation, no package mutation, no cache files).
- No arbitrary limits, no assumed defaults. No `unwrap`/`expect` in library
  code; thiserror-typed errors; every failure path handled and honest.
- No file over 500 lines; `mod.rs` = declarations/re-exports only.
- NO backwards-compat shims. NO `#[allow]`/`biome-ignore` (bypasses, not
  fixes). No worktrees — work directly in the main tree; you are the only
  mutating agent.
- **#131 trigger**: `bun run check` is a gate with a NAMED 13-finding
  baseline (apps/aion-ops-console/CHECK-BASELINE.md). Delta-zero vs names
  for every file you don't touch; any baseline file you DO touch must have
  its findings CLEARED by refactor (facade.ts carries a
  noExcessiveCognitiveComplexity 19 vs max 15 — if you touch facade.ts,
  it leaves your lane at ≤15).
- Match surrounding code idiom; tests prove behavior (pair absence with
  survival — a read-only pin proves both that reads work AND that no write
  path exists/fires).

## Gates you run (exit codes in your evidence file, verbatim)

Allowed and required:
1. `cargo test -p aion-server` (incremental — this is clearance object 1;
   scope any additional `-p` ONLY to crates you actually touched, and you
   should not need to touch any crate besides aion-server).
2. In apps/aion-ops-console: `bun run check` (delta vs the named baseline),
   the console test suite, and the production build (clearance object 2).

FORBIDDEN (other seats' deferred windows — do not run): workspace-wide
cargo test/clippy/fmt, examples/staged-rounds anything, cargo clean of any
kind.

## Deliverables

- Implementation, uncommitted, in the main tree (the seat verifies bytes
  and lands — do NOT commit or push).
- `docs/design/aion-authoring/LANE-91-LEG2-STATE.md`: files touched (full
  list), gates run with verbatim exit lines, design decisions made where the
  brief left freedom, deviations flagged loudly, anything you saw that is
  wrong but out of scope (report, don't fix).
- If you find the ruled design unimplementable as stated, STOP and write
  the blocker into the state file rather than improvising around it.
