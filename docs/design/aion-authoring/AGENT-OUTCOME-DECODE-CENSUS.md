# AgentOutcome historical-payload decode census (#241)

Date: 2026-08-15. Scope: every production path found by following an
`ActivityCompleted.result` from an `EventStore` read through replay/resume,
describe, MCP history, HTTP history, and the ops console. “Decode” below is
reserved for interpreting the payload bytes as the canonical agent-outcome
record; decoding the enclosing `Event` or opaque `Payload { content_type,
bytes }` is listed separately so it is not mistaken for safety.

## Contract and verdict

A pre-#241 agent result is JSON with `text`, `stop_reason`, and `session_id`.
The current record requires those fields plus `final_message`; there is no
default. The only semantic result decoder is the current workflow output
codec. Therefore:

- any replay or resume that re-executes the agent activity's await and feeds a
  recorded three-field result to the current four-field codec hard-fails at
  that await;
- merely reading, listing, describing, projecting, or displaying a completed
  history does **not** decode AgentOutcome and continues to work;
- a completed run is safe for today's read-only describe/history/console
  surfaces, but it is not globally safe to ignore: replay-inspect, reopen, or
  any future execution path that revisits that result can reach the strict
  decoder.

The deployment verdict is consequently conservative: **do not swap the binary
while any retained run history contains a three-field agent result unless every
execution/read path that could revisit that result is proven unused for that
run.** “The run is completed” alone proves only that the ordinary read-only
surfaces below are safe; it does not prove the history will never be replayed or
reopened.

## Enumerated sites

| Site | What it decodes | Verdict for a three-field agent result |
|---|---|---|
| `crates/aion-store/src/memory.rs` (`read_history*`) | Returns already typed `Event` values; `result` remains `Payload`. | **Opaque / safe read.** No AgentOutcome decode. |
| `crates/aion-store-libsql/src/read.rs` (`read_history*`) | Decodes persisted rows into `Event`, including the generic payload envelope and bytes. | **Opaque / safe read.** Missing `final_message` is not inspected. |
| `crates/aion-store-haematite/src/store.rs` (`read_history*`) | Decodes stored `Event` values and generic payload bytes. | **Opaque / safe read.** Missing `final_message` is not inspected. |
| `crates/aion-core/src/event.rs` (`Event::ActivityCompleted`) | Serde shape is `result: Payload`, not `AgentOutcome`. | **Opaque / safe envelope decode.** |
| `crates/aion-proto/src/convert.rs` (`decode_event`) and `crates/aion-proto/src/events.rs` (`EventFrame::decode_event`) | Decode transport envelopes to `Event`; the nested result stays `Payload`. | **Opaque / safe transport decode.** |
| `crates/aion/src/durability/resolver.rs` (`ActivityCompleted` arm) | Clones the recorded `Payload` into `Resolution::ActivityCompleted`. | **Opaque / safe at this layer.** |
| `crates/aion/src/durability/replay.rs` (`Replay::step` / `drive`) | Orders and returns recorded resolutions; it does not inspect result JSON. | **Opaque here, hazardous downstream.** Replay that sends this resolution back to workflow execution reaches the strict codec below. |
| `crates/aion/src/durability/recovery.rs` (`recover_workflow`) | Startup recovery builds `Replay` from an active run and resumes its loaded workflow command stream. | **Breaks affected active/in-flight runs** when replay revisits the recorded agent await. |
| `crates/aion/src/lifecycle/pause.rs` (`resume`) and `crates/aion/src/lifecycle/reopen.rs` (`reopen`) | Respawn workflow execution from retained history. | **Breaks an affected resumed/reopened run** if execution revisits the old agent result. Completion is not a waiver for reopen. |
| `crates/aion/src/durability/replay_inspect.rs` (`inspect` / mocked replay) | Replays a command stream over retained results, preserving result payloads as generic values until workflow-side interpretation. | **Potentially breaks inspection that executes the current workflow codec;** event-only projection remains opaque. Treat as a semantic read path, not as describe. |
| `crates/aion/src/runtime/nif_activity_await.rs` (`recorded_step`) | Converts `Resolution::ActivityCompleted` to the raw JSON byte string returned to Gleam. | **Handoff point.** Still opaque in Rust, but it feeds the semantic decoder directly. |
| `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam` (`await`) | Invokes the activity's current `output_codec.decode(payload)`. | **Semantic decode / hard failure.** A missing required field becomes `ActivityDecodeFailed`. |
| `gleam/aion_flow/src/aion/agent.gleam` (`outcome_codec`) | Requires `text`, `final_message`, `stop_reason`, and `session_id` with `decode.field`. | **Semantic decode / hard failure.** This is the canonical Gleam AgentOutcome decoder used by the agent activities. |
| `crates/aion-server/src/api/handlers/describe.rs` and `crates/aion-server/src/mcp/tools/run_view.rs` | Read complete histories to fold status and run scope; results remain generic payloads. | **Opaque / completed-history reads safe.** |
| `crates/aion-server/src/mcp/tools/describe.rs` (`describe_run`) | Matches activity terminal metadata and does not project result bytes. | **Opaque / completed-history reads safe.** |
| `crates/aion-server/src/mcp/tools/history.rs` (`read_history`) | Serializes/project events and optionally elides generic payload bytes. | **Opaque / completed-history reads safe.** |
| `crates/aion-server/src/api/http/history.rs` (`fetch_history`, `fetch_event`, `project_event`) | Serializes `Event`; payload limiting counts or replaces the generic `bytes` array. | **Opaque / console history reads safe.** |
| `apps/aion-ops-console/src/lib/api/workflow-history-contract.ts` (`decodeHistoryWindow`, `decodeEventResponse`) | Validates only the event envelope and leaves `data.result` as generated `Payload`. | **Opaque / console reads safe.** |
| `apps/aion-ops-console/src/types/generated/index.ts` (`Event` / `Payload`) | Types `ActivityCompleted.result` as opaque `Payload`. The generated `AgentOutcome` type is not used by a history decoder. | **No semantic decode / console reads safe.** |

## Swap consequence by run state

- **Running, parked, paused, crashed-but-active, or otherwise recoverable:** the
  swap is unsafe if history already contains a three-field agent completion;
  startup recovery or operator resume can replay it into the four-field codec.
- **Terminal/completed and only read through describe, MCP history, HTTP history,
  or the console:** those reads are safe because they never interpret the
  payload as AgentOutcome.
- **Terminal/completed but eligible for replay-inspect, reopen, or another
  execution-bearing operation:** unsafe unless that path is proven unused for
  the specific run. The retained three-field bytes have not been migrated.
