# Dean-Whiting-laptop Mac worker scoping — steward's docket (2026-08-18, Vesper)

Commissioned by Tom's expansion order (item THREE). **Standing constraint: NOBODY touches that box until this scoping is agreed — a resident team is on it.** This document is the ask; nothing has been executed.

## Why a Mac worker at all
The estate's gate coverage is Mac-shaped in one irreplaceable way: the **pinned gleam/OTP toolchain leg** (Mac OTP 29/erts 17.0.3/gleam 1.17.0 is the mix whose stdlib beams beamr accepts — the #64 delta). A second Mac gives: (a) darwin-native gate legs (aion-darwin-acl and path-safety surfaces only build/test honestly on macOS), (b) battery offload so my Mac's ~25-minute batteries stop serializing the landing chain, (c) a pinned-toolchain gleam venue independent of the box's poisoned mix.

## Exact auth list for Tom's docket (nothing else)
1. **claude** — Claude Code login on Dean's box (interactive; Tom or Dean runs `claude` once and authenticates). Needed for any counsel/dev seat; NOT needed for script-only gates seats.
2. **norn** — `--account default` auth (interactive first run). Needed only for dev seats.
3. **gh** — GitHub CLI auth with read+push to ablative-io/aion. Needed for clone/push in fleet runs; a fine-grained PAT scoped to the repo is enough.
4. **rust toolchain** — rustup stable (non-interactive, no secret).
5. **bun** — only if counsel seats are wanted there (`bunx @agentclientprotocol/claude-agent-acp`).

Minimum viable (gates-only seat): items 3+4. Full fleet parity: all five.

## Mechanics (from tonight's scars, so they're not relearned)
- macOS has NO setsid binary: detached launches use the python double-fork recipe (os.fork → os.setsid → fork → execv) or harness-tracked tasks; verify every launch by first artifact, never by pid.
- PATH pinned inside every script; worker run-json law applies (exit-zero-always, inner code as data).
- One mutating lane per tree; worktree isolation for concurrent legs.

## Proposed seats, in order
(1) gates/battery offload worker (scripts only, auth items 3+4) → (2) pinned-toolchain gleam leg venue → (3) dev/counsel seats only if Tom grants 1+2+5 and a measured solo flight lands clean. Each step is its own go.
