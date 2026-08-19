# The estate's dispatch layout — namespaces, queues, placement, and numbers

**2026-08-19, Vesper.** For Waffles the Terrible to put in front of Tom.
Design only; nothing here is built. Every mechanism claim below was read from
the code today, and where a mechanism does **not** exist I say so rather than
designing on top of it.

The release-through-aion workflow is a tenant of this layout, so this comes
first. Designing the release chain before the building would be furnishing a
room whose walls aren't decided.

Tom's words are used throughout: **quick** and **long** queues, first-come
first-serve inside each. Anything fancier — ranking, preemption, balancing — is
Fulcrum's territory when the load-balancer work comes back around, and is
deliberately out of scope here.

---

## 1. The three axes, and what each one is for

Aion routes on `(namespace, task_queue, node)`. They are not three flavours of
the same thing, and the commonest mistake would be using one to do another's job.

| Axis | What it is | What it is NOT |
|---|---|---|
| **namespace** | The grouping. A tenant, a programme, a blast radius. Carries the placement directive. | Not a priority. Not a machine. |
| **task_queue** | A **queue of work** — `quick` or `long`. Part of the pool address, so a worker pulling one never sees the other's work. | **Not a worker type.** Not a ranking. |
| **node** | A **locality label**, matched at selection time. A within-pool filter. | **Not part of the pool key.** Not a process id. |

A worker's identity is just which queues it pulls plus which activities it
implements. Nothing in the dispatch path cares what kind of thing it is.

### Quick and long is the right split, and not for the obvious reason

Splitting by **how long a job holds a worker** is mechanically better than
splitting by how urgent it is, and the difference is not a matter of taste.

There is no priority notion anywhere in dispatch, the outbox, or the registry.
Nothing is ranked and nothing preempts, so a queue cannot deliver urgency — it
can only deliver **isolation**: work in the quick queue is never stuck behind
work in the long queue, because it was never in the same line. First-come
first-serve within each queue is therefore not a compromise, it is the whole
mechanism stated honestly.

An urgent/routine split would have promised something the engine cannot do. A
quick/long split promises exactly what it does. And as §2 shows, duration is
*also* the thing that determines whether a worker gets mistaken for a dead one —
so the same split that gives isolation is the one that makes the liveness
numbers tractable. That is a rare piece of luck and it is worth taking.

### Placement is real, and it is enforced twice

Node affinity is declared on the **namespace** as `NamespacePlacement`:

- `Unplaced` — dispatch to any live worker. The default.
- `Prefer { nodes }` — **soft**. Preference tiers at selection, spilling to any
  live worker when no preferred label is up.
- `Pinned { nodes }` — **hard**. A required label set: dispatch stalls rather
  than spilling. Isolation over availability.

Two enforcement points, and they matter separately:

1. **At registration** — a worker whose advertised `node` is absent from a
   `Pinned` namespace's label set has its **whole registration rejected**,
   loudly, naming the namespace, the node, and the required set. A
   wrongly-labelled worker cannot even sign up.
2. **At dispatch** — `Pinned` resolves to a required set and waits; everything
   else resolves to preference tiers with a spill.

I nearly told the estate the opposite. The doc comment on the type said "this
slice is storage only — nothing reads `Prefer`/`Pinned` yet", which was true once
and has not been for some time; the readers live in a different crate, which is
how the stale claim survived. Corrected in `738eb2142`. **A single-box queue is a
coincidence; `Pinned` is a mechanism.** If we want work to stay on a box, say so
with placement.

---

## 2. Concurrency, and the trap in sizing it

`WorkerConfig.max_concurrency` is **per worker process**, **required with no
default** (the builder errors; the serve loop refuses zero), and enforced as a
`tokio::Semaphore` in the serve loop. Not per queue, not per activity type. Two
workers at 3 is six slots; there is no pool-level ceiling above them.

`[namespaces] max_in_flight_activities` (default 1024) looks like the per-tenant
ceiling and **enforces nothing** — its own documentation says "config + record
field only". It must not travel in a sizing conversation as though it worked.

### The hazard that decides the numbers

An **exhausted** concurrency budget parks the serve loop awaiting a permit
*inside its stream arm*, before spawning. While every permit is held the loop
reads no further frames — including the liveness ping. **A worker saturated flat
out is indistinguishable from a worker that has died**, and the heartbeat sweeper
will withdraw it for doing its job.

The live server's own figures: heartbeat window **30 s**, sweep every **7.5 s**.
A worker whose permits are all held for longer than 30 s is a candidate for
withdrawal mid-activity.

The test that pins this names the cure exactly: *two permits — one for the held
activity, one spare*. A ping behind a running activity is answered while it runs,
**as long as there is a spare permit**.

### The sizing rule that follows

> **`max_concurrency` is a headroom setting, not a throttle.**
> Set it to the work you want in flight **plus at least one**. Take the throttle
> from how many workers you run, never from saturating one.

Sizing `max_concurrency` to exactly the desired concurrency guarantees a worker
hitting its target load stops answering. That is the failure we would otherwise
have found live, mid-publish.

**This is where quick/long pays for itself twice.** A quick worker's permits turn
over well inside the 30-second window, so its exposure is small whatever the
numbers. The long queue is where the spare permit is genuinely load-bearing — and
because long work is now segregated, that is a small, named set of workers to be
careful with instead of a property of the whole estate.

---

## 3. The proposed layout

Tom's constraints: a couple of activities at a time as the throttle feel, some
degree of node affinity, and the thing-finder pipeline as the probable first
workload.

### Namespaces

| Namespace | For | Placement |
|---|---|---|
| `release` | The publish chain — crate waves, liminal-rs, beamr, future cuts | `Pinned { release-box }` |
| `pipeline` | thing-finder and the data/OCR work behind it | `Prefer { workshop-box }` |
| `default` | Ad-hoc runs, probes, canaries, anything not yet worth its own tenant | `Unplaced` |

`release` is `Pinned`, not `Prefer`, deliberately. Publishing needs credentials
and a known toolchain; a spill to another box is not a degraded publish, it is a
*wrong* publish, and stalling until the right box is up is the correct answer.
The registration gate is the real prize — a worker on the wrong box cannot join
the release pool at all, so the failure surfaces at sign-up rather than three
legs into a chain.

`pipeline` is `Prefer` because thing-finder work spilling to another box is
slower, not wrong.

### Queues and workers

| Pool | Carries | Workers | `max_concurrency` | In flight | Spare |
|---|---|---:|---:|---:|---:|
| `release/quick` | Version bumps, tag pushes, index checks | 1 | 3 | 2 | 1 |
| `release/long` | The publishes themselves — anything that waits on a registry | 1 | 3 | 2 | 1 |
| `pipeline/quick` | Interactive work — a stocktake someone is standing in front of | 2 | 3 | 4 | 2 |
| `pipeline/long` | Batch OCR, backfills, sweeps | 2 | 3 | 4 | 2 |
| `default/quick` | Probes, canaries, ad-hoc runs | 1 | 3 | 2 | 1 |

Seven worker processes, 21 permits: 14 the intended working set, 7 held back so
no worker can saturate itself into looking dead.

Three is the smallest number that gives two in flight and a spare. I have not
padded past it — the spare permit is a correctness requirement, and anything
above it is throughput nobody has asked for. If the real throttle wants to be
four in flight, it moves to `max_concurrency = 5`, **not** to more workers.

`default` gets no long queue until something needs one. An empty queue is a
worker budget doing nothing.

**The release pools are single-worker on purpose.** Publish legs have ordering
constraints between crates; a second worker in the same queue buys parallelism
the chain cannot use and adds a second process that can die mid-leg.

---

## 4. What this layout does not give, and must not pretend to

- **No ordering within a queue.** If the release chain needs crate B strictly
  after crate A, that ordering lives in the **workflow**, as steps — not in the
  queue. This is the most important thing for the release design to inherit, and
  I will build it into that document rather than let a queue appear to do it.
- **No per-tenant activity ceiling.** `max_in_flight_activities` is not wired.
  The only real ceiling is workers × `max_concurrency`.
- **No preemption, no ranking, no balancing.** Nothing an operator does makes a
  running activity yield; cancellation is the only lever and it is cooperative.
  This is the Fulcrum boundary Tom drew, and the layout is built to sit under it
  rather than to grow into it.

## 5. What I would want measured before this is built

The numbers above are derived from a stated throttle and a proven hazard, which
makes them defensible, not verified. Two measurements turn them from reasoning
into figures:

1. **How long a thing-finder activity actually holds a permit.** Comfortably
   under 30 s and the spare-permit rule is belt-and-braces. If OCR runs long, the
   spare permit is the only thing between us and workers withdrawn mid-job, and
   the quick/long split stops being a convenience.
2. **Whether the release legs are long enough to matter.** A crate publish
   waiting on the index is exactly the shape that holds a permit while doing
   nothing, and it is the leg where a withdrawal would cost most.

Both are one instrumented run each, and both are cheaper than finding the answer
during a publish.
