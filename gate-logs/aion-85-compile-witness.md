# aion#85 — the compile witness (direct, dated, from the running server)

Source: `/Users/tom/.aion/logs/server-stdout-respawn.log` (server pid 84023, aion
0.18.1 / beamr 0.19.2). Extracted 2026-08-19T03:4xZ **before** the server restart
that clears it. No instrumentation was added — beamr logs cranelift compiles at
INFO already.

## 1. Every JIT compile event in the whole log (17 total)

```
2026-08-18T15:39:06.174236Z
2026-08-18T15:56:28.219958Z
2026-08-19T02:15:53.112540Z
2026-08-19T02:38:50.310100Z
2026-08-19T02:46:09.381554Z
2026-08-19T02:50:25.364187Z
2026-08-19T02:57:33.280691Z   <-- fatal
2026-08-19T03:08:54.883517Z
2026-08-19T03:16:36.335391Z   <-- fatal
2026-08-19T03:21:09.702840Z
2026-08-19T03:21:26.299148Z   <-- fatal
2026-08-19T03:36:18.624343Z
2026-08-19T03:36:29.588515Z
2026-08-19T03:36:44.095943Z   <-- fatal
2026-08-19T03:36:45.174963Z   <-- fatal
2026-08-19T03:36:45.175437Z
2026-08-19T03:36:45.175889Z
```

(`grep "cranelift_jit::backend" | sed -n 's/.*"timestamp":"\([^"]*\)".*/\1/p'`)

## 2. Correlation with every death

| workflow | type | died | preceding compile | gap |
|---|---|---|---|---|
| bc957fdf | watch_inbox | 02:57:36.595 | 02:57:33.280 | **3.3 s** |
| cf164116 | watch_inbox | 03:16:39.675 | 03:16:36.335 | **3.3 s** |
| aa5b1437 | nosleep_wall | 03:21:26.563 | 03:21:26.299 | **0.26 s** |
| 66b4beef | nosleep_wall | 03:23:21.235 | — **none** | poisoned-cache death |
| e8bfbb79 | nosleep_wall | 03:24:17.157 | — **none** | poisoned-cache death |
| e9c9d386 | shape3_wall | 03:36:44.406 | 03:36:44.095 | **0.31 s** |
| 2a09c6e0 | sleep_wall r2 | 03:36:46.216 | 03:36:45.175 | **1.04 s** |
| a48b0fd8 | watch_inbox | 03:41:37.512 | — **none** | poisoned-cache death |

Two-sided, and that is the point:

- **First death of a workflow type is always preceded by a compile within 0.3–3.3 s.**
- **Repeat deaths of an already-poisoned type have no compile at all** — they die on
  the cached broken code. The *absence* of the event is as diagnostic as its presence.

## 3. Base rate — stated honestly, because it qualifies the claim

11 of the 17 compiles are **not** followed by a death. Compiling is not sufficient.
Notably 02:50:25 fired *mid-run* of watch_inbox bc957fdf (02:47:03–02:57:36) and did
not kill it; 02:57:33 did. So the fatal event is compiling **the particular MFA on the
suspend/resume path**, not compiling anything. Any claim stronger than that outruns
this evidence.

## 4. Finest-grained trace — shape3_wall, the first resume through new code dies

Activities were completing every ~0.1–0.25 s, metronomically:

```
03:36:43.967537Z | activity completed
03:36:44.063614Z | activity completed
03:36:44.095943Z | *** JIT COMPILE ***
03:36:44.310562Z | activity completed
03:36:44.406349Z | activity completed
03:36:44.406464Z | activity completion delivery failed
                 |   error: "runtime error: process 13 is not live"
```

The workflow process is dead ~310 ms after the compile. The activity itself
**succeeded** (`outcome: succeeded, duration_ms: 2`); only the delivery of its result
found no live process. The workflow died on the resume, not on the work.

Same shape on Tom's pipeline at 03:41:37:
```
"activity completed" activity_id=activity:0 activity_type=scan_inbox outcome=succeeded duration_ms=2
"activity completion delivery failed" error="runtime error: process 14 is not live" workflow_pid=14 correlation_id="activity:0"
```

## 5. The delivered value is the suspension's own token

| arm | crash | the binary in the error |
|---|---|---|
| activity (nosleep_wall, shape3_wall, watch_inbox) | `{bad_generator, <<"activity:200">>}` / `<<"activity:0">>` | the **correlation id** |
| sleep (watch_inbox) | `{bad_generator, <<"3000">>}` | the **sleep duration in ms** |
| sleep-only (sleep_wall r2) | `VM execution error: bad argument` | — |

`bad_generator` is a list comprehension handed a non-list. The stack is
`gleam_json_ffi:...:-object/1-lc$^0/1-0-` at line 63 — the comprehension *inside*
`object/1`, which expects a list of field pairs. So the suspension token arrives
where the awaited result belongs: **right value, wrong destination**, literally.

## 6. Destination probe verdict — the destination does NOT move

`shape3_wall` was built with a deliberately different import profile (2 workers, 2
actions, extra types, Float + String + List fields) so the function and import tables
differ, and it deployed under a different module hash (`d7cbc819` vs `1c9fad3f`).

Result: **identical destination** — `gleam_json_ffi:object/1` line 63, same
comprehension — and an identical death point: activity 200, 607 events, byte-identical
to `nosleep_wall`. Function-specific, not position-dependent.

## 7. Sleep-arm verdict — compiled sleeps break, and no worker is involved

`sleep_wall` run 1 completed clean at **901 sleeps** (below the 1000-call threshold —
confounded, discarded as evidence, per Artemis's catch). Run 2 on the **same module**
`1c9fad3f`, carrying run 1's count, **failed after 102 sleeps** (cumulative ~1003).

`sleep_wall` has no worker and no activity at all. So the defect is in the durable
**wait/resume path itself**, not in activity dispatch.

The ~1003-vs-1000 fit is a *fit*, and I have been fooled by one of these before (1012
vs 1000, attributed to the native-services defect, refuted by soak). It is offered as
corroboration only — section 2 is the evidence. The decisive upgrade is a
proportionality test: set `jit_threshold` to 100 and both arms should die at ~1/10th
the count (sleep ~100 rounds, activity ~20 activities). That predicts *two* numbers
rather than explaining one, which is why it is worth the knob.

## 8. Operational consequence

Poisoning is **per module** and **survives across runs**; only a server restart clears
it. Tom's product pipeline dies after ~200 rounds (~10 min) and thereafter cannot
start at all — the restart at 03:41:34 died in **3.1 seconds** at `activity:0`.
