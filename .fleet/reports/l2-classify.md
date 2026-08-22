# L2 classification report — policy-refused fallback

## Delivered

- Added a first-class `HarnessError::PolicyRefused` classification with explicit
  `PolicyRefusalDetection::{Structured, Textual}` provenance.
- Kept policy refusal stochastic: `HarnessError::is_deterministic()` returns
  `false`, following the measured same-act survivals recorded by L1.
- Classified norn refusals at the adapter boundary using the exact structured
  discriminator `/error/code == "cyber_policy"`, plus only the measured message
  phrase `provider rejected the request under its cybersecurity policy` as the
  admitted textual fallback.
- Classified ACP `StopReason::Refusal` before its general non-completion path.
- Added `ActivityErrorKind::PolicyRefused` through the Rust worker protocol,
  server bridge, generated protobuf surfaces, TypeScript SDK, Python SDK, and
  Gleam public error vocabulary.
- Preserved the class through the existing string-only engine seam with the
  exact `policy_refused:` prefix.
- Added focused tests for adapter recognition, ordinary-error non-recognition,
  worker classification, protobuf conversion, server prefix rendering, SDK
  mapping, and Gleam in-VM parsing.
- No AWL fallback syntax, task-queue routing, retry-budget change, replay event,
  settlement predicate, or other L3 behavior was implemented.

## Classification behavior observed

### Shared harness taxonomy

- `PolicyRefusalDetection` is declared at
  `crates/aion-integrations/src/error.rs:11` and is publicly re-exported at
  `crates/aion-integrations/src/lib.rs:56`.
- `HarnessError::PolicyRefused { detail, detection }` is at
  `crates/aion-integrations/src/error.rs:99-104`; its constructor is at
  `:191-195`.
- The exhaustive determinism mapping includes policy refusal in the
  nondeterministic group at `crates/aion-integrations/src/error.rs:256-266`.
  This is intentionally distinct from ordinary deterministic terminal defects.

### Norn adapter

- Classification occurs before JSON-RPC error flattening in
  `crates/aion-integration-norn/src/session.rs::response_into_payload`, whose
  declaration is at `:290`.
- The recognizer at `crates/aion-integration-norn/src/session.rs:326-338`
  accepts exactly:
  - structured `error.data.pointer("/error/code") == "cyber_policy"`, recorded
    as `Structured`; or
  - the measured phrase in `JsonRpcError.message`, recorded as `Textual`.
- An unrelated generic JSON-RPC server error remains `HarnessError::Harness`.
  The structured, textual, and negative cases are asserted in
  `crates/aion-integration-norn/src/session/tests/outcomes.rs:125-187`.
- Advice normalization explicitly passes the class through unchanged at
  `crates/aion-integration-norn/src/advice.rs:155`.

### ACP adapter

- `StopReason::Refusal` is separated before generic non-completion handling at
  `crates/aion-integration-acp/src/outcome.rs:155-159`.
- It is mapped to `PolicyRefused` with structured provenance at
  `crates/aion-integration-acp/src/outcome.rs:173-179`, preserving bounded
  partial response text.
- `MaxTokens` and `MaxTurnRequests` remain ordinary harness non-completions.

### Worker, server, and engine seam

- `ActivityErrorKind::PolicyRefused` is declared at
  `crates/aion-core/src/error.rs:14`. Existing `ActivityError::is_retryable()`
  remains true only for `Retryable`, so this dedicated kind is non-retryable at
  the activity taxonomy layer without being relabelled `Terminal`.
- Worker classification is declared and converted in
  `crates/aion-worker/src/activity.rs:27`, `:49-52`, and `:98-104`.
- `harness_error_to_outcome` maps policy refusal before the ordinary binary
  determinism mapping at `crates/aion-worker/src/runtime/agent.rs:347-381`.
  Ordinary nondeterministic harness failures still map to retryable failures.
- Intervention-only paths combine `PolicyRefused` with the existing stale-target
  NACK behavior at `crates/aion-worker/src/runtime/agent.rs:250-253` and
  `:338-341`; this does not alter result-path classification.
- The server bridge emits `policy_refused:<message>` at
  `crates/aion-server/src/worker/bridge.rs:460-485`, with an exact-prefix test at
  `:2054-2073`.
- A second pre-existing prefix renderer, discovered through exhaustive
  compilation, is handled at
  `crates/aion-server/src/worker/declared_body.rs:483`.
- Gleam parses the exact 15-byte prefix into the public typed error at
  `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:138-140` and
  re-encodes it at `:191`. The public variant is at
  `gleam/aion_flow/src/aion/error.gleam:28`; the in-VM round-trip assertion is at
  `gleam/aion_flow/test/aion_tier_test.gleam:291-303`.

## Protocol and binding propagation

- Protobuf adds `ACTIVITY_ERROR_KIND_POLICY_REFUSED = 3` at
  `crates/aion-proto-generated/proto/worker.proto:240`.
- Hand-written Rust protocol conversions are at
  `crates/aion-proto/src/worker.rs:24`, `:303`, and `:318`; the round-trip test is
  at `:378-392`.
- The ops-console ts-rs output was regenerated.
- The TypeScript worker protobuf output exposes value `3` at
  `sdks/typescript/aion-worker/src/proto/worker.ts:29`; the public failure shape
  adds `policyRefused?: boolean` at
  `sdks/typescript/aion-worker/src/session.ts:294`, and mapping gives it
  precedence over `retryable` at `:875-880`.
- Python exposes `PolicyRefusedError` at
  `sdks/python/aion-worker/aion_worker/errors.py:22`, exports it from
  `aion_worker.__init__`, and maps it to the generated policy-refused enum at
  `sdks/python/aion-worker/aion_worker/activity.py:165-168`.
- Python and TypeScript protobuf bindings were regenerated from the modified
  repository schema; no hand-authored substitute enum was used.

## Moved and additional seam coordinates observed

- The L1 artifact names a nonexistent norn
  `outcome_into_payload` secondary symbol. On this tree the secondary result
  envelope translator is
  `crates/aion-integration-norn/src/translate/outcome.rs::run_result_to_outcome`
  at `:54`, re-exported at `crates/aion-integration-norn/src/translate.rs:109`.
  The L2 refusal signal is present earlier in the JSON-RPC error envelope, so
  classification was correctly placed in `response_into_payload`; the result
  translator has no explicit refusal reason to classify.
- The ACP refusal seam is
  `crates/aion-integration-acp/src/outcome.rs::outcome_into_payload` at `:131`,
  with the refusal branch at `:155`, rather than a `prompt_error` function.
- Exhaustive compilation exposed the additional worker result prefix renderer in
  `crates/aion-server/src/worker/declared_body.rs:483`; the governing documents
  named only the bridge renderer.
- Protocol propagation required both the schema and the hand-written Rust
  `ProtoActivityErrorKind` conversion surface, plus generated Python files. The
  governing document's generated-surface list undercounted those concrete
  locations.
- Current primary coordinates after implementation are:
  - `HarnessError`: declaration at `crates/aion-integrations/src/error.rs:25`
    (enum span `:25-137`);
  - `HarnessError::is_deterministic`: `:256-266`;
  - norn `response_into_payload`: `crates/aion-integration-norn/src/session.rs:290`;
  - ACP `outcome_into_payload`: `crates/aion-integration-acp/src/outcome.rs:131`;
  - `harness_error_to_outcome`: `crates/aion-worker/src/runtime/agent.rs:354-381`;
  - bridge `complete_activity`: `crates/aion-server/src/worker/bridge.rs:460`;
  - Gleam `parse_error`: `activity_dispatch.gleam:129-163`;
  - Gleam `encode_error`: `activity_dispatch.gleam:181-198`.

## F-3 report-only finding

No F-3 repair was made in this leg.

- Gleam's current activity error parser still does not recognize the engine/server
  prefixes `exhausted:`, `parked:`, `lost:`, `transport-exhausted:`, or
  `superseded:`. Those values can therefore continue to fall through to the
  generic activity-engine failure path.
- `infrastructure:` is different: the observed producer places that text in the
  message body of an already typed activity error, rather than using it as an
  unparsed outer classification prefix.
- Representative current producers include
  `crates/aion/src/runtime/nif_activity_retry.rs:37`, `:49`, `:76`, and `:382`,
  and `crates/aion-server/src/worker/transport_loss.rs:65-71`.
- This finding is recorded only; changing that vocabulary is outside L2.

## Validation observed

### Rust

- `cargo check -p aion-integrations -p aion-integration-norn \
  -p aion-integration-acp -p aion-worker -p aion-proto -p aion-server`: passed.
- Focused library suites for integrations, norn, ACP, worker, and proto: passed.
- Exact protobuf policy-refused round trip: passed.
- Exact server bridge `policy_refused:` rendering test: passed.
- Final-tree `cargo test --workspace --all-targets`: passed.
- Final-tree validation chain:
  `cargo fmt --all && cargo test -p aion-integrations \
  -p aion-integration-norn -p aion-integration-acp -p aion-worker --lib \
  && cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `AION_UPDATE_BINDINGS=1 cargo test -p aion-core export_ops_console_wire_types`:
  passed and refreshed the checked-in generated type.

### TypeScript

- `bun run check:proto`: passed.
- `bun run typecheck`: passed.
- `bun run test`: passed, 6 files and 119 tests.

### Python

- `python3 -m pytest`: passed, 141 tests.
- `ruff check aion_worker tests`: passed.
- `mypy aion_worker`: not green. It reports 11 errors only in generated
  `aion_worker/proto/*.pyi` files: missing `Mapping` type arguments and the
  generated `Payload.bytes` name/type collision. The same result occurs when
  invoking the configured exclusion form, because this mypy invocation still
  follows imported generated stubs. No hand-written policy-refusal module was
  named by the errors. This is recorded as a generated-stub/tooling validation
  issue, not silently treated as a pass.

### Gleam

- The official Gleam 1.18.1 compiler archive was checksum-verified.
- Touched Gleam files were formatted.
- Host `gleam test` compiled the project but could not execute because the host
  Erlang installation lacks EUnit.
- The same suite in an `erlang:27-alpine` container passed: 166 tests, no
  failures.

### Repository and scope

- `git diff --check`: passed.
- Scope grep found no fallback-routing, retry-bound, or L3 implementation; its
  only `fallback` hits are the documented norn textual classifier and its test.
- Generated TypeScript drift check passed; Python protobuf output was regenerated
  using the repository-declared generator dependencies after discarding an
  incompatible system-`protoc` attempt.
- File-length tooling flagged pre-existing oversized seam files:
  `crates/aion-server/src/worker/bridge.rs`,
  `crates/aion-server/src/worker/declared_body.rs`,
  `crates/aion-proto/src/worker.rs`, and
  `sdks/typescript/aion-worker/src/session.ts`. L2 changed only the required
  exhaustive classification/rendering sites; splitting these established modules
  would be a separate broad refactor outside this leg.
- A final independent read-only audit re-derived every shared-class, adapter,
  catch-all, worker/wire, generated-binding, SDK, and Gleam seam from the current
  tree. It found no blocking defect and no L3 scope leakage. It independently
  confirmed that the artifact's nonexistent norn secondary symbol unambiguously
  refers to `translate/outcome.rs::run_result_to_outcome`, and that the terminal
  settlement predicate remains L3 work.
- The focused Rust suites, full-workspace clippy, TypeScript drift/type/test
  gates, Python tests/lint, formatting, and whitespace gates were re-run from the
  committed implementation tree on 2026-08-22 and passed.
- No provider policy refusal occurred during implementation or validation.

## Committed-tree receipt

The required existence receipt was produced with:

```text
git diff --name-only HEAD^..HEAD | while IFS= read -r path; do
  git ls-tree --name-only HEAD -- "$path"
done
```

Observed output:

```text
.fleet/reports/l2-classify.md
apps/aion-ops-console/src/types/generated/index.ts
crates/aion-core/src/error.rs
crates/aion-integration-acp/src/outcome.rs
crates/aion-integration-acp/src/wire_tests/outcomes_and_permissions.rs
crates/aion-integration-norn/src/advice.rs
crates/aion-integration-norn/src/session.rs
crates/aion-integration-norn/src/session/tests/outcomes.rs
crates/aion-integrations/src/error.rs
crates/aion-integrations/src/lib.rs
crates/aion-proto-generated/proto/worker.proto
crates/aion-proto/src/worker.rs
crates/aion-server/src/worker/bridge.rs
crates/aion-server/src/worker/declared_body.rs
crates/aion-worker/src/activity.rs
crates/aion-worker/src/runtime/agent.rs
crates/aion-worker/src/runtime/agent_tests.rs
gleam/aion_flow/src/aion/awl/error.gleam
gleam/aion_flow/src/aion/error.gleam
gleam/aion_flow/src/aion/internal/activity_dispatch.gleam
gleam/aion_flow/src/aion/testing/mock.gleam
gleam/aion_flow/test/aion_tier_test.gleam
sdks/python/aion-worker/aion_worker/__init__.py
sdks/python/aion-worker/aion_worker/activity.py
sdks/python/aion-worker/aion_worker/errors.py
sdks/python/aion-worker/aion_worker/proto/worker_pb2.py
sdks/python/aion-worker/aion_worker/proto/worker_pb2.pyi
sdks/python/aion-worker/tests/test_ar008.py
sdks/typescript/aion-worker/src/proto/worker.ts
sdks/typescript/aion-worker/src/session.ts
```
