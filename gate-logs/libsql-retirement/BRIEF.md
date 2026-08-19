# BRIEF — libsql backend retirement: haematite becomes aion's store

TICKET: **aion#35** (minted at dispatch 2026-08-17 per Amendment 1 —
https://github.com/ablative-io/aion/issues/35). Closes-by-subtraction
#85 and #229.

DISPATCHED: 2026-08-17 ~14:4x+1000 at Vesper's seat on Waffles' GO
(02:04Z DM — the lane no longer queues behind the mailbox flight; Tom's
concurrency order 01:53Z). Landing stays at Waffles' seat. Baseline:
aion main 00064c41c, workspace version 0.16.0.

## DISPATCH-TIME AMENDMENTS (2026-08-17, LAW — read before the charter)

A. **R0 DISCHARGED — the leak premise is STRUCK from this lane** (docs
   brief AMENDMENT 2026-08-17b at commit 5033b30, folded verbatim in
   substance): malloc_history on the instrumented running server proved
   the aion#32 memory accumulator is HAEMATITE (`LeafNode::deserialise`
   via `materialize_all_shards` and `WalRecovery::recover_path`); libsql
   appears in NO retained stack. The leak work is a separate haematite
   lane on aion#32 and is NOT a requirement, acceptance clause, or
   expectation of this brief. Any earlier "footprint stays FLAT"
   acceptance gates that haematite lane, not this one. **This removal
   must never be represented as fixing the memory defect** — not in
   commit messages, changelog entries, docs, or the delivery report.
   The retirement proceeds on the #85 double-close merits (the "Why"
   below stands) plus Tom's re-affirmed order (2026-08-17 01:50Z).

B. **Population check DONE (Amendment 2, performed at dispatch,
   Vesper's own hand + Waffles' independent lsof)**: live server
   pid 74635 — lsof shows **0** libsql/.sqlite/.db handles and **66**
   open handles under `stack/aion/aion-data` (haematite shards,
   writer.lock, per-shard WALs); latest recorded startup banner says
   `store_backend=haematite`; zero sqlite/db files anywhere under
   `~/.aion`. libsql is not in the serving path at all. Consequence:
   expect pure code/dependency surgery with NO live-data migration
   step. If your census (R-census below) finds a libsql file store on
   disk anywhere, its disposition line must say why the serving path
   never opened it.

C. **Census evidence commits FIRST.** Before any removal commit,
   commit the census: `cargo tree -i` on the libsql crates plus a text
   sweep (with a planted canary proving the instrument sees) across
   Cargo.tomls, source, docs, workflows, and config surfaces — every
   hit listed with its disposition. The final absence claim must cite
   this committed census.

D. **Published-consumer boundary (docs brief R4, reconciled with
   ratified F1)**: F1's ratified answer stands — v1 refuses with
   remedy, NO migration tool, and the 0.16 binary must not link
   libsql. For crates.io consumers on 0.14/0.15 with a libsql store,
   the 0.16 release notes must state plainly: this build does not read
   libsql database files; keep a 0.15 binary to access old data. If
   you find that honest statement impossible to write truthfully,
   STOP and report — a STOP-REPORT is a pass, not a failure.

STATUS: RATIFIED 2026-08-15 09:09:16Z by Waffles (lane response 10283385
under arc post acb3a76b) — all three forks per the recommendations below,
plus THREE AMENDMENTS folded verbatim as law:

  AMENDMENT 1 — the LANE NAME is canonical over the colliding number: this
  lane is "libsql-retirement"; the real ticket number is minted at dispatch
  and recorded here then. No artifact in this lane cites "#203".
  AMENDMENT 2 — F1's population check measures what the LIVE server LOADED,
  not the default-path file: the check reads the running server's resolved
  config (the store backend the live pid actually constructed), not merely
  ~/.aion/config.toml on disk. A config file is a claim; the loaded backend
  is the fact.
  AMENDMENT 3 — the expected-red census updates in the SAME landing motion:
  any battery/gate record listing #229 or #85 as expected/classified reds
  (release runbooks, known-issues entries, attestation templates) is swept
  and updated in this lane's landing commit — a subtraction that leaves its
  own expected-failure entries behind would teach the next battery to
  excuse a red that can no longer occur.

DIRECTIVE: Tom 06:08Z (as relayed by Waffles 09:03Z; scope confirmed at
ratification).
QUEUE POSITION (ratified shelf order): #246 → #247 → libsql-retirement, all
behind Waffles' walls. DO NOT FIRE until #247 lands and his walls clear.

## What this is

aion carries two durable store backends: haematite (the default) and libsql
(the alternative). Tom's directive retires the libsql backend entirely —
haematite becomes aion's store, full stop. This is a subtraction lane under
the NO BACKWARDS COMPATIBILITY law: replace, don't add alongside; no compat
shims, no deprecation markers, no zombie code.

## Mechanism verified at bytes (2026-08-15, tree at f39825564/main)

The retirement surface, each edge verified by grep/read at my hand:

1. **The crate**: `crates/aion-store-libsql/` — store.rs, append/, read/,
   outbox.rs (+outbox_tests.rs), pool.rs, connection.rs, schema.rs, timer.rs,
   visibility.rs, namespace.rs, package.rs, worker_deployment.rs, config.rs,
   error.rs. Own test suites: `tests/{conformance,outbox_hold,persistence,
   replica_sync}.rs`.
2. **Workspace edges**: root `Cargo.toml:9` workspace member; root
   `Cargo.toml:75` `libsql = "0.9"` workspace dep; `crates/aion-server/
   Cargo.toml:25` optional dep behind feature `libsql-backend` (:182), :132
   dev-dependency (integration tests), :152 direct `libsql` workspace dep;
   `crates/aion-cli/Cargo.toml:140` re-exported feature (the documented
   "lightweight opt-out" path `--no-default-features --features
   libsql-backend`, comments at :77/:132/:138); `crates/aion/Cargo.toml:46`
   dev-dependency.
3. **Selection surface**: `StoreBackend::LibSql` variant
   (`aion-server/src/config/sections.rs:55-62`); `store.url` field
   (sections.rs:71); `AION_STORE_URL` env var (config/env.rs:31-46, which
   force-flips backend to LibSql at :46); `--store-url` CLI flag semantics
   (config/load.rs:402-409, same flip); validation arms load.rs:449-452
   (url required for libsql) and :472 (cluster refused off haematite).
   The durable DEFAULT is already haematite (load.rs:1193-1196 pins it).
4. **Construction arms**: `aion-server/src/state.rs:9-10` cfg-gated
   `LibSqlStore` import + construction arms; run.rs:531/:1351; cluster.rs;
   observability/instrumented_store.rs. The outbox dispatcher's
   shared-connection design notes (state.rs:75/:993/:1916/:1962/:3078)
   describe the libsql leg specifically.
5. **Outbox portability — the load-bearing fact**: `OutboxStore` is a trait in
   `aion-store/src/outbox.rs:384`; **haematite implements it itself**
   (`aion-store-haematite/src/store.rs:1919`). The outbox does NOT depend on
   libsql; libsql's `impl OutboxStore` (aion-store-libsql/src/store.rs:377) is
   a parallel leg. The memory backend has no outbox (by design, state.rs:75).
6. **Test population using LibSqlStore** (18 files):
   aion-server src tests (state.rs, run.rs, cluster.rs,
   instrumented_store.rs, worker_deployments_tests.rs, outbox_dispatcher.rs)
   + integration tests `deploy_restart_e2e, lsub3_failover_e2e,
   worker_deployment_spawn_fence, namespace_restart,
   graceful_shutdown_park_e2e, lsub1_xnode_dispatch_e2e,
   outbox_delivery_contract_e2e, run_server_outbox_support/helpers,
   run_server_outbox_e2e, outbox_transport_e2e` + `aion/tests/outbox_e2e.rs`
   + fixture `update_check/fixtures/aion-cli-index.jsonl`.
7. **Docs**: docs/GETTING-STARTED.md:863-882 documents `backend = "libsql"` +
   `url` as the durable config; design docs reference it (DESIGN-OVERVIEW,
   COMPONENT-ARCHITECTURE, outbox briefs, others); root CLAUDE.md names
   `aion-store-libsql` as "the alternative durable libSQL backend".
8. **Known defects that die by subtraction**: #85 (libsql 0.9.30 double-close,
   proven 2026-08-13 unconditional — every connection closed twice) and #229
   (slim libsql build not warning-clean, 29 sites). #186's shipped artifact
   (the slim build) is redefined by fork F2.
9. **Wire**: no `.proto` in aion-proto references store backend selection
   (builder re-verifies at bytes). **No wire is touched — no skew floors
   apply.** If the builder finds ANY wire edge during implementation, STOP
   and report; do not improvise a skew story.

## Fix shape

Whole-hog subtraction plus a truthful boundary:

- **Delete** `crates/aion-store-libsql` (workspace member, the crate, its test
  suites), the `libsql` workspace dep, the `libsql-backend` features in
  aion-server and aion-cli, all cfg-gated construction arms, the server's
  direct `libsql` dep, and both dev-dependency edges.
- **Delete** `StoreBackend::LibSql`, `store.url`, `AION_STORE_URL`, and the
  `--store-url` force-flip semantics — not the flags/keys silently ignored,
  but REFUSED (R3).
- **Migrate** every test in edge 6 to haematite (durable semantics needed:
  restart/failover/outbox e2es) or memory (where the store is pure fixture)
  — per-test verdict, R4.
- **Sweep** docs by the claim's vocabulary (libsql/libSQL/store.url/
  AION_STORE_URL/--store-url/slim build), each hit dispositioned by its own
  verdict, R6.

## Requirements

- **R1 — the crate is gone.** `aion-store-libsql` absent from the workspace;
  `rg -i libsql crates/ --type rust` returns zero production hits (doc-comment
  hits die with their subjects). The next release publishes 21 crates, not 22
  — the release runbook/count assertion is updated in the same motion.
- **R2 — haematite is the one durable backend.** `StoreBackend` =
  {Memory, Haematite}. The bare-default config still resolves Haematite
  (existing pin load.rs:1193-1196 keeps its meaning; do not weaken it).
- **R3 — the boundary refuses with a remedy (the #145 law).** A config file
  with `backend = "libsql"`, a config with `store.url` set, env
  `AION_STORE_URL`, or the `--store-url` flag each produce a TYPED refusal at
  load time that (a) names what was found, (b) names the remedy
  (`backend = "haematite"` + `store.data_dir`), and (c) states plainly that
  existing libsql database files are not read by this build. No silent
  ignore, no auto-migration, no fallback (the safety layer nobody audits —
  a fallback here would run a workflow against an empty store).
- **R4 — migrated tests keep their teeth.** Every edge-6 test compiles and
  passes on its new backend AND still proves its original subject: for each
  test whose defect-class is reachable (restart durability, failover,
  outbox delivery), the builder demonstrates at least one representative
  mutation/regression still turns it red post-migration (mutation on the
  test's path; the literal prior defect where one exists in history). A test
  that only exercised libsql-specific mechanics (replica_sync, outbox_hold,
  persistence, conformance in the crate's own tests/) dies with the crate —
  enumerated in the lane record with per-file verdicts, not waved as a batch.
- **R5 — store conformance stays two-legged.** aion-store's backend
  conformance battery runs memory + haematite after the subtraction; the
  libsql leg's removal is named in the lane record, and no conformance
  assertion silently loses its only implementor.
- **R6 — docs tell one truth.** GETTING-STARTED's `[store]` reference teaches
  haematite (+memory for dev); the sweep covers docs/ + CLAUDE.md + Cargo.toml
  comment blocks + assistant-facing resources; each hit gets its own verdict
  (historical design records may keep their history — they are records, not
  claims about the present; anything normative changes).
- **R7 — no invented constants.** The subtraction introduces zero new
  defaults, caps, or tuning values. Haematite's existing configuration
  surface is not touched.

## Red-first legs

- **Red A (refusal tests, committed red)**: the four R3 refusal tests
  (config-key, url-key, env, flag) are written and committed FAILING against
  the pre-subtraction tree (which accepts all four inputs today — load.rs
  accepts `backend = "libsql"` at :731-759). The fix commit turns them green.
  Control: a `backend = "haematite"` + data_dir config still loads clean in
  the same suite (the refusal must not over-fire).
- **Red B (single-gate mutation)**: all four doors must route through ONE
  refusal site on the load path. Mutation removing that site turns all four
  tests red (proves the tests sit on the path and the refusal isn't
  duplicated truth — one truth in two places has drifted).
- **Red C (migration validity, per R4)**: representative per-class mutations
  post-migration, using literal prior defect text where history carries one.

## Forks for ratification (recommendation first)

- **F1 — encounter story for existing libsql data.** RATIFIED as recommended:
  v1 refuses with remedy (R3c wording), NO migration tool. Population check
  owed BEFORE build, per Amendment 2: the check measures what the LIVE server
  LOADED — the store backend the running pid actually constructed (e.g. the
  server's own startup log/config disclosure or /build-adjacent disclosure),
  not merely the default-path config file on disk. Every tracked
  example/fixture config is also swept. A claim about "nobody runs libsql"
  carries its population or it carries nothing. If a real libsql deployment
  surfaces, this fork reopens.
- **F2 — what replaces the lightweight opt-out build** (#186's
  `--no-default-features --features libsql-backend` path). RATIFIED as
  recommended: retire the opt-out entirely — haematite compiles in every
  server build; `memory` remains the dev backend at runtime, not at link
  time.
- **F3 — crates.io disposition.** RATIFIED as recommended: stop publishing
  `aion-store-libsql` from the next cut (count 22→21); no yank of published
  versions (they are history; yanking breaks no-one-should-depend claims we
  cannot measure).

Per Amendment 3, R6's sweep explicitly includes the EXPECTED-RED CENSUS:
every battery/gate/runbook/known-issues record that lists #229 or #85 as an
expected or classified red is updated in the landing commit itself — the
subtraction may not leave behind an expected-failure entry that would teach
a future battery to excuse a red that can no longer occur.

## Battery (scope follows the closure)

Full `cargo test --workspace` + the aion-server integration battery (the
migrated e2es are the subject), clippy ×2 (`--workspace` AND per-crate — the
preserve_order resolver split, compaction-11), `cargo fmt` tree-wide, doc
gates (doc_pointer_gate + any gate watching GETTING-STARTED). Quiet box per
the standing walls; exit-code manifest per the attestation protocol.

## Out of scope

- Haematite feature work of any kind (this lane only subtracts).
- #245 supervision design, #246/#247 (their own lanes, ahead in queue).
- The memory backend (unchanged, still no outbox).
- beamr, liminal, SDKs (no SDK touches store selection — builder re-verifies
  with one grep and reports).

## Dispatch mechanics (pre-staged at ratification, NOT before)

Fresh norn session (the K9 lesson: never resume a near-full session),
self-contained brief = this file verbatim, worktree off main created at
dispatch time, xhigh, commit-as-you-go, never push. Envelope + stderr in this
directory. Fire order per queue position above.
