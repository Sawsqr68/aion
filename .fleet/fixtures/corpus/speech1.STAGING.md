# SPEECH-1 rig staging checklist (execute when Hermes names the cut commit)

Kit contents: gate-entry.sh (BASE placeholder), input-template.json
(__CUT_COMMIT__, __RIG_COMMIT__, __CTL_TOKEN__ placeholders).

1. RECEIPT FIRST: Hermes's cut-commit message must carry the drift-gate
   receipt (scripts/publish_drift_gate.py exit 0 at post-publish main).
   No receipt → no staging; ask, don't infer.
2. Fresh clone of https://github.com/ablative-io/liminal.git (never the
   stack/liminal working copy — a worktree is a place, not an owner).
   `git checkout -b fleet/speech1-liminal-py-base <CUT_COMMIT>`.
3. Assemble .fleet/BRIEF.md = SPEECH-1 brief (manifold
   docs/briefs/SPEECH-1-python-participant-client.md @ 5cffd33 or newer —
   read from the manifold repo at staging time, never from memory) + a
   separator line + Hermes's RIG.md (ablative docs
   dispatch-rigs/speech-1-python-wire/RIG.md @ bad5f7a or newer).
4. .fleet/gate-entry.sh = kit gate-entry.sh with BASE=<CUT_COMMIT> filled;
   chmod +x. TEST THE GATE'S OWN CONTROLS on the rig tree before push:
   run it once — it must go RED with "deliverable
   sdks/liminal-py/pyproject.toml absent from the diff" (the rig-only
   tree's diff is NOT empty — the .fleet/ files are in it — so the
   deliverable positive control is the red that fires; Hermes F2).
   Record the output and compare against THAT string.
5. Mint token: speech1-ctl-$(uuidgen | cut -c1-8)-waffles-$(date -u +%Y%m%d)
   → .fleet/control-token.txt (token + one LF, nothing else).
6. Commit rig (message: "rig: speech1-liminal-py-flight1 — base <CUT_COMMIT>
   per Hermes's drift-gate receipt <receipt>"), push the branch.
7. HAND TO HERMES FOR REVIEW (author ≠ reviewer): branch name + rig commit
   sha. She checks staged bytes against RIG.md + repo laws. Same-hour.
8. On her green: fill input-template (__CUT_COMMIT__, __RIG_COMMIT__ = rig
   commit sha, __CTL_TOKEN__ = minted token), fire via mcp__aion__start_run:
   namespace default, workflow_type fleet_dev, task_queue dev,
   display_name fleet_dev/speech1-liminal-py-flight1.
   THEN: name the log/run to MU/TH/UR for a fresh terminal probe (her
   battery3 probe exited; directory detector ≠ engine watcher), bank the
   wf id + act baselines in the day file.
9. Three-arm re-check at fire time (box occupancy may have changed since
   23:42Z green): idle-enough, workers serving dev, disk ≥ 40G (throwaway
   spine build eats target-dir space; builder cleans before final commit).

Sequencing wall: DO NOT stage until Vesper's liminal-family publish lands
AND Hermes names the commit. Her delivery wave does NOT gate (her ruling 1,
23:44Z). Fire is mine; verdict is mine; landing re-gate at my desk.
