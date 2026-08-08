# Aion Authoring DX — Operator & Agent Guide

> **Audience.** A Claude Opus agent (or an engineer) operating or extending Aion's
> authoring developer-experience surface. Every command, flag, route, request
> field, config key, and invariant below is verified against the source at the
> commit named in Provenance. Where this guide and a code comment disagree, the
> code wins — and this guide follows the code.
>
> **Scope.** The authoring DX cluster `aion-authoring` (briefs WA-001..WA-007),
> with this guide centred on the three most recently landed:
> **WA-002** (`aion dev` + dev server), **WA-006** (`aion new agent`),
> **WA-007** (`aion check --deterministic`, generated test scaffolds, `aion input`).
> The earlier WA-001/003/004/005 commands (`aion generate` — which replaced the removed `aion codegen`, `aion package`,
> `aion deploy`, `aion inspect`, the `/authoring/*` compile API) are referenced where
> a runbook needs them and marked *prior-landed*.

## Provenance

- **Branch/commit:** `main` @ `91b8acbc` (pushed). Built/verified 2026-06-21.
  **Everything below describes the surface as of that date**, except where a
  later correction is marked inline. The build and test observations in §10 and
  §11 are that day's readings, not standing facts — re-run them before relying
  on them.
- **Briefs:** `docs/design/aion-authoring/briefs/WA-002.json`, `WA-006.json`, `WA-007.json`.
- **Design:** `docs/design/aion-authoring/{DESIGN,CHECKLIST,USER-STORIES}.md`; ADRs in `docs/design/DECISIONS.md`.
- **Invariants & standards:** `/Users/tom/Developer/ablative/stack/aion/CLAUDE.md`.

---

## 1. Mental model (do not violate)

Within a **Gleam** workflow package, the typed Gleam workflow module is the single
source of truth (ADR-014): no second schema inside the package, and the DX commands
below wrap an author loop around that one module.

> **This is not the only authoring surface.** AWL (`.awl`) is Aion's other
> first-class surface and has shipped: its own parser, checker, formatter,
> schema derivation, language server, and direct compiler, driven by
> `aion awl check|fmt|emit|scaffold|schema|lsp`, `aion deploy <file.awl>`, and
> `aion run <file.awl> --input <json>`. An AWL document carries its own types
> and contracts and is compiled directly — it has no `aion generate` step and
> no `workflow.toml`. See [`docs/authoring/AWL.md`](AWL.md). The `aion dev`,
> `aion new`, `aion generate`, `aion check --deterministic`, and `aion input`
> commands documented in this guide are the **Gleam** package loop.

Five load-bearing invariants constrain everything here. Breaking one is a correctness
bug, not a style nit:

1. **Type-erased events.** The engine/store carry opaque `Payload` bytes; only the Gleam SDK knows types.
2. **Determinism boundary.** Workflow code is re-executed on replay and must be a pure function of recorded history. Time = `workflow.now()`; entropy = `workflow.random()`/`workflow.random_int()`. **No wall clock, no entropy source** in workflow-visible paths. This is exactly what `aion check --deterministic` enforces statically.
3. **Single writer per workflow.** Exactly one `Recorder` appends per workflow. **Never call `EventStore::append` directly.** The dev surface honours this — it drives the real engine, never the store's append path.
4. **Status is a projection** of history — the seven variants of `WorkflowStatus` (`crates/aion-core/src/status.rs`) are `Running`, `Completed`, `Failed`, `Cancelled`, `TimedOut`, `ContinuedAsNew`, and `Paused`. `Paused` is **non-terminal**: it is a `WorkflowPaused` marker with no later `WorkflowResumed`, and the run can still complete, fail, or be cancelled. Residency (Resident/Suspended) is orthogonal — there is no `Suspended` status, and a suspended run waiting on a signal is still `Running`.
5. **Content-hash module namespacing.** Each package version is an immutable module `logical_name$hash`. This is what lets `aion dev` hot-load a new version while in-flight runs stay pinned to theirs.

ADRs the DX surface encodes — an agent extending this code **must** uphold them:

- **ADR-001 — no invented defaults/limits/caps.** Configurable values come from the operator/author or are omitted. Examples enforced here: `aion dev --gleam-path` is **required** (no default binary); the watch loop has **no default poll interval**; the agent scaffold's review deadline is a **required input field**; `aion input` skeletons carry **no default values**; `[authoring].project_root` is **required when `gleam_path` is set**. The `[deploy].max_*_bytes` ceilings are the deliberate exception and are **optional**: they are security ceilings rather than behavioural values, so enabling deploy fills them with conservative defaults (64 MiB / 256 MiB) rather than refusing to boot for want of a knob — see §6 and `crates/aion-server/src/config/defaults.rs`.
- **ADR-002 — no backwards-compat/zombie/dead code.** Replace and delete; never leave a dormant alternative.
- **ADR-003 — no default deadlines.** Agent step/approval deadlines are author-chosen.
- **ADR-011 — agent runtime stays worker-side.** The `aion new agent` scaffold bundles no LLM/agent runtime; the agent step is a parameterised, worker-served activity.
- **CN4 — no mock-only engine.** The dev server has no dev-only execution path that diverges from production; mocking decorates the production dispatcher.

---

## 2. Repository map

| Surface | Location |
|---|---|
| CLI entry (`Command`/`ClientCommand` enums + dispatch) | `crates/aion-cli/src/main.rs` |
| `aion dev` watch loop | `crates/aion-cli/src/dev/{mod,args,session,pipeline,watch}.rs` |
| `aion new` scaffolder + templates | `crates/aion-cli/src/new/{scaffold,template,agent}.rs`, `crates/aion-cli/templates/awl/**`, `crates/aion-cli/templates/agent/**` |
| `aion check --deterministic` driver | `crates/aion-cli/src/check_deterministic.rs` |
| `aion input` driver | `crates/aion-cli/src/input.rs` |
| `aion generate` (codegen + test scaffold wiring) | `crates/aion-cli/src/generate.rs` |
| Determinism analyzer + workflow facts | `crates/aion-package/src/structure/{determinism,facts}.rs` |
| Test-scaffold & input-skeleton generators | `crates/aion-package/src/codegen/{test_scaffold,input_skeleton}.rs` |
| Dev-server handlers (transport-agnostic) | `crates/aion-server/src/dev_ui/{mod,handlers,mock}.rs` |
| Dev-server HTTP facade (`/dev/*`) | `crates/aion-server/src/api/http/dev_ui.rs` |
| Server config (`[dev]`/`[authoring]`/`[deploy]`) | `crates/aion-server/src/config/{mod,env}.rs` |

---

## 3. Global CLI flags

Defined on the top-level `Cli` struct (`crates/aion-cli/src/main.rs`), `global = true`, so
they precede or follow any subcommand:

| Flag | Meaning | Default |
|---|---|---|
| `--endpoint <URL>` | gRPC server endpoint addressed by client commands (`dev`, `start`, `deploy`, `inspect`, …). Missing scheme ⇒ `http://`. | a built-in default endpoint |
| `--namespace <NS>` | Workflow namespace for operations. | a built-in default namespace |
| `--subject <S>` | Caller subject metadata. | a built-in default subject |
| `--token <T>` | Bearer token (overrides `AION_TOKEN`); absent ⇒ dev header paths. | none |
| `--pretty` | Pretty-print JSON output. | off |

---

## 4. Command reference

### 4.1 `aion new <name> --template agent --worker rust` (WA-006)

Scaffolds a complete, immediately-buildable durable **agent loop** project.

```
aion new <name> --template agent --worker rust
```

- `<name>` — **required** positional. Lowercase `snake_case`; becomes the directory and either the AWL workflow name (`<name>.awl`) or Gleam entry module (`src/<name>.gleam`). AWL template names also cannot be AWL reserved words.
- `--template <value>` — `value_enum`, default `awl`. Values: `awl`, `hello-world`, `approval-flow`, `saga`, `dev-pipeline`, `agent`.
- `--worker <lang>` — `value_enum` (`rust`). **Required for `agent`** (and `dev-pipeline`): the agent steps are worker-served activities, so a `worker/` crate is scaffolded. Omitting it for `agent` is a loud refusal (`template.rs::worker_requirement_reason`).

**Emits** (`crates/aion-cli/templates/agent/`): `src/<name>_io.gleam` (the **authored boundary types**, the types-first source of truth), `src/<name>_codecs.gleam` (**pre-generated** JSON codecs), `schemas/{input,output,step_input,step_output,decision,review_signal,agent_status}.json` (**pre-generated**, marker-carrying), `src/<name>.gleam` (the workflow, from `project.gleam`), `workflow.toml`, `README.md`, and a `worker/` crate (`Cargo.toml` + `src/main.rs` with trivial echo handlers — replace these with your agent driver). Prints a JSON `NewOutput { project, path, template, worker, files, next_steps }`.

The scaffold is on the types-first generated path (like `hello-world` and `dev-pipeline`): the shipped codecs module and schemas are byte-identical to what `aion generate` reproduces from `src/<name>_io.gleam`, so `aion generate . --check` passes on a fresh scaffold. Because the template pins the published hex `aion_flow` 0.4.x (`crates/aion-cli/templates/shared/gleam.toml`, which predates `activity.declare`/`aion/manifest` and `workflow.entrypoint`), the workflow keeps the generated-marker `run` adapter and the worker `main.rs` stays hand-authored with structs mirroring the step types. It is still on that pin: `aion_flow` 0.5.0 and 0.7.0 have since been published to Hex and the `dev-pipeline` template moved to `>= 0.5.0`, but the agent template did not, so it has no declared-activities `manifest()` path (generated wrappers, worker `main.rs`, wire-compat golden) yet. See §7 for its anatomy.

### 4.2 `aion dev [path] --gleam-path <PATH> [--debounce-ms <MS>]` (WA-002)

The instant authoring loop: watch a project, and on every save rebuild → repackage →
hot-load the new content-hash version into a **running server**, with no restart.

```
aion dev ./my_project --gleam-path "$(which gleam)" --endpoint http://localhost:8080
```

- `[path]` — project root (contains `gleam.toml` + `workflow.toml`); its `src/` tree is watched. Default `.`.
- `--gleam-path <PATH>` — **required, no default** (ADR-001). The external `gleam` binary the rebuild spawns (same shell-out the server-side authoring API uses, via `aion_toolchain::build_project`).
- `--debounce-ms <MS>` — **optional, omitted by default** (ADR-001: no invented interval). Coalesces an editor's save-burst into one rebuild. A `0` value is rejected (no-op the author did not mean). When absent, every change drives a rebuild and the content-hash dedupe makes a redundant rebuild a no-op load.
- Targets the server via the global `--endpoint`.

**Mechanism** (`dev/pipeline.rs`): on a relevant `src/**.gleam` change → `aion_toolchain::build_project` (external `gleam`) → `aion_package::package_project` → push the new `.aion` through the **operator deploy RPC** (`crate::deploy::deploy`). The new version registers under its own content-hash module; **runs already in flight stay pinned** to the version they started on (invariant #5). The watcher is event-driven (`notify` crate) — there is no poll interval.

**Server prerequisites:** the target server must have **`[deploy]` enabled** (hot-load goes through the deploy RPC) and, to serve the dev-UI endpoints in §5, **`[dev].enabled = true`**. See §6.

### 4.3 `aion check [--deterministic] [path]` (WA-007)

The static **determinism gate** — a CI check that a workflow reads no wall clock or entropy.

```
aion check --deterministic ./my_project    # exit 0 = clean, non-zero = violation
```

- `--deterministic` — **required to do anything**; the bare `aion check` exits with an error explaining the flag (it is the only mode today).
- `[path]` — project root. Default `.`.

**Behaviour** (`check_deterministic.rs`): reads `workflow.toml`, and for each `[[workflow]]` entry reads `entry_module` + `entry_function`, loads `src/<entry_module>.gleam`, and runs `aion_package::analyze_determinism(source, entry_function)`.

- **Clean** ⇒ prints `{ "workflows": ["<entry_module>", …], "deterministic": true }`, exit 0.
- **Violation** ⇒ exits **non-zero**; stderr names every flagged call with its function, the fully-qualified call (e.g. `erlang.system_time`), kind (`wall_clock`/`entropy`), and a remedy (`use workflow.now()` / `use workflow.random()`). A pretty JSON detail array rides in the error.

**What it flags** (`structure/determinism.rs::FORBIDDEN_CALLS`): `erlang.{system_time,monotonic_time,now,timestamp,unique_integer}`, `os.{system_time,timestamp,perf_counter,erlang_timestamp}`, `rand.{uniform,uniform_real,bytes}`, `crypto.strong_rand_bytes`, `float.random`, `int.random` — reachable from the entry function through the call graph. The deterministic `workflow.*` surface is deliberately not in the set. See §8 for the analyzer's exact reachability semantics and its one residual limit.

### 4.4 `aion generate [path] [--check]` (test scaffolds: WA-007; codegen: prior-landed)

Regenerates every per-activity artifact from the package's typed declarations, **and** (WA-007)
emits an `aion/testing` skeleton per workflow.

```
aion generate ./my_project           # write generated files (incl. test scaffold)
aion generate ./my_project --check    # CI drift gate: verify on-disk == fresh, no writes
```

- `[path]` default `.`. `--check`: regenerate in memory and exit **non-zero** if any on-disk generated file differs (drift gate) — writes nothing.
- The test skeleton lands at **`test/<entry_module>_scaffold_test.gleam`** (Phase 6 of `generate::run`, via `aion_package::generate_test_scaffold`). It targets the existing `aion/testing` harness — **each activity pre-mocked, one clock advance per durable timer, a replay-determinism assertion** — with `todo` placeholders and **no invented values** (ADR-001). The timer count is derived from source via `aion_package::extract_workflow_facts`.

### 4.5 `aion input <workflow_type> [path]` (WA-007)

Emits a structurally-valid JSON **input skeleton** derived from a workflow's input *type*.

```
aion input my_project ./my_project    # -> {"task_id":"", ...} a valid, decodable starting payload
```

- `<workflow_type>` — **required** positional; the `entry_module` name of the target `[[workflow]]`. On no match, errors and lists the available types (it does not guess).
- `[path]` default `.`.

**Behaviour** (`input.rs`): resolves the matching `[[workflow]].input_schema` from `workflow.toml`, then `aion_package::build_input_skeleton(schema_path, schema)`. The result decodes through the workflow's generated input codec without error. Required fields appear with type-shaped placeholders (`""`, `0`, `0.0`, `false`, `[]`; enums take their first wire value); **optional fields are omitted** — no invented defaults (ADR-001). Generated from the type, never hand-written.

---

## 5. Dev-server endpoints (WA-002 R2)

Three HTTP routes (`crates/aion-server/src/api/http/dev_ui.rs`), **mounted only when
`[dev].enabled = true`** — otherwise every `/dev/*` path is a plain `404` (dark by default).
All run over the **real** engine/store/event-stream (CN4) and authorize the caller via the
same namespace guard as production. Request types use `#[serde(deny_unknown_fields)]`.

### `POST /dev/runs` — trigger a run

Request `TriggerRunRequest`:
```json
{ "namespace": "default", "workflow_type": "my_project", "input": { "...": "..." } }
```
Response `TriggerRunResponse`:
```json
{
  "workflow_id": "<uuid>",
  "run_id": "<uuid>",
  "stream_subscription": {
    "path": "/events/stream",
    "subscribe": { "type": "subscribe",
      "subscription": { "per_workflow": { "namespace": "default", "workflow_id": "<uuid>" } } }
  }
}
```
Starts the run through the same path `/workflows/start` uses. To watch it live, connect the
existing **`/events/stream`** WebSocket firehose and send the `subscribe` frame verbatim — the
dev server opens **no second stream**.

### `POST /dev/mocks` — opt-in per-run activity mock

Request `RegisterMockRequest` (`outcome` is internally tagged by `kind`):
```json
{ "namespace": "default", "workflow_id": "<uuid>", "activity_name": "scout",
  "outcome": { "kind": "succeeds", "result": { "...": "..." } } }
```
`outcome` is one of `{"kind":"succeeds","result":<json>}` or `{"kind":"fails","message":"<str>"}`.
Response `RegisterMockResponse { workflow_id, activity_name }`. The mock is installed in the
shared `ActivityMockRegistry` the engine's dispatcher already consults (a decorator over the
production `WorkerActivityDispatcher`) — **the engine is untouched**, the canned result is
recorded identically to a real one. If `[dev].enabled` is false there is no registry and this
returns a backend error ("dev activity mocking is not enabled on this server").

### `POST /dev/replay` — re-drive a failed run

Request `ReplayRunRequest`:
```json
{ "namespace": "default", "workflow_id": "<uuid>" }
```
Response `ReplayRunResponse { replayed_workflow_id, workflow_type, workflow_id, run_id, stream_subscription }`.

> **Sharp edge (read this).** Replay reads the failed run's recorded `WorkflowStarted`
> (type + input) and **starts a fresh run** of the same workflow through the real engine. It is
> **not** resume-from-the-failed-step. It rejects any run not in `Failed` status. Do not
> describe `/dev/replay` as "resume".
>
> True resume landed elsewhere and this endpoint was **not** switched over to it.
> `Engine::reopen_workflow` (AD-012) is live (`crates/aion/src/engine/api.rs`) and reachable
> as `aion reopen <workflow-id>`: it appends a single `WorkflowReopened` superseding the
> terminal event and re-drives the **same** run through the recovery path, so replay returns
> every recorded result and only the reopened / in-flight step re-dispatches. It accepts a
> terminal `Failed` **or** `Cancelled` run. `replay_run` (`dev_ui/handlers.rs`) still does the
> start-fresh thing described above and remains the single switch-point if the dev endpoint is
> ever moved onto reopen.

---

## 6. Server config for the DX surface

Set in the server config file (TOML) or via env; CLI overrides exist for authoring.
The relevant sections (`crates/aion-server/src/config/{mod,env}.rs`):

### `[dev]` — the dev-UI surface (WA-002)

| Key | Type | Env | Notes |
|---|---|---|---|
| `enabled` | bool | `AION_DEV_ENABLED` | Dark by default. `true` mounts `/dev/*` **and** installs the `ActivityMockRegistry` + dispatcher decorator. With `false`/absent, none of it is reachable. |

### `[authoring]` — server-side compile API (WA-003, *prior-landed*; also what `aion dev` mirrors)

| Key | Type | Env | CLI | Notes |
|---|---|---|---|---|
| `gleam_path` | path | `AION_AUTHORING_GLEAM_PATH` | `--gleam-path` | Dark by default. Setting it commissions the `/authoring/*` compile loop; **must not be empty**. |
| `project_root` | path | `AION_AUTHORING_PROJECT_ROOT` | `--authoring-project-root` | **REQUIRED when `gleam_path` is set** (ADR-001, no default). A built project dir with `gleam.toml`, the `aion_flow` dep, `workflow.toml`, and `schemas/`. |

### `[deploy]` — operator deploy RPC (*prior-landed*; **required for `aion dev` hot-load**)

| Key | Type | Env | Notes |
|---|---|---|---|
| `enabled` | bool | `AION_DEPLOY_ENABLED` | Dark by default. `aion dev` pushes rebuilt packages through this RPC, so it must be **on** for the hot-load loop to work. |
| `max_archive_bytes` | u64 | `AION_DEPLOY_MAX_ARCHIVE_BYTES` | Defaults to 64 MiB when enabled; override to size for your packages. Upload ceiling. |
| `max_inflated_bytes` | u64 | `AION_DEPLOY_MAX_INFLATED_BYTES` | Defaults to 256 MiB when enabled; when set, ≥ `max_archive_bytes`. Guards a ~1000:1 zip-bomb. |

---

## 7. The agent scaffold anatomy (WA-006)

The scaffolded `src/<name>.gleam` (from `templates/agent/project.gleam`) is a durable
**scout → act → verify → signal-gated review** loop. Key facts an agent extending it needs:

- **Boundary types** live in `src/<name>_io.gleam` (authored, types-first); `aion generate` derives `src/<name>_codecs.gleam` and every `schemas/*.json` from them.
- **Start input** `Input { task_id, scout_prompt, act_prompt, verify_prompt, review_timeout_ms: Int }` — all fields **required**; `review_timeout_ms` is the human-review deadline in milliseconds (no default, ADR-003). Use `aion input <name>` to get a valid skeleton.
- **Loop** (`handle`): `set_status("scouting")` → `run_step("scout", …)` → `set_status("acting")` → `run_step("act", …)` (fed `scout` output as context) → `set_status("verifying")` → `run_step("verify", …)` → `set_status("awaiting_review")` → `await_review`.
- **Each step** is `workflow.run(step_activity(stage, StepInput{task_id,prompt,context})) -> StepOutput{result}` — a **worker-served activity**. The in-module `local_step` is a stub used **only** by the `aion/testing` harness; a deployed run dispatches to the connected `worker/`. **No step deadline** — agentic work runs unbounded until the worker answers (ADR-003). The worker bundles the agent runtime, not the workflow (ADR-011).
- **Approval pause** (`await_review`): `workflow.with_timeout(fn() { workflow.receive(review_signal()) }, duration.milliseconds(review_timeout_ms))` — a **durable signal wait** raced against the deadline, **not** a poll loop. The run suspends (seconds or weeks), survives restarts, and stays `Running` while suspended (invariant #4).
- **Signal** `agent_review` (`const review_signal_name`), payload `ReviewSignal { decision: "approve"|"reject", reviewer: String }`.
- **Query** `agent_status` (`const status_query_name`), returns `AgentStatus { stage, task_id }`, answered after replay with no extra author code.
- **Dispositions:** `approve` ⇒ `disposition: "applied"`; `reject` **or** deadline lapse ⇒ `disposition: "held"`. **Either way the run `Completes`** — a held artifact is a successful, fully-recorded run awaiting a human follow-up. (The timeout arm matches `error.TimedOutError(error.TimedOut(...))`; engine faults surface as `GateFailed`.)
- `workflow.toml` sets `timeout_seconds = 604800` (7 days) — this is an **author-chosen whole-run ceiling baked into the template as an example**, distinct from the (absent) step/approval defaults; edit it for your run.

---

## 8. The determinism analyzer — exact semantics (WA-007)

`aion_package::analyze_determinism` (`structure/determinism.rs`) is a **token-based**
analysis, deliberately *not* a Gleam type-checker. It reuses the WA-005 tokeniser/scanner so
the linter and the graph projection agree on "reachable from workflow code".

- **Reachability:** from the entry function, it follows a local helper both when **applied
  directly** (`helper(...)`) and when **passed as a bare function value in argument position**
  (`list.map(items, helper)`, paren-depth ≥ 1). It recurses each helper once (a `visited` set
  terminates mutual recursion).
- **Soundness bias:** a call outside the fixed forbidden vocabulary is never flagged (no false
  positive on the author's own helpers); a forbidden call reachable from the entry is always
  flagged. Entropy/wall-clock inside a *recorded activity* (a separate module reached via
  `wrappers.<name>_activity`) is correctly **not** flagged — that is the legitimate place for
  side effects.
- **One residual limit (documented, intentional):** a helper aliased through a `let` binding
  and then passed (`let f = helper  list.map(items, f)`) is not followed — name-level token
  analysis cannot resolve the alias without a type-checker. The same limit applies to the twin
  timer counter in `facts.rs`. If you extend this, add alias resolution in both walkers and a
  fixture for each; do not silently narrow it.

---

## 9. End-to-end runbook

```bash
# 1. Scaffold a durable agent project + worker crate.
aion new my_agent --template agent --worker rust
cd my_agent

# 2. Gate determinism early (and wire this into CI).
aion check --deterministic .

# 3. Verify the pre-generated artifacts (codecs module + schemas) match the
#    authored types module src/my_agent_io.gleam; rerun `aion generate .`
#    after editing a boundary type.
aion generate . --check     # CI drift gate

# 4. Start a server with the dev + deploy surfaces enabled (config or env):
#      AION_DEPLOY_ENABLED=true      # the ceilings default; set AION_DEPLOY_MAX_ARCHIVE_BYTES /
#                                    # AION_DEPLOY_MAX_INFLATED_BYTES only to override them
#      AION_DEV_ENABLED=true
aion server --config server.toml

# 5. In another terminal: the instant loop — edit src/my_agent.gleam and watch it hot-load.
aion dev . --gleam-path "$(which gleam)" --endpoint http://localhost:8080

# 6. Get a valid start payload from the input type, fill in prompts + review_timeout_ms.
aion input my_agent .

# 7. Trigger a run via the dev server and watch it live over the firehose.
curl -XPOST localhost:8080/dev/runs \
  -d '{"namespace":"default","workflow_type":"my_agent","input":{"task_id":"t1","scout_prompt":"…","act_prompt":"…","verify_prompt":"…","review_timeout_ms":86400000}}'
# connect /events/stream (WebSocket) and send the returned `subscribe` frame.

# 8. Approve the run (or let the deadline hold it).
#    Send the `agent_review` signal {decision:"approve", reviewer:"you"} via the normal
#    signal path (aion client signal / gRPC), or mock an activity for the next run:
curl -XPOST localhost:8080/dev/mocks \
  -d '{"namespace":"default","workflow_id":"<uuid>","activity_name":"scout","outcome":{"kind":"succeeds","result":{"result":"canned"}}}'
```

---

## 10. Sharp edges checklist (the bullet-proofing)

- **`--gleam-path` is required** for `aion dev`; there is no default `gleam` binary (ADR-001).
- **`aion dev` needs the server's `[deploy]` enabled** (hot-load uses the deploy RPC) and `[dev].enabled` for the `/dev/*` endpoints.
- **`/dev/replay` is start-fresh, not resume** (only `Failed` runs). Never call it "resume". True resume is `Engine::reopen_workflow` / `aion reopen` (AD-012, landed) on a different path; `/dev/replay` was not moved onto it — see §5.
- **Dev surface and authoring/deploy surfaces are dark by default** — nothing dev-specific is reachable until explicitly enabled; mocking is a decorator, never a mock-only engine (CN4).
- **Activity mocking targets `workflow_id`** (the run), not a `run_id` field — see `RegisterMockRequest`.
- **`aion input <workflow_type>` requires the type positional** and lists available types on a miss; it does not auto-pick a sole workflow.
- **`aion check` without `--deterministic` errors by design** (the flag is mandatory today).
- **Determinism analysis does not follow `let`-aliased function values** (§8) — a known, documented gap.
- **The agent template requires `--worker rust`**; the review deadline (`review_timeout_ms`) and step deadlines are never defaulted.
- **Runtime-gated tests:** the gleam/worker E2Es (`dev_hot_reload_e2e`, `new_agent_e2e`, `authoring_quality_e2e`, etc.) detect a missing toolchain at runtime, print a skip line, and return `Ok(())` — they are **never** `#[ignore]`. Run them with `gleam`/`cargo` on `PATH` to exercise the full loop.
- **Red test observed on 2026-06-21:** `crates/aion-cli/tests/stacked_dev_live_e2e.rs` failed in sandboxes where it could not spawn `cargo` for the worker build (`No such file or directory`). It was pre-existing, untouched by this cluster, and not a regression. That was one day's reading of one environment — the test file is still there; re-run it before treating it as red or green.

## 11. Verification

These are the commands that were run on 2026-06-21, not a standing claim about
the current tree. Run them yourself:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

On that date the workspace suite was green apart from the environment-bound
`stacked_dev_live_e2e` noted in §10.

Quality bar (CLAUDE.md): no `unwrap`/`expect`/`panic`/`todo`/`unreachable` in library code; no
`#[allow]`/`#[ignore]` bypasses; `mod.rs` is only `pub mod` + re-exports; no file > 500 code-LOC.

## 12. Known follow-on

- **WA-004 in-VM live-lens** (not yet authored): a fully-faithful in-VM time-travel lens with
  live per-step workflow-visible state and actual random-draw counts. Needs an engine
  determinism-observation hook + an `aion-cli` local-engine bootstrap, and pairs naturally with
  the WA-002 dev-server/local-engine surface. Author as its own brief.
