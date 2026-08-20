# venue at arming — fix/gates-lib-posix-source

- box: Tom's Mac, 10 cores
- armed: 2026-08-20 21:15 +1000
- frozen HEAD: 2a7ed1eb2688ea112aa24e4b730afb6afe1f2f3f
- base: dfea2ec838e19993b493dc6e3545647194a581e9 (origin/main, merge of PR #104)
- disk at arming: 109 GiB free (reclaimed from #94's finished 78 GiB target)
- load at arming: 14.20 23.88 20.70
- **target/ is COLD in this worktree** — expect a full workspace build
- deps: `bun install --frozen-lockfile` run at arming, 1000 packages, rc 0.
  Both `node_modules` roots present, so the five console/ts-worker legs measure
  code rather than a missing toolchain.
- tenants: Tom's server (pid 67116) plus two `aion worker awl` processes
  (release-ops-mac 87647, gate-ops-mac 87917). None is a venue; all are
  permanent load on this box.

## Why it was armed at this moment, not earlier

Waffles was building in `aion-wt-106` verifying PR #106 (my own #94). Firing
into that would have contended their run — the exact contention that reddened
#94's first battery on the known #54 shard-actor flake. Idleness was confirmed
by basename-normalised `ps` **and** `pgrep -x` (two different resolution paths),
with the control keyed to a specific pid rather than to the name — the first
attempt used a pattern that was blind to path-exec'd processes and would have
declared the box idle while a build was running.
