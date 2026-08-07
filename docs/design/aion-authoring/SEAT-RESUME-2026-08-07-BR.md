# SEAT RESUME — marker BR — 2026-08-07

**Supersedes BQ.** BQ + its amendments 1–2 still carry the battery/clippy record.

---

## 1. REVIEW 14 CAME BACK — **REFUSE**, 7 findings

Reviews 6–13 all refused; 14 makes it eight. Its findings are GOOD — I verified
every severe one at my own hand and every one I checked was **right**.

Full text: the agent's return in the transcript. Summary + my verification:

| # | finding | verified? | fixed? |
|---|---|---|---|
| 1 | deck taught `timeout` as a real bound on a declared command — it is INERT | ✅ mine | ✅ |
| 2 | "being refused is silent on both sides" — false on BOTH sides | ✅ mine | ✅ |
| 3 | the chimeric query fixture was replaced with ANOTHER chimera | ✅ mine | ⬜ **OPEN** |
| 4 | deploy output block omits `deployed_entry_module` | ✅ mine | ✅ |
| 5 | 39 lines of rationale attached to the WRONG test (`api.rs:1224-1262`) | ⬜ | ⬜ **OPEN** |
| 6 | two gates report success without checking their subject | ✅ mine | ✅ |
| 7 | minors: workdir mismatch · "Forty lines" · underscore names · rust-version · `RefusingReadStore` | ✅ (4 of 5) | ✅ (4 of 5) |

---

## 2. WHAT I FIXED THIS TURN — all in `docs/authoring/`

### F1 — `timeout` is inert (THE BLOCKER; Tom was reading this)
`crates/aion-server/src/worker/declared_body.rs:280` says verbatim *"Held, not
wired: nothing cancels a single declared-command attempt today."*
`docs/workers/DECLARED-COMMANDS.md:239-246` — which the deck cites — says
*"Nor does a declared `timeout` bound it… nothing on the Rust side reads that
field."* The deck said `timeout` = "how long one attempt may take" and put
`timeout 30m` on both cargo commands. **A false safety claim.**

- **Removed all 7 `timeout` lines** from `repo_gate-3-clone.awl` (1),
  `repo_gate-4-commit.awl` (2), `repo_gate.awl` (4). All 5 documents still
  `aion awl check` clean.
- Removed both `timeout` table rows.
- **Added a 🔴 bullet to the safety slide** naming it: the word exists, it does
  nothing on a declared command, cancellation is not wired either, so
  `timeout 30m` on a hung `cargo test` will not end it — **bound the command
  inside the command**.

### F2 — the refusal-silence claim
`crates/aion-server/src/worker/contracts.rs:115-119`: *"**Every refusal this
gate returns, it also SAYS** — at WARN… naming the queue, the node, the worker
build and the reason."* `api/worker_grpc.rs:660-662` maps `Mismatch` to
`failed_precondition(error.to_string())` — the worker gets the full diagnosis.
Rewrote to "**you are told, twice**" and corrected the over-broad "full set of
actions every deployed document declares" to the actually-reachable set
(narrowed twice: reachable versions only, this node only).

### F4 + the moved hash — RE-MEASURED IN A CLEAN WORLD
Removing `timeout` changed every content hash, so the deploy slide had to be
re-measured, not edited. Built a clean world (`~/tut-clean`, port 8098/50098,
own store) and **replayed the reader's exact sequence**: stage-3 `run` →
stage-4 `run` → final `deploy` → green `run` → no-`on failure` case → red gate.

New deploy block (real bytes, and now 7 keys incl. `deployed_entry_module`):
- `content_hash` / module: `9926c2fe0c205d11ceae7799c132f98cc5a46610127b963f7d5f2318b66fb5b0`
- `freshly_loaded: true`, `route_changed: true`
- superseded: `0d5f38c4…`, `24d3b951…` — **exactly two**, the reader's own
  stage 3 and stage 4. (My old deck showed three, from MY iterations.)
- 🔑 **The same file deployed to a DIFFERENT server produced the SAME hash** —
  an independent confirmation that the hash is content-derived. Now said on
  the slide.

Green run re-measured with the deck's own `$HOME/gate-work-5`, so command and
output agree by construction. It revealed a teaching point worth having:
`cargo test` emits **three** verdicts (lib 0, integration 11, doc 2) — added a
bullet, because a gate reading only the first would call a repo green with
broken doc examples.

Red gate re-measured: `workflow 8 returned error` /
`AwlOutcomeFailure` / `reason: the gate did not pass`, EXIT=1.
No-`on failure` case: `workflow 7` / `AwlActivityFailed` / git's exit-128 words.
**Added a 🔴 bullet telling the reader to IGNORE the number** — it is #151's
internal counter, not an id they can look up. (It printed 4, 5, 7, 8 across my
runs — more evidence for #151.)

### F6 — the two blind gates
- `gate_verbs` checked only that the VERB resolved. Now derives the FLAGS from
  each slide too and checks each against that verb's help (falling back to root
  help, because `--endpoint` is global). `taught_verbs()` returns
  `{verb: flags}`; also fixed a second blindness — a positional argument used
  to `break` the token loop, so flags AFTER a filename (`run file.awl --input`)
  were never collected.
- `main()` **printed** the binary's version and never compared it. Now refuses
  on mismatch.
- `gate_command_payloads` keyed on an allowlist of flags (omitting
  `--data-binary`/`--data-ascii`). **Re-keyed on the SHAPE of the argument** —
  anything opening `{`/`[` must be JSON, whichever flag carries it — plus a
  zero-floor that refuses rather than passing on nothing.

### F7 minors
- command said `$HOME/gate-work`, output said `gate-work-3` → command fixed.
- "Forty lines" (was 43, now 39) → **derived**: `FINISHED_LINE_COUNT` counts
  the file.
- "starting with a letter or underscore" → **PROVEN WRONG LIVE**: a document
  named `_repo_gate` fails with ``stray character `_` ``. The server's
  `validate_document_name` (`awl/documents.rs:264-272`) accepts a leading `_`
  the lexer (`aion-awl/src/lexer/cursor.rs:100-104`) rejects. Deck corrected;
  **the underlying defect still needs filing** (see §5).
- install slide said "if you have Rust" while `crates/aion-cli/Cargo.toml:11`
  is `rust-version = "1.88.0"` (workspace floor 1.85) → now names the floor,
  **derived** from the manifest via `CLI_RUST_MIN`.

---

## 3. MUTATIONS RUN THIS TURN (all restored `shasum`-identical)

| # | mutation | result |
|---|---|---|
| M7 | version check deleted (literal prior text) + stub `aion` on PATH | 🔴 **the stub got `ok` from `gate_documents` on all 5 documents AND from the skeleton gate** — that is the vacuity the check closes |
| M8 | same stub, check PRESENT | RED, names both versions |
| M9 | `--endpoint` → `--endpointt` on one slide, real binary | RED: `aion run --endpointt` |
| — | `"--endpoint"` (wrong literal) | **NON-RUN** — assertion caught it, not counted |

`gate_command_payloads`' zero-floor is **NOT mutation-proven** — stated, not
claimed.

---

## 4. 🔴 STILL OPEN — REVIEW 14 FINDINGS 3 AND 5

### Finding 3 — `crates/aion-cli/tests/error_rendering.rs:123-153`, `:369`
**I verified this myself and the reviewer is right.** My "fix" of yesterday's
chimera introduced a second one.

- `WorkflowTerminal` is built at exactly two sites,
  `api/handlers/error.rs:35` (`signal_terminal_error`) and `:42`
  (`cancel_terminal_error`), called ONLY from `workflows.rs:182` (signal) and
  `:278` (cancel).
- **A query against a terminal run does NOT produce it.** It returns an **`Ok`**
  response carrying `QueryNotRunning` on the `QueryResponse.error` oneof —
  `workflows.rs:248-252` routes `EngineError::Query(_)` that way, and
  `error_engine.rs:57` + `query_wire()` make it `QueryNotRunning` via
  `not_running_with_type`. The server's OWN test pins it:
  `workflows.rs:574-595` `query_handler_returns_not_running_outcome_for_terminal_workflow`.

**WHAT TO DO** (I had just started this when the ceiling hit):
1. In the CLI mock's `query` handler, the `"terminal"` case must become an
   **`Ok(outcome_error(...))`** carrying `QueryNotRunning` — not
   `Err(typed_status(FailedPrecondition, WorkflowTerminal))`. Model it on the
   existing `"fails"` case at `:112`, which already rides the oneof correctly.
2. Keep the `"shutdown"` case — `ShuttingDown` IS reachable
   (`aion-server/src/error.rs:645`) and its renderer arm in
   `crates/aion-cli/src/render.rs` is correct and already mutation-proven.
3. If `WorkflowTerminal` rendering deserves coverage, exercise it through
   **signal or cancel**, where it actually occurs.
4. Rewrite the 🔴 comment block at `:123-133` — it is factually wrong as
   written (it asserts a query-terminal produces `WorkflowTerminal`).
5. Re-run `cargo test -p aion-cli --test error_rendering` and mutation-prove.

### Finding 5 — `crates/aion/src/engine/api.rs:1224-1262`
39 lines of rationale (including "the decisive observable is (c), not (a)" and
the `installed_workflow_catalog() == None` argument) document
`a_failing_teardown_step_does_not_skip_the_ones_after_it` but are ATTACHED to
`a_second_failing_engine_shutdown_step_is_reported_not_swallowed` at `:1280`,
which makes no such assertion. The test it actually describes is at `:1324`
and has no doc. Both new in this lane. **Not yet verified at my hand.**

### Finding 7 residue — `RefusingReadStore`
`crates/aion-store/src/testing/refusing_reads.rs:196-201` accepts writes while
`:210-233` disclaims every shard. Reviewer's own impossible-state argument, one
layer over. **Not yet verified at my hand.**

### Reviewer's own RELAYED-NOT-VERIFIED list (treat as leads, not findings)
- the `run`-body-after-config-line "rule" may be a printer convention, not a
  parser constraint;
- `config_heredoc`'s only check is `"$HOME" in body`;
- `taught_verbs` is blind to verbs taught in PROSE (`aion awl scaffold` at the
  "When a command is not enough" slide).

---

## 5. TASKS

- **#151 FILED** — CLI prints `workflow <n>` where the workflow id belongs.
  Now confirmed to vary per world (4, 5, 7, 8 observed). The deck warns about
  it explicitly.
- **#152 FILED** — a `~` in `store.data_dir` is taken literally; proven live.
- 🔴 **STILL TO FILE**: `validate_document_name` accepts a leading `_` that the
  AWL lexer rejects — one truth in two places, and the reader meets it as
  ``stray character `_` ``. Proof:
  `printf '//! _r workflow.\nworkflow _r\n  outcome done: type Placeholder, route success\n\ntype Placeholder { value: String }\n'`
  → `aion awl check` → `2:10: error: stray character `_``, EXIT=1.

---

## 6. ON RESUME — in this order

1. **Tom takes priority.** He is mid-tutorial. The deck is now correct on every
   point above; if he asks, the headline is: `timeout` was a lie and is gone.
2. **Close finding 3** (recipe in §4). Then finding 5, then the
   `RefusingReadStore` item.
3. **File the leading-underscore defect** (§5).
4. **Re-run** `cargo test -p aion-cli --test error_rendering`, then the full
   workspace battery on the new tree, then clippy both resolutions.
   (Previous battery: 243 declared / 247 verdicts / **0 deficit** / 4210 passed
   / 1 failed — the single red is #85 libsql, 3/3 green isolated. Clippy exit 0
   on all six runs.)
5. **Review 15 on Opus.** Give it BR, BQ, BP, `scratchpad/review-14-brief.md`.
6. **Commit** in three lots, paths named EXPLICITLY, never `git add -A`.
   Lot (a) tutorial now also includes the four re-measured slides.
   **KEEP EXCLUDED**: `docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md`.

---

## 7. LIVE STATE — TEARDOWN OWED

- 🔴 **Tom's production pids 45425 / 48399 — DO NOT TOUCH.**
- **Tutorial server pid 50822**, `~/my-first-flow`, `:8099`/`:50099` — Tom's.
  `repo_gate` there is now the OLD (pre-`timeout`-removal) hash plus one
  re-deploy of the new one.
- 🆕 **My clean world: pid 92685, `~/tut-clean`, `:8098`/`:50098`.**
  **TEAR DOWN BY PORT**: `lsof -tiTCP:8098 -sTCP:LISTEN`, never a pgrep pattern.
  Then `rm -rf ~/tut-clean`.
- 🆕 Proving clones to remove: `~/gate-clean-3`, `~/gate-clean-4`,
  `~/gate-clean-5`, `~/gate-clean-red`, `~/gate-work-3`, `~/gate-work-4`,
  `~/gate-work-5`, `~/gate-work-red`. (`~/gate-work-red2` already removed.)
- Probe world `~/tilde-probe` already removed; port 8097 free.
- Scratchpad shim `scratchpad/shimbin/aion` — a stub used as a positive
  control. Harmless, but it is a fake `aion`; do not leave it on PATH.

## 8. BUILD OUTPUT AS OF THIS MARKER

```
gating the tutorial with aion 0.11.0 (/Users/tom/.cargo/bin/aion)
      ok  all 5 documents are one workflow: `repo_gate`
      ok  the skeleton slide is the server's own template
      ok  all 4 taught verbs resolve and all 6 flag use(s) are documented: server, awl check, run, deploy
      ok  2 pasted command payload(s) parse as JSON
      ok  all 13 keyword(s) the examples use are explained
wrote docs/authoring/first-workflow.html — 30 slides, 7 excerpts from 6 of 6 example files, 5 documents checked
```
(13 keywords, not 14 — the gate saw `timeout` leave the examples.)
