# Worker lifecycle through the server — decision brief, 2026-08-08

**What this is.** The #27 decision brief: the last paper before the worker-lifecycle
build is dispatched. It folds Tom's four direction threads (his DMs, 2026-08-08)
into the already-decided ground, states what has changed in the tree since the
papers of record were written, and puts the decisions that are still open crisply
for ruling. It goes to Waffles for review before any build moves. Written by
Vesper Lynd from a fresh code scout at this seat (every file:line below verified
2026-08-08 against main `0e780935f`).

**Papers of record it extends — it re-litigates none of their rulings:**

- `docs/design/RUNTIME-OPERATIONS.md` — R6 (server deploys workers: durable
  `WorkerDeployment` record, per-node in-process reconciler, local-restart-first,
  drain-then-replace upgrades, external workers first-class but
  `ownership=external`), R7 (leases cover the connection, not just in-flight
  tasks), R8 (containment created before user code runs, owned by the harness;
  `Killed` only after OS-level confirmation), R9 (cancel cascades; `worker stop` /
  `worker drain` become real admin ops), R10 (killed-but-resumable is a distinct
  state; replay is authoritative).
- `docs/design/WORKER-LIFECYCLE-BUILD.md` — build units W-0..W-6, explicitly
  *"not yet build-approved"*, ending with two questions held for the operator:
  the phase plan, and the artifact posture. This brief answers both with
  recommendations.
- `docs/design/WORKER-AUTHORING-STORY.md` — Tom's 2026-07-17 ruling that worker
  lifecycle through the server is the linchpin and lands first.
- `docs/design/aion-workers/WORKER-CONTRACT-HANDSHAKE.md` — owns registration and
  contract gating (RUNTIME-OPERATIONS §8 already deferred W-3's sketch to it).
- `docs/design/aion-authoring/WORKER-TELEMETRY-DIRECTION.md` — Tom's 2026-08-04
  ruling: live telemetry plane separate from durable history.
- `docs/design/aion-authoring/SHIP-AND-RUN-TO-WORKER-AUTHORING-2026-08-08.md` —
  the verified authoring defects; D3 (no `[outbox]` in the templates) is now fixed
  on main (`fb631c78c`), D4 (`aion new` taught the wrong language) is now fixed on
  main (`0e780935f`), D2 (the console's divergent scaffold generator) is open.
- `docs/design/WORKER-DEPLOYMENT.md` — draft prior art only (tiers, hard
  problems); where it disagrees with RUNTIME-OPERATIONS, the latter rules.

Constraints already ruled and binding: Aion-native supervision, **no launchd,
ever** (operator ruling 2026-07-14); single-node first; ADR-001/003/022.

---

## 1. What has changed in the tree since the papers (all verified this seat)

These are facts the build units were written without, and they mostly make the
build shorter:

1. **The server already runs OS child processes today.** Declared-body actions
   execute server-side through the worker SDK's own executor
   (`crates/aion-server/src/worker/declared_body.rs` →
   `aion_worker::shell::ShellAction` → `crates/aion-worker/src/process.rs`), with
   real process-group containment: `process_group(0)`, `killpg` SIGTERM → bounded
   grace → SIGKILL → signal-0 probe, `Cancelled` withheld until the group is
   confirmed empty. W-2's "spawner" is not greenfield — the containment core
   R8 demands already exists, is tested
   (`worker_shell.rs`: `cancellation_reaps_the_entire_spawned_process_group`),
   and is exercised in production by every declared body.
2. **But nothing triggers it.** Cancellation of a declared body is deliberately
   not wired (`docs/workers/DECLARED-COMMANDS.md`), and the authored `timeout`
   travels to the engine and is read by nothing. The machinery exists; the
   lifecycle build is what gives it a caller.
3. **`POST /workers/{id}/drain` and `/workers/{id}/stop` exist**
   (`crates/aion-server/src/api/http/router.rs:271-273`) — but `stop` is
   registry-only: a drain frame plus a timer that force-deregisters and fails
   in-flight tasks. It never touches an OS process, because the server holds no
   PID, no artifact, no desired state — `ConnectedWorkerRegistry` state dies with
   the server process. W-0 (durable deployment record) is still the true first
   unit. No console surface calls either route.
4. **Service honesty landed.** The per-(action,node) census (#154,
   `queue_service/census.rs`) and the admission audit are exactly the "Phase 1
   service honesty" RUNTIME-OPERATIONS sequenced first — that phase is
   substantially done on main.
5. **The harness adapters are R8's open orphan surface.** All three spawn agent
   children with `kill_on_drop(true)` and **no process group**
   (`aion-integration-acp/src/harness.rs:260`,
   `aion-integration-norn/src/harness.rs:236`,
   `aion-integration-cli/src/harness.rs:85`). The lifecycle build must route
   these through the same containment core, not grow a fourth copy.
6. **The de-facto supervisor today is `scripts/setup.sh`** — nohup, pgrep
   idempotence, log-scrape readiness, no stop verb. It is the measure of what the
   build replaces.

## 2. Tom's four threads, folded

### (a) "Will we use beamr's supervision trees to host the workers?"

**The honest architecture answer:** no — and yes, in the way that matters.
beamr supervises processes *inside the VM*: workflow executions, engine
services. Workers are OS processes outside the VM boundary, so beamr's trees
cannot hold them directly (the one-boundary-module rule also forbids threading
beamr into the server's process management). What R6 decided is the same
*semantics* at the OS level: a supervisor with desired state, bounded restart
intensity (OTP's escalation discipline), local-restart-first. The supervision
philosophy is beamr's; the mechanism is the server's per-node reconciler.

**The genuinely new option Tom's question opens** — and my recommendation on it:
the reconciler could itself be *driven by a durable workflow* (Aion supervising
its own fleet with its own primitives — the living-software direction). I
recommend **not** for the mechanism: the reconciler must function when the
engine is degraded, before any package is deployed, and during store recovery —
supervision that depends on the thing it supervises inverts R8's spirit. The
right split is: **mechanism in-process (R6, reaffirmed); policy durable** — a
fleet-policy workflow *above* the reconciler (restart budgets, escalation to
the operator, scheduled scale changes) is a natural later phase and dogfoods
the engine without putting it under itself. Phase-1 scope: reconciler only.

### (b) "Packaging and bundling the pre-built workers with the install."

Today the only shipped binary is `aion`, and the pre-built workers ship as
**verbs**, not artifacts: `aion worker shell`, `aion worker agent` (norn + acp
harnesses compiled in by default). Everything else is an `examples/*` crate the
operator builds by hand.

This resolves W-1's open artifact-posture question cleanly. Recommendation:

- **`kind = builtin` deployments need no artifact at all.** A `WorkerDeployment`
  record may name the server's own binary and a verb (`worker agent --harness
  norn ...`, `worker shell --manifest ...`). The spawner launches the aion
  binary it is already running. This makes R6's "batteries included: a stock
  server serves shell and agent defaults the moment it starts" real on day one
  with zero new distribution machinery — and it *is* the answer to "package the
  pre-built workers with the install": they are already in the install; the
  deployment record just names them.
- **`kind = artifact` adopts W-1's additive `.aion` entry** (executable or
  script + platform + declared activity types), content-hash identity exactly as
  module namespacing already works (load-bearing invariant 5). WORKER-DEPLOYMENT
  §3.4's rejection of bare-binary-via-URL stands.
- A separate artifact-object store (the other branch of W-1's question) is
  rejected: it duplicates deploy, hashing, and namespacing that `.aion` already
  owns.

### (c) "Making writing workers in AWL a better experience in the authoring surface."

This thread is #30's lane (AWL-declared workers) plus one defect and one seam,
and this brief only sequences it:

- The `.awl` worker block **is already the declaration** — the tutorial now
  performs it end to end, and `aion worker agent <doc.awl>` already serves from
  the document. #30 makes the worker file first-class (declare worker vs
  workflow; declarative action bodies; scaffold-from-studio).
- **D2 must die in the lifecycle build, not after:** the console's scaffold is a
  second divergent generator (`crates/aion-server/src/awl/scaffold.rs` — stale
  version pin, hardcoded endpoint, `todo!()` bodies). One generator serves both
  surfaces (the CLI's is the truth; the console derives from it). One truth in
  two places has drifted — this is that law with a file path.
- The console's ship-and-run panel today can only *ask* about availability
  (`POST /awl/workers/availability`). When W-4's lifecycle API exists, the same
  panel grows start/stop/drain — the refusal it shows today becomes an
  affordance ("start the worker this document needs"), which is the actual UX
  Tom is asking for. W-6 (console management surface) rides the telemetry plane
  per the 2026-08-04 ruling, never durable history.
- #158 (the pre-flight gate consults a narrower population than the act) sits in
  the same census machinery and stays its own lane at Waffles' seat.

### (d) "Was the ACP worker ACP version 2? Because that was what I asked for."

Verified truth, both places: the adapter pins `agent-client-protocol-schema =
"=1.5.0"` and speaks `ProtocolVersion::V1` (`aion-integration-acp/src/
protocol.rs:58`, gated at handshake). It is **v1, not v2**. The design was
checked against the draft v2 module when written: v2's headline change removes
the fs/terminal surfaces in favour of MCP — surfaces the adapter already
declines. So the upgrade is narrow and mechanical: bump the schema pin, move
`v1::*` → `v2::*` in `protocol.rs`/`translate.rs`, re-gate the handshake, and
re-verify the translate layer against a live v2 agent. **Size: S** (single
crate, exact-pin compile errors enumerate the work). Recommendation: its own
small lane, after the #139 cut clears (worker rebuilds are frozen estate-wide
until then anyway); it does not couple to the lifecycle build.

## 3. The decisions put for ruling

1. **Artifact posture (W-1's open question):** `builtin` verb-deployments +
   additive `.aion` worker-artifact kind; no separate artifact object.
   *Recommended as §2(b).*
2. **Phase plan (W-0..W-6, the other open question):** adopt, with the
   RUNTIME-OPERATIONS §8 amendments already ruled (registration gating defers to
   WORKER-CONTRACT-HANDSHAKE; versioning takes R6's semantics), plus three
   amendments from §1's tree facts: (i) W-2's spawner is the existing
   containment core given a caller — declared-body cancellation and the unread
   `timeout` get wired in the same unit, closing that named gap; (ii) the three
   harness adapters route through the same core (no fourth spawn copy);
   (iii) W-6 kills D2 by deriving the console scaffold from the CLI generator.
3. **Reconciler placement:** in-process per-node mechanism (R6 reaffirmed);
   durable *fleet-policy* workflow deferred to a later phase, named as the
   living-software follow-on. *Recommended as §2(a).*
4. **ACP v2:** separate size-S lane after the #139 cut. *Recommended as §2(d).*
5. **Sequencing:** W-0 (durable deployment record) first — everything else
   stands on it; the existing `/workers/{id}/stop` route is finished (touches
   the OS process through the containment core) inside W-4 rather than left
   registry-only. #30 proceeds independently; #158 stays queued at Waffles'
   seat.

Silence-default: none — this brief waits for Waffles' review before any build
unit is dispatched, per the standing rule that #27 is design-first.
