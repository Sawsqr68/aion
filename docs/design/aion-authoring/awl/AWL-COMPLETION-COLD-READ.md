# AWL Completion — the Cold-Read Study and the Language Gaps

**Status:** Design — Bundle A of Tom's Jul 25 split (language: Cally Ray; runtime: Vesper Lynd, `aion-workers` lifecycle doc, forthcoming)
**Author:** Cally Ray
**Method evidence:** two independent blind panels, Jul 25 2026 — panel A (this doc): Sol medium, Sol xhigh, fresh Claude, zero workspace context, shown `wait_signal.awl` + `cargo_gates.awl` + `remediation_packet.awl` cold, no documentation; panel B (Vesper's): Claude Opus + two Sols, same play, run independently with zero shared context. Structured envelopes retained under the session scratchpad (`blind-readers/`) and `~/.norn/delegations/`.
**The bar (Tom, verbatim intent):** so simple it needs no explanation; no uncanny-valley feeling of almost-something-but-not-quite.

## 1. Headline result

The language largely passes the bar at its foundation. All three panel-A readers reconstructed the execution model cold from three samples — durable runs, signal waits, fork/join concurrency, typed terminal outcomes, guarded transitions, per-action node/timeout/retry policy, resumed agent sessions — at roughly 90% fidelity, self-flagging their guesses. The misses are not diffuse vagueness; they are a short, specific, fixable list, and the two panels found the same items independently. Everything below is that list, ordered by evidence strength.

## 2. Grammar completion (unanimous or cross-panel findings)

### 2.1 Split transition from termination — THE change (3/3 readers, independently)

`route` currently means three things: outcome-channel binding (`outcome done: type Done, route success`), step transition (`route select`), and terminal construction (`route clean(gates_run: 4)`). `outcome` means two: header terminal declaration and in-step guarded branch. Every reader's worst misreads traced here; every reader independently proposed the same fix shape. Ruling:

- **`goto <step>`** for step transitions. (Chosen over `next`: unambiguous under reordering, and honest — this is a state machine.)
- **`finish <outcome>(fields…)`** for terminal construction. Header keeps `outcome <name>: type T, route success|failure` as the only place `route` survives.
- In-step guarded branching becomes a single ordered block, first-match-wins **declared in the grammar, not assumed**:

```awl
  decide
    when land.halt: goto wrap_halt
    when land.escalations is not empty: goto hold_ruling
    when not land.packet_complete: goto select
    otherwise: goto close
```

First-match ordering was flagged as assumed-not-stated by both panels; `decide` makes priority a stated property of the block.

### 2.2 The invisible-dataflow rule (4/4 readers across both panels — must-fix)

`join -> lane_results`, `wait ruling -> rulings` bind values that are then never passed; the true dependency rides "recorded in this run's history" prompt prose. Panel B's Opus reader called it "the one genuinely surprising convention." Rule: **typed bindings are the carrier; history is the audit trail — never the reverse.** Agent actions that need prior results receive them as declared, typed arguments (the transforms layer §4 makes the reshaping cheap). A binding that is never consumed becomes a checker warning; the flagship gets rewritten to pass `lane_results` and `rulings` explicitly.

### 2.3 Small unanimous repairs

- **`is not empty`** joins the grammar; `not x is empty` scoping ambiguity dies (2 panels).
- **`max N visits`** moves to the head of the step it bounds and declares its breach edge: `max config.max_waves visits else finish halted(reason: "wave budget exhausted", …)`. A 30-day orchestrator's stated safety bound may not have invisible semantics (3/3 severe).
- **One retry dialect.** `retry 5 backoff 30s..10m` and `retry 3 every 30s` unify: `retry N every D` is defined as `backoff D..D`. One concept, one spelling with a degenerate case (3/3).
- **Fork binding unified.** Named forks bind per-branch, collection forks bind at join — two shapes for one construct (2 panels). Collection forks gain nothing from per-branch names; the fix is documentation-of-record plus checker text that names the distinction when misused.
- **Explicit failure edges.** No sample shows what happens when an action exhausts retries, a fork sibling dies, or a timeout fires; readers inferred a hidden error channel (3/3). Every step gains an optional `on failure` edge (already a stopgap-dialect construct; lands natively with the parity arc), and the checker requires one wherever a declared retry can exhaust.
- **`""` vs absent for optionals** (2 panels): the checker rejects passing `""` to `String?` parameters; absence is spelled by omission.

## 3. Missing capabilities

### 3.1 Signals: the surface is half-built

Inbound exists and reads perfectly cold (`signal ruling: RulingBatch` + `wait ruling -> rulings` — no reader stumbled). Missing:

- **In-language send.** Today a workflow signals another run only by shelling out (`aion signal <run-id> …` via a shell worker — the flagship teaches operators the CLI in a heartbeat message). Signal send becomes a statement: `signal <run-ref>.<signal-name>(payload…)`, typed against the target workflow's declared signal, checkable when the target type is known (children; same-package workflows) and contract-checked at the boundary otherwise (handshake machinery).
- **Declared buffering semantics.** All readers guessed at signal-before-wait behavior. The semantics (buffered per declared signal, delivered on next `wait`, duplicates queue) get one paragraph in the language reference and a conformance fixture — whatever the engine already does becomes *stated*, and the docs say it.

### 3.2 Shared types: kill the hand-copy

`CommandResult` in `remediation_packet.awl` is hand-copied and comment-labelled "the exact `run_command` output contract of the general worker" — a duplicated contract maintained by discipline. Types become importable across workflow files within a package (`use types from "general_worker.awl"` or a package-level `types.awl`), and worker contracts (handshake doc §2.7) become the single source for action shapes — a workflow *imports* a worker's contract types rather than restating them. This is the language-side face of the handshake.

### 3.3 The transforms layer

Design already authored: `DATA-TRANSFORMS.md` (transforms / query / templating, pure and total, schema-in→schema-out). The study hardened it from independent reinvention: Sol-xhigh, unprimed, produced `transform name(args) -> Type` declarations with the same purity rules keyword-for-keyword; Sol-medium produced a pure `let` expression layer with typed transform declarations; Claude produced `let`/`where`/`each` + typed-hole templates + `shape A from B` mapping blocks, and volunteered the purity doctrine verbatim. Two consequences promoted into that doc:

- **Templates replace `+` prompt-gluing** as the *only* sanctioned way to build model-facing text (3/3 named the 1,400-character `+` chains the worst lines in the flagship).
- The expression layer retires the F16 workaround family: `Failing` can carry `gates where not it.passed` instead of everything-plus-flags. All three readers cited the F16 comment itself as the language admitting this gap.

Surface choices (hole syntax, stage keywords vs pipes) wait on the competition-landscape leg (§6).

## 4. Honest limits — what types cannot promise

Sol-xhigh's sharpest structural observation: the flagship is a **typed shell around an advisory core** — merge rules, completion criteria, and holding discipline live in prompt prose; the types guarantee shape, not behavior. This is not a defect to fix in grammar, it is a limit to state in documentation and to shrink deliberately: every rule that CAN move from prose into a construct (the escalation set → workflow state, owned by Bundle B; completion criteria → `decide` guards; heartbeat posture → declared edge) should move. What remains advisory must be visibly advisory.

## 5. Bugs filed from the study

1. **Flagship doc/code contradiction:** the operator note claims the run finishes only when a select turn returns zero entries; `build_wave` routes directly to `close` on `land.packet_complete`. One of them is wrong; the code path is the more defensible design (fix the comment, or remove the shortcut — decision with the flagship's owner at next touch).
2. **`cargo_gates` invariant gap:** the header promises "real exit codes, never pass-claims" but the green guard trusts worker-supplied `passed`; `GateResult` permits `exit_code: 1, passed: true`. With the transforms layer the verdict computes in-language from `exit_code == 0`; until then the comment overstates.

## 6. Open — pending the landscape leg

External survey (jq failure modes, JSONata/JMESPath and the Step Functions migration, CUE/Jsonnet, XSLT's cautionary arc, Temporal payload converters, typed-reshaping precedents) resumes after a provider drop; its output selects the transforms surface details in `DATA-TRANSFORMS.md` §8. Nothing in §§2–5 depends on it.

## 7. Handoffs to Bundle B (delivered to Vesper, Jul 25)

Retry × non-idempotent effects and retry × `session_key` turn semantics; heartbeat/side-channel actions on the critical path (both panels, independently); workflow-owned escalation state; missing-worker posture (agreed split: refusal at run start, park-visibly for mid-run vanishing); worker blocks read as contracts with an invisible deployment boundary — cold-read confirmation of the server-deploys-workers direction.

## 8. Non-goals

- No renaming spree: `fork`/`join`/`wait`/`step`/`worker`/`child` all read correctly cold and stay.
- No pattern-matching language, no lambdas, no method chains — the readers' own sketches stayed keyword-led and so does the design.
- No behavior change smuggled in as grammar: every §2 item is surface or *stating* an existing semantic, except where an explicit ruling is marked.
