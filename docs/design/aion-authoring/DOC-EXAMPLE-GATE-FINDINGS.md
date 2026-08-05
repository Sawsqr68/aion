# Gating the documentation's AWL examples — first pass

**5 August 2026. In progress; this records what has been measured and, more
importantly, what has not.**

The [docs-site shape](DOCS-SITE-SHAPE.md) puts one piece before any site: make
the AWL examples in the documentation check, and keep them checking. This is
the first pass.

## Method

Every ` ```awl ` fenced block was extracted from the user-facing documentation
into its own file, named for the document and block index it came from, so any
failure walks straight back to the line that made the claim. Each was then run
through `aion awl check` against the installed 0.11.0 — the same binary a
reader would have.

Extractor and results: `scratchpad/awlgate/` (not committed; regenerate with
`extract.py`).

**44 blocks** across 6 files. That figure was arrived at twice independently —
once by `grep -c '^```awl'` over the doc set, once by the extractor — and
agrees.

## Result

| | count |
|---|---|
| check clean as complete documents, unmodified | **3** |
| complete document, deliberately does not check | 1 |
| fragments — not documents at all, so **not yet verified** | 40 |

### The 3 that check clean

`docs/authoring/AWL.md` §1, `docs/GETTING-STARTED.md`, and
`examples/assistant/resources/AWL-AUTHORING.md` §1 — the three flagship
"here is a whole workflow" examples. The most-read examples in the
documentation are correct, which is the right place for them to be correct.

### The 1 complete document that does not check

`AWL-REFERENCE.md` §1, the canonical-order skeleton. It declares
`query status: Status` and never answers it, and the checker refuses:

> query `status` is declared but never answered — an operator asking for it
> would be told the workflow has no such query, which is not what this document
> says

**This is not a defect.** The prose immediately below it says: *"The sample
illustrates declaration placement; adapt the flow rather than copying unused
declarations."* The block is a placement diagram, and the document says so.

It is, however, the case that decides the gate's design. A gate that demands
all 44 check clean would force a change to a correct, deliberately illustrative
example. So the gate needs a way for a block to be marked illustrative — and
the mark must carry a reason, so that "illustrative" cannot become the place
wrong examples go to hide.

### The 40 that are not verified

These are fragments: one to sixteen lines showing a single construct. They fail
with *"a workflow document opens with `//!` narration"* — which is the checker
correctly observing that a bare `step` or `type` line is not a document.

By opening construct: 9 `step`, 6 `worker`, 5 `workflow`, 5 `type`, 4 narration,
3 `fork`, 2 `child`, and one each of `subflow`, `route`, `loop`, `on failure`,
`const`, and three bare call/constructor expressions.

**Verifying these is the actual work**, and it is not yet done. Each class needs
a wrapper that supplies the surrounding document its construct requires.

## What the five `workflow`-opening fragments showed

These were the cheap subset — near-complete documents needing only a narration
line. Adding one:

- **1 now checks clean** (`docs/workers/SHELL-WORKER-MANIFEST.md`).
- **4 fail** on `declares no outcomes`. These are the `gated_item` /
  `gated_review` pair, printed with their `outcome` lines elided for brevity.
  Their prose does *not* disclaim the elision the way §1 does.

### A law-4 instance found on the way

`gated_item` and `gated_review` each appear **twice** — once in
`AWL-REFERENCE.md`, once in `AWL-AUTHORING.md` — and **the two copies are not
identical**. The same named example, maintained in two places, with nothing
forcing them to agree. It has already drifted; that is what the rule predicts.

## What this does NOT establish

**No false claim about the language has been found so far. That is not evidence
that there are none.** 40 of 44 blocks have not been checked — they have only
been shown to be fragments. Reporting "the doc examples are fine" on this
evidence would be reporting the absence of a measurement as a clean result,
which is the exact failure this project keeps writing down.

The honest statement is: 4 of 44 verified as documents, 3 correct, 1 correct and
deliberately unrunnable, 40 outstanding.

## Next

1. Build the per-class wrappers and verify the remaining 40.
2. Decide the illustrative-block mark, with a required reason.
3. Fix what the verification finds.
4. Reconcile the duplicated `gated_*` examples down to one copy.
