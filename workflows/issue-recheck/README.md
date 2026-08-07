# `issue_recheck` — is this reported-missing thing still missing?

A large share of our open issues are one shape: someone reported that a symbol,
a knob, or a relation is absent. Those reports rot. The only honest way to keep
one alive is to go and look again — and looking again by hand is where the
mistake lives.

## The mistake this exists to prevent

A search pointed at the wrong directory returns zero hits. A genuinely absent
symbol also returns zero hits. **The comfortable number and the true number are
the same number**, so nothing downstream can tell them apart and the wrong
answer arrives wearing the right answer's clothes.

This is not hypothetical. Verifying issue #150 by hand, the first search I ran
was scoped to the whole workspace instead of the server crate, came back with
55/78/13 hits, and read as "plenty of call sites — the report is false". It was
the wrong denominator, and it would have closed a real issue.

So the workflow asks a question whose answer it already knows *first*. If the
control symbol cannot be found either, the search cannot distinguish absence
from failure, and the run routes to `unmeasured` and reports **nothing**.

## Inputs

| Input | What it is |
|---|---|
| `issue` | Which issue this re-tests. Carried into the result so an answer is never orphaned from its question. |
| `scope` | Absolute path to search under. Must be absolute: a declared body runs in the server's own working directory, which the document cannot see. |
| `claim` | The literal string the issue says is missing. |
| `control_symbol` | A string certain to be present under `scope`. This is the whole safety mechanism — pick something that cannot plausibly be absent. |

## Outcomes

- **`standing`** (success) — the search was proved able to find things, and here
  is what it found. Carries `hits`, `control_hits`, and a `finding` written to
  say exactly what was measured.
- **`unmeasured`** (failure) — the control came back empty, so no finding is
  reported at all.

## What it does not prove

It measures whether a **literal string** appears under a path. That is evidence
about a claim of absence; it is not the claim itself. A feature can be present
under a different name, and a symbol can survive only in a comment. A `hits: 0`
means the string is not there — read it as that and no further.

The #152 run below is the worked example: the string `home_dir` came back
present, which looked like a refutation. Going and reading the one hit showed it
was a test helper, not config expansion — so the original claim survived, and
the trip to read it turned up the *second* half of the issue that hand-checking
had missed. The workflow's job is to send you to look, not to decide for you.

## Runs on the record

All against `aion 0.11.0`, 2026-08-07, re-run end to end on that date.

🔴 **`scope` must be ABSOLUTE.** A declared `run "…"` body executes in the
SERVER's working directory, not the caller's, so a relative scope resolves
somewhere else, the control returns 0, and the workflow refuses. That refusal is
correct — it is the document declining to report a finding it cannot support —
but the cause is a path, not a missing symbol.

`<repo>` is the absolute repository root. **Every column here is an input or an
output of the run** — that is the point of the table, and the first version of it
failed the test: it recorded a control COUNT with no `control_symbol` and no
`scope`, so no row could be re-run by anyone, including me. When I came back to
re-run them, the #150 rows' old control of 180 could not be reproduced by any
symbol I tried against that scope. The claims were fine; the evidence for them
was not checkable. A number without its owner is the thing this whole workflow
exists to catch, and the table had one.

| Issue | scope | `control_symbol` | control hits | `claim` | hits | Result |
|---|---|---|---|---|---|---|
| #150 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `visibility_reconciliation_interval` | 0 | absent — claim stands |
| #150 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `signal_delivery` | 0 | absent — claim stands |
| #150 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `completion_retry` | 0 | absent — claim stands |
| #107 | `<repo>/crates` | `pub fn` | 460 ⚠️ | `list_children` | 0 | absent across all of `crates/` — claim stands |
| #136 | `<repo>/crates/aion-cli/templates` | `workflow` | 42 | `.awl` | 0 | absent from the templates — claim stands |
| #152 | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `home_dir` | 1 | present — sent me to read it (see above) |
| control | `<repo>/crates/aion-server/src` | `pub fn` | 77 | `WorkerAvailabilityRequest` | 2 | present — proves the `now_present` arm is reachable |
| inverted | `<repo>/NO-SUCH-DIR` | `workflow` | 0 | `anything` | — | **refused**, exit 1, no finding reported |

The last two rows are the ones that make the other six mean anything. Without a
proven `now_present` arm, "absent" could be the only answer the document can
produce. Without the inverted control, a broken search would report absence with
the same confidence as a real one.

⚠️ **The #107 control is a function of BUILD STATE, and its first recorded value
proved it.** This row originally recorded 479; a later re-run of the document's
own command gave 460, because `<repo>/crates` contains git-ignored build output —
104 of the 460 hit files are git-ignored build artefacts under the fixtures'
`build/` trees — 92 vendored packages (`crates/aion/tests/fixtures/*/build/packages/gleam_*`)
plus 12 compiler-generated Erlang sources (`…/build/dev/erlang/*/_gleam_artefacts/*.erl`),
all of which exist only after the fixtures have been built. The number a fresh
clone reproduces is the **tracked-only** control: `git grep -lF -- 'pub fn' --
crates` → **356**, and that is the durable owner of this row's claim (the claim
itself is build-state-immune: `list_children` is 0 in both the full tree and the
tracked set). The workflow searches the filesystem, so its control will keep
moving with build state on this scope — a scope containing generated files makes
the control a lower-bound proof that the search works, not a reproducible count.

## Running it

```sh
aion awl check workflows/issue-recheck/issue_recheck.awl

aion run workflows/issue-recheck/issue_recheck.awl --input '{
  "issue": "150",
  "scope": "/absolute/path/to/crates/aion-server/src",
  "claim": "visibility_reconciliation_interval",
  "control_symbol": "pub fn"
}'
```

No worker is needed. The one action carries a `run "…"` body, so the server
executes it directly — see [`DECLARED-COMMANDS.md`](../../docs/workers/DECLARED-COMMANDS.md),
including the part where it says there is no sandbox.
