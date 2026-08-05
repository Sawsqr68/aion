# Seat resume — marker AM, 5 August 2026

Supersedes AL. **Latest letter wins.** AL's record stands as history (it holds
the #102 analysis); everything below is newer. AK holds #84, the R2.4
amendment, and the #101 re-measurement.

`origin/main` = `HEAD` = `75dbbc4ff`. **Tree clean** but for the three NO-SWEEP
untracked dirs.

## ✅ #102 CLOSED — `75dbbc4ff`, pushed

HTTP start/signal decoded an `input` envelope as `ProtoPayload`, whose
`content_type` is the gRPC wire string `"application/json"`. The HTTP surface
EMITS `aion_core::Payload`, serde form `"Json"`. So a payload read off a run
could not be handed back: `content_type_from_wire`
(`crates/aion-proto/src/convert.rs:459`) accepts exactly `"application/json"`.

**Proven live on the running server, with a control, before the fix:**

| submission | result |
|---|---|
| plain JSON input, unknown workflow type | `404 WorkflowTypeNotFound` — past the payload layer |
| the server's OWN emitted envelope, same unknown type | `400 payload content type is unknown` |

The two differ in nothing but the payload form, so the refusal is attributable
to it. The unknown type meant neither could start a run — no side effect on
Tom's store. This is the method worth keeping: **an unknown-type start is a
side-effect-free probe of everything upstream of type resolution.**

### The fix

`http_input_payload`'s envelope branch decodes `aion_core::Payload` and
converts via the public `impl From<aion_core::Payload> for ProtoPayload`.
Refusal message shows the accepted form. Three unit tests replace one seam test
that asserted `"application/json; charset=utf-8"` passes through — a
pass-through the next layer refuses, so it could not have been true end to end.

**Gated at the ROUTE, not just the function**, because that is where the defect
lived. `assert_start_emitted_envelope_reaches_type_resolution` submits an
envelope serialised from a real `aion_core::Payload` and requires `404`, which
is only reachable by getting past normalisation; its control is
`assert_start_plain_json_missing_workflow` reaching the same verdict with a
plain input. Mutation-checked at my own hand: reverting the decode kills it
with `left: 400, right: 404` — the live probe's own numbers, reproduced
in-process. Restored by file copy, verified by `diff` + grep (`RESTORE-VERIFIED`,
mutation-string count 0).

**🔴 The fix changed what an existing assertion measured.**
`assert_start_invalid_payload_envelope` used `"application/json"` with
malformed `bytes`. After the fix it would be refused on the *content type*, not
on the bytes it names — still green, no longer measuring its own claim.
Respelled to `"Json"`. **Generalise: narrowing an accepted set can silently
turn a downstream assertion vacuous. When you narrow, re-read every test that
submits the old wide form.**

### Docs moved with it — every line settled against served bytes

`docs/operations/API.md` described raw protobuf shapes this surface stopped
emitting. Corrected from a live `GET /workflows` and `POST /workflows/describe`:

- start returns flat `{"workflow_id":"…","run_id":"…"}` — **not** nested
  `{"uuid":…}`. The old parenthetical claiming `aion start` "flattens" it was
  wrong twice: there is nothing to flatten.
- `GET /workflows` carries no `run_id` / `start_time` / `close_time` /
  `search_attributes`; `failed_step` and `failure_reason` appear only on a
  failed run (`skip_serializing_if`).
- describe returns `{summary, history, unserved}` — plain `WorkflowSummary` and
  `Event`, status at `summary.status`, no `{content_type, data}` wrappers.
- the start input envelope, with `Json` named as the only content type, so the
  envelope buys round-tripping rather than extra encodings.

**The WebSocket subscription examples were checked and LEFT ALONE**: their
nested `{"uuid":…}` ids are correct, because `PerWorkflowSubscription`
(`crates/aion-proto/src/events.rs:194`) genuinely holds `Option<ProtoWorkflowId>`.
The same-looking shape is wrong on REST and right on the socket. The asymmetry
is now stated in the doc. **Correcting a doc to something false is worse than
leaving it — verify each claim separately even when they look like one claim.**

### Gates

- `cargo fmt --all` clean.
- `cargo clippy -p aion-server --all-targets -- -D warnings` **EXIT=0**.
- `cargo test -p aion-server` **EXIT=0 — 950 top-level passed / 0 failed /
  0 ignored, 42 targets.** Baseline 948 at `e314caf1`; +2 for three tests
  replacing one. The two new route assertions ride inside an existing test
  function, so they add coverage without adding a count.
- Doc example gate **EXIT=0, 43 PASS / 1 UNVERIFIABLE / 2 ILLUSTRATIVE** —
  unchanged. **It reads ```awl blocks only and API.md has none, so API.md's
  JSON examples are UNGATED.** Today's corrections are true against served
  bytes and nothing keeps them true. Named, not built — a JSON doc gate is its
  own decision.

### 🔴 Counting correction to marker AK

AK recorded "951 passed / 46 targets". Both figures were off. Three
`test result:` lines are **nested re-execs of the lib binary** (they carry a
non-zero `filtered out` count) whose passes are already inside the parent's
769 — so the naive sum double-counts them — and there are **42** targets
(41 binaries + `Doc-tests`), not 46. Like-for-like: **948 → 950**. The verdict
is unchanged; the numbers now mean what they say.

**Rule:** a `test result:` line with a non-zero `filtered out` is a re-exec,
not a target. Count with
`awk '$12+0==0 {top+=$4}'`, never a bare sum.

## Board

- ✅ #84 (`e314caf1`), #144, #145, #101 (`2afd537db`), #102 (`75dbbc4ff`).
- ✅ R2.4 discharged by Amendment 1 in `BRIEF-139-LEG-A-REWORK.md` (`cfc4ac42`).
- ⛔ **#94 examined and deliberately NOT taken** — its intervals are configured
  values needing discussion, and it must land as one policy with #58. Route it.
- 🔴 **Route-retire / type accumulation is Waffles' to take to Tom** as a design
  question. **Do not build against it.**
- Held on Tom: the #139 live cut, #142, the 0.11.1 decision. Queued: #141 —
  and #102 is a worked example of exactly what #141 sweeps for.

## Method earned today (all now memories)

1. A mutation check only measures what the test actually EXECUTES.
2. One truth kept in two places, equal only because a person keeps it equal,
   has drifted or it will.
3. Confidence is what stops you measuring, so the confident claim is the one
   that most needs it.
4. **New, from #102:** an unknown-type start probes everything upstream of type
   resolution without side effects — use it to attribute a live refusal to the
   layer that issued it. And: **narrowing an accepted set can quietly make a
   downstream assertion vacuous.**

Waffles' framing, worth keeping whole: *carefulness at the moment of writing
does not survive into the moment of execution — only mechanisms do.*

## Hazard passed on by Athena (Doug's seat, 2026-08-05)

A dispatched agent's gate output overwrote its own seat's scratchpad evidence —
they collided on a filename, silently, discovered only on going to read it.
**A citation can point at bytes a later sibling replaced.** Cure: namespace
scratchpad evidence by writer. This seat adopted a `seat-*` prefix from this
marker onward (`seat-102-*.log`). No agents were dispatched during #102, so no
evidence cited above is at risk.

## Unchanged standing constraints

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker, and
restarting an already-built worker is not a rebuild. **Editing aion-server
source is not a worker rebuild, but do not restart Tom's live processes.**
🔴 Tom's `~/.aion/config.toml` `[ops_console] FileSystem` line must stay until he
installs a binary built from `b20d2da1` or later. 🔴 beamr JIT blocker with
Artemis. ⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
Backticked commit messages via `git commit -F -`. Do not edit `CLAUDE.md`.

Live pids, untouched all session: server **45425**, worker **48399**, both from
the repo root. Server log: scratchpad `server.log`.
