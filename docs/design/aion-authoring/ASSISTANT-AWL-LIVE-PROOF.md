# The full assistant session, authored in AWL, run on the live server

Measured 2026-07-30 on Tom's own server (pid 99844, `127.0.0.1:8080`). Every
line below is copied from a command I ran or from recorded event bytes.

Workflow `1b3afbde-823e-4374-8910-7e3fa3a83fae`, run
`02428b4e-4e68-4f3f-bf56-5a94b005262c`, package
`d9774285e352824b78f6596f5aef1d5492735796228c23bc28c549eb3e405215`, compiled
from `examples/assistant/awl/assistant.awl` (content hash
`76257db0408240d0e807444c272ca27c626806fbec7b7b7f78c3779022c7bdd0`).

## The one-line version

The full operator-paced agent session — provision, agent round, park on a
signal, resume the same conversation, end cleanly on the operator's word —
now runs from an AWL document. The Gleam original could never start at all
under queue-routed serving. Two rounds, two signals, fourteen events, and
every field of the authored outcome came back exact.

## What had to be true first

Queue `default` was unservable when this session began: `available: false`,
`connected_workers: 0`, and 974 recorded registration refusals. Three things
were wrong and all three are now fixed in the tree.

1. The worker advertised action *names* but no typed *descriptors*, while
   admission reads descriptors. Fixed by deriving the descriptors from
   `assistant.awl` itself (`examples/agent-dev/worker/src/descriptors.rs`).
2. `assistant_spike.awl` declared `assistant_provision(repo_path, run_id)`
   and `assistant.awl` declared it with `objective` as well. Input
   compatibility is contravariant, so one worker could not satisfy both and
   was refused outright. Fixed by making the two documents agree.
3. Four superseded `assistant` versions and one superseded `assistant_spike`
   version were still loaded. A stale version is still a *reachable*
   contract, so it still gates admission. All five unloaded.

Then, and only then:

```
POST /awl/workers/availability {"namespace":"default","task_queue":"default"}
→ {"available": true, "connected_workers": 1, "scaffold_hint": null}
```

and, server-side, `registered liminal worker in-band` at 12:34:48Z with no
refusal after it.

## Deployed through the studio, not the CLI

Both documents were written into the authoring workspace with
`PUT /awl/documents/{path}` and deployed with `POST /awl/deploy` — the same
path the ops console uses. This was deliberate: a CLI deploy leaves the
document invisible in the studio (task #91), so deploying this way is the
only route that leaves Tom able to *see* what is running.

```
check    direct compiler accepted workflow assistant
compile  9180 bytes of direct BEAM compiled
package  d9774285e352824b78f6596f5aef1d5492735796228c23bc28c549eb3e405215 built
deploy   deployment eec2d3c8-79b0-4f01-84bd-2d0b4fb27058 loaded
```

## The run

```
 1  12:36:30  WorkflowStarted
 2  12:36:30  SearchAttributesUpdated
 3  12:36:30  ActivityScheduled     assistant_provision
 4  12:36:30  ActivityStarted
 5  12:36:34  ActivityCompleted     workspace + 4143-byte opening prompt
 6  12:36:34  ActivityScheduled     assistant  (round 1)
 7  12:36:34  ActivityStarted
 8  12:37:26  ActivityCompleted     round-1 reply
 9  12:38:34  SignalReceived        assistant_continue {"message":"…","end":false}
10  12:38:34  ActivityScheduled     assistant  (round 2)
11  12:38:34  ActivityStarted
12  12:38:39  ActivityCompleted     round-2 reply
13  12:41:18  SignalReceived        assistant_continue {"message":"","end":true}
14  12:41:18  WorkflowCompleted
```

### Round 1 — grounded in the repository, not guessed

The objective asked it to name the file defining AWL loop lowering and say
what a loop returns when `max` is reached rather than `until`. Recorded
reply, verbatim from the event bytes:

> The reference AWL loop lowering is defined in
> `crates/aion-awl/src/emitter/loops.rs`; if the `max` bound is reached while
> `until` remains false, the loop returns `Ok(var)` — or `Ok(#(var, count))`
> for a `counting` loop — exactly as it does when `until` succeeds, with no
> error or exhaustion flag.

The file is correct. The `counting` variant was not asked for and is also
correct. That last clause independently confirms why `assistant.awl` carries
a `stalled` field at all: the two loop exits are indistinguishable, so the
document has to make them distinguishable itself.

### Round 2 — the operator's message verbatim, into the same conversation

The dispatched input at seq 10 is the operator's text and nothing else:

```json
{"prompt": "Without me repeating it: what file did you just name, and what is
 the ONE extra detail you gave about counting loops that I did not ask for?"}
```

No opening prompt, no re-sent contract — the round-one prompt appears exactly
once in the whole history. The reply, five seconds later:

> The file was `crates/aion-awl/src/emitter/loops.rs`. The extra detail was
> that a `counting` loop returns `Ok(#(var, count))` rather than `Ok(var)`.

It answered from memory of round one, which is the whole point of pinning one
norn session per action type per run.

### The outcome

```json
{
  "outcome": "ended",
  "payload": {
    "disposition": "OperatorEnded",
    "rounds": 2,
    "last_reply": "The file was `crates/aion-awl/src/emitter/loops.rs`. …",
    "workspace_path": "/Users/tom/.aion/clones/1b3afbde-…/repo"
  }
}
```

Every field is exact. That single object exercises most of the language
surface the port depends on: the `counting` binding produced `rounds: 2`, the
`Round(...) -> round` reconstruction threaded `last_reply` out of a nested
scope, the outer `until` fired on the threaded value, the outcome ordering
picked `operator_ended` over the blank-message condition when the operator
set both, and `session.path` was still readable after the loop.

## What this does NOT prove

Say it plainly, because the parts that did not run are the parts that would
matter in a bad week.

- **The `stalled` and `capped` paths never executed.** Both are
  budget-exhaustion paths; reaching them live means spending 20 nudges or 50
  agent rounds. They are covered analytically by
  `crates/aion-awl/tests/assistant_example.rs`, which evaluates the
  document's own loop conditions across the whole domain and was verified by
  mutation — but analytically is not live.
- **The degraded-session resume path never executed.** This was a fresh
  session, so `--allow-degraded-session` was passed but never needed. The
  fix for a session that outlives a provider epoch is proven separately and
  by hand (`DURABLE-AGENT-SESSION-EPOCH-FINDING.md`), not by this run.
- **The prompt is still JSON-wrapped on the wire** (task #98). Round two
  delivered `{"prompt": "…"}` rather than a bare string, and the agent coped.
  It works *despite* this, not because of it. Until #98 is settled, every
  claim here is a claim about the wrapped form.

## Two seams found while proving it

- **`POST /workflows/start` demands `content_type: "application/json"`;
  `POST /workflows/history` returns `content_type: "Json"` for the very same
  payload.** Read and write disagree on the same field, so a payload read
  back off a run cannot be resubmitted without translation. Filed.
- **History elides payload bytes above a default limit** (`{"__elided":
  true, "size_bytes": 4143}`) unless `payload_limit_bytes: 0` is passed. That
  is a sensible default, but it means the obvious way to inspect a run hides
  exactly the agent replies an operator wants to read.
