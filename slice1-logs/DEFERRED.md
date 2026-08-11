# Deferred and corrected — worker bring-up slice one

What this lane did NOT run, and one claim it made too broadly. Written into the
tree because a deferral that lives only in a chat log is a deferral nobody
inherits.

## Deferred BY RULING: the full `cargo test -p aion-cli` suite

The complete `aion-cli` test suite is **not run in this worktree**. It runs once,
at landing, against the merge result on main's warm cache — the same pattern the
#200 landing gate used. The ruling is a disk decision: a cold `aion-cli`
integration build in a second worktree costs more headroom than this box has
while another builder holds its own target.

**What that leaves unrun, stated at its real width.** It is NOT true to say "the
integration binaries are unchanged". Their SOURCES are unchanged; the ARTIFACT
every one of them execs is not. `main.rs` now routes every invocation of the
binary through `parse_or_refuse`, so **argv parsing changed for every verb**, not
only for `worker agent`. The integration tail is therefore unrun here and swept
by reading; it runs in full at landing.

The sweep, so the reader can check it rather than trust it:

- `crates/aion-cli/tests/error_rendering.rs:39` asserts a RUNTIME exit-1 path, not
  a parse failure — untouched by the parse wrapper.
- No test in the tree asserts on the string `unexpected argument`, so no test
  reads the text clap emits before the advice.
- `--help` and `--version` still exit 0: clap classifies both as errors with
  exit code 0, and `parse_or_refuse` preserves `error.exit_code()` exactly rather
  than substituting one.
- The advice itself is now gated on the `worker agent` subcommand path (NEW-1),
  so no other verb's failure output changed at all.

Each of those four lines is checked against the REBUILT binary in
`slice1-logs/R3-verb-gate-live.log`, not asserted from reading: `--help`,
`--version` and `worker agent --help` all report `[exit=0]` there, and the
`worker awl` arm shows clap's error with no advice appended.

What would falsify the sweep is a test that asserts on a parse-failure's exact
stderr for some other verb. None exists. The landing run is what proves it.

## Corrected: the m10 claim in commit `1beb5b04f` was overbroad

That commit's message says "Every gate log carries the command and its exit code
where it was regenerated this round". That is **not accurate**, and the record is
corrected here rather than rewritten:

- `slice1-logs/live-binary-acceptance.log` **does** carry a header (binary,
  version, tree, date) and an `argv` + `[exit=N]` line per case. That is the log
  the claim was true of.
- The `slice1-logs/R2-*.log` files carry **neither** argv nor an exit line. They
  are raw `cargo` output, redirected. Reading one tells you what happened but not
  what was run to make it happen, and a suite that crashed before printing its
  verdict line would look the same as one that printed nothing.

Of the round-three logs (`slice1-logs/R3-*.log`), six carry gate.sh's treatment
(argv, cwd, tree, date, exit code); `R3-verb-gate-live.log` carries its own
equivalent header with per-case argv and `[exit=N]` lines; `R3-build-awl.log`
and `R3-new3-seam-tests.log` carry neither and reconcile only by reading.
The R2 logs are left as they are — history is not rewritten to make an old
sentence true.

## Standing: the three shipped documents with no `harness` section

`crates/aion-server/assistant-embed/assistant.awl`,
`workflows/norn-fleet/norn_fleet.awl` and `workflows/norn-dev/norn_dev.awl`
declare agent actions and carry no `harness` section, so `aion worker agent`
refuses each by name until one is written. No section is added for them here:
every field is operator policy with no default, and the embedded assistant
document is content-hashed into package identity, so editing it changes the
assistant package hash. This needs Tom's or Waffles' word on the values.

## Standing: managed-worker records minted before this change

A `builtin` worker-deployment record stores its argv verbatim and durably, so a
record minted before the launch flags were deleted fails on its next supervised
restart and keeps failing until its restart budget is spent. There is no
migration and deliberately no tolerant parse; the refusal names the record-shaped
remedy (re-mint the deployment). Full reasoning in
`docs/workers/AGENT-WORKERS.md`, section "A managed-worker record minted before
this change".
