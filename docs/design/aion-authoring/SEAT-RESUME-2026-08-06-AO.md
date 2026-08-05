# Seat resume — marker AO, 6 August 2026

Supersedes AN. **Latest letter wins.** AN (+ its two amendments) holds #107, the
#141 sweep, the absence-claim rule and the row-verdict law; its record stands.

`origin/main` = `HEAD` = `be82eaf9c` at the time of writing. Tree clean but for
the three NO-SWEEP untracked dirs.

## 🔴 TOM IS LIVE AND HAS GIVEN A DIRECT ORDER — THIS IS THE ACTIVE WORK

Two DMs, 2026-08-05 23:30Z and 23:43Z. **Task #146** holds the measurement; this
marker holds the plan and the constraints.

### What he asked (second DM, verbatim intent)

> "let's put together a tutorial like that I can follow… that does the aion
> worker, the aion workflow, does the deploying and all that kind of stuff and
> we'll follow along. We'll do it, we'll go through it together… with a
> completely fresh start… and at each point we can make the fixes.
> **If you could first try to run through it yourself and make sure that it's
> working, then we go through it together and make adjustments as needed.**
> Also, you might want to get some sub agents to have a look at — there have
> been some issues raised on the aion repository."

**⇒ THE ORDER IS: build it, PROVE IT MYSELF END TO END FROM SCRATCH, then walk
it with him live.** The self-run is not optional and is the whole point — he is
going to follow it keystroke by keystroke.

### Answered already (first DM) — do not re-litigate

He asked whether you can author an AWL workflow from scratch in the console.
**YES, and it is live in the bundle he is running.** "New workflow" box at the
top of the document list on the Authoring page (`DocumentList.tsx:39-87`),
name + `+` button → `POST /awl/documents`. A new document is NOT blank:
`new_document_source` (`crates/aion-server/src/awl/documents.rs:281-288`) writes
a skeleton **and parses + canonically prints it before returning**, so it cannot
emit something the checker would reject.

🔴 **Bundle-proof method that nearly bit me, keep it:** the served entry
`index-COjdYRoc.js` does NOT contain those strings — the app is code-split. It
references `index-CQI5fYti.js`, which the live server returns HTTP 200 /
552266 bytes and which contains "New workflow" / "Create workflow" /
"awl/documents" / "order_flow". **An earlier grep hit the chunk file directly
and would have produced a claim about a bundle that was not the served one.**

Two real gaps I told him about (both still unfixed, both small):
1. **Nothing says WHERE the file lands** — `aion-authoring/`, resolved against
   the server's CWD. Exactly the uncertainty he named.
2. **The name rule is discoverable only by failing** — `^[a-z_][a-z0-9_]*$`,
   error shown only after a bad submit.

⛔ **Do NOT rebuild the console bundle to fix those two without talking first** —
Tom is running that `dist`.

## 🔴 GITHUB ISSUES — HE ASKED FOR AGENTS ON THESE

`gh issue list --state open` → **#10, #9, #7, #6, #5, #1**. #10 and #9 were both
filed **2026-08-05T23:37Z, six minutes before his message** — those are the ones
he means.

**An Opus subagent is IN FLIGHT on #9 and #1** (read-only, no builds, told to
date-check for staleness and to check whether existing tests could actually
fail). Its report had not arrived when this marker was written. #7, #6, #5 are
NOT yet dispatched.

### 🔴 ISSUE #10 IS ON THE TUTORIAL'S CRITICAL PATH — READ IT FIRST

Filed by **annabelgray89 (Min)**, an outside user, working through
`docs/GETTING-STARTED.md` on 0.10.0. **It is the from-scratch experience
failing — precisely what Tom wants a tutorial to prove works.** Three points:

1. **The documented store backend is not in the shipped binary.** Guide gives
   `backend = "libsql"`; `aion-cli` default features are
   `["haematite-backend", "liminal-transport", "norn", "acp"]`. Startup dies
   with *"store.backend = libsql requires the aion-server `libsql-backend`
   feature"*.
   **✅ VERIFIED STILL TRUE ON MAIN TODAY** — `docs/GETTING-STARTED.md:851`
   still says `backend = "libsql"` / `url = "aion.db"` in the copy-paste config
   block, and `crates/aion-cli/Cargo.toml:114` confirms the default set.
   The shipped template `crates/aion-cli/templates/shared/aion.toml:11-22` is
   already correct — **the guide is the piece that is behind.**

2. **`websocket.cluster_broadcast_capacity` required with no default, absent
   from docs.** ⚠️ **PARTLY STALE — DO NOT REPEAT AS WRITTEN.** It IS in the
   docs now (`docs/operations/operations.md:201`) and
   `defaults.rs:37,46-48` shows it **has a default and is only rejected when
   explicitly set to zero** (`CLUSTER_BROADCAST_CAPACITY_REQUIRED` literally
   says *"omit … to use the default"*). GETTING-STARTED now also marks
   `query_timeout_ms` and `event_broadcast_capacity` "Optional; N when omitted."
   **Was measuring 0.10.0; the required-no-default half no longer holds.**
   Still unchecked: whether `cluster_broadcast_capacity` appears in
   GETTING-STARTED specifically.

3. **Relative `store.data_dir` resolves against the process CWD** — the same
   trap already in my memory `aion-server-must-start-from-repo-root`. Min hit it
   independently and **"very nearly reported that durable recovery had
   failed."** Log prints the value unresolved (`data_root="haematite-data"`).
   Their own conclusion: **durability works**; once the path was absolute and
   ports isolated, the kill -9 exercise passed exactly as documented — run
   recovered `Running`, 5 events unchanged, worker execution counter stayed at 1
   across the crash (activity replayed from history, not re-dispatched).
   They offer a PR against GETTING-STARTED.

**Their suggested directions:** resolve relative `data_dir` against the config
file's directory, or log the absolute resolved path; use absolute paths in the
guide; refresh the guide from the shipped template.

## Plan for the tutorial (nothing built yet)

Outline sent to Tom, **silence-default = build this**:
1. where the button is / what you get · 2. making it do one real thing ·
3. adding the worker · 4. deploying · 5. running and watching.

**Build it the way the AWL deck is built.** `docs/authoring/build-deck.py`
slices every snippet out of a real document under `docs/authoring/deck-examples/`
(10 files, `01-basics.awl` … `10-waits-failure.awl`) and **refuses to write the
deck at all if any example stops passing `aion awl check`.** Output is
`docs/authoring/awl-deck.html` (~50 KB), shell is `deck-shell.html`.
⇒ It cannot teach a shape the language rejects. Keep that property.

**One question put to him, unanswered:** is the deck for him showing someone
over his shoulder, or someone alone with no one to ask? I guessed the first.

### 🔴 How the self-run MUST be done

- **Isolated world.** `AION_HOME` is the isolation control (#113, `8d731436`).
  Separate ports. Model it on **#143's fresh-machine test**, which stood a world
  up and tore it down with **Tom's pids untouched**.
- 🔴 **Tom's live pids are 45425 (server) and 48399 (worker), both started from
  the repo root. DO NOT TOUCH, RESTART, OR KILL THEM.** They had ~6h uptime and
  `/health/live` 200 at last check.
- **Use absolute paths for `store.data_dir`** — issue #10 point 3 is the trap
  that makes a working system look broken, and the tutorial must not walk Tom
  into it.
- **Do not copy the GETTING-STARTED config block** — its `backend = "libsql"`
  will not start on a default install. Use the shipped template's haematite
  form.

## Standing constraints (unchanged, all still binding)

🔴 Worker rebuilds frozen estate-wide until the #139 cut — **but Tom has now
directly ordered a tutorial that builds a worker.** Tom outranks; even so, do
the build in the isolated world and **never restart his live processes**.
Raise it with Waffles rather than assuming.
🔴 Tom's `~/.aion/config.toml` `[ops_console] source = { FileSystem = … }` line
must stay until he installs a binary built from `b20d2da1` or later.
🔴 beamr JIT blocker with Artemis — keep long workflows off until fixed beamr.
⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
⛔ NO WORKTREES in aion; one mutating lane. ⛔ Never publish artifacts for Tom —
deliverables are local files he opens. Backticked commit messages via
`git commit -F -`. Do not edit `CLAUDE.md`. Never mention prospekt.
NO-SWEEP untracked: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.

## Board

- ✅ Closed at this seat: #84, #101, #102, #103. R2.4 discharged.
- 🔬 #107 measured, NOT built — with Tom via Waffles.
- 🔬 #141 both measurable legs done (`9598d6c41`, `be82eaf9c`); lane shrunk and
  approved; oracle unbuilt, queued behind the #139 cut.
- 🔴 **#146 IS THE ACTIVE WORK** — Tom's tutorial.
- Held on Tom: #139 live cut, #142, the 0.11.1 decision.
- Waffles holds for Tom's docket: route-retire, the hardcoded
  `DEFAULT_PAYLOAD_LIMIT_BYTES`/`DEFAULT_HISTORY_LIMIT`, #107.

## Method memories current at this marker

`a-stated-reason-is-evidence-both-ways` ·
`a-census-is-decisive-only-over-what-is-not-exported` ·
`a-verdict-invariant-to-the-error-cannot-see-it` ·
`narrowing-makes-downstream-tests-vacuous` ·
`confidence-is-what-stops-you-measuring` ·
`mutation-must-sit-on-the-tests-path` ·
`withdrawal-proven-against-the-artifact` (earned again today on the console
bundle — the served artifact, not the file on disk).
