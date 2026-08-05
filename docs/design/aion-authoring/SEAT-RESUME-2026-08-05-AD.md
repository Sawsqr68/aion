# Seat resume — marker AD, 5 August 2026

Supersedes AC. Latest letter wins.

## 1. THE FRESH-MACHINE TEST IS DONE AND PASSED ✅

`scripts/setup.sh` now runs clean end to end from nothing. **Verified, not
assumed:**

- `==> Done`, console at `http://127.0.0.1:18080`.
- Assistant deployed: `content_hash ae8b0ea1…`, `route_changed: true`.
- Worker **pid 14366** running, dialling `--address 127.0.0.1:18061` (the
  isolated port). Server-side registration count: 1.
- **Tom's server pid 85467 untouched**, still answering on 8080; his worker
  pid 66189 still on 50061.

Two real defects were found and fixed on the way — full record in
`docs/evidence/setup-fresh-machine-2026-08-05.md`, committed `6b6ad6c2`:

1. **Version skew (the blocker).** `setup.sh` required `workspace_root` from
   the startup banner; that field landed in `fe702945` *after* `v0.11.0` was
   published; the script installs 0.11.0. So it could not finish on any
   machine running the published CLI — Tom's included. Fixed by **installing
   the CLI from the checkout**, so script and server come from one tree.
   Bonus: a checkout-built server reports a real commit (`369b2dab…`,
   `dirty:false`) where a published one reports `unknown` — the #123
   capability, silently absent on every published install.
2. **120s of silence with no cause** on a failed server start, while the
   worker loop in the same file had always checked for a dead process. Now 2s
   with the server's own error quoted. Controlled both ways.

**Trap:** `git merge-base --is-ancestor` lies on a `--depth 1` clone (object
absent). Content check won. Identity is content, not label.

### 🔴 CLEANUP OWED
`rm -rf /Users/tom/aion-freshtest-20260805` — tens of GiB in Tom's home, plus
**stop the isolated processes first**: server on 18080, worker pid 14366.
Do NOT touch pid 85467 (his server) or 66189 (his worker).

## 2. TOM'S LIVE ASK — REBUILD THE DECK AS A SYNTAX BREAKDOWN

His words (03:01Z): *"shift the focus of the deck not to make it so much promo
kind of language… I actually sort of wanted a bit like a keyword breakdown…
breakdown of the syntax and all that kind of stuff. Because that's the kind of
stuff that I really want to be able to walk people through myself."*

I replied: yes, rebuilding it as a syntax walkthrough — each keyword, what it
does, a small real example, the rules that bite. Cutting the sell entirely.
**This is the active task.**

### Method (already in motion)
Every snippet on a slide must be an **excerpt from a complete document that
passed `aion awl check`**. Those documents live in
`docs/authoring/deck-examples/` so the deck's claims stay re-verifiable.

**Written and CLEAN so far (5):** `01-basics.awl` (types, enums, optionals,
const, header, route) · `02-workers.awl` (worker/action, config line,
node/timeout/retry, call override, advisory, pipes) · `03-agent-seam.awl` ·
`04-declared-command.awl` (`run` bodies) · `05-children-subflows.awl`
(child, spawn, subflow).

**Still to write:** steps/`after`/outcomes/`when`/`otherwise`/guard narrowing ·
`fork`/`join` (collection, `sequential`, named) · `distribute`/`sequence`/
`collect` (strict + tolerant `collect x?`) · `loop … max` + flow loop with
`max N visits` · `wait` / timed wait / `sleep` / `on failure` · `send … to` ·
`query`/`answer` · inline `schema {…}`.

**Learned building them:** a subflow invocation must be the ONLY statement of
its step — bind with `->`, then route from the NEXT step.

### Deck slide order planned
document shape & comments → types → schema doors → const → workflow header →
worker/action → config line → agent seam → run bodies → child/spawn → subflow →
step/after → `->` and `|>` → value ops & guards → outcome/when/otherwise/route →
fork/join → distribute/collect regions → loop/max → flow loops → wait/sleep →
send → query/answer → on failure → **the rules that bite** → the commands.

Target file: `docs/authoring/awl-deck.html` (replace the promo deck).
🔴 **A LOCAL FILE, NEVER AN ARTIFACT.** Tom: *"I don't want an artefact… I
don't like sharing those things."* He opens it with
`open docs/authoring/awl-deck.html`.

## 3. Documentation example gate — running total

`docs/design/aion-authoring/DOC-EXAMPLE-GATE-FINDINGS.md` (commit `0978e8c2`).

44 blocks. **Verified clean so far: 8** — 3 standalone, 1 via added narration
(`SHELL-WORKER-MANIFEST`), 4 via a type-fragment wrapper (`AWL-AUTHORING__02`,
`AWL-REFERENCE__02`, `__10`, `WORKERS__03`). **36 still unmeasured.** Do not
let "no false claims found" stand — most blocks are unchecked.

Open: `gated_item`/`gated_review` exist in BOTH `AWL-REFERENCE.md` and
`AWL-AUTHORING.md` and **the copies differ**; 4 fail on `declares no outcomes`
(elided, and unlike §1 their prose does not disclaim it). `AWL-REFERENCE__04`
needs `schemas/existing.schema.json`, which does not exist.

## 4. Standing constraints (unchanged)

🔴 Worker rebuilds frozen estate-wide until the #139 live cut. 🔴 beamr JIT
blocker with Artemis. 🔴 #142: merge `fix/worker-shell-dial-and-audibility` on
the cut, **then run the operator's real command**. ⛔ Never `git add -A` here.
⛔ Never bare `cargo clean`. Backticked commit messages via `git commit -F -`.
Do not edit `CLAUDE.md` — proposal sits with Waffles. Tom DM:
`dm:c9255b2a-5731-4d17-8124-e3bfa2224186`.

## 5. Commits this session

`10d00d44` docs-site shape · `0978e8c2` doc-example gate · `6b6ad6c2` setup.sh
fixes + evidence · `0b5f2cff` marker AC. All pushed.
