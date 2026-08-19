# canary — a workflow whose only job is to still be running

The smallest useful workflow in the repository. It sleeps thirty seconds, writes
down that it woke, and sleeps again. That is the whole thing.

Its value is entirely in its own survival. While it is `Running` with a rising
event count, the engine is appending events, firing durable timers, projecting
status and replaying workflow code. If it stops without being cancelled,
something is wrong with the engine — and because it has no worker, no child, no
filesystem and no network, there is nothing else that *could* have stopped it.

```sh
aion deploy examples/canary/canary.awl
aion start canary --input '{"note":"engine canary"}'
aion describe <workflow-id>
```

## What to look at

Two things, and the second is the one that matters:

- the status says `Running`;
- **the event count is higher than it was a minute ago.**

`Running` alone cannot tell a ticking canary from a wedged one. About two events
land per thirty-second round, so a minute apart should show four more.

## Why it is shaped this way

A canary that can die of unrelated causes is not a canary, it is a second thing
to debug. Three absences are the design:

- **No header `timeout`.** A run deadline is optional in AWL, and every declared
  one is a countdown started at the moment of the start. A workflow that
  declares `timeout 4h` stops after four hours no matter how much work you give
  it, because the deadline is not counting work.
- **No worker.** An action needs something to serve it, and that something can
  exit, have its contract changed underneath it, or be routed to a queue another
  deployment has claimed. Each of those ends the run for a reason that is not the
  engine.
- **No children.** Nothing it starts can outlive or block it.

## The two limits, stated rather than hidden

**It stops after about four years.** `max` is mandatory in AWL — the language
will not let a loop be unbounded, which is it keeping its promise that every run
terminates. Five million rounds at thirty seconds is a little over four years.
Landing there is a *completed* run whose report says it ran out; it will never
look like a failure.

**Its history grows the whole time and is never trimmed** — a few thousand
events a day. AWL has no continue-as-new surface, so that is the honest price of
an eternal workflow. Restart it when the price stops being worth paying; a fresh
run starts a fresh history and nothing is lost but the count.

## No query, deliberately

AWL has no arithmetic — `+` joins strings — so a round counter cannot be
incremented in workflow code without an activity, and an activity is the one
thing this document refuses to have. The pulse is read from outside instead,
which is the better signal anyway.
