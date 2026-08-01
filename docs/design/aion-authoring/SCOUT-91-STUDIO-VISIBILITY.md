# SCOUT #91 — a CLI-deployed AWL workflow is invisible in the studio

Read-only Opus scout, 2026-08-01, tree c6202bda. Every claim carries file:line
evidence; nothing was executed (source-read only), no live-server probe was run.
Seat recommendation at the end; decision routed to Waffles before any build.

## Q1 — the deployed artifact does NOT retain the AWL source

- `crates/aion-awl-package/src/assemble.rs:80-83` doc comment says source is
  deliberately not archived because "the format-v1 loader rejects unknown
  entries, so source provenance is deferred to a loader-tolerance follow-up."
- 🔴 **That stated blocker is FALSE at the bytes.** The loader SKIPS unknown
  entries, it does not reject them: `crates/aion-package/src/package.rs:330-366`
  iterates archive entries and silently ignores anything that is not
  `manifest.json`, `contract.json`, `beam/*.beam`, or `src/*.gleam`.
- `assemble.rs:129-131` uses the no-source `PackageBuilder::new`; the
  source-bearing `PackageBuilder::with_source` exists
  (`crates/aion-package/src/builder.rs:45-62`) and is used only by the Gleam
  project path (`crates/aion-package/src/project/assemble.rs:133-137`, gated on
  `config.include_source`). `compiled.sidecar_bytes` (`.gleam_types`, produced
  at `crates/aion-awl/src/compile.rs:216,239`) is dropped by the assembler.
- Source, if embedded, survives everything: identity excludes it
  (`crates/aion-package/src/hash.rs:26-27`, pinned at `builder.rs:346`);
  archives persist whole (`crates/aion/src/loader/persistence.rs:38-70`);
  `Package::to_archive_bytes` (`package.rs:265-273`) re-serialises through
  `with_source`, so `src/` entries round-trip. Read-back APIs exist:
  `Package::source()` (`package.rs:123-125`), `EventStore::list_packages()`
  (`crates/aion-store/src/package.rs:79`), `Engine::store()` public
  (`crates/aion/src/engine/api.rs:163`).
- The engine keeps only `StagedLoad` facts in memory
  (`crates/aion/src/loader/load.rs:151-245`); read-back is a store round-trip.

## Q2 — what serves the studio's world-view

- Endpoints: `crates/aion-server/src/api/http/router.rs:174-192`; handlers
  `crates/aion-server/src/api/http/awl.rs:59-109`.
- `GET /awl/documents` is a directory walk, nothing more:
  `crates/aion-server/src/awl/documents.rs:70-94` (`ConfinedDir.list_awl()`);
  a missing workspace returns `[]` (`documents.rs:74-77`).
- Workspace root: `awl.rs:272-281` reads `authoring.workspace_dir`; resolution
  at `crates/aion-server/src/config/resolution.rs:288-320` (legacy CWD
  `aion-authoring` wins only if it exists as a real non-symlink dir; explicit
  AION_HOME disqualifies legacy; `LegacyNotice` reported). Constants at
  `config/defaults.rs:170-183`.
- ✅ **The `~/.aion/authoring` fallback is DELIBERATE** — refuting the task's
  suspicion: doc-commented (`defaults.rs:170-177`, `sections.rs:597-626`),
  unit-pinned (`config/load.rs:1378-1394`), e2e-pinned
  (`tests/authoring_out_of_box_e2e.rs:117-147,233-271`). The studio is always
  mounted; only the Gleam authoring loop is dark by default
  (`router.rs:167-176`).
- Console: `apps/aion-ops-console/src/features/authoring/components/AuthoringView.tsx:17-18,63`
  — `GET /awl/documents` is the SOLE source of the document list; the canvas
  projection rides the `/awl/check` response
  (`components/EditorPane.tsx:124`, facade
  `features/authoring/lib/facade.ts:156,218-240`). **The canvas has no other
  input** — a workflow absent from the workspace directory is invisible, full
  stop.
- 🔴 **Adjacent defect (not in the brief):**
  `features/authoring/components/DocumentList.tsx:122-125` renders a HARDCODED
  `deploys green` badge with a healthy status dot on every listed document,
  unconditionally — no check result feeds it.

## Q3 — what the server knows about a deployed AWL workflow

- Read model `WorkflowVersionInfo` (`crates/aion/src/loader/version_info.rs:8-23`),
  surfaced by `GET /deploy/versions`
  (`api/handlers/deploy.rs:166-182`), already consumed by the console Actions
  panel (`features/actions/hooks/useDeployPackage.ts:25-40`).
- Deeper facts need a store round-trip: manifest schemas/activities/timeout
  (`crates/aion-package/src/manifest.rs`), `.v5` contract
  (`crates/aion-package/src/contract.rs:25-49`).
- 🔴 **There is NO AWL-origin marker anywhere.** Only inferential structural
  signals: fixed ABI `entry_function == "run"` (`assemble.rs:117`); beam set =
  embedded SDK closure (`crates/aion-awl-package/src/bundle.rs:20-24`) + one
  entry module (`assemble.rs:109-113`); `src/` empty for AWL. Pattern-matching,
  not a declaration.

## Q4 — the deploy path's seam

- **Deploy is client-side compile, server-side load; the server never sees
  `.awl` source on this path.** `crates/aion-cli/src/main.rs:555-573` →
  `one_motion::package_file` → `deploy::deploy_bytes`; wire is bytes only
  (`crates/aion-proto-generated/proto/deploy.proto:20-22`); server decodes at
  `api/handlers/deploy.rs:57-102`.
- A server-side registration hook could only live at
  `api/handlers/deploy.rs:70-101` and can only write source it has — i.e. it
  is conditional on Q1 changing first. Writers are ready-made:
  `awl/documents.rs:168 write()`, `awl/revisions.rs:78 record_deployment()`.
- The client-side seam: `main.rs:555-573` holds both the source path and a
  server connection; it could `PUT /awl/documents/<name>.awl` alongside
  `deploy_bytes` with no format change.
- **The studio's own deploy path already does all of this**
  (`crates/aion-server/src/awl/run_loop.rs:154-237`): saved document →
  content-addressed revision → compile → load → `DeploymentRecord`. It is the
  only producer of deployment records and is unreachable from `aion deploy`.

## Q5 — fix shapes

Revision machinery any shape touches: `crates/aion-server/src/awl/revisions.rs`
— state under `<workspace>/.aion-authoring/` (0700/0600, symlink-confined,
`revisions.rs:9-11,45-55,212-220,268-278,310-325`); `DeploymentRecord`
(`:20-30`); drift = hash mismatch (`:139-147`).

**(a1) client writes the doc into the workspace** — `PUT /awl/documents/…`
alongside deploy at `main.rs:555-573`; no server/format change; endpoint
already behind the deploy grant (`api/http/awl.rs:103`). Risks: partial state
(workspace write can fail after successful load — must be reported, never
swallowed); `PUT` is a blind overwrite (`documents.rs:168-190`) — deploying
`foo.awl` from a scratch dir silently clobbers a different `foo.awl` the
operator was editing; there is no create_new guard and no ownership concept;
deploy-time writes bypass the studio's naming rule (`documents.rs:264-279`
enforced only in `create`, not `write` `:192-209`). Schema-import documents
land in the list and check RED unless the sibling schema files are copied too
(staging: `awl/handlers.rs:220-265,149-200`).

**(a2) server writes the workspace** — requires (c) first; server has no source.

**(b) studio lists deployed workflows from server truth** — feasible for a
LIST; **not for a canvas**: the projection consumes a parsed `Document`
(`awl/projection/build.rs:24`, `awl/studio_projection.rs:47`); no source → no
canvas. Would join `/deploy/versions` catalog (route_active lives there) with
store `list_packages()`. No AWL/Gleam filter exists (Q3). Note the Actions
panel already lists versions — the defect as the operator hit it is "the
CANVAS can't show what I deployed," not "the server can't list it."

**(c) embed the AWL source in the archive** — the enabler for a2 and the only
path to a canvas for (b). Cheaper than the false `assemble.rs:80-83` comment
claims. Two forms: reuse `src/` via `with_source` (**trap 1:** `with_source`
overwrites the contract with `PackageContract::from_manifest`
(`builder.rs:51`) — `.with_contract` must be re-applied after or the durable
contract silently downgrades; **trap 2:** `src/` entries are keyed
`<name>.gleam` (`builder.rs:172`) — AWL under a `.gleam` name lies about
content type and destroys the src-empty signal), or a new `awl/` prefix
(touches `package.rs:330-366`, `builder.rs:149-178`, `package.rs:265-273`,
`Package` fields/PartialEq; ⚠️ UNVERIFIED-but-inferred: the re-serialiser
rebuilds from manifest/beams/source only, so an unknown entry is DROPPED on
persistence re-write unless `Package` gains a real field — the new-prefix
variant requires the field, not just tolerant reading). Old binaries reading
new archives silently lose source — flag under the no-backwards-compat policy
rather than assume. (c) alone still leaves `schema import` documents checking
red unless imported schema files are archived too (compile flattens schemas
into the manifest; the import LINE survives in source).

**(d) route `aion deploy <file>.awl` through the studio pipeline** — CLI PUTs
the document then `POST /awl/deploy {path, content_hash}`; one pipeline
instead of two; drift/revision/run-status surfaces work for CLI deploys for
free. Costs: compile moves client→server (CLI currently validates locally
before any connection, `main.rs:281-283`); `/awl/deploy` is HTTP while `aion
deploy` is gRPC (`crates/aion-cli/src/deploy.rs:12,35-42`) — second transport;
the guided path's hash-match refusal (`run_loop.rs:164-169`) becomes a new
CLI failure mode. ⚠️ Seat note: the scout's table marked schema imports
"workspace already holds them" for (d) — that presumes the document already
lives in the workspace; a CLI deploy from a scratch directory would need to
PUT the schema siblings too, same as (a1).

## Constraining facts (scout's summary, seat-adopted)

1. The canvas requires AWL source text; nothing else reconstructs it.
2. The archive has no source today, and the stated reason is false at the bytes.
3. There is no AWL-origin marker; any AWL-only filter is a heuristic.
4. The gap is TWO PIPELINES, not missing server information: the studio's
   deploy path produces every artifact the studio needs; the CLI path produces
   none of them.
5. `PUT /awl/documents/<path>` is an unguarded overwrite — any auto-write
   shape needs a collision answer first.

## Not verified

- No live probe of the running server or ~/.aion/authoring on this box.
- Nothing executed; all claims are source-reads.
- The unknown-entry-dropped-on-rewrite constraint on (c) is inferred, not run.

## SEAT RECOMMENDATION (Vesper, routed to Waffles for ruling)

**(c) with a first-class `awl/` archive entry (a real `Package` field so it
survives re-serialisation; imported schema files archived alongside), then (b)
on top: a read-only "deployed" surface in the studio whose canvas renders from
the archived source.** Deploy never writes into the operator's editing
workspace — the clobber/ownership/partial-state hazards of (a1)/(a2) never
exist, and the archived source doubles as the AUTHORITATIVE AWL-origin marker
(identity from content, not a label — closing the Q3 gap for free). Fix the
false `assemble.rs:80-83` comment in the same motion. (d) stays on the table
as a later unification but changes CLI behaviour (server-side compile, second
transport, new refusal mode) and is not needed to close the operator's actual
complaint. The permissive branch (deploy auto-writing the operator's
workspace) is NOT recommended and is the branch that would most need an
explicit ruling if ever revisited.

Adjacent defect filed separately: the hardcoded `deploys green` badge
(`DocumentList.tsx:122-125`) — a fabricated claim on every listed document.
