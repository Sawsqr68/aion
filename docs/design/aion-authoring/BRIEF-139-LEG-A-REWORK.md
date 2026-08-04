# Brief: #139 leg A rework — bodied provision with server-resolved workspace root

**Date:** 2026-08-04. **Seat:** Vesper Lynd. **Ratified:** Waffles the
Terrible, 21:57Z (option (b), two conditions below). Context:
`BUNDLE-139-NOTES.md` §"2026-08-04 late" and the parked drafts in
`139-drafts/`.

## The decision being implemented

The bodied provision (server-run `assistant_provision`) must NOT take the
workspace root as a workflow start input (Tom's no-friction ruling: start is
`objective` + `repo_path` only, nothing else, ever). Instead:

- The server expands a **`{workspace_root}` placeholder** in declared-body
  command strings, resolved to **the server's own aion home `clones/`
  directory** (`config::home::aion_home()` → `<home>/clones`). No new
  configurable value exists: the home is already ruled (#113) and `clones/`
  is the established location (the crate worker's `handlers.rs` resolution,
  post-#175).
- **Ratification condition 1 (loud failure):** if the root cannot resolve to
  an absolute directory that exists (create it under the home if missing —
  creation failure is failure), the dispatch of a body that USES the
  placeholder is refused **terminally, by name**, at body-launch time. No
  fallback to cwd, temp, or anything else. Bodies that do not use the
  placeholder are untouched by resolution failure.
- **Ratification condition 2 (resolved path in history):** durable history
  records the RESOLVED path, never the placeholder. Mechanism: the provision
  script prints the resolved workspace path as its **sole stdout**, the
  bodied result type carries `stdout`, and the workflow binds
  `workspace_path` from it. The placeholder never appears in any recorded
  value — it lives only in the deployed contract's command string.
- **Single resolution point:** the server resolves the root ONCE. Any second
  independent resolution discovered anywhere is a defect to report, not keep.
  The startup banner gains a `workspace_root` field so composition points
  (setup.sh today, the leg C verb later) READ the value instead of
  re-deriving it.

## R1 — server: `{workspace_root}` expansion (Rust)

Files: new `crates/aion-server/src/worker/workspace_root.rs` (logic + its
tests in a sibling or `#[cfg(test)]` — keep every file ≤500 LOC excluding
tests/comments); `crates/aion-server/src/worker/declared_body.rs` (491 lines
— add only the call seam, not the logic); `worker/mod.rs` (pub mod line);
`run.rs` (banner field).

1. Resolve the root at `DeclaredCommandDispatcher` construction (or
   equivalent boot seam): `aion_home()` → `home.join("clones")`. Store the
   `Result` — resolution failure must NOT crash boot; it surfaces as the
   terminal refusal when (and only when) a placeholder-bearing body
   dispatches.
2. In `run_declared_command`, before `ShellAction::new`: if the command
   contains `{workspace_root}`, expand every occurrence with the resolved
   root. At that moment: ensure the directory exists (`create_dir_all`;
   idempotent; loud terminal refusal naming the io error on failure);
   refuse terminally if the resolved root contains characters that would
   change the parsed command shape (whitespace, `$`, `'`, `"`, NUL) — the
   root is server-controlled so this is theoretical, but a silent reshape is
   exactly what the template layer exists to prevent. Log the expansion
   (tracing, `operation = "declared_command_dispatch"` family) with the
   resolved path.
3. Startup banner (`run.rs:198` area): add `workspace_root` alongside the
   existing address fields, value = the resolved root (or omitted/marked
   unresolvable if resolution failed — do not fabricate).
4. Error strings ride the existing `terminal:`/`retryable:` FFI prefix
   contract. All refusals are `terminal:` (retrying cannot change the
   server's home).

## R2 — the two documents (AWL)

**Start from the LIVE documents** `examples/assistant/awl/assistant.awl` and
`examples/assistant/awl/assistant_spike.awl` (these carry the no-caps ruling:
inputs `objective` + `repo_path` only, loops `max 1000000` as documented
formality). Graft the bodied provision from
`docs/design/aion-authoring/139-drafts/assistant_bodied_provision.awl` —
**do NOT copy the drafts wholesale: they predate the no-caps ruling and
carry dead `max_rounds`/`max_nudges`/`workspace_root` inputs.**

1. Action becomes `assistant_provision(run_id: String, repo_path: String) ->
   Provisioned` with `type Provisioned { exit_code: Int, stdout: String }`.
2. Body command: keep the draft's script shape (run_id single-component
   validation, rename-aside collision discipline `mv` to
   `.superseded-<stamp>-<pid>`, scratch `git init` vs `git clone`) with
   these changes: the root arrives as the server-expanded `{workspace_root}`
   argv word (`-- {workspace_root} $run_id $repo_path`), NOT `$workspace_root`
   (the checker refuses interpolating a parameter the action does not
   declare; braces are literal text and pass); every tool's output is
   redirected to stderr (`git init`/`git clone` chatter must not pollute the
   contract stdout); the script's final statement is
   `printf '%s' "$1/$2/repo"` — the resolved workspace path as the SOLE
   stdout. Keep the empty-root guard (`$1` empty → exit 9) as
   defence-in-depth against a defective expansion.
3. Workflow: `assistant_provision(run_id: workflow.id, repo_path: repo_path)
   -> provisioned` then `provisioned.stdout -> workspace_path`. The old AWL
   composition `workspace_root + "/" + workflow.id + "/repo"` is DELETED —
   the recorded activity result is now the one source of the path
   (ratification condition 2). Everything downstream (`situate` branch,
   prompt splice, outcomes' `workspace_path`) reads the same binding,
   unchanged.
4. **Byte-identical body across both documents** — they share queue
   `default`; different bodies for one action name refuse every dispatch
   terminally (`declared_body.rs` Ambiguous arm). Copy, don't retype.
5. Update the in-document comments that say the CALLER states the root — the
   server states it now. Keep the draft's excellent comments on collision
   discipline and the stdout contract; adjust the `Provisioned` doc to name
   why `stdout` is in the result.
6. `aion awl check` both documents (one file per invocation), then
   `aion awl fmt` (formats in place), inspect diff, check again. The checker
   is the oracle — if it refuses a shape this brief assumed, STOP and report
   the diagnostic verbatim; do not invent syntax around it.

## R3 — tests

- `workspace_root.rs` unit tests: expansion replaces every occurrence;
  command without the placeholder passes through byte-identical; unresolved
  root → terminal refusal naming the reason; shape-changing characters in
  root → terminal refusal; directory created when missing.
- `declared_body.rs` (or its test module) integration: a
  `DeclaredBodyLookup::Declared` command bearing `{workspace_root}` executes
  with the expanded path observable in the result (e.g. `echo` the argv word,
  assert stdout equals the resolved root) — through the real dispatcher
  seam, tokio multi-thread like the existing tests.
- Worker admission: verify the existing `examples/agent-dev/worker` crate
  still ADMITS against the new contract (the #65 machinery: contract
  declares a body for provision — confirm the worker is not refused for
  advertising/not advertising it). If admission refuses, **STOP and report**
  — that changes the lane's scope (setup.sh worker story) and is not yours
  to improvise.
- Example crate: `cargo test` in `examples/agent-dev/worker` —
  `descriptors_tests.rs` is contract-derived and must stay green against the
  reworked documents.

## Constraints (violations are lane-stopping, not style)

- Main tree, NO worktrees, no branches — this repo runs a one-mutating-lane
  regime and you are the lane.
- No `cargo clean`, default target dirs only, incremental builds.
- **DO NOT deploy, route, start, or stop anything on any server.** In-tree +
  gates only. The live cut is a separately planned motion.
- Do not touch `scripts/setup.sh`, the `139-drafts/` files (they are a
  preserved record), or anything outside the files this brief names without
  reporting why first.
- CLAUDE.md standards in full: no unwrap/expect in libraries, thiserror
  errors, every failure handled or propagated, no `#[allow]`/`#[expect]`/
  `#[ignore]` bypasses — if clippy fires, fix the code.
- Gates before declaring done: `cargo clippy --workspace --all-targets -- -D
  warnings` clean, `cargo fmt` (write, not check), targeted test runs for
  aion-server + aion-awl + the example crate, `aion awl check` clean on both
  documents. Report every gate's exit status and counts verbatim — a gate
  you did not run is reported as NOT RUN, never implied green.

## Report back

Files touched with line counts; every gate command + verbatim tail of its
output; the exact final body command string (both documents — confirm
byte-identical, e.g. matching shasum of the extracted body line); any
diagnostic, refusal, or surprise, verbatim; anything this brief got wrong
about the code as found.
