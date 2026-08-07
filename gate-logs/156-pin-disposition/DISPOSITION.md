# Disposition of the 140 `refs/rescue/20260806-*` pins (task #156)

Seat: Vesper Lynd. Date: 2026-08-08. Assigned by Waffles (no deadline; stated
failure direction: visible retention). Every command in this record is
read-only; nothing was deleted, repacked, or pruned in producing it.

## Provenance

All 140 pins were minted in one scripted sweep on 2026-08-06 during the
history-scrub aftermath (637 → 33 MiB). No minting record exists at any seat;
they were recorded inherited/ownerless at assignment. Names are
`refs/rescue/20260806-<sha>`.

## Census method

- `git for-each-ref refs/rescue` → 140 refs, all pointing at **commit** objects.
- Reachability: the full commit set of every non-rescue ref (9 local branches,
  14 remote-tracking, `refs/stash`, 3 tags, 3 `refs/yggdrasil/*`) was computed
  twice by independent methods (`git rev-list | wc -l` and
  `git rev-list --count`), both returning 5000 — the round number was
  re-verified rather than trusted. **Zero** of the 140 pinned commits are
  reachable from any live ref: every pin is the sole named retainer of its
  commit.
- Closure: `git rev-list <pins> --not <live refs>` returns exactly **140**
  commits — every pinned commit's parents are live. These are single dangling
  snapshots, not chains of lost history.
- Retained objects: 1517 total (140 commits, 880 trees, 497 blobs),
  **1.9 MiB on disk** (`git cat-file --batch-check='%(objectsize:disk)'`, sum).

## Classification (pins-census.csv carries the per-pin rows)

| Class | Count | What it is |
|---|---|---|
| metadata-twin-of-live-commit | 65 | (author-date, subject) pair exists verbatim in live history — the pre-scrub form of a commit whose rewritten form survives. Metadata correspondence only: **content identity was not proven**, and a scrub changes trees by design. |
| superseded-iteration-or-wip | 49 | No metadata twin. Reads as amend-iterations and abandoned WIP from the June build campaign (e.g. the same subject three times in ten minutes, only a later form surviving). |
| stash-entry | 26 | `index on <branch>` / `WIP on <branch>` / `On <branch>` commits — stashes swept up dangling. |

Commit dates span 2026-06-04 → 2026-08-06. Authors: tomWhiting 122,
JoshuaBearup 11, Benji 5, Annabel 2.

## The 1.3 GiB is not the pins

`.git/objects` is 1.3 GiB against a 33 MiB scrubbed history. The pins account
for **1.9 MiB** of that. The bulk is two overlapping packs:

- `pack-97149fed…` — 687 MiB, dated 2026-08-05 17:44 (scrub era).
- `pack-70069d3a…` — 631 MiB, dated 2026-08-08 00:35:50 — the minute the gc
  pair released (`gc.auto` unset + `gc.pruneExpire=never`). Auto-gc fired a
  repack on the next eligible command and, exactly as the pair intends,
  dropped nothing — duplicating still-retained bulk into a fresh pack.

What retains that bulk is **reflogs (3016 entries) plus `pruneExpire=never`**,
not `refs/rescue/*`. Largest single blobs: 110 MiB / 25 MiB / 24.9 MiB (new
pack), 58 MiB (old pack) — pre-scrub committed build artifacts. Attributing
this to the pins would be the global-gauge error; the local counts above are
the attribution.

## Disposition — RECOMMENDED, routed to Waffles before any action

**Retain all 140 pins indefinitely.** Grounds:

1. The total cost of retaining everything is 1.9 MiB — below any threshold
   that would justify an irreversible act.
2. 75 pins (superseded + stash) are the sole retainer of content that exists
   nowhere else. Releasing them discards it permanently.
3. The 65 twins are matched by metadata, not content. Proving content
   equivalence for a rewritten history is real work with no payoff at this
   size — and identity is content, not label.
4. The stated failure direction is visible retention. This disposition fails
   in exactly that direction, visibly, for 1.9 MiB.

**Consequence for the `gc.pruneExpire` row (Cally's, conditioned on this
disposition):** the pins are named refs, so their objects are ref-reachable
and immune to prune under any `pruneExpire` value. Restoring stock
`pruneExpire` therefore cannot touch anything this disposition retains. What
stock expiry *would* eventually release is the reflog-expired share of the
1.3 GiB — that is a separate decision about reflog retention on a scrubbed
repository, and it belongs to Waffles/Tom with the disk numbers above, not
to this record.

No pin is deleted, renamed, or rewritten by this disposition. If a future
ruling releases any class, the release must cite this census by pin sha and
land as its own recorded act.
