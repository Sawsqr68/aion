# 📍 SEAT RESUME — 2026-08-04 (U)

**Supersedes T. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the
MODEL, not my name.

## 🔴 LIVE: #139 legs A+B — A DONE ON DISK (UNCOMMITTED), B AGENT RUNNING

The build plan is **BUNDLE-139-NOTES.md §"Scoped build plan"** — read it.
Scout findings ratified there (declared-body output capped at
exit_code/stdout/stderr → provision SPLIT: prompt→document consts,
workspace→shell body; env=PATH-only → new `workspace_root` input; no
resource packaging today → prompt points into the clone).

**LEG A — DONE, seat-verified, NOT YET COMMITTED. On-disk changes:**
- `examples/assistant/awl/assistant.awl` + `assistant_spike.awl`: bodied
  `assistant_provision(workspace_root, run_id, repo_path) -> Provisioned
  {exit_code: Int}`; run body = one-line `sh -c '<script>' --
  $workspace_root $run_id $repo_path` (POSIX sh, rename-aside collision
  discipline, no apostrophes; proven under `env -i PATH=` through all 5
  paths); prompt ported to triple-quoted raw-string consts;
  when/otherwise diamond with convergent same-name bindings for the
  scratch/attached branch; workspace path composed in AWL
  `workspace_root + "/" + workflow.id + "/repo"`. Both check ok (5
  steps), fmt'd, shared declarations byte-identical (diff-verified).
- `examples/agent-dev/worker/src/descriptors_tests.rs`: MY fixes — both
  failing shape tests now DERIVE from the contract (refusal names every
  bodiless action and must NOT name bodied ones; plain side of split ==
  unmarked bodiless set, may be empty). Crate 76/0, clippy -D clean,
  fmt'd. aion-awl assistant_example 7/7 at my hand.
- `examples/assistant/DAVID.md` + `scripts/setup.sh` guidance updated
  (callers must now supply `workspace_root` — AWL inputs have no
  defaults).

**LEG B — agent still running** (adding `--norn-arg` + `--norn-unset-env`
to `aion worker agent`, promoting degraded-session advice into
aion-integration-norn, argv-parity tests). It already wrote its
AGENT-WORKERS.md doc section (visible on disk). When its notification
arrives: verify at my hand (run `cargo clippy -p aion-cli
-p aion-integration-norn --all-targets -- -D warnings` + `cargo test -p
aion-cli -p aion-integration-norn` + fmt), review the argv-parity test
expected argv, THEN commit legs A+B together + push + tell Tom. If the
agent died on a usage limit, resume it via SendMessage (worked before).

**DEPLOY IS NOT PART OF THE COMMIT.** New documents mint a new content
hash; live assistant stays 67128ff6 until a planned `aion route` cut —
NOT during Tom's demo hours (he said 04:25Z he's "working somebody
through some stuff"). Old version stays resident; rollback is route-back.

## Standing context (compressed)

- **Tom's orders sequence**: docs order DONE (4d5283f2) → docs
  completeness sweep DONE (a2dcb61d, all audits+writers landed, verified)
  → #139 NOW → then telemetry+#59 as one lane (his 04:00Z two-plane
  ruling captured in WORKER-TELEMETRY-DIRECTION.md; announced
  silence-default).
- **Worker-language answer given to Tom** (04:46Z): contract fully AWL;
  serving = declared bodies + shell + agent verb + scaffold; unbuilt =
  #30 (worker as document kind) + #27 (server-supervised); recommended
  #139 → #30+#27 together.
- **Athena (04:56Z)**: census caught leg B's builds in her liminal
  battery quiet window. Declaration SENT (identity, work, draw,
  duration); offered to hold my verification re-run until her battery
  done — proceed on her silence, be courteous about box contention.
- **Live processes**: server pid 48168 (0.11.0, ports 8080/50051/50061)
  NEVER touched; assistant worker pid 77969 (gpt-5.6-sol) — do NOT
  restart during demo hours; fresh worker binaries on disk await natural
  restart. Both fine.
- Repo state: origin/main = a2dcb61d + these uncommitted #139 changes.
  Untracked KEEP dirs: .aion-accepted-binary-20260731,
  .aion-jjuabk-snapshot-20260731, .aion-data-preswap-20260804.
- Leg A finding parked: spike's `assistant` action stays unmarked (agent
  marker asymmetry predates this change, out of scope).
  ASSISTANT-AWL-PORT.md §1 superseded by this change (not edited).
- Small debts: #136 .awl template, #137 MANIFEST stale, broken Linux
  python3 in ~/.local/bin (tell Tom sometime).

## Comms (IDs from meridian-comms-bridge-state.md memory ONLY)

Tom dm:c9255b2a-5731-4d17-8124-e3bfa2224186 (plain, short, answer-first)
· Waffles dm:896955e1-86dd-4d6a-9c25-f3f978b189a9 · Athena
dm:45cf420e-621d-467b-b96c-5bffb62c04f3 · Cally
dm:15a23351-009a-4e69-8d63-45602a805f1c.
