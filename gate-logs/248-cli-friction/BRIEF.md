# #248 — CLI friction bundle: the daily verbs answer like an operator asked

Wave 1 of Tom's general go (01:54Z, "especially the uncontroversial
stuff... no cut corners"), carved by Waffles as one brief from his
operator-experience report §5 (docs/tracking/
aion-operator-experience-20260815.md). Every line below is measured
friction from one night of real operation.

## R1 — `aion list` filters and a human form

Defect: `aion list` returns the entire unfiltered history as one JSON
blob — 35KB of mostly-old runs before tonight's appear; every liveness
check tonight round-tripped through python.

- `--status <status>` filters on the status projection (repeatable or
  comma form — pick one, document it). Status vocabulary is the
  projection's own (Running, Completed, Failed, Cancelled, TimedOut,
  ContinuedAsNew, Paused) — derived from the aion-core enum, not a
  hand-list that drifts.
- `--since <when>` bounds by started_at. Accept RFC3339 and the humane
  relative forms (`2h`, `30m`, `3d`) — parse strictly, refuse garbage
  with the accepted grammar named in the error.
- Human table default when stdout is a TTY (name, short id, type,
  status, started, ended/duration), `--json` for the machine form —
  and when piped, JSON stays the default so existing scripts and
  tonight's python habits keep working unchanged. Timestamps carry
  explicit offsets (house law).

## R2 — `aion describe` (and every id-taking verb) accepts short ids

Defect: every surface (console, logs, list) shows 8-char short ids;
`aion describe` refuses them ("invalid length"), forcing expand-by-eye.

- Unique-prefix resolution against known workflow ids: unambiguous
  prefix (≥8 chars) resolves; ambiguous prefix REFUSES loudly, naming
  the matching candidates so the operator picks; unknown prefix says
  unknown. Never guess, never resolve a 50%-match silently.
- Sweep every id-taking verb by the claim's vocabulary (describe,
  cancel, signal, query, pause, resume, reopen, inspect, tail when
  #244 lands) — one resolution helper, one behavior; list every verb
  touched and every verb checked-and-left in the report.

## R3 — attempt and holder are displayed facts

Defect: nothing surfaces "this activity is on attempt 2" or which
worker holds it; Waffles learned attempt 2 existed via pid archaeology.
Post-#246 this is operator-SAFETY information: a surviving prior
attempt means a possible concurrent writer.

- `aion describe` renders, per in-flight activity: attempt number,
  scheduled/started times per attempt, and the holding worker identity
  as the server knows it (dispatch target). If the server does not
  retain the holder, surfacing what IS retained honestly (attempt +
  dispatch timestamps) is R3's floor and the gap is named in the
  report — not papered over.
- The console attempts panel gets the same fields (composes with #215's
  direction without taking #215's whole scope).

## Red-first discipline

Each R pins red first: R1 — a fixture store with mixed statuses/ages
asserts filtered/tabled output, committed failing against today's
firehose; R2 — short-id describe pinned failing on today's "invalid
length", plus the ambiguity-refusal case; R3 — a mid-flight activity's
describe output pinned to carry attempt identity, failing today.

## Out of scope

- #244's tail verb (its own brief; R2's resolution helper serves it).
- Server-side pagination semantics (#194 owns that defect).
- Any landing/swap/restart while fleets fly (standing wall).

## House laws

Worktree-only, commit-as-you-go, never push, red-first with quoted reds,
no lint bypasses, ≤500 LOC per file, clippy ×2 + fmt + battery to files,
box load recorded beside any red. Waffles' re-gate before landing.
