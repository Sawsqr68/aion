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
