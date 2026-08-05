# Contributing

This repository runs mission-critical infrastructure. The standard for every
change is: **would you trust this with patient records, financial
transactions, or legal documents?** If not, it is not ready.

## The one command

```sh
scripts/battery.sh
```

That is every gate this repository has, in one run. It exits non-zero if any
gate failed, and it prints a manifest of every gate with its exit code — so
"the battery passed" is a claim you can check rather than a memory.

Run it before you land anything. If a gate is red, fix the code; a red gate is
never a formality to be argued past.

### Why it exists as a script

GitHub Actions is **disabled** on this repository. No workflow in
`.github/workflows/` has ever run. That has mattered three separate times, each
the same shape — a correctly designed gate that executed nowhere:

- the all-features Rust gate, which existed and had never run;
- `wire-types-no-diff`, whose own comment shows its author saw the exact trap
  it was guarding, in a lane that is off;
- the Python and TypeScript SDK proto-drift gates, which are the remediation
  for an incident where **both worker SDKs went wire-dead with green suites**.

Before `scripts/battery.sh` there was no justfile, no Makefile, and no runnable
equivalent, so "the battery" was whatever each person remembered to type. And
what people type is the Rust battery — which does not run pytest, does not run
bun, and does not typecheck the console. Every gate outside Rust was
load-bearing and unexecuted.

### Two properties of the battery worth knowing

**Every gate runs even if an earlier one fails.** A battery that stops at the
first failure hides the other three.

**A missing toolchain is a FAILURE, not a skip.** If `uv` or `bun` is absent,
the battery says so and goes red. Install them, or run the gates you can and
say explicitly in your report which ones you did not. A skip is how a gate
becomes invisible, which is the defect the script exists to fix.

The battery can also prove its own red path:

```sh
AION_BATTERY_SELF_TEST=1 scripts/battery.sh   # MUST exit 1, "1 of 2 gates failed"
```

If that ever exits 0, the script cannot report a real failure either.

## What the battery runs

| Gate | What it is |
|---|---|
| `rust:fmt` | `cargo fmt --all -- --check` |
| `rust:clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `rust:test` | `cargo test --workspace --no-fail-fast` |
| `rust:test-all-features` | the same, `--all-features` |
| `posture:*` | feature-posture controls: each crate built in the feature shape it actually ships in, so a feature that only compiles alongside another cannot hide |
| `rust:preserve-order-pins` | `aion-package` + `aion-awl` under `serde_json/preserve_order` |
| `posture:serde-json-*` | the sorted and insertion-order postures, separately |
| `gates:costly-announced` | asserts every runtime-gated costly proof printed its skip line (see below) |
| `console:typecheck`, `console:test` | the ops console (`bun`) |
| `python:aion-worker`, `python:aion-client` | the Python SDKs and their proto-drift gates (`uv`) |
| `ts-worker:proto-drift`, `ts-worker:typecheck`, `ts-worker:test` | the TypeScript worker SDK (`bun`) |

`--no-fail-fast` on the Rust gates is load-bearing, and its absence cost a P0
on 2026-07-31: without it, `cargo test` stops at the first failing crate and
the battery inherits exactly the early-stop behaviour it refuses one level up.

### Gates the battery does not run

State these honestly rather than assuming coverage:

- **`bun run check` (biome) in the ops console.** The battery runs the
  console's typecheck and tests, not its lint/format check. Run it yourself
  from `apps/aion-ops-console` when you touch console source — the standard
  there is green-not-delta: **zero findings**, not "no new findings".
- **The costly durability proofs.** The cross-node `kill -9` failover proof and
  the parked-timer resume proof spawn real clusters and take minutes, so they
  are gated at runtime on `AION_RUN_COSTLY_TESTS`. They are **not** `#[ignore]`d
  — a default run prints a checked `COSTLY_PROOF_SKIPPED (PROVES NOTHING)` line
  naming the claim it did not establish, and `gates:costly-announced` asserts
  those lines appear. To actually run them, set `AION_RUN_COSTLY_TESTS=1`. A
  green run of these is only a proof when that variable was set.

## The rules that are not negotiable

These are enforced in review, and violating one is a correctness bug rather
than a style nit. The full set is in [`CLAUDE.md`](CLAUDE.md); the ones that
bite most often:

- **No lazy code.** No partial implementations, no deferred work, no "good
  enough for now."
- **No silent failures.** Every error handled, logged, or propagated. No
  swallowed `Result`s, no empty catch blocks, no bare `continue` on `Err`.
- **No arbitrary limits, no assumed defaults.** Caps, timeouts, retry policies,
  poll intervals and thread counts come from the builder or defer to the layer
  that owns them. A new hardcoded value is a review finding regardless of how
  sensible it looks. Discuss values before implementing them.
- **No backwards compatibility** during this build. Replace, do not add
  alongside: no compat shims, no zombie code, no `#[deprecated]`. If the only
  consumer of something is a thing you just deleted, it goes too.
- **No god files.** Nothing over 500 lines of code excluding tests, comments,
  and whitespace. `mod.rs` holds only `pub mod` declarations and re-exports;
  `lib.rs`/`main.rs` are thin entry points.
- **Never `.unwrap()` or `.expect()` in library code.** `thiserror` for library
  errors, `anyhow` only in the binary. Lock poison is handled explicitly and
  mapped to a typed error.

### Suppressions are not fixes

If clippy fires, fix the code. `#[allow(...)]`, `#[expect(...)]`,
`#[deny(...)]`, `#[ignore]` on tests, `_var` renames, and `#[cfg(any())]`
dead-code hiding are all bypasses. The same applies in the console:
`biome-ignore`, `ts-expect-error`, and skipped tests are bypasses.

A test that needs a runtime it may not have gates at runtime — read an
environment variable, emit a `tracing::info!` skip line, return `Ok(())` — so
the absence is stated in the run's own output. Never `#[ignore]`.

## Two traps this repository has actually fallen into

Both cost real incidents. Neither is theoretical.

**A green that could have been produced by something other than the thing
under test is worth nothing.** If your harness *constructs* the condition it
is testing — extracting, copying, generating, patching, enabling — that
construction can fail quietly, and the run then measures the baseline while
claiming to measure the treatment. Make the harness assert the condition landed
and refuse to run otherwise.

**A change is not done until the artifact that is actually served carries it.**
The ops console ships as a generated bundle committed at
`crates/aion-server/ops-console-embed/`. Editing `apps/aion-ops-console/src/`
changes nothing about the shipped binary until you regenerate it:

```sh
cargo xtask build-ops-console      # rebuild + sync into the committed bundle
cargo xtask verify-ops-console     # freshness guard: rebuilt must match committed
```

**Commit the refreshed bundle in the same change.** A stale embed has already
caused one production incident. The same reasoning applies to any generated,
committed output — prove the change against the artifact, not against the
source.

## Documentation

Documentation claiming something the code does not do is worse than no
documentation: it costs the reader the time to try it and then the time to work
out why it failed. So when you change behaviour, change the document that
describes it in the same commit, and cite the file that makes the new claim
true — that is the house convention and it is what makes a later comb possible.

`docs/README.md` is the map of everything under `docs/`.

## Local workflow artifacts

Some development tools create local process state while you work here. These are
intentionally gitignored and should not be committed:

- `.yggdrasil-worktrees/` — temporary workflow worktrees created by
  orchestration tooling.
- `.meridian/` — local Meridian/Norn state, profiles, tasks, hooks, and workflow
  metadata.
- `.claude/` — local Claude Code settings, skills, and process configuration.
- `server.log` — local server output from development runs.
- `.commit-msg.tmp` — temporary commit-message scratch file.

These are machine- or session-specific. Keep them on disk when your local tools
need them, but treat them as local-only process files rather than repository
source.
