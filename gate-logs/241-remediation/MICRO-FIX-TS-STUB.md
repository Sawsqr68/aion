# #241 micro-fix — the TS worker stub the remediation missed

Follow-up on YOUR OWN completed #241 remediation (branch 241-final-message,
worktree .worktrees/241-final-message). The definitive battery you recorded
(gate-logs/241-remediation/final-battery.log) is 21/23; the two reds are ONE
defect: sdks/typescript/aion-worker/src/proto/worker.ts is stale against
crates/aion-proto-generated/proto (the #233-era CancelActivity proto change
moved under it; you regenerated the PYTHON stubs in 330030ea8 but not the TS
worker's). This is a bounded micro-fix on the same branch; commit-as-you-go;
never push.

## Required work

1. Regenerate the TS worker's protocol stub using the SDK's OWN declared
   regeneration mechanism (find it in sdks/typescript/aion-worker — its
   package.json scripts / scripts/generate-proto.mjs — do not hand-edit
   generated output).
2. Commit the regenerated worker.ts (and any sibling generated files the
   mechanism owns). The commit message quotes:
   - the pre-fix red: the ts-worker:proto-drift failure already recorded at
     gate-logs/241-remediation/final-battery.log (the drift-check section),
   - the post-fix green drift check run at your hand,
   - the post-fix ts-worker test run result (see honesty rule below).
   Same recipe as your python-stub commit 330030ea8.

## Honesty rule for ts-worker:test

Task #253 (already filed, NOT yours to fix) records that the gate runs
`bun test` where the SDK declares `vitest run`, and bun's vi shim lacks
advanceTimersByTimeAsync. After your regen, run the gate's own command. If it
stays red, verify at the output that every remaining failure is the #253
runner-shim class (or otherwise NOT the stub), and say exactly that in the
commit message — do not chase it green, and do not claim the gate green if it
is not. The drift check itself MUST be green.

## Floor

- Scope: generated stub files + the evidence commit only. No production or
  test source edits.
- Verification: the battery's ts-worker:proto-drift check green at your hand;
  ts-worker typecheck green; the test run reported honestly as above.
- Commits atop 844174e55 on 241-final-message. Never push.
