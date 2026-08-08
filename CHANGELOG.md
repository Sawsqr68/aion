# Changelog

All aion crates share one workspace version; entries below cover the
whole stack (crates.io) plus the `aion_flow` Gleam SDK (hex) where noted.

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
