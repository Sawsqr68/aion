# Battery of record — lane 42-fork-ordering

**Tree:** `56c5a71db` ("docs(42): the built-vs-expressible answer, and the
instrument proven red"), off base `4150d3817`, off main `efc71eda4`.
**Standard:** `ablative/docs/GATE-EXTRACTION-STANDARD.md`.
**Script:** `gate-logs/42-fork-ordering/battery.sh` — the standard's fenced
block **copied verbatim** (only a `#!/bin/sh` shebang prepended; `diff` against
the standard's fence is empty). `NORN_BIN=/Users/tom/.cargo/bin/norn` set for
every run so the agent gates execute.

---

## Instrument proven red first

Before trusting an empty extraction, the instrument was run once against a
deliberate plant (an unused variable plus an always-false assert, appended to
`crates/aion/tests/concurrency_e2e.rs`, then reverted). **All three legs
emitted** — `gate-logs/42-fork-ordering/battery-redproof.jsonl`:

```
{"type":"clippy","level":"warning","lint":"unused_variables","file":"crates/aion/tests/concurrency_e2e.rs","line":1133}   <- check leg
{"type":"clippy","level":"error","lint":"unused_variables","file":"crates/aion/tests/concurrency_e2e.rs","line":1133}     <- clippy leg
{"type":"clippy","level":"error","lint":"clippy::assertions_on_constants","file":"crates/aion/tests/concurrency_e2e.rs","line":1134}
{"type":"nextest","test":"aion-rs::concurrency_e2e$battery_instrument_control_planted_failure"}                            <- nextest leg
{"type":"nextest","test":"aion-rs::concurrency_e2e$a_parallel_layer_records_one_trail_whichever_member_settles_first"}
```

The plant was then reverted (`git checkout --`, tree clean) before the run of
record.

---

## Verdict — the verbatim run

`gate-logs/42-fork-ordering/battery-final.jsonl`, **exactly one finding**:

```json
{"type":"nextest","test":"aion-rs::concurrency_e2e$a_parallel_layer_records_one_trail_whichever_member_settles_first","message":"test failed"}
```

- **check leg: clean** (zero lines).
- **clippy leg: clean** (zero lines) — `-D warnings`, `--all-targets`,
  `--keep-going`, no `#[allow]`/`#[expect]`/`#[ignore]` anywhere in the lane.
- **nextest leg: one finding**, and it is this lane's charter-mandated red
  reproduction, left deliberately red because REPORT.md §3 reserves its
  resolution to the steward. It is not a regression and it is not silenced.

No fmt gate (builders autoformat; `cargo fmt -p aion-rs` was run).

---

## Completion proof

The verbatim script sends every leg's stderr to `/dev/null`, so an empty
extraction alone cannot distinguish "ran and found nothing" from "never ran".
A second pass at the **same tree** ran the **same three commands with the same
jq**, keeping each leg's stderr (`completion-proof.sh`; artefacts
`proof-check.stderr`, `proof-clippy.stderr`, `proof-nextest.stderr`,
`proof-findings.jsonl`). The verdict of record remains the verbatim run; this
pass exists to prove those legs executed.

**nextest Summary line:**

```
Summary [ 223.114s] 5137 tests run: 5135 passed (6 slow, 1 leaky), 2 failed, 0 skipped
```

5137 = the 5135 of the base tree plus this lane's two new tests. Per-leg stderr
kept at 312 B (check), 313 B (clippy), 639 KB (nextest).

**Caveat on `proof-legs.log`:** its `rc=0` lines are `jq`'s exit status, not
cargo's — the script is `/bin/sh`, which has no `PIPESTATUS`. Those three lines
are **not** a verdict and must not be read as one. Per the standard, "rc codes
alone are not a verdict under this style — the finding lines are." The Summary
line above is the completion evidence; the finding lines are the verdict.

---

## The two runs disagreed by one test — disclosed, investigated, not mine

The verbatim run found **1** failure; the completion-proof run found **2**. The
extra one:

```
FAIL [0.155s] (1144/5137) aion-cli::bin/aion worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group
  Error: ParseIntError { kind: Empty }
```

**Diagnosed, not waved away.** `wait_for_pid_file`
(`crates/aion-cli/src/worker_shell.rs:711-721`) waits only for `path.is_file()`,
but the fixture shell writes the pid with `echo $$ > "$1"` — the redirect
**creates and truncates** the file before `echo` writes into it. `read_pid`
(`worker_shell.rs:729-731`) then does `read_to_string(path)?.trim().parse()?`
and parses an empty string. There is a real create-vs-write window, and a
loaded box widens it.

- **Pre-existing, not this lane's.** The file was last touched by `1712a8086`
  ("Merge branch 'fix/worker-shell-dial-and-audibility'"), an ancestor of this
  lane's base. This lane modified exactly one source file,
  `crates/aion/tests/concurrency_e2e.rs`, in a different crate.
- **Confirmed load-sensitive:** 3/3 green when run solo
  (`cargo test -p aion-cli --bin aion -- ...cancellation_reaps...`), red under
  the full-workspace run.
- **Not fixed, by charter.** The lane's zone is `crates/aion`; the charter
  requires reporting spill beyond it before editing. This is reported, not
  edited.
- It is the same *class* as this lane's defect — a check sampling a state that
  is only eventually consistent — but a distinct object, and it deserves its
  own row.

**Honest reading of the disagreement:** one battery run is not a measurement of
flake. The contended run is the informative one here; the verbatim run's
1-finding result should not be read as evidence that `aion-cli` is clean.

---

## Non-claims

- The battery does **not** claim a green tree. It claims: check clean, clippy
  clean, and a nextest leg whose only lane-owned finding is the deliberate red
  reproduction.
- `covered::covered_set_is_byte_identical` (the differential that motivated the
  lane) **passed** in both runs. That is not evidence the defect is absent —
  five solo greens already existed. Nothing here re-measures it.
- `.aion-held-execution.json` is gitignored and untracked; verified with
  `git ls-files`.
