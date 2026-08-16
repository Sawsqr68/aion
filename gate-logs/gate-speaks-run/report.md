# BRIEF-GATE-SPEAKS delivery report

Run date: **August 16, 2026**  
Branch: `gate-speaks`  
Base tree: `ab6284f7e`  
Gate implementation commits: `ba8c0fd81`, `a45a26055`

## Date and scope ruling

The supplied dispatch addendum is labeled **August 17, 2026**, which is one day in the future relative to this run on **August 16, 2026**. I treated it as the controlling dispatch text because it explicitly says it supersedes the draft numbers and its stated base commit, branch, reference file, and corrected population all match this worktree.

Accordingly, this run covers this aion repository only. The `apps/manifold/gate-logs/read-path-leg1/gate.sh` specimen named in the original draft is in a different repository and is out of scope for this run.

No product code and no tests changed. The committed implementation changes are gate scripts only; this report is the only other committed file. Nothing was pushed.

## Census and controls

### Population instrument

Builder command:

```sh
git ls-files \
  | /usr/bin/grep -E '(^|/)gate[^/]*\.sh$' \
  | tee /tmp/gate-speaks-population.txt
/usr/bin/wc -l < /tmp/gate-speaks-population.txt
```

Scope: paths tracked by this worktree's repository index whose basename matches `gate*.sh`. Because `git ls-files` is index-backed, duplicate files below untracked `.worktrees/` directories are not admitted.

Counter: `/usr/bin/wc -l`.  
Result: **17 tracked gate scripts**.

This independently re-derived count agrees with the addendum's corrected **17**, not the draft's contaminated **32**.

### Pre-change absence claim and positive control

The corrected absence claim is not “zero gates write stderr.” At base commit `ab6284f7e`, **16 of 17 tracked gate scripts contained no literal `>&2` write at all**, while **1 of 17** already contained one and was the compliant reference implementation.

The base-tree instrument used `/usr/bin/grep`, did not use `2>/dev/null`, and read every tracked shell file directly from the base commit:

```sh
base=ab6284f7e
git ls-tree -r --name-only "$base" \
  | /usr/bin/grep -E '\.sh$' \
  > /tmp/gate-speaks-base-shells.txt

/usr/bin/awk -F/ '$NF ~ /^gate.*\.sh$/ {print}' \
  /tmp/gate-speaks-base-shells.txt \
  | LC_ALL=C sort \
  > /tmp/gate-speaks-base-population.txt

: > /tmp/gate-speaks-base-has-stderr.txt
: > /tmp/gate-speaks-base-no-stderr.txt
while IFS= read -r f; do
  if git show "$base:$f" | /usr/bin/grep -q -- '>&2'; then
    printf '%s\n' "$f" >> /tmp/gate-speaks-base-has-stderr.txt
  else
    printf '%s\n' "$f" >> /tmp/gate-speaks-base-no-stderr.txt
  fi
done < /tmp/gate-speaks-base-population.txt

/usr/bin/wc -l < /tmp/gate-speaks-base-population.txt
/usr/bin/wc -l < /tmp/gate-speaks-base-has-stderr.txt
/usr/bin/wc -l < /tmp/gate-speaks-base-no-stderr.txt
```

Counters and results:

- `/usr/bin/wc -l` counted **17** tracked gate scripts.
- `/usr/bin/wc -l` counted **1** gate script containing `>&2`.
- `/usr/bin/wc -l` counted **16** gate scripts containing no `>&2`.
- The sole pre-existing match was `gate-logs/console-live-fleet/gate.sh`.

Repository-wide positive control, using the same base-tree content and the same `/usr/bin/grep -q -- '>&2'` predicate without stderr suppression:

```sh
: > /tmp/gate-speaks-base-shell-has-stderr.txt
while IFS= read -r f; do
  if git show "$base:$f" | /usr/bin/grep -q -- '>&2'; then
    printf '%s\n' "$f" >> /tmp/gate-speaks-base-shell-has-stderr.txt
  fi
done < /tmp/gate-speaks-base-shells.txt

/usr/bin/wc -l < /tmp/gate-speaks-base-shells.txt
/usr/bin/wc -l < /tmp/gate-speaks-base-shell-has-stderr.txt
```

Counters and results:

- `/usr/bin/wc -l` counted **67 tracked shell files** in this repository.
- `/usr/bin/wc -l` counted **17 of those 67 shell files** containing `>&2`.

That **17-of-67** canary proves the search was live. It differs from the draft's old **254** because the dispatch addendum narrowed this run to one corrected repository population rather than the draft's cross-repository sweep.

### Post-change census

```sh
/usr/bin/xargs /usr/bin/grep -l -- '>&2' \
  < /tmp/gate-speaks-population.txt \
  | /usr/bin/wc -l

/usr/bin/xargs /usr/bin/grep -L -- '>&2' \
  < /tmp/gate-speaks-population.txt \
  | /usr/bin/wc -l
```

Counters and results:

- `/usr/bin/wc -l` counted **17 of 17** tracked gate scripts containing an stderr write after the change.
- `/usr/bin/wc -l` counted **0 of 17** tracked gate scripts lacking an stderr write after the change.
- `/usr/bin/grep` reported **0** remaining `LEG_CODES` matches across the population.

## Implementation

The changes preserve each script's existing stdout verdicts and gate decision while adding red-only stderr disclosure:

- The **9** code-only accumulator gates now retain `name=code` pairs in `LEG_RESULTS` and retain the exact logfile for each leg in `LEG_LOGS`.
- The **4** direct-variable multi-leg gates build named results only after their original success conjunction fails. Their composition retry behavior is unchanged, and `comp_log` tracks whether the final composition verdict came from the first run or the mandatory rerun.
- The **1** scalar workflow gate now retains every check's `name=code`, writes the complete gate output to `gate-workflow-dispatch.log` in the executing script's directory, and uses zsh `MULTIOS` to preserve fd 1 and fd 2 while duplicating both streams into that real log.
- The **2** single-command gates add a red-only stderr summary while preserving the wrapped command's exact nonzero exit code.
- The **1** reference implementation was already compliant and was left unchanged.
- Existing setup exit codes `90` through `94` remain unchanged; formerly silent setup exits now say which directory operation failed.

Every red summary names every final nonzero leg or check, its code, and its full-output path. Every green summary path exits without writing the new summary to stderr.

## Per-file disposition

Table-row counter: **17 total rows** = **16 fixed** + **1 already compliant** + **0 shape-does-not-apply** + **0 unexamined**.

| File | Disposition | Per-file adaptation |
|---|---|---|
| `gate-logs/244-fix-leg-2/gate.sh` | FIXED | Replaced code-only `LEG_CODES` with named `LEG_RESULTS`; added exact logfile map and all-red-leg stderr summary. |
| `gate-logs/244-fix-leg/gate.sh` | FIXED | Same named accumulator adaptation across its existing 11 legs; stdout and final any-red-to-1 rule retained. |
| `gate-logs/246-adoption/gate-rerun.sh` | FIXED | Named results retain rerun-specific filenames; HEAD lookup failure now speaks without changing exit 90. |
| `gate-logs/246-adoption/gate.sh` | FIXED | Named accumulator and exact per-leg logfile summary; original four-leg decision retained. |
| `gate-logs/247-leg2/gate.sh` | FIXED | Named accumulator and exact per-leg logfile summary; existing HEAD-stamped logs and decisions retained. |
| `gate-logs/247-transcript-spool/gate.sh` | FIXED | Named accumulator and exact per-leg logfile summary; existing eight-leg selection retained. |
| `gate-logs/console-live-fleet/gate.sh` | ALREADY COMPLIANT | Reference implementation already retained names, named every red leg/code/log on stderr, and kept green stderr silent; unchanged. |
| `gate-logs/declared-actions-fleet/gate.sh` | FIXED | Adapted direct final variables into named results after the original conjunction fails; composition summary follows the rerun logfile when rerun supplies the final red. |
| `gate-logs/declared-faces-pilot/gate.sh` | FIXED | Same direct-variable adaptation; mandatory composition rerun semantics and both evidence logs retained. |
| `gate-logs/delivery-rejudge/gate.sh` | FIXED | Single `rust` leg now speaks on stderr when red and still exits with the exact `rust_exit`. |
| `gate-logs/identity-first-fleet/gate.sh` | FIXED | Adapted five direct final variables; final composition code maps to the active first-run or rerun log. |
| `gate-logs/libsql-retirement/gate.sh` | FIXED | Named accumulator and exact logfile summary across all six existing legs. |
| `gate-logs/m210-attach-backpressure/gate.sh` | FIXED | Named accumulator and exact logfile summary across both existing legs. |
| `gate-logs/status-front/gate.sh` | FIXED | Named accumulator and exact logfile summary across all nine existing legs. |
| `gate-logs/traffic-classes-fleet/gate.sh` | FIXED | Adapted five direct final variables; final composition code maps to the active first-run or rerun log. |
| `gate-logs/workflow-dispatch/gate.sh` | FIXED | Scalar `fail` decision retained; `LEG_RESULTS` preserves check names/codes; zsh `MULTIOS` preserves stdout/stderr and writes a real combined full-output log beside the script. |
| `slice1-logs/gate.sh` | FIXED | Generic single-command wrapper now names the failed command/code/log on stderr and preserves the exact wrapped status. |

No file is `SHAPE-DOES-NOT-APPLY`: all non-reference shapes required an adaptation, even where they did not begin with an array. No file is `UNEXAMINED`.

## R4 — server-field ruling and terminal fd proof

### Server-recorded field proof

Per the dispatcher's addendum, the server path was already field-proven by run `21b9ea9a` on **August 16, 2026**, and was explicitly waived from repetition in this run.

Committed before text:

```text
the declared command `zsh` exited 1 with no standard error output
```

Committed after text supplied by the ruling:

```text
GATE RED — 3 of 6 legs failed: ...
```

This report does not represent that server query as newly executed; it records the dispatcher's controlling field proof exactly as instructed.

### Representative terminal proof with stdout and stderr separated

Representative modified script: `slice1-logs/gate.sh`.

Forced-red command before the change:

```sh
sh slice1-logs/gate.sh gate-speaks-before-red.log sh -c 'exit 7' \
  > /tmp/gate-speaks-before-red.out \
  2> /tmp/gate-speaks-before-red.err
```

Captured result before the change:

```text
exit code: 7
stdout:
sh -> exit=7  (slice1-logs/gate-speaks-before-red.log)
stderr:
<empty; 0 bytes>
```

Forced-red command after the change:

```sh
sh slice1-logs/gate.sh gate-speaks-after-red.log sh -c 'exit 7' \
  > /tmp/gate-speaks-after-red.out \
  2> /tmp/gate-speaks-after-red.err
```

Captured result after the change, verbatim:

```text
exit code: 7
stdout:
sh -> exit=7  (slice1-logs/gate-speaks-after-red.log)
stderr:
GATE RED — 1 of 1 legs failed:
  sh exited 7  (full output: slice1-logs/gate-speaks-after-red.log)
log directory: slice1-logs
```

The named failure is on fd 2, not fd 1, and identifies the failed leg, exact code, full-output log, and log directory.

## R5 — unchanged gate decisions

The same representative script was exercised green and red before and after the change.

| Case | Before exit | After exit | Before stderr | After stderr |
|---|---:|---:|---:|---:|
| Green command `true` | 0 | 0 | 0 bytes | 0 bytes |
| Forced-red command `sh -c 'exit 7'` | 7 | 7 | 0 bytes | Named failure summary |

Green stdout before:

```text
true -> exit=0  (slice1-logs/gate-speaks-before-green.log)
```

Green stdout after:

```text
true -> exit=0  (slice1-logs/gate-speaks-after-green.log)
```

The log filename differs only because separate before/after captures were used. The verdict format, green exit `0`, red exit `7`, leg selection, and wrapped-command semantics are unchanged. The only intentional behavioral delta is red-path disclosure on stderr.

Static review of the remaining shapes found the same decision preservation:

- Former `LEG_CODES` gates still exit `0` iff every recorded code is zero and otherwise normalize to `1`.
- Direct-variable gates still use their original success conjunction and mandatory composition retry.
- `workflow-dispatch` still returns only its original scalar `fail` values, `0` or `1`.
- `delivery-rejudge` still returns exact `rust_exit`.
- Setup failures retain their original `90`-through-`94` codes.

## Verification

### Syntax

```sh
parse_count=0
while IFS= read -r f; do
  zsh -n "$f"
  parse_count=$((parse_count + 1))
done < /tmp/gate-speaks-population.txt
printf 'parse_ok_count=%s\n' "$parse_count"
```

Counter: `parse_count`, incremented once per successful `zsh -n`.  
Result: **17 of 17** gate scripts parsed successfully; the parse-error file contained **0 bytes**, counted by `wc -c`.

### Patch hygiene

`git diff --check` passed before each gate commit. The gate commits contain **16 changed gate scripts**, counted from `git diff --name-only`; the reference gate remained unchanged.

### Runtime shape checks

- A zsh runtime probe exercised the named associative logfile map with a hyphenated leg name and produced the expected stderr line with the exact logfile.
- A focused zsh `MULTIOS` probe preserved stdout, preserved stderr, duplicated both into one combined log, and preserved exit `7`.
- Independent review first identified two output-location defects: stale composition-log mapping after rerun in four gates, and no real workflow full-output log. Commit `a45a26055` corrected both.
- Focused independent re-review returned **READY; no remaining blocker** and confirmed all five follow-up scripts pass `zsh -n`.

## Scope, refusal surface, and timing

- Refusal surface: **n/a**. This adds disclosure and refuses nothing new.
- Timing sensitivity: **not timing-sensitive**.
- Product code changes: **0 files**, counted by the committed path list.
- Test changes: **0 files**, counted by the committed path list.
- Gate decision changes: **0 intended and none found** by the representative before/after proof plus independent static review.
- Pushes: **0**.
- Live estate ports touched: **0**.
- `.manifold*` paths touched: **0**.
- New dotfolders: **0**.
- Daemons started: **0**.
- `CARGO_TARGET_DIR` overrides: **0**.

Pre-existing untracked gate-run evidence under `gate-logs/gate-speaks-run/` was not added except for this required `report.md`. The temporary representative logs created by this run were not committed.

## Final disposition

**READY.** All **17** tracked gate scripts are dispositioned; **16** were fixed and **1** was already compliant. All **17** parse. The representative red gate keeps exit `7`, names `sh=7` on stderr, and points to its full output. The representative green gate remains exit `0` with silent stderr. No file was left unexamined.
