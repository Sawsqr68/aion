# Shell instrument traps

A family reference for defects in the *measuring* shell code — the scripts
that derive manifests, run gates, and write evidence — where the failure mode
is not a crash but a **plausible wrong value**. An instrument that dies is
caught by its exit code; an instrument that fails into a comfortable number is
caught only by a control. Every member of this family was found because a
control existed or a reviewer built one; none announced itself.

## Family 1 — zsh special-variable shadowing

zsh ties a set of *special parameters* to interpreter state (`$parameters`
lists 114 of them on this box, flagged `special`). Declaring a `local`,
`typeset`, or `readonly` with one of those names does not create an ordinary
variable — it shadows live interpreter state for the function's extent, and
the surrounding work keeps appearing to succeed.

**Shared symptom:** the script fails *silently into a plausible value* —
empty, null, or zero where a real reading belongs — while every gate around
it runs green.

### Enrolled members

| Name | Round paid | What happened |
| --- | --- | --- |
| `status` | #57 r1 | `status` is the read-only special aliased to `$?`. A local by that name shadows the exit-status readout inside the function, so status handling reads the shadow, not the command's result. |
| `path` | #57 r2 | `path` is the array special tied to `PATH`. `local path` **emptied PATH for the whole function** — every `command -v` then missed, and `tool_identity` degraded to nine nulls in the committed r2 manifest while the gates themselves ran fine. Renamed to `tool_location`. |

### The rule

**No `local`/`typeset`/`readonly` in an instrument script may take a special
parameter's name.** The generalized statement lives above `tool_identity()`
in `workflows/gates/gate-runner.zsh`; the enforcement is
`gate-logs/57-gates/sweep2.zsh` — a sweep that intersects every declared
name in a script against the live specials table, with a control on **both**
halves (the table must contain `path`/`status`/`argv`; the extractor must see
a known shadowing fixture). The controls are not decoration: the v1 sweep
extracted zero names and reported "no collisions" — the tool itself failed
into the comfortable answer (`gate-logs/57-gates/sweep2-run.log` is the
current clean run).

Checking a new instrument script:

```zsh
zsh gate-logs/57-gates/sweep2.zsh path/to/script.zsh
```

## Enrolment

A new trap joins this file when a lane pays for it: name the special (or the
new family), the round that paid, the plausible value it failed into, and the
control that would have caught it. A trap recorded only inside the file it
bit will not reach the next script — that is what this reference is for.

Related reading: `docs/operations/lane-gates.md` (the manifest/identity
derivation these instruments serve).
