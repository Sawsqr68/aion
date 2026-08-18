# Capture leg report

Branch: `leg/capture`

## What changed and why

### GAP 1 — session transcripts

- Added executable `workflows/fleet-dev/bin/fleet-sessions.sh` with the two briefed literal modes, `primary` and `one`.
- Confined writes to the named run's primary repo or shaped leg worktree, guarded session ids and labels, searched the pinned norn then Claude stores with test-only environment overrides, committed either the transcript or a typed absence, verified every claimed artifact at `HEAD`, and made repeat capture report `already-committed`.
- Added the `sessions` and `session` actions to both AWL documents.
- Wired per-builder capture onto each leg branch, fix-round capture inside the loop, and primary capture between judgement and push.
- Added `sessions_exit` and `sessions_note` to `FleetDevReport`, so a capture refusal remains typed report data rather than invalidating already judged work.

### GAP 2 — green gate tails

- Updated `workflows/fleet-dev/bin/fleet-gates.sh` so the round count includes red and green history.
- Added the green arm that commits the routed last 120 log lines as `.fleet/gates/green-<n>.md` and verifies the file at `HEAD`.
- Left the existing red arm and `RED-LATEST.md` behavior unchanged.

### GAP 3 — leg branch refs

- Updated `workflows/fleet-dev/bin/fleet-fanin.sh` to read each `leg/<id>` head before merging, read the merge commit after merging, and commit the rows to `.fleet/legs-merged.md` after all merges land.
- Added in-script checks that the record is at `HEAD`, every requested leg has a row, and every recorded SHA resolves to a commit.
- Left the conflict path unchanged.

### Behavioral gate and documentation

- Added executable `workflows/fleet-dev/selfcheck/capture-selfcheck.sh`, a confined POSIX-shell scratch test with ten assertions covering all three gaps.
- Assertion 10 requires exactly the four expected SHAs for the two-leg fixture as well as checking that they resolve; this prevents a missing record from passing vacuously.
- Updated `workflows/fleet-dev/README.md` to describe `capture_sessions` and the three artifact families a landed run carries.

## Files changed

- `workflows/fleet-dev/bin/fleet-sessions.sh` — new transcript/absence capture and commit operation.
- `workflows/fleet-dev/bin/fleet-gates.sh` — green-tail capture and two-color round numbering.
- `workflows/fleet-dev/bin/fleet-fanin.sh` — committed, verified leg branch-ref record.
- `workflows/fleet-dev/fleet_ops_worker.awl` — two bodied session actions and accurate fanin/gate comments.
- `workflows/fleet-dev/fleet_dev.awl` — two action declarations, three capture call sites, the seventeenth step, and report fields.
- `workflows/fleet-dev/selfcheck/capture-selfcheck.sh` — new behavioral acceptance check.
- `workflows/fleet-dev/README.md` — updated workflow and landed-artifact documentation.
- `.fleet/reports/capture.md` — this implementation, command, acceptance, mutation, and scope report.

I did not edit `.fleet-hardening-gate.sh`, `.fleet/plan.md`, or `workflows/fleet-dev/bin/fleet-lib.sh`.

## Implementation and diagnostic command log

The following commands were run before the definitive acceptance and mutation runs below.

1. `git status --short && git branch --show-current && ls -l workflows/fleet-dev/bin/*.sh .fleet-hardening-gate.sh`
   - Exit 0. Starting tree was clean on `leg/capture`; the nine existing scripts and gate were executable.
2. `chmod +x workflows/fleet-dev/bin/fleet-sessions.sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh && git diff --check && /home/aion/.cargo/bin/aion awl check workflows/fleet-dev/fleet_dev.awl && /home/aion/.cargo/bin/aion awl check workflows/fleet-dev/fleet_ops_worker.awl && for s in workflows/fleet-dev/bin/*.sh; do sh -n "$s" || exit 1; done && sh -n workflows/fleet-dev/selfcheck/capture-selfcheck.sh`
   - Exit 0. Output:
     ```text
     ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
     ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
     ```
3. `sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh`
   - First run exited 1: assertions 1–8 and 10 passed; assertion 9 failed because the fixture sampled `leg/alpha` before the transcript capture commit. I moved the expected-head sample to immediately before fanin, as the brief requires.
4. `sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh`
   - Exit 0 after the correction; all ten assertions passed.
5. `git diff --check && git status --short && git diff --stat && git diff -- workflows/fleet-dev/bin/fleet-sessions.sh workflows/fleet-dev/bin/fleet-gates.sh workflows/fleet-dev/bin/fleet-fanin.sh workflows/fleet-dev/fleet_ops_worker.awl workflows/fleet-dev/fleet_dev.awl workflows/fleet-dev/selfcheck/capture-selfcheck.sh workflows/fleet-dev/README.md`
   - Exit 0. No whitespace errors; only the briefed workflow directory files were changed at that point.

## Definitive acceptance commands and verbatim results

### 1. Workflow document

Command:

```sh
/home/aion/.cargo/bin/aion awl check workflows/fleet-dev/fleet_dev.awl
```

Exit: 0

```text
ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
```

### 2. Worker document

Command:

```sh
/home/aion/.cargo/bin/aion awl check workflows/fleet-dev/fleet_ops_worker.awl
```

Exit: 0

```text
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
```

### 3. Every operation script parses

Command:

```sh
for s in workflows/fleet-dev/bin/*.sh; do sh -n "$s" || echo "PARSE FAIL $s"; done
```

Exit: 0

```text
(no output)
```

### 4. Standalone behavioral self-check

Command:

```sh
sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
```

Exit: 0

```text
ok: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
ok: 3 repeated session capture is idempotent
ok: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
ok: 7 green gate commits its numbered non-empty routed tail
ok: 8 fanin commits the leg branch-ref record
ok: 9 leg-ref rows preserve every pre-merge branch head
ok: 10 every recorded leg and merge sha resolves to a commit
```

### 5. Full fleet-hardening gate

Command:

```sh
sh .fleet-hardening-gate.sh
```

Exit: 0

```text
gate: aion awl check workflows/fleet-dev/fleet_dev.awl
ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
gate: aion awl check workflows/fleet-dev/fleet_ops_worker.awl
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
gate: sh -n workflows/fleet-dev/bin/fleet-clone.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanin.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanout.sh
gate: sh -n workflows/fleet-dev/bin/fleet-gates.sh
gate: sh -n workflows/fleet-dev/bin/fleet-lib.sh
gate: sh -n workflows/fleet-dev/bin/fleet-plan-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-push.sh
gate: sh -n workflows/fleet-dev/bin/fleet-report-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-sessions.sh
gate: sh -n workflows/fleet-dev/bin/fleet-teardown.sh
gate: parsed 10 operation scripts
gate: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
ok: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
ok: 3 repeated session capture is idempotent
ok: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
ok: 7 green gate commits its numbered non-empty routed tail
ok: 8 fanin commits the leg branch-ref record
ok: 9 leg-ref rows preserve every pre-merge branch head
ok: 10 every recorded leg and merge sha resolves to a commit
fleet-hardening gate: GREEN
```

## Mandatory mutation checks

Each mutation was applied only to the working tree, the gate was run, the original implementation was restored from a temporary copy, and the gate was run green again. `MUTATION_GATE_EXIT=1` below is the measured exit of the deliberately broken gate; the wrapper itself exited zero only so restoration could complete and its result could be recorded.

### GAP 1 mutation — disable transcript copy

Mutation command:

```sh
cp workflows/fleet-dev/bin/fleet-sessions.sh /tmp/fleet-sessions.sh.capture-backup && python3 - <<'PY'
from pathlib import Path
p = Path('workflows/fleet-dev/bin/fleet-sessions.sh')
s = p.read_text()
old = '        cp "$src" "$tree/$rel" || op_guard_fail "cannot copy $src to $tree/$rel"\n'
assert old in s
p.write_text(s.replace(old, '        : "MUTATION: transcript copy disabled"\n', 1))
PY
sh .fleet-hardening-gate.sh
rc=$?
cp /tmp/fleet-sessions.sh.capture-backup workflows/fleet-dev/bin/fleet-sessions.sh
rm /tmp/fleet-sessions.sh.capture-backup
printf 'MUTATION_GATE_EXIT=%s\n' "$rc"
exit 0
```

Exit: wrapper 0; mutated gate 1.

Stdout:

```text
gate: aion awl check workflows/fleet-dev/fleet_dev.awl
ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
gate: aion awl check workflows/fleet-dev/fleet_ops_worker.awl
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
gate: sh -n workflows/fleet-dev/bin/fleet-clone.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanin.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanout.sh
gate: sh -n workflows/fleet-dev/bin/fleet-gates.sh
gate: sh -n workflows/fleet-dev/bin/fleet-lib.sh
gate: sh -n workflows/fleet-dev/bin/fleet-plan-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-push.sh
gate: sh -n workflows/fleet-dev/bin/fleet-report-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-sessions.sh
gate: sh -n workflows/fleet-dev/bin/fleet-teardown.sh
gate: parsed 10 operation scripts
gate: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
FAIL: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
FAIL: 3 repeated session capture is idempotent
FAIL: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
ok: 7 green gate commits its numbered non-empty routed tail
ok: 8 fanin commits the leg branch-ref record
ok: 9 leg-ref rows preserve every pre-merge branch head
ok: 10 every recorded leg and merge sha resolves to a commit
MUTATION_GATE_EXIT=1
```

Stderr:

```text
fatal: path '.fleet/sessions/ABSENT-plan_legs.md' does not exist in 'HEAD'
fatal: path '.fleet/sessions/ABSENT-judgement.md' does not exist in 'HEAD'
GATE FAILED: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
fleet-hardening gate: RED
```

After restore, command `sh .fleet-hardening-gate.sh` exited 0 with:

```text
gate: aion awl check workflows/fleet-dev/fleet_dev.awl
ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
gate: aion awl check workflows/fleet-dev/fleet_ops_worker.awl
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
gate: sh -n workflows/fleet-dev/bin/fleet-clone.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanin.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanout.sh
gate: sh -n workflows/fleet-dev/bin/fleet-gates.sh
gate: sh -n workflows/fleet-dev/bin/fleet-lib.sh
gate: sh -n workflows/fleet-dev/bin/fleet-plan-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-push.sh
gate: sh -n workflows/fleet-dev/bin/fleet-report-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-sessions.sh
gate: sh -n workflows/fleet-dev/bin/fleet-teardown.sh
gate: parsed 10 operation scripts
gate: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
ok: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
ok: 3 repeated session capture is idempotent
ok: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
ok: 7 green gate commits its numbered non-empty routed tail
ok: 8 fanin commits the leg branch-ref record
ok: 9 leg-ref rows preserve every pre-merge branch head
ok: 10 every recorded leg and merge sha resolves to a commit
fleet-hardening gate: GREEN
```

### GAP 2 mutation — delete the green-tail arm

Mutation command:

```sh
cp workflows/fleet-dev/bin/fleet-gates.sh /tmp/fleet-gates.sh.capture-backup && python3 - <<'PY'
from pathlib import Path
p = Path('workflows/fleet-dev/bin/fleet-gates.sh')
s = p.read_text()
old = '''else
    mkdir -p .fleet/gates || op_guard_fail "cannot create .fleet/gates"
    tail -n 120 "$log" > ".fleet/gates/green-$n.md" || op_guard_fail "cannot write the green tail into the tree"
    git add .fleet/gates && git commit -q -m "gates: green round $n — tail verbatim" || op_guard_fail "cannot commit the green tail"
    git cat-file -e "HEAD:.fleet/gates/green-$n.md" 2>/dev/null || op_guard_fail "green-$n.md is not at HEAD after commit"
'''
assert old in s
p.write_text(s.replace(old, '', 1))
PY
sh .fleet-hardening-gate.sh
rc=$?
cp /tmp/fleet-gates.sh.capture-backup workflows/fleet-dev/bin/fleet-gates.sh
rm /tmp/fleet-gates.sh.capture-backup
printf 'MUTATION_GATE_EXIT=%s\n' "$rc"
exit 0
```

Exit: wrapper 0; mutated gate 1.

Stdout:

```text
gate: aion awl check workflows/fleet-dev/fleet_dev.awl
ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
gate: aion awl check workflows/fleet-dev/fleet_ops_worker.awl
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
gate: sh -n workflows/fleet-dev/bin/fleet-clone.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanin.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanout.sh
gate: sh -n workflows/fleet-dev/bin/fleet-gates.sh
gate: sh -n workflows/fleet-dev/bin/fleet-lib.sh
gate: sh -n workflows/fleet-dev/bin/fleet-plan-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-push.sh
gate: sh -n workflows/fleet-dev/bin/fleet-report-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-sessions.sh
gate: sh -n workflows/fleet-dev/bin/fleet-teardown.sh
gate: parsed 10 operation scripts
gate: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
ok: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
ok: 3 repeated session capture is idempotent
ok: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
FAIL: 7 green gate commits its numbered non-empty routed tail
ok: 8 fanin commits the leg branch-ref record
ok: 9 leg-ref rows preserve every pre-merge branch head
ok: 10 every recorded leg and merge sha resolves to a commit
MUTATION_GATE_EXIT=1
```

Stderr:

```text
GATE FAILED: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
fleet-hardening gate: RED
```

After restore, `sh .fleet-hardening-gate.sh` exited 0 and printed the same full green output recorded in the definitive acceptance section.

### GAP 3 mutation — delete the leg-ref record write

Mutation command:

```sh
cp workflows/fleet-dev/bin/fleet-fanin.sh /tmp/fleet-fanin.sh.capture-backup && python3 - <<'PY'
from pathlib import Path
p = Path('workflows/fleet-dev/bin/fleet-fanin.sh')
s = p.read_text()
start = s.index('mkdir -p .fleet || op_guard_fail "cannot create .fleet"\n')
end_marker = '} > .fleet/legs-merged.md || op_guard_fail "cannot write .fleet/legs-merged.md"\n'
end = s.index(end_marker, start) + len(end_marker)
p.write_text(s[:start] + ': "MUTATION: leg-ref record write disabled"\n' + s[end:])
PY
sh .fleet-hardening-gate.sh
rc=$?
cp /tmp/fleet-fanin.sh.capture-backup workflows/fleet-dev/bin/fleet-fanin.sh
rm /tmp/fleet-fanin.sh.capture-backup
printf 'MUTATION_GATE_EXIT=%s\n' "$rc"
exit 0
```

Exit: wrapper 0; mutated gate 1.

Stdout:

```text
gate: aion awl check workflows/fleet-dev/fleet_dev.awl
ok: workflows/fleet-dev/fleet_dev.awl (17 steps)
gate: aion awl check workflows/fleet-dev/fleet_ops_worker.awl
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
gate: sh -n workflows/fleet-dev/bin/fleet-clone.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanin.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanout.sh
gate: sh -n workflows/fleet-dev/bin/fleet-gates.sh
gate: sh -n workflows/fleet-dev/bin/fleet-lib.sh
gate: sh -n workflows/fleet-dev/bin/fleet-plan-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-push.sh
gate: sh -n workflows/fleet-dev/bin/fleet-report-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-sessions.sh
gate: sh -n workflows/fleet-dev/bin/fleet-teardown.sh
gate: parsed 10 operation scripts
gate: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
ok: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
ok: 3 repeated session capture is idempotent
ok: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
ok: 7 green gate commits its numbered non-empty routed tail
FAIL: 8 fanin commits the leg branch-ref record
FAIL: 9 leg-ref rows preserve every pre-merge branch head
FAIL: 10 every recorded leg and merge sha resolves to a commit
MUTATION_GATE_EXIT=1
```

Stderr:

```text
fatal: pathspec '.fleet/legs-merged.md' did not match any files
GATE FAILED: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
fleet-hardening gate: RED
```

After restore, `sh .fleet-hardening-gate.sh` exited 0 and printed the same full green output recorded in the definitive acceptance section.

### AWL mutation — judgement routes to a nonexistent step

Mutation command:

```sh
cp workflows/fleet-dev/fleet_dev.awl /tmp/fleet_dev.awl.capture-backup && python3 - <<'PY'
from pathlib import Path
p = Path('workflows/fleet-dev/fleet_dev.awl')
s = p.read_text()
old = '  outcome clear: when judged.final_message == judge_pass, route capture_sessions\n'
assert old in s
p.write_text(s.replace(old, '  outcome clear: when judged.final_message == judge_pass, route capture_sessions_missing\n', 1))
PY
sh .fleet-hardening-gate.sh
rc=$?
cp /tmp/fleet_dev.awl.capture-backup workflows/fleet-dev/fleet_dev.awl
rm /tmp/fleet_dev.awl.capture-backup
printf 'MUTATION_GATE_EXIT=%s\n' "$rc"
exit 0
```

Exit: wrapper 0; mutated gate 1.

Stdout:

```text
gate: aion awl check workflows/fleet-dev/fleet_dev.awl
gate: aion awl check workflows/fleet-dev/fleet_ops_worker.awl
ok: workflows/fleet-dev/fleet_ops_worker.awl (worker `fleet_ops`, 10 actions)
gate: sh -n workflows/fleet-dev/bin/fleet-clone.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanin.sh
gate: sh -n workflows/fleet-dev/bin/fleet-fanout.sh
gate: sh -n workflows/fleet-dev/bin/fleet-gates.sh
gate: sh -n workflows/fleet-dev/bin/fleet-lib.sh
gate: sh -n workflows/fleet-dev/bin/fleet-plan-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-push.sh
gate: sh -n workflows/fleet-dev/bin/fleet-report-check.sh
gate: sh -n workflows/fleet-dev/bin/fleet-sessions.sh
gate: sh -n workflows/fleet-dev/bin/fleet-teardown.sh
gate: parsed 10 operation scripts
gate: sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh
ok: 1 session one captures and commits on the leg branch
ok: 2 path-limited capture leaves unrelated staged content uncommitted
ok: 3 repeated session capture is idempotent
ok: 4 primary capture commits present transcripts and typed absences naming both roots
ok: 5 tree and session-id confinement guards refuse unsafe inputs
ok: 6 red gate commits numbered and stable findings
ok: 7 green gate commits its numbered non-empty routed tail
ok: 8 fanin commits the leg branch-ref record
ok: 9 leg-ref rows preserve every pre-merge branch head
ok: 10 every recorded leg and merge sha resolves to a commit
MUTATION_GATE_EXIT=1
```

Stderr:

```text
workflows/fleet-dev/fleet_dev.awl:386:65: error: no step or workflow outcome named `capture_sessions_missing` to route to
workflows/fleet-dev/fleet_dev.awl:391:6: error: step `capture_sessions` is unreachable: no route targets it and control never falls through to it
workflows/fleet-dev/fleet_dev.awl:393:30: error: `counsel_control` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:393:79: error: `control` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:394:19: error: `planned` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:394:54: error: `judged` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:398:6: error: step `push_result` is unreachable: no route targets it and control never falls through to it
workflows/fleet-dev/fleet_dev.awl:402:29: error: `judged` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:402:83: error: `cloned` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:406:6: error: step `clean_up` is unreachable: no route targets it and control never falls through to it
workflows/fleet-dev/fleet_dev.awl:408:20: error: `planned` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:408:43: error: `leg_ids` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:408:61: error: `reports` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:408:79: error: `judged` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:409:18: error: `cloned` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:410:46: error: `gate_result` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:411:20: error: `control` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
workflows/fleet-dev/fleet_dev.awl:411:57: error: `counsel_control` is bound on some path but not guaranteed on every path into this step — bindings are readable only where guaranteed
GATE FAILED: aion awl check workflows/fleet-dev/fleet_dev.awl
fleet-hardening gate: RED
```

After restore, `sh .fleet-hardening-gate.sh` exited 0 and printed the same full green output recorded in the definitive acceptance section.

## Mutation outcome summary

- GAP 1 disabled copy: gate RED; assertions 1, 3, and 4 failed; gate named `sh workflows/fleet-dev/selfcheck/capture-selfcheck.sh`.
- GAP 2 removed green arm: gate RED; assertion 7 failed; gate named the self-check.
- GAP 3 removed record write: gate RED; assertions 8, 9, and 10 failed; gate named the self-check.
- AWL missing route target: gate RED; `aion awl check workflows/fleet-dev/fleet_dev.awl` reported the nonexistent target and the gate named that command.
- Every mutation was restored. The definitive full gate after restoration was GREEN.

## Brief accuracy and out-of-scope observations

- The brief's prescribed AWL wiring checked exactly as stated: 17 workflow steps and 10 worker actions.
- The supplied session-script design worked as described; no contract-level divergence was needed.
- The self-check initially sampled the alpha head before the capture commit; that was my fixture error, not a brief error, and was corrected to sample immediately before fanin.
- I noticed pre-existing README trade-off text that refers to historical workflow/action timeout values even though the current AWL action declarations shown in this family do not carry those timeout clauses. It is outside the three commissioned capture gaps, so I did not revise that historical section.
- I did not deploy, push, merge, land, or alter the already committed gate entry.
