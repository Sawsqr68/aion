# Ship & Run → worker authoring: direction

**What this is.** Direction capture, 2026-08-08, answering Tom's ask of 2026-08-07:
after walking the tutorial he said writing your own worker with AWL needs to be
understood and served better — "the ship and run thing that we have at the end
needs to better serve it." This document maps the ground truth, names the
verified defects, and proposes the build direction. It is not yet a
build-approved design; decisions are listed at the end and routed to Waffles.

**Papers of record it extends** (it re-litigates none of their rulings):
`docs/design/WORKER-AUTHORING-STORY.md` (Tom's 2026-07-17 rulings: three worker
layers, namespaced word-sets, batteries included, problem-driven vocabulary
growth) and `docs/design/aion-authoring/awl/AWL-BIG-PICTURE.md`. Task context:
#30 (AWL-declared workers / scaffold-from-studio), #136 (no AWL templates),
#146 (tutorial, remaining scope), #154 (availability fix, landed 401a3b6dd),
aion #158 (pre-flight blind spots, Waffles' one-mechanism ruling).

## 1. The gap, stated once

The tutorial teaches `run` bodies superbly and ends worker authoring in four
bullets, one command name, and a directory path (slides 29–30). The reader who
follows that pointer hits a wall in the first minute, and each wall is a
verified defect below. The deeper shape: **the path from "I have a document"
to "I have a worker serving it" exists in the CLI, is misdescribed by the
tutorial, is duplicated-and-divergent in the console, and is unconfigured in
the minted server.** Nothing new needs inventing; the existing pieces need to
agree, connect, and be true.

## 2. Verified defects (each measured 2026-08-08, at this seat's hand)

**D1 — The tutorial's ending recommends a command that refuses the reader's
own document.** Slide 29 says `aion awl scaffold` "writes that worker's crate
for you from this same document" (`docs/authoring/build-tutorial.py:1387`).
Run against `docs/authoring/tutorial-examples/repo_gate.awl` it exits 1:
*"every action it declares carries a `run` body … There is nothing for a
worker to serve."* The refusal is correct (`codegen/awl_worker/plan.rs:96-100`
— a scaffold for an all-bodied document would emit a worker admission
refuses); the tutorial's claim is false for the only document the reader has.

**D2 — The console's scaffold is a second, weaker generator that emits workers
the server refuses.** `crates/aion-server/src/awl/scaffold.rs` is separate
code from `crates/aion-package/src/codegen/awl_worker/`. Measured differences:
it never calls `ActionContract::worker_owed()` (scaffolds bodied actions a
worker cannot be admitted for), registers no schema descriptors (any queue
with a deployed contract refuses the connection with
`WORKER_CONTRACT_MISMATCH` — the generated shell README even says so while
the Rust path does it), emits `todo!()` handler stubs (`scaffold.rs:312`) that
the house lints deny, pins `aion-worker = "0.8.0"` (`:137`, three majors
stale), and hardcodes `http://127.0.0.1:50051` (`:158`). This violates the
rule of record at `contract.rs:249-265` ("every surface that splits a queue's
actions into worker-owed and server-run … must call this") and is exactly the
hazard Waffles named on aion #158: a demand rule known in two places has
already drifted. **Filed as its own task.**

**D3 — A freshly minted server cannot accept any worker, silently.** Neither
`crates/aion-cli/templates/shared/aion.toml` (what `aion new` writes) nor
`docs/authoring/tutorial-examples/aion.toml` carries `[outbox]` or
`liminal_listen_address` (0 grep hits in both). Per
`docs/workers/WORKER-AUTHORING.md:27-60`, a worker's dial against such a
server is "simply refused, forever, with nothing on the server side saying
why." The tutorial never needed it (`run` bodies only) — which means the
reader who follows slide 29 hits this wall immediately after D1. **Filed as
its own task.**

**D4 — `aion new` teaches the wrong language** (#136, already on the board):
six templates, all Gleam, zero `.awl`; the `agent` template warns at mint that
`aion start` will refuse its output and names a successor lane
("template-AWL-migration") that exists nowhere in the tree
(`crates/aion-cli/src/new/scaffold.rs:69-84`).

**D5 — Ship & Run receives the served/server-run split and shows none of it.**
The availability response now carries `worker_actions` and
`server_run_actions` (#154); the console facade maps only `scaffoldHint`
(`guided-facade.ts`), and the panel's refusal is a thrown string naming a UI
section ("Workers & Actions") that lives in a different view mode with no
link. The scaffold affordance exists (`WorkerScaffold.tsx`, studio mode) but
is unreachable from the place that tells you to use it — and it is driven by
the D2 generator.

## 3. Direction

Five moves, ordered by leverage. P1 is subtraction; P2 makes the tutorial's
promise true by changing the document rather than softening the words.

**P1 — One generator.** Delete the console scaffold implementation
(`awl/scaffold.rs`) and have `POST /awl/scaffold` call the codegen crate the
CLI uses: compiled `PackageContract` in, `worker_owed()` split,
per-`(action, node)` connection plan, schema descriptors advertised, terminal
(not `todo!()`) stubs, SDK version from the binary. The console keeps its
copy/download presentation; it gains correctness it cannot drift from. This
is the same one-mechanism law Waffles ruled for #158, applied to the
generator: demand knowledge flows through `worker_owed()`'s single home.
The Python runtime refusal stays (no Python scaffold exists anywhere; a
console option must not pretend otherwise).

**P2 — The tutorial ending becomes a worker chapter, and the claim becomes
true.** Extend `repo_gate.awl` (or a staged successor document — decision 1)
with one genuinely worker-owed action that a command cannot do: the natural
candidate is an `agent`-marked action (`judge(prompt: String) -> String`)
that has an agent read the gate's failures and write the verdict prose —
which also teaches the `agent` marker, currently taught nowhere in the
tutorial. The ending then *walks* the real path on the reader's own document:
`aion awl scaffold` (now succeeds — or `aion worker agent` for the
agent-marked shape, no crate at all), start the worker, watch admission
accept it, run the gate. Slide 29's four bullets become a step the reader
performs; the two bullets about admission refusal become something they see
survive. Ship & Run's availability answer then shows a mixed queue honestly
(P3). This also resolves the tutorial's unreconciled double meaning of
`worker` ("in practice, a queue" on slide 14 vs "a worker process" on slide
29) by demonstrating both in one document.

**P3 — Ship & Run displays the split and links the remedy.** Surface
`worker_actions` / `server_run_actions` in the panel (the data already
arrives); when the queue is unserved, the hint becomes an affordance — a
control that takes the operator to Workers & Actions (or scaffolds in place)
instead of a string naming a section they must find. Respects the
no-bundle-rebuild constraint until the next cut; TSX + facade work only.

**P4 — AWL-first templates** (#136): `aion new` gains AWL templates — the
tutorial's `repo_gate` (all-bodied, first contact) and the P2 worker-owed
variant (the second document anyone needs). The self-declared-broken Gleam
`agent` template is retired, not shimmed (no-backwards-compat law); the
remaining Gleam templates stay for the SDK story.

**P5 — A minted server can accept a worker.** `aion new`'s `aion.toml` and
the tutorial's config carry `[outbox]` with `liminal_listen_address` present
— written explicitly in the operator's file, with the one-line comment saying
what it is for. This is not an assumed default smuggled into the loader (the
no-assumed-defaults law holds: the *loader* still requires the value; the
*template* writes it where the operator sees and owns it). Exact value
handling is decision 2.

**Non-goals of this lane.** No worker-AWL vocabulary design (WORKER-
AUTHORING-STORY.md §4 owns it; growth stays problem-driven per Tom's ruling).
No Python/TS scaffold (the SDKs exist; generating against them is its own
lane). No lifecycle-through-server work (#27 territory, the declared
linchpin, separately sequenced).

## 4. Relation to #30 and aion #158

P1+P3 are the first concrete slice of #30's "scaffold-from-studio" headline —
delivered by unification rather than new machinery. The full #30 (studio
offers to scaffold a *missing* worker while authoring, deploy lands under
server-owned lifecycle) stays open behind #27. Nothing here touches #158's
build; both obey the same ruling, and the #158 design brief should name the
generator as a third site the one-mechanism boundary covers.

## 5. Decisions routed (Waffles)

1. **P2 document shape:** extend `repo_gate.awl` in place (one document, the
   ending stays continuous) vs a staged second document (`repo_gate` stays
   pure first-contact; the worker chapter opens `repo_gate_judged.awl`).
   Seat's recommendation: **staged second document** — the tutorial's
   self-gating build slices staged files already, and first-contact purity is
   worth protecting.
2. **P5 value handling:** template writes a concrete
   `liminal_listen_address` (loopback + fixed port, visible and editable) vs
   writes the section commented-out with instructions. Seat's recommendation:
   **concrete value** — a commented-out line is D3 with extra steps.
3. **Sequencing:** P1 (server+console) and P2/P4/P5 (docs+templates+CLI) are
   independent lanes; P3 rides whichever console cut follows #139. Seat's
   recommendation: P1 first (it is subtraction and closes a live
   emits-refused-workers defect), then P2+P5 together (the tutorial chapter
   needs the config), P4 behind them (templates ship what the tutorial
   proved).
