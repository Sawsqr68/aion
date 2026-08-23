# Upgrade Experience — design note

Authored 2026-08-23 by Vesper Lynd (program owner), from Tom's
direction of the same day: "reliably stop the server… an update
command… update through the UI." Theme for the cut after the AWL
builder line. This note is the design ground for the implementation
briefs; it records what exists, what is missing, and the shape of the
cure. Code citations were taken against main at `30d1fcc29`
(post-v0.21.0).

## The finding that shapes everything

Both halves of this theme are half-built, and the halves that exist
are good:

- **Stopping**: `aion server` has a complete, carefully argued
  graceful shutdown — SIGTERM/SIGINT drain with a two-signal
  escalation (`crates/aion-server/src/run.rs:302-324`, `441-462`),
  drain-then-park semantics that converge on the kill-9 shape so
  replay recovers parked work (#207, `shutdown.rs:161-236`), managed
  workers stopped only after the drain with signal-zero-proven empty
  process groups (`shutdown.rs:251-270`), a death note
  (`death_note.rs`), and an engine teardown that accumulates errors
  instead of early-returning precisely so a cross-process writer lock
  can never outlive the process's interest
  (`crates/aion/src/engine/api.rs:610-757`). The haematite writer
  lock is a kernel-released flock — it cannot go stale
  (`lock_acquisition.rs:195-196`). Held-execution claims reconcile
  exact-incarnation stale state (#148, `spawn_guard.rs:101-155`).
- **Knowing about updates**: the update *check* is a shipped slice —
  `crates/aion-server/src/update_check/` (the embedded AWL check
  document fetching the crates.io sparse index through a declared
  command, keeping the binary itself free of outbound-HTTP deps),
  `GET /update-status` + `GET /build`, the console's update pill and
  "check now" panel, and `workflows/update_watch.awl` for
  operator-named cadence. Its design law, stated three times in the
  tree: **"knowing a newer aion exists and becoming it are different
  acts"** — manual-only, no install, no restart machinery
  (`update_check/mod.rs:21-26`).

So this theme is NOT "build shutdown" or "build an update checker."
It is: **give the existing stop a name and a report, and build the
second act the check deliberately left unbuilt** — with the ordering
law that currently lives in one orphaned markdown file encoded as
mechanism.

## The gaps, precisely

1. **The stop has no address.** No pid file; the launcher prints the
   pid into a log line and tells the operator to `kill {pid}`
   (`launch.rs:190,383`). No `aion server stop|status|restart` verbs
   (`ServerArgs` carries only config/address/threads). No server-level
   drain endpoint (only per-worker routes exist).
2. **The stop's outcome is thrown away at the process boundary.**
   `ShutdownOutcome` distinguishes Clean / Parked / TimedOut / Forced
   (`shutdown.rs:33-43`), but Clean and Parked both exit 0 — an
   operator cannot tell a clean drain from a 30s timeout that parked
   in-flight work without reading logs.
3. **The becoming act does not exist.** No `aion update` verb (zero
   hits in `aion-cli/src/main.rs`), no binary swap machinery, and
   `setup.sh` is install-only with no upgrade branch.
4. **The ordering law is prose, not mechanism.** `204-CUT-RUNBOOK.md`
   (repo root, referenced by exactly one other file) carries the law —
   *queue moves and document deploys BEFORE any worker rebuild or
   restart* — plus the proven hazard it sequences around: worker
   registration is admitted against reachable deployed contracts, and
   a worker restarted onto a new shape against an old document is
   refused with `WORKER_CONTRACT_MISMATCH`
   (`crates/aion/src/engine/admission.rs:1-34`, the 2026-07-30
   incident).
5. **The estate is version-skew blind.** No CLI↔server version
   negotiation; `RegisterWorker.identity` is logged and echoed but
   never compared; `RegisterAck` carries no server version. The only
   skew surface that exists is the managed-worker
   `deployed_binary`-vs-`spawn_binary` pair ("the two disagreeing is
   the upgrade, stated" — `supervisor/executable.rs:10-11`,
   `status.rs:111-118`), which is the right idea in one corner.

## Assets the design inherits (build on, never re-mint)

- **The deploy grant** as the authority seam for every mutating act
  (`deploy/guard.rs`; audit-line and metric conventions at
  `operations.md:619-631`) — already reused by managed-worker
  lifecycle and cluster topology; update joins the same seam.
- **Binary identity is content, never path or version string**:
  SHA-256 discipline everywhere (`build_identity.rs`,
  `DeployedBinaryIdentity.content_hash`, per-spawn capture).
- **The `/update-status` + `/build` pair** — the console already
  knows what is installed and what is newer.
- **The managed-worker supervisor** — durable desired state, argv as
  the whole launch contract, TERM→grace→KILL with EOF-proven stops,
  boot reconciliation. The restart half of update already exists for
  workers.
- **Swap-by-rename** (`docs/operations/server-restart.md:80-85`) and
  the detached start posture — the runbook steps that become code.

## Design

### Piece 1 — the stop verb: address + report

- **Pid file**: the server writes `<AION_HOME>/run/aion-server.pid`
  (pid + start-instant + binary content-hash, so a reused pid is
  detected by incarnation, not number — the held-execution claims
  already model this). Written on bind, removed on clean exit; a
  stale file is reconciled by incarnation check, never trusted.
- **`aion server stop`**: resolves the pid file, sends SIGTERM, waits
  bounded on exit, and **prints the drain outcome as a typed
  report** — which of Clean/Parked/TimedOut/Forced, how many
  dispatches parked, which workers were stopped, what was abandoned,
  by name. Wait patience is operator-ruled at invocation with no
  hardcoded default beyond the config's existing `drain_timeout`.
- **`aion server status`**: pid-file + `/build` + `/health/ready` in
  one read, stating pid, version, commit, uptime, store path — the
  "which layers am I actually running" question answered in one verb
  (the three-layers reconciliation, read side).
- **Exit-code honesty stays**: Parked remains success at the process
  boundary (the #207 argument is right — durable state means parked
  work is safe by construction), but the VERB reports the distinction
  the exit code cannot. The rich outcome crosses the boundary as the
  stop report, not as a return code.
- **Drain-outcome surface**: the server exposes its last
  `ShutdownOutcome` in the death-note/report path so `stop` can read
  it after exit (a file the dying process writes — the death note
  already owns this seam; extend it with the outcome record rather
  than minting a second file).

### Piece 2 — `aion update`: the becoming act

One verb that mechanizes the 204-runbook's seven manual acts, each
step typed, reported, and refusable — the runbook's own line, "the
runbook proves the migration; it does not assert it," becomes the
command's contract:

1. **Resolve**: target version from `--to X.Y.Z` or the recorded
   update-check result (never an implicit network fetch — the check
   is the knowing act; `update` consumes its record, or the operator
   names a version. `update` with neither refuses, pointing at
   "check now").
2. **Acquire**: `cargo install aion-cli --locked --version X --root
   <staging>` — the from-index acquisition the release ceremony's
   `verify_install` already proves per release. Identify the staged
   binary by SHA-256; record it.
3. **Swap by rename** onto the serving path — the running process
   keeps its old image (server-restart.md:80-85 becomes code).
4. **Ordering law, encoded**: document deploys and queue moves run
   BEFORE any worker restart. The command's step graph makes the
   wrong order unrepresentable, not discouraged — worker restarts are
   a later stage of the same typed plan, and a plan that sequences a
   worker restart ahead of its document deploy refuses at
   construction.
5. **Supervision-record migration**: re-write records whose argv
   carries retired flags, then **prove** one supervised restart of a
   test record before any live record restarts (runbook step 5,
   verbatim, as a gate).
6. **Restart stages**: server restart via piece 1's stop verb
   (drain report captured into the update record), then workers
   through the existing supervisor. Each restart captures
   `spawn_binary` — the upgrade is stated per process, inherited from
   the managed-worker discipline.
7. **Post-swap ladder as checks**: `/build` shows the target
   commit/hash (asserted on the BODY, never the status code — the SPA
   catch-all trap at `build_identity.rs:19-39`), `aion versions`
   non-empty, worker re-registration observed, changelog endpoint at
   the new version. The update record carries the ladder's results.

The whole run is itself durable where it can be: the update plan and
its step results are a record the console can render and a resumable
act — a box that loses power mid-update reconciles from the record
and the on-disk truth (every step is idempotent-or-refusing:
acquire re-verifies by hash, swap re-checks the serving path's hash,
restarts re-read live state). Aion's own event-sourced discipline,
applied to its own upgrade.

### Piece 3 — console-driven update

The console's update panel gains the second act beside "check now":
an **update action behind the deploy grant** (the same seam as every
mutating affordance; hidden or explicitly disabled when the server
does not grant or mount it — the console's standing rule). It fires
the same server-side mechanism as the CLI verb and renders the same
typed update record, step by step.

**The actuator sits outside what it fires into.** A server cannot
honestly supervise its own replacement: the in-process update
mechanism ends at "stage acquired + plan recorded"; the
stop-swap-restart tail is executed by a detached updater child (the
launcher's detached posture, reused) that outlives the server process
and completes the plan against the record. The child is spawned
before the stop, owns no listener, and reports into the update
record; the restarted server's boot reconciliation reads the record
and states the outcome. This is the one genuinely new mechanism in
the theme and gets its own brief with failure-path tests first
(updater dies mid-swap; power loss between stop and start; hash
mismatch after rename).

### Policy: auto vs on-word

Per-box **update policy as configuration**: `on-word` (default —
nothing moves without an explicit operator act, CLI or console) or
`auto` (the box follows a channel and runs the same typed plan on
publish). Shipped default is on-word; auto is an explicit per-box
enablement. Nothing in the mechanism forks on the policy — auto is
the same plan with a different trigger — so the "auto-update vs
update-on-his-word" question becomes a per-box enablement Tom
exercises when a box earns it, not a design decision blocking work.
(NO ASSUMED DEFAULTS: auto's cadence/channel are demanded
configuration with no fallback values.)

### Version-skew honesty (small, rides along)

`RegisterAck` gains the server's build identity; workers log the
skew; the CLI states server-vs-CLI version in `aion server status`.
Comparison stays observation — no admission gating on version (the
contract admission already gates on the thing that matters, the
typed action surface). Skew becomes a stated fact everywhere the
estate already states binary identity.

## Walls

- ⛔ **The check stays manual-only and side-effect-free** — the
  "knowing vs becoming" line survives; `update` never fetches the
  index itself.
- ⛔ **No launchd/systemd**: supervision remains Aion-native (the
  standing non-goal at `operations.md:706-708`).
- ⛔ **Swap by rename only**; binaries identified by SHA-256 at every
  step; a version string is never an identity.
- ⛔ **The ordering law is structural**: a plan that restarts a worker
  before its document deploy is unrepresentable, with a red test.
- ⛔ **Worker restarts under `on-word` remain gated by the operator's
  word within the plan** — the update command stages them and asks,
  it does not seize them (the runbook's "ON TOM'S WORD ONLY" step
  generalizes to plan-level operator confirmation, not silent
  seizure).
- ⛔ **No compat shims**: the runbook file retires INTO the mechanism
  in the same motion the mechanism lands (its prose becomes the
  command's documentation; the orphan file goes).
- **Credential surfaces** (registry tokens for private registries, if
  ever): Cally Ray's lane; this design assumes the public registry
  and names nothing about token handling.

## Brief decomposition (proposed, for commissioning)

1. **stop-verb brief**: pid file + incarnation reconciliation,
   `aion server stop|status`, the stop report, death-note outcome
   extension. Smallest, foundation, no new process mechanism.
2. **update-plan brief**: the typed plan (resolve/acquire/swap/
   deploy-order/record-migration/restart stages/ladder), plan record
   + resumability, CLI verb, ordering-law structural encoding.
   Depends on 1.
3. **updater-child brief**: the detached actuator, failure-path
   tests first. Depends on 2.
4. **console + policy brief**: the console action behind the deploy
   grant, per-box policy configuration, auto trigger. Depends on 2–3.
5. **skew-honesty brief**: RegisterAck identity, status surfaces.
   Independent; can ride any flight.

— Vesper, 2026-08-23. Sources: the two code censuses of this date
(server-stop; deploy-ritual/update-check), 204-CUT-RUNBOOK.md,
docs/operations/server-restart.md, docs/operations/operations.md
§deploys/§managed-workers, crates/aion-server/src/update_check/,
Tom's direction as relayed and recorded on the docs ledger.
