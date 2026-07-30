# A durable workflow outlives its agent session

Measured 2026-07-30 on the live server, on a real 28-day-old run. Every claim
below is from recorded history or a command I ran, not from reading code.

## The one-line version

Aion parked a workflow for twenty-eight days and resumed it flawlessly. The
**agent session underneath it could not survive a provider epoch change**, and
the worker never asked for permission to resume across one. The engine's
durability is real; the agent layer's is not, and nothing said so until a
message was sent.

## What was there

Run `f9b062c2-d260-43f9-8374-d3210330f567`, workflow type `assistant`, started
`2026-07-02T22:30:54Z`. Twelve events, all clean:

```
WorkflowStarted · SearchAttributesUpdated
ActivityScheduled/Started/Completed   (assistant_provision)
ActivityScheduled/Started/Completed   (assistant, round 1)
SignalReceived                        (assistant_continue, {"message":"hi"})
ActivityScheduled/Started/Completed   (assistant, round 2)
```

Round 2's recorded result, decoded from the event bytes:

> "Hi! I'm here and ready to help with aion. I don't currently have the real
> aion repository in this scratch workspace, so if you ask about exact SDK
> APIs or implementation details I'll be careful to say what I can and can't
> verify..."

Then it parked on `assistant_continue` and waited.

**It was not stuck, and it was not asleep.** A live query answered instantly,
twenty-eight days later:

```
POST /workflows/query {"query_name":"assistant_status"}
→ {"result":{"phase":"awaiting_operator","round":2}}
```

That is a workflow holding live, queryable, in-memory state four weeks after
its last event. The durability machinery works.

## What happened when it was used

A signal was sent — `assistant_continue` with a message. The engine did every
single thing correctly:

```
SignalReceived        10:46:51.276Z
ActivityScheduled     10:46:51.332Z   (assistant, activity_id 3)
ActivityStarted       10:46:51.332Z   attempt 1
ActivityFailed        10:46:51.537Z   Retryable
ActivityStarted       10:46:56.578Z   attempt 2
ActivityFailed        10:46:56.770Z   Retryable
ActivityStarted       10:47:01.819Z   attempt 3
ActivityFailed        10:47:02.054Z   Terminal
WorkflowFailed        10:47:02.111Z
```

Signal recorded, workflow woken, round dispatched, retry policy honoured
exactly as authored (3 attempts, 5s fixed backoff), failure recorded with a
cause. Eleven seconds from signal to honest terminal failure.

The refusal was norn's, verbatim from the recorded event:

```
run/execute failed (code -32603): agent error: session error: storage error:
open_session failed: session 'f9b062c2-d260-43f9-8374-d3210330f567-assistant'
is a fresh-epoch projection; explicit resume approval is required
```

## Root cause

`norn --help`:

```
--allow-degraded-session
    Allow a coherent migrated legacy session to resume from a fresh provider
    epoch when its exact provider transcript cannot be replayed. Corrupt or
    ambiguous legacy records remain inspect/export-only
```

The session is **coherent**. It is not corrupt. It simply predates the current
provider epoch, and norn — correctly — will not silently resume across one.
It demands explicit approval.

The agent-dev worker composes `--resume-if-exists` and `--account` and
**never** composes `--allow-degraded-session`. So every agent session that
outlives a provider epoch is unresumable through the worker, permanently, with
no route out visible to the operator.

## The fix, proven before it was written

Run by hand against the exact session id:

```
norn --print --session-id "f9b062c2-d260-43f9-8374-d3210330f567-assistant" \
     --resume-if-exists --account default --allow-degraded-session --quiet
```

Asked to name one concrete thing from the earlier conversation, it answered
correctly and unprompted:

> "Yes—we discussed that aion workflow boundary types should be declared first
> and codecs generated with `aion generate .`."

So the session resumed **with its memory intact**. "Degraded" here means the
exact provider transcript could not be replayed — not that the conversation is
lost.

The worker fix is therefore: an explicit operator opt-in flag that composes
`--allow-degraded-session`, plus a refusal that names that flag when norn
raises this exact condition. Opt-in rather than defaulted, because degraded
resume is a real fidelity tradeoff and the house rules forbid assuming it on
the operator's behalf.

## Honest accounting

I destroyed the run. Signalling it is what converted a latent failure into a
terminal one, and the workflow is now `Failed` rather than `Running`.

Two things are true about that. It was the correct probe — the session was a
zombie, and the operator's own next message would have killed it identically
and with no diagnosis attached. And it is still a durable run that no longer
exists, which is a cost, and the cost was mine to take and is named here
rather than folded into a success story.

The `assistant_status` query was the right first move and cost nothing. The
signal should have been understood as destructive before it was sent, because
a failing activity under a 3-attempt retry policy has exactly one ending.

## A second defect, found in the same bytes

The terminal failure is recorded as:

```json
{"kind":"Terminal","message":"retryable:harness reported failure: ..."}
```

The retry-exhaustion path flips the kind to `Terminal` and leaves the
`retryable:` prefix sitting in the message text. An operator reading the
terminal failure of a dead workflow is told, in the durable record, that it was
retryable. Filed separately.

## Why this is the important one

The whole premise of durable agents as infrastructure is that a workflow can
park for months and pick up exactly where it was. This run proves aion does
that: four weeks parked, queryable, signalable, and it woke on demand.

And it proves the thing underneath it does not. An agent conversation is state
living in a third party's storage under an epoch the engine does not control
and cannot see. Aion's replay guarantees stop at the activity boundary; past
that boundary the session is only as durable as the harness and the provider
make it.

That is not a bug to close and forget. It is a property of building durable
agents on hosted providers, and the design answer — refuse loudly, resume on
explicit approval, and never let a session look alive when it cannot be
resumed — needs to be a stated part of the architecture rather than something
each worker rediscovers.
