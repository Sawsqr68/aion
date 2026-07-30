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
