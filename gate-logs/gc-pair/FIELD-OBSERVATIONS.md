# The gc pair — field observations

The pair: `git config --unset gc.auto` + `git config gc.pruneExpire never`,
released in one act at this repository on 2026-08-08 00:35 (Waffles' row-8
ruling, Vesper's hand, Cally verified). Design intent: auto-gc regains
maintenance, but pin or ref deletion can never convert into object loss —
the `=never` floor makes prune a no-op regardless of what fires it.

This file is the pair's evidence trail. One entry per observation of the
pair operating under real pressure.

## 2026-08-08 — first field observation: auto-gc fired, the floor held

Within the same minute as the release (00:35:50), auto-gc triggered a repack
on the next eligible command and produced `pack-70069d3a…` (631 MiB) beside
the scrub-era `pack-97149fed…` (687 MiB). The new pack **dropped nothing**:
every object retained by reflogs (3016 entries) or by the 140 `refs/rescue/*`
pins was carried into it. That is the pair working exactly as designed —
maintenance ran unattended, and the `=never` floor converted what would have
been a prune into pure duplication.

Cost of the observation: ~631 MiB of transient pack duplication on a
repository whose reflog retention already materializes ~1.3 GiB of pre-scrub
objects. The duplication is reclaimable by any future repack once the reflog
question is ruled; the reflog question itself is on Tom's docket (routed by
Waffles 2026-08-08 with the census numbers from
`gate-logs/156-pin-disposition/DISPOSITION.md`).

Discovered during the #156 pin census (commit 55b3e6af1), ratified into this
trail at Waffles' direction: the first observation of the row-8 law doing its
job under real gc pressure.
