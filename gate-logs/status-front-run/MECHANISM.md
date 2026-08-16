# R2 — status mechanism audit

## Verdict

Event history is the lifecycle source of truth, but `POST /workflows/list` does
**not** project status from history at read time. It returns a durable visibility
row whose mutable `status` field is a cached projection. That cache is normally
rebuilt from the workflow's full history after an append and during startup or
adoption reconciliation, but the append and visibility upsert are separate
operations. Consequently, a successful history append followed by an
interrupted or failed visibility upsert can leave the list surface stale until
reconciliation.

The banked F1 observation was therefore a real visibility-cache divergence: two
histories already contained `WorkflowFailed` while the list still displayed
`Running`. The sources prove the separate-write window that permits this state,
and the completion retry path explicitly handles an attempt that appended a
terminal event and died before upserting visibility. They do not prove which
particular interruption produced those four wild rows.

The alleged F2 state was not a second writer or a side-channel status mutation.
It was an observation error caused by examining exactly 500 events from histories
that contained 604 and 605 events. Full-history collection shows matching
`WorkflowFailed` events at sequences 604 and 605. Thus R2(b)'s premise — that
something wrote `Failed` without a terminal event — is disproven by the complete
record.

## (a) Where `/workflows/list` status comes from

### List reads the durable visibility projection

The list request follows this path:

1. The HTTP router maps `POST /workflows/list` to `post_list_workflows`
   (`crates/aion-server/src/api/http/router.rs:369-378`, specifically line 377).
2. The HTTP handler converts the wire request and delegates to the shared
   visibility handler (`crates/aion-server/src/api/http/workflows.rs:171-180`).
3. The shared handler applies namespace authorization/scoping, constructs a
   `ListWorkflowsFilter`, and calls `engine.visibility_store().list_workflows`
   (`crates/aion-server/src/api/handlers/visibility.rs:39-69`, especially
   lines 54-60).
4. `VisibilityStore` is explicitly a **durable** visibility-query contract with
   an upsert method and a list method (`crates/aion-store/src/visibility.rs:12-36`).
   Its persisted `VisibilityRecord` contains a mutable projection field named
   `status` (`crates/aion-store/src/visibility.rs:46-68`, especially lines
   55-56), and that value is copied directly into the returned
   `WorkflowSummary.status` (`crates/aion-store/src/visibility.rs:71-109`,
   especially lines 77 and 96-97).
5. The LibSQL implementation persists the complete row with `INSERT OR REPLACE`,
   including encoded status (`crates/aion-store-libsql/src/visibility.rs:19-70`),
   reads list rows from that table (`crates/aion-store-libsql/src/visibility.rs:73-98`),
   and exposes those operations through the trait implementation
   (`crates/aion-store-libsql/src/visibility.rs:128-143`).

Therefore the list summary is neither a live registry lookup nor a fresh history
projection. It is the currently stored visibility projection.

### Describe takes a different, history-authoritative path

`POST /workflows/describe` is adjacent in the router
(`crates/aion-server/src/api/http/router.rs:377-379`) but does not share the list
read mechanism. Its HTTP handler delegates to `handlers::describe`
(`crates/aion-server/src/api/http/workflows.rs:268-285`). The shared describe
handler reads the selected workflow's full event history and constructs its
summary with `WorkflowSummary::from_history`
(`crates/aion-server/src/api/handlers/describe.rs:41-74`, especially lines
55-64).

This split means list and describe can disagree while a visibility row is stale:
describe re-projects from history, while list returns the cached durable row.

## The visibility writer and projection rule

There is an actual mutable status value in the visibility table, but no arbitrary
status setter in its engine writer path. The engine's lifecycle helper reads the
workflow's complete history, builds the record from that history, and only then
calls `record_visibility` (`crates/aion/src/lifecycle/visibility.rs:13-28`). The
builder sets `VisibilityRecord.status` by calling
`status_from_events(history)` (`crates/aion/src/lifecycle/visibility.rs:68-84`,
specifically line 78).

The core projection scans lifecycle events from newest to oldest. A start,
reopen, or resume projects `Running`; the five terminal lifecycle events project
their matching terminal statuses; pause projects `Paused`; non-lifecycle events
are ignored (`crates/aion-core/src/status.rs:49-105`). In particular,
`WorkflowFailed` is the only event in that projection that produces `Failed`
(`crates/aion-core/src/status.rs:68-78`, specifically line 72).

Recorder operations that refresh visibility use the same ordering and
derivation:

1. append the event to the event store
   (`crates/aion/src/durability/recorder.rs:1011-1022`);
2. afterward, reread the full history and call the visibility-recording path
   (`crates/aion/src/durability/recorder.rs:1024-1031`; representative
   append-then-refresh call sites are lines 477-484 and 507-513);
3. build the row from that history, including
   `status: status_from_events(history)`
   (`crates/aion/src/durability/recorder.rs:1050-1085`, specifically line 1079).

LibSQL then replaces the durable cache row through the `VisibilityStore` backend
cited above. Accordingly, the writer that stores `Failed` is the ordinary
history-derived visibility upsert, and the status it receives comes from a
recorded `WorkflowFailed` event.

## (b) What wrote `Failed` for the two alleged F2 specimens?

Nothing wrote `Failed` without a terminal event. The premise came from truncated
history observations.

The history endpoint defaults to 500 events and caps a request at 2,000
(`crates/aion-server/src/api/http/history.rs:17-19`). `limit` is an optional
request field (`crates/aion-server/src/api/http/history.rs:21-32`); the handler
applies the default and clamp (`crates/aion-server/src/api/http/history.rs:71-78`),
reads from the requested sequence, computes a continuation from the item at the
limit boundary, and truncates the returned tail to the requested limit
(`crates/aion-server/src/api/http/history.rs:79-113`, especially lines 102-103).

The original records demonstrate that boundary directly:

- `e4ad76e8-3bd5-4e54-8943-5e537875bb22` displayed `Failed`, but the collected
  prefix had exactly 500 events and ended at `ActivityScheduled`
  (`/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front/census-raw.json:1455-1466`).
- `70f3b331-54b4-4f24-8fdc-a54e51019f49` displayed `Failed`, but its collected
  prefix also had exactly 500 events and ended at `ActivityCompleted`
  (`/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front/census-raw.json:1603-1614`).

The in-run R1 census explicitly requested `limit=2000`; no workflow exceeded the
endpoint cap, so each history fit in one response
(`gate-logs/status-front-run/CENSUS.md:11-16`). That complete census found 143
consistent rows and zero F1 or F2 rows
(`gate-logs/status-front-run/CENSUS.md:26-34`). For the two named specimens it
found:

- `e4ad76e8-3bd5-4e54-8943-5e537875bb22`: 604 events, with
  `WorkflowFailed` at sequence 604 (`gate-logs/status-front-run/CENSUS.md:137`);
- `70f3b331-54b4-4f24-8fdc-a54e51019f49`: 605 events, with
  `WorkflowFailed` at sequence 605 (`gate-logs/status-front-run/CENSUS.md:147`).

Both terminal events were therefore beyond the old 500-event prefixes. The
observed list value `Failed` matched each complete history. There is no recorded
or source-level evidence for a writer that assigned terminal status in the
absence of a matching terminal event.

## (c) Why F1 could persist, and what re-derives it

### The banked observation is genuine

The main-tree disposition record states that all four `mm_capture_run` rows were
shown as `Running` although their last events included two `WorkflowFailed`
events. It explicitly calls out “WorkflowFailed history under a Running list
projection” (`/Users/tom/Developer/ablative/stack/aion/gate-logs/262-dig/orphan-worker-disposition.md:13-18`).
That is direct evidence of stale visibility projection, not an inference from
worker liveness.

### The permitted crash-consistency window

History append and visibility replacement are not one atomic transaction. In
the ordinary completion/failure path, the recorder writes `WorkflowCompleted`
or `WorkflowFailed` first; the source names that successful append as the durable
point and calls the later visibility upsert bookkeeping
(`crates/aion/src/lifecycle/completion.rs:411-429`). The visibility write occurs
after the recorder lock has dropped and is explicitly described as an
idempotent derived-index write that successor recovery can redo
(`crates/aion/src/lifecycle/completion.rs:521-536`). The retry arm names the exact
failure shape: an earlier attempt may have appended the terminal and then died
before upserting, so it rereads history and performs the upsert rather than
leaving a permanently stale index
(`crates/aion/src/lifecycle/completion.rs:449-466` and lines 490-499).

The recorder's own visibility-refreshing operations show the same separation:
the event-store append is at `crates/aion/src/durability/recorder.rs:1011-1022`,
while the later full-history reread and visibility update are at lines 1024-1031.
For those operations, a projection failure is deliberately nonfatal and the log
explicitly names the crash-consistency window until reconciliation
(`crates/aion/src/durability/recorder.rs:1034-1045`). An interruption between the
operations has the same persisted shape: newer history with an older visibility
row.

That mechanism is sufficient to explain how `Running` can remain stored after a
terminal event exists. The evidence does **not** identify whether the four wild
rows arose from a process interruption, a visibility-store error, or some other
historical implementation defect. Claiming a more specific specimen cause would
be guessing.

### Startup and adoption reconciliation

Cold startup invokes visibility reconciliation before active-workflow
repopulation (`crates/aion/src/engine/startup.rs:64-86`, reconciliation at
lines 70-74 and repopulation at line 85). The adoption path also invokes it
(`crates/aion/src/engine/startup.rs:103-110`).

Reconciliation:

1. reads every existing visibility summary;
2. enumerates every workflow ID from the event store;
3. reads each workflow's full history;
4. rebuilds the complete visibility record; and
5. overwrites the durable row when it differs

(`crates/aion/src/lifecycle/visibility.rs:31-65`). The rebuilt status again comes
from `status_from_events(history)` (`crates/aion/src/lifecycle/visibility.rs:68-84`).
This is the mechanism that should repair a stale F1 row on startup/adoption.

The current R1 observation's zero-F1 result
(`gate-logs/status-front-run/CENSUS.md:26-34`) establishes that no divergence was
present at that later boundary. The sources establish that reconciliation can
produce that repair, but the available observation does not isolate the exact
operation that repaired each earlier row; no stronger causal claim is warranted.

## (d) Invariant-4 audit: every mutable status field in the chain

### 1. Durable visibility status: yes, as a projection cache

`VisibilityRecord.status` and `WorkflowSummary.status` are mutable/persisted
projection values (`crates/aion-store/src/visibility.rs:46-68` and
`crates/aion-store/src/visibility.rs:87-109`). The LibSQL visibility backend
stores the value through `INSERT OR REPLACE`
(`crates/aion-store-libsql/src/visibility.rs:19-70`). This field is the direct
source of `/workflows/list` output, but it is not lifecycle authority: every
engine construction path audited above derives it from complete event history,
and startup/adoption reconciliation overwrites divergent rows from history.

### 2. Active-registry handle status: yes, as process-memory cache

A live `WorkflowHandle` also contains `cached_status`. Its constructor input is
described as a cached projection (`crates/aion/src/registry/handle.rs:96-118`,
especially lines 110-111); the handle documentation says it is reconciled from
the durable event projection and kept separate from engine residency
(`crates/aion/src/registry/handle.rs:120-137`). Construction copies the input into
the private field (`crates/aion/src/registry/handle.rs:146-167`), and the public
reader returns it (`crates/aion/src/registry/handle.rs:317-321`).

The registry's replacement operation first computes
`status_from_events(events)` and then invokes the handle's restricted
`replace_projected_status` method
(`crates/aion/src/registry/table.rs:222-243`). That mutation method is documented
as replacing the cache with the durable event projection result and simply
assigns that result (`crates/aion/src/registry/handle.rs:348-351`).

Recovery likewise reads full history, calculates `status_from_events`, and uses
that projection to decide whether a workflow is terminal, paused, or eligible
for recovery (`crates/aion/src/engine/startup.rs:172-215`, especially lines
194-213). When a resident is registered, the already projected value is copied
into `WorkflowHandleParts.cached_status`
(`crates/aion/src/engine/startup.rs:295-312` and
`crates/aion/src/engine/startup.rs:339-371`, specifically line 367).

This handle cache is not on the `/workflows/list` read path; the list handler
reads the visibility store. It is a second derived cache used by active engine
operations, not a stored lifecycle authority.

### 3. Authoritative stored status: none

The audit found two mutable status-bearing representations:

| Representation | Residency | Consumer | Derivation |
|---|---|---|---|
| `VisibilityRecord.status` / `WorkflowSummary.status` | durable visibility store | `/workflows/list` | full event history via `status_from_events` |
| `WorkflowHandle.cached_status` | process memory | active engine/registry operations | durable event projection via `status_from_events` |

Neither is independently authoritative. The actual durable lifecycle facts are
the events. `WorkflowStatus` itself documents that status must be obtained only
by projection from event history and never assigned or stored as an independent
mutable field (`crates/aion-core/src/status.rs:7-11`). Within the audited engine,
server, and store chain, no separate authoritative status column or
non-history-derived terminal-status writer was found.

## Answers, condensed

- **(a)** `/workflows/list` returns the durable visibility-store summary; it does
  not project history at read time. `/workflows/describe` independently reads
  and projects full history.
- **(b)** No writer set `Failed` without history. The two supposed F2 histories
  were 500-event prefixes; their matching `WorkflowFailed` events are at
  sequences 604 and 605.
- **(c)** F1 can persist because the authoritative event append precedes a
  separate, nonfatal visibility upsert. Startup/adoption reconciliation rereads
  full histories and overwrites divergent visibility rows. The precise wild
  interruption is not established by the evidence.
- **(d)** Mutable status exists only as two derived caches: a durable visibility
  projection and an in-memory handle projection. Event history remains the
  lifecycle authority.
