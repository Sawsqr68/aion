# 📍 SEAT RESUME — 2026-08-04 (S)

## ✅ SWAP EXECUTED ON TOM'S GO (02:25Z "you can do what you need to do")

- Old 99844 drained gracefully; NEW SERVER pid 48168 = installed
  `~/.cargo/bin/aion` 0.11.0, same ports/config/store. `/build` body names
  0.11.0; console 200; 21 executions intact; deploy smoke green; worker
  77969 re-registered on its own; REAL assistant round proven ("I can hear
  you." — gpt-5.6-sol) then cancelled. Startup logged 6 dispatch-eligibility
  withdrawals (liveness ping raced connection setup) — SELF-HEALED, dispatch
  proven by the round. Log: ~/.aion/server.log.20260804.
- **Assistant UPGRADED live d9774285→67128ff6** (current awl/assistant.awl:
  agent marker, query surface, corrected teaching text; live d977 predated
  the Jul-30/31 document edits). Proven by a live round on 67128
  ("The updated aion AWL authoring assistant is live."), cancelled clean.
  Rollback: `aion route` back to d977 (still loaded).
- **scripts/setup.sh SHIPPED + PROVEN** (Tom's one-command setup ask):
  clones repo if standalone, installs aion-cli if missing, chmod-700 home,
  starts server (banner-derived addresses, /build BODY readiness), deploys
  awl/assistant.awl (the tracked truth — assistant.aion archive is
  GITIGNORED and stale, do not use), builds+starts worker, waits for
  registration. Proven end-to-end TWICE on isolated AION_HOME
  (~/.aion-setup-test, offset ports, torn down): fresh path + idempotent
  re-run path, plus a real agent round on the test server. Re-run on Tom's
  box = clean no-op (his server/worker detected and left alone).
  ⚠️ `~/.local/bin/python3` is a LINUX x86-64 ELF on this macOS box —
  broken for anything bash resolves; the script was rewritten sed-only
  because of it. Tell Tom sometime.
- Tom's clarified direction: the REAL target is install-for-a-stranger with
  no help — script is today's answer; #139 (assistant inside the aion
  distribution, no repo needed) is the proper build. Tom also said the
  /build endpoint suggestion wasn't helpful — dropped, don't re-raise.

**Supersedes R. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the MODEL,
not my name.

## 🔴 LIVE: SERVER SWAP (#108) STAGED — WAITING ON TOM'S "GO"

**Tom is DEMOING TO PEOPLE TODAY (Aug 4) and asked for the best possible
version (21:59Z Aug 3).** Everything is done EXCEPT the swap; I sent him the
plain four-part (~22:0xZ): say "go" → one minute. Rule stands: pid 99844 is
never killed without his word.

**Swap runbook (execute on his GO):**
1. ✅ DONE: store backup `.aion-data-preswap-20260804` (APFS clone of the
   live 6.4G `./aion-data`, taken 22:00Z Aug 3, crash-consistent). Live store
   is repo-root `./aion-data` (haematite shards); server cwd = repo root;
   config `~/.aion/config.toml` DELIBERATELY leaves store.data_dir unset so
   the resolver picks the legacy `./aion-data` — do not "fix" that.
2. ✅ DONE (22:05Z Aug 3): preflight GREEN. `~/.cargo/bin/aion server` with
   scratch store/authoring under `~/.aion/preflight-0110/` (owner-only chain
   — the earlier /private/tmp refusal was the path gate BY DESIGN), ports
   18080/18051/18061: started clean, console 200, build identity answered.
   Preflight killed, dir deleted, live processes verified untouched after.
   ⚠️ ROUTE CORRECTION: the build-identity route is **`/build`**, NOT
   `/api/build` — the SPA fallback answers any unmatched path with 200
   text/html, so probes MUST assert on the BODY
   (`curl -s http://127.0.0.1:8080/build | jq -e .version`). The installed
   binary answers `{"version":"0.11.0","commit":"unknown",...}` — commit
   "unknown" is CORRECT for a cargo-install build (no git tree), the
   version field is the identity here. The old live binary answers /build
   with HTML (no such route) — that contrast IS the swap verification.
3. On GO + green preflight: `kill -TERM 99844` (graceful drain), confirm
   exit, then from repo root:
   `nohup ~/.cargo/bin/aion server > ~/.aion/server.log.20260804 2>&1 &`
   (installed 0.11.0 = the battery-proven published binary; embeds the new
   console; same ports 8080/50051/50061 from ~/.aion/config.toml).
4. Verify: `/build` body names 0.11.0 (#123); console 200; workflow list
   shows the pre-swap runs (store intact); assistant worker (pid 77969,
   gpt-5.6-sol) redials 50061 and re-registers — check its log
   `/tmp/agent-dev-worker-20260803.log` for a fresh "connected and
   registered"; then a new-surface deploy smoke
   (`aion deploy` a rev2 fixture, e.g. step-bodies/valid/send_signal.awl).
5. **ROLLBACK if 0.11.0 refuses the store**: old binary is GONE from disk
   (99844 runs a deleted image — once killed, unrecoverable), but
   `./target/release/aion` (the ACCEPTED Jul-31 binary, sha f748956…,
   1,427,144 KiB dir — preserved through the sweep for exactly this) starts
   the same way; restore store from `.aion-data-preswap-20260804` if writes
   corrupted. Then tell Tom plainly what the demo runs on.
6. Tell Tom done + what's new on screen; offer the demo crib sheet
   (demo philosophy: HE drives; my job = known-good situation + crib sheet).

## Since marker R (all pushed; origin/main = 9e9cfebe)

- **Assistant fully set up (Tom's orders Aug 3)**: worker restarted on
  rebuilt binary (new teaching content) and on **gpt-5.6-sol** via NEW
  `--norn-model` pass-through flag (commit 4ea05617; crate gates 86/0,
  clippy/fmt clean; no default — omitted defers to norn). Live pid 77969,
  argv: `--address 127.0.0.1:50061 --norn-account default --norn-model
  gpt-5.6-sol --norn-allow-degraded-session`, cwd = repo root, log
  /tmp/agent-dev-worker-20260803.log, startup line carries
  norn_model=Some("gpt-5.6-sol").
- **Installed CLI now 0.11.0** (`cargo install aion-cli` replaced 0.10.0 in
  ~/.cargo/bin; ~25 min cold). Verified: `aion 0.11.0`; fixtures
  query_answer/send_signal/dev_brief all PASS at my hand (zsh trap: unquoted
  `$var` does NOT word-split — iterate literals).
- **Install-verify receipt banked**: gate-logs/deans-box-0110-install-verify
  .txt, 107/0 (commit 9e9cfebe) — was /tmp-only, cured for Cally's census.
- **Cally census ANSWERED** (Tom's ask, three buckets with receipts, ~03:53Z
  Aug 3): finished = release arc + docs + console debt + #91 + assistant +
  sweep; in-flight = #108 (Tom's word), #139 (Tom's word), #27, #77/#92
  (behind #82/Artemis), #127 (Apollo), #90 (re-scope), #57; discussed-only =
  #136/#137/#132/#102/#103/#94/#107/#23/#30/#37. Named my one
  CLAIMED-NOT-VERIFIED: the local 3-fixture spot-check (transcript-only).
- **#139 boarded**: assistant bundling (teaching resources into the package
  + server-supervised worker; answer to Tom's "bundle it for other people" —
  keep the process, hide it behind the product; `aion worker agent` already
  exists in 0.11.0 with --harness norn/acp). GATE: Tom's word.
- **#138/#135 closed** on the board.

## Standing (unchanged)

pid 99844 never killed without Tom's word (this window may BE the word —
wait for his "go") · stash@{0} not ours · seat's untracked dirs:
.aion-accepted-binary-20260731/, .aion-jjuabk-snapshot-20260731/,
.aion-data-preswap-20260804/ (MINE, the swap backup — keep) ·
beamr JIT blocker (Artemis): Benji off hours-long workflows · never bare
cargo clean · no launchd · Athena's ceilings on local cargo (and WIRE the
anchor check, don't just declare it) · Waffles receipts culture ·
usage wall may cut sessions — this file is the resume point.

## Comms (IDs from meridian-comms-bridge-state.md memory ONLY)

Tom dm:c9255b2a-5731-4d17-8124-e3bfa2224186 (plain, short, answer-first,
four-part for decisions) · Waffles dm:896955e1-86dd-4d6a-9c25-f3f978b189a9 ·
Athena dm:45cf420e-621d-467b-b96c-5bffb62c04f3 · Cally
dm:15a23351-009a-4e69-8d63-45602a805f1c.
