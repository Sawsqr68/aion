# A Gleam-authored package with activities cannot start under queue-routed serving

Measured 2026-07-30 on Tom's live server while restoring the assistant. Not
inferred from reading — the package was rebuilt from source twice, deployed
twice, and refused at start both times.

## The claim in CLAUDE.md

> Workflow authoring has two first-class surfaces: the typed Gleam SDK under
> `gleam/aion_flow/`, and AWL `.awl` documents.

## What is actually true

Only AWL can produce a package that starts, whenever the server serves
activities through task queues — which is the entire remote-worker posture the
worker SDKs exist for.

The chain, each link read at the bytes:

1. `crates/aion-package/src/manifest.rs:68-77` — `DeclaredActivity` has
   **exactly one field**, `activity_type: String`. There is nowhere in the
   manifest to record a queue.

2. `crates/aion-package/src/contract.rs:74-78` —
   `PackageContract::from_manifest` maps **every** declared activity into
   `unscoped_activities`. Its own doc says so plainly: these are
   *"activity names retained by legacy project manifests that cannot declare a
   queue or typed activity surface"*, and *"this never invents a queue ... for
   bare activity names."*

3. `crates/aion/src/lifecycle/start_admission.rs:37-47` — start is refused with
   `NO_QUEUE_DECLARATION` when `activity_serving() == QueueRouted` and
   `unscoped_activities` is non-empty.

4. `crates/aion-awl/src/compile.rs:381` — the AWL compiler sets
   `unscoped_activities: Vec::new()` and emits a real worker contract with
   queues.

So: (1) makes it impossible for a Gleam/TOML package to be scoped, (2) marks
every one of its activities unscoped, (3) refuses to start anything unscoped,
and (4) is the only path that produces a scoped package.

## The trap that cost the most time

`gleam/aion_flow/src/aion/activity.gleam:218` exports
`activity.task_queue(activity, name)`, documented as *"select the task queue
this activity is dispatched on"*. It looks exactly like the fix.

It is not. That value is read at DISPATCH time from the in-process activity
value; it never reaches the package manifest, because the manifest has no
field to carry it. Adding it to both assistant activities changed the content
hash, deployed cleanly, and produced the identical refusal.

An API that appears to declare the thing the gate demands, and whose value
cannot reach the gate, is worse than no API — it converts a five-minute
"this surface can't do that" into an hour of plausible attempts.

## Scope of the claim, stated precisely

- Under `ActivityServing::QueueRouted` — remote workers, the posture Tom's
  server runs in — a Gleam-authored package **with any activity** cannot start.
- Under in-process serving the check is deliberately skipped
  (`start_admission.rs:33-36`), so Gleam packages still work there.
- A Gleam package with NO activities is unaffected.

So this is not "the Gleam SDK is broken". It is: **the Gleam SDK cannot author
a workflow that uses a remote worker**, which is most of the reason to use one.

## How it presented

`examples/assistant` — the flagship "the assistant runs AS an aion workflow"
demo, with its own hand-over crib sheet (`DAVID.md`) — cannot be started at
all. Rebuilt from source with the current toolchain, deployed successfully
(`route_changed: true`), then:

```
NO_QUEUE_DECLARATION: workflow `assistant` version `249e7489…`
has unscoped activities assistant,assistant_provision;
re-deploy from a checked AWL contract
```

Note that DEPLOY SUCCEEDS. The package is accepted, routed, and looks live in
the console. The refusal only arrives at the first start, which means a
deployed-and-broken package is indistinguishable from a deployed-and-working
one until someone tries to use it.

Someone has hit this before and left a note:
`crates/aion-cli/tests/lsub5b_osproc_kill9_failover_e2e.rs:21` records
`NO_QUEUE_DECLARATION` firing *"172 times out of 172: the harness-built
package"*.

## What this is NOT

It is not the cause of the assistant stopping on 6 July — that was a missing
worker (see `ASSISTANT-STUCK-FINDING.md`, task #87). This is the reason the
assistant could not be RESTORED by simply rebuilding and redeploying it, and
therefore why it stayed dead.

## The decision this needs

Three coherent options, and this is a design call rather than a bug fix:

1. **Give the manifest a queue.** Add the field to `DeclaredActivity`, thread
   `activity.selected_task_queue` through `aion package` into the manifest, and
   the Gleam path becomes first-class as documented. Largest change; honours
   the stated architecture.
2. **Migrate the examples to AWL** and say plainly in CLAUDE.md that the Gleam
   SDK does not author remote-worker workflows. Smallest change; contradicts
   the current claim of two first-class surfaces, so the claim must change too.
3. **Make `activity.task_queue` refuse to compile or emit** for a package that
   cannot carry it, so the dead-end is discovered at authoring time rather than
   at first start. This is the honesty floor and should happen under 1 or 2
   either way.

What must NOT happen is leaving an exported SDK function that silently cannot
do what its name and docs say.
