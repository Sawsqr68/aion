# Ruling: the agent seam belongs in the language (task #98)

Investigated 2026-07-30 at the bytes of a live run. This is the ruling and the
reasoning; the build is a separate lane because it touches the AWL emitter.

## The symptom

An AWL-authored agent round delivers a JSON **object** where the harness
expects a **string**. Recorded from run `1b3afbde-…`, event seq 10, the
dispatch input of the second agent round:

```json
{"prompt": "Without me repeating it: what file did you just name, …"}
```

The agent received that literally, understood it anyway, and answered
correctly. **It works despite being wrong**, which is the worst failure mode
available: nothing is red, nothing is logged, and the defect only surfaces on
a harness less forgiving than a language model.

## Why it happens — both halves are behaving correctly

**AWL side.** An action call lowers its named arguments into a record, always.
`assistant(prompt: session.opening_prompt)` therefore encodes
`{"prompt": "…"}`. There is no arity-1 special case, and there should not be
one for ordinary actions: a single-parameter action served by a Rust, Python,
or TypeScript worker has a struct-shaped handler and expects the object.

**Harness side.** `prompt_from_spec` in
`crates/aion-integration-norn/src/harness.rs:242-255` unwraps a bare JSON
string and passes anything else through as raw JSON text. Its doc comment is
explicit that this is deliberate: "a structured payload is a legitimate prompt
for some harnesses, and inventing a projection here would lose information."
That is a defensible contract.

Neither side is wrong on its own terms. **The two have never been told they
are the same seam.**

## The root cause, which is bigger than the symptom

AWL has no concept of an agent action. Nothing in a `.awl` document
distinguishes `assistant(prompt: String) -> String` — one prompt in, one
terminal text out, executed by a coding agent — from `assistant_provision(...)
-> Workspace`, which clones a git repository.

That distinction currently lives as a **hardcoded array in one example
binary**:

```rust
// examples/agent-dev/worker/src/main.rs:50
const AGENT_ACTIVITY_TYPES: [&str; 4] = ["scout", "dev", "review", "assistant"];
```

The worker's own code already documents how sharp this is
(`examples/agent-dev/worker/src/descriptors.rs:92-101`): handing the whole
descriptor set to `AgentHarnessConfig::with_activity_descriptors` would route
`assistant_provision` — a git clone — to norn as a prompt. The split had to be
hand-maintained against a document that cannot express it.

So the true statement of #98 is: **the language cannot say what kind of action
an action is, and every consumer therefore guesses — the emitter guesses
"record", the harness guesses "raw JSON", and the worker keeps a private list.**

## Options considered

**A — unwrap a single-field object in the norn adapter.** Rejected. It is
magic at the wrong layer, it contradicts the adapter's documented contract for
structured prompts, and it changes behaviour for every harness to fix one
language's emission. A two-field agent action would still be broken, silently.

**B — arity-1 unwrap in AWL for all actions.** Rejected outright. Every
existing single-parameter action across the Rust, Python, and TypeScript
worker SDKs expects the object form. This would break all of them at once, and
break them at the wire, where it is hardest to see.

**C — declare the agent seam in AWL.** RULED. An action declaration carries an
`agent` marker, exactly as it already carries `advisory`
(`crates/aion-awl/src/ast/document.rs:381-387` — declaration-only, its own
span, refused at the call site). Concretely:

```awl
worker default
  action assistant(prompt: String) -> String
    agent
```

Three things follow from one declaration, and this is why C is right rather
than merely adequate:

1. **The emitter puts the bare value on the wire** for an `agent` action, so
   the harness's existing string-unwrap path is hit and the agent receives the
   text. No adapter change at all.
2. **The worker stops hand-maintaining a list.** `AGENT_ACTIVITY_TYPES` is
   deleted; `advertised_surface()` already parses the document, so `split()`
   partitions on the declared marker instead of a const. The hazard the
   descriptors module warns about becomes structurally impossible.
3. **Admission can check it.** The marker reaches the package contract, so a
   worker that routes an action through an agent harness while the document
   declares it plain (or the reverse) is a contract mismatch caught at
   registration rather than at the first dispatch.

## Constraints on the build

- **An `agent` action must be arity-1 with a `String` parameter and a `String`
  return.** That IS the harness contract — one prompt in, one terminal text
  out. The checker must refuse anything else at check time, with a message
  that says so plainly. Anything looser recreates the ambiguity this ruling
  exists to remove.
- **No backwards compatibility.** Per house rules, `AGENT_ACTIVITY_TYPES` is
  deleted, not deprecated alongside.
- **`examples/assistant/awl/assistant.awl` and `assistant_spike.awl` both gain
  the marker**, and they must keep agreeing — they share queue `default`, and
  a divergence between them took the queue down once already.
- **Acceptance is a live run whose recorded dispatch input is a bare JSON
  string**, not an object. Read it from the event bytes with
  `payload_limit_bytes: 0`; do not accept a green test as the proof.

## Honest note on what this invalidates

Every claim made about the assistant seam before this lands — including the
live proof in `ASSISTANT-AWL-LIVE-PROOF.md` — is a claim about the *wrapped*
form. The workflow logic, the loop, the signal path, and the outcome are
unaffected and remain proven. The prompt delivery is not, and is stated as
such there.

## AMENDMENT — task #181 (2026-08-10): the return constraint is superseded

The first build constraint above — *"an `agent` action must be arity-1 with a
`String` parameter and a `String` return"* — is **superseded in its return
half** by the ruling on task #181 (docs repo commit `c223c8c`,
`tracking/waffles-thread-state-20260805.md`). The text above is retained
unedited as the record of what was believed on 2026-07-30.

What #181 found: the `-> String` return rule had universalized ONE harness
path's shape. The ACP adapter honestly emitted `{stopReason, text}`
(camelCase, undeclarable in snake_case AWL), the norn adapter passed the
envelope's `output` through verbatim (String OR Dict — live repro `cc1d82c2`,
"Expected String, found Dict", a serde decode error where a contract refusal
belonged), and the CLI adapter emitted a third shape. "Parse it downstream"
flattened the stop reason — an operational distinction (agent-chose-to-stop
vs cut-off) retry logic needs.

What #181 ruled and what now stands:

- The canonical **agent-outcome record**, defined by aion in `aion-core`
  (`AgentOutcome { text: String, stop_reason: String }`, serde snake_case),
  is the wire truth every harness emits from `wait_result`. Every harness
  maps its native terminal outcome at its own integration boundary;
  `stop_reason` is the snake_case rendering of the existing neutral
  `StopKind` set (`end_turn | tool_use | limit_reached | cancelled | error |
  other`), spelled by the ONE mapping `StopKind::canonical_stop_reason` — no
  second stop-reason table exists.
- The checker now demands an `agent` action's declared return type be a
  record of exactly `text: String` and `stop_reason: String` (author-declared
  name, structural check — the `RunOutcome`/DECLARED-COMMANDS precedent). A
  bare `-> String` is the refused shape; the refusal names the record and
  both fields.
- A norn envelope whose completed `output` is not a String is a typed
  contract refusal at the harness boundary, naming the canonical contract and
  what was found — never a downstream serde decode error.
- The arity-1 `String`-parameter half of the constraint, the no-`run`-body
  rule, the call-site refusal, and the marked/unmarked identical-lowering
  control all stand unchanged.

Migrated in the same change (#181, ruling item 5): the assistant embed
(`crates/aion-server/assistant-embed/assistant.awl`) and its byte-coupled
spike (`examples/assistant/awl/assistant_spike.awl`), the Gleam originals
(`examples/assistant`, `examples/agent-dev` — decoders now read the record's
`text`), all four harness boundaries (ACP, norn, CLI, mock), the checker
fixtures and MIR goldens, and the guide/reference text — "parse it
downstream" is dead everywhere it was written.

## AMENDMENT 2 — task #181 R2/R3 (2026-08-10): the full migration surface and its gates

The first amendment's migration list was written before the R2 review swept
the whole tree and understated the surface. The complete record:

**Five further in-tree documents carried the retired shape and were migrated
in the same change:** `workflows/norn-dev/norn_dev.awl`,
`workflows/norn-fleet/norn_fleet.awl`,
`docs/authoring/tutorial-examples/repo_gate_judged.awl`,
`docs/authoring/deck-examples/03-agent-seam.awl` — each now declares the
record and threads `.text` through its guards and routes — plus
`examples/general-worker/awl/general_worker_example.awl`, which had been
UNPARSEABLE since #98 made `agent` a reserved word (it bound `-> agent`;
nothing checked it, the same defect class). The deck and tutorial builders'
prose and both committed HTML artifacts (`awl-deck.html`,
`first-workflow.html`) were regenerated to teach the record, and both
production workflow READMEs (`workflows/norn-{dev,fleet}/README.md`) were
rewritten — the fleet README's "trade this shape records" section now cites
THIS ruling (#181) as the enforced shape rule, superseding its citation of
#98/`8451e0e5`.

**Two derived gates now hold the class closed** (a hand-enumerated corpus is
how four documents escaped in the first place):

- `crates/aion-cli/tests/awl_corpus_gate.rs` — every living `.awl` in the
  tree (corpus derived by walking the tree; excluded CLASSES with stated
  reasons: fixtures/goldens, templates, `docs/design`, `gate-logs`, build
  output) must pass the real `aion awl check`. Proven red on the unmigrated
  tree (it named all five documents), green after.
- `crates/aion-cli/tests/retired_doctrine_gate.rs` — no living markdown may
  spell the retired doctrine ("parse it downstream", "one String in, one
  String out" and its variants), matched over normalized text with the same
  derived-scope discipline. Proven red against the actual pre-fix README
  text, green after.

**Retry classification joined the ruling's mechanism** (R2 M4): the norn
Dict/null completed-output refusal is `HarnessError::Contract`, a variant
whose deterministic/transient classification lives in `aion-integrations`
(`HarnessError::is_deterministic`, exhaustive match — a new variant is a
compile error at the deciding site) and which the worker maps to a TERMINAL
activity failure, so a permanent misconfiguration can no longer burn a
`retry 5 backoff 30s..5m` budget of live agent runs. A completed
`"output": null` is in the deterministic class, settled against norn's
DRIVEN-PROTOCOL.md stop-reason table (null output belongs only to
non-completed reasons). All potentially-transient classes (transport,
protocol framing, harness-reported failure, staleness, capability gating)
remain retryable.

**The CLI harness stopped inventing its stop reason** (R2 M1/M2, R3 mi4):
the pump retains the last demuxed `Stop` and the record reports it
(`end_turn` only as the announced-nothing fallback on a clean end); a
signal-killed child completes with `stop_reason: "other"` — never
`end_turn` — and the pump synthesizes a terminal transcript `Stop { Other {
"terminated by signal" } }` so the detail survives on the channel built to
carry it.
