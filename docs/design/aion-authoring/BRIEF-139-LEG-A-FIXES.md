# Brief: #139 leg A review fixes — one pass, all findings

**Date:** 2026-08-05. **Seat:** Vesper Lynd. Companion to
`BRIEF-139-LEG-A-REWORK.md`; executes the Opus review's punch list with the
seat's rulings folded in. The uncommitted rework on main is the base — do
not revert any of it.

## Seat rulings the fixes must respect

- Refusal-widening lands without asking (F1). Permissive deviations do not.
- Value decisions are NOT invented here: no `retry` clause on the provision
  action, no reaper/retention policy, no symlink refusal on the root. Each
  is RECORDED in `BUNDLE-139-NOTES.md` (see "Record" below) instead.
- Old worker-side behavior was also one-attempt (its handler classified
  these failures terminal), so comment corrections in F5 are
  behavior-preserving, not a semantics change.
- F7's zombie machinery is DELETED, not kept: CLAUDE.md replace-don't-add.
  Live-cut safety is the seat's problem (replay never re-executes recorded
  activities; the route cut is planned separately) — not a reason to keep
  dead code.
- The `.aion-*` untracked directories the review flagged are Tom-ordered
  protected holds, not dirt. Leave them alone.

## The fixes

**F1 (MAJOR) — whole-word placement guard.**
`crates/aion-server/src/worker/workspace_root.rs`: `expand` refuses (new
`WorkspaceRootError` variant, named for the refusal) unless EVERY placeholder
occurrence stands as a whole top-level argv word of the command as the
template parses it — not inside any quoted region, not adjacent to
non-whitespace text. Implement against the real tokenizer semantics
(`crates/aion-worker/src/shell/template.rs`), not a regex guess: walking the
command with the template's own quoting rules (single quotes literal until
the closing quote, double quotes likewise, whitespace splits) is required.
Unit tests: accepted bare word; refused inside single quotes; refused inside
double quotes; refused glued to adjacent text (`x{workspace_root}`,
`{workspace_root}/sub`); refused twice-glued; accepted multiple bare
occurrences. The shipped documents' command must pass (it uses
`-- {workspace_root}` bare).

**F2 (MAJOR) — pin the derivation and the banner.**
Factor the derivation into a pure `fn root_under(home: &Path) -> PathBuf`
(home → `home/clones`), call it from `resolve()`, and pin it with a test
(`root_under(Path::new("/x")) == PathBuf::from("/x/clones")` plus a test
that renaming `CLONES_DIRECTORY` breaks). For the banner: `run.rs`'s
banner-line construction of the workspace_root string (resolved → display,
unresolved → `unresolvable: <error>`) moves into a small pure function with
its own two-case test; the banner calls it.

**F3 (MAJOR) — guard the shipped body's stdout purity end to end.**
In `examples/agent-dev/worker` tests (the crate already embeds the document
and depends on the worker SDK): extract the `run` command line from the
embedded assistant document (parse the compiled contract's body — derive,
don't copy the string), expand it via
`aion_server::worker::WorkspaceRoot::from_resolution(Ok(<tempdir>))` — if
aion-server as a dev-dependency is unacceptable (check the dependency
graph; no cycles), replicate expansion by `str::replace` and note why —
then execute through `aion_worker::shell::ShellAction` with real arguments
and assert stdout equals `<root>/<run_id>/repo` EXACTLY on: the scratch
branch (empty repo_path), the attached branch (clone of a local fixture
repo), and the rename-aside branch (pre-existing run_id directory). Also
assert the failure branches (malformed run_id, empty root) produce EMPTY
stdout. This is ratification condition 2's standing guard.

**F4 (MAJOR) — close the composition-point divergence.**
`scripts/setup.sh`: after the server is up, extract `workspace_root` from
the startup banner (same `banner_field` mechanism as the addresses; it is a
top-level banner field). If it reads `unresolvable: …`, fail setup loudly
quoting it. Otherwise `export AION_WORKSPACE_ROOT="<value>"` before
starting the worker, so the worker's own resolution
(`handlers.rs::resolve_workspace_root`, override wins) is BY CONSTRUCTION
the server's value — one resolved value threaded into both sides, from the
server that will use it. Comment it with exactly that sentence. Do not
change the worker's resolution order.

**F5 (MAJOR) — correct the false retry claims in both documents.**
`examples/assistant/awl/assistant.awl` + `assistant_spike.awl`: the comments
claiming "bodied actions retry by default" and "a non-zero exit fails the
dispatch and retries instead of returning" are FALSE (no declared `retry`
policy → exactly one attempt; the engine's stand-down is
`nif_activity_retry_dispatch.rs:136-140`). Rewrite them truthfully: a
non-zero exit fails the dispatch; with no declared retry policy the action
gets exactly one attempt and the failure reaches the operator; the
rename-aside discipline guards a RE-RUN on a colliding workflow id, not an
automatic retry. Keep the discipline itself unchanged. THE TWO BODIES MUST
REMAIN BYTE-IDENTICAL (comments differ from the body; the `run` line must
not change at all). Re-run `aion awl check` + `fmt` on both.

**F6 + F7 (MAJOR) — delete the fossils AND the zombie machinery they pin.**
In `examples/agent-dev/worker`: delete the worker-side provisioning path
entirely — the `assistant_provision` handler, `handlers/opening_prompt.rs`,
`handlers/skill_resources.rs`, their registration in `registry.rs`, their
tests, and the two wire_compat fossils
(`assistant_provision_input_matches_the_awl_action_declaration`,
`assistant_workspace_matches_the_awl_type_declaration`) plus the
`AssistantProvisionInput`/`Workspace` types they pin. The worker serves the
`assistant` agent action; the server runs provision from the contract. Keep
`resolve_workspace_root`/`workspace_root_from` — the harness `--workspace-
root`/`-C` composition still needs them (F4 threads the value in). Keep
every remaining test green; the derived-surface admission tests must still
pass (bodied actions are filtered from the expected set). If anything else
in the crate consumes the deleted modules, follow the thread and report it.

**F8 (MINOR) — honest docs on the four inert copies.**
`state.rs` `from_parts*` constructors: add one comment each stating the
field is inert there (no declared-body dispatcher is built), and correct the
`workspace_root()` accessor doc to say the banner claim holds for
`build`-constructed states. Do not change signatures.

**F9 (MINOR) —** `from_resolution` doc gains one sentence: single-derivation
is a convention the production path upholds, not an enforced invariant.

**F10 (MINOR) — property test tying the refusal set to the real parser.**
For every printable ASCII character c: root = `/absolute/with<c>char`;
either `expand` refuses, or the expanded probe command
(`printf %s {workspace_root}` — placeholder as a bare word) executed
through `ShellAction` yields stdout EXACTLY equal to the root. No
enumeration of "safe" characters anywhere in the test.

**F11 (MINOR) —** dispatcher-level tests for `ShapeChanging` and
`CreationFailed`: both must surface as `terminal:` naming the action, like
the existing `Unresolvable` case.

**F12 (MINOR) —** rename
`a_command_without_the_placeholder_passes_through_untouched` to claim what
it checks (`…_is_not_expanded`), or extend it to assert byte-identity at the
dispatcher seam. Either; be honest.

**F13 (MINOR) —** create the root with mode 0700
(`std::os::unix::fs::DirBuilderExt`, `.mode(0o700)`; this workspace is
macOS/unix — if a portability gate exists in the crate, follow its
pattern). Applies to directories `expand` creates; document that an
already-existing root's permissions are the operator's own.

**F17 —** `git add` both brief files
(`BRIEF-139-LEG-A-REWORK.md`, `BRIEF-139-LEG-A-FIXES.md`) so they land with
the code. Do NOT commit — staging the tree is fine, the seat commits.

## Record (append one section to BUNDLE-139-NOTES.md)

Append "## 2026-08-05: review verdicts and open questions" recording, each
in one or two sentences: (1) provision has exactly one attempt — whether it
should declare a `retry` clause is an undecided value (old worker behavior
was also single-attempt-terminal); (2) AWL has no way to mark a run-body
failure terminal vs retryable — language gap, recorded; (3) superseded
workspace directories accumulate unboundedly by design ("never deleted") —
retention/reaping is an undecided value; (4) the root may be an
operator-planted symlink and is followed — operator's own configuration,
banner reports the configured path; (5) `state.rs`/`run.rs` exceed the
500-line law pre-existingly — standing debt, not grown materially by this
lane; (6) mv-collision on same-second-same-pid superseded names is
effectively unreachable and nests rather than fails — recorded.

## Gates (all, before reporting done)

`cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo fmt`
(write) · `cargo test -p aion-server` · `cargo test -p aion-awl` · example
crate `cargo test` · `aion awl check` both documents + `fmt` zero-diff ·
`bash -n scripts/setup.sh`. Byte-identity of the two `run` lines re-proven
by hash. Report verbatim tails + exit codes; unrun = NOT RUN.

## Constraints

Unchanged from the rework brief: main tree, one mutating lane, no commits,
no server/deploy/route actions, no `cargo clean`, protected holds untouched,
CLAUDE.md standards absolute, no `#[allow]`/`#[ignore]` bypasses. If a fix
collides with the code in a way this brief did not anticipate, STOP that
item and report rather than improvise.
