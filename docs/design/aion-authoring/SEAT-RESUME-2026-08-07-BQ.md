# SEAT RESUME — marker BQ, 7 August 2026

Supersedes **BP**. BP's amendments 1–3 stand (all seven Review-13 findings closed,
eleven mutations, clippy green). This marker carries what happened after.

## 1. Tom's coherence complaint — DONE, deck rebuilt

His words: *"it's not coherent start to finish… order flow… greek flow… first
flow… make it a single workflow, a logical step… explain what the first block
is… it needs to work up to one that runs our gates kind of command — clones the
repository, runs checks and tests."*

The full write-up is **`docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md`,
section "AMENDMENT — the coherence rewrite"**. Read that, not this summary.

Short form: the deck is now ONE workflow, `repo_gate`, built in five stages
(`tutorial-examples/repo_gate-{1-skeleton,2-contract,3-clone,4-commit}.awl` and
`repo_gate.awl`), each checked, three of them deployed and run for real. It
clones `https://github.com/dtolnay/itoa`, runs `cargo clippy` and `cargo test`,
routes `passed`/`failed`. Green run and red run both measured. Way-one/way-two
worker fork DELETED — worker processes are one closing prose slide.

Deleted: `00-skeleton.awl`, `first_flow.awl`, `first_flow_awl.awl`,
`worker.toml`, and `gate_manifest`/`declared_actions` in the builder.

Build output: **30 slides, 7 excerpts, 5 documents checked, exit 0.**

Tom was told (Meridian DM, sent). He already made `order_flow` in the console;
the deck now says `repo_gate` and he was told to either remake it or translate.

## 2. THE OPEN ITEM — one battery failure fixed, NOT yet re-run

Battery `scratchpad/bp-workspace.log`: **RC_WS=101**. Reconciled WITHIN the run:
243 distinct targets (223 `Running` + 20 `Doc-tests`), 247 verdict lines,
**zero targets emitting no verdict** (so no crashed binary), 4209 passed,
2 failed, 0 ignored.

**Failure A — MINE, real, now fixed.**
`each_query_wire_code_renders_distinctly_end_to_end`
(`crates/aion-cli/tests/error_rendering.rs`). The mock's `"terminal"` case was a
CHIMERA: `not_running_with_type("ShuttingDown", "target run already reached
Completed")`. No server produces that pairing — the real terminal refusal is
`WorkflowTerminal` (`aion-server/src/api/handlers/error.rs:36`), and
`ShuttingDown` is built at exactly one place
(`aion-server/src/error.rs:645`) carrying that error's own message. My F3 change
made the renderer READ the type, so the chimera rendered "the server is shutting
down" for a run that had simply completed.

Fixed in the FIXTURE, not the renderer:
- `"terminal"` now carries `WorkflowTerminal`;
- new `"shutdown"` case carries the real `ShuttingDown` + its real message;
- `typed_error_type_from_status_details_reaches_stderr` now expects
  `server error type: WorkflowTerminal`.
- **5/5 green.** Mutation: deleted the `ShuttingDown` guard arm from
  `render.rs` (literal prior text) → `query shutdown` RED with the generic hint.
  Restored `shasum`-identical.

**Failure B — NOT mine, environment.**
`emitted_order_saga_schemas_are_semantically_equivalent_to_the_authored_originals`
(`-p aion-package --test schema_equivalence`) died on
`gleam export package-interface failed: HTTP error … hex.pm/api/packages/gleam_json`.
A hex.pm outage. Needs a re-run to confirm, not a code change.

## 3. ON RESUME — in this order

1. **Re-run the two failed targets**, then the full workspace battery on the new
   tree (`-p aion-cli --test error_rendering` and
   `-p aion-package --test schema_equivalence` first, they are quick).
   Battery script: `scratchpad/battery-bp.sh` (nohup+disown), log
   `scratchpad/bp-workspace.log`, `RC_WS=` appended last, marker
   `scratchpad/bp-battery.done`. **Delete the .done before re-running.**
2. **`cargo clippy --workspace --all-targets -- -D warnings`** — must be exit 0.
   Remember: `--workspace` resolves `preserve_order` ON, `-p <crate>` off; both
   runs are required.
3. **Review 14 on Opus** over the whole lane (reviews 6–13 ALL refused). Give it
   this marker, BP, and `scratchpad/review-13.jsonl`.
4. **Commit, three lots, paths named EXPLICITLY, never `git add -A`:**
   - (a) tutorial — `docs/authoring/build-tutorial.py`,
     `docs/authoring/tutorial-examples/aion.toml`,
     `docs/authoring/tutorial-examples/repo_gate-1-skeleton.awl`,
     `repo_gate-2-contract.awl`, `repo_gate-3-clone.awl`,
     `repo_gate-4-commit.awl`, `repo_gate.awl`,
     `docs/authoring/first-workflow.html`,
     `docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md`,
     plus the DELETIONS (`00-skeleton.awl`, `first_flow.awl`,
     `first_flow_awl.awl`, `worker.toml` — use `git rm`).
   - (b) the #7 lane, once a review passes.
   - (c) markers.
   **KEEP EXCLUDED:** `docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md`
   (1.1 MB raw agent JSONL under a `.md` path; issue #5's artefact; NOT deleted).
5. **Tom takes priority over all of it** — he is mid-tutorial.
6. **Teardown of the tutorial world when he is done — BY PORT**
   (`lsof -tiTCP:8099 -sTCP:LISTEN`), never a pgrep pattern. Also remove
   `~/gate-work-3`, `~/gate-work-4`, `~/gate-work-5`, `~/gate-work-red`,
   `~/gate-work-red2` (my proving clones).

## 4. Live state — DO NOT TOUCH

- **Tom's production pids 45425 (server) / 48399 (worker)**, `:8080` health 200.
- **Tutorial server pid 50822**, `~/my-first-flow`, `:8099`/`:50099`,
  `AION_HOME=~/my-first-flow/aion-home`. Deployed on it now: `repo_gate`
  (hash `12699b87e2…`, two superseded), plus earlier `first_flow`,
  `first_flow_awl`, `envprobe`, `gateprobe`.
- Tom's console Authoring list holds ONLY `order_flow.awl`, which he wrote.

## 5. Facts measured today that are not written anywhere else

1. A declared `run` body's environment is **`PATH` and nothing else** — no
   `HOME`. Measured with `run "env"`.
2. `cargo` works anyway, including fetching 53 crates from the registry.
   Measured twice (zero-dep crate and a crate with a dependency).
3. A non-zero exit fails the action → faults the step → lands in `on failure`.
   `crates/aion-worker/src/shell/action.rs:190`. So a gate needs no comparison.
4. `aion run` **exits 1** on a failed outcome.
5. `on failure` cannot take a `///` doc comment — "doc lines attach to
   declarations". Use `//`.
6. A step with no `on failure` faults the run outright with `AwlActivityFailed`
   carrying the command's own words; with one, `AwlOutcomeFailure` carrying
   your `reason`.
7. `cargo` writes progress to **stderr**, test verdicts to **stdout**.
8. The CLI error message says `workflow 9 returned error` — an internal
   counter, not the workflow id. **Unfiled defect, worth a task.**

---

## AMENDMENT 1 — 2026-08-07 08:2x AEST

**Both previously-failed targets re-run on the new tree, both GREEN.**

- `cargo test -p aion-cli --test error_rendering` — **5 passed / 0 failed, RC=0**.
  The chimeric-fixture fix holds. Log: `scratchpad/requick-cli.log`.
- `cargo test -p aion-package --test schema_equivalence` — **1 passed / 0 failed,
  RC=0**. The earlier red was the hex.pm outage and nothing else; confirmed
  transient, no code change was owed. Log: `scratchpad/requick-pkg.log`.

**Full workspace battery relaunched** on the new tree. Script
`scratchpad/battery-br.sh`, log `scratchpad/bp-workspace.log` (the previous
run's log preserved as `bp-workspace-prev.log`), `RC_WS=` appended last, marker
`scratchpad/bp-battery.done`. Box was quiet at launch — another seat's
`frame-cli-verbs-build` had just finished, 90 GiB free.

**Two defects filed, both measured, neither on recall.**

- **#151** — `aion run` prints `workflow 9 returned error` on a failed outcome.
  `9` is an internal counter, not the workflow id, so the one identifier the
  error hands the operator is the one that cannot be looked up. Worst exactly
  where the message exists to help: a gate went red and they want that run's
  history.
- **#152** — a `~` in `store.data_dir` is taken **literally**. Proven live, not
  by inspection: the server started **clean**, logged
  `"data_root":"~/tilde-probe/store"` verbatim, and created a directory named
  `~` in the cwd. Evidence `scratchpad/tilde-probe-server.log`; probe world
  removed, port 8097 free.
  The census behind it: no `shellexpand` anywhere in `crates/`, and the only
  `~` literals in the tree are JSON-Pointer escaping in
  `crates/aion/tests/awl_bc4_differential/report.rs:340`.
  🔴 The sting: `crates/aion-server/src/error.rs:31` — the server's own
  path-safety refusal tells the operator to *"move store.data_dir beneath the
  private Aion home (`$AION_HOME`, default `~/.aion`)"*. An operator who hits
  that refusal and follows the remedy verbatim lands in this defect. Same class
  as #145 — a refusal that names a remedy which cannot be followed. And because
  the store path is cwd-relative, launching from elsewhere next time silently
  yields a different, empty store with a clean banner both times.

ON RESUME items 1 (partially), 7 and 8-first-row are discharged. The rest of §3
stands unchanged.

---

## AMENDMENT 2 — 2026-08-07, battery + clippy on the new tree

**Full workspace battery, `cargo test --workspace --no-fail-fast`, RC_WS=101.**
Log `scratchpad/bp-workspace.log`.

Reconciled WITHIN the one run, keyed on the binary path under
`target/debug/deps/` — NOT on the `Running <label>` text, which collides (26
targets are all labelled `unittests`):

| quantity | count |
|---|---|
| distinct test binaries | 223 |
| doc-test targets | 20 |
| **declared total** | **243** |
| verdict lines | 247 |
| **deficit** | **0** |

The +4 surplus is fully attributed, not waved off: the `aion_server` lib
unittest binary emits **4** verdict lines and `Doc-tests aion` emits **2**, both
because they spawn child cargo processes (the #74/#75 gates). **A surplus cannot
hide a crashed binary; only a deficit can.** So: no SIGSEGV'd binary silently
dropping its `test result:` line.

**4210 passed · 1 failed · 0 ignored.**

**The single red is #85, not this lane.**
`outbox_crash_recovery_rearms_stranded_done_row_and_finishes` in
`-p aion-rs --test outbox_e2e`, failing with
`Store(Backend("SQLite failure: 'bad parameter or other API misuse'"))` — verbatim
one of #85's two recorded faces (libsql 0.9.30 double-close / use-after-free,
mechanism verified in the dependency source). Isolated re-runs **3/3 green**
(`scratchpad/outbox-iso-{1,2,3}.log`). `outbox_e2e` is untouched by this lane.

⚠️ **Caveat stated, not hidden**: `-p` and `--workspace` resolve features
differently, so the isolation control does NOT hold feature resolution constant.
What it establishes is that the failure is non-deterministic and its error
string matches a known dependency defect — not that the two resolutions are
equivalent. Review 14 has been asked to attack this attribution.

**Clippy, both resolutions, all exit 0, zero diagnostics.**

| run | RC |
|---|---|
| `--workspace --all-targets -- -D warnings` | 0 |
| `-p aion-rs --all-targets` | 0 |
| `-p aion-cli --all-targets` | 0 |
| `-p aion-client --all-targets` | 0 |
| `-p aion-server --all-targets` | 0 |
| `-p aion-store --all-targets` | 0 |

Logs `scratchpad/clippy-ws.log`, `scratchpad/clippy-<crate>.log`.

**REVIEW 14 DISPATCHED** on Opus. Brief committed to
`scratchpad/review-14-brief.md` — it carries the reviews-6-to-13-all-refused
history, the diff-driver trap, the evidence above marked VERIFY-DO-NOT-TRUST,
and six named attack surfaces (the five invariants; tests that pass while
measuring nothing; chimeric fixtures like yesterday's `ShuttingDown`/terminal-run
pairing; the tutorial's factual claims; whether each `gate_*` can see its own
subject; god files).

ON RESUME §3 items 1 and 2 are now discharged. Item 3 is in flight. Nothing
commits until it returns.
