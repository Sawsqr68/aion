# #262 root-cause dig — findings (Vesper, 2026-08-16 ~03:5xZ)

Scope: Waffles' 03:40Z amendment (Tom's word) — dig pulled forward, forensics
only, parallel with the read-path watch. Evidence: the dying incarnation's log
`gate-logs/server-restart-20260815-18xxZ.log` (pid 74870, launched 18:19:02Z
08-15 from my session's Bash tool call), OS diagnostics, session transcript,
and code at my own read (aion @ working tree, haematite local repo, beamr).

## 1. The timeline, corrected

The banked mechanism said "shard timeouts → tap flood". The log says otherwise:

- 18:36:02Z — transient wedge pulse #1: shard actors 24/33/11 ReplyTimeout
  (tap publish, outbox claim, quota broadcast all fail). RECOVERED.
- 19:51:06–39Z — board #260's ENOSPC event: "shard WAL error: wal tree error:
  invalid tree node" ×5 through completion_retry; completion became durable
  19:52:10 after backoff. RECOVERED (this incarnation survived zero-disk).
- 22:47:29Z — transient wedge pulse #2: same three faces, one beat. RECOVERED.
- 02:20:23Z — builder activity completes; next dev turn continues.
- **02:25:45.704Z — tap flood BEGINS with NO preceding shard-timeout line.**
  12,131 "no available capacity" rejections over ~4m51s.
- 02:30:23–34Z — first fatal-window shard timeouts surface (actors 43/57,
  outbox + quota + liveness-adjacent paths).
- 02:30:36.582Z — log ends mid-flood. No panic, no shutdown line.

Reading: the queue filled ~4.5 minutes BEFORE any timeout printed. Timeouts
only print after the caller's timeout elapses — a HANGING drain fills the
queue silently first. The flood is the visible shadow of a stalled
durable-append, not a separate actor.

## 2. Tap-capacity coupling: SETTLED, not a killer

`aion-server/src/worker/liminal_transport.rs:1899-1906`: the observability
tap hands events to a bounded ordered drain queue via `try_send`; a full
queue DROPS the event with a WARN ("never buffered without bound"). By
design fail-open. Consequence of the flood: ~12k transcript events dropped
(observability data loss for that window), zero dispatch impact, zero exit
path. The tap cannot kill the server.

## 3. The shard-actor wedge: beamr-scheduled

`haematite/crates/haematite/src/shard/actor/handle.rs` (CORE-007): a shard
actor is a **beamr process**. The handle queues a command and wakes the shard
pid via `beamr::scheduler::Scheduler::enqueue_atom_message`; the reply comes
back on an mpsc with a caller-supplied timeout; ReplyTimeout = "timed out
waiting for shard actor N". So the wedge face means the BEAMR SCHEDULER did
not run the shard process (or its handler stalled) within the timeout.

Supporting shape: the wedged actor pids DIFFER across pulses (24/33/11, 51,
43/57/58/59) — scheduler-level starvation under load, not one corrupted
shard. All three pulses coincide with heavy periods (fleet reseat at 18:36,
post-ENOSPC churn, the builder's transcript-heavy dev turns). Couples
directly to #227 (six beamr VMs in one idle server, ~58% of a core burned)
and sits on Artemis' side of the beamr boundary for the scheduler half;
haematite's exposure is that CORE-007 has a fixed-pid handle and CORE-008
(router/supervisor re-spawn) owns restart — whether a wedged-not-dead actor
is ever recycled is the open store-side question.

## 4. The silent exit: SIGKILL by elimination

- No self-exit: `process::exit`/`abort` appear NOWHERE in aion-server, aion
  engine, or aion-cli production code (sole workspace hit is a beamr helper
  binary's main).
- Not a panic: panics unwind (no abort profile), print to stderr; stderr was
  redirected to the log file; nothing printed. Lint denies panic!.
- Not SIGTERM/SIGINT: run.rs installs handlers; the FIRST act after a signal
  is `info!("shutdown signal received; beginning graceful drain")`
  (shutdown.rs:168). No such line exists.
- Not SIGSEGV/SIGABRT: ReportCrash provably works for this binary family
  (namespace_restart .ips files exist); NO .ips for the server on 08-16.
- Not jetsam/OOM: no JetsamEvent report in the window (12:30:36 local); the
  two nearby jetsam files (12:34 08-15, 08:30 08-16) contain no aion entries
  and predate the death by hours.
- Unified log: zero events for pid 74870 or "aion" in the window.

The only terminator that leaves exactly this nothing is **SIGKILL**.

## 5. The kill footprint and the prime suspect

Footprint at 02:30:36Z: server (74870) died; its norn builder child (86486)
died — the norn agent is a DIRECT CHILD of the server process (harness
composed at the binary root; log line 1), so pipe-EOF kills it the instant
the server dies, no group kill needed; the managed worker mm-capture
(77649) SURVIVED — workers live in their own containment process groups
(the #27 supervision spine). One SIGKILL aimed at the server pid (or its
process group) explains every observation.

Prime suspect: the host background-task reaper (the documented behavior
that kills session-spawned background processes in waves — the reason the
norn delegation law says "detached nohup+marker dispatch"). The 18xxZ
launch was `nohup aion server > log 2>&1 &` inside a foreground Bash tool
call — nohup shields SIGHUP only; SIGKILL goes straight through. My session
was ASLEEP 02:23:25→02:49:00Z (ScheduleWakeup gap) — the death at 02:30:36
falls inside it, consistent with an asynchronous reaper wave, inconsistent
with anything I ran. Tom's word (via Waffles 02:52Z): he did not take it
down. Death #1 (01:23Z 08-15) matches the same class (silent, no record)
but its incarnation's launch provenance is unpinned.

Status: suspect, NOT proven. No direct observation of the killer exists on
the box (SIGKILL leaves no attribution anywhere we can read). Attribution
rests on elimination + the documented reaper behavior + timing.

## 6. LIVE EXPOSURES found during the dig (present tense)

- **The new incarnation (98979) sits in orphaned process group 98976 — the
  dead launcher shell's group.** disown removed job-table membership only;
  the pgid is unchanged. If the reaper kills by pgid of ended tool shells,
  the current server carries the SAME vulnerability that (per §5) killed
  its predecessor. True containment = launch in its own session (double-fork
  daemonization / Python os.setsid — macOS has no setsid binary; launchd is
  ruled out by standing law). #245 (self-supervision) is the durable answer.
- **Duplicate worker identity live NOW**: old mm-capture 77649 (orphan of
  the dead server, 9.5h old) AND new mm-capture 46640 both running, same
  identity, same endpoint :50051. Double-serving is the #254/#258 hazard
  class. Not touched — disposition needs a verified-idle check first.

## 7. What becomes fix-shaped (queues in ruled order, per the amendment)

1. #245 (launch posture + self-supervision): the fix for §5/§6a. The cut+
   restart in the ruled sequence should ALSO change the launch to a real
   session-detached posture, not just nohup+disown.
2. Store-side (#262 proper): CORE-008 supervisor should detect a
   wedged-not-dead shard actor (ReplyTimeout streak) and recycle it;
   surfaced to aion as retry-after-recycle, not an operator-invisible stall.
3. beamr-side (Artemis, via Tom/Waffles): scheduler starvation under load —
   same lane as #227's six-VM burn; the wedge pulses are its symptom.
4. Tap flood hygiene (small): 12k identical WARNs in 5 minutes is a
   log-flood; the #147 transition-rule pattern (state-change logging) fits.
5. Duplicate-worker reconnect: a worker surviving its server should not
   re-register alongside its replacement under the same identity without
   the server noticing the collision (#254/#258 adjacency).

## Evidence pointers

- Dying log: gate-logs/server-restart-20260815-18xxZ.log (12,347 lines).
- Launch record: session transcript 5849e0d8 @ 2026-08-15T18:19:02.562Z.
- Tap try_send: crates/aion-server/src/worker/liminal_transport.rs:1899.
- Shard handle/timeout: haematite crates/haematite/src/shard/actor/handle.rs
  (ShardError::ReplyTimeout, CORE-007 header).
- Signal path: crates/aion-server/src/run.rs:301-327,436-457;
  crates/aion-server/src/shutdown.rs:161-172.
- Jetsam/crash listings: ~/Library/Logs/DiagnosticReports/ +
  /Library/Logs/DiagnosticReports/ (no aion entries 08-16).
