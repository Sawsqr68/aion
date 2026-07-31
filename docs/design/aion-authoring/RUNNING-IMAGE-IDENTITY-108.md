# What code is Tom's live server actually running? (#108)

**Measured 2026-08-01, read-only, against pid 99844 on `:8080`. Nothing was written.**

## Why this had to be measured now and can never be measured again

The running image is unreachable on disk. A rebuild wrote over its path; the original
survives only as an open handle inside the process. Both preserved copies —
`target/release/aion` and `.aion-accepted-binary-20260731/aion` — are a *different, newer*
build (identical to each other, sha `f748956c…`, 58,245,680 bytes; the running image is
58,087,280 bytes at inode 1069431794). Found by Athena.

So restarting is a **substitution, not a restoration**, and until this measurement nobody
could say what was being substituted *for*.

**The process is the only artefact that can answer, and it answers only while it lives.**
It is also the only "endpoint absent" fixture that exists — every image that can still be
built has the routes it lacks.

## Method: the process testifying about itself

Probing which API routes the running image serves, and dating each route's introduction in
git. Every route present sets a lower bound; every route absent sets an upper bound.

**The status code is not the discriminator — the content-type is.** The deployed
composition falls through to the ops-console SPA, so an absent top-level path answers
`200 text/html`. A status-only probe would have reported every route present.

| path | live answer | verdict |
|---|---|---|
| `/outbox/dead-letters` | `POST` 415 | **present** — route matched, no body sent |
| `/awl/edit`, `/awl/scaffold`, `/awl/check` | `POST` 415 | present |
| `/workflows/intervene`, `/attempts`, `/transcripts`, `/reopen` | `POST` 415 | present |
| `/namespaces/records`, `/deploy/versions` | `GET` 200 **application/json** | present |
| `/workflows/count`, `/schedules` | `GET` 400 | present |
| `/queues/unserved` | `GET` 200 **text/html** | 🔴 **ABSENT** — the SPA answered |
| `/workflows/unrecoverable` | `GET` 404 | absent (expected — landed `e86378b2`) |

`405` on `POST` to an absent path is the static-asset service refusing the method, not a
route. `415` is a real handler rejecting a bodyless request. The two are the whole
distinction and they are invisible to a status-only reading.

## The result

| bound | commit | when |
|---|---|---|
| lower — `/outbox/dead-letters` present | `51411a41` | 2026-07-30 05:41:19 +1000 |
| upper — `/queues/unserved` absent | `24370cab` | 2026-07-30 21:36:42 +1000 |
| upper, tightened by start time | `2fd5cd63` | 2026-07-30 20:55:22 +1000 |

`ps -p 99844 -o lstart=` → **Thu 30 Jul 20:59:46 +1000**, four minutes after `2fd5cd63`
and **37 minutes before `24370cab` existed at all**.

> **The running image is built from the window `51411a41..2fd5cd63` — 76 commits, all on
> 2026-07-30, and the build happened within minutes of `2fd5cd63`.**

Two independent methods agree: the route probe and the process start time each exclude
`24370cab`, from opposite directions. Neither was derived from the other.

## What it does not narrow, and why I stopped

Only ONE router change lands inside the window (`24370cab` itself, the upper bound), so no
further route probe can discriminate. Narrowing past this needs either a response-shape
difference nobody has catalogued, or a WRITE to Tom's live server — **which is refused**.
76 commits is the honest floor for a read-only method.

## For the restart decision

The substitution is now **bounded and nameable** instead of unknown: restarting moves the
server forward across a known 76-commit window plus everything since. That is a decision
someone can actually take. It was not one before.

Nothing here helps pid 99844 itself. Nothing can. `GET /build` (`6f18949f`) means no
future server ever needs this reconstruction — and this reconstruction is the last time
the question has to be answered the hard way.
