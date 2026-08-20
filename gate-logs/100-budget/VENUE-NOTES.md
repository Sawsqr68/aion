# Venue — aion#100 transport-loss budget wiring

- Worktree: `.worktrees/100-budget`, branch `fix/100-transport-budget-wiring`
- **FROZEN HEAD**: `b767a1ead58d0ef4c07b8d2cd679247179c1346e`
- **Parent**: `8e0cb3f3877d5fad02229877e7862c37868e3aaf` (= origin/main at arming, the #108 merge)
- Box: 10 cores, load at arming: 12.18 13.73 13.25
- Free disk at arming: 80 GiB on /System/Volumes/Data
  (⚠️ `df -g /` reads the SEALED SYSTEM VOLUME and reports a misleading figure)
- Node deps: `bun install --frozen-lockfile`, 1000 packages, both
  `node_modules` and `apps/aion-ops-console/node_modules` present.
- python3 selected by the gate lib: `/Users/tom/.local/bin/python3`

## Disk had to be reclaimed before arming

Free space was **1 GiB (100% full)** — firing there would have filled the disk
mid-battery and produced an **UNMEASURED red, not a verdict**.

Two reclaims, each gated at the moment of action:
1. `.worktrees/105-run-aware-seam/target` (**74G**) — #105 is merged
   (`d8110d649` confirmed an ancestor of origin/main, 0 unpushed), **all 30 pid
   files in that tree confirmed dead**, no battery running on the box, and the
   tree carried no uncommitted files. Only build artifact removed; source
   verified intact afterwards.
2. `.worktrees/base-attrib/target` (**4.6G**) — merged, its one pid dead, and
   its two uncommitted files (`build-base.log`, `build-base.pid`) verified
   surviving the removal.

**Merged is not idle** — the pid check is the one that matters, not the branch
status.
