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

Runtime meaning (cross-review, Vesper): **`goto` is a durable checkpoint** — step results commit to the journal before control moves. The lifecycle doc owns the full statement; here it is the reason `goto` is honest rather than merely blunt, and why `decide`'s stated-first-match is replay-deterministic.

### 2.2 The invisible-dataflow rule (6/6 — every reader, both panels — must-fix)

`join -> lane_results`, `wait ruling -> rulings` bind values that are then never passed; the true dependency rides "recorded in this run's history" prompt prose. Panel B's Opus reader called it "the one genuinely surprising convention." Rule: **typed bindings are the carrier; history is the audit trail — never the reverse.** Agent actions that need prior results receive them as declared, typed arguments (the transforms layer §4 makes the reshaping cheap). A binding that is never consumed becomes a checker warning; the flagship gets rewritten to pass `lane_results` and `rulings` explicitly.

### 2.3 Small unanimous repairs

- **`is not empty`** joins the grammar; `not x is empty` scoping ambiguity dies (2 panels).
- **`max N visits`** moves to the head of the step it bounds and declares its breach edge: `max config.max_waves visits else finish halted(reason: "wave budget exhausted", …)`. A 30-day orchestrator's stated safety bound may not have invisible semantics (3/3 severe).
- **One retry dialect.** `retry 5 backoff 30s..10m` and `retry 3 every 30s` unify: `retry N every D` is defined as `backoff D..D`. One concept, one spelling with a degenerate case (3/3).
- **Fork binding unified.** Named forks bind per-branch, collection forks bind at join — two shapes for one construct (2 panels). Collection forks gain nothing from per-branch names; the fix is documentation-of-record plus checker text that names the distinction when misused.
- **Explicit failure edges.** No sample shows what happens when an action exhausts retries, a fork sibling dies, or a timeout fires; readers inferred a hidden error channel (3/3). Every step gains an optional `on failure` edge (already a stopgap-dialect construct; lands natively with the parity arc), and the checker requires one wherever a **non-advisory** declared retry can exhaust. *Posture ruling landed (RUNTIME-OPERATIONS.md R5, built 2026-07-28):* the advisory/side-channel action class exists — `advisory` is a declaration-only config marker; exhaustion warns on the run (`ActivityAdvisoryExhausted`) and never faults the step — so the checker requirement scopes to non-advisory actions as this repair intended. **Margin note (landing ruling, 2026-07-28): the edge-rule half of this repair is TRUE AS WRITTEN BUT NOT YET MACHINE-ENFORCED.** The direct/MIR backend — the production compile path — refuses `on failure` outright (`mir/lower/flow.rs`), so a checker rule demanding it would refuse documents the compiler cannot lower (measured: 17 files, including the live remediation-packet workflows). Enforcement chain of record: the `on failure` lowering lane first (boarded, after the R4 retry-counting lane), then the checker edge rule as its dependent, widened to cover retrying actions in both call and pipe-stage positions. Until that chain lands, no checker refuses a missing edge — a reader must not assume enforcement. Evidence: `docs/evidence/advisory-class-red-green.md`.
- **`""` vs absent for optionals** (2 panels): the checker rejects passing `""` to `String?` parameters; absence is spelled by omission.
- **Retry counting stated in one word** (panel B, both Sols): neither reader could tell whether `retry 5` means five total attempts or five retries after the first. The reference defines it as "N further attempts after the first" and the grammar keeps the word `retry` honest to that.
- **Visit counting stated** (panel B): `max N visits` declares that the first entry counts as visit one and that action retries within a visit do not increment it — both were guessed at.
- **Empty collection fork = immediate join, stated** (panel B): `fork x in xs` over an empty collection joins immediately with an empty binding; currently assumed by every reader, promised by none.
- **Shell exit codes: data, not failure** (panel B + §5.2): when an action's result type carries `exit_code`, a nonzero exit is a *successful* action returning data — retry triggers only on *transient* transport/infrastructure failure (worker lost, timeout, connection). A malformed or schema-rejected payload is NOT retryable — retrying cannot change a shape mismatch; it fails the attempt loudly, engine-side (lifecycle doc R4; corrected here in cross-review — an earlier revision wrongly listed malformed payloads as a retry trigger). The current corpus chooses neither reading; this ruling makes `retry 3` on `run_command` mean what it should. Failure-classification detail (what counts as which) is a lifecycle-doc ruling; the language reference cites it.
- **`session_key` is a convention wearing a parameter's clothes** (panel B, xhigh): agent-session continuity is invisible in the grammar — it reads as an ordinary `String?`. If session affinity becomes engine-guaranteed (the lifecycle doc's session-as-first-class-object direction), it graduates to a keyword; until then the reference names the convention explicitly.

## 3. Missing capabilities

### 3.1 Signals: the surface is half-built

Inbound exists and reads perfectly cold (`signal ruling: RulingBatch` + `wait ruling -> rulings` — no reader stumbled). Missing:

- **In-language send.** Today a workflow signals another run only by shelling out (`aion signal <run-id> …` via a shell worker — the flagship teaches operators the CLI in a heartbeat message). Signal send becomes a statement: `signal <run-ref>.<signal-name>(payload…)`, typed against the target workflow's declared signal, checkable when the target type is known (children; same-package workflows) and contract-checked at the boundary otherwise. Mechanism (confirmed implementable, cross-review): deploy records the workflow's declared signal surface as a **first-class durable contract keyed by content hash** — exactly the handshake §2.7 shape, a lookup at send time, never a re-parse of stored source. **The failure posture is sender-side**: a mismatched send is a typed refusal at the sender; the receiver never sees garbage. The same contract record replicates through haematite in the cross-server design, so networked signal send is boundary-checked for free (lifecycle doc, Vesper).
- **Declared buffering semantics.** All readers guessed at signal-before-wait behavior. The buffering *paragraph* (what the language reference states, plus a conformance fixture) is this doc's; the buffering *behavior* — durability of queued signals across failover, ordering, duplicate handling under redelivery — is a Bundle B ruling, and this reference will cite the lifecycle doc rather than freeze current behavior before it has been checked to survive failover honestly.

### 3.2 Shared types: kill the hand-copy

`CommandResult` in `remediation_packet.awl` is hand-copied and comment-labelled "the exact `run_command` output contract of the general worker" — a duplicated contract maintained by discipline. Types become importable across workflow files within a package (`use types from "general_worker.awl"` or a package-level `types.awl`), and worker contracts (handshake doc §2.7) become the single source for action shapes — a workflow *imports* a worker's contract types rather than restating them. This is the language-side face of the handshake.

### 3.3 The transforms layer

Design already authored: `DATA-TRANSFORMS.md` (transforms / query / templating, pure and total, schema-in→schema-out). The study hardened it from independent reinvention: Sol-xhigh, unprimed, produced `transform name(args) -> Type` declarations with the same purity rules keyword-for-keyword; Sol-medium produced a pure `let` expression layer with typed transform declarations; Claude produced `let`/`where`/`each` + typed-hole templates + `shape A from B` mapping blocks, and volunteered the purity doctrine verbatim. Two consequences promoted into that doc:

- **Templates replace `+` prompt-gluing** as the *only* sanctioned way to build model-facing text (3/3 named the 1,400-character `+` chains the worst lines in the flagship).
- The expression layer retires the F16 workaround family: `Failing` can carry `gates where not it.passed` instead of everything-plus-flags. All three readers cited the F16 comment itself as the language admitting this gap.

Surface choices (hole syntax, stage keywords vs pipes) wait on the competition-landscape leg (§6).

## 4. Honest limits — what types cannot promise

Sol-xhigh's sharpest structural observation: the flagship is a **typed shell around an advisory core** — merge rules, completion criteria, and holding discipline live in prompt prose; the types guarantee shape, not behavior. Panel B's xhigh reader quantified the same boundary from the other side: control-flow graph readable 8/10 cold, operational semantics 5/10 — "the prompts look procedural but most of their contents are not engine-level steps." The 8 is this doc's to keep; shrinking the 5 is the lifecycle doc's job (making the runtime contract stated). This is not a defect to fix in grammar, it is a limit to state in documentation and to shrink deliberately: every rule that CAN move from prose into a construct (the escalation set → workflow state, owned by Bundle B; completion criteria → `decide` guards; heartbeat posture → declared edge) should move. What remains advisory must be visibly advisory.

## 5. Bugs filed from the study

1. **Flagship doc/code contradiction:** the operator note claims the run finishes only when a select turn returns zero entries; `build_wave` routes directly to `close` on `land.packet_complete`. One of them is wrong; the code path is the more defensible design (fix the comment, or remove the shortcut — decision with the flagship's owner at next touch).
2. **`cargo_gates` invariant gap:** the header promises "real exit codes, never pass-claims" but the green guard trusts worker-supplied `passed`; `GateResult` permits `exit_code: 1, passed: true`. With the transforms layer the verdict computes in-language from `exit_code == 0`; until then the comment overstates.

## 6. Open — pending the landscape leg

External survey complete (one provider drop, session resumed). Landed — lessons folded into DATA-TRANSFORMS.md §8 (absence taxonomy, explicit cardinality, XSLT-class prohibitions, typed escape hatch, converters-as-protocol, adopted precedents). Nothing in §§2–5 depended on it.

## 7. Handoffs to Bundle B (delivered to Vesper, Jul 25)

Retry × non-idempotent effects and retry × `session_key` turn semantics; heartbeat/side-channel actions on the critical path (both panels, independently); workflow-owned escalation state; missing-worker posture (agreed split: refusal at run start, park-visibly for mid-run vanishing); worker blocks read as contracts with an invisible deployment boundary — cold-read confirmation of the server-deploys-workers direction; **the signal-carriage seam**: receive syntax and semantics are this doc's, while carriage, buffering durability across failover, ordering, and duplicate handling under redelivery are the lifecycle doc's rulings (§3.1 cites it accordingly). Cross-review note for symmetry: §2.2's explicit `lane_results` passing moves payload weight from history onto typed arguments — arg-size limits/elision interplay is acknowledged in the lifecycle doc.

## 8. Non-goals

- No renaming spree: `fork`/`join`/`wait`/`step`/`worker`/`child` all read correctly cold and stay.
- No pattern-matching language, no lambdas, no method chains — the readers' own sketches stayed keyword-led and so does the design.
- No behavior change smuggled in as grammar: every §2 item is surface or *stating* an existing semantic, except where an explicit ruling is marked.
