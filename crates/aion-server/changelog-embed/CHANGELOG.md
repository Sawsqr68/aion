# Changelog

All aion crates share one workspace version; entries below cover the
whole stack (crates.io) plus the `aion_flow` Gleam SDK (hex) where noted.

## 0.24.0 — 2026-08-24

**Server control verbs: `aion server stop` and `aion server status`.** The
estate's kill discipline, mechanized. The server now claims its home with a
pid record (`<AION_HOME>/run/aion-server.pid` — pid, start instant, binary
SHA-256, version, commit, bound addresses, and its drain window), and both
verbs verify the recorded incarnation against the live process table before
touching anything. A recycled pid names both incarnations: `stop` refuses
it — never a kill — and `status` reports the mismatch and answers 1.

- **`aion server stop`** SIGTERMs the verified incarnation, waits up to
  `--patience <seconds>` (absent, the verified-running record's own drain
  window, then the config's `drain.timeout_seconds` — the verb never
  invents a value), and renders the death note's account of the exit: the
  drain outcome by name, parked work by worker and task, or the honest
  absence when nothing was recorded. Exit codes a script keys on: `0` the
  goal state holds (stopped, already gone, or no claim — stopping is
  idempotent), `1` exclusively still-draining at patience, `2` the verb
  could not complete.
- **`aion server status`** answers "is the recorded server up?" through
  three independently observed layers — the record, the build identity
  (`GET /build`, asserted on the body, never the status code), and
  readiness — with each layer's absence its own honest state. Exit `0`
  verified-running, `1` not, `2` the verb itself failed.
- **A configuration that cannot load never destroys either verb's
  answer.** `status` states the failure and still answers its predicate;
  `stop` notes it and proceeds — the patience is only needed to wait on a
  verified-running server, so an idempotent pre-stop over an empty home
  still answers `0`, and only a running server that genuinely needs the
  window turns the failure into a refusal naming `--patience` as the
  remedy.
- The control verbs are **local by contract** and refuse a named
  `--endpoint`: they read this machine's `AION_HOME` pid record, and a
  destructive verb that silently ignores where the operator pointed it
  lands on the wrong machine.
- **One home, one claim.** A server booting into a home whose record a
  LIVE server holds boots UNCLAIMED and says so: it serves, but the
  control verbs address only the recorded claimant — `aion server status`
  prints a note (and answers 1) when an aion server answers HTTP without
  being the verified recorded incarnation. A claim needs the mutation
  lock beside the pid file.
- `aion launch`'s ready report now names `aion server stop` as the stop
  path (it suggested a raw `kill <pid>` before), and says which server
  the verb would address when the launched one is not the recorded
  claimant.
- **MSRV: the workspace `rust-version` is now 1.89** (was 1.85; first
  raise since the workspace was initialized). The branch's let-chain
  sweep requires it, and every published 0.24.0 crate carries the floor.
- The shutdown drain census now sees server-executed declared-command
  bodies: one still executing at drain end is reported by name in the
  drain outcome record, and the next boot re-dispatches the attempt.
- The server-restart runbook (`docs/operations/server-restart.md`) is
  rewritten around the verbs; the by-hand ritual survives as the fallback
  for servers started by pre-verb binaries.

## 0.23.0 — 2026-08-24

**Boot self-sufficiency: the writer-lock patience keys are retired.** The
0.22.0 estate deploy surfaced the defect: `store.lock_acquisition_patience_ms`
and `store.lock_acquisition_retry_cadence_ms` were required-with-no-default,
so a config written before they existed refused to boot — and the patience
ceiling itself guarded a hazard that cannot occur. The writer lock is a
kernel-released flock: its holder is either a live process (a draining
predecessor mid-handover, or another server on the same data directory) or
already dead, in which case the kernel has released it. A bounded wait could
only convert a slow handover into a refused boot.

- **The keys are deleted from the surface.** `HaematiteStore::open` /
  `open_or_create` no longer take a lock-acquisition ruling;
  `LockAcquisitionRuling` and its error type are gone. Boot waits for the
  writer lock indefinitely and **reports at intervals while it waits** —
  naming the lock file and the elapsed wait, so a log tail shows a handover
  in progress, never a silent hang. The wait probes lock availability with
  its own cheap `try_lock` and re-runs the real open only when the lock
  looks takeable, so waiting never multiplies recovery I/O. Two live
  writers still cannot coexist: the waiter never opens while the lock is
  held.
- **Configs carrying the keys stay legal.** TOML keys and the
  `AION_STORE_LOCK_ACQUISITION_*` environment variables are warned and
  ignored — never refused — so an existing estate's config boots unchanged.
  The first-run template documents the new behaviour.
- `aion-store-haematite` now carries a 1.89 rustc floor (std file-lock
  probes); every other crate keeps the workspace's 1.85.

**Instant doors: the transports bind before the startup catch-up.** A server
restarted over a large owed-timer backlog (an estate watcher down for hours
owes thousands of fires) used to keep every listener shut for the whole
sweep — tens of minutes of healthy catch-up presenting as a dead server.
Startup recovery is now staged: the workflow-residency leg (replay actives
to resident) still completes before the doors open, so signals, queries,
and cancels answer correctly from the first request; the unbounded legs —
owed timer fires, schedule-coordinator catch-up, schedule recovery — run as
a background task behind already-open doors. Every catch-up fire goes
through the same idempotent record-once path the live timer wheel uses
against a serving engine. A catch-up interrupted by shutdown resumes from
durable state on the next boot; a catch-up error is reported with its
remedy and never kills the serving process.

- New staged engine surface for deferred builds:
  `Engine::recover_workflows_on_startup` and `Engine::run_startup_catchup`,
  with `Engine::run_startup_recovery` unchanged as the composed pair.
  Ordering misuse answers the new typed
  `StartupCatchupBeforeWorkflowRecovery` error.
- The backlog is still delivered fire-by-fire; collapsing missed
  fixed-cadence fires and durable catch-up progress marks are follow-up
  work, deliberately not in this release.

## 0.22.0 — 2026-08-24

**The AWL command surface.** Two cuts of the AWL authoring surface land
together: the typed command-declaration surface (cut 1) and the declared
recipe surface (cut 2). Both ship with their teaching material — the ops
console's authoring guide and the assistant's `AWL-REFERENCE.md` twin move
in the same landings as the code, so the surface is documented the moment
it exists.

- **Typed command-declaration surface (cut 1).** Gate-leg commands and the
  diff-census family: commands are declared and typechecked rather than
  assembled as free text, with the census surface reporting what a
  declaration actually covers. Refusal causes are a closed vocabulary —
  an unknown declaration is a check error carrying a span, never a silent
  acceptance.

- **Declared recipe surface (cut 2).** Assembly acts, wall projection,
  census, and receipts: a recipe is a declared object whose verdict is a
  recorded `VerdictRecord`, the compare surface is generic over what it
  compares, and `record_of` is one-way by construction — a gate leg
  consumes a baseline as a RECORD, never as a recipe declaration. The
  amendment-1 fence (record is assembly-side, check is gate-leg-side) is
  enforced by fixture, not convention.

- **Teaching surface moves with the code.** The ops console authoring
  guide asset and `examples/assistant/resources/AWL-REFERENCE.md` are
  inside both cuts' gate walls, with a conformance test refusing drift
  between the guide asset and its source of truth.

## 0.21.0 — 2026-08-23

**A stability wave.** The fleet-plan contract joins the workspace as a
first-time crate, an intermittent test red is cured at its mechanism, the
fleet delivery script survives the declared-command executor's cleared
environment, and held-execution claims stop dirtying worktrees.

- **New crate: `aion-fleet-plan`** — the fleet-stages-v1 staged-chain plan
  contract: plan grammar, validation, the normalized ordering graph, the
  sealed conclusion verifier, and the replay-deterministic chain
  decide/step/apply/report surface, with the `workflows/fleet-chain/`
  documents driving it and `fleet-plan-check.sh` wiring it into fleet-dev.
  The runtime NIF pass rides the same landing: the `now_value` test helper
  now refuses error tuples instead of silently comparing them as
  timestamps.

- **Norn held-execution claims no longer dirty worktrees.** The unchanged
  `.aion-held-execution.json` reservation now lives below Aion's state area,
  keyed by the canonical worktree path, while preserving live-sibling
  exclusion, drop release, reattach, and exact-incarnation stale cleanup.

- **Failover-showcase survivor reads wait for convergence (#151).**
  `replicate_append` commits on a strict majority — the proposer plus
  exactly one of the two peers — and the peer that did not carry the
  quorum applies the batch on its next inbound-write drain, 295µs–5.4ms
  later. Reading a named survivor the instant an append returned sampled
  a race the protocol never promised to win; under whole-workspace load
  that window stretched into an intermittent red. Both survivor reads now
  poll through `await_replicated`, bounded by the same operation timeout
  as every other cluster step, and a survivor that never converges fails
  by name and count. Evidence pack at
  `docs/evidence/seqfix-quorum-visibility/`.

- **`fleet-push.sh` derives HOME under the declared-command executor.**
  Declared commands run with a cleared environment — PATH plus declared
  entries only, by design — so git resolved no per-user credential
  configuration and died prompting for a username. The delivery script now
  derives HOME from the passwd entry of the uid it runs as, with a typed
  refusal when it cannot, keeping the credential chain reachable on every
  fleet box.

## 0.20.0 — 2026-08-23

**The beamr 0.20.0 floor.** A dependency-floor release, one motion across
the estate: beamr 0.19.3 → 0.20.0 (the accessor-lifetimes release — the
heap-backed binary accessors now take a `HeapBorrow<'heap>` witness),
haematite 0.9.0 → 0.11.0 (the release cut declaring `beamr ^0.20.0` on both
edges), and the liminal family moved from a git pin to its registry 0.10.0
wave (`liminal-rs` 0.7.0, `liminal-server` 0.10.0, `liminal-sdk` 0.9.0 —
all declaring the same floors), so every resolved graph holds exactly one
beamr.

aion's own public surfaces do not move: the witness threading lives inside
the engine's `runtime` boundary module (helpers, the timer dispatch
closure, payload/outcome/delivery conversion). The exception-reason path
reads through an ETS-owned copy because beamr 0.20.0 gives a
`NativeHandler` no heap borrow (beamr#36); that bridge drops out when the
upstream accessor ships. Gate evidence for the migration tree lives at
`gate-logs/beamr-020-bump/` (clippy `-D warnings` clean, full workspace
suite green, with the voided first run adjudicated in place).

## 0.19.0 — 2026-08-21

**Breaking.** This release moves public surfaces in six crates
(cargo-semver-checks: 23 findings against 0.18.1), all deliberate:
`HaematiteStore::open`/`open_or_create` now take the ruled lock-acquisition
patience and cadence; the run-scoped read arc adds `RunSummary.started_seq`,
`DescribeWorkflowResponse.generations`, and `EngineError::RunNotInHistory`;
the server config family gains `jit_threshold` and the required
store/observability fields; the MCP server's task and dispatch signatures
are reworked; `DeclaredCommandDispatcher::new` and
`cancel_in_flight_activities` grow a parameter; `ServerError` gains
variants. Consumers upgrade by taking the new surfaces — this line ships no
compatibility shims by ruling.

*Release record:* a 0.18.2 cut of this same tree was partially published on
2026-08-21 (17 of 21 crates) before the semver gate's verdict was ruled to
mean what it said — these changes are a major step, not a patch. Every
0.18.2 crate was yanked within minutes; 0.18.2 was never a release, and no
tag exists for it.

- **Haematite writer-lock acquisition is ruled and retried** (the rb-lockfix
  arc, PR #113): a node opening a busy store retries on an explicit
  operator-ruled patience and cadence instead of dying on first contention.
  The ruling (`store.lock_acquisition_patience_ms` /
  `store.lock_acquisition_retry_cadence_ms`) is required with no defaults,
  transient writer-lock inheritance is retried, and generated configs state
  the ruling. The estate's flake carriers were diagnosed and cured at their
  causes, not waived.
- **Run-scoped read surfaces resolve exactly the run they name.** The query
  seam resolves a workflow's CURRENT run, never a stale predecessor;
  reconcile projects exactly the named run and an absent run is an error,
  pinned at the raise and both wire halves (#94); `list_runs` decodes the
  summary actually on the wire (#83); reopen admits by handle identity (#51).
- **Costly proofs announce or run — never silently skip.** The five
  `#[ignore]` costly suites are runtime-gated (`AION_RUN_COSTLY_TESTS=1`) and
  each announces, on the real stderr handle, which proof did not happen; the
  battery consumes the announcements through a venue-aware gate plus a
  vacuity scanner that attributes every announcement to the leg that
  produced it. Armed, all five run green — the cross-node kill-9 failover,
  parked-timer exactly-once, and hex.pm scaffold proofs are now real battery
  passes. Two named-unproven gaps are tracked loudly in the run output
  itself: the outbox seam (#114) and the stacked-dev start refusal, which is
  pinned as a test that fails the day the gap closes (#115).
- **The battery runs exactly what it declares** (line 28): one gate list —
  the runner executes `gates.json` verbatim, verified after every run by
  `scripts/gates/verify-declaration.sh`, and the gate library sources
  cleanly under plain `sh`.
- **The release ceremony and the repo battery are workflows in the tree.**
  `release_ceremony` makes standing order 4 structural — the tag step's
  input is the per-crate index-confirmation set, so tag-without-publish is
  inexpressible; changelog dating, the semver surface, the declared publish
  order, and idempotent per-crate resume are all legs. `repo_battery` runs
  the declared gate list with typed receipts, offline and live proof
  harnesses, and honored worker-environment and teardown refusals. The
  issue-investigation and fleet-dev workflows ship alongside.
- Package file IO failures are classified in the toolchain; engine task
  descriptors release on drop; all-target build artifacts are bounded; the
  transport-loss budget is a constructor parameter (#100).

## 0.18.1 — 2026-08-19

- **The beamr >=0.19.2 floor lands across the workspace** (closes #70, the
  migration that closes #64 for real), proven at the resolved lock by the
  landing gate's lock-witness leg.
- **haematite `^0.8.3` -> `^0.9`.** The 0.18.0 pin was deliberate emergency
  isolation (see below); the beamr floor forces the pair — haematite 0.8.x
  pins beamr ^0.18. Fresh resolutions from the index take haematite 0.9.1.
- **Spawned children record their parent** (closes #77):
  `Event::WorkflowStarted` carries `parent_workflow_id` (serde-defaulted, old
  histories decode unchanged), the spawn path threads the parent's workflow
  and run ids, and `WorkflowSummary` projects the parent from history —
  handoff chains read backward as well as forward.
- The embedded AWL SDK closure was re-harvested on the landing venue and its
  checksum pins moved with it; the freshness oracle's venue-dependence (BEAM
  bytes follow the host toolchain) is tracked as #78.

## 0.18.0 — 2026-08-19

- **Upgrade adoption for pre-budget stores** (closes #75): a haematite store
  written before `node_cache_budget` existed now adopts the caller's ruling
  on open instead of refusing to boot. A present ruling wins untouched;
  corrupt configs are refused with a typed error naming the path. An upgrade
  must never present as data loss.
- haematite pinned at `^0.8.3` — deliberate emergency isolation for the
  same-night ship, superseded by 0.18.1's `^0.9`.
- **Haematite is Aion's durable store.** The libsql backend, the
  `libsql-backend` and `haematite-backend` cargo features, and the
  `aion-store-libsql` crate are retired; the cut ships without the
  `aion-store-libsql` crate. Configurations that name `backend = "libsql"`,
  `store.url`, `AION_STORE_BACKEND=libsql`, `AION_STORE_URL`, or
  `--store-url` are refused at
  startup with the haematite remedy — named, never ignored, never silently
  migrated. This build does not read libsql database files. **0.17.0 is the last
  release that can read them**: if you ran libsql on any release up to and
  including 0.17.0, keep a 0.17.0 binary to reach that data before upgrading.
  This subtraction closes the retired backend's #85 and #229 defect surfaces; it
  does not claim to fix the separate haematite memory defect tracked in #32.
- First release of the whole 21-crate graph published and checksum-verified
  at the index. The 0.16.0/0.17.0 tagged-never-published drift is #76; the
  ceremony now gates the tag on publish-and-verify-at-index.

## 0.17.0 — 2026-08-17

- Recovery adoption (#246), retryable occupancy (#33), recovery ordering
  (#266), the death-note breadcrumb, and the aion-authoring skill —
  43 commits since v0.16.0.
- **Tagged but never published to the index** (#76). Superseded by 0.18.0,
  the first cut published under the tag-gated-on-index ceremony.

## 0.16.0 — 2026-08-16

- **A workflow's displayed status is reconciled from its full recorded history —
  never from a stale cached row.** `POST /workflows/list` now rebuilds each row
  from the run's complete event history before filtering and serving: a finished
  workflow can never be shown Running, and a mid-flight workflow can never be
  shown Failed off a divergent durable visibility row (the stored cache is
  corrected in the same pass). Opaque terminal messages of the form
  `workflow N returned error` are replaced by the first line of the recorded
  structured error detail; full detail stays in event history. Both behaviours
  are pinned by red-first regressions.

- **The fleet document fans out (#240):** parallel legs in isolated worktrees
  with a server-run merge; fan-in merges before it carries. Run 2 went green
  end-to-end on the resumed pipeline.

- **Child runs are visible while they run (#244):** the server derives child
  legs from parent run history and resolves the current run for child
  projection (concurrently and observably); the console folds live child leg
  transcripts into the run page and discovers fanout from live parent events;
  the CLI tails parent and child transcripts together, follows successor leg
  attempts over sockets, reports tail backfill gaps truthfully, replays
  workflow events across the tail-attach window, and scopes replay payloads to
  the selected run. Unknown parent runs are rejected.

- **An agent outcome carries a four-field contract everywhere (#205):** the
  outcome names the conversation that produced it, and the final message is
  carried distinctly from the outcome fields. Worker protocol stubs (Python and
  TypeScript) regenerated to the current contract.

- **A cancelled run tells its workers to stop (#233):** cancellation propagates
  to serving workers at the wire; the last delivery arm (TransportCannotCarry)
  is red-pinned.

- **Sibling agent spawns into a live worktree are refused (#246),** as are
  pending responder collisions on the same worker — both explicit refusals,
  both red-first.

- **A red gate says WHY, in the record (#236).**

## 0.15.0 — 2026-08-13

- **A norn agent's working directory is declared — and each job can name its
  own (#222, #228).** The directory an agent stands in decides which tree it
  reads and which files it edits. For a norn agent that value appeared in NO
  document, NO argv and NO log: it was inherited from wherever the worker
  process happened to be started. That is invisible configuration nobody wrote
  down and nobody reviewed, and ACP's `cwd` was already required and
  gate-tested — the asymmetry was the defect.

  `cwd` is now a REQUIRED setting on every `harness` section, norn as well as
  acp, held to the same absolute-path rule. It may be written in either of two
  forms. A LITERAL path pins the section to one tree:

      harness
        kind norn
        cwd "/srv/agents/checkout"

  A BINDING makes the tree a property of the WORK rather than of the LAUNCH,
  taking each attempt's working directory from a named input of the action
  being served — so one worker serves many repositories and the dispatcher
  need not know the path before the run exists:

      harness
        kind norn
        cwd from repo
      action delegate(repo: String, prompt: String) -> Reply
        agent

  The binding is settled STATICALLY, at `aion awl check`, against every action
  the harness actually serves — the serving closure, which is narrower than
  the declaring closure. A document naming a parameter no action declares, or
  declaring it at the wrong type, is refused by name at the `cwd` line. The
  adapter refuses an attempt that carries no such value BEFORE spawning
  anything, so a misrouted dispatch cannot reach an agent at all.

  A LITERAL path may lead with `{workspace_root}`, expanded by the launching
  box, so one document serves boxes that keep their trees in different places.
  A BOUND value is not expanded — it arrives from the caller already made — but
  it travels the same net: absolute, and present on disk, or the attempt is
  refused terminally before anything is spawned.

  The worker's startup line now states the directory it will spawn children in,
  read through the RESOLVED harness rather than off the document, so the
  reported value is the value the child gets.

  **🔴 Who this affects — action required.** A norn `harness` section written
  against 0.14.1 carries no `cwd` line, because declaring one was refused there
  as a setting foreign to norn. Every such document now fails `aion awl check`
  with a named refusal until a `cwd` is added. Write the literal form to keep
  the behaviour you had — name the directory your worker was being started
  from — or the binding form to serve many trees from one worker. That shell
  discipline is no longer load-bearing either way: the declaration decides.
  ACP documents already carried a required `cwd` and are unaffected.

- **The authored action `timeout` is an armed per-attempt deadline (#223,
  closes #198).** AWL lowered `timeout`, the Gleam SDK put it on the wire as
  `timeout_ms`, and NO reader existed in the engine — the only field parsed out
  of that dispatch config was `retry`. A hung activity ran forever while its
  document claimed a bound. The engine now reads the bound and arms it; expiry
  is recorded as a non-terminal failure that NAMES the bound, and the retry
  policy (if any) drives re-delivery with an incremented attempt.

  **Who this affects.** Anyone whose documents already declared `timeout` —
  the declaration now has force where it previously had none. A workflow that
  was silently relying on an unbounded activity will begin seeing that activity
  expire at the bound it declared.

- **A run carries a display name, and the history keeps every one (#211).**
  `aion.display_name` is a search attribute; renames are recorded as
  `SearchAttributesUpdated` events rather than mutating a field, so the run's
  own history is the record of what it has been called. The console always
  shows the short UUID beside the name — the name is a label, never an
  identifier, and there is no by-name parameter on any API.

- **Timestamps render in the viewer's own timezone (#212).** One render truth
  in the console, with the raw UTC value preserved on the `<time dateTime>`
  attribute so the record on the page is still the record. RFC3339 offsets are
  mandatory at the boundary.

- **`update_watch` — the update check as a durable workflow (#213).** The
  half-hourly check now runs as an ordinary Aion workflow with an
  operator-named cadence, driving the server's embedded check. Manual-start
  only, by design: a schedule is a value an operator names, not a default.

- **beamr 0.17.0 → 0.18.2 (#220).**

- **Fixes.** A worker document's timeout refusal states a reason that is true;
  a document name the server accepts is a name AWL can declare (#153); a
  relative path in an ACP document is a configuration refusal rather than a
  protocol fault; the manifests no longer claim the default build excludes the
  ablative stack (#218); the documented slim libSQL build (`--no-default-features
  --features libsql-backend`) is real again (#186).

- **Known issue, still open and now measured more sharply: the libsql backend
  can fail whether or not the machine is busy (#85).** This release pins the
  same `libsql` 0.9.30 as 0.14.0 and carries the same defect. 0.9.30 remains
  the newest published stable libsql, so there is no version to move to yet.

  **Correction to what 0.14.0 said.** That release described the failure as
  arriving "rarely, under concurrent load". That is how it *looks*, and it is
  the wrong picture. The underlying fault is not occasional and does not need
  load: **every libsql connection this backend closes is closed twice**, always,
  including on a single-threaded run of one test. What load and concurrency
  change is only whether you *notice* — whether the freed memory has been handed
  to something else by the time the second close reads it. A quiet machine is
  not protection; it is a machine where the same fault is less likely to be
  seen.

  Proven on 2026-08-13 by re-running the store's own test binary under macOS
  Guard Malloc, which unmaps freed pages instead of recycling them. The run
  faults immediately and in the same place every time: reading the connection
  handle's state field inside `sqlite3_close_v2`, called from libsql's
  `Connection::disconnect` — a read of memory the *first* close already
  released. Without Guard Malloc that same read usually returns whatever now
  occupies the block, which is why the visible symptom is intermittent.

  **What it looks like from the outside.** A store error reading
  `SQLite failure: bad parameter or other API misuse`, or a failure to open a
  database at all reading `Unable to open connection to local database <path>:
  21`, or a process abort. All three are the same fault wearing different
  clothes: SQLite's "misuse" code, code 21.

  **Why it is the dependency's and not ours.** In the SQLite that libsql
  bundles, the open call can only return code 21 down two paths: a null output
  pointer, which libsql never passes, and a check on the open flags. Aion's
  flags always pass that check. So the flags the call read were not the flags
  that were written — a memory-safety fault inside the dependency, which no
  input from aion can cause or prevent.

  **Who this affects.** Servers explicitly configured with
  `store.backend = "libsql"`. The default haematite backend and the memory
  backend do not use libsql and are unaffected. Within libsql it is not an
  exotic path: the double close was caught on the ordinary startup catalog
  reload, the read every libsql-backed server does when it comes up.

  **What to do.** Prefer the default haematite backend — and note that this
  backend is on its way out. Retiring `aion-store-libsql` in favour of
  haematite is planned as the next release's work, so an operator standing up a
  new server should choose haematite now rather than migrate later. If you run
  libsql and see one of the faces above, it is this known defect rather than an
  aion regression.

## 0.14.1 — 2026-08-12

- **The embedded assistant document is launchable by the verb the server
  recommends (#209).** 0.14.0's QUEUE MOVE warning told the operator to start
  the assistant's worker with `aion worker agent assistant.awl …` — and the
  embedded document carried no `harness` section, so that exact invocation
  refused. The document now declares its own launch: `kind norn`,
  `concurrency 4`, the reconnect policy carried verbatim from the invocation
  the docs published for this document before the flag migration (1s initial,
  30s cap, 100 attempts — the old flags had no defaults; these were the
  documented values), `env_pass "PATH", "HOME"`, and the norn session
  arguments (`--session-id {workflow_id}-{activity_type}`,
  `--resume-if-exists`, `--account default`).

  **Who this affects.** Anyone following the 0.14.0 queue-move instructions,
  and anyone re-exporting the document with `aion assistant document` — the
  exported bytes change.

  **What you will see.** `aion worker agent assistant.awl --liminal-address
  <addr> --identity assistant-worker` now comes up on queue `assistant` as
  the warning says it will. Three behaviours differ from a pre-#204 flag
  launch: the spawned norn child's environment is constructed from `env_pass`
  alone (exactly `PATH` and `HOME` — a box whose norn needs another variable
  adds it to that list), sessions run under the `default` norn account alias
  (edit that one token in the document for a different account), and the
  session prompt now instructs the agent to work inside its per-session
  workspace rather than asserting a confinement the launch does not provide
  (`--workspace-root`/`-C` were flag-era settings; operators who want
  filesystem confinement add the pair to the document's harness `args` with
  their own root before deploying — `{workspace_root}` expansion, which would
  return that confinement to the shipped artifact, is board task #210).

  **What to do.** Nothing, if you already made the queue move with a working
  launch. Otherwise follow the warning's own instructions — they now produce
  a worker that comes up on the assistant's queue instead of a refusal. The
  warning also tells the whole truth about the listener: a
  boot whose outbox does not dispatch on liminal (memory-backend quickstart
  included) is told it binds NO liminal worker listener, and which `[outbox]`
  settings commission one, instead of being handed an address that was never
  going to answer.

## 0.14.0 — 2026-08-11

- **The built-in assistant no longer claims queue `default` (#200).** The
  embedded assistant document now declares its own private queue, named after
  the workflow type it exports, and the server refuses the embedded document
  at load if it declares anything else. `default` is the queue every
  out-of-box worker comes up on with no configuration, and contract admission
  holds a registering worker against every reachable contract on its queue —
  so an assistant sitting on `default` refused the first worker a newcomer
  started.

  **Who this affects.** Operators whose catalog was created before this
  release: the routed assistant there still declares the old queue.

  **What you will see.** Nothing moves by itself — a restart never re-points
  a route an operator placed. The server logs `QUEUE MOVE PENDING` at WARN,
  naming the queue the routed version serves, the private queue the embedded
  version declares, and the commands that perform the move; sessions on the
  routed version keep running where they are.

  **What to do.** Make the move deliberately: `aion assistant document
  --output assistant.awl`, then `aion deploy assistant.awl` (which loads and
  routes it), then start a worker on the private queue. A worker still
  serving the old queue will not receive the new version's dispatches.

- **A worker document's `harness` section is now the one truth for how its
  agent is launched (#204).** Agent-carrying worker documents declare the
  harness in the document itself — `kind` (norn or acp) with the fields that
  kind requires — and the spawned agent child runs under the DECLARED
  environment: the `env_pass` list names exactly which variables reach the
  child, and nothing is inherited from whatever shell or supervisor started
  the worker. Fourteen launch flags are deleted from `aion worker agent`
  (`--harness`, `--concurrency`, the three `--reconnect-*`, the five
  `--acp-*`, and the three `--norn-*`); each setting lives in the `harness`
  section under the field the refusal names.

  **Who this affects.** Anyone starting `aion worker agent` from an old
  runbook line, and any durable Builtin supervision record written with the
  old flags — a supervised restart of such a record is refused, naming the
  record, the flag, and the remedy. `aion worker awl` is untouched:
  `--concurrency` and the `--reconnect-*` trio remain live, required flags
  there, and worker-family documents cannot carry a `harness` section (every
  action in them has a declared body; there is nothing for an agent to
  serve).

  **What you will see.** On `aion worker agent` with a deleted flag: clap's
  own argument error, followed by one sentence per deleted flag naming the
  `harness`-section field that replaced it. On a supervised restart of an
  old-flag record: a refusal naming the record and the remedy — never a
  silent drop or a half-started worker.

  **What to do.** Move each named setting into the worker document's
  `harness` section and redeploy; re-write Builtin supervision records to the
  new shape before relying on supervised restart (the release runbook carries
  the verification step).

- **gRPC workers now prove the server can reach them before they are given
  work (#197).** A worker connected over gRPC is sent a liveness ping down the
  same stream its tasks arrive on, and answers it. Until it has answered two in
  a row it is not selected for dispatch — the identical probation a
  liminal-connected worker has always served. Previously the probe never
  reached gRPC workers at all, which is why `aion worker awl serve` could
  register successfully and then sit idle forever.

  **Who this affects.** Any worker running a binary built before this release
  on a server that runs the liveness probe: it does not know how to answer the
  ping, so it never clears probation and stops being dispatched to. The bundled
  Python and TypeScript SDK stubs are in this position until their regeneration
  lands (#201). Rust workers rebuilt against this release answer automatically —
  the worker runtime answers, not your activity code, so nothing in a workflow
  or an action needs changing.

  **What you will see.** Not silence. The server says why, by worker id: a
  `SERVING ITS DISPATCH PROBATION` line while a worker is earning eligibility,
  a `WITHDRAWING DISPATCH ELIGIBILITY` line naming the reason if a worker that
  had it stops answering, and a `DISPATCH ELIGIBILITY RESTORED` line when it
  recovers. A dispatch that finds no usable worker now also distinguishes "no
  worker is registered for this queue" from "workers are registered and none is
  currently eligible" — the second tells you to read the verdict above rather
  than start another worker, which would only serve the same probation.

  **What to do.** Upgrade the worker binary to one built from this release. If
  you are on the Python or TypeScript SDK, the answer arrives with #201's stub
  regeneration; until then those workers are excluded from dispatch on a server
  whose probe is running.

- **Known issue: the libsql backend can fail under concurrent store access
  (#85).** The `libsql` dependency this release pins (0.9.30) carries a
  double-close defect — a use-after-free in connection teardown — that can
  fire when many tasks drive one store concurrently. It surfaces as a store
  error naming `SQLite failure: bad parameter or other API misuse`, or as a
  process abort (SIGSEGV/SIGABRT). The defect is in the dependency, not in
  aion's store adapter, and it predates this release: 0.13.8 ships the same
  pin. The concurrent-tasks-against-one-store shape is exactly how the server
  uses its store, so this is reachable in production, not only in tests.

  **Who this affects.** Servers explicitly configured with
  `store.backend = "libsql"`. The default haematite backend and the memory
  backend do not use libsql and are unaffected.

  **What you will see.** Rarely, under concurrent load: the API-misuse store
  error above, or a process crash. In this release's own gate battery the
  defect fired roughly once per full workspace test run — each time at a
  different test, always with the same face.

  **What to do.** Prefer the default haematite backend. If you run libsql
  and hit this face, it is this known defect, not an aion regression; the
  remedy is a dependency move, tracked as #85.

## 0.13.8 — 2026-08-11

- **Errors explain themselves in the editor (#187).** An AWL diagnostic now
  carries its full extent on the wire as a UTF-16 `{ from, to }` range — on
  `/awl/check` and on `/awl/fmt`'s 422 arm alike — and the console editor
  shows it: squiggles under the error's whole extent (the old one-character
  approximation is deleted), hover and cursor tooltips carrying the remedy
  prose, and a diagnostics panel listing every diagnostic in full — the
  assistive-tech path to the same information. A rangeless response, or one
  with fractional or negative positions, is refused at the boundary rather
  than guessed at.
- **The language panel organizes by the grammar (#188).** The console's
  language panel now groups its 63 words by grammatical form — with a
  By form / By section toggle, since the CLI still speaks sections — and
  every word card wears its form chip and the full list of positions where
  the parser actually admits it. The whole §1–§16 language reference renders
  in the panel in a collapsed disclosure, pipe tables and all. The grammar
  table behind the panel is promoted into the compiler and gated against the
  parser itself, so the panel and the editor's completion are structurally
  unable to disagree about a word's form — a promotion that caught and
  corrected fourteen words the old table offered at positions the parser or
  checker refuses.
- **The server knows the latest installable aion (#189).** A manual,
  operator-started update check — an AWL workflow shipped inside the binary,
  whose declared action curls the crates.io index through the ordinary
  declared-body spine, so the server compiles in no outbound-HTTP dependency
  — records the greatest installable `aion-cli` version: not yanked, not a
  prerelease. `GET /update-status` (authenticated like `/build`) answers
  `{ latest_known, checked_at }`, honest nulls until the first check. The
  console header shows an "update available" pill only when the latest is
  newer than the running build, with a loud "version unreadable" state
  instead of a silent hide when either side won't parse. Manual-only by
  design: no cadence, no timer — nothing fires without an operator's start.

## 0.13.7 — 2026-08-11

- **Agent steps return a structured outcome (#181).** An `agent` action now
  returns the canonical record `{ text, stop_reason }` from every harness —
  Claude (ACP), Norn, and the plain-CLI adapter alike — in place of one bare
  string whose shape depended on which harness produced it. `text` carries the
  agent's terminal text (honestly empty when a completed turn emitted only
  tool calls); `stop_reason` is one closed vocabulary:
  `end_turn | tool_use | limit_reached | cancelled | error | other`. A
  workflow declares a record of that shape on its `agent` action and reads
  `reply.text` / `reply.stop_reason` as ordinary fields — so it can route on
  *why* the agent stopped, not guess from the text.
- **The checker demands the record.** `aion awl check` refuses an `agent`
  action whose declared return is the retired bare `String`, and the refusal
  names both fields the record needs. Every shipped example, workflow, guide
  entry, and document is migrated; the language guide and reference teach the
  record.
- **Misconfigured agent steps fail once, not five times.** A harness contract
  violation — a completed run carrying a null or non-string output — is now a
  terminal, deterministic activity failure that names the violated contract,
  instead of burning the retry budget on a failure retries cannot fix.

## 0.13.6 — 2026-08-10

- **Every guide example is a playground (#185).** Each of the 118 examples in
  the console's language guide — the Language tab and the floating guide
  overlay alike — carries a **Try it** button below its source: one click
  creates a workspace document from the example's verbatim bytes, named after
  the example's own `workflow`/`worker` declaration (collision-safe with a
  `_2`/`_3` suffix, each attempt an atomic server-side create), and lands you
  on it in the editor — ready to check and deploy, and to run when the example
  declares a workflow. A refusal (say, no authoring workspace configured) is
  announced in place in the console's failure vocabulary rather than passing
  silently.
- **`POST /awl/documents` accepts an optional `source`.** When present, the
  document is created carrying those bytes verbatim (unchecked, exactly like
  saving); when absent, the placeholder is synthesized as before. Same name
  validation, same atomic create, same `DocumentExists` refusal.

## 0.13.5 — 2026-08-10

- **The assistant can write (#184).** The MCP surface grows from eight run
  tools to thirteen: `list_documents`, `read_document`, `check_document`,
  `save_document`, and `deploy_document` give an MCP-connected assistant the
  same authoring loop the console studio has — list and read what is in the
  workspace, check source, save it, and deploy the saved document by
  `{path, content_hash}`. Reads need only authentication; saving and
  deploying require the same deploy grant as `POST /awl/deploy`, and a
  caller without it is refused as `deploy_denied` naming the grant. There is
  deliberately no way to deploy unsaved source.
- **`check_document` answers in what a model can act on** — `ok`,
  `deploys_green`, the deploy `steps`, and per-line `diagnostics`. The
  editor's semantic index (hundreds of kilobytes on a real document) stays
  on the HTTP surface where the console uses it.
- **A missing document is now "not found" everywhere (#161).** A freshly
  started server has its authoring workspace configured but not yet created
  (nothing creates the directory until the first write). Reading, checking,
  or deploying against that state — or naming a document that simply is not
  there — now answers `DocumentNotFound` with guidance on every transport,
  instead of a raw operating-system error: HTTP deploy of a missing path is
  `404` (previously a `500` on a fresh server, or a `422` labelled as a
  revision-store fault), and the MCP tools point at `list_documents`.
- `deploy_document` is honestly annotated non-idempotent: every deploy
  appends a fresh deployment record to the ledger, while `save_document`
  converges on identical bytes and stays idempotent.

## 0.13.4 — 2026-08-10

- **A first boot configures itself (#180).** `aion server` finding no config
  at any discovery layer now writes `<AION_HOME>/config.toml` (owner-only)
  from its embedded template, with the deploy surface and the outbox worker
  channel commissioned and every choice explained in comments. An existing
  config is never touched; an explicit `--config` naming a missing file still
  refuses. `scripts/setup.sh` no longer writes config at all.
- **Bare `aion` is the launcher.** Typing `aion` with no subcommand starts
  the server in the background, waits for its own liveness probe, reports in
  plain lines where the config, data, console, and log live (and the pid that
  stops it), and opens the ops console. `--foreground` keeps the server in
  the terminal; `--no-open` skips the browser. An already-running server is
  reported, never double-started; a port held by something else is refused by
  name.
- **Machines set up by the old `setup.sh`** (which wrote a minimal
  `~/.aion/config.toml` enabling `[deploy]` and `[outbox]` only) will see the
  server refuse to boot with
  `outbox.transport=liminal requires outbox.liminal_listen_address …` naming
  that file. The one-line fix: add
  `liminal_listen_address = "127.0.0.1:50061"` to the `[outbox]` section of
  the named file.
- Tilde expansion: a leading `~` now expands against `$HOME` in
  `store.data_dir` and `authoring.workspace_dir` (as it already did for
  `AION_HOME` and `--config`); `~user` is refused by name.

## 0.13.3 — 2026-08-10

- **Ship & Run is retired.** The all-in-one check-deploy-start button is gone;
  the editor header's separate Deploy and Run actions are the one honest way
  to do each. The run view survives only for worker documents, where it hosts
  their managed-worker deploy surface.

## 0.13.2 — 2026-08-10

- **Deploy and Run, from the editor.** The authoring editor's header gains two
  separate buttons. **Deploy** saves the buffer you are looking at and ships
  exactly that revision — the outcome (or the server's refusal, verbatim) stays
  on screen in its own line until the next deploy, and the button explains
  itself when the document isn't ready. **Run** opens a panel beside the editor
  hosting the schema-driven start form, seeded with the document's workflow
  type, with a line reporting whether anything currently serves the task queue.
  Deploying never starts a run; running never deploys; you never leave the
  console.
- The run panel keeps its own per-document form draft, so it never clobbers a
  half-filled form on the Actions page — and vice versa.

## 0.13.1 — 2026-08-10

- **The console tells you what's new.** A "What's new" button in the console
  header opens this changelog, fetched from the new `GET /changelog` endpoint.
  The document is embedded in the server binary at compile time — never read
  from disk at runtime — so the panel always describes the version actually
  serving the console, and a drift gate pins the embedded copy byte-identical
  to the authored `CHANGELOG.md`.

## 0.13.0 — 2026-08-10

The everything-embedded release: `cargo install aion-cli` carries the whole
product — console, guide, examples, assistant — with no repository checkout
and nothing fetched at runtime.

- **The guide teaches by example.** One AWL glossary now stands behind every
  authoring surface — `aion awl guide` on the CLI, hover in the LSP, and a
  non-modal overlay in the console — with citations into the full language
  reference. 118 canonical examples ride with it, every one checked by the
  gate that ships them, rendered in the console with the editor's own
  tree-sitter highlighting in collapsible sections.
- **The language reference ships inside the binary.** `aion awl guide
  --reference` prints the full reference from bytes embedded at compile time;
  an installed build carries exactly what the repository holds, and a drift
  gate pins the embedded copy byte-identical to the authored document. This
  also fixes the 0.12.0 packaging escape where the reference was read from a
  path outside the `aion-awl` crate.
- **Everything doable from the console.** Starting a workflow gets a
  schema-driven form built from the document's declared input schema. Managed
  worker lifecycle — deploy a worker document, start, stop, restart — runs
  over HTTP from the console. The deployed surface shows each version's
  authored AWL read-only, and deep links (`/workflows/<id>`, deployed
  `?type=&version=`) survive a reload.
- **Quiet is not a fault.** The console's connection indicator distinguishes
  `idle` (nothing has subscribed to the event stream) from `disconnected`
  (reconnect exhausted); an idle stream shows nothing instead of a false
  "Disconnected". Deployed versions that carry no archived AWL source are
  withheld from the source browser and counted honestly instead of opening an
  empty editor.
- **The assistant ships in the server.** `assistant.awl` is embedded in the
  binary with an operator verb to deploy it — no checkout needed — and its
  resource documents are current against this release's system.
- **MCP server.** A Model Context Protocol endpoint (2026-07-28 stateless
  revision) at `POST /mcp`: eight tools over run-scoped activity transcripts.
  Browser-borne requests are refused by default (empty `allowed_origins`);
  programmatic clients are unaffected.
- **The server supervises worker processes.** Restart policy, backoff
  windows, and kill-proving (drain + ESRCH) are configuration
  (`[worker_supervision]`, all six keys required, no invented defaults) —
  workers can be started, stopped, and restarted from the console.
- **AWL worker documents.** A `worker` file family that serves a queue:
  declared actions with bodies, captures, checker fixtures for the valid and
  invalid shapes, and `aion worker_awl serve` to run one.
- **Query arguments carry end to end** — through AWL, the SDK closure, and
  both store backends in one act.
- **Observability grows a run axis.** Activity transcripts are scoped to the
  run that produced them; a command's output streams into its activity
  transcript; `describe` joins live state — current step, liveness, notes,
  transcript tail.
- **beamr 0.17.0.** A JIT-compiled frame re-entering the interpreter now
  carries its native services instead of an empty registry — the
  replay-durability fix for long-running workflows.
- **New crate: `aion-mcp`** — the protocol layer behind the MCP endpoint.

## 0.12.0 — 2026-08-08

- **`aion new` scaffolds AWL by default.** A new `awl` template is the
  default: a bare `aion new <name>` emits a checked, runnable
  `<name>.awl` (typed input/output, three steps, one `when`/`otherwise`
  decision — no worker required), the shared dev `aion.toml`, a
  `.gitignore`, and a README with the exact check/fmt/deploy/run steps.
  Project names for the `awl` template additionally cannot be AWL reserved
  words (refused before anything is written). The five Gleam templates are
  unchanged and keep their ids (`hello-world`, `approval-flow`, `saga`,
  `dev-pipeline`, `agent`); select them with `--template`. The shared
  scaffold `.gitignore` now also ignores the default haematite store
  directory (`aion-data/`).

- **`POST /awl/workers/availability` answers service, not connection.** The
  endpoint (and with it the console's ship-and-run pre-flight) used to answer
  `available: connected_workers > 0`, which refused to start documents whose
  actions all carry declared `run` bodies — the server runs those itself and
  no worker ever connects, so the tutorial's own closing ship-and-run step
  refused a start that completes green. The answer is now derived from the
  same reachable contracts worker admission reads: actions with declared
  bodies never count against availability, and each worker-owed action is
  censused against the live fleet per `(action, node)` dispatch address, so a
  connected worker that does not advertise a demanded action — or sits on the
  wrong node for its pin — no longer vouches for it. The response gains two
  fields, `worker_actions` and `server_run_actions` (sorted), naming who
  serves what; existing consumers read only the pre-existing fields and are
  unaffected. The `scaffold_hint` now diagnoses which of four states refused
  the queue — nothing connected, a worker being refused at admission, a
  connected worker that does not advertise the demanded action (named), or
  workers on the wrong node for a pinned action (both named) — instead of
  answering every one with "no connected worker". The answer is per-queue: a
  document with several `worker` blocks must be checked once per queue, and
  it reads declaration-level node pins only — a call-site `node` override is
  visible to neither this answer nor admission.
- **One-command setup.** `scripts/setup.sh` takes a machine from nothing to a
  running server with the console, the assistant workflow deployed, and the
  assistant worker connected and serving it. It is safe to re-run: every step
  detects work already done and skips it. Run standalone it clones the
  repository itself — shallowly, on purpose, because the history carries about
  2 GiB of build artifacts committed by an old gate pipeline.
- **The setup script now works on a machine that has never run Aion.** It
  could not before: it created the Aion home but wrote no `config.toml`, and
  both surfaces it needs — the deploy gRPC service and the outbox's liminal
  worker listener — are dark when their config section is absent. Every proof
  had run on a home that already carried a config, so the one condition that
  mattered was never tested. It now writes a minimal config when none exists,
  enabling exactly those two switches and inventing no other value. The file
  and the script's own output both say what enabling deploy MEANS: deploying
  runs code on this server, which is why it ships off, and it listens on
  localhost only.
- **`{workspace_root}`: a declared action body can learn where to work.** A
  declared body runs with its environment cleared to `PATH` alone, so it
  cannot expand `~` or read a variable to find where session workspaces live,
  and the start contract deliberately does not carry the location — it is a
  property of the server, not of any one run. A command may now carry the
  literal placeholder `{workspace_root}`, which the server replaces with its
  own resolved root (`<AION_HOME>/clones`) before the command is parsed.
  Resolved once at startup and read everywhere, including the startup banner's
  `workspace_root` field; a resolution failure is carried rather than raised,
  so the server still boots and the refusal lands at the first
  placeholder-bearing dispatch. Two refusals guard the splice: every
  occurrence must stand alone as a whole unquoted argv word, and a root whose
  text would change the command's parsed shape is refused rather than spliced.
  Both are terminal. Documented in full in `docs/workers/DECLARED-COMMANDS.md`.
- **No caps on the assistant**, and the built-in agent worker reaches norn
  parity — the operator can name the model.
- **The ops console's Search view is withdrawn.** It was a navigable page at
  `/search`, in the nav, on `g e` and `/`, calling `POST /events/search` — a
  route the server has never registered. It 404'd by construction for every
  operator who reached it. Rather than build an endpoint to justify a link,
  the view is removed along with the client layer beneath it, and event search
  is recorded as a wanted capability with no implementation (roadmap RM-030).
  The `g e` and `/` keybindings are gone with it.
- **Documentation.** The `docs/` tree is organised into `authoring/`,
  `workers/`, `operations/`, `contracts/` and `vision/` with a front door at
  `docs/README.md`. `CONTRIBUTING.md` now carries the real gates
  (`scripts/battery.sh`) instead of a list of gitignored artifacts. Three
  wrong status codes are corrected in `docs/operations/errors.md`
  (`not_running` is 412, `sequence_conflict`'s gRPC code is `Aborted`, and the
  `unauthenticated`/`unavailable` row named codes that do not exist), and the
  failover demo's regression command — which had quietly become a no-op that
  printed green after the test moved from `#[ignore]` to runtime gating — now
  sets `AION_RUN_COSTLY_TESTS` and says what a green run without it proves.

## 0.11.0 — 2026-08-01

- **Every valid AWL document in the corpus now deploys on the direct
  compiler.** Four construct classes were held at the lowering staging gate
  and have landed, retiring the gate itself rather than shrinking it: value
  route payloads (`route out(<value>)`, coerced toward the outcome's declared
  type rather than passed through raw), `subflow` declarations, dependency
  parallel layers (an `after` naming more than one step, or two steps sharing
  a predecessor — not a keyword, which is why the flagship `dev_brief` hit it
  with no visibly parallel syntax), and substeps. A layer the SDK cannot
  dispatch concurrently degrades to written order and **says so** with a
  `degraded_parallel` marker rather than silently serialising. The claim is
  pinned as an invariant, not a count: `lowers_cover_and_golden` asserts the
  refused set is empty over the whole valid population, so a construct that
  legitimately refuses in future forces a knowing decision instead of a quiet
  regression.
- **`aion run` is gated end to end, and its wait can be bounded.** The
  one-motion pipeline (compile → deploy → start → await → report) was wired
  but untraversed by any test, and the await was an unbounded poll loop, so
  `aion run` against a non-terminating workflow hung forever. A live e2e gate
  now drives the real binary against a real server. New
  `aion run --timeout <duration>` bounds **the wait only**, written as an AWL
  duration literal (`30s`, `5m`, `3h`, `2d`) — the flag is parsed by aion-awl's
  own lexer, so it and a document's `timeout` declaration share one
  vocabulary. There is no default and no invented margin: absent the flag, the
  wait is bounded only by the document's own declared `timeout`, and a
  document declaring neither waits indefinitely. Reaching the bound exits 1
  with the workflow id, run id, and describe line — **the run continues on the
  server**.
- **A deployed AWL package carries the document it was compiled from.** The
  `.aion` archive gains two entry families rooted at the document's own
  directory: `awl/document/<filename>` (exactly one, the original filename
  verbatim) and `awl/schema/<document-relative path>` (every `schema(…)`
  import, nesting preserved), so a consumer can stage the pair and re-check
  the document exactly as deployed. Every deploy path carries it — CLI
  one-motion, server deploy, revision recompile. **Archived source is
  provenance and a declared non-property of package identity:** it is excluded
  from the content hash, so a carried document cannot re-version a deployed
  workflow or orphan pinned runs. A schema import resolving above the
  document's directory refuses at assembly; unknown `awl/`-prefixed entries
  and schemas without their document refuse at load.
- **The deployed surface shows its archived AWL source, read-only.**
  `GET /awl/deployed` lists every deployed version as the union of the
  engine's loaded catalog and the store's persisted archives, each row stating
  which side it came from; `GET /awl/deployed/{type}/{hash}` returns the
  archived document with four-state source honesty (available, absent, not
  persisted, unreadable with reason). The routes are deploy-guard authorized,
  mounted on `[deploy].enabled`, and reject every mutating method. Reading the
  deployed surface changes neither workspace nor store. The ops console gains
  a read-only deployed view at `/authoring?source=deployed`.
- **`aion worker agent` — an agent worker without writing a crate.**
  `aion worker agent <FILE.awl>` serves the agent harness compiled into the
  binary on the queue the document declares. Previously `aion worker` had
  exactly one subcommand (`shell`), every agent worker in `examples/` was a
  bespoke `main.rs`, and the binary built a default harness and immediately
  dropped it — so even Norn, the shipped default, was not servable out of the
  box. The document is the only possible source of the advertised schemas (an
  agent activity's output is decided by the agent, so there is no Rust type to
  derive one from). The queue is derived when the document declares exactly
  one `worker` block and selected with `--task-queue` when it declares
  several; a queue is served whole or the verb refuses by name. The worker
  dials `--liminal-address` candidates, not the global gRPC `--endpoint`.
  `--harness` selects among the adapters the binary was built with (the
  default build carries `norn` and `acp`, so it requires the flag), and a flag
  configuring a harness this invocation is not serving is refused rather than
  ignored. `--check` prints the exact advertised action surface, the resolved
  harness, and its intervention controls, then exits without connecting.
- **Aion provisions its own sensitive roots instead of asking the operator
  to.** The Aion home, the haematite data root, and the AWL authoring root are
  Aion's directories, so the server creates them owner-only (`0700`, from the
  `mkdir` itself, never a create-then-chmod window) and, when one already
  exists but grants group or world access and the server's own user owns it,
  tightens it in place and logs the change at INFO with the previous and
  applied modes. The previous behaviour — refusing startup with `chmod 700`
  remediation for a `~/.aion` that a conventional umask had left at `0755` — is
  gone; it made the operator hand-run a step the server could always take
  itself. No security property was relaxed to get there. A root Aion cannot
  make safe still refuses, now naming the path, the exact problem, and the
  remedy: a root owned by another uid (Aion will not modify another
  principal's directory), a symlinked root or component, a name occupied by a
  non-directory, and a filesystem that will not carry Unix modes (detected by
  re-reading the descriptor after the `fchmod` rather than assuming it took).
  The path-ambient backend's ancestor gate is untouched and still fails closed:
  a data root under a world-writable ancestor such as `/private/tmp` is refused
  outright, because tightening the leaf would not close that hole and the
  ancestor is not Aion's to change.

## 0.10.0 — 2026-07-23

Family convergence release: the whole stack moves to the current
ablative family in one act — **beamr 0.15.4 → 0.16.0** (caret absorbs
0.16.1), **haematite 0.5.0 → 0.7.0**, **liminal 0.3.0 → 0.4.0**
(liminal-protocol 0.3.1) — restoring single-version dependency
discipline across the tree. The resolved-graph duplicate census is
committed in-repo (`gate-logs/convergence-duplicate-census.txt`): one
beamr, one haematite, one liminal family. Verified GREEN end-to-end on
a second machine (1498 tests / 0 failed across 108 suites).

The 0.10.0 (breaking-slot) number follows from beamr being a public
dependency: `beamr::native::NativeFn` and beamr term types sit in
`aion-rs`/`aion-nif` public signatures, and beamr 0.16.0 is a breaking
release upstream.

- **beamr 0.16.0 uptake.** The retired `gleam_erlang_ffi` selector shadow
  is no longer registered (beamr removed `register_selector_bifs`; the
  selector family is served by loaded `gleam_erlang_ffi.beam` bytecode —
  aion-hosted Gleam code using process selectors must ship that bytecode,
  which modern `gleam_erlang` does). The last `spawn_link_dirty` call is
  migrated to `spawn_link` and the call-site dirty branch removed: dirty
  dispatch is a property of the registered native entry at 0.16.0, so the
  spawn API is uniform (equivalence pinned upstream in beamr's
  `dirty_scheduler` suite).
- **haematite 0.7.0 uptake.** `DatabaseConfig` literals move from the
  removed `sweep_interval` field to the new `executor_threads: Option<usize>`
  (`None` = automatic sizing, semantically identical to prior behavior).
  Aion uses none of the surfaces the 0.7.0 TreePolicy break changed
  (no tree/branch mutation calls, no `fork_at`, no `run_indexed_parallel`);
  existing v1 data directories open unchanged forever — no migration rides
  this release.
- **liminal 0.4.0 uptake.** Wire-compatible pin bump; no aion code
  changes. Closes the pre-drop-drain transcript-tail exposure class on
  clean worker stops (observability-channel publishes; durable outcomes
  were never affected).
- **Workspace-wide clippy 1.95 zero.** The full workspace passes
  `cargo clippy --workspace --all-targets -- -D warnings` on the 1.95
  toolchain with zero `#[allow]` additions; all changes are
  behavior-preserving (match-guard collapses, zipped counter loops,
  `is_ok_and`/`map_or`, whole-second `Duration` constructors, a named
  tungstenite `Callback` impl in test support).

## 0.9.2 — 2026-07-20

Out-of-box authoring, the central Aion home, security hardening of the
default-on studio surface, and honest event-stream recovery semantics.
Every change below survived independent adversarial review to a
zero-findings verdict (6 safety passes on the authoring/home branch,
5 correctness passes on the stream branch).

- **Aion home centralizes server configuration and state.** `AION_HOME` wins,
  otherwise the server uses `~/.aion`, without creating it during config reads.
  Server config discovery is now explicit `--config` > project-local
  `./aion.toml` > `<AION_HOME>/config.toml` > built-in defaults; every discovered
  file remains a loud typed failure when unreadable or invalid. Unconfigured
  haematite data and AWL studio roots are now `<AION_HOME>/data` and
  `<AION_HOME>/authoring`. If the old CWD-relative `aion-data` or
  `aion-authoring` directory exists, the corresponding unconfigured default
  keeps using it and emits an unambiguous startup migration warning naming both
  paths; explicitly configured paths never activate the guard. Sensitive roots
  are private by contract: on Unix Aion creates directories as `0700` and files
  as `0600` independent of umask, and an existing permissive Aion home fails
  startup with `chmod 700` remediation. Setting `AION_HOME` explicitly does not
  opt into a shared-home mode; shared Aion homes are unsupported. On non-Unix
  targets Aion does not claim to install or validate an owner-only ACL: startup
  fails closed for default home, data, authoring, and authoring-state roots.
  Operators must pre-provision private directories and explicitly configure
  `AION_HOME`, `store.data_dir`, and `authoring.workspace_dir`; accepted explicit
  roots emit a loud startup warning that ACL privacy was not verified.
- **AWL studio works out of the box.** A stock `aion server` exposes the full
  document, layout, check-backed editing, direct deploy, revision, run-status,
  and scaffold experience under the Aion-home authoring root, creating document
  and state directories on first write. Operators can still set
  `authoring.workspace_dir` / `AION_AUTHORING_WORKSPACE_DIR` explicitly. CN7 is
  unchanged: the separate Gleam loop stays unmounted until `gleam_path` is set.
- **The studio surface authenticates.** With `[auth]` enabled, every AWL
  document, layout, check, revision, run-status, and availability route
  requires a valid caller; document/layout/run-binding mutations require the
  deploy grant, and `worker_availability` is namespace-scoped. Auth-off stock
  behavior is unchanged. Workspace filesystem access is confined through held
  no-follow directory capabilities (symlinked legacy roots are never adopted;
  temp files are randomized `create_new`), and AWL `schema()` imports are
  confined to the workspace.
- **Path-ambient data-root protection (macOS).** Where the haematite backend's
  post-startup I/O is path-based rather than descriptor-authoritative, startup
  now requires every ancestor of the resolved data root to be owner-controlled
  — POSIX modes and Darwin extended ACLs both checked, ACE principals compared
  by UUID identity from a single owned ACL snapshot. New helper crate
  `aion-darwin-acl` carries the minimal Security-framework binding.
- **Honest event-stream contract and recovery.** The ops console speaks the
  server's real per-workflow `resume_from_seq` cursor (replay proven by
  integration test), runs one server-enforced subscription per socket, and
  never claims recovery it cannot prove: durable feeds recover via cursor
  replay with cursor-advance only after successful application; live-only
  feeds (filtered/firehose/cluster) degrade visibly with an explicit
  possible-gap state cleared only by a confirmed, quiesced, generation-guarded
  refetch. Cluster frames are contract-validated and each fresh snapshot is a
  new sequence epoch, so a server restart can no longer silently mute deltas.
  `WorkflowTimedOut` remains exported but unwired (no runtime emission
  exists); its build-or-drop decision is recorded as an open ruling.

## 0.9.1 — 2026-07-19

Dependency-alignment release — no aion surface changes.

- **haematite 0.4.1 → 0.5.0** and the **liminal family to 0.3.0**
  (`liminal-rs` 0.3.0, `liminal-sdk` 0.3.0, `liminal-server` 0.3.0) in one
  motion: `liminal-rs` 0.3.0's error types speak haematite 0.5.0, so the
  two pins are coupled and move together. haematite 0.5.0's kind-aware
  merge surface (its deliberate break) touches no API aion consumes.
- The embedded liminal worker front door passes `websocket: None` and
  `participant: None` for `liminal-server` 0.3.0's two new optional
  sections — documented byte-identical to the pre-0.3.0 build. Electing
  either is a future feature decision.

## 0.9.0 — 2026-07-18

Workspace version **0.9.0**. beamr pin now **0.15.4**. Three new crates
published for the first time this release: `aion-awl`, `aion-awl-lsp`,
`aion-awl-package`. ~660 commits since 0.8.0 (2026-06-29); 0.7.0 and 0.8.0
shipped without changelog entries — see the stub below.

### Language & authoring

- **AWL, a new first-party workflow language.** Full toolchain landed in
  four waves (AWL-1..AWL-4 — lexer, lossless canonical-printer parser,
  typechecker, Gleam-source emitter), then a rev-2 front-end rebuild with
  three Tom-ratified language rulings. Flow vocabulary landed in six waves:
  B1 ergonomics (`const`, raw strings, JSON literals, `schema of`), B2 flow
  shape (`subflow`, `distribute`/`sequence`, `collect`, `visits`), B3
  canvas projection (scoped substep graphs in the console), B4 lowering
  (distribute/sequence fan-out, subflows, settled collection), B5 true
  parallelism (multi-step `distribute` lowers to implicit child
  workflows), B6 direct-path distribute parity. `aion awl check` / `fmt` /
  `emit` give a typecheck-gated authoring loop from the CLI.
- **Types-first codec generation; schema-first removed.** The authored
  source of truth is now the Gleam types module `src/<package>_io.gleam`
  (types only); `aion generate` derives the codecs module
  (`src/<package>_codecs.gleam`), the EMITTED `schemas/*.json` artifacts,
  and the activity plumbing. **`aion codegen` is removed** — migrate by
  stripping the generated header/functions from the io module, deleting
  authored schemas, and running `aion generate .` (recipe in
  docs/guides/codegen.md). The test scaffold is now write-once.
  `examples/order-saga` is migrated and drift-gated in CI.
- **Direct-to-bytecode compilation** (BC-0..BC-3, new `aion-awl-package`
  crate): AWL documents compile straight to BEAM bytecode — MIR lowering,
  select/regalloc/assemble, and a programmatic package assembler producing
  the `.beam` container and an embedded SDK closure with zero `gleam`
  invocation on the product path. Typed codecs pin codec identity.
- **One-motion `aion deploy` / `aion run FILE.awl`** (B-3): a `.awl` file
  goes from source to a running workflow in one command over the
  direct-compile path, no intermediate Gleam project. Studio deploy uses
  the same path — deploy identity is derived from the AWL declaration
  itself, never the uploaded file name or scaffold template (fixes a
  cross-type clobber, pins regression #21); timeout edits now correctly
  redeploy as a new version.
- **AWL LSP** (new `aion-awl-lsp` crate): a thin server over the one real
  checker giving semantic hover and go-to-definition. Editor integrations:
  a Zed extension (Rust/Wasm, grammar-pinned) and a zero-hard-dependency
  Neovim plugin, both backed by a public `tree-sitter-awl` grammar.

### Runtime & engine

- **Engine auto-retry (#197).** Retryable activity failures are retried
  automatically at the dispatch seam, driven by a policy that was already
  on the SDK wire but unplumbed. `aion_flow` gains the matching config.
- **In-VM activity execution tier (Cut 3).** `InVm` activities now execute
  for real via a new engine NIF, `aion_flow_ffi:dispatch_activity_in_vm/4`,
  spawning the SDK-composed runner thunk as a LINKED child process of the
  workflow process (beamr 0.12.0's `Scheduler::spawn_link_closure`).
  Recorded-result semantics match remote activities by construction; a
  runner crash surfaces as a terminal `ActivityFailed` and the workflow
  process survives. New worker-free example: `examples/invm-demo`.
- **Remediation early-abort.** Identical-failure early-abort plus wave
  non-cascade on child failure, so one bad branch doesn't take the whole
  remediation wave down with it.
- **Durable timers and reopen.** Reopened runs get their durable timers
  rearmed again (#222/#223, previously lost on reopen); Failed/Cancelled
  workflows can be reopened end-to-end from the console (#180); a
  workflow-level durable Pause/Resume (#204) landed with a dispatch-hold
  enforced on the backpressure claim path.
- **Control-Plane, Phase 1 + 2.** A durable namespace registry
  (`NamespaceRecord`/`NamespaceStore`) minted on first `START`, backed by a
  quorum-CAS haematite implementation (`aion-store-haematite`); a
  namespace placement API with `Prefer` dispatch spill; per-namespace
  keyed backpressure at the outbox claim. Surfaced on the ops console.
- **kill-9 parked-timer failover.** A `#[ignore]`d repro (#148) pins the
  known post-kill-9 adopted-shard quorum-membership bug so it can't regress
  silently while it's worked; CI keystone adds a path-filtered kill-9 lane.

### Server & console

- **Ops console** (renamed from "dashboard", #156): always embedded by
  default (#154, the `embed-dashboard` feature is gone), running on the
  real wire contract (ts-rs-exported DTOs field-for-field), with
  browser-safe WS auth and config-driven CORS. A full design-system pass
  (one token vocabulary, terracotta/strategy-blue accent swap, motion kit,
  central keybinding registry, omni-palette); the **authoring studio**
  (CM6 editor, worker-backed tree-sitter highlighting, idle diagnostics,
  deploys-green strip, fmt-as-delta, flow-vocab canvas projection);
  **time-swimlanes** (time axis, true sub-lanes, continuous scrubber) and
  **nested-swimlanes** (recursive embedded child-run views with durable
  backfill); a durable attempt navigator, an assistant panel (session
  list/chat, `sendSignal`), a zustand draft-state layer, and a
  scrubber-continuity + Tab-indent UX pass.
- **NOI, a harness-agnostic observability layer.** A durable transcript
  spine (dedicated keyspace, single-writer sequencer) with mid-run
  intervention routing (operator → server → worker), a console
  `TranscriptPanel` with capability-gated `InterventionControls`, and a new
  `aion-integrations` `AgentHarness` SDK crate with two independent
  integrations (the norn adapter and `aion-integration-cli`). Durable
  attempt identity is now carried on activity lifecycle events (NOI-0).

### Workers & integrations

- **norn integration.** The norn adapter aligned to the `norn-driven/1`
  contract (stop envelope, protocol gate, tool cwd); a norn worker crate
  with developer and review-lens roles and a configured gate battery.
- **dev-brief, the general norn dev pipeline (#235).** A flagship
  AWL-authored example: developer runs the gate battery, concurrent
  adversarial review lenses judge the result, bounded fix cycles loop back,
  then land — with in-repo worktrees, reviewer read-only rooting,
  `{base_commit}` pinning, and a post-accept verify stage.
  (`dev_brief.awl`, `review_round.awl`.)
- **General-purpose worker crate**, a standalone worker honoring exact
  activity contracts without any language-specific SDK.
- `aion_flow` (hex) moved 0.5.0 → 0.6.0 → 0.7.0 across the range
  (`workflow.entrypoint` generated run adapter, AWL glue hoisted into the
  SDK, embedded closures regenerated against the current SDK).

### Dependencies

- **beamr 0.11.0 → 0.15.4.** Notable stops: 0.12.0 `spawn_link_closure`
  (the in-VM tier's foundation); 0.14.0 `loader/encode` (the `.beam`
  container writer behind the new `encode` feature, consumed by BC-3);
  0.15.2 routes `make_fun`/`put_map` near-full-heap allocation through the
  GC's `ensure_space` safety net and fixes an off-by-one DOWN-message heap
  reservation — closed a fatal `heap full` crash hit by aion's first
  direct-BEAM AWL child workflow in production; 0.15.3 adds a
  non-blocking, exactly-once exit-observation API (`take_exit_outcome`) and
  an uncompressed, byte-symmetric `.beam` `LitT` chunk (emitted bytes for
  any module carrying literals differ from 0.15.2's — expected, shifts
  package version identity once on next recompile); 0.15.4 adds an
  additive `BifRegistry::replace_existing` API.
- **liminal, promoted from an optional path-dependency spike (#13, feature-
  gated at 0.8.0) to the default workspace transport** — the ablative
  stack (haematite + liminal) is now the default build, libSQL/memory
  opt-in (#142/#143) — pinned through published 0.2.1 → 0.2.5 (`liminal-rs`
  0.2.5, `liminal-sdk` 0.2.4, `liminal-server` 0.2.5). 0.2.4's park-flip
  retires the embedded front door's per-connection busy-spin: idle
  connections now park on socket readiness instead of burning a slice
  every scheduler tick (measured ~140%+ CPU with zero connected workers
  before the fix); a push's reply deadline is now a pure wait quantum, no
  longer cancelled by an elapsed caller poll (G7).
- **haematite, promoted from a path dependency to a published crate**
  (crates.io, dropping the cross-repo path), pinned at 0.4.1 and now
  backing the durable namespace registry via `aion-store-haematite`'s
  quorum-CAS `NamespaceStore`.

## 0.7.0 / 0.8.0 — 2026-06

Both versions shipped without changelog entries. 0.7.0 (2026-06-22) bumped
beamr to 0.9.0 (cross-process local send + ref round-trip). 0.8.0
(2026-06-29) spiked and landed dispatch over liminal (#13, honest-ack
retry + dedup composition), started the haematite storage-swap design, and
rewrote the dashboard into a real-time ops console on the real wire
contract (browser WS auth, config-driven CORS). Their content is folded
into the history above; the AWL toolchain and everything else in this
document postdates both.

## 0.6.1 — 2026-06-13

### Engine (via beamr 0.6.1)

- **Workflow-process heap-reservation fix.** Pinned beamr 0.6.1, whose
  interpreter now runs `ensure_space` before the `put_list`/`put_tuple2`
  allocations, so a data-dependent burst of cons/tuple construction — e.g.
  decoding a large stage report inside workflow code — triggers GC and
  heap growth instead of dying with a fatal `heap full` that bypassed the
  collector. Surfaced by the brief_dev real-norn dogfood: a 12 KB scout
  report decoded in the workflow process crashed the run silently right
  after scout while a 10 KB one survived — a heap-reservation cliff, not
  genuine exhaustion (the process heap grows to ~1 MB). No aion code
  change; the bump adopts the VM fix. `aion_flow` is unchanged at 0.4.0.

### Notes

- Workflow code should stay thin (ADR-012): large activity results are
  best threaded as opaque payloads and decoded only by the consuming
  activity on the worker, where the heap is full-size and the work runs
  once rather than on every replay. The thin-workflow reshape is tracked
  as RM-023 and the standard library that makes it cheap as RM-022;
  observability so a crash never again looks like a hang as RM-024.

## 0.6.0 — 2026-06-13

### Engine

- **No default activity timeouts.** The engine-imposed 30s activity
  dispatch timeout is gone. Activity waits are unbounded and terminate
  only on completion, worker loss, server shutdown, or a workflow-level
  timeout the author chose. Agentic activities that run for an hour-plus
  are first-class.
- **Worker loss now delivers.** Losing the worker mid-activity fails the
  dispatch with a typed error instead of leaving the workflow parked
  forever (stream teardown previously only deregistered the worker).

### CLI

- **`aion new`** — scaffold a workflow project from four embedded
  templates: `hello_world`, `saga`, `approval_flow`, and `dev_pipeline`
  (the stacked-dev agentic pipeline, `--worker rust`). Scaffolds build,
  package, and pass their own test suites out of the box.
- **`aion codegen <dir>`** (and `--check`) — generate Gleam types and
  JSON codecs from JSON Schemas, with loud typed errors carrying file
  and RFC 6901 pointer context.
- Stale command hints in CLI output corrected for the unified `aion`
  binary.

### SDKs

- `aion_flow` **0.4.0** (hex) — `testing.mock_child`: typed child
  workflow doubles for unit-testing parent workflows without running
  their children. Scaffold templates now pin
  `aion_flow >= 0.4.0 and < 0.5.0`.
- `aion-rs` testing harness gains the matching in-process child doubles.
- Worker SDK logs session establishment; reconnect behavior hardened
  against server restarts.

### Examples

- `examples/stacked-dev` — the full agentic dev pipeline (provision →
  agent dev rounds → scoped verify → workspace gate → human review
  signal → land) with a standalone Rust activity worker, proven live
  against the real yg/norn/cargo/meridian CLIs end to end.
- Nested-workflow e2e suite: three-level chains, recursion,
  recovery-at-depth, and cancellation semantics pinned.

## 0.5.0 — 2026-06-11

- Unified `aion` binary (server runs as `aion server`; `aion-server` is
  lib-only).
- First release validated outside-in end to end (deploy → start →
  signal → query → recover).
