# 📍 SEAT RESUME — 2026-08-05 (W)

**Supersedes V. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the
MODEL, not my name.

## 🔴 LIVE BOX STATE (post-reboot, all verified at my hand)

The box REBOOTED ~21:44Z Aug 4 — old server 48168 and worker 77969 are
DEAD; every standing note about those pids is obsolete. Current:

- **Server pid 85467** — installed aion 0.11.0, started 23:03Z **from the
  REPO ROOT** (cwd matters!). Console :8080, gRPC :50051, liminal :50061.
- **🔴 THE WRONG-STORE TRAP (my own error tonight, do not repeat):**
  `~/.aion/config.toml` DELIBERATELY leaves `store.data_dir` unset so the
  resolver picks the legacy CWD-RELATIVE `./aion-data` — the real store is
  `<repo>/aion-data`. I first restarted the server from `~/.aion` → it
  opened `~/.aion/data` (a stale store), console showed disconnected/empty,
  Tom hit it live and escalated. Fix: kill mine, restart from repo root.
  **`aion server` on Tom's box MUST start with cwd = repo root.**
- **Worker pid 66189** — fresh binary (Aug 4 14:16),
  `--address 127.0.0.1:50061 --norn-account default`, registered 23:03:11.
- **End-to-end proof**: spike run `b6bd67bc` COMPLETED in 8 s (provision
  served → norn agent round answered → clean completion) at 23:04Z on the
  real store. assistant `6f43c809` route_active confirmed via `aion
  versions`. Tom told to hard-refresh; awaiting his confirmation.
- WITHDRAWING-DISPATCH-ELIGIBILITY warnings in server.log are TRANSIENT
  (base rate: 6 in the old proven-working log; restoration is silent).
  Not evidence of a broken worker.

## The #139 leg A lane — ✅ LANDED main `fe702945`, pushed 2026-08-05

**POST-LANDING UPDATE (same day, after the section below was written):** my
read completed (F1 guard, F5 comments, deletions, provision_body.rs — all
sound), both new test surfaces re-run green at my hand (15/15 + 5/5), the
ONE commit landed and pushed as `fe702945`, Tom was told LLM-STARTUP.md is
on GitHub, and Waffles got the landing note. Next-actions 1–5 below are ALL
DONE — do not repeat them. What remains is item 6: the live-cut plan.

Full history: BRIEF-139-LEG-A-REWORK.md (implementation brief),
BRIEF-139-LEG-A-FIXES.md (review punch list + my rulings), BUNDLE-139-NOTES
§2026-08-05 (open questions recorded). Waffles RATIFIED option (b) 21:57Z
with two conditions (loud failure; resolved-path-in-history) — both
implemented and review-verified.

State: build agent green → Opus review said DO-NOT-LAND-YET (6 musts, 11
minors) → fix agent executed ALL findings (F1-F13, F17, Record) → all
gates green at the fix agent's hand (workspace clippy -D warnings 0;
aion-server / aion-awl / example crate all ok incl. new
provision_body.rs 5/5 and the printable-ASCII property test; awl check+fmt
both docs clean; run-line sha256 `e377d3e8…` byte-identical both docs;
`bash -n setup.sh` ok).

**MY READ (Opus-work gates through my read) — partially done:**
- ✅ Verified at my hand pre-fix: workspace_root.rs core, declared_body
  seam, state/run/mod diffs, assistant.awl full diff.
- ✅ Verified at my hand post-fix: scripts/setup.sh F4 diff (banner-read →
  refuse unresolvable → export AION_WORKSPACE_ROOT before worker).
- ⬜ REMAINING: F1 placement-guard code in workspace_root.rs (now 760
  lines / 269 LOC), F5 comment diffs in both .awl docs, example-crate
  deletion sanity (handlers/ gone, kept resolve_workspace_root), the new
  tests/provision_body.rs.

**NEXT ACTIONS, IN ORDER:**
1. Finish my read (list above).
2. ONE commit: the whole lane + both briefs (already staged) +
   BUNDLE-139-NOTES + this resume doc + **LLM-STARTUP.md** (mine — new
   root doc Tom asked for, LLM-facing setup guide; fix agent flagged it as
   unattributed dirt: it is MINE, attributed here). Push.
3. Tell Tom LLM-STARTUP.md is on GitHub (promised ~23:00Z "within the
   hour").
4. Landing note to Waffles: ratification executed, review verdicts,
   commit hash. Route through him; Athena got pre-notes (her seat is on
   Tom-ordered HOLD — no census corroboration available).
5. Write memory: the wrong-store trap (server cwd = repo root).
6. NOT in this lane: deploying/routing the bodied documents on the live
   server (live cut planned separately; live contract still 6f43c809
   worker-served provision — the RETIRED worker-side provision handler is
   deleted in-tree, so the live cut must deploy+route the bodied version
   BEFORE any worker restart onto a rebuilt binary, or new starts break).
  🔴 Also: worker binary on disk ≠ in-tree source now — do NOT rebuild
  and restart the worker until the live cut is planned.

## Queue after this lane

1. Leg C: embed document in binary + verb (`aion assistant up`); #27.
2. Telemetry + #59 one lane. 3. #136 .awl template, #137 MANIFEST.
4. Broken Linux python3 in ~/.local/bin — still un-told to Tom.

## Standing

Tom is ACTIVELY setting up aion for another person's machine (with an AI
helper — that's what LLM-STARTUP.md is for). Meridian IDs from
meridian-comms-bridge-state.md memory ONLY (Tom
dm:c9255b2a-5731-4d17-8124-e3bfa2224186 · Waffles
dm:896955e1-86dd-4d6a-9c25-f3f978b189a9 · Athena
dm:45cf420e-621d-467b-b96c-5bffb62c04f3). Local collective CLI :19876
DOWN (with Cally). Protected holds: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`, backup
mirror `../aion-pre-scrub-backup-20260804.git`. Never bare cargo clean ·
no launchd · beamr JIT blocker (Artemis) stands · autonomous /loop active
(ScheduleWakeup dynamic; re-arm after each turn).
