# #245 — The server survives its own death
## Design draft for Tom's sign-off (r1, 2026-08-15, Vesper)

Status: DRAFT — Waffles reviews, then Tom signs before anything is built.
Tom asked to own this design himself (his word, 01:47Z: "I'd like to make
sure that I sign up on the design for that one").

---

Authored: Vesper. Ratified for Tom's desk: Waffles (r1 review 01:53Z,
three amendments folded — reboot gap named, breadcrumb closed, numbers
proposed).

## 1. What we're solving, in one paragraph

Tonight (2026-08-15 01:23Z) something on the machine killed the Aion server.
Nothing was wrong with Aion — when it was restarted by hand two minutes
later, both in-flight production runs picked up exactly where they were,
every worker reconnected on its own, and no data was lost. The recovery
half of the story is proven. The gap is the first half: a person had to
notice the server was dead and type the restart. Tom's acceptance case,
verbatim spirit: **kill the server by any means and watch it come back on
its own, untouched, with the runs still going.** It doesn't matter what
killed it — unless it was intentional, it must return.

## 2. What staying dead actually costs (measured tonight)

- The console went dark ("could not fetch workflows") — the operator's
  window on everything vanished.
- The longer the server is dead, the further the world drifts from what it
  knew: tonight the restart re-sent in-flight agent jobs whose original
  agents had survived, minting duplicate agents into the same working
  trees (#246). Fast automatic return SHRINKS that window. It does not
  cure #246 — that is its own fix — but every minute of deadness makes
  the return messier.
- Nothing else was lost. Durable execution held, twice now.

## 3. The two honest shapes

### Shape A — the operating system watches (launchd KeepAlive)

A launchd job with `KeepAlive`: macOS itself restarts the server whenever
it dies. No new process of ours exists at all.

- **For:** launchd cannot be killed by anything short of the kernel. The
  exact killer we saw tonight (a host process-reaping wave) cannot touch
  it. Zero code to write; the "watcher" is one plist.
- **Against, and it is load-bearing:** **Tom ruled launchd out on
  2026-07-14 — "no launchd for anything"** — and that ruling is why the
  workers stay manually started today. This draft does not quietly
  reverse a ruling of his; it presents the collision and lets him
  adjudicate it with full information.
- Also honest: a LaunchAgent dies with the login session; a LaunchDaemon
  runs as root and system-wide, which is its own conversation. And it is
  macOS-only — every other deploy target would need its own equivalent
  (systemd etc.), so the supervision story fragments per-OS.

### Shape B — Aion watches itself (Aion-native supervision)

The `aion` launcher grows a small supervisor: it starts the server as a
fully detached child (never a terminal-session child — the reaper lesson,
now standing law), watches it, and restarts it with backoff when it dies.

- **For:** this is the direction Tom's July ruling pointed ("Aion-native
  supervision is the forward answer"). It composes with the worker-
  supervision spine already landed under #27 — the same policy machinery
  (backoff, all-or-nothing config, kill-proving) applied one level up, so
  Aion has ONE supervision story, ours, portable to every OS. Ships in
  the binary; `aion server` just IS durable, on any machine, out of the
  box — which is the product promise, not just this box's fix.
- **Against:** it is one resident process of ours (small, but real), and
  a sufficiently indiscriminate kill wave could take the supervisor and
  the server together. Detachment makes that unlikely — tonight's wave
  took session-parented processes while every detached process survived
  — but only the OS can guarantee resurrection from ALL kill classes.
  That residual is stated, not hidden.
- **The reboot gap, stated plainly:** after a machine reboot, nothing
  starts the supervisor at all — `aion server` is a person's hand
  again. Shape A covers reboot for free; Shape B does not. Today the
  honest answer under Shape B is that the estate's boot ritual (a
  person) remains the boot-time starter. That may be perfectly
  acceptable — the estate already boots by ritual — but it is the one
  place Shape A's guarantee is categorically wider, not just
  marginally, and it belongs in view when adjudicating.

### Non-shapes, named and refused

- **Cron polling** — seconds-to-minutes of guaranteed deadness per kill,
  and a scheduler dependency. Refused.
- **"Harden the server so it can't die"** — tonight's kill was external
  (SIGKILL-class). No in-process code survives that. Refused as a
  substitute; welcome as ordinary robustness.

## 4. Decisions the design fixes regardless of shape

1. **Default launch posture is detached, disowned, reaper-proof**
   (Waffles' standing rule from tonight). Even the supervisor must never
   be a session child.
2. **No invented defaults.** Restart backoff, hold-down thresholds, and
   the return-time bound come from config, values discussed before build
   (house law). **Proposed for your word — not defaults** (grounded in
   tonight: the server was dead ~2 minutes only because a person had to
   notice; the binary itself starts in seconds):
   - **Return bound: 15 seconds** from death to the console answering
     200 again (detection ≤ ~2s, restart, startup, ready).
   - **Backoff between restart attempts: 1s doubling to 60s** — first
     death restarts near-instantly; a struggling server stops being
     hammered.
   - **Hold-down threshold: 5 deaths inside 10 minutes** — then the
     supervisor writes the breadcrumb, stretches to 5-minute attempts,
     and keeps trying at that pace rather than giving up silently.
   React to these, replace any of them; the slots stay empty until your
   word fills them.
3. **Crash-loop honesty.** A server that dies repeatedly in a short
   window must not thrash silently forever: escalating backoff, then
   hold-down. The breadcrumb is decided, not carried open: **a durable
   file at a fixed path in the aion data directory** (the one place
   that survives everything the server doesn't), written before the
   supervisor gives up — and on the NEXT successful return, the server
   logs the full death history it answered, so the record of the bad
   night lives in the run history where the operator already looks. The
   file is the floor for the night nothing returns: a dead console
   can't announce anything, but a file can't be missed by the person
   who goes looking, and going looking is the only mode that exists at
   that point.
4. **Swap compatibility.** Live binaries are swapped by rename; a restart
   execs the path, and the path points at the new bytes. Supervision must
   therefore roll naturally onto a swapped binary — and must never
   restart INTO a swap uninvited while a fleet flies (the no-swap wall
   stays; restart-same-artifact = recovery, ratified precedent).
5. **Restarts are events.** Every supervised restart is logged where the
   operator will look, with the death it answered — feeding the future
   console uptime/restart surface.
6. **#246 stays its own line.** Supervision shrinks the dead window; the
   duplicate-agent mint on recovery is a separate engine fix (fence the
   re-dispatch on evidence the surviving worker still holds the stream,
   or make duplicates self-detecting at spawn).

## 5. Recommendation

**Shape B — Aion-native**, with the detached posture making the
supervisor itself hard to kill, and the honest caveat in writing that
only the OS can guarantee return from absolutely every kill class. It
follows Tom's own prior ruling, it composes with the #27 spine instead
of adding a second supervision system, and it makes durability a
property of Aion everywhere rather than of this Mac. If Tom weighs
tonight's specific killer (reaper waves) above that and wants the
guarantee, Shape A is one plist and one reversed ruling away — his call
to make, which is exactly why this document exists.

## 6. Acceptance tests the build must pass (either shape)

- `kill -9` the server → console answers 200 within the configured
  bound, both runs resumed, workers re-registered. Automated, on a
  scratch estate — and demonstrated live for Tom on this one.
- Kill it N times rapidly → hold-down engages, breadcrumb written, no
  silent thrash.
- Swap-by-rename, then kill → it returns on the NEW binary, proven by
  hash, not by version string.
- Kill the supervisor too → Shape A: launchd restarts everything;
  Shape B: documented residual, plus the supervisor's own return story
  (launcher re-run) stated plainly.
- Reboot the machine → Shape A: everything returns without a hand;
  Shape B: the boot ritual starts it — the test documents the ritual
  and proves one launcher invocation restores the whole estate posture
  (server, supervision, detachment) rather than leaving a hand-crafted
  sequence.

## 7. Tonight's incident as the acceptance case (the record)

01:23Z pid 56424 killed by a host reaper wave (no crash report; a
sibling capture process died the same instant). 01:25Z restarted by hand,
same binary (sha a8b8614513fbca2f), detached. Console 200; versions
intact; runs b7c5fb1f (K9 fleet) and 7649531a (#207) resumed Running;
all four workers re-registered, verified at the socket level. Recovery:
proven. Noticing and restarting: a person. This design removes the
person.
