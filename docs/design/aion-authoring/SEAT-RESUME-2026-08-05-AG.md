# Seat resume — marker AG, 5 August 2026

Supersedes AF for the LIVE item. AF's gate work (§1, §1b) still stands.

## 🔴 LIVE P0 — THE ASSISTANT IS UNUSABLE FROM THE OPS CONSOLE

Tom, 04:24Z: *"the assistant is just not working at all through the ops
console"* + the refusal:

```
signal `assistant_continue` was refused for workflow
9b966811-f279-4389-a095-ddda8a7f9513 run 53eee761-9247-4eed-8c98-b974a2a03061
against the contract declared by package version 6f43c809…c984:
<root>: "end" is a required property
```

Then 04:27Z: *"Can you fix it like I don't like it like I just need to be able
to refresh the browser and look."* — **he wants a browser refresh, not a CLI
workaround.** He has already been given the CLI workaround.

### The defect (CONFIRMED at source and in the shipped bundle)

`examples/assistant/awl/assistant.awl:171` declares
`type Continuation { message: String, end: Bool }` — **neither field optional**,
so the package contract requires BOTH on every send.

The console sent exactly one of them, in both directions:
- continue → `{ message }` — refused, `"end" is a required property`
- end session → `{ end: true }` — would be refused, mirror error

So both directions were dead. That is why it read as "not working at all".

### 🔴 The green suite pinned the defect

`assistant.test.tsx:124` asserted `expect(assistantEndPayload()).toEqual({ end:
true })` and **passed for as long as the console could not end a session**. The
oracle read the console's belief off the console. The contract file's own
comment claimed it existed so UI and workflow "can only drift in one visible
spot" — a comment cannot force agreement.

## WHAT IS DONE (uncommitted at compaction — CHECK `git status` FIRST)

All in `apps/aion-ops-console/`:
1. `src/features/assistant/lib/contract.ts` — added
   `assistantContinuePayload(message)` → `{ message, end: false }`;
   `assistantEndPayload()` → `{ message: '', end: true }`.
2. `src/features/assistant/components/AssistantSessionView.tsx` — the
   `mode === 'awaiting'` send now goes through `assistantContinuePayload`;
   import updated; two stale contract comments corrected.
3. `src/features/assistant/lib/mode.ts` — stale comment corrected.
4. `src/features/assistant/__tests__/assistant.test.tsx:124` — the assertion
   that pinned the defect now expects `{ message: '', end: true }`.
5. **NEW** `src/features/assistant/__tests__/assistant.contract.test.ts` — the
   forcing function. Reads `type Continuation` **out of
   `examples/assistant/awl/assistant.awl`** at test time and requires both
   builders to carry exactly its required fields. Throws (never returns empty)
   if it cannot find the declaration, so it cannot go vacuous.

**Measured:** `bun test` 604 pass / 0 fail. `bun run typecheck` clean.
**Control run at my hand:** breaking `assistantContinuePayload` back to
`{ message }` makes the new gate report 2 fail / 3 pass. It bites.

## ✅ SHIPPED AND LIVE-PROVEN — Tom told to refresh (04:33Z)

Commit `9b30ca5e`, pushed. All steps below were carried out and verified.

1. `bun run build` → `apps/aion-ops-console/dist/`. **Artifact verified**, not
   source: the built JS carries `SX(e){return{message:e,end:!1}}` and
   `EX(){return{message:"",end:!0}}`.
2. `~/.aion/config.toml` gained `[ops_console] source = { FileSystem = {
   asset_path = ".../apps/aion-ops-console/dist" } }`, with an in-file comment
   saying why and when to remove it. **Backup:** scratchpad `config.toml.bak`.
3. Old server pid 85467 stopped; restarted from repo root as bare `aion server`
   → **pid 45425**. Data root resolved to `.../aion/aion-data` (his real store).
4. **Verified at the wire, not on disk:** HTTP 200 on :8080 · `aion versions`
   non-empty · the bundle *fetched over HTTP from his server* carries both
   fields · parked run still `Running`.
5. **End-to-end proof:** sent `{"message":"","end":false}` (the workflow's own
   documented no-op nudge, so it cost Tom nothing) → `accepted: true`, and
   `SignalReceived` is now event 9. **The contract refusal is gone.**

## 🔴 NEW DEFECT FOUND WHILE VERIFYING — worker dispatch eligibility flaps

Worker pid 66189 (`./examples/agent-dev/worker/target/release/agent-dev-worker
--address 127.0.0.1:50061 --norn-account default`, cwd = repo root) was
restarted with **identical argv** → **pid 48399**. Not a rebuild; the freeze
holds.

It registers and is withdrawn ~2 seconds later, on a *fresh* connection:

```
04:32:36 INFO  registered liminal worker in-band  identity=agent-dev-worker task_queue=default
04:32:38 WARN  WITHDRAWING DISPATCH ELIGIBILITY … has not been able to reach its
               dispatch path within the window  liveness_ping=2 silence_window_ms=30000
```

Two seconds is far inside the 30 s silence window, so the withdrawal is firing
on ping *count*, not elapsed silence — worth reading against #58 (liveness probe
poisoned its own connection, closed 2abcdd47/2460103d/30e28488) and the held
branch `fix/worker-shell-dial-and-audibility` (#142). **Do not assume #58
regressed without measuring; the same symptom can have a different cause.**

Consequence, stated to Tom honestly: the signal lands, but the next agent round
may not be dispatched. He was told to try it and report.

Server log kept at scratchpad `server.log`; worker log at `worker.log`.

## 🔴 STILL OWED

- `cargo xtask build-ops-console` to regenerate
  `crates/aion-server/ops-console-embed/` and commit, so the next binary is
  correct and the `[ops_console]` config override can be removed.
- The worker eligibility defect above.

## Original plan (kept — it is what was executed)

He cannot see any of this yet. His server serves the **compile-time embedded**
bundle, so source fixes are invisible until the assets he is served change.

Facts established, do not re-derive:
- His server: **pid 85467**, bare `aion server`, started 09:03:01 AEST,
  **cwd `/Users/tom/Developer/ablative/stack/aion`** (repo root — correct).
- His config: `~/.aion/config.toml`, HTTP `127.0.0.1:8080`, gRPC
  `127.0.0.1:50051`, liminal `127.0.0.1:50061`, deploy enabled.
- `aion server` has **NO** CLI flag for the console asset path. It is a config
  section: `OpsConsoleConfig { source }` where `OpsConsoleAssetSource` is
  `Embedded` (default) or `FileSystem { asset_path }`
  (`crates/aion-server/src/config/sections.rs:290-308`).
- `rust_embed` reads from disk in debug builds and embeds in release. His is the
  installed 0.11.0 release → **embedded, so a restart alone changes nothing.**

**THE CHOSEN PATH (minimal blast radius — do not substitute a binary rebuild):**
1. `cd apps/aion-ops-console && bun run build` → `dist/`.
2. Add to `~/.aion/config.toml`:
   ```toml
   [ops_console]
   source = { FileSystem = { asset_path = "/Users/tom/Developer/ablative/stack/aion/apps/aion-ops-console/dist" } }
   ```
   Verify the exact serde spelling against `sections.rs` before writing it —
   `OpsConsoleAssetSource` is an externally-tagged enum, so the TOML shape must
   match or the server refuses to start.
3. Restart pid 85467 **from the repo root** as bare `aion server` (🔴 the store
   is cwd-relative `./aion-data`; starting from `~` gives a silent empty store).
4. Verify: `aion versions` non-empty, HTTP 200 on :8080, the parked run still
   `Running`, then Tom refreshes and sends a message.

Rejected: rebuilding/installing the aion binary. It would carry every main
change since his running image into his live server — far more than he asked
for, and exactly the "substitution presented as a restoration" risk
`crates/aion-server/build.rs` was written about.

Also still owed: sync the built bundle into `crates/aion-server/ops-console-embed/`
(`cargo xtask build-ops-console`) and commit, so the NEXT binary is correct too.

## His run is FINE — verified, tell him so if asked again

`aion describe 9b966811-f279-4389-a095-ddda8a7f9513 --endpoint
http://127.0.0.1:50051` → status **Running**, history ends at seq 8
(`ActivityCompleted`, the agent already replied "Hi! I'm ready to help…").
The refused signals were never recorded. It is parked, waiting.

## Standing constraints (unchanged)

🔴 Worker rebuilds frozen until the #139 cut — **the console is not a worker**,
this does not breach it. 🔴 beamr JIT blocker with Artemis. 🔴 #142 on trigger.
⛔ Never `git add -A` here. ⛔ Never bare `cargo clean`. Backticked commit
messages via `git commit -F -`. Do not edit `CLAUDE.md`.
Tom DM: `dm:c9255b2a-5731-4d17-8124-e3bfa2224186`.

## Commits before this item

`adecb099` the two "duplicate" examples were never a duplication ·
`de4cc3ca` command gate (92 PASS / 0 FAIL) · `309b7f18` marker AF.
All pushed. The console fix above is NOT yet committed.
