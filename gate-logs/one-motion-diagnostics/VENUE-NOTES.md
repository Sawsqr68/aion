# Venue — the one-motion boot-diagnostics fix

- Worktree: `.worktrees/100-budget`, branch `fix/one-motion-boot-diagnostics`
- **FROZEN HEAD**: 96737f44becb72fa362dceaf14a72aae204a3db5
- **Parent**: ab9915f5b618b3ba8b44915076562a3739ee35b5 (= origin/main at arming, the #109 merge)
- Box: 10 cores, load at arming: 5.26 5.37 7.24
- Free disk at arming: 33 GiB on /System/Volumes/Data
- Tom's live server (pid 71475) is up and holds 8080/50051/50061,
  exactly as it was during the #100 battery. Same venue, deliberately.

## Why the FULL battery for a leaf test file

The changed file is a build-graph leaf — `git grep one_motion_live_e2e` finds
no other reference, so nothing else compiles it — and a scoped gate would have
been defensible. The full battery runs anyway for a reason beyond standards:
it re-runs `rust:test-all-features`, the exact leg that produced #100's one
unexplained red, **now instrumented**. If that boot failure recurs here it will
carry the server's own account of it, converting an unexplained red into a
diagnosed one. That chance is worth the box time.
