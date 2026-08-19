# 🔴 TOM'S RULINGS — 2026-08-14 06:40Z, HIS OWN DM (dm:c9255b2a-5731-4d17-8124-e3bfa2224186)

Answers to the five map decisions + corrections. **Read with COMPACT-57-STATE.md.**

## 1. 🔴 NO ARTIFACTS — HE DOESN'T WANT THEM

> "I really don't care for those like artifacts. I'd much rather they just be
> HTML pages that you write up on your own things that you publish online"

**STOP using the Artifact tool for deliverables.** Write plain HTML pages and
publish them the ordinary way. The readiness map went out as an artifact —
that was against his preference. **Save to memory.**

## 2. CI — GITHUB ACTIONS RULED OUT, FLAT

> "I don't want to use GitHub actions for any of the CI. Aside from the fact
> that they're expensive, they're also stupid."

**Decision 1 ANSWERED: Aion is its own CI.** My recommendation stands, his
reasoning is cost + quality. Build toward repo_gates + fleet as runner pool.

## 3. TRUST BOUNDARY — DE-ESCALATED, BUT NOT DISMISSED

> "I wouldn't stress too much about that. Things can be run. Everybody here is
> trusted but the main thing it just be like work pool pollution... we'll just
> run things connected to this server and when we need to expand that out once
> they're ready for the more network approach, we'll expand it out from there.
> The boxes get access, don't worry so much about that, we'll just handle that."

**Decision 2 ANSWERED.** The real risk he cares about is **WORK POOL
POLLUTION** — the wrong worker picking up work / queues crossing — NOT
attackers. Sequence: keep workers connected to THIS server first; widen to the
network when ready. **He handles box access.**
⚠️ My auth finding (`auth: None`, registration validates CONTRACT not IDENTITY)
still stands as fact and still matters for pool pollution — an unintended
worker can register and take work. Frame it as **pollution/ownership**, not
security. The liminal shared-token Connect gate (exists since 0.2.3, we pass
None) is the cheap remedy and is now a POOL-HYGIENE argument, not a threat one.

## 4. THE REMAINING DECISIONS ARE MINE

> "I don't know most of this stuff sounds like this is a you decision to make."

Covers: idle cost (#227), the intermittent server failures, the long-running
issues. **Decide at my hand; do not route back to him.**

## 5. 🔴 THE FRAME FOR THE WHOLE WEEK

> "part of this work is identifying all those things that cause those issues."

Finding the causes IS the deliverable, not a detour from it. #82, #196, #227,
#233 are the work, not obstacles to it.

## 6. 🔴 CONCRETE DESIGN HE GAVE — DURABLE AGENT RESUME

> "norn has the `--resume-if-exists` flag, and that allows us to basically just
> have it resume mid run. But we can also have the Claude Code sessions — if we
> set their session ID, which is very easy to do, then we just generate the ID
> that we use... Claude Code sessions need to be a UUID version 4, but we can
> generate that at the start of the workflow as the first step, and then just
> use that throughout. So that way if a forward step fails, we can just have it
> go a different pathway and go to the resumed step."

**The pattern, in AWL terms:**
1. FIRST STEP of the workflow generates a session id (UUIDv4 for Claude Code).
2. That id is threaded through every agent step as a declared parameter.
3. Forward step fails → `route` to a **resumed** step that re-enters the SAME
   session (norn: `--resume-if-exists`; Claude Code: the pinned session id).

This is durable agent execution built out of AWL routing + a stable session id.
**Directly answers #205 (resume a fleet run with the same coordinator session).**
Design this into the dev-leg templates.

## 7. 🔴 CORRECTION TO MY MAP — OFF-MACHINE HAS RUN BEFORE

> "the stuff has run off machine before, that was actually some of the earliest
> stuff. So there's not much more to do — it definitely works across, we just
> point, we just listen over the network and new workers can connect like that."

**My map said "every Norn round we have ever run has been on this box" and
called it the highest-value proof of the week. Tom says that is WRONG** —
remote workers over the network are among the earliest proven things.

⚠️ **DO NOT just accept OR just dismiss this.** My claim was an absence of
measurement *in my own evidence*; his is recall of earlier work. Both can be
true: it worked earlier, and I have no artefact for it. **The right move is to
find the evidence** (earlier remote-worker runs / tests / logs), not to re-prove
from scratch and not to silently drop the item. Downgrade "never run" →
"proven earlier, evidence not in hand — locate it".

## 8. WHAT HE PARKED

Worker spin-up timing ("when to spin up workers, all that kind of stuff") —
**come back to later.**

## 9. HIS CLOSING INSTRUCTION

> "make sure you dig in deep and just make sure we understand all this stuff
> properly."

Depth over speed on the causes. This licenses the digging.

## IMMEDIATE CONSEQUENCES FOR THE MAP

- Decision 1 (CI): **ANSWERED — no Actions, Aion runs its own.**
- Decision 2 (trust boundary): **ANSWERED — de-escalated; reframe auth finding
  as work-pool pollution; LAN widening is staged and he handles access.**
- Decision 3 (per-node credentials): he says boxes get access, he'll handle —
  likely moot, confirm when node onboarding is written.
- Decision 4 (#82 first): **effectively MINE now** ("a you decision").
- Decision 5 (idle cost): **MINE.**
- "Second-box proof" item: **REWRITE** per §7 — locate prior evidence first.
- Republish the map as a **plain HTML page, not an artifact** (§1).

## 10. WAFFLES RULINGS 06:42Z (after the auth read)

- **TAG: STAMP IT.** Blob-identity at cut commit 5c5711a53 discharges both his
  conditions. Tag the CUT COMMIT (not branch head); message names both reds +
  dispositions (outbox_e2e = #85 classified; changelog_embed drift = present in
  run 2's tree, synced IN the cut commit, verified by blob identity there).
- **CORRECTION TO MY "config-shaped" WORDING.** `auth: None` is constructed in
  CODE at `run.rs:836`. Threading a token = a **small aion build with a config
  surface**, not pure config. TWO ENDS: server reads the value and passes
  `Some(token)`; worker-side outbox config carries it. **They land together or
  READER-FIRST** — a server refusing tokens before workers can present them
  BRICKS REGISTRATION.
- **DO NOT START IT** until Hermes answers whether liminal's Connect path runs
  any admission check regardless of `auth: None`. If there is a second check,
  the shape changes.
- Decision 2 amended to Tom: dissolves into "open the port and turn the gate on,
  together."

## 11. STATE OF THE THREE AUTHORING LANES (Waffles opened all four, one closed)

1. Node onboarding runbook — NOT STARTED.
2. Swarm doc rewrite onto isolated trees (#221) — NOT STARTED.
3. Gate evidence fix (#236) — RE-AIMED, no longer blocking anything (see below).
4. Inbound-worker auth read — ✅ DONE (findings in §3 + §10).

**#236 re-aimed and SMALLER:** the gate KEEPS everything; full logs live at
`gate-logs/gates-runs/<workflow-id>/<gate>.log` but that dir is **GITIGNORED**
(`gate-logs/.gitignore:7`), so full logs never join the COMMITTED evidence
bundle, which carries only a 40-line tail. Text exists only on the box that ran
it — wrong under a multi-node fleet where the reader is elsewhere. Fix: surface
failing-target list + per-target failure text into the committed bundle.
NOT a blocker; the "evidence fix before characterization" ruling is MOOT
(nothing to characterize — #235 is settled, not flaky).

## 12. HERMES'S ANSWER 06:43Z — TOKEN GATE IS THE WHOLE ADMISSION STORY

Measured at `connect_response`, `crates/liminal-server/src/server/connection/apply.rs:350-410 @19ea485`:

- With `auth: None` the check is **skipped entirely**. Code's own comment:
  "When no token is configured the server is open."
- Connect then does **only** version negotiation + participant-posture selection.
- The pre-auth frame fence **only engages when a token is configured**.
- The check is `constant_time_eq` and runs **BEFORE** version negotiation —
  a refused peer learns nothing about the protocol.
- Everything deeper (enrollment secrets, attach credentials, roster refusals)
  is **authority WITHIN an admitted session, not admission** — none of it
  substitutes for the token.

**⇒ My read is confirmed by the dependency owner. There is NO second admission
check. `bind + token as ONE change` is correct AND complete for connection
admission.**

## 13. 🔴 ROUTING CHANGED — TOM DIRECT, WAFFLES NOT TRACKING

Waffles: "Tom has taken this thread direct with you — he's told me he's given
you his feedback, that they've run things over the network before, and that I'm
not to track it. The build shape and timing are between the two of you now.
My earlier 'don't start until Hermes answers' is discharged."

**So: this thread is TOM-DIRECT. Do not route it back through Waffles.**
Note this is a NARROW exception to the standing "decisions route via Waffles"
law — it applies to THIS thread (network/workers/dev-loop) because Tom took it
himself. Everything else still routes via Waffles.

⚠️ Per Tom §3: widening is STAGED — run connected to THIS server first, expand
later, he handles box access. So the token work is **not urgent this week**;
it is the thing to have right BEFORE the pool spans boxes.

## 14. WHAT I OWE TOM NEXT

1. **Find the off-machine evidence** (his §7 correction). Cheap, and I said I'd
   either produce it or say plainly that I couldn't.
2. Redo the readiness map as a **plain hosted HTML page**, not an artifact.
3. Fold the resume-session design (§6) into the dev-leg templates.
4. Dig into the CAUSES (#82, #196, #227, #233) — his framing: that IS the work.

## 15. ✅ OFF-MACHINE EVIDENCE FOUND — TOM WAS RIGHT

`docs/demo/MULTI-HOST.md` (216 lines), landed **2026-06-29** in
`3193306bf ultracode(infra): demo deploy-enable + parameterized multi-host mesh
config + remote-worker docs`. Six weeks before my map claimed it had never run.

**The capability is real and specific:**
- `scripts/demo/gen-cluster-config.sh` has THREE modes: loopback / shared-IP /
  true multi-host via `DEMO_HOSTS="ip1 ip2 ip3"` (one routable IP per node).
- Four listeners per node, all host-interpolated: HTTP `8090+i`, gRPC `50051+i`,
  haematite replication `7700+i`, **liminal push `8190+i`**.
- Remote worker, TWO transports: (A) liminal server-push via repeatable
  `--address HOST:PORT` → `aion_worker::serve_with_redial`, one per candidate
  node so it MIGRATES on failover; (B) gRPC pull via `WorkerConfig.endpoint`.
- Reachability summary states exactly which port a remote worker needs.

**⇒ Correct the map: the capability is BUILT and DOCUMENTED. What I lack is a
current RUN RECORD, which is a different and much smaller claim.**
No `DEMO_HOSTS` usage outside docs/scripts; no non-loopback addresses in
gate-logs. **Absence of a log is not absence of a run** — the doc says "what the
code does today", written by whoever built it.

## 16. 🔴 MY AUTH FINDING WAS ALREADY DOCUMENTED — CORRECT MY OWN FRAMING

`MULTI-HOST.md` §"Security posture" ("the honest current state") states it
plainly, 6 weeks before I reported it as a finding:

> "**Dev-open auth.** The demo emits no `[auth]` section, so auth is disabled.
> In that mode the server **trusts the worker-supplied `x-aion-namespaces`**
> header (and `x-aion-deploy` for deploy) — **a remote worker effectively
> self-declares its own authorization.**"
> "**No TLS.** ...Inter-node replication, gRPC peer traffic, and liminal push
> are therefore plaintext across hosts."
> "**Deploy is enabled in the demo configs.** ...any caller that can reach the
> node may deploy."
> "keep the mesh on a trusted network."

**What I actually added** was the liminal-layer mechanism (`auth: None` at
`run.rs:836`) + Hermes's confirmation that the token gate is the WHOLE admission
story. The POSTURE was known. Do not present it as newly discovered.

**🔴 AND THERE ARE TWO GATES, NOT ONE — my "bind + token" named only one:**
1. **liminal layer / ADMISSION** — shared-token Connect gate. Exists since
   liminal 0.2.3. We pass `None`. Decides WHO MAY CONNECT.
2. **aion layer / AUTHORIZATION** — `[auth] enabled = true` + `jwks_url`,
   Bearer validation. Exists. Demo emits no `[auth]`, so the server trusts
   `x-aion-namespaces` from the worker. Decides WHAT AN ADMITTED PEER MAY DO.

They guard different things and neither substitutes for the other.
**#2 is precisely Tom's "work pool pollution" vector**: a worker self-declaring
its namespaces can take work that isn't its own. The doc's own hardening list
(add `[tls]`, enable `[auth]`+jwks, stop trusting `x-aion-namespaces`) is the
pre-existing answer and should be the spine of the write-up.
