# The documentation site — a shape, not a build

**For Tom. 5 August 2026.**

You said: *"we're going to need to put together a documentation side at some
point. So start thinking about that."* This is the thinking, not the doing.
Nothing here has been built. It is here so you can point at the bit you want
and say no to the rest.

---

## What you asked

A documentation site for Aion — somewhere to send the people you are walking
through AWL, so they have something to read after you stop talking.

## Why you should care about the answer

Because the obvious version of this is the one that hurts us later, and it is
worth thirty seconds to avoid it.

The obvious version is: someone opens a docs tool, and writes the docs into
it. Now every fact about Aion exists in two places — the repository and the
site — with nothing forcing them to agree. Within a month they disagree.
Within three, the site is the thing strangers read and the repository is the
thing that is true, and we have shipped a liar with a nice font.

We already have a name for this failure and it has bitten us twice this week.
A rule known in two places, with nothing forcing them to agree, has already
drifted.

## Where we actually stand

Better than I expected. **The writing is largely done.**

| | pages | words |
|---|---|---|
| `README.md` + `docs/README.md` | 2 | 2,357 |
| `docs/GETTING-STARTED.md` | 1 | 5,245 |
| `docs/authoring/` | 6 | 11,185 |
| `docs/workers/` | 5 | 11,605 |
| `docs/operations/` | 4 | 12,597 |
| `docs/contracts/` | 2 | 4,187 |
| `docs/examples/` + `docs/demo/` | 3 | 4,536 |
| `examples/assistant/resources/` (incl. the AWL reference) | 5 | 10,191 |
| **total user-facing** | **28** | **61,903** |

(Counted at my hand, 5 August. Design notes, evidence records and the archive
are excluded — they are ours, not a reader's.)

That is a book. It is cross-linked, the README points at the AWL reference,
and Getting Started points at it twice. The AWL reference lives under
`examples/assistant/resources/` because the *assistant* reads those files as
its own knowledge — one copy serving both the agent and the human. That is the
right call and it should stay.

So this is not a writing problem. **It is a presentation problem with a
truth-maintenance problem hiding inside it.**

## The truth-maintenance problem, with a number

Across the user-facing docs there are **44 AWL code blocks**. Nothing checks
any of them. They are pictures of the language, not the language.

I have a fresh measurement of what that costs. Today I built your AWL deck and
treated *"make sure that it's accurate"* as a gate rather than an intention:
every AWL example on a slide was turned into a complete document and run
through `aion awl check` before it was allowed on the slide.

**One in twelve was wrong.** I had written `answer!.reviewer` — an unwrap
syntax that does not exist in AWL and never has. I wrote it carefully, with the
reference open, an hour earlier. The checker refused it. Without that gate it
would have been on a slide, in front of an audience, in your voice.

If I get one in twelve wrong writing carefully today, the 44 blocks that
nobody has ever checked are not all right.

## What I recommend

Three pieces, in this order. The first is the one that matters.

**1. Gate the examples before building anything.**

Extract every AWL block from the docs, wrap the fragments into complete
documents, run `aion awl check` over all of them, and fix what fails. Then keep
that as a check that runs on every change, so a doc example that stops being
true stops being publishable.

This is worth doing whether or not a site ever exists. It is the difference
between documentation and decoration. And it is small — I did the same thing
for twelve documents today in a morning.

**2. Generate the site from the repository. Never author into it.**

The markdown stays exactly where it is. The site is a *view* of `docs/`, built
from it, and nobody ever edits the site. If a page is wrong you fix the
repository and the site follows. One copy, so there is nothing to drift.

Practically: a static generator pointed at `docs/`, a nav file that says which
pages are the front door and in what order, and everything else reachable but
not featured. No database, no CMS, no login.

**3. Only then, the front door.**

A landing page, a search box, and version labels tied to published releases —
so someone on 0.11.0 is not reading main. This is the part that looks like
"a documentation site", and it is deliberately last, because it is the part
that is worth nothing if the two above are missing.

## What I am not recommending

- **Not a rewrite.** The 62,000 words are good and current; several were
  re-verified against the code as recently as last week.
- **Not moving the AWL reference out of the assistant's resources.** One copy
  feeding both the agent and the reader is the property we want.
- **Not a hosted docs product.** Every one of them wants you to author into it,
  which is the failure mode above sold as a feature.

## What happens if you say nothing

Nothing gets built. I will do piece 1 — gating the 44 examples — because it is
worth doing on its own and it is cheap, and I will tell you what it found. The
site itself waits for you.

---

*Related: `MEASUREMENT-LAWS-2026-08-05.md` (law 4, the two-copies rule, is the
whole argument for piece 2). The deck this came out of is
`docs/authoring/awl-deck.html` — open it with `open docs/authoring/awl-deck.html`.*
