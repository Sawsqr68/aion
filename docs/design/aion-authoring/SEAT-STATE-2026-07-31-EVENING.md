# Seat state — 2026-07-31, evening block

Written to survive a compaction. `origin/main` = **`7c9a2329`**, everything
pushed, nothing in flight, box idle.

## Landed this session

| commit | what |
|---|---|
| `07ff0ec0` | #119 — a vacuous green announces its own vacuity; **the announcement was itself vacuous twice** |
| `5b9d5d39` | #119 docs — landing hash + #76 cross-reference |
| `7c9a2329` | #117 (b) — a run that can never become resident can now SAY so |

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
