# Owning the whole stack: what aion already gets from it, and what it is leaving on the table

**Seat:** Vesper Lynd (aion). **Date:** 2026-08-01.
**Asked by:** Waffles the Terrible, relaying Tom's standing directive — *"from
literally the runtime up to the view that's shown in the browser, the entire
thing is ours. If there's anything we're not maximally capitalizing on, I want
to hear about it."*

Two questions, answered at the file and design-decision level. Section 2 is the
one worth the reading time: section 1 is things we already did right, section 2
is where aion is still wearing a shape that a rented dependency pressed into it.

Every claim below is marked **[verified]** (I read the code this pass) or
**[proposal]** (a design argument, not a measurement). I have not benchmarked
any of the performance claims; where I say "cost" I mean structural cost, and I
say so.

---

## 1. What ownership already buys — the showing story

### 1.1 Content-hash module namespacing beats a hard VM limit **[verified]**

Load-bearing invariant #5. Each `.aion` package version becomes a distinct,
immutable module named `logical_name$hash`. Stock BEAM gives you *two* live
versions of a module — old and current — and a third deploy kills anything
still running on the oldest. A workflow that sleeps three months is exactly the
thing that limit destroys.

We do not raise the limit; we sidestep it, because we control the module naming
and the loader. **A team gluing libraries together cannot do this at all** —
they would be reduced to userland version shims that still terminate at the
code server's behaviour. This is the single clearest "only because we own it"
in the system, and it is the one I would lead the story with.

### 1.2 The determinism boundary is enforced by construction, not by lint **[verified]**

Invariant #2. `workflow.now` is the recorded event timestamp; `workflow.random`
is seeded from `WorkflowId` + `RunId`. Workflow-visible code has *no* path to a
wall clock or an entropy source.

Everyone else in this category polices determinism with SDK discipline and
documentation — "don't call `time.Now()`" — and catches violations in
production, as replay divergence, months later. We own the runtime, so the
unsafe thing is not reachable rather than discouraged. **That is the difference
between a rule and a wall.**

### 1.3 AWL emits BEAM bytecode directly, and we can prove it equals the reference **[verified]**

`aion awl emit --target beam` (`EmitTarget::Beam`) compiles authored documents
straight to bytecode with no Gleam round-trip, and the BC-4 differential harness
proves the direct path and the Gleam reference path produce byte-identical
normalised trails.

Owning the language *and* the compiler *and* the VM is what makes both halves
possible: the shortcut, and the proof that the shortcut is honest. A language
hosted on someone else's toolchain gets to have one or the other.

### 1.4 The editor understands documents that do not parse **[verified]**

Because the checker is ours, AWL completion works on unparsed input (#31) and
references/rename are checker-driven and *refusal-carrying* (#111) — the editor
tells you why it won't, in the checker's own words.

This is a small thing that reads as a large one to an operator, and it is
unavailable to anyone building on a third-party grammar, because a borrowed
parser gives you an error, not a partial understanding.

### 1.5 The cluster's truth lives in our own store **[verified, design]**

Membership, discovery and placement are durable haematite state. beamr is a
sensor; haematite is the authority. The conventional shape here is a
consensus sidecar — etcd, Consul, ZooKeeper — bolted next to a database that
already has durability, giving you **two** sources of truth with different
failure modes and no shared transaction.

We have one. That is an operational simplification an assembled stack cannot
buy at any price.

---

## 2. Missed leverage — where we are still wearing someone else's shape

### 2.1 🔴 The wire contract is an IDL, and that is why both worker SDKs went wire-dead **[verified]**

The strongest finding here, because it already cost us.

We carry `aion-proto` (hand-written contracts) *and* `aion-proto-generated`
(tonic/prost stubs). In #28 **both** worker SDKs were wire-dead while their test
suites were green — the tests validated against fossilised generated stubs, so
the suite proved the fossil self-consistent and said nothing about the wire.
#63 was the same class again in the Python caller SDK. #67 records that the
TypeScript caller SDK still has no drift gate.

**This defect class is a property of the architecture, not of anyone's care.**
An IDL exists so that two teams who do not share a type system can agree. We own
both ends of every one of these seams — the engine's Rust types, the Rust
worker SDK, the Python and TypeScript SDKs, and the console. There is no second
team.

#53 already demonstrated the alternative in miniature: `PayloadElision` was
defined as a ts-rs Rust type and the TypeScript was *generated from the type the
engine actually uses*, so drift became unrepresentable rather than merely
gated. **[proposal]** Extend that discipline to the whole wire surface and the
fossil class stops existing — not "gets caught earlier", stops existing.

The counter-argument deserves stating: protobuf buys cross-language codegen and
a stable binary format cheaply, and an external customer writing a worker in Go
wants an IDL. That is a real reason to keep protobuf **at the public edge**. It
is not a reason for the *internal* contract between our engine and our own SDKs
to be a separately-maintained artifact.

### 2.2 HTTP status codes are lossily re-deriving types we already had **[verified]**

#125, closed today, is the cleanest example. `aion-toolchain` mapped every
non-zero `gleam` exit to `ToolchainError::TypeCheck`, and the server mapped that
to an inline 400 — so a hex.pm outage told an author, by the error's own type
*and* its HTTP class, that their code failed type-checking. The fix added a
`DependencyLayer` variant and routed it to 503.

That fix is right and it is also a patch on a lossier seam than we need. We own
the server, the wire, *and* the console. Compressing a rich typed error into a
three-digit integer and then having the console re-derive meaning from the
integer is a convention from a world where the client is a browser you have
never met. #102 (start demands `"application/json"`, history returns `"Json"`)
and #90 (a `kind:Terminal` error whose message still begins `"retryable:"`) are
both the same seam leaking in different places: **the type was right somewhere
and got flattened in transit.**

**[proposal]** Carry the typed error to the console as a type, and let the
status code be a *projection* of it — the same relationship `WorkflowStatus`
already has to history (invariant #4). We already believe this pattern; we just
haven't applied it to errors.

### 2.3 JSON is the payload encoding on a seam where both ends are ours **[verified]**

`Payload` is correctly type-erased — bytes plus a `ContentType` tag
(`crates/aion-core/src/payload.rs`), which is invariant #1 and is right. But
`ContentType::Json` is the default and the practical universal case, and
`payload.rs:77` runs `serde_json::from_slice` on every read.

For the console and for external callers, JSON is correct and should stay. For
the engine↔store and engine↔Gleam-SDK paths, **both ends are ours and both ends
already speak BEAM terms.** We are encoding to JSON and decoding back across a
boundary where no translation is required — a tax inherited from the assumption
that the other side is a stranger.

**[proposal]** A native term content-type on the internal paths, JSON retained
as the view. **I have not measured this and will not claim a number** — the
honest statement is that the *structure* pays a conversion that ownership makes
unnecessary, and it should be measured before it is believed. The type-erased
`Payload` design means this is additive, not a rewrite: it is a new
`ContentType` variant, which is exactly what that enum is for.

### 2.4 Routing keys are strings, so mismatches are representable **[verified]**

The routing model is namespace > task_queue > node. Those are strings. #93 was a
defect where worker admission *read a different field than the refusal logged* —
a whole task queue unservable. #99 was a spike and a port failing to share queue
`default`. #92 is a shell worker refused on schema drift in a field name.

String-keyed routing is Temporal's convention and it exists because Temporal's
namespaces are created by an operator through an API, in a system that does not
own the language its workflows are written in. **We own the language.** AWL
already knows, at check time, what queue a document targets, and the registry
already mints namespaces on use.

**[proposal]** Make the routing key a checked type that the checker can refuse
at authoring time, so "this worker can never serve this workflow" is a
compile-time refusal with a span, not a runtime silence. This one directly
serves the thing Tom keeps asking for — that authoring tells you the truth
while you are still typing.

### 2.5 There are two answers to "what is deployed" **[verified]**

#91: a CLI-deployed AWL workflow is invisible in the studio, because deploy
never registers the document with the authoring workspace. That is filed as a
bug and it is one. It is also a symptom: the `.aion` archive is a **file
artifact** that is then hashed and loaded, which is artifact-repository
thinking — the shape you adopt when your build system and your runtime are
different products from different vendors.

**[proposal]** The package is content-addressed already (invariant #5). A
content-addressed object belongs *in* the content-addressed store we wrote. If
haematite holds the package, then "what is deployed" has exactly one answer, the
studio and the CLI cannot disagree, and #91 cannot recur in a new form. This is
the largest change I am proposing and the one I am least certain about — it
touches Apollo's seat and needs a real design pass, not a paragraph.

### 2.6 Retry policy is a constant justified by precedent **[verified]**

`BUILD_ATTEMPTS: u32 = 5` in the shared gleam test support is documented as
matching "the only bounded-retry precedent in this repository". That is an
honest comment and a fair call for a test harness.

The pattern it reflects is worth naming though, because the house rule
**NO ARBITRARY LIMITS / NO ASSUMED DEFAULTS** exists precisely against it. #34
records retryable-but-not-retried activities; #90 records a retry-exhausted
activity mislabelled. We own the scheduler and the store — **[proposal]** retry
state could be durable, projected and inspectable in the console like everything
else, rather than a constant compiled into a decision an operator cannot see.

---

## 3. The one that would change a ruling

Waffles said explicitly that things requiring a ruling change are welcome, so:

**§2.1 puts the `aion-proto` / `aion-proto-generated` split in question**, and
that split was a considered decision. My argument is not that it was wrong when
made — it is that it was made when the SDKs were plausibly third-party surfaces,
and they are now ours end to end. The evidence that the cost is real is #28,
#63 and #67: three separate incidents of one defect class, one of which shipped
two dead SDKs behind green suites.

**The generalisation I would put to Tom:** an IDL, a status code, and a
serialization format are all *treaties between strangers*. Every one of them
costs fidelity in exchange for not needing to trust the other side. We are not
strangers to ourselves. **Wherever the stack is talking to itself through a
treaty, that is missed leverage** — and it is worth a sweep with exactly that
question, because it is the shape that produced the most expensive defects on my
board this month.

---

## 4. What I am NOT claiming

- No performance numbers. §2.3 in particular is a structural argument; the
  conversion cost should be measured before anyone repeats it as fact.
- §2.5 is a direction, not a design. It needs Apollo and a real pass.
- I have not audited the console or the Gleam SDK with the same care as the
  Rust crates; there may be more of §2.2's shape in `sdks/` than I have seen.
- Sections 1.1–1.5 are things we already have. They belong in the showing story
  as **built**, not as intentions.
