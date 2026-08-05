# Seat resume — marker AI, 5 August 2026

Supersedes AH. AH's record stands; everything below happened after it.
**Latest letter wins.** `main` = `origin/main` = `483c933a`, tree clean but for
the three NO-SWEEP untracked dirs.

## ✅ Tom asked for a real workflow, and got one that is proven

His words: *"a workflow that checks all the directories in a given directory,
sees which ones are git repositories, runs git status on each, and saves the
output to a file"* — plus *"can I see all that in the authoring surface in the
ops console?"*

**`examples/git-sweep/git_status_sweep.awl`** — three steps, every action a
`run` body, so there is no worker, no build, no process. Landed `9dc54b49`.

- Run `4a4f7e83-79d5-4e7d-b2ab-1fe337a30e58` → `outcome: swept`.
- **The report file is the oracle, NOT the exit code.** Deleted before the run,
  so its existence after is positive evidence: 2662 bytes, 7 `=== ` blocks
  (aion, beamr, frame, haematite, liminal, lys, norn).
- Visible in his console, measured not assumed: `GET /awl/documents` lists
  `git_status_sweep.awl`; `GET /awl/deployed` carries the route-active version
  with `source: {state: "available"}`. A copy lives at
  `~/.aion/authoring/git_status_sweep.awl` (mode 600) — that is where the
  running server looks.

## 🔴 THE FINDING THAT MATTERS — a green that did nothing

`docs/design/aion-authoring/DECLARED-BODY-FINDINGS-2026-08-05.md` has all three.

**Inside single quotes a declared command is LITERAL, dollar included, so `$$`
is not an escape there.** `$$1` reaches `sh` as its own pid + `1`. My first
version globbed against a directory named `753901`, wrote a file *named*
`753901` into the SERVER'S cwd (the aion repo root, content `753902`), and
returned `exit_code: 0` — **byte-identical payload to the correct run.** I
removed the stray file. Only checking for the report file caught it.

Verified, not inferred: one document declaring BOTH `'echo "$1"'` and
`'echo "$$1"'` checks `ok (2 steps)`. The checker cannot tell them apart.

**Closed in `483c933a`:** DECLARED-COMMANDS.md gains a three-region table
(bare / double / single) with each row cited to its pinning test
(`single_quotes_are_literal_including_dollar_signs`,
`a_doubled_dollar_is_a_literal_dollar`), and explains why the existing
`{workspace_root}` example writes bare `"$1/$2"`. WORKERS.md gains the same
rule **because that is what the assistant teaches from** — it showed
`sh -c '...'` with no mention of the dollar. Doc gate re-run after: EXIT=0,
41 PASS / 1 UNVERIFIABLE / 2 ILLUSTRATIVE / 0 FAIL, report unchanged.

## Still open — tasks #144 (part done) and #145

- **✅ #144 CLOSED (`4bed6a81`, pushed).** See the section below.
- **✅ #145 CLOSED (`fc1c10cc`, pushed).** See the section below.

## ✅ #145 CLOSED — a refusal now carries what the operator needs to act

`fc1c10cc`, pushed. `DeclaredBodyLookup::Ambiguous` carries the declaring
versions instead of a count — hash, workflow types, route-active — and
`crates/aion-server/src/worker/declared_body_ambiguity.rs` turns them into
runnable `aion unload <type> <hash>` commands. Three situations, three remedies:
superseded version left behind (retire it) · nothing route-active (deploy first,
THEN retire) · two route-active packages that genuinely disagree (nothing to
retire — reconcile the documents).

**The oracle is the deploy API's own parser, not a length.** A short hash is
refused by `decode_version_target`, so an abbreviated one would print a command
that cannot run. `the_printed_hash_parses_back_as_a_content_hash` takes a real
`ContentHash` through the same `to_string` the engine reader uses and parses the
printed token back. Watched failing against a deliberately truncated hash:
`InvalidLength { found: 8 }`, four tests dead.

Gates on this tree: clippy `-p aion-server --all-targets -D warnings` EXIT=0 ·
`cargo test -p aion-server` **944 passed / 0 failed**, EXIT=0, 45 targets · doc
example gate EXIT=0, 41 PASS / 1 UNVERIFIABLE / 2 ILLUSTRATIVE / 0 FAIL
(unchanged). One libSQL flake mid-run —
`schedule_namespace_enforcement_survives_restart`, *"bad parameter or other API
misuse"*, the known **#85** shape — passed alone and in the clean full re-run,
and is nowhere near this change. **Note the trap it exposed:** `cargo test`
STOPS at the first failing target, so a red run's totals are truncated, not
comparable. Re-run clean before quoting a number.

Docs moved with it: `DECLARED-COMMANDS.md` gains *"Changing a body makes a NEW
version, and the old one stays"* (deploy → unload → run); the findings doc
records finding 2 closed.

## ✅ #144 CLOSED — and adding a doc to the gate was not the one-liner it looked

`4bed6a81`, pushed. `DECLARED-COMMANDS.md` had **zero ` ```awl ` fences** — all
seven blocks untagged. Adding the path to `SOURCES` alone would have extracted
nothing and reported coverage of a document the gate never read. Two blocks are
real AWL (tagged + specced); the other five are annotated illustrations, and
tagging them to raise the count would report coverage of text that was never a
document. The specs entry says so where the next person will look.

**It paid on the first run.** The document's opening example returned
`RunOutcome`, a type it named twice and declared nowhere: a reader copying the
very first block got `unknown type RunOutcome`. Watched failing before the fix
(gate EXIT=1, `FAIL … 10:33`). Both examples now declare their return type, and
the doc states the result shape — `exit_code`, `stdout`, `stderr`, subsets
allowed — which it had never written down.

Gate now: EXIT=0, 46 blocks / 7 documents, **43 PASS / 1 UNVERIFIABLE /
2 ILLUSTRATIVE / 0 FAIL** (was 41 PASS from 6). Both new blocks pass with an
EMPTY scaffold. The stated limit stands: the gate catches syntax and type drift
and would NOT have caught the `$$` trap — the ✅/❌ pair it cannot check is
exactly the pair whose two halves both check.

## Method notes worth keeping

- **The SPA fallback answers everything.** Six invented API paths all returned
  `200` because the console serves `index.html` on any unknown path. Discriminate
  on the BODY (`<!doctype` = fallback), never the status code.
- **The authoring root is the home default.** `~/.aion/config.toml` says absent
  fields select the working-directory locations; that held for `./aion-data`
  (exists) and NOT for authoring — the server's own line reads
  `"authoring_root":"/Users/tom/.aion/authoring"`. Legacy location wins only
  when it exists. Trusting the comment puts documents where the console never
  looks.
- `aion` has no `history` verb. `aion versions` / `route` / `unload` are the
  operator verbs.
- A `collect` step name is refused — reserved keyword.

## Unchanged standing constraints

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker, and
restarting an already-built worker is not a rebuild. 🔴 #142 held on the #139
trigger. #141 queued behind the parked cut. 🔴 Tom's 0.11.1 decision is his and
unanswered — nothing published without his word; his `~/.aion/config.toml`
`[ops_console] FileSystem` line must stay until he installs a binary built from
`b20d2da1` or later. 🔴 beamr JIT blocker with Artemis. ⛔ Never `git add -A`
here. ⛔ Never bare `cargo clean`. Backticked commit messages via
`git commit -F -`. Do not edit `CLAUDE.md`.

Live pids, untouched all session: server **45425**, worker **48399**, both from
the repo root. Server log: scratchpad `server.log` (fd 1 and 2, still open).

## Commits this session, all pushed

`23f1e2af` (battery re-earned on the final tree — an embedded asset is source) ·
`9dc54b49` (git-sweep example + three findings) ·
`483c933a` (#144 docs: no `$$` escape inside single quotes) ·
`0ce74ea0` (this marker) ·
`fc1c10cc` (#145: the refusal names a followable remedy) ·
`2d5d3a6c` (this marker, #145) ·
`4bed6a81` (#144: the doc joins the gate; the gate finds an undeclared type).

**Board after this:** #144 and #145 both closed. What is left is all held or
queued, not startable: #139 live cut and #142 wait on Tom; #141 queues behind
the parked cut; the 0.11.1 decision is his and unanswered.
