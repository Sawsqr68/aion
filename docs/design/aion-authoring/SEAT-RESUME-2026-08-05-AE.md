# Seat resume — marker AE, 5 August 2026

Supersedes AD. Latest letter wins.

## 1. THE DECK IS REBUILT AS TOM ASKED ✅ (`4d11fdf5`)

His words (03:01Z): *"shift the focus of the deck not to make it so much promo
kind of language… I actually sort of wanted a bit like a keyword breakdown…
because that's the kind of stuff that I really want to be able to walk people
through myself."*

`docs/authoring/awl-deck.html` — **27 slides, one construct at a time.** Each
carries the bare grammatical form, a real excerpt, and the rules that bite. The
sell is cut. Open it with `open docs/authoring/awl-deck.html`.
🔴 **A LOCAL FILE, NEVER AN ARTIFACT.**

### The method matters more than the deck
- `docs/authoring/deck-examples/*.awl` — **12 complete documents, all
  `aion awl check` clean.**
- `docs/authoring/build-deck.py` — slices excerpts out of those documents by
  anchor and **refuses to write the deck** unless every document in that
  directory checks. Not only the ones a slide quotes.
- `docs/authoring/deck-shell.html` — CSS/JS shell with a `<!--SLIDES-->` slot.
- Rebuild + re-verify: `python3 docs/authoring/build-deck.py`.

### What that fixed
The previous deck's caption said *"12 documents, checked against aion 0.11.0"*.
Those twelve were real and were checked — **in a scratchpad, never committed**.
The claim could not be re-verified by any reader, me included one session later.
Generated-not-tracked. The twelve are now in the tree and re-checked on build.

### Learned writing the examples
- `///` attaches only to **declarations**. Inside a step body use `//`.
- A subflow invocation is the **only statement of its step**.
- ⚠️ BSD `sed` does not understand `\s` — `^\(\s*\)` matched *column-zero*
  lines and silently demoted top-level `///` to `//`. Caught by reading the
  rendered output, not by the exit code.

### Still open on the deck
`04-declared-command.awl` and `03-agent-seam.awl` are quoted but the deck says
nothing yet about worker *registration* — how a worker advertises the action
surface a document declares. Candidate slide if Tom wants the worker story too.

## 2. FRESH-MACHINE TEST — CLOSED AND CLEANED UP ✅

Task **#143 closed**. Full record: `docs/evidence/setup-fresh-machine-2026-08-05.md`
(`6b6ad6c2`). Two real defects found and fixed; the whole path passes.

**Cleanup discharged this session:** isolated server pid 7887 and worker pid
14366 stopped, `/Users/tom/aion-freshtest-20260805` removed (2.5 GiB), Monitor
`bp8ga72oy` stopped. **Tom's server pid 85467 verified answering 200 on :8080
afterwards; his worker pid 66189 alive.** Neither touched at any point.

## 3. Owed to Tom

1. **Tell him the deck is rebuilt** — he drives demos by hand, so the crib is
   "open the file, arrow keys, F for full screen."
2. **The setup finding** is still worth telling him plainly if not yet done:
   the documented path could not have worked on his machine either, and now
   does. Four-part, plain language.
3. Docs-site shape already delivered (`DOCS-SITE-SHAPE.md`, `10d00d44`).

## 4. Documentation example gate — unchanged, still owed

`DOC-EXAMPLE-GATE-FINDINGS.md` (`0978e8c2`). 44 blocks, **8 verified, 36
outstanding.** Do not let "no false claims found" stand.

Open: `gated_item`/`gated_review` exist in BOTH `AWL-REFERENCE.md` and
`AWL-AUTHORING.md` and **the copies differ**. `AWL-REFERENCE__04` needs
`schemas/existing.schema.json`, which does not exist.

**Note the new leverage:** `build-deck.py`'s gate is the shape the doc-example
gate needs — check a directory of complete documents, refuse on any failure.
The 36 fragments need per-class wrappers to become documents first.

## 5. Standing constraints (unchanged)

🔴 Worker rebuilds frozen estate-wide until the #139 live cut. 🔴 beamr JIT
blocker with Artemis. 🔴 #142: merge `fix/worker-shell-dial-and-audibility` on
the cut, **then run the operator's real command**. ⛔ Never `git add -A` here.
⛔ Never bare `cargo clean`. Backticked commit messages via `git commit -F -`.
Do not edit `CLAUDE.md` — proposal sits with Waffles. Tom DM:
`dm:c9255b2a-5731-4d17-8124-e3bfa2224186`.

## 6. Commits this session

`10d00d44` docs-site shape · `0978e8c2` doc-example gate · `6b6ad6c2` setup.sh
fixes + evidence · `0b5f2cff` marker AC · `a9eb9520` first 5 examples + PASS ·
`4d11fdf5` the syntax deck + generator + 7 more examples.
