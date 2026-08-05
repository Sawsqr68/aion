# Seat resume — marker AF, 5 August 2026

Supersedes AE. Latest letter wins.

## 1. DOC EXAMPLE GATE — DONE, EXIT 0 ✅ (`50426f50`, `c6f5f236`)

**40 PASS · 2 ILLUSTRATIVE · 2 UNVERIFIABLE · 0 FAIL.** Was 8 verified / 36
unmeasured at marker AE.

- `docs/authoring/doc_example_gate.py` — extracts every ```awl block from the
  six user-facing documents, wraps each fragment in the smallest document its
  construct needs, checks it, **and checks it again with the fragment's
  identifiers mangled**. A block whose mangled form still passes is reported
  `UNMEASURED`, never `PASS`. None were.
- `docs/authoring/doc_example_specs.py` — one entry per block; the gate
  **refuses to report at all** if a block has no entry, so a newly added
  documentation example cannot slip through unmeasured.
- `docs/design/aion-authoring/DOC-EXAMPLE-GATE-FINDINGS.md` — **generated**.
  Re-run the gate to re-derive it; do not hand-edit.
- Run: `python3 docs/authoring/doc_example_gate.py [--write] [--keep DIR]`.
  Exit 0 only when nothing fails.

### What it found and what was fixed
1. **Five blocks nobody could copy** — the whole `gated_item`/`gated_review`
   worked example, in BOTH `AWL-REFERENCE.md` and `AWL-AUTHORING.md`, printed
   with its `outcome` declarations elided and never disclaimed. Fixed
   additively: outcomes, the inputs the bodies already used, two missing `type`
   declarations, terminal routes, and an answer for `last_note` (declared,
   never answered).
2. **A sixth that only appeared when the harness stopped helping.** The gate
   had been lending `//!` narration to any whole-document block that lacked it.
   Removing that lending — subtraction — exposed
   `docs/workers/SHELL-WORKER-MANIFEST.md`'s worked example as passing on the
   gate's charity. Its narration now lives in the document.

### The "two copies" item — CLOSED BY MEASUREMENT, and the diagnosis was wrong

This section originally said `gated_item`/`gated_review` were two differing
copies and that choosing a survivor was a deletion decision to route. Measured
before routing, and the premise did not hold:

- The two are **not** copies of one rule. `AWL-AUTHORING.md` §7a demonstrates
  the minimum — one `send`, one `query`. `AWL-REFERENCE.md` §12/§13 demonstrate
  every payload form. Different scope of demonstration, same language.
- **They disagree on nothing.** Every claim AUTHORING §7a makes about targets,
  payload forms, inbound-`signal` refusal, and must-answer was checked against
  REFERENCE §12/§13 and agrees. AUTHORING already names REFERENCE as the
  authority in-text, so the *rule* lives in one place — the Law-4 cure was
  present the whole time.
- Both prose command claims were verified against the binary, not assumed:
  `aion query WORKFLOW_ID QUERY_NAME` exists; `aion awl schema --queries`
  exists and is documented as "the read-only surface … every declared `query`
  name". `AWL-REFERENCE.md:664` states the completed-run rule identically.

The one real defect was narrow: the small examples wore the **same names** as
the exhaustive ones, so a reader meeting both concludes one is stale. Renamed
to `item_announcer` / `review_reporter`. A rename, not a deletion — nothing to
route. Gate re-run: exit 0, 40 PASS / 0 FAIL, findings doc re-derives
byte-identical.

## 1b. COMMAND GATE — BUILT AND GREEN ✅ (`de4cc3ca`)

The prose gap named above is closed. `docs/authoring/doc_command_gate.py` walks
the real `--help` tree and judges all 94 distinct `aion …` strings across seven
documents (the six above plus `examples/assistant/resources/COMMANDS.md`).

**92 PASS · 1 ABSENT · 1 BINARY-ONLY · 0 FAIL.** Report is **generated**:
`docs/design/aion-authoring/DOC-COMMAND-GATE-FINDINGS.md`. Do not hand-edit.

```
python3 docs/authoring/doc_command_gate.py --self-check   # prove the refusals fire
python3 docs/authoring/doc_command_gate.py --write        # judge + regenerate
```

A PASS establishes only that the path resolves and every flag is accepted —
**not** that the flag means what the sentence says. The report states this.

### What it found — nearly all of it in the gate's own controls
1. Corrupting each claim caught **21 false passes** in run one: a bogus token in
   *subcommand* position was filed as a positional and waved through.
2. 17 more "failures" were the walk refusing root-level global flags written
   *before* the subcommand — the form every operator command uses.
3. The one genuine finding is a **true negative**: `WORKERS.md` names
   `aion worker python` to say no script host ships. The gate could not tell
   "this works" from "this does not exist". Denials are now declared in
   `ASSERTED_ABSENT` and checked **both ways** — build one and its denial fails.
4. 🔴 The coverage baseline was computed *with the extractor it audits*, so
   crippling the extractor reported zero misses. Now an independent expression
   (`BACKTICK_SPAN`). My own catalogued law, broken by my own hand again.
5. Denials keyed on the whole line never matched a command carrying arguments.
   Keyed on the resolved command path now.

`--self-check` re-proves all four refusals on demand; each exists because the
first version of the check it exercises passed while broken.

Also standing: `AWL-REFERENCE.md:92` imports `schemas/existing.schema.json`,
which the repository does not contain — recorded UNVERIFIABLE, deliberately not
papered over with a fixture (a fixture would not help a reader either).

## 2. THE DECK (marker AE, unchanged) ✅ `4d11fdf5`

`docs/authoring/awl-deck.html` — 27 slides, keyword by keyword, promo cut.
Generated by `docs/authoring/build-deck.py` from 12 checked documents in
`docs/authoring/deck-examples/`; refuses to build if any stops checking.
🔴 **LOCAL FILE, NEVER AN ARTIFACT.** `open docs/authoring/awl-deck.html`.

Tom has been told: the deck rebuild AND the fresh-machine setup answer.

## 3. Fresh-machine test — CLOSED ✅ (#143)

`docs/evidence/setup-fresh-machine-2026-08-05.md`. Cleanup discharged: isolated
pids 7887/14366 stopped, `/Users/tom/aion-freshtest-20260805` removed, Tom's
pids 85467/66189 verified alive afterwards.

## 4. Standing constraints (unchanged)

🔴 Worker rebuilds frozen estate-wide until the #139 live cut — note the doc
edits above touch the assistant's embedded resources but rebuild nothing.
🔴 beamr JIT blocker with Artemis. 🔴 #142: merge
`fix/worker-shell-dial-and-audibility` on the cut, **then run the operator's
real command**. ⛔ Never `git add -A` here. ⛔ Never bare `cargo clean`.
Backticked commit messages via `git commit -F -`. Do not edit `CLAUDE.md`.
Tom DM: `dm:c9255b2a-5731-4d17-8124-e3bfa2224186`.

## 5. Commits this session

`10d00d44` docs-site shape · `0978e8c2` first gate pass · `6b6ad6c2` setup.sh
fixes + evidence · `0b5f2cff` AC · `a9eb9520` first 5 examples + PASS ·
`4d11fdf5` syntax deck + generator · `def023ba` AE · `50426f50` the gate ·
`c6f5f236` every doc example checks. Push `c6f5f236` if not already pushed.
