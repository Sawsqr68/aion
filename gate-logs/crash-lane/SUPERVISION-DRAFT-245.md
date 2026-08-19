# DRAFT — #245 Aion-native server supervision (for Tom's ruling, via Waffles)

Status: DRAFT, written 2026-08-17 (~15:4xZ 08-16 UTC) under Tom's crash-lane
order. Nothing here is built. launchd is EXCLUDED by Tom's standing word
(2026-07-14: no launchd for anything) — this draft is the Aion-native shape.

## Acceptance cases (all real, all this week)

1. 2026-08-15 01:23Z — server died mid-fleet-run, anonymous.
2. 2026-08-16 07:10:32Z — pid 52844, log ends on a routine activity line.
3. 2026-08-16 15:11Z — pid 2681, two consumer-lagged WARNs then silence.

All three: zero external evidence, estate down until a human noticed (case 2:
~2.5 hours). "Working by morning" means case 4 restarts itself and tells us
who killed case 4.

## The shape: a supervisor parent, not a peer, not launchd

One new posture: the production server runs as the CHILD of a minimal
supervisor process — the `aion` binary itself in a supervise mode (e.g.
`aion server --supervised`, exact verb open), which does only:

1. spawn `aion server` as a child in the supervisor's own fresh session
   (reaper-proof: no terminal ancestry, no shared pgid — the #262 exposure);
2. `waitpid` on it;
3. record every exit VERBATIM — exit code, or the terminating signal — with
   a UTC timestamp, into the same evidence trail as the death note
   (`~/.aion/logs/aion-server.death.log` gains a supervisor-written line);
4. restart the child under the operator's declared backoff policy;
5. stop supervising on an operator stop verb, never on its own judgment.

Why a parent: `waitpid` status is the ONE observation point that sees every
death including SIGKILL — the exact class the in-process death note (built,
branch `death-note`) structurally cannot record. The pair is complete:
inside-view (panic/signal/clean, rich detail) + outside-view (wait status,
never blind). Case 2/3's anonymous face cannot recur under both.

## Requirements

- R1 **Exit ledger.** Every child exit recorded before any restart: wall
  time, pid, wait status decoded (exit N / signal NAME), restart decision.
- R2 **Declared policy, no invented defaults.** `[server_supervision]`
  config section, all-or-nothing exactly like `[worker_supervision]`:
  backoff base/multiplier/max and max-restarts-per-window all operator-set;
  absent section = no supervision, stated loudly at start (the
  uncommissioned-but-wanted warning pattern already shipped for workers).
- R3 **Reaper-proof posture.** Supervisor and child each in their own
  session; nothing inherits a killable terminal pgid. The posture is
  asserted at spawn, not assumed.
- R4 **Recovery is the existing recovery.** A restarted server adopts
  in-flight work per #246 adoption-at-recovery (on main). Supervision adds
  no second recovery path.
- R5 **Honest limit.** The supervisor's own death leaves the server running
  but unsupervised; that state is visible (supervisor writes a heartbeat
  presence the console/truth report can show, shape open). No infinite
  regress: the supervisor takes no config reloads, serves no sockets, and
  is small enough to audit by reading.

## Explicitly out / rejected inline

- launchd (Tom's word), cron, or any OS supervisor.
- A durable workflow restarting its own server (chicken-and-egg).
- Mutual-watch peer pairs — right shape for distributed failover later,
  overweight for one box tonight.

## Open for ruling

1. Verb and default: does bare `aion` (the launcher) commission supervision
   by default, or is it config-only? **Recommendation: config-only first
   cut** — the launcher learns it when the section exists; consistent with
   how defaults have been ruled all season (declared, never invented).
2. Backoff and window values — operator's numbers, to be discussed, not
   defaulted (house law).
3. Whether the supervisor's exit ledger lives in the death-note file (one
   corpse file, my lean) or its own.

— Vesper
