# The Aion Swing — Design Brief

Status: drafted 2026-08-09 (Vesper). Routed to Waffles for review before
any code. Tom's rulings are quoted as constraints; everything else is
proposal.

## 1. Frame

Tom's big swing for aion, in his words across the 2026-08-08/09 exchanges:
aion becomes a **one-stop shop** — author, operate, report, query, and get
operational answers, all through the product itself. The assistant is
**built into aion**, not a separate worker someone runs. The assistant gets
a **purpose-built set of Aion tools** — agents keep their own file
read/edit tools; ours are aion-only, **fewer and deeper**, not
over-generalized. **Mid-step intelligence is a priority**: what's
happening in a step, what the current step's agent is doing, what a
command step has printed — not just the final recorded result.

Ruled 2026-08-09 (Tom, confirmed against primary sources): the MCP server
is built to the **2026-07-28 stateless spec**, not the handshake era.

## 2. Constraints inventory (Tom's rulings, binding)

- R-T1. Assistant BUILT IN (not an external worker). One-stop shop.
- R-T2. Purpose-built Aion toolset; agents bring their own general tools.
- R-T3. Fewer deep tools, not many shallow ones.
- R-T4. Mid-step intelligence is a priority.
- R-T5. New MCP spec (2026-07-28, stateless). Tasks via the official
  extension. (Footnotes accepted: ext spec still draft + zero external
  clients today — acceptable because the built-in assistant means we own
  both ends; pin the ext version, track churn. Handshake-era shim is an
  explicitly deferred decision, not built speculatively.)
- R-T6. A blocking tail tool is wrong for MCP (locks the agent) — reads
  are cursor pages that return immediately.

## 3. The worker story (#30 + W-1..W-4)

Today only the WORKER is hand-started; both harness adapters spawn the
agent as a child process per dispatch (verified: aion-integration-cli and
aion-integration-norn both Command::spawn with piped stdio through the
jsonrpc envelope seam in aion-integrations). Three serving paths already
exist for AWL: declared `run "…"` bodies server-run; `aion worker shell` +
manifest; SDK worker for custom logic (scaffold generates compiling Rust).

The swing closes the two gaps:
- **#30 — AWL-declared workers** (last leg of the authoring story): a file
  declares worker vs workflow; richer declarative action bodies;
  scaffold-from-studio.
- **W-1..W-4 — lifecycle**: the server supervises workers; workers
  supervise agents. Two clean supervision layers; no hand-started
  processes. Deploy/start/stop/supervise from the console (#27's frame).
  Aion-native supervision (no launchd — standing ruling).

## 4. The built-in assistant

- Ships inside `aion server` — present the moment the server runs (same
  out-of-box doctrine as the ops console).
- **Pluggable brain**: norn is today's default engine; the brain is a
  seam, not a hard dependency.
- **ACP remains the door** for external agents-as-workers; the built-in
  assistant does not replace that path.
- The assistant's tool surface is exactly the MCP server's tool surface
  (one contract, two consumers: the embedded brain and any external host
  an operator connects).

## 5. The Aion MCP server (2026-07-28 stateless)

- **Transport**: streamable HTTP per the new spec (POST-only endpoint,
  Mcp-Method/Mcp-Name headers, no sessions, no SSE resumability). stdio
  for local tooling. `server/discover` answered; per-request `_meta`
  carries version/capabilities.
- **Statelessness maps onto the engine natively**: all cross-call
  correlation rides in explicit handles (workflow_id, run_id,
  activity_id, attempt, seq cursors) — the spec's own stateful-tools
  pattern, which is already how aion's API thinks. No connection state
  anywhere.
- **Tasks**: implement the official `io.modelcontextprotocol/tasks`
  extension (pin the version implemented; it is draft — track churn).
  Server may return `resultType:"task"` for long operations (e.g.
  start-and-await); status via `tasks/get` (result embedded on
  completion); cooperative `tasks/cancel`. Advertised only to clients
  declaring the extension — our built-in assistant declares it day one.
- **Caching fields** (`ttlMs`/`cacheScope`) on list/read results per spec.
- **Instructions**: `server/discover` returns a few hundred words teaching
  an agent how to drive aion (check the run, read the step transcript,
  never invent ids, prefer describe_run before acting).
- **Deferred, explicitly**: handshake-era (2025-xx) compatibility shim.
  The tool surface is era-identical, so the shim is additive if ever
  needed. Decision point: when an external host we care about needs it.

## 6. The tool contract (few and deep)

All tools carry `outputSchema` + structured content; all read tools carry
read-only annotations. Handles in, handles out; every read returns
immediately with what exists now.

1. `start_run` — start a workflow (namespace, type/document, input) →
   {workflow_id, run_id} immediately. Task-shaped variant awaits
   completion under the tasks extension.
2. `describe_run` — THE join (see §7): summary + status projection +
   current step + live-attempt liveness + transcript tails + unserved
   detail, one call.
3. `read_transcript` — cursor page of a (workflow, activity, attempt)
   observability stream; `from_seq`/`limit` → {events, next_from_seq,
   head_seq}. Replaces any notion of a blocking tail (R-T6).
4. `read_history` — cursor page of run history with payload eliding.
5. `list_runs` — filters (type, status, time, parent).
6. `signal` — deliver a signal.
7. `query` — mid-step workflow query (needs §7.4 query-arguments fix to
   be whole).
8. `cancel` — cancel a run.

Candidates deliberately NOT tools (kept few): deploy/authoring mutations
stay CLI/console verbs for now — the assistant can guide, the operator
acts. Revisit only with Tom.

## 7. Observability builds (feed the tools; verified against code)

Verified state: agent-step transcripts are durably appended per-event at
arrival and readable mid-run via cursor reads (POST /workflows/transcript;
non-blocking; per-(workflow,activity,attempt) streams). Heartbeats carry
an optional progress payload stored in-memory and READABLE BY NOTHING.
Declared commands capture stdout/stderr only at completion via
read_to_end; shell activities emit zero observability events. No
server-side "current step" answer exists anywhere; no composed endpoint.

Build order:
1. **Current-step projection + describe_run join** — one endpoint doing
   the 4-source stitch (summary/history-fold current step/attempts/
   transcript heads + tails). Serves MCP, console (#59), CLI alike.
2. **Command output into the transcript machinery** — replace the
   read_to_end capture with line-streaming that emits ActivityEvents;
   the whole downstream pipeline (durable append, cursor reads, WS tail)
   already exists and is harness-neutral. Kills the biggest mid-step
   blind spot.
3. **Heartbeat "doing" note read surface** — the wire slot and in-memory
   store exist; add the read (into describe_run), decide persistence.
4. **Query arguments** — the wire drops the payload today; forward it.

Riders from the audit, scoped small: transcript stream discovery is
unannotated (no activity_type/current marker); retention cap makes long
tails live-only (describe_run must say so rather than show silence);
transcript key carries no RunId (continue-as-new collision unproven —
verify before the MCP resource URIs freeze).

## 8. Sequencing

1. **#139 assistant cut first** — lifts the estate-wide worker-rebuild
   freeze; unblocks #142 and #141.
2. **This brief through Waffles** for review BEFORE code (Tom's
   confirmation of the MCP section already in hand).
3. **#30 + W-1..W-4 as parallel lanes** once the brief is ratified.
4. **Observability builds** (§7 order) — can start alongside, they are
   additive server surfaces.
5. **MCP server** consumes §7; tasks extension + discover instructions.
6. **#164 weave** (AWL guide/help through the authoring surface) last.

## 9. Open questions for Waffles/Tom

- Q1. Does describe_run's current-step projection live server-side as a
  fold-on-read, or as a maintained projection updated on append? (Perf vs
  simplicity; fold-on-read is the honest start.)
- Q2. Heartbeat "doing" note: persisted (survives restart, costs writes)
  or volatile-with-honest-absence? Propose volatile first, stated in the
  response shape.
- Q3. Tool auth: the MCP server inherits the server's existing auth
  model — anything extra for mutating tools (signal/cancel/start)?
- Q4. Where does the MCP endpoint mount — same HTTP surface as the
  console (one port, one-stop) or its own listener?
