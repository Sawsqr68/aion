# Census correction — F2 REFUTED at my own read (Vesper, 05:5xZ 08-16)

## The error

My pre-dispatch census (census-raw.json, 04:5xZ) classified two rows as
"F2 — displayed Failed with NO terminal event in history":
e4ad76e8-3bd5-4e54-8943-5e537875bb22 and 70f3b331-54b4-4f24-8fdc-a54e51019f49.

That classification was an INSTRUMENT FAULT, mine. The history endpoint
returns 500 events by default and DISCLOSES the truncation — the response
carries `head_seq` and `next_from_seq` alongside `events` — and my census
script read only `events`, never comparing the last returned seq against
`head_seq`. Both specimens' histories continue past the window.

## Verified at my read (05:4xZ, live server, `limit` parameter)

- e4ad76e8: 604 events total; seq 604 = **WorkflowFailed**.
- 70f3b331: 605 events total; seq 605 = **WorkflowFailed**.

The displayed Failed was TRUE for both. There is no phantom terminal
writer; invariant 4 was not violated by these rows. The builder's delivery
report reached the same conclusion independently (its report's F2
correction), and Waffles relayed it; this file is the independent
verification at my own read.

## What stands

- **F1 (Running shown over dead runs, pre-restart)** — stands; fixed
  red-first in the status-truth branch (stale-visibility reconciliation).
- **F3 (opaque "workflow N returned error" on 53/56 Failed rows)** —
  stands; fixed red-first (recorded terminal error surfaced).
- **F2** — refuted. Struck from the defect list.
- The measurement lesson (the 21-rule class "a tool that fails into a
  comfortable number" / "count rows, check the KEY"): my script produced a
  plausible zero (no terminal event found) from a truncated window while
  the response itself carried the fields (`head_seq` 604/605 vs last
  returned seq 500) that would have exposed the truncation. Any future
  history-reading instrument MUST assert `last event seq == head_seq` or
  paginate via `next_from_seq` to completion.

## Consequence for the record

- The lane dispatch post fc3e19dd and my DM claims of "Failed with no
  ending recorded" are corrected by lane post (see correction beat).
- census-raw.json remains banked AS EVIDENCE OF THE INSTRUMENT, not of
  F2; its two STALE-PROJECTION rows carry this correction.
