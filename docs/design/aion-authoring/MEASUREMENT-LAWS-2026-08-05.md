# Two measurement laws, minted 2026-08-05

**For the estate measurement-defect catalogue.** Both were found the same day,
in aion, by the defects they name. Waffles adopted both and called the second
the better one — this document says why he was right about that.

Author: Vesper Lynd (`5849e0d8-4802-4869-8e0f-9f7fd187e198`).

---

## Law 1 — A treatment that can become its own control is a vacuous green

### The statement

Whenever a harness **constructs** the condition it is testing — extracting it,
copying it, generating it, patching it in, switching it on — that construction
can fail quietly. When it does, the run measures the **baseline** while
reporting that it measured the **treatment**. Nothing errors. The output looks
exactly like data.

**A harness that constructs its test condition must refuse to run unless that
condition verifiably landed.**

### How it was found

My own A/B harness extracted the configuration under test out of a script with
a `sed` pattern. The pattern silently matched nothing. The "treatment" arm ran
with an **empty** configuration — that is, it ran as a second control — and
produced a clean, plausible, entirely worthless result.

The thing to sit with: **the green was real.** The run completed, the numbers
were genuine measurements of something. They were measurements of the control,
twice, reported as a comparison.

### Why it is hard to catch by looking

Every instinct for spotting a bad result is tuned for results that look wrong.
This one looks right. There is no error, no empty output, no zero row, no
warning. The A arm and the B arm agree — and *agreement between arms is exactly
what a null result looks like*, so the harness's failure is indistinguishable
from its most publishable finding.

### The same shape elsewhere

An agent reports a passing suite, then reverts the change that made it pass.
The suite really did pass. Something other than the thing under test produced
the green. Same family, different actor.

### The guard, and it is cheap

Assert the condition landed. Exit non-zero if it did not. Mine now greps for
both configuration sections it was supposed to have extracted and refuses to
run if either is missing — four lines.

The guard belongs **inside the harness**, before the measurement, and it must
be a refusal rather than a warning. A warning in a log nobody reads is how the
defect survived in the first place.

### The question to ask of any green

> **What produced this?**

If one honest answer is *"the setup step no-oped"*, the green is worth nothing
until the harness is guarded. Ask it of every harness we own that builds its own
conditions — which is most of them.

---

## Law 2 — An environment that carries the thing under test cannot measure whether it is required

### The statement

A proof run on a machine that already satisfies a precondition **cannot observe
that the precondition exists**. The environment supplies the missing piece
silently, the run succeeds, and the success is attributed to the artifact
instead of to the box.

**Before proving a setup or bootstrap path, ask what the test environment
already carries that the target environment will not — then strip exactly that,
and re-run.**

### How it was found

`scripts/setup.sh` had been live-proven, repeatedly, over weeks. It could never
have worked on a fresh machine. It needed a configuration file it does not
write, and every single proof ran against a home directory that already had
one — because the box had been running Aion for months.

Every one of those proofs was honest. Every one of them was worthless as
evidence about a new user, and a new user is the only person the script exists
for.

*"It works on Tom's box"* has never been evidence of anything except Tom's box.

### Why this is the more dangerous of the two

Law 1 describes a harness lying to itself. That is fixable **from inside** — you
add an assertion and the harness stops lying.

Law 2 describes the **environment** lying to a harness that is behaving
perfectly. No assertion placed inside the measurement can detect a precondition
the box silently satisfies, because from inside the measurement there is nothing
to detect: the file is there, the tool is on PATH, the daemon is up. The
information needed to spot the defect does not exist within the run.

The only instrument is **subtraction** — deliberately removing what you suspect
the environment is contributing, and running again. That is a different kind of
act from adding a check. It requires you to first *imagine* the dependency,
which is precisely the thing a working system trains you not to do.

That is why it is the better law. It names a class of defect that no amount of
rigour inside the harness will ever surface.

### How to apply it

1. Write down what the target environment is: a fresh machine, a new tenant, a
   cold cache, a user who has never run this.
2. Enumerate what your box has that theirs does not — installed tools,
   credentials, config files, directories, environment variables, warm state,
   prior deploys, DNS entries, group membership.
3. **Strip exactly one** and re-run. Then the next.
4. Anything that survives all of them is proven. Anything that does not was
   never proven — it was hosted.

Where a full strip is impractical, say so in the record and name what stayed.
An unstripped precondition that is *written down* is a known gap; an unstripped
precondition nobody noticed is a false claim.

---

## What the two share

Both are one failure at two layers: **something outside the measurement supplied
the result the measurement claimed to produce.** In Law 1 it was the harness's
own broken setup step; in Law 2 it was the machine.

That is the generalisation worth carrying, because it gives one question that
covers both:

> **If the thing under test were absent, would this run still be green?**

If yes, the run measures something other than the thing under test. Law 1 is
that question asked of the harness; Law 2 is it asked of the environment.

## Where they already have teeth in aion

- The A/B harness asserts both configuration sections and exits non-zero if
  either is missing.
- `scripts/setup.sh` now **writes** the minimal `config.toml` it needs when the
  home has none — the fix that makes a stock server usable at all. That fix was
  found by applying Law 2, not by any test. No test could have found it, because
  every test ran on a box that already had the file.

## Catalogue neighbours

Sits beside [[base-rate-before-attribution]] (measure how often the outcome
holds *without* the cause first), [[assert-the-invariant-not-the-enumeration]]
(pair absence with survival; a baseline must be independent of what it
measures), [[a-global-gauge-cannot-attribute]] (the gauge measures the box, not
the actor), and [[withdrawal-proven-against-the-artifact]] (the source is a
claim, the served artifact is the fact).
