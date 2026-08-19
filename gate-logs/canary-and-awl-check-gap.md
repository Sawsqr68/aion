# The engine canary, and the `aion awl check` hole it uncovered

**2026-08-19 07:00Z.** Tom asked whether there was "a workflow we can leave
running forever, as a canary in the coal mine."

## The correction that came first

There was one running — `cbc288e4`, his inbox watcher, started at 06:37Z when he
said "run it indefinitely". I had set `scans: 1000000` and told him that was
years' worth.

**The scan count was never the binding constraint.** `watch_inbox.awl` declares
`timeout 4h` in its header, and the engine starts a deadline timer for it at the
run's third event:

```
seq 3  TimerStarted  Named "deadline:1101e4b5-…"  fire_at 2026-08-19T10:37:47Z
```

A four-hour countdown, started at the start. It would have stopped at 20:37
local, and no scan count survives it, because the deadline is not counting
scans. *A number means something only in a known slot* — I checked the number I
set and not the one the document sets. Told Tom plainly.

## The canary

`prototyping/thing-finder/pocs/aion/canary.awl` — new, deployed as
`canary$87f604b19b90577e…`, started as
**`499cd0cb-bb89-48cb-9077-4075106f4cb5`** at 07:00:24Z. Tom's watcher was NOT
touched; its 4h bound is correct for real work.

It sleeps 30s, records that it woke, and routes back to itself. That is all of
it. Three absences are the design, each one a way it could otherwise die of
something that is not the engine:

- **no header `timeout`** — the defect above, not repeated;
- **no worker** — an action needs a server, and a server can exit, have its
  contract changed underneath it, or be routed to a queue another lane claimed;
- **no children, no filesystem, no network.**

A canary that can die of unrelated causes is not a canary, it is a second thing
to debug. Nothing can end this one but the engine.

### Verified, not assumed

Two samples 70s apart, plus the side-by-side that makes the deadline claim
falsifiable rather than a restatement of intent:

```
canary       start=07:00:24Z  Running  events 3 → 7 → 9   deadline=NONE
watch_inbox  start=06:37:47Z  Running  events 2319        deadline=2026-08-19T10:37:47Z
```

The **comparison has two sides**: "no deadline" is only a measurement because
the same extractor found one on the other workflow. A `deadline=NONE` from a
probe that cannot see deadlines reads identically to health.

The rising event count is also the readout handed to Tom, and deliberately so:
`Running` alone cannot tell a ticking canary from a wedged one.

### The two limits, stated rather than hidden

- **~4 years, not forever.** `max` is mandatory in AWL — the language will not
  let a loop be unbounded, which is it keeping its promise that every run
  terminates. 5,000,000 rounds at 30s. Landing there is a COMPLETED run whose
  report says it ran out; it will never look like a failure.
- **History grows and is never trimmed** — ~2 events per 30s round, so ~5,800
  a day. AWL has no continue-as-new surface, so this is the honest price of an
  eternal workflow. Restart it when the price stops being worth paying.

## 🔴 The defect this uncovered: `aion awl check` passes what `deploy` refuses

`aion awl check` reported `ok` on a document the compiler then rejected.

**Minimal repro** (`scratchpad/visits_repro.awl`):

```awl
workflow visits_repro
  input note: String
  outcome done: type Report, route success

type Report { rounds: Int, note: String }

step tick
  sleep 1s
  outcome over: when visits >= 3, route done(rounds: visits, note: note)
  outcome again: otherwise, route tick
  max 3 visits
```

```
$ aion awl check visits_repro.awl   → exit 0   "ok: visits_repro.awl (1 step)"
$ aion awl emit  visits_repro.awl   → exit 1   "the workflow start needs `visits`,
                                                which is neither an input nor a
                                                language-owned counter — the
                                                document did not check cleanly"
```

Exit codes measured WITHOUT a pipe. My first reading had them both at 0, which
was `tail`'s status, not the command's — the same trap as reading a suite's
result through `| head`.

**Which side is wrong: the checker.** The language's own rule is stated by the
checker itself elsewhere — *"`visits` is the builtin visit counter — it is
readable only in the outcome guards of the step that declares `max … visits`"*.
It enforces that for `answer rounds(visits)` and misses it for
`route done(rounds: visits)`. The outcome **payload** is an unhandled position
in the same scope rule. So the payload use is genuinely illegal and `check`
has the hole; `emit` is right to refuse.

Two things follow, and neither is cosmetic:

1. **`check` is the authoring gate.** It is what the skill tells an author to
   run until clean, what the LSP surfaces, and what a CI job would gate on. A
   green from it means "this document is internally consistent". Here it wasn't,
   and the author only found out at deploy — which on a live box is the worst
   place to find out.
2. **The error message blames the author for the thing that passed.** "the
   document did not check cleanly" is read by someone who has just watched it
   check cleanly. The message should name the illegal position.

Filed as a defect against the AWL checker, not worked around. The canary itself
uses a `const` for the round ceiling, so the bound is written once and read
three times rather than copied.
