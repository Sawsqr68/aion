# The assistant "just stops", and the record says Running

Measured on Tom's own live server (pid 47610) 2026-07-30 ~10:30Z, against
namespace `default`. This is not a test-rig finding; it is the state of the
machine he is looking at.

## The observation

Workflow `0cd47656-0317-425d-bc35-194d59e9cd4c`, type `assistant`, status
**`Running`**. Its entire history:

```
seq  1  2026-07-06T06:40:21  WorkflowStarted
seq  2  2026-07-06T06:40:21  SearchAttributesUpdated
seq  3  2026-07-06T06:40:21  ActivityScheduled assistant_provision id=0 queue=default
seq  4  2026-07-06T06:40:21  ActivityStarted   id=0
seq  5  2026-07-06T06:40:28  SignalReceived
seq  6  2026-07-30T02:55:03  ActivityScheduled assistant_provision id=0 queue=default
seq  7  2026-07-30T02:55:03  ActivityStarted   id=0
seq  8  2026-07-30T02:58:16  ActivityScheduled assistant_provision id=0 queue=default
seq  9  2026-07-30T02:58:16  ActivityStarted   id=0
seq 10  2026-07-30T07:08:12  ActivityScheduled assistant_provision id=0 queue=default
seq 11  2026-07-30T07:08:12  ActivityStarted   id=0
```

`assistant_provision` has been **started four times and completed zero times**,
over 24 days. Seq 4 → seq 6 is a 24-day gap containing no event at all.

And:

```
POST /awl/workers/availability {"namespace":"default","task_queue":"default"}
{"available":false,"task_queue":"default","connected_workers":0, ...}
```

Nothing serves the queue. The only running worker is `staged-rounds-worker`
(`--identity tharsis-fleet`), which serves the staged-rounds queue.

## The control that makes it a defect rather than a circumstance

Workflow `f9b062c2-d260-43f9-8374-d3210330f567`, same type, started
2026-07-02, is the healthy case:

```
seq  3..5   assistant_provision id=0  scheduled -> started -> COMPLETED (same second)
seq  6..8   assistant            id=1  scheduled -> started -> COMPLETED
seq  9      SignalReceived
seq 10..12  assistant            id=2  scheduled -> started -> COMPLETED
```

It is also `Running`, and that is CORRECT — a long-lived assistant session
idles awaiting the next signal. So `Running` covers both "working, waiting for
you" and "dead since the 6th", and nothing in the projection distinguishes
them. I misread the healthy one as stuck on first look, which is the point:
if the shapes are indistinguishable to someone reading the history directly,
they are certainly indistinguishable in the console.

## What is actually wrong, in order of severity

1. **An activity dispatched to a queue with zero workers records
   `ActivityStarted`.** Started should mean a worker took it. Here it is
   recorded with `connected_workers: 0`, so the event asserts something that
   did not happen, and every downstream reader — console, status projection,
   an operator reading history — is misled in the same direction.

2. **There is no upper bound on an in-flight activity.** No timeout fires, no
   dead-letter is written, nothing is surfaced. #42 already records that the
   authored workflow timeout is wired to nothing; this is that gap with a
   24-day receipt attached.

3. **Restart re-arms it silently and forever.** Three re-arms today alone
   (02:55, 02:58, 07:08), each on a server start. Each one makes the workflow
   look freshly active. A run that has been dead for three weeks presents as
   having started minutes ago.

4. **`Running` is load-bearing in the console and cannot carry this.** Status
   is a projection of history (invariant 4) and the history genuinely contains
   no terminal event — so the projection is not lying. The gap is that nothing
   projects *reachability*: whether anything can still advance this run.

## Why this is the "it just stopped" report

Tom said the assistant stopped and that it had stopped a while ago. It had —
on 6 July. The server told him nothing then and tells him nothing now, and the
one surface he would check says `Running`.

This was diagnosed while investigating whether a restart was safe, not from
the complaint. The complaint had been attributed to the tharsis run dying,
which was a separate and genuine fault (#82) — but it was not what he was
looking at.

## Relationship to open items

- **#58** dead-man switch on worker connection + loud rejection/disconnect
  logging. This is the missing-worker half of the same hole.
- **#42** `WorkflowTimedOut` wired to nothing. Item 2 above is its receipt.
- **#83** a runtime fault records `{"tag":"AwlFailed"}` and nothing else. Same
  family: the system knows more than it says.
- **#72** server could not exit while an activity waited for a worker — fixed
  and landed, but note the running binary predates it, which is exactly the
  condition here.

## The restart question this arose from

Restarting the server will re-arm `assistant_provision` a fifth time to the
same empty queue. The restart is still correct — it is the only way to get
today's fixes into the process Tom is using — but it will not fix this run,
and it must not be reported as having fixed it. Starting a worker on the
`default` queue is the separate action that would.
