# Brief 147 — the admission refusal names itself, server-side, on every transport

**Task:** #147. **Defect record:** `SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md`
(measurement + amendment 1). **Disposition:** Waffles, 2026-08-06T00:06Z — mine
to drive, with one standing condition quoted in R1.

**Status of the axis check:** discharged. This is **not** a regression of #78 or
#80; both documented narrowings at `contracts.rs:102-119` apply correctly and my
measured case legitimately defeats both. The gate refusing is designed
behaviour. What follows is about the refusal being **unsayable**, not wrong.

---

## What is now established, at my own hand

Amendment 1 read the gRPC half. This brief adds the liminal half, which was
listed there as *"still not established"*, and it changes the shape of the work.

### One gate. Two callers. Opposite observability.

`crates/aion-server/src/worker/contracts.rs:120` `validate_worker_contracts` is
the single gate. Both transports call it, and both construct the same fully
diagnosed `ContractAdmissionError::Mismatch`.

| | liminal transport | gRPC transport |
|---|---|---|
| Call site | `liminal_transport.rs:1568` via `validate_registration_contract` | `worker_grpc.rs:67` via a local wrapper at `:625-666` |
| Funnelled through one admission fn | **yes** — `admit_registration:1592` | no |
| Server-side log on refusal | **`tracing::warn!` "REFUSED liminal worker registration"** (`:1687-1712`) with pid, identity, queue, namespaces, **both** advertised sets, and the reason | **none** |
| Pinned by a test | **yes** — `tests/dead_man_switch_refusal_logs.rs:83` | no |

### The gRPC path logs the case where it *skipped* the check, and stays silent on the case where it *refused*

`worker_grpc.rs:632-640` emits a `tracing::warn!` when the state carries no
engine handle and the contract check is therefore skipped. The module imports
`tracing` and already writes to it. The refusal branch — thirty lines below —
maps to `Status::failed_precondition` and returns. The silence is not an
oversight of plumbing; it is the one branch that was never given a voice.

### The liminal side already carries my incident, verbatim, dated 2026-07-29

`admit_registration`'s doc comment (`liminal_transport.rs:1592-1601`):

> *Split out of `ConnectionNotifier::on_worker_registered` so EVERY refusal —
> contract mismatch, unbound supervisor, registry error, lease failure — funnels
> through exactly one place that logs it. Before this split the rejection reason
> was carried in the `Rejected` ack and logged NOWHERE: on 2026-07-29 a worker
> whose `.v4` contract field-mismatched was refused on every single dial with the
> server silent on all of them, and **the refused process looked merely asleep**.*

That is the phenomenon I measured on 2026-08-06, on the other transport, in an
isolated world, with a control and a reversal. **This defect has already been
found, diagnosed, fixed and pinned once — on one of its two hosts.** The other
host never got the fix, and nothing in the tree could have noticed.

⇒ This is `one-truth-in-two-places-has-drifted` in its literal form: the
observability contract for an admission refusal is known at two call sites, with
nothing forcing them to agree. It drifted. **The cure is subtraction, not a
second copy of the liminal log.**

---

## R1 — the gate names its own refusal, once, for every caller

> **Waffles' standing condition, binding whichever way the axis check fell:**
> *the admission refusal must name itself, server-side, whatever else changes.*
> A worker refused at registration with zero bytes on either side is the
> no-silent-failures law in its purest form, and registration-time refusal
> detached from the deploy that caused it is the worst variant — the operator
> debugging it is looking at the wrong event entirely. *The crash is kinder than
> the silence.*

**The naming moves INTO `validate_worker_contracts`**, at the point it
constructs `Mismatch`. Not into the gRPC wrapper. A per-caller log is a
convention that a third transport can forget; a gate that speaks when it refuses
cannot be forgotten by anyone, because there is nowhere else to put the refusal.

The gate logs what the gate knows: `task_queue`, `node`, `identity`, and the
full `details` string whose own doc comment (`contracts.rs:73-79`) says it is
*"the only thing standing between a two-minute fix and an hour of guessing."*
It does **not** learn `connection_pid` or `namespaces` — those are transport
facts and belong to the transport's own record.

**Consequence for the liminal caller, and it must be faced, not deferred.**
Once the gate speaks, `on_worker_registered`'s `inspect_err` restates the
contract reason a second time. Under NO BACKWARDS COMPATIBILITY the answer is to
restructure, not to add alongside: the liminal caller keeps its log for the
refusal classes the gate cannot see (unbound supervisor, registry error, lease
failure) and stops restating the contract class.

`tests/dead_man_switch_refusal_logs.rs:83`
(`a_refused_registration_is_named_in_both_logs_on_the_first_dial`) asserts on the
literal string `"REFUSED liminal worker registration"`. Its **stated contract** is
that a refused registration is named on both sides on the first dial — which the
gate's line satisfies. **Amend that assertion to the gate's message; do not
delete the test, and do not weaken it to "some line was logged."** The pin's
subject is the naming, and the naming survives. Record the amendment against
this brief's commit, per the ratified-orders rule.

### R1a — repeats are silence because they carry no new information

#94 records the liminal path re-logging the full refusal on every redial —
2/second, 12 MB/hour, *"observability buried."* Adding a log to the gRPC path
naively gives that defect a second host. It must not ship that way.

**The rule is a transition, not a cap.** The full diagnosis is emitted when the
refusal for a given `(identity, task_queue, node)` is *new or different* —
first refusal, changed reason, or a refusal following a successful admission.
An identical repeat says nothing the first line did not, so it says nothing.

This is a semantic, not a configured value, and it invents no default —
**NO ARBITRARY LIMITS is not violated: there is no interval, no count, no cap.**
A time-based throttle would violate it, and is refused here.

Because the rule lives in the gate, **this closes #94 in the same motion rather
than spreading it.** That is the payoff of subtraction, and it is the reason to
do both together.

> 🟡 **Decision for Waffles, with my recommendation.** One motion (R1 + R1a,
> closing #94), or land gate-logging alone and take #94 separately?
> **Recommend one motion.** Splitting it means knowingly landing a defective log
> volume on a second transport, and the fix for that volume is the same three
> lines in the same function.

### R1b — the transition state needs an owner, and this is the real implementation cost

`validate_worker_contracts` is a free function with no state. Transition
tracking needs somewhere to live. **Pass a handle in as a parameter** — both
callers already assemble this call, and both have somewhere to hold it
(`ServerState` for gRPC, the notifier for liminal). Keying is
`(identity, task_queue, node)` against a digest of `details`.

**This part wants a review pass before it is written**, not because it is hard,
but because a stale or unbounded key map is its own defect and the eviction
condition (a successful admission for that key; a disconnect) has to be right.
Do not let the map grow without a stated reason it cannot.

#### R1b design, for Waffles' gated review — submitted before writing

Waffles' condition: *bounded by construction, entries cleared on admission or
departure, because an unbounded key map keyed on refused identities is a slow
leak wearing a diagnostic's clothes.*

**The trap, named first.** The obvious key is
`(identity, task_queue, node)` — and `identity` is a **client-supplied string**.
A refused worker that varies its identity per dial grows the map forever, and
"bounded by construction" would be false while the code looked right. A refused
connection never enters the registry, so nothing else would ever evict it. **A
map keyed on anything the refused party controls cannot be bounded by
construction.**

**The design: key on what the SERVER controls, put the client's part in the
value.**

- **Key:** `(task_queue, node)` — both server-derived, and already enumerated by
  the gate from the deployed catalog. **The map cannot hold more entries than
  the catalog has queue/node pairs**, which is the bound, and it is structural
  rather than enforced.
- **Value:** a digest over `(identity, details)` — the whole refusal, including
  who was refused.

Checked against R1a's four cases:

| Case | Digest | Speaks? | Wanted |
|---|---|---|---|
| same worker redials identically | unchanged | no | ✅ |
| reason changes (same worker) | changes | yes | ✅ |
| a different worker build, same queue | changes (identity is in it) | yes | ✅ |
| two differently-broken workers alternating on one queue | flips each dial | both speak, every dial | ⚠️ see below |

**The fourth case is a real cost and it is stated rather than hidden.** Two
distinct broken workers hammering one queue will each re-log on every dial —
the #94 volume returns for that shape. I am accepting it rather than adding a
cap, on the grounds that **two independent faults on one queue is a situation an
operator should hear about repeatedly**, and any suppression that survives it
would have to suppress genuinely new information. If review disagrees, the
alternative is a per-queue set of digests, which reintroduces the unbounded
client-controlled dimension by the back door.

**Clearing, both directions:**
- **On admission** — a successful `validate_worker_contracts` for a
  `(task_queue, node)` removes that entry, so a worker that is fixed and then
  breaks again is heard immediately rather than being suppressed by its own
  history.
- **On departure** — entries whose `task_queue` no longer appears in the
  admission catalog are pruned lazily on the next admission call. No timer, no
  sweeper, bounded work per call, and the catalog read already happens on that
  path. **Departure needs no new mechanism because the gate already asks the
  catalog what exists.**

**Ownership:** the handle is passed into `validate_worker_contracts` as a
parameter. Both callers already assemble this call and both have somewhere to
hold it (`ServerState` for gRPC, the notifier for liminal). This keeps the
single-log-site property that is the whole point of R1.

## R2 — the availability hint stops naming a remedy that cannot be followed

`crates/aion-server/src/awl/run_loop.rs:275`:

> *"No connected worker serves task queue `desk2`. Scaffold and run this worker
> from Workers & Actions, then retry start."*

This is delivered to an operator whose worker **is** running and **is** being
refused. Following it cannot help. It was the only signal I had during the
self-run, and it described the wrong problem.

The mechanism is at `:264-269`: `connected_workers` is counted from
`all_workers()` — the registry of **admitted** workers. A refused worker never
reaches the registry (the refusal precedes `accept_registration`,
`worker_grpc.rs:73`), so the count is 0 in both worlds and the hint cannot tell
them apart. **The hint is a guess in the grammar of a diagnosis.**

R1b's audit state is what makes it answerable: when a refusal is on record for
this queue, the hint names *that* — the queue is being served by a worker the
server refused, and why — instead of telling the operator to start what they
already started. Where no refusal is on record, the existing text is correct and
stays.

**This is not a second mechanism.** It is R1's state, read.

## R3 — routed to Tom via Waffles, NOT built against

Whether one queue shared across documents should be refusable **at all** is a
design question, not a bug. Content-hash module namespacing means a queue
accumulates every reachable version's demands, so *"one document per worker
queue, carrying that queue's full action set"* is currently a load-bearing
authoring rule that **nothing states and nothing enforces** — a beginner's
natural second move walks into it.

The tutorial (#146) now teaches the rule and teaches the availability probe as
the way to confirm arrival. That is a documentation patch over a design seam.
**Nothing in this brief changes admission semantics**, and nothing should until
Tom has ruled.

---

## Red first

The harness for the gRPC red already exists in the same file that pins the
refusal itself: `tests/worker_contract_registration.rs` stands up a real gRPC
service with real deployed AWL contracts and a `register()` helper, and
`mismatched_worker_is_refused_with_field_level_diff:169` already drives a real
refusal — it asserts only on the `Status` the *worker* receives, and says
nothing about the server. The subscriber pattern for asserting on captured
`tracing` output is in `dead_man_switch_refusal_logs.rs`.

1. **Watch it fail.** A gRPC-path test asserting the server named the refusal —
   red at `HEAD`, because it names nothing.
2. **Watch the transition rule fail.** N identical refusals ⇒ one diagnosis; a
   *changed* reason ⇒ a second. Both halves watched failing separately: a rule
   that only ever suppresses is indistinguishable from one that never logs.
3. **Watch the hint fail.** Availability probed against a queue with a refused
   worker, asserting the response does **not** say "scaffold and run" — red now.

**The mutation must sit on the test's path.** Earned the hard way on 2026-08-06:
my tutorial's verb gate first refused for the wrong reason and read exactly like
a pass. For each red above, confirm the failure message names the *subject*, not
a setup fault.

## Acceptance

- A contract-refused registration is named server-side on **both** transports,
  by the gate, at `WARN`, carrying the full `details` diagnosis.
- The gRPC path, re-run against the measured scenario in
  `SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md`, produces a server log line. The
  original measurement — 0 ERROR, 0 WARN, 0 bytes — is the baseline, and it was
  taken before this brief existed, so it is independent of what it measures.
- Identical repeat refusals do not re-emit; a changed reason does.
- The availability hint distinguishes *no worker* from *refused worker*.
- No time interval, count, or cap is introduced anywhere in the change.
- `dead_man_switch_refusal_logs.rs`'s pin is **amended, not deleted**, and still
  fails if the naming is removed.
- Admission semantics are **unchanged**. R3 is not built.

---

## AS BUILT — 2026-08-06

**Commits:** `d4d915f18` (red + the audit), `3d034c686` (the gate names itself),
and the R1b bound correction below. R1, R1a, R1b and R2 are built; **#94 is
closed by the transition rule**, in the same motion, as recommended and
approved.

### Where the build DEPARTED from this brief, and why

**1. The remembered value is the whole refusal, not a digest of it.**
R1b specified *"a digest over `(identity, details)`"*. R2 has to SAY the reason
to the operator, and a digest can answer "different?" and nothing else. Keeping
the refusal itself served both readers and **deleted `DefaultHasher` entirely** —
the remedy removed code rather than adding it.

**2. The catalog-read failure names itself too.** The brief scoped the naming to
the `Mismatch` branch. Leaving `Catalog` voiceless would have reproduced the
defect's own shape: the gRPC path already logged the case where it SKIPPED the
check while swallowing the case where it refused.

**3. 🔴 The R1b bound as approved was FALSE, and the fix is recorded here rather
than quietly.** Waffles' condition was *bounded by construction*. The approved
design justified that with *"`(task_queue, node)` — both server-derived, and
already enumerated by the gate from the deployed catalog."* **Both arrive on the
wire** in the registration request (`worker_grpc.rs:652-653`,
`liminal_transport.rs:1571-1572`). The design named the trap — *"a map keyed on
anything the refused party controls cannot be bounded by construction"* — moved
`identity` out of the key, and **left `node` in it**, which is the same
dimension by another name.

Measured consequence: `dispatch_can_reach` demands every UNPINNED action of
every locality, so a refused worker varying its advertised node allocated one
entry per dial without limit. Confirmed by control: with the advertised node
used as the key, 100 dials produce **100 sites**.

`task_queue` turned out to be bounded — but by a mechanism nobody had written
down, not by the one the brief claimed. A queue holding no reachable contract
demands nothing, so it **admits** rather than refuses and never reaches the map
at all.

⇒ **The site is now derived from the CATALOG** (`contracts::refusal_site`), as
the type `RefusalSite`. A node names a site only when some demanded action is
pinned to it; a node the catalog never mentions cannot have changed the verdict,
so every such connection is one site. An unreadable catalog is **one** site
rather than one per queue name tried. Bound: catalog queues × (their pinned
nodes + 1) + 1 — every component server-derived.

That collapse is also more truthful than the original: two connections owing an
identical action set and failing identically are ONE fault, and reporting them
separately was #94 volume for no information.

**Both directions are controlled**, because a bound test alone can be satisfied
by collapsing everything:

| mutation | `…never_pins_cannot_allocate_a_site` | `a_node_the_catalog_pins_is_a_site_of_its_own` |
|---|---|---|
| none (as built) | ok | ok |
| key on the raw advertised node (the original defect) | **FAILED** (100 ≠ 1) | ok |
| collapse every locality into one site | ok | **FAILED** (1 ≠ 2) |

Each control kills exactly its own test.

**4. `clear_queue` is wired to the queue-retired case**, which R1b described as
lazy departure pruning. It is hygiene against a fossil refusal silencing the
first refusal after a redeploy — **not** the bound, which is structural.

### The lesson this brief earned

**A bound is a claim about every dimension the client can move, so it takes one
witness per dimension.** The original test gave the refused party a thousand
identities and proved only that `identity` had been kept out of the key. The
dimension that was actually open was never asked about, and the module's own doc
asserted it was safe. `admission_audit_tests.rs` now carries one witness each
for identities, nodes and queues, and the gate-level bound is measured through
`validate_worker_contracts` against a real deployed catalog rather than assumed
of it.

## Not in scope, and said plainly

- The worker's own silence. `aion worker shell` installs no tracing subscriber
  (`docs/workers/SHELL-WORKER-MANIFEST.md`), so the refusal `Status` arrives and
  the redial loop swallows it. Amendment 1 called the silence *"two halves
  meeting"* and both halves should eventually move. **This brief moves the
  server half**, which is Waffles' condition and the half an operator can reach
  without rebuilding anything. The client half is held by the estate-wide worker
  rebuild freeze until the #139 cut, and is #142's neighbourhood.
- Anything that changes which workers are admitted.
