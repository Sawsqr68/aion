# Tharsis fleet run #1 — postmortem and re-run plan

Run `9da28cf9-5eaa-45b8-9b3d-eccc194dceed` / `8fed4511-401c-49dc-b2e7-44d3624dffdf`,
2026-07-30 07:10:44Z → 08:06:17Z (55m 33s), on Tom's own server, target
`/Users/tom/Developer/ablative/dev-ops/tharsis` @ base `15f8c81c`.

## Verdict

The pipeline worked. The engine did not.

Everything the workflow was asked to do, it did: it read a
planning-documentation-only repository, produced a plan, fanned out to three
parallel items in three real git worktrees on three real branches, ran three
independent agents to completion, and put all three through a five-gate
battery. Then the engine dropped an internal handle and the run died. See
`FLEET-LIVE-STATE.md` for the full evidence chain (task #82).

## What it actually built — this is not scaffolding

`b1-m1-core-model`, 2 commits, **829 insertions across 7 files**:

- `crates/tharsis-core/src/{attribution,layer,mount,store}.rs`
- `crates/tharsis-core/tests/model.rs` — **372 lines, 11 tests**
- `crates/tharsis-server/src/build_lock.rs`, `docs/briefs/B1-tharsis-core.md`

The module documentation cites the repository's own design corpus by name and
by requirement id — VISION property 3, "the record shape is ARCHITECTURE's
verbatim", R13/R14/R15, HAEMATITE-NOTES — and it defers rather than invents:

> "Bracket semantics (flush-on-end, crash behaviour, overlap rule) are S6's
> deliverable; this module holds the types and the store, and adopts S6's spec
> when measured."

It also got the hard call right. `context: None` means the write happened
outside any bracket and is recorded as exactly that (R13) — unattributed, never
guessed. That is the invariant the whole attribution feature rests on, and it
would have been very easy to paper over.

Test names are the real signal, because they are claims:

- `mem_store_buffers_commits_diffs_and_fences_stale_writers`
- `content_addresses_are_order_independent_and_dropped_writes_are_discarded`
- `generated_forks_diverge_without_mutating_their_shared_root`
- `snapshots_remain_immutable_and_promotions_are_writable`
- `generated_lineage_records_each_parent_and_fork_root`
- `thousand_fork_storm_stays_within_the_model_bar`
- `attribution_brackets_are_layer_scoped_and_queries_return_ground_truth`
- `redirections_use_component_aware_longest_prefixes`

`s1-lite-readonly-export`, 2 commits: a whole spike — `spikes/s1/` with
`Cargo.toml`, `src/main.rs`, three Python harness scripts (`matrix.py`,
`prepare.py`, `read_loop.py`), `run.sh`, README — plus
`docs/briefs/S1-nfs-loopback-viability.md`.

`d1-project-foundations`, 1 commit: `.github/workflows/ci.yml`, `clippy.toml`,
`rust-toolchain.toml`, `rustfmt.toml`, `docs/ARCHITECTURE.md`, five design
briefs, two decision records, and `run.sh` for spikes s2–s8.

## The gates worked too, including by REFUSING

Decoded `gate_item` result for `d1-project-foundations`:
`gate scope-fence exited 1`, listing the files it wrote outside its declared
fence. That is the fence doing exactly its job. The item would have gone to the
four-lens review panel and come back through `fold_phase` as blocked, with the
finding attached, for another round. **The loop was working.** The engine died
before the panel ran.

Note the honest detail: `rustfmt.toml` in that item carries

> `# B3 owns this pre-existing file; D1 cannot normalize it without crossing its`
> `# scope fence. Remove the exception when B3 next changes the file.`

The agent understood the fence, worked within it, and left a note for whoever
owns the other side.

## Provider overload was real, and independently confirmed

Bare `norn --print --account default --fast --reasoning-effort high` — nothing
of ours in the path — hit the IDENTICAL `server_is_overloaded` error ~10 times
before punching through in ~50s (wrote `/tmp/asparagus.md`, exit 0). So the
overload is genuine and not our code.

The asymmetry is UNEXPLAINED, not verified: a small request cleared in 50s
while the planner (full brief + 15 pointer docs + a large output schema + a
repo to read) took 15 minutes. "Big requests are shed first" is plausible and
unproven. Do not repeat it as fact.

## Re-run checklist — do these BEFORE starting run #2

1. **#82 must be fixed.** Re-running into the same engine fault burns another
   hour for the same ending.
2. ~~**Rebuild the worker.**~~ **DONE** — `cargo build --release` exit 0 at
   2026-07-30 18:56 local, so
   `examples/staged-rounds/worker/target/release/staged-rounds-worker` now
   carries both halves of the #79 staging fix (`d2b32d84`, `5008a747`).
   **The RUNNING worker (pid 49833) is still the old code** and holds its own
   inode; it must be restarted to pick this up. Without that restart the fix
   is not in the loop and the scope-fence gate will keep failing items for
   committing their own build output.
3. **Rebuild `target/release/aion`** — it is currently MISSING, removed by an
   agent's cargo activity mid-session. The running server holds its own inode,
   so it survived; there is no binary on disk to restart from.
4. Clean up run #1's worktrees and branches in tharsis, or give run #2 a fresh
   run root. Three worktrees + three `staged/9da28cf9…/*` branches remain.
5. Consider whether `max_rounds 8` and 4 lenses are right for a re-run, or
   whether to prove the merge/integration half first with a smaller fan — that
   half (`merge_once`, `fix_conflicts`, `verify_integration`, `fix_integration`)
   **has never executed in its life**.

## The one number that matters

Three agents, three worktrees, one repository, zero collisions and zero git
damage. The explicit file-ownership fencing in each brief held.
