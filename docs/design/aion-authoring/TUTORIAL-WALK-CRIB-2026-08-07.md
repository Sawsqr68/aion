# Tutorial walk — crib sheet, 7 August 2026

For walking `docs/authoring/first-workflow.html` with Tom, live.

**Tom's standard, in his words:** *"Just make sure that the tutorial is
accurate. I don't want to have to tell anybody to change anything from the
tutorial."* Everything below is measured against that bar.

## How this was proven

Not by reading the deck. The commands were **extracted from the built HTML and
executed verbatim** — `/bin/bash` fed the deck's own text, nothing retyped,
nothing substituted. That is the only method that can catch a command which
looks right on the slide and breaks when pasted, and it caught one immediately
(see D2 below).

Installed `aion 0.11.0` (`/Users/tom/.cargo/bin/aion`), world at
`~/my-first-flow` on `:8099`/`:50099`, torn down afterwards and verified free
**by port**. Tom's pids 45425 / 48399 alive at every checkpoint — 1d 16h,
`/health/live` 200.

## Three defects found and fixed today

**D1 — the tutorial shared a folder with his live server.** The config set no
authoring root, so it defaulted to `~/.aion/authoring`, where `assistant.awl`,
`assistant_spike.awl` and `git_status_sweep.awl` already live. A reader would
have seen someone else's real work inside the tutorial and could have edited
it. **Fixed in the deck** — the start command now carries
`AION_HOME=~/my-first-flow/aion-home`, and the "where things land" slide names
the concrete path instead of a variable.

**D2 — the availability command did not work when pasted.** 🔴 The `-d` payload
was wrapped across two lines to fit the slide. A `\` continuation *ends* a
quoted string, so the next line's leading spaces became an argument separator
and `curl` received only the first 42 characters — `EOF while parsing a value
at line 1 column 43`. It was valid shell, it looked perfect on the slide, and
the earlier self-run missed it because that run used a hand-typed `curl`
instead of the deck's. **Fixed** (one unbroken line) **and gated** — see below.

**D3 — the config needed hand-editing.** `data_dir` was `/Users/you/…`. A `~`
does not work there: the config parser is not a shell, so it is taken literally
and **creates a directory actually named `~`** — measured, and a defect in its
own right (silently accepted, `data_root` logged with the tilde intact; worth
its own row). **Fixed** — the config slide is now a paste-able
`cat > aion.toml <<EOF` block whose `$HOME` the shell expands as it writes.
Verified: `data_dir = "/Users/tom/my-first-flow/aion-data"` landed on disk with
nothing typed by hand. The run step's `/Users/you` placeholder went the same
way — it is now `$HOME`.

## The new gate, and its mutation proof

`gate_command_payloads()` in `build-tutorial.py` joins each command the way a
shell would, splits it the way a shell would, and asserts that whatever lands
in a `-d` / `--data` / `--input` slot parses as JSON. It does **not** check
syntax — the broken form was valid shell, and `bash -n` called it fine. It
checks the property the reader actually depends on.

**Mutation-killed with the literal prior text:** restoring the old two-line
wrap fails the build with `RC=1` at *column 43* — the same column the live
paste failed at. Restore was byte-identical by `shasum`, and the rebuild is
green. The `$HOME` guard in `config_heredoc()` was mutation-killed the same
way.

Build now reports: `ok 2 pasted command payload(s) parse as JSON`, and the
unused-file census reads `3 excerpts from 4 of 4` — it was reporting
`aion.toml` as unquoted once the config moved into a heredoc, so the census was
taught to count both doors. A census blind to one of its own doors reports a
published file as dead.

## The arc, with today's measured results — all verbatim from the deck

| Deck step | Result |
|---|---|
| `aion --version` | `aion 0.11.0` |
| config heredoc | `data_dir = "/Users/tom/my-first-flow/aion-data"`, no literal `~` dir |
| start server | `health 200`; `authoring_root` inside `~/my-first-flow/aion-home` |
| `aion awl check first_flow.awl` | `ok: first_flow.awl (1 step)`, rc 0 |
| `aion … deploy first_flow.awl` | rc 0, `freshly_loaded: true`, `route_changed: true` |
| `aion … worker shell …` | registers |
| availability probe | `{"available":true,"connected_workers":1,"scaffold_hint":null}` |
| `aion … run … --input "{\"folder\": \"$HOME\"}"` | rc 0, outcome `done`, real listing returned |
| console | `GET /` 200, `/awl/documents` 200 |

## ⚠️ The one step NOT proven

**That the completed run appears in the console listing.** The console serves
(200) and `/awl/documents` answers, but my check of the run list failed to
parse — I guessed the wrong endpoint path — and I tore the world down before
re-checking. So the claim above is *"the console serves"*, not *"the run is
listed"*. Watch that step when he walks it; it is the first thing to confirm
by eye. Do not let the crib sheet's 200s stand in for it.

Also still unproven, and the deck says so itself: **the `kill -9` durability
step.** It is offered as "try it yourself next". ⚠️ It touches the beamr JIT
blocker — keep long workflows off.

## Traps to watch for during the walk

1. **`connected_workers: 0` with everything apparently fine** — the
   shared-queue silent refusal (`SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md`,
   `642f30579`). A worker is admitted only if it advertises the **full** action
   set every deployed document declares for that queue. A second document
   reusing the `desk` block with a different action makes the queue unservable
   — silently, both sides — and **a live worker is not evicted**, so it breaks
   at the *next* restart, detached from the deploy that caused it.
2. **Two console gaps from marker AO remain unfixed** — where the file lands,
   and the name rule. The deck answers both in prose. ⛔ Do not rebuild the
   console bundle without talking to Tom.

## His open question from marker AP, still unanswered

Is the deck for him with someone over his shoulder, or for someone alone with
nobody to ask? It was built for the first. Worth asking while walking it.

## Method note earned today

The earlier self-run declared this deck proven while D2 sat in it, because that
run **retyped the commands instead of executing the deck's own text**. A proof
that paraphrases its subject is not a proof of the subject. Same family as the
teardown that printed `world removed: yes` while the server was still
listening: target the identity that actually distinguishes the thing — the
published bytes, the listening port — and read the receipt back from that same
identity.

---

# AMENDMENT — the coherence rewrite, 7 August 2026

## What Tom said

> "it's not coherent start to finish — the first thing says to make a workflow
> called order flow, then it started talking about a greek flow, then it starts
> going through a first flow. Make it coherent start to finish, a single
> workflow, a logical step. And I'd prefer it wasn't just 'cut and paste this'
> — explain what the first block is, what the second block is with the types.
> It needs to work up to one that runs our gates kind of command — clones the
> repository, runs checks and tests."

He was right on every count, and the third name he heard as "greek flow" was
`greet_flow`. The deck named three different workflows: `order_flow` in the
naming rule, `greet_flow` in the skeleton, `first_flow` in the walkthrough —
plus `first_flow_awl` in the way-one/way-two fork. Four names, one tutorial.

## What the deck is now

**One workflow, `repo_gate`, from the first slide to the last.** It is built up
in five stages, each a real checked file, each deployed and run before the next
block is added:

| file | what it adds | proven |
| --- | --- | --- |
| `repo_gate-1-skeleton.awl` | what the console writes | derived from the server's own template |
| `repo_gate-2-contract.awl` | `input`, `outcome`, `type` | `ok: (0 steps)` |
| `repo_gate-3-clone.awl` | `worker`, `action`, `run`, `step`, `route` | run — `outcome: passed`, real clone |
| `repo_gate-4-commit.awl` | a second action, a second field | run — `"commit": "1577ed9"` |
| `repo_gate.awl` | `checks`, `tests`, a second step, `after`, `on failure` | run green AND run red |

The gate clones `https://github.com/dtolnay/itoa`, runs `cargo clippy` and
`cargo test` against it, and hands back the commit it judged plus both
transcripts. Nothing was reworded: the deck's outputs are the runs' bytes.

The way-one/way-two fork is **gone**. Worker processes are one closing slide of
prose, not a decision the reader is asked to make mid-tutorial. That fork was
half of what made it incoherent.

## Measured facts the deck now states, none of which were known before

1. **A declared body's environment is `PATH` and nothing else.** No `HOME`, no
   shell variables. Measured with `run "env"` — one line came back.
2. **`cargo` works anyway**, including fetching 53 crates from the registry,
   because it finds its home without `HOME`. Measured twice: a zero-dependency
   crate and a crate with a dependency.
3. **A non-zero exit fails the action, faults the step, and lands in
   `on failure`.** So a gate needs no comparison logic at all — the exit status
   IS the verdict. `crates/aion-worker/src/shell/action.rs:190`.
4. **`aion run` exits 1 on a failed outcome.** So the gate drops into any
   existing build system with no glue. Measured, `EXIT=1`.
5. **The two failure shapes differ and both are useful.** With `on failure`:
   `AwlOutcomeFailure` carrying your own `reason`. Without one:
   `AwlActivityFailed` carrying the command's own words —
   `the declared command 'git' exited 128: fatal: destination path … already
   exists`.
6. **`cargo` writes progress to stderr, test verdicts to stdout.** Read the
   wrong stream and the report is empty with nothing to say why. The first
   draft of the gate had exactly that bug.

## Three new gates, and the one that caught itself

- **`gate_one_workflow_name`** — every example document must declare the same
  `workflow`. This is the direct mechanical cure for Tom's complaint: a name is
  not checkable by reading carefully, it is checkable by counting the distinct
  ones. Mutation: renaming one stage → refuses, naming both files.
- **`gate_skeleton_is_the_servers_own`** — the skeleton slide is derived from
  the format string in `aion-server/src/awl/documents.rs` and put through
  `aion awl fmt`, then compared byte for byte. Mutation: one field type changed
  → refuses, printing both. The deck cannot ask the server for it directly
  without leaving a document in the reader's own workspace.
- **`taught_verbs()`** — the list of `aion` verbs to check is now DERIVED from
  the commands printed on slides, not kept beside them. It found its own defect
  on first run: anchoring on `tokens[0]` skipped
  `AION_HOME=… aion server …`, so `server` was untaught and unchecked while
  the gate reported success. Fixed to find the `aion` token anywhere in the
  line; the count went 3 → 4.

**And the keyword gate could not see its own subject.** Deleting the `after`
row from the table left the build GREEN — the gate only ever looked at the
FIRST word of a line, and `after` sits in the middle of
`step verify after fetch`. Fixed by unioning two derivations: leading words as
before, plus the language's whole vocabulary read out of `Keyword::as_word` and
intersected with every word the examples contain. Both halves are needed —
`run` is parsed contextually and is not a lexer keyword, so the vocabulary half
alone would have missed the very word the previous drift was about.

The widened gate immediately found a real gap: `success` was on the first line
the reader writes and nothing explained it. Now it does.

## Mutations run (each with the literal prior text, each restored `shasum`-identical)

| # | mutation | result |
| --- | --- | --- |
| M1 | rename `workflow repo_gate` → `clone_flow` in one stage | RED, names both files |
| M2 | change the skeleton's field type | RED, prints server's vs deck's |
| M3 | delete the `after` row (before the fix) | **GREEN — the gate was blind** |
| M3b | delete the `after` row (after the fix) | RED |
| M4 | `deploy` → `deployy` on a slide | RED, `this binary does not answer: deployy` |
| M5 | delete the `success`/`failure` row | RED |

## What Tom needs to know

He already created `order_flow` in the console. The deck now says to name it
`repo_gate` and every command prints that file name. Either make a new one
called `repo_gate` and follow along literally, or keep `order_flow` and read
`repo_gate` as "whatever you called yours".

**Still nothing of his authored for him.** The console's Authoring list holds
only what he made.
