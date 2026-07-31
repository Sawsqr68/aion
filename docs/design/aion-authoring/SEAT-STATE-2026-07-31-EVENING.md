# Seat state — 2026-07-31, evening block

## 🔴 READ FIRST ON RESUME

**Nothing is in flight. Box idle. Tree clean, everything pushed.** No block is
open with Athena; any compiler invocation needs a fresh declaration + GO, and
must name the mutation class if it mutates source to probe coverage.

**THE TOP ITEM IS #85, and it outranks everything else on this board.** A test in
the durable write path failed once in 8 runs; two more in the same subsystem
fail the same way. Rate is **non-zero and unquantified** — one event in eight
gives a 95% interval of roughly 0.3%–53%, which cannot support a decision.
The libSQL error text ("bad parameter or other API misuse" under concurrency)
points at a shared-connection defect — if that is right it is a PRODUCTION bug in
the durable write path, and these flakes are the only thing reporting it. Test
that hypothesis before treating any of the three as test isolation.

⚠️ **CORRECTED — the earlier "every green carries that rate" claim was
DIRECTIONALLY WRONG and is struck.** A test that spuriously FAILS produces false
REDS. It does not taint greens: when the assertion passed, it ran and held. The
1021, the 1022 and the 58-targets-0-unpaired claim stand. What a red on this
command now means is *re-run before believing it*.
The structure that WOULD taint a green is a wait that can pass without
observing — checked by reading on 2026-08-01, and it does not exist here:
`wait_for_history` returns `Err` on timeout in both copies
(`crates/aion/tests/outbox_e2e.rs:183`,
`crates/aion-server/tests/run_server_outbox_support/helpers.rs:301`), no call
site swallows the result, and every predicate is an equality against a positive
constant (`== FAN_OUT` where `FAN_OUT = 4`, or `== 1`) that an empty or partial
history cannot satisfy.

**Then #121** (both recovery-loop guards uncovered; needs an adversarial
`EventStore` whose `list_active` returns a terminal/paused run anyway).
**Then #117's remaining work** — the operator-facing surface (crosses
`aion-server`, needs its own block) and requirements (1)-(4) of the ruling, which
is pinned verbatim below. Requirement (5) is already discharged as a test.

⚠️ **Do not inherit the superseded #121 framing.** I published that the two guards
had opposite coverage. That was a flake seen once. Both are uncovered.


Written to survive a compaction. `origin/main` = **`c1252dc5`**, everything
pushed, nothing in flight, box idle.

## Landed this session

| commit | what |
|---|---|
| `07ff0ec0` | #119 — a vacuous green announces its own vacuity; **the announcement was itself vacuous twice** |
| `5b9d5d39` | #119 docs — landing hash + #76 cross-reference |
| `7c9a2329` | #117 (b) — a run that can never become resident can now SAY so |
| `ecbf59ea` | #117 — Waffles' five binding requirements pinned verbatim |
| `4b6f01fe` | #120 — the obvious truncation control is wrong, cause confirmed |
| `476e5372` | #117 (5) pinning test — **and the guard it names has NO coverage** |
| `81c92490` | **#120 CLOSED** — the battery now checks a run reached the end |

Detail lives in `GATE-TRAP-119-STATE.md` and
`DEGRADED-RUN-QUERYABLE-117B-STATE.md`. Both are landed records, not plans.

## 🔴 #117 (c) — RULED YES, NARROWLY. FIVE BINDING REQUIREMENTS.

**Pinned here verbatim because this ruling was issued once at ~20:15 on 30 Jul
and DIED BETWEEN COMPACTIONS.** Waffles' own framing, kept as law:
*a ruling not in the builder's hands isn't issued.* Received via Cally
2026-07-31 14:54Z; that message is the authoritative copy.

**THE RULING: YES, narrowly — never-alive runs, cancellation only, nothing
else — under five binding requirements.**

1. *"The direct write goes through the SAME writer-exclusion primitive normal
   writers acquire, and re-proves both preconditions UNDER that lock at write
   instant: run not resident, package reload refuses NOW — measured, not cited;
   if reload unexpectedly succeeds, STOP and route to the normal verb."*
2. *"Terminal-append-only: exactly one cancellation record carrying who/when/why
   plus the measured refusal — and now also citing the engine's remembered
   non-viability reason, since she's landed that memory; the record and the
   reason should reference each other."*
3. *"The verb names the true state — never 'not found' for a row the read side
   serves as Running."*
4. *"Rehearse on a copy of the preserved fixture first; the first live case is
   0756ecd5 itself."*
5. *"BOOT HONORS A CANCELLATION RECORD BEFORE MINTING A WRITER — a cancelled run
   is terminal at boot, unconditionally."* — adopted as binding from Cally's
   routing note. Waffles' reasoning: requirement (1) proves vacancy at the write
   instant, which is **spatial**; redeploy is the intended remedy for stranded
   packages and is precisely the event that can make the impossible writer
   possible later — without (5), *"can never come alive" is a present-tense
   claim wearing an all-futures costume.* With it the invariant is **temporal**.

**Requirement (2) leans directly on `7c9a2329`** — the remembered non-viability
reason. Waffles: the memory fix *"converts the whole class from archaeology to
lookup."*

### How (5) discharges — agreed shape

(5) needs **no new code**. Verified by reading:
`repopulate_active_workflows` reads history → projects status → **skips if
terminal** → and only then mints a writer, and `WorkflowStatus::is_terminal()`
is true for `Cancelled` (`aion-core/src/status.rs:42`). Because status is a
projection (invariant #4), the cancellation append changes what the history
projects to, so every reader including that guard sees it with nothing to
remember. **The write closes the door behind itself.**

⚠️ That is a structural argument, **not an executed proof**. So (5) is
discharged as a **PINNING TEST, BUILT BEFORE THE FEATURE**: boot against a
history carrying a cancellation record, assert the run is never made resident
and no writer is ever minted. A load-bearing behaviour with no pinning test is
one refactor away from fiction.

### Build order when this lane is picked up

1. The (5) pinning test — FIRST, and it needs none of the rest.
2. (1) writer-exclusion + re-prove BOTH preconditions under the lock, measured.
3. (2) the single terminal record, cross-referencing the remembered reason.
4. (3) fix the verb's refusal so it never says "not found" for a served row.
5. (4) rehearse on a COPY of the preserved fixture; 0756ecd5 is the first live
   case. **Do not touch the preserved fixture itself.**

Not urgent — Waffles and I both say so. Build when reached.

## 🔴 OPEN AND WAITING ON SOMEONE ELSE

**#88** — awaiting Tom: is Gleam a supported authoring surface, or is AWL the
only one. HOLD OPEN.

## Next, in the order I'd take them

1. **#117 operator surface (BLOCK E, needs a declaration + GO).** Engine
   accessor exists: `registry.unrecoverable().get(&id)` / `.list()`. `describe`
   is CLI-side built from a server summary, and `WorkflowSummary` is built
   `from_history` — the degraded fact is NOT in history, it is engine state. So
   this crosses `aion-server` and possibly the wire contract. Not started.
2. **#120 — and it is bigger than the board says.** See below.
3. #71 (GitHub Actions disabled — repo setting, not mine to change), #76.

## 🔴 #120 — measured tonight, and the obvious fix is WRONG

`scripts/battery.sh` carries a comment about #120 (*"A count is only a coverage
claim if the run reached the end. Read the target list, not the tally."*) and
**no mechanical control**. That is *printed is not checked* — an instruction, not
a control — sitting inside the comment that states the law.

I have also been certifying every green tonight with
`awk '{p+=$4; f+=$6}'` over `test result:` lines, which is **exactly the
instrument #120 condemns**: if a run truncates, the tally sums fewer targets and
still reports `failed=0`.

**Tonight's greens are sound** — checked, all four logs have
`targets_started == result_lines`:

```
.evidence/117/tests.log              58 / 58   COMPLETE
.evidence/119/tests-A-allfeatures     3 / 3    COMPLETE
.evidence/119/tests-B-worker-default  2 / 2    COMPLETE
.evidence/119/tests-C-proto-default   1 / 1    COMPLETE
```

**But the obvious control does not work.** Measured against the real battery
logs, `started` and `results` DISAGREE on a fully green run:

```
.evidence/118/battery-final.log        started=45  results=48
.evidence/118/shakedown-allfeatures    started=240 results=243
```

**CAUSE CONFIRMED at the bytes**, not inferred. `battery-final.log:225-238`:

```
test config::load::home_tests::aion_home_env_probe_child ... ok

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 693 filtered out;
```

A test that **re-executes its own binary as a child** with a filter — the
`AION_HOME` env probe from #113 — and the child's `running 1 test` /
`test result:` land in the PARENT's stdout, mid-target. Three such probes, three
extra lines. Same family as #74's child-process pin.

⇒ **A naive `started == results` gate would fire on every green run.**

### The correct invariant, derived from that measurement

Don't count. **Assert the pairing**: every `Running` / `Doc-tests` line must be
followed by at least one `test result:` line before the next target line (or
EOF). Truncation shows up as a target with no result after it. Child
re-executions add EXTRA results inside a target's own block, which this
invariant tolerates by construction.

The counting version is wrong in both directions: it false-positives on child
output, and — worse — a truncation that happens to coincide with N child extras
would net to zero and read as complete.

⚠️ Note the filter trap: a legitimately filtered top-level run also reports
`filtered out > 0`, so "has filtered out" cannot be used to identify child
lines. The `posture:*` gates filter deliberately.

This is the **fourth** time tonight that measuring against real recorded output
caught a control that reasoning alone would have shipped broken.

## Block discipline (Athena's seat, and it is working)

- Declare any compiler-invoking BLOCK **and wait for the GO** — I breached the
  second half once at 14:08 and it cost nothing only by luck.
- The GO names exact commands. A third command needs a fresh declaration.
- **Take the baseline from Athena's log, not from my own first measurement.**
  Post-compaction, the earliest number I hold is not the earliest number that
  exists. This error occurred twice and both times understated the truth
  (Block C: reported +1.0 for a real +4.55; Block D counterfactual: ~4 for a
  real 12).
- `target/release` NEVER touched (#108 — Tom's live server, deleted binary).
- Band 50 GiB. Block D ended free 73, `target` 18G.

## Laws minted or sharpened tonight

- **A green can be a result about a different program** (#119).
- **Printed is not checked** — an announcement nothing compares is
  observability, not a gate.
- **`cfg!` is a compile-time constant**, so `assert!(!cfg!(..))` under the
  matching `cfg` folds to nothing. Use a `const` block; fail the BUILD.
- **A name that claims coverage the run does not have** is the same defect one
  level up from the vacuous green it was built to catch.
- **Check whether a control already exists before building one.** It saved two
  sub-items of #117 and a redundant gate.
- **Prefer a baseline comparison to an enumeration** — and tonight the baseline
  half failed FIRST, catching a test with no control in it.
- **An instrument returning zero is a claim about the instrument first**
  (zsh `$inv` not word-splitting; Athena's self-matching `pgrep`).
- **A monitor with no liveness channel and a healthy one are indistinguishable
  from inside** — the cure must be a property of the artifact, not something the
  monitor volunteers.


## 🔴 #121 — SETTLED, and I was wrong in the middle of it

**BOTH guards are uncovered.** Disabling either — `is_terminal()` at
`startup.rs:197` or `== WorkflowStatus::Paused` at `:212` — leaves
`cargo test -p aion-rs --all-targets --no-fail-fast` at **exit 0**.

I earlier reported they had OPPOSITE coverage, on a single EXIT 101 observation.
Refuted: the mutated tree passes twice, and a deterministic causal path cannot
pass. The failing test was a known-flaky `outbox_e2e` member (#85).

**The shape worth keeping:** comfortable explanation → measured, refuted it, felt
good → *the refutation became the new comfortable belief and got broadcast.* The
correction was more seductive than the error because it arrived wearing the
evidence of rigour. And the masking explanation I loudly discarded was correct:
both backends filter `== Running` (`memory.rs:374`, libSQL `read.rs:133`).

**Athena's asymmetry, which is why one block settled it:** mutated-PASSES is
strong refutation; mutated-FAILS-twice is weak confirmation, because **p² is not
distinguishable from causality without knowing p**. I had built an enumeration
and called it an experiment — three hours after writing a seam test to avoid
exactly that shape.

**Fix:** an adversarial `EventStore` whose `list_active` returns a terminal (and
separately a paused) run anyway. No cheap route exists; that depended on the
refuted claim.

## 🔴 A NON-DETERMINISTIC TEST IS INSIDE THIS SUBSYSTEM

`outbox_happy_path_completes_with_n_terminals_in_order` failed **once in 8 runs**
of `cargo test -p aion-rs --all-targets --no-fail-fast` on 2026-07-31.

⚠️ **Two things this does NOT support, both of which I wrote and both struck on
2026-08-01:**

1. *"the 1021, the 1022 and the 58-targets-0-unpaired all carry an unknown error
   rate."* Wrong in direction. A spurious FAILURE is a false RED. Those greens
   were observations that ran and held. Reds on this command need a re-run before
   they are believed; the greens stand. (The green-tainting structure would be a
   wait that passes without observing — checked by reading, and it does not exist
   here; see the READ FIRST section for the byte-level evidence.)
2. *"1 in 8" as a rate.* One event in eight runs has a 95% interval of roughly
   0.3%–53%. Say **"one failure in 8 runs; non-zero, unquantified"** — a figure
   like "1 in 8" gets requoted as though it were measured.

What survives both corrections, and is the whole point: this is the
third `outbox_e2e` member joining **#85**'s two — one subsystem, three tests, one
suspected shared-connection defect in the libSQL store. #85 already states the
stakes: *if it is a shared-connection bug it is a PRODUCTION defect, and the
flakes are the only thing currently reporting it.* **This is now the highest-value
open item on my board.**

## (superseded) #121 as first reported

`engine/startup.rs::repopulate_active_workflows` has two adjacent guards. Both
look masked by `list_active`'s `== Running` filter. **They have opposite
coverage, measured:**

```
if false && projected_status.is_terminal()          -> EXIT 0,   nothing notices
if false && projected_status == WorkflowStatus::Paused -> EXIT 101, one test fails
                                    (outbox_happy_path_completes_with_n_terminals_in_order)
```

The terminal guard — the second defence behind Waffles' requirement (5) — can be
deleted today and the entire `aion-rs` suite stays green.

**And the Paused result refuted my own explanation.** I had written that
`list_active` masks the region so nothing *can* reach it. That predicted the same
zero coverage for Paused. Wrong. The region IS reachable from the existing suite,
so the terminal guard is **reachable-but-untested**, not
unreachable-by-construction. Masking was an excuse, and it would have converted a
real hole into an inevitability that nobody re-measures.

**Next step, cheapest first:** trace what reaches the Paused guard (start from
`outbox_happy_path_completes_with_n_terminals_in_order`) and see whether the same
path can carry a terminal run — that would avoid the adversarial-`EventStore`
boilerplate entirely.

## Block discipline — two corrections from Athena, both adopted

- **A block closes at BOX IDLE, not when I feel done.** I closed Block E at
  "15:05, delta ≈0" while two more commits landed under her GO through 15:22
  (real delta +1.39). Her window is GO → idle and that is the one that counts.
  Third sub-window error; the first that misdescribed what she was answerable
  for rather than merely what I banked.
- **Declare the mutation class.** For ~11 min the tree carried a deliberately
  disabled safety guard while she reasoned about a tree that did not exist. One
  line in the declaration: *"this block mutates source to probe coverage and
  reverts."*

## The unifying statement (Athena's, and it covers all of tonight)

**A channel with no delivery confirmation reports success and absence
identically.** Her sampler could only say "armed". Waffles' ruling could only say
"issued" (and died between compactions — *a ruling not in the builder's hands
isn't issued*). The battery comment could only say "read the target list". My
`#[cfg]` arms could only say "this build has the feature". Each was
indistinguishable from working until someone measured the **artifact** instead of
reading the **report**.

## Unattributed working-tree entries

`.aion-jjuabk-snapshot-20260731/` and `.aion-accepted-binary-20260731/` were both
already untracked at 14:15:44Z, my first `git status` after resuming. Not
knowingly created by me, not touched, **not deletable on my say-so** — a
pre-compaction version of me cannot be ruled out. Treat as unattributed.

## 🔴 RULING PINNED — #122 builds shape (1), the connection pool

**Waffles, 2026-08-01, via Cally Ray. Verbatim, because a ruling not in the
builder's hands has not been issued — that failure already cost one ruling
between compactions this week.** Full context:
`docs/design/aion-authoring/LIBSQL-SHARED-CONNECTION-FINDING.md`.

> "the only shape that dissolves the hazard class instead of guarding it — a
> transaction is connection-scoped BY CONSTRUCTION, it cannot leak into a
> stranger's statements because there are no strangers on the line, and changes64
> becomes per-operation the same way. The lock disappears rather than grows:
> correctness lives in the structure, not in 41 methods remembering to take a
> mutex."

**Shape (2) — one lock over every statement — is NOT a step toward this and must
not be built as one.** Pre-declared contingency, trigger named now: it activates
only if a live failure *attributed to this mechanism* arrives before the pool
lands. *"A decision inherited, not a dilemma."* Shape (3) rejected.

**Three constraints ride it:**
- **(a)** the pool is justified by the PROVEN MECHANISM, never by #85's flakes.
  If the flakes vanish post-pool that is recorded **consistent, not
  load-bearing**, and the failing test's interleave **graduates to a deliberate
  pin test** rather than dying as a flake.
- **(b)** writer contention is **explicit and loud** — WAL still admits one
  writer; busy handling gets a stated timeout and a NAMED failure mode, never a
  silent retry-forever.
- **(c)** pool parameters declared **with rationale and units**. No assumed
  defaults; sizing is a stated choice, discussed if it touches deployment
  targets. (The house NO ARBITRARY LIMITS rule, applied to pool sizing.)

Building requires a block declaration to Athena and the GO, with the band
re-derived from a reclaimed tree.
