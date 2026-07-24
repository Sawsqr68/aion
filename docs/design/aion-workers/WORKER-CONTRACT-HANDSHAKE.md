# Worker Contract Handshake

**Status:** Design — Phase 1 foundations centerpiece (post-FFI-audit arc)
**Authors:** Cally Ray, with Vesper Lynd (joint audit findings, Jul 24 2026)
**Doctrine:** A workflow must never fail mid-run for a structural or semantic reason. Every such failure becomes a refusal at the last safe moment before it.

## 1. Problem — evidenced, not hypothetical

The workflow↔worker boundary is unchecked at every layer until runtime:

- **Compile time:** `check_workers_and_children` (aion-awl `checker/decls.rs:220-261`) validates only internal name-collision and type-resolution. Worker action signatures are declared on faith; nothing verifies them against any real worker.
- **Registration:** workers advertise a bare name set (`aion-worker/src/activity.rs:119-171`). No schemas cross the wire.
- **First real check:** runtime input serde, surfacing as a terminal activity failure (`activity.rs:264-279`).
- **Output mismatch:** caught even later, by workflow-side codec decode — the worst failure point.

Consequences on the record (postmortem sweep, Jul 24 2026, all 224 stored workflows):

1. **Five child-decode deaths** from a durable-payload/codec incompatibility — stored array vs expected object (`dev_brief/codecs.gleam:770-812`). A version-pinned contract would have refused at registration.
2. **One mid-run `badarg`** — a malformed value reached `gleam_json_ffi:string` 52 minutes into a run. Output-side contract rigor would have refused at the boundary it crossed.
3. **The flagship deployed against a queue nothing served.** `remediation_packet` declares `worker general`; deployed workers advertise `remediation` and `dev_brief` only. Historical runs succeeded because an operator hand-launched a worker from example source — the system never knew the dependency existed. Habit papered over the gap; nothing recorded that the hand-launched worker was load-bearing.

The historical mid-flight deaths attributed to "Erlang shims" trace here instead. This is the foundation the estate's no-shims doctrine demands.

## 2. Design

### 2.1 Contract capture at deploy

Deploying a workflow writes its **declared worker contract** into durable cluster state (haematite as authority), keyed **per content-hash** of the compiled workflow package:

- For every declared worker: queue name, and for every action: name, input schema, output schema, retry envelope.
- For every declared child workflow and signal: the same typed shape.
- Multiple content-hashes of the same workflow coexist (observed reality: three loaded `remediation_packet` hashes). **A run is pinned for its lifetime to the contract of the content-hash it started under.** Fleet upgrades never mutate a live run's expectations.

### 2.2 Registration-time validation

Worker registration carries typed activity schemas, not just names. The server validates the worker's advertised schemas against **every deployed contract for its queue**:

- Match → registered, and the compatibility result is recorded per (worker-build, contract-hash).
- Mismatch → **refusal with a field-level diff**: which action, which field, expected vs advertised type, which contract-hash demands it. A precise diff is what distinguishes a refusal the doctrine tolerates from a new species of mystery.

### 2.3 Run-start hard gate

A run cannot start unless every queue its contract references has at least one live, contract-compatible worker registered. This is the conversion point: mid-flight structural death becomes an up-front refusal. The gate is absolute — no override flag. (An operator who wants to start a doomed run has no legitimate use case; the historical hand-launched-worker pattern is exactly what this forbids.)

### 2.4 Deploy posture: warn/park, with strict mode

Deploy itself **warns** (or parks the workflow as `awaiting-workers`) rather than hard-failing on liveness, because deploy-then-start-worker ordering is legitimate. `--require-live-workers` makes deploy strict for postures that want it. Compatibility (schema) violations at deploy against *already-registered* workers are always hard errors — there is no legitimate ordering that fixes a type mismatch.

### 2.5 Outputs carry the same rigor as inputs

The contract covers **outputs identically to inputs**. Workflow-side decode of a worker result must never be the discovery point of a shape mismatch; if a worker's advertised output schema matches the contract, the workflow-side codec is guaranteed decodable by construction. The lane_run badarg and the dev_brief array/object deaths were both output-side.

### 2.6 Compatibility rules — written, not vibes

Fleet upgrades against pinned old-version runs follow explicit rules:

- **Compatible (same contract version):** adding an *optional* output field; widening an input the worker accepts.
- **New contract version required:** removing/renaming any field, narrowing any type, changing any retry envelope semantics, changing an action's queue.
- **Refused:** anything else. Undeclared drift is the invisible-habit dependency this design exists to kill.

### 2.7 Schema source: schemars now, AWL workers as end state

Rust workers derive JSON schemas mechanically (`schemars` or equivalent) from the same serde types they already use — generated glue, nothing hand-maintained. The end state is AWL-declared workers: workflow and worker in one language, one type system, the handshake checkable at compile time on both ends, with the same contract machinery enforcing it for foreign-language workers. The wire contract format is identical in both worlds; only the producer changes.

### 2.8 Runtime constraint

beamr has no port subsystem (`open_port` is unconditionally `badarg`; no `os:find_executable`). The shell-exec worker shape — and every external-process worker — is **host-native under worker supervision**, never an in-VM port shim. This is law, not accident; a `cli_ffi`-style shim runs only on real BEAM and would silently fork the runtime story.

## 3. Relation to the authoring harness

The handshake is the deploy/run-time half of one system. The authoring harness (fixture ladder: static checks → simulated runs on the **real** engine with fixture workers → mixed → live) is the same contract exercised earlier: Tier 0 validates fixtures against these schemas at edit time; Tier 1's synthetic fixture-worker registers through this same handshake path, so the harness cannot drift from production semantics. The build_stacktrace divergence (real beamr killing what the OTP-based test double passed) is the standing proof that any tier below the real engine inherits false comfort — which is why Tier 1 runs real beamr and why registration-time checks must live in the server, not the tooling.

## 4. What this does not do

- No schema inference from observed traffic — contracts are declared, never guessed.
- No auto-migration of stored payloads — the dev_brief incompatibility class becomes a *refusal* with a named migration task, not a silent fix.
- No relaxation for "trusted" workers — the diff is cheap; trust is what the contract provides, not what exempts from it.

## 5. Open implementation notes

- Contract storage shape in haematite and its read path at registration/run-start (owner: engine side).
- Field-level diff rendering — one format shared by CLI refusal output and ops-console surfacing.
- Registration protocol extension carrying schemas (additive to current name-set registration; old workers refuse cleanly with "contract required").
- The `general-worker` example graduates from `examples/` to a deployable with a declared contract — the flagship's dependency stops being folklore.

## 6. Evidence base

Audit and sweep records: `docs/briefs/beamr-build-stacktrace-heap-divergence.md`; postmortem sweep envelope (zero GC-signature matches, two boundary root causes) and BC-2 gap inventory, Jul 24 2026, session records with Cally Ray. Boundary trace citations verified at bytes by both authors.
