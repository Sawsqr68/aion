# Venue — aion#105 battery, 2026-08-20

Recorded at arming, so the run names its own inputs rather than leaving them
to be reconstructed afterwards.

## Subject

- Worktree: `.worktrees/105-run-aware-seam`
- Branch: `fix/105-run-aware-resolve`
- **Frozen HEAD: `6b9cc01a20c81b1521c698f1865a8b20088cc3cb`**
- **Parent: `a510c72522348f9a52fa589a46bf4eb8b66b4f1f`** (= `origin/main` at arming,
  the PR #107 merge)
- Diff: two files under `crates/aion/src/runtime/` — the fix and its tests.
  No shell, no manifests, no lockfiles.

🔴 This attestation names a **(commit, parent)** pair. Land by MERGE. A rebase
mints new shas and voids it.

## Box

- 10 cores, load average 6.11 / 6.91 / 9.02 at arming
- Free space **78 GiB** at arming, on the data volume (`/System/Volumes/Data` —
  `df -g /` reads the sealed system volume and reports a misleading 16)
- `target/` carried into the run: 12 GiB warm from the pre-gate checks
- Deps installed `bun install --frozen-lockfile`, rc 0, 1000 packages; both
  `node_modules` and `apps/aion-ops-console/node_modules` present. A missing
  `node_modules` produced false reds in an earlier run on another branch.
- 24 legs declared in `gates.json`
- Tom's server (pid 67116) is live on this box throughout, as it was for the
  #83 and #94 runs.

### Disk was a blocker and was cleared deliberately

Free space was **18 GiB** before arming — 99% full. A cold workspace build here
has taken 63–73 GiB, so firing at 18 would have filled the disk mid-battery and
produced an **UNMEASURED red, not a verdict**.

Reclaimed exactly one directory: `.worktrees/gates-lib-posix/target` (63 GiB).
Gated before removal, not assumed safe — that branch's HEAD (`016346386`) was
confirmed an ancestor of `origin/main` (PR #107 merged), with nothing unpushed.
The directory is pure build artifact; the worktree's source tree is untouched,
including its dirty `workflows/gates/worker/Cargo.lock`.

Nothing else was reclaimed. The other worktree targets belong to lanes whose
state I have not measured, and Tom's data backups are never touched without his
word.

## What this gate will not say

It will not say the estate is green. `estate-guard` measures a list of
repositories, and its own output says it is silent about the repository being
landed in unless that repository is on the list.

## Run identity

Battery pid **28568**, fired detached (`nohup`) 2026-08-20 23:24 AEST, observed
exiting 2026-08-21 00:13:38. Watched by PID, never by a command-line pattern —
a `pgrep -f` on a pattern present in the watcher's own argv matches itself and
can never see the subject go.

⚠️ The battery was launched through `nohup … &`, so that wrapper's exit status
is the WRAPPER's, not a verdict. The verdict in `README.md` is derived from the
log's own leg lines and from `verify-declaration.sh`, never from an rc.
