# A second document on a worker's queue silently un-serves it

**Found:** 2026-08-06, during the self-run of Tom's from-scratch tutorial (#146).
**Binary:** `aion 0.11.0` (`/Users/tom/.cargo/bin/aion`), the published release.
**World:** isolated — `AION_HOME` and ports of its own (`:8099` HTTP, `:50099`
gRPC), absolute `store.data_dir`. Tom's live pids 45425 / 48399 untouched
throughout.

## The claim

**A worker is admitted only if it advertises the full action set that *every*
deployed document declares for its queue.** So deploying a second document that
reuses a worker queue with a different action makes that queue unservable by
any single-document worker — and **nothing anywhere says so**. The worker
process stays alive, writes zero bytes, and the server logs neither a warning
nor an error.

This is the natural second move for a beginner: write one workflow, then write
another that reuses the same `worker` block. It is exactly what a follow-along
tutorial would walk someone into.

## The measurement

One worker, one manifest file, one document — none of the three edited between
the two probes. The only variable changed is *what else is deployed*.

| # | State | Probe: `POST /awl/workers/availability` on queue `desk2` | Result |
|---|---|---|---|
| 1 | `slow_flow2` deployed (declares `desk2` → `slow_look`); worker started | `connected_workers` | **1** — serving |
| 2 | `other_flow` deployed too (declares `desk2` → `other_look`); **same worker still running** | `connected_workers` | **1** — a live worker is *not* evicted |
| 3 | Same worker killed and **restarted, byte-identical command and files** | `connected_workers` | **0** — refused, permanently |
| 4 | Remedy: `both_flow` declares `desk2` → `slow_look` **and** `other_look`; worker started against it | `connected_workers` | **1** — serving again |

Rows 1 and 3 are the same worker invocation against the same files. Row 2
isolates the seam: the refusal is at **registration**, not eviction — which is
why it does not appear when you deploy, only later when the worker next
restarts. Row 4 is the reversal: restoring the full action set in one document
restores service, so the queue name is not poisoned, the *contract* is.

An independent control ran first: with only `first_flow` (queue `desk`)
deployed, its worker registered on the first attempt (`connected_workers: 1`)
and a full run completed. The refusal is not a property of the world, the
binary, or the shell worker.

## The silence, quantified

- **Worker side:** `wc -c` on the worker's log = **0 bytes**, for every refused
  start. The process remains alive (verified by `pgrep`) and keeps redialling.
- **Server side:** the whole `server.log` for the session held **0 lines** at
  `ERROR` and **0** at `WARN`. The only worker-stream lines present are two
  `INFO` "worker stream closed with status … BrokenPipe" entries whose
  timestamps match my own `pkill`s — they are my kills, not refusals. A refused
  registration produced **no log line at all** and never got a `WorkerId`.
- **Operator's only signal:** `POST /awl/workers/availability` returning
  `{"available": false, "connected_workers": 0}` with the generic hint *"No
  connected worker serves task queue `desk2`. Scaffold and run this worker from
  Workers & Actions, then retry start."* — which describes the wrong problem.
  The worker *is* run; it is refused. Following that hint cannot fix it.

The documented rule *"Silence from `aion worker shell` is not success"*
(`docs/workers/SHELL-WORKER-MANIFEST.md`) is therefore load-bearing, and the
availability probe it points at is the only instrument that sees this.

## What is not established

- **Which code path refuses.** I read no engine source for this; the verdict
  above rests entirely on black-box behaviour with a control and a reversal.
  The mechanism should be confirmed against the admission seam before any fix.
- **Whether this is a regression.** Task #78 ("contract admission demanded the
  WHOLE queue of every connection") is recorded closed, and #80 ("one stale
  deploy made a whole task queue unservable") likewise. The behaviour here is
  the same *shape* as both. Whether the fixes covered a different axis
  (node-partitioning in #78's case) or whether this regressed is **unmeasured**
  — I did not date-check it.
- **Whether an accumulated older version counts.** Deployed versions accumulate;
  I did not test whether unloading or re-routing the conflicting document
  restores admission, only that a merged document does.

## Consequence for the tutorial (#146)

The tutorial teaches **one document per worker queue, carrying that queue's
full action set** — and teaches the availability probe as the way to confirm a
worker actually arrived, because the worker's own silence proves nothing. Both
are now measured, not assumed.

## Also observed in the same self-run

- **The store's path-safety gate refuses a world-writable ancestor.** A
  `store.data_dir` anywhere under `/private/tmp` (mode `1777`) is refused at
  startup with a clear, actionable message naming the offending ancestor. This
  is correct behaviour, and it means a tutorial cannot site a project in `/tmp`.
- **`authoring_root` resolves to `$AION_HOME/authoring`**, printed at startup.
  That answers "where does the new document land" — the gap Tom named — for any
  server started with `AION_HOME` set.
- **`GET /workflows` without `namespace`** answers `Failed to deserialize query
  string: missing field 'namespace'` — a raw deserializer message on a
  documented operator surface.
- **`"parent": null`** appears on every listing row, which is #107 showing up
  live in the listing the console reads.

---

# AMENDMENT 1 — the mechanism is read, and the silence is explained

Added 2026-08-06 after Waffles' disposition (drive it, and *the admission
refusal must name itself server-side whatever else changes*). Tree `d78a38308`.

## The refusal is real, rich, and never logged

The gate is **not** missing. `crates/aion-server/src/worker/contracts.rs:120`
`validate_worker_contracts` builds `ContractAdmissionError::Mismatch`, whose
own doc comment (`contracts.rs:73-79`) says the whole diagnosis rides in
`details` — which package version, why it was held, which action, which field,
expected vs advertised, and the operator remedy — because *"A refused worker
retries silently forever otherwise, and the message is the only thing standing
between a two-minute fix and an hour of guessing."*

**That message is produced and then thrown at the wire.**
`crates/aion-server/src/api/worker_grpc.rs:67`:

```rust
validate_worker_contracts(&self.state, &register)?;
```

and the mapping at `worker_grpc.rs:659-666` turns `Mismatch` into
`Status::failed_precondition(error.to_string())`. The `?` propagates that
`Status` straight out of the streaming handler. **No `tracing` call anywhere on
this path**, so the server emits nothing — which is exactly the measured
result: 0 ERROR, 0 WARN, no line at all, and no `WorkerId`, because the
refusal happens *before* `accept_registration` at `worker_grpc.rs:73`.

The diagnosis therefore exists in full and is delivered only to the worker.

## Why the worker is silent too — the second half of the pair

`docs/workers/SHELL-WORKER-MANIFEST.md` already records that
`aion worker shell` **installs no tracing subscriber**, so the redial driver's
failures "go nowhere and the process just sits there." The refusal Status
arrives, the redial loop swallows it, and the manifest doc's own warning —
*"Silence from `aion worker shell` is not success"* — is the documented
consequence.

⇒ **The silence is not one defect but two halves meeting:** the server declines
to log a refusal it has fully diagnosed, and the client that receives it has no
subscriber to print it. Either half alone would have left a trail. Fixing only
the client would still leave an operator with no server-side record; fixing
only the server satisfies Waffles' standing condition. **Both should move.**

## Axis: this is NOT a regression of #78 — it is the narrowing working correctly

`contracts.rs:102-119` documents two deliberate narrowings, "because a worker
that is doing its whole job must not be refused": only **reachable** package
versions are held (that is #80's axis — a stale non-routed deploy no longer
poisons a queue), and within those, only the actions whose dispatch can reach
this connection's **node** (that is #78's axis — node-partitioned workers).

My case defeats **both** narrowings legitimately. Both documents were freshly
deployed and routed (`route_changed: true` on each), so both are reachable —
narrowing 1 does not apply. Neither declared a node, so every action is
reachable from the connection — narrowing 2 does not apply. The gate then
correctly demands the union and correctly refuses a worker offering half of it.

**So the refusal is the designed behaviour, not a regression.** The defect is
narrower and sharper than the original write-up implied:

1. **The refusal is invisible server-side** (Waffles' condition — a violation
   of no-silent-failures in its purest form, and the worst variant, because a
   registration-time refusal is detached from the deploy that caused it, so the
   operator debugging it is looking at the wrong event entirely).
2. **The availability hint names the wrong remedy** — *"Scaffold and run this
   worker … then retry start"* to an operator whose worker is already running
   and being refused.
3. **Whether one queue shared across documents should be refusable at all is a
   design question**, not a bug: content-hash namespacing means a queue
   accumulates every reachable version's demands, so "one document per queue"
   is currently a load-bearing authoring rule that nothing states or enforces.

## Contradiction resolved: task #94 is about a different path

Task #94 records *"a contract-refused worker reconnects at 2/second forever and
the server re-logs the full refusal each time — 12MB/hour."* That is the
opposite of the measured silence here, and both are true: #94 is the
**liminal** transport's admission call
(`worker/liminal_transport.rs:1568`), which does log; this is the **gRPC**
registration path (`api/worker_grpc.rs:67`), which does not. Two callers of one
gate, opposite observability. That divergence is itself the finding — the same
rule known in two places, disagreeing.

## Still not established

- ~~I have not read `liminal_transport.rs:1568`'s logging in detail~~ —
  **discharged in amendment 2 below.**
- Nothing was built, run, or tested for this amendment — it is a source read
  against an already-measured behaviour.

---

# AMENDMENT 2 — the liminal half is read, and this defect has been fixed before

Added 2026-08-06 while writing the fix brief
(`BRIEF-147-ADMISSION-REFUSAL-NAMES-ITSELF.md`). Tree `a5ce6c65d`. Source read
only; nothing built or run.

The contrast amendment 1 left unconfirmed is **confirmed, and stronger than #94
attributed.** The liminal path funnels every refusal through
`admit_registration` (`liminal_transport.rs:1592`) and names it at `WARN` in
`on_worker_registered` (`:1687-1712`) with connection pid, identity, queue,
namespaces, **both** advertised sets, and the structured reason — the same text
that rides back in the `Rejected` ack. It is pinned by
`tests/dead_man_switch_refusal_logs.rs:83`.

`admit_registration`'s own doc comment records why it exists:

> *Before this split the rejection reason was carried in the `Rejected` ack and
> logged NOWHERE: on 2026-07-29 a worker whose `.v4` contract field-mismatched
> was refused on every single dial with the server silent on all of them, and
> **the refused process looked merely asleep**.*

That is this defect, dated a week earlier, on the other transport. **It was
found, fixed and pinned once — and the gRPC path never received the fix.**
Nothing in the tree forces the two callers' observability to agree, so it
drifted, and only a black-box self-run on the untreated transport could see it.

One further detail sharpens it: `worker_grpc.rs:632-640` **does** log — a
`tracing::warn!` for the case where the contract check was *skipped* for want of
an engine handle. The refusal branch thirty lines below is the one branch with
no voice.

⇒ The defect is `one-truth-in-two-places-has-drifted` in literal form, and the
cure is subtraction: the naming belongs in the gate, not in a second copy of the
liminal log. That is R1 of the brief.

---

## RESOLVED — 2026-08-06

The naming moved **into** `validate_worker_contracts`, and the liminal caller
**stopped restating it**. One site down from two, not a second copy added. A
third transport cannot forget to log a refusal, because logging is no longer
something a transport does.

`dead_man_switch_refusal_logs.rs` was **amended, not replaced** — the message it
reads for changed from the caller's `REFUSED liminal worker registration` to the
gate's `REFUSED worker registration`; the property it pins is untouched, and it
still fails when the naming is removed. Both refusal-log pins were watched
failing with the `tracing::warn!` deleted and passing with it restored
byte-identically.

**#94 is closed in the same motion.** A refused worker redials forever, so the
gate emits its diagnosis when the refusal for a site is new or different and
meets an identical repeat with silence. That is a transition rule and not a rate
limit: no interval, no count, no cap, nothing configurable — NO ARBITRARY LIMITS
holds. Adding a log to the second transport without it would have given #94's
12 MB/hour a second host.

### The close-out found a second defect, in the fix's own bound

The first cut keyed the transition map on `(task_queue, node)` and its doc
called both *server-derived*. **Both arrive on the wire.** The design had named
that exact trap and then walked into half of it — `identity` was kept out of the
key, `node` was not. A refused worker varying its advertised node grew the map
per dial; measured under control, 100 dials produced 100 entries.

The site is now derived from the deployed catalog rather than the request, so
nothing a refused party can say allocates an entry. Full record, including the
two-direction control table, in
`BRIEF-147-ADMISSION-REFUSAL-NAMES-ITSELF.md` § AS BUILT.

**What generalises from it:** a bound is a claim about every dimension the
client can move, so it takes one witness per dimension. A single witness over
one dimension reads exactly like a proof of the whole claim — and the doc
asserting the other dimension was safe is what stopped anyone checking.

**R3 is unchanged and unbuilt.** Whether one queue shared across documents
should be refusable at all remains a design question held at Waffles' seat for
Tom. Nothing here changed admission semantics.
