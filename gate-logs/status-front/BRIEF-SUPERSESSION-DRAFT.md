# BRIEF-SUPERSESSION (DRAFT) — a superseded failure reads as history, not as alarm

**Status:** DRAFT FOR WAFFLES' REVIEW — authored during the amendment wait per
Tom's unfreeze (07:24Z 2026-08-16). Dispatch gated on the status-truth landing
(ruled 04:55Z: own follow-on, after landing). Drafted against the list surface
at e790ada74, which the amendment run does not touch.

## The problem this closes (the second half of Tom's complaint)

The status-truth branch makes every displayed status TRUE and every failure
reason HONEST. What remains is the wall-of-red multiplier: truth at row level
still misleads at list level. Census fact (Vesper, 04:5xZ 2026-08-16, live
server, banked at gate-logs/status-front/census-raw.json): of 56 Failed rows
in namespace `default`, **34 were followed by a later completed run of the
same workflow type** — infrastructure casualties and iteration artifacts that
the operator already recovered from, visually indistinguishable from the 22
failures that still matter. Tom opens the list and reads 56 alarms where ~22
are live.

**Ruled frame (Waffles 04:55Z):** list-surface presentation — superseded rows
are marked or grouped, NEVER hidden. This brief is presentation + derivation;
it is NOT projection-truth work (that landed) and NOT retention/deletion.

## The open design question this draft puts to Waffles first

**What relation makes row B supersede row A?** The census instrument used
"same workflow type, later run reached Completed" — right for measuring the
phenomenon, too coarse to ship as-is: two honest failures of a shared utility
type (e.g. `shell_run`) would mark each other superseded. Candidate relations,
strictest first:

- (a) Same continue-as-new / retry CHAIN (run_chain relation) — precise,
  already history-derivable, but misses the census's actual population
  (re-dispatched fresh runs carry new workflow ids).
- (b) Same workflow type + same display-name lineage — catches re-dispatches
  ("status-truth-fix2" → "status-truth-fix2-amend" would NOT match; a re-fired
  identical name would).
- (c) Same workflow type, any later Completed (the census instrument) —
  maximal recall, wrong precision for utility types.

Draft position: **(a) ∪ (b)** as the shipped relation, with the census re-run
against it as calibration (R1 measures how much of the 34 it captures and
names what falls outside). Waffles rules the relation before dispatch; the
rest of the brief is stable under any choice.

## Requirements (stable under the relation ruling)

R1. **Relation at the bytes + calibration.** Implement the ruled supersession
    relation as a pure derivation from history/visibility data (name the code
    path). Re-run the banked census against it: report captured/uncaptured
    counts of the 34, with per-row ids, as a committed evidence log. Counts
    carry their counter.
R2. **No stored status, no invariant erosion.** Supersession is DERIVED at the
    list/read layer (or a reconciled visibility attribute recomputed from the
    same inputs) — never a stored mutable flag an operator or recovery path
    must maintain. Invariant 4 (status is a projection) extends here: a
    superseded marking must never alter `status`, `failure_reason`, or any
    history byte. The superseding run's id travels WITH the marking (the row
    says what superseded it, not just that something did).
R3. **List surface: marked and groupable, never hidden.** The list response
    carries the supersession fact per row (field shape to match existing
    summary conventions). The console list renders superseded rows visually
    quieter (grouped or dimmed) with the count visible ("34 superseded"), an
    affordance to expand/collapse, and NO configuration in which they are
    absent from the data. Filters may target them explicitly
    (`superseded: true/false`) — an explicit filter is operator intent, not
    hiding.
R4. **The demonstration row.** Pin with tests at the server list layer: a
    failed run + a later run satisfying the relation → the failed row carries
    the superseded marking naming its superseder; a failed run with NO such
    later run → unmarked; the marking never appears on non-terminal rows.
    Console: rendering test that the group header count equals the marked-row
    count (no silent divergence between badge and rows).
R5. **Gates.** Full standing gate legs EXIT=0 single-instance at final HEAD;
    red-first ordering per face; honest delivery report.

## REQUIRED: Check tiers
R2's derived-not-stored is the structural tier for the marking itself. For the
console count (R4), mechanical: the count is computed from the same marked-row
set it labels.

## REQUIRED: Census controls
R1's calibration IS the census: denominator 34 (banked ids), instrument = the
shipped relation, positive control = a banked row known captured under (a) or
(b), negative control = a utility-type pair known NOT to satisfy the ruled
relation.

## REQUIRED: Numbers
142 rows / 56 Failed / 34 superseded-by-census-instrument — Vesper's count,
04:5xZ 2026-08-16, census-raw.json. All figures re-measured at build time
against the live census the run takes itself (the wild population has grown
since — today's fail-loud runs included, several of which are THEMSELVES
superseded specimens: 4c4ef686 read-path, 4c7c1127 fix2).

## REQUIRED: Refusal surface
n/a — no new refusals; the list gains a derived fact, refuses nothing new.

## REQUIRED: Timing sensitivity
Not timing-sensitive at test level (fixtures construct both runs explicitly).

## REQUIRED: Absence claims
"No stored mutable supersession flag exists" carries its search (grep scope +
denominator) in the delivery report.

## Hard constraints (estate law)
Fresh worktree off the POST-LANDING main (base = the landed status-truth
merge; NOT e790ada74 directly). Live :8080 READ-ONLY for the census. NEVER
push. No new dotfolders/daemons/target overrides. Single-instance gate law +
pre-battery census for a live server-spawned gate (both standing rules).

## Done means
The ruled relation shipped as a derivation; calibration evidence committed;
list + console carry marked-never-hidden superseded rows with superseder ids;
all pins red-first; gates green single-instance; delivery report states the
uncaptured remainder of the 34 plainly.
