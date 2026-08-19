# Orphan mm-capture disposition record (#262 dig follow-on)

## Ruling
Waffles 03:53Z: verified-idle check at my hand; if orphan 77649 holds no
claimed work, terminate it, captured pid only, evidence banked. 46640
untouched either way. (NB: that ruling was issued while 46640 was believed
to be the new server's spawn — see census correction below.)

## Verified-idle evidence (pre-kill, 04:1xZ)
- 77649: state SN, 0.0 %cpu, TOTAL cputime 1.38s over 9h28m of life; one
  ESTABLISHED gRPC connection to :50051; command string verified verbatim
  (aion worker awl mm_capture_worker.awl --identity mm-capture-worker).
- All four Running-projected mm_capture_run workflows are STALE: last
  events 2026-08-11T14:32 (WorkflowFailed), 08-11T23:03 (ActivityScheduled),
  08-11T23:53 (WorkflowFailed), 08-12T05:43 (ActivityCompleted). No capture
  dispatch in ~4 days; nothing claimed, nothing in flight.
  (Side-note defect face: two of those show WorkflowFailed history under a
  Running list projection — the #174-adjacent staleness class.)

## Action
04:1xZ: SIGTERM to 77649 (command-string check gating the kill in the same
compound); exited on SIGTERM within 3s; confirmed gone via ps.

## Census correction (post-kill)
The pair was actually a QUADRUPLE. mm-capture instances at 04:1xZ:
- 99543 — the NEW server 98979's true managed spawn (boot log line
  "managed worker spawned ... pid":99543; etime matches boot). KEEP.
- 77649 — orphan of the 18xxZ incarnation (74870). KILLED per ruling.
- 46640 — etime ~11h → spawned ~17:1xZ 08-15 = orphan of the 17xxZ
  incarnation, NOT the new spawn. HELD (ruling's letter said untouched).
- 56246 — etime ~1d2h → orphan of an incarnation ~2 days back. HELD
  (outside the ruling's letter entirely).
Both held orphans show the same idle profile class; disposition of
46640/56246 awaits Waffles' word under the corrected identification.

## Standing observation
Managed workers surviving their server's death and re-registering beside
the replacement's fresh spawn is now a THREE-deep pile-up — the identity-
collision detection gap (#254/#258 adjacency, FINDINGS.md §7.5) is not
hypothetical.

## Fresh word executed (Waffles 03:56Z, corrected-census disposition)
04:2xZ: both remaining orphans dispositioned under the same gate:
- 46640: state SN, 0.0 %cpu, cputime 1.62s over ~11h — idle-gated,
  command-string verified in the compound; SIGTERM; confirmed gone in 3s.
- 56246: state SN, 0.0 %cpu, cputime 4.08s over ~26h — same gate;
  SIGTERM; confirmed gone in 3s.
- 99543 (keep, identified by the boot log's own "managed worker spawned"
  record): alive post-kills, etime 01:02:26.
Pile-up banked per ruling: 3 same-identity orphans across 3 incarnations =
the #254/#258 identity-collision gap concrete + the provenance-row
argument (a worker should disclose which incarnation spawned it; a census
must read identification off records, not etime arithmetic).
