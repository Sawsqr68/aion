# Ops console guide

The ops console is a single-page web application embedded in the `aion`
binary. It is part of the product: a plain `cargo build` or `cargo install`
ships the real UI, served at `/` on the HTTP listener — there is no cargo
feature and no "not embedded" state
(`crates/aion-server/src/ops_console/assets.rs`). With the default
configuration that is <http://127.0.0.1:8080>.

```sh
aion server --open
```

`--open` resolves the served URL from the effective config (the same
`--listen-address`/config-file/default precedence the server uses), waits
for the HTTP listener to accept a connection, and opens the default
browser. It is best-effort: if the listener never comes up or there is no
opener, the failure is logged and the server run is unaffected
(`crates/aion-cli/src/server.rs`).

The console router is merged **after** the public API and WebSocket
routes, so assets can never shadow an API endpoint; unknown paths fall
back to `index.html`, which is why a hard refresh on any console route
(`/authoring`, `/assistant/<id>`, …) works.

## The shell

Every view shares the same chrome:

- **Namespace selector** — most views are scoped to the selected
  namespace and show an explicit "no namespace selected" state until you
  pick one.
- **Connection indicator** — the live event-stream (WebSocket) status:
  *Connected*, *Reconnecting*, *Disconnected*, or *Live with possible
  gap* after a resync. When the stream is degraded, the last typed socket
  error is rendered under the badge.
- **Command palette** (`⌘K` on macOS, `Super+K` on Linux; also the chip
  in the footer) — fuzzy search over live entities (workflows, assistant
  sessions, namespaces, packages, workers) and verbs. `Tab` cycles the
  entity mode; selections persist to a recents section. Every result is a
  deep link — the palette navigates to the surface that performs an
  operation and never mutates anything itself.

Global keyboard shortcuts (all rebindable; the palette chip shows your
real binding): `g w` workflows, `g n` namespaces, `g a` assistant. `mod`
is `⌘`/`Super`, never Control.

## Workflows (`/`)

The workflow list for the selected namespace: one row per workflow with
ID, type, status, started, and ended. You can:

- **Filter** by workflow type (free text), status, and a started
  after/before window.
- **Page** through results with cursor-based pagination.
- **Watch it live** — rows transition (Running → Completed) and new
  workflows appear without a refetch. Off the first page, new arrivals
  surface as an "N new workflows started above" banner with a jump-to-top
  action rather than mutating the page you are reading.

Selecting a row opens the run's detail view.

## Workflow detail (`/workflows/:id`)

The recorded history of one run, backfilled from the history API and then
kept live from the event stream. The view offers a **List ⇄ Swimlane**
toggle:

- **List** — the event timeline, entry by entry, with payloads.
- **Swimlane** — activity/timer/child lanes over time, with a scrubber.
  Selecting a bar opens a bottom-docked detail sheet below the chart.

Selection and view mode live in the URL (`?seq=N`), so any view is a
shareable link. Below the timeline, the **attempt navigator** enumerates
the durable activity attempts; selecting one streams that attempt's
transcript (for agent-backed activities), and a terminal run's transcripts
remain cold-loadable from durable retention. While an attempt is live, the
intervention controls can inject a message into it via
`POST /workflows/intervene`; the server's ack (`Applied`,
`CapabilityNotSupported`, `StaleTarget`) is reported verbatim.

A run whose projected terminal outcome is **Failed** or **Cancelled**
shows a **Reopen** action. It renders a before/after preview computed
purely from recorded history — green rows are recorded results that will
be reused, amber rows will re-dispatch, struck rows are the superseded
terminal failure — and committing posts `POST /workflows/reopen`. A
rejection (non-reopenable state, absent workflow, authorization) is shown
as a visible error, never reinterpreted as success; the live stream then
drives the run back to Running.

## Namespaces (`/registry`)

The live durable namespace registry (`GET /namespaces/records`), updated
from socket deltas — new namespaces appear the moment a worker registers,
a workflow starts, or one is created explicitly, with no refresh. Columns:
name, placement, quota (a live in-flight/ceiling badge), created, last
seen, and origin (worker register / workflow start / explicit create /
inferred from state).

Two mutating controls live here: **create a namespace**
(`POST /namespaces`) and **edit a namespace's placement**
(`PUT /namespaces/{name}/placement`).

## Actions (`/actions`)

The mutating operator surface. Success is shown only after the server
confirms.

- **Start a workflow** — workflow type, JSON input, and namespace. The
  namespace is the selected one when set; otherwise you type it free-form
  and `POST /workflows/start` mints it on use (open registration policy),
  so a fresh server with no namespaces is not a dead end. The success
  panel renders only after the server returns real
  `workflow_id`/`run_id` values, and deep-links to the run.
- **Deploy a package** — upload a `.aion` archive and view the loaded
  workflow versions. This panel is rendered only when the server actually
  grants the caller deploy authority, discovered at runtime from
  `GET /whoami` — not a build-time flag. A cluster running with
  `[deploy] enabled = false` answers 404, which the panel renders as an
  explicit "deploy disabled" state.

Starting a run and deploying a package are distinct authorities: start is
namespace-scoped, deploy is deployment-scoped.

## Assistant (`/assistant`)

Round-based agent sessions on the deployed `assistant` workflow type. The
root view starts a new session (objective + optional repository path,
submitted as a normal `POST /workflows/start` of type `assistant`) and
lists existing sessions live.

A session view (`/assistant/:id`) shows the live attempt transcript with a
chat dock beneath it. The dock's verb follows the session's state: while a
round is live your message is injected into the running attempt (the same
intervention path as the workflow detail view); when the round has ended
and the workflow is parked awaiting continuation, your message is sent as
the `assistant_continue` signal to start the next round. Ending the
session sends the same signal with an `{ end: true }` payload — the
workflow listens on exactly one signal name. All state is history + socket
driven; there is no polling.

This surface presumes a workflow of type `assistant` implementing that
contract is deployed on the server; without one there are no sessions to
list and a started session will not behave as a session.

## Authoring (`/authoring`)

The AWL studio, split into two deliberately separate surfaces:

- **Workspace** — the editable AWL documents in the server-side workspace
  directory (default `<AION_HOME>/authoring`, overridable with
  `authoring.workspace_dir` / `AION_AUTHORING_WORKSPACE_DIR`). You can
  create documents, edit with syntax highlighting and live check
  diagnostics (the buffer is checked server-side as you type), format, and
  save (`mod+s`). The open document lives in the URL (`?doc=path`), so
  any view is one shareable link. View modes (`mod+1`..`mod+4`): text,
  canvas, split, and studio. The canvas and studio apply structured edits
  — add step, add action contract, connect routes and fall-throughs, edit
  prose, rename bindings, delete steps — each printed by the server and
  applied to the editor as a single undo unit. A worker scaffold generator
  is included. The **run** panel drives the full guided pipeline: check →
  compile → package → deploy → start → watch, refusing up front when the
  buffer does not check green or no worker is connected to the target task
  queue, and then watching the started run live. Run mode watches the
  *deployed* revision and flags drift from the editor buffer.
- **Deployed** — read-only AWL source carried by the packages actually
  deployed on this server, keyed by workflow type and immutable content
  hash (`?type=…&version=…`). Nothing here writes, and a deploy never
  writes into your workspace — the two surfaces can legitimately
  disagree, which is why they are not one list with a badge.

Every example in the language guide (the **Language** tab and the floating
guide overlay alike) carries a **Try it** button: it creates a workspace
document from the example's verbatim source — named after the example's own
workflow or worker, collision-safe with a `_2`/`_3` suffix when the name is
taken — and lands you on it in the editor, ready to check and deploy — and
to run, when the example declares a workflow.

If the server has no authoring workspace configured, the workspace side
says so explicitly. The server-side Gleam authoring loop
(`--gleam-path` + `--authoring-project-root`) is a separate, optional
facility and is not required for the AWL studio.

## Incidents (`/incidents`)

The three-AM triage view: a ranked list of incident cards for the selected
namespace — failures and timeouts first, then stuck-running workflows —
each with a single deep-link action into the relevant view. Incident
classes the server does not yet feed (worker / outbox / shard / fenced)
are rendered as honest "awaiting server support" rows, never fabricated
counts. This view is entirely read-only.

## Failover (`/failover`)

A purpose-built demonstration surface for the multi-node failover demo,
not a general cluster dashboard. It renders the node grid with live
liveness and worker counts from the cluster event stream, the fan-out
progress bar, the adoption moment, an exactly-once completion counter, and
an event log, for a specific fan-out demo workflow (`collect_four`, seed
arity 4). The cluster shape and workflow can be seeded through URL query
parameters; without a seed it discovers the most recent workflow.
`?mode=fallback` renders a deliberately minimal tier that depends only on
HTTP polling (`/health/live` + history describe) so the headline numbers
stay honest even if the WebSocket path is down.

## Configuration

The console is configured by the `[ops_console]` section
(`crates/aion-server/src/config/sections.rs`); the legacy section name
`[dashboard]` still parses via a serde alias.

```toml
[ops_console]
# Default: the compile-time embedded bundle. To serve a bundle from disk
# (e.g. a dev build) instead:
source = { FileSystem = { asset_path = "/srv/aion/ui" } }
```

Validation requires a non-empty `asset_path` whose directory contains
`index.html`. The embedded bundle lives in `ops-console-embed/`
(committed, regenerated with `cargo xtask build-ops-console`); the console
source is `apps/aion-ops-console/`.

## What mutates and what does not

The console is read-mostly: almost every surface is a projection of
recorded history plus the live event stream. The complete set of mutating
actions is:

| Surface | Action | Endpoint |
|---|---|---|
| Actions | start a workflow | `POST /workflows/start` |
| Actions | deploy a package | deploy API (grant-gated) |
| Workflow detail | reopen a failed/cancelled run | `POST /workflows/reopen` |
| Workflow detail / Assistant | inject a message into a live attempt | `POST /workflows/intervene` |
| Assistant | continue / end a session | signal `assistant_continue` |
| Namespaces | create a namespace | `POST /namespaces` |
| Namespaces | change placement | `PUT /namespaces/{name}/placement` |
| Authoring | create/save/format documents, apply structured edits, save layout | `/awl/*` |
| Authoring | guided ship-and-run (deploy + start) | deploy + start APIs |

Everything else — lists, timelines, transcripts, incidents, failover —
only reads. Mutating affordances the server does not grant or
mount are hidden or rendered as explicit disabled states, never faked.
