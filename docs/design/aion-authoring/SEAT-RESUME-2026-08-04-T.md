# 📍 SEAT RESUME — 2026-08-04 (T)

## 🔴 LIVE UPDATE: docs COMPLETENESS SWEEP (Tom 03:36Z: "documentation for
## absolutely everything") — MID-FLIGHT, supersedes the closed docs order

Two audits ran (whole-product coverage + AWL reference vs code). Writers
dispatched on disjoint files; 4 died on a usage limit mid-write and were
RESUMED via SendMessage after Tom's /login (research context intact).

DONE + seat-verified: docs/operations/ops-console.md (247 ln, new) ·
docs/operations/API.md (23→64 verified routes, gRPC+WS tables) ·
examples/README.md (30/30, links clean) · errors.md (all 14 wire codes) ·
docs/workers/SHELL-WORKER-MANIFEST.md (fields match struct) ·
GETTING-STARTED.md shell-worker cmd +--awl (required flag) ·
docs/README.md wiring (console+manifest entries, SDK section ×8 links
verified, examples link, honest aion-authoring-dx line) · root README
(docs/README.md pointer + new rows) ·
docs/design/aion-authoring/WORKER-TELEMETRY-DIRECTION.md (Tom's 04:00Z
two-plane telemetry ruling: durable history lean, live stream unrecorded,
worker-local capture; replay never reads telemetry).

SWEEP COMPLETE (this commit): all five writers landed. operations.md
322→626 ln (full config ref regenerated — old "Required keys" table was
WRONG: those keys default now; deploy ceilings are 64/256 MiB not 16/64;
shard_count default 64 verified sections.rs:657; env overlay is a
33-var allowlist not a pattern; [tls] parses-then-refused not rejected) ·
AWL-REFERENCE.md amended per the 8-item order incl. NEW §15 "what check
does not check" + guard-narrowing section; amender CORRECTED 5 audit
claims on verification (pipe-stage arity is a CHECK rule not lowering;
only TIMED waits refused in on-failure — count_wait_fns gates on
timeout.is_some; indexing refusals are parallel-forms-only; fixture path;
named-fork branches may bind) — all 5 overrides re-verified at seat's
hand · link check zero broken across all 11 changed files · worker
rebuilt release clean (77969 untouched). Lane order told to Tom
(silence-default): #139 → telemetry+#59 as one lane.

**Supersedes S. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. Fable is the MODEL,
not my name.

## 🔴 LIVE TASK: Tom's docs order (03:10Z Aug 4) — MID-FLIGHT

Tom's words: "set out the stuff with the norn-worker and all that kind of
stuff. And then we need to put together like an organized like proper
documentation folder for everything."

**DONE (committed this window):**
- docs/ reorganized with `git mv` into: `authoring/` (AWL.md, authoring.md,
  packaging.md, workflows.md, codegen.md, aion-authoring-dx.md), `workers/`
  (WORKER-AUTHORING.md, DECLARED-COMMANDS.md, activities-and-workers.md),
  `operations/` (operations.md, API.md, errors.md), `contracts/`
  (CANCELLATION-CONTRACT.md, PAUSE-RESUME-DESIGN.md), `vision/`
  (DURABLE-AGENTS…, ROADMAP, AION-DISTRIBUTION-DESIGN,
  WORKFLOW-AUTHORING-AGENT-IDEA), REVIEW-23-07 + WORKFLOW-RESILIENCE →
  `archive/`. `guides/` is gone. GETTING-STARTED.md stays at docs/ top.
- ALL live-surface links fixed and CHECKED (link-checker loop: README,
  GETTING-STARTED both, CONTRIBUTING, examples READMEs, assistant
  resources, every moved doc's internal links — final pass = zero broken).
  Historical records (remediation lanes, design briefs) deliberately NOT
  rewritten — they describe their moment.

**UPDATE: ALL FOUR REMAINING STEPS DONE (this commit).** docs/README.md
index written · docs/workers/AGENT-WORKERS.md written (sources: `aion worker
agent --help`, norn_invocation.rs, main.rs flags, setup.sh) · both
link-checked zero-broken · worker binary rebuilt release exit 0 (live pid
77969 NOT restarted — fresh binary waits for the next natural restart) ·
Tom told. Docs order COMPLETE; next lane is #139 per BUNDLE-139-NOTES.md.

**REMAINING (do next, in order):**
1. **docs/README.md** — the index/map of the whole docs tree (the
   "organized proper folder" front door). List each section + one-liner.
2. **docs/workers/AGENT-WORKERS.md** — the norn-worker story Tom asked
   for: `aion worker agent <awl>` (0.11.0, document-declared surface,
   --harness norn|acp required, --check prints surface, liminal-only
   transport), the assistant example worker
   (examples/agent-dev/worker: --address/--norn-account required,
   --norn-model pass-through NO default, --norn-allow-degraded-session
   opt-in + the parked-28-days advice story), session pinning
   ({workflow_id}-{activity_type} + --resume-if-exists), scripts/setup.sh
   as the one-command entry. Sources verified this window: `aion worker
   agent --help` output, norn_invocation.rs, setup.sh.
3. **Rebuild the assistant worker binary** (examples/agent-dev/worker,
   cargo build --release) — its include_str! teaching resources
   (WORKERS.md etc.) changed doc paths. DO NOT restart live worker 77969
   during Tom's demo hours unless he asks; binary-on-disk fresh is enough,
   note it for the next natural restart.
4. Commit + push, tell Tom plainly what the folder looks like now.

## This window, earlier (all done, all pushed)

- **#108 CLOSED**: server swapped on Tom's go. Live server pid 48168 =
  installed ~/.cargo/bin/aion 0.11.0, ports 8080/50051/50061, store
  intact (21 executions), worker 77969 re-registered, REAL round proven.
  Log ~/.aion/server.log.20260804. Backup .aion-data-preswap-20260804 KEEP.
- **Assistant upgraded d9774285→67128ff6** (current awl/assistant.awl),
  proven by live round; rollback = `aion route` to d977.
- **scripts/setup.sh shipped+proven** (4c2c11ee): one command, clones if
  standalone, idempotent, per-home detection, sed-only (no python3 —
  ⚠️ ~/.local/bin/python3 is a LINUX x86-64 ELF on this macOS box, broken
  for bash-resolved tools; tell Tom sometime). REPO IS PUBLIC (verified) —
  one-liner: `curl -fsSL https://raw.githubusercontent.com/ablative-io/aion/main/scripts/setup.sh | bash`.
  Prereq floor told to Tom: git, Rust, logged-in norn.
- **#139 boarded in_progress** with design notes
  (docs/design/aion-authoring/BUNDLE-139-NOTES.md, d3c28704): assistant
  inside the distribution; MVP = provision as declared command body →
  `aion worker agent` serves agent action → embedded document + verb.
  Starts after the docs order (docs took precedence — Tom's direct word).
- Tom told: swap done · script done+proven · one-liner · what a stranger
  can do after it (assistant = front door; command workflows need NO
  worker; dev-fleet workers deliberately not included).
- /build route lesson: assert BODY not status (SPA fallback answers 200).
  Tom found the /build endpoint suggestion unhelpful — don't re-raise.

## Standing (unchanged)

stash@{0} not ours · seat's untracked dirs: .aion-accepted-binary-20260731,
.aion-jjuabk-snapshot-20260731, .aion-data-preswap-20260804 (KEEP) ·
beamr JIT blocker (Artemis): Benji off hours-long workflows · never bare
cargo clean · no launchd · Waffles receipts culture · Tom-facing text =
plain language, answer-first.

## Comms (IDs from meridian-comms-bridge-state.md memory ONLY)

Tom dm:c9255b2a-5731-4d17-8124-e3bfa2224186 · Waffles
dm:896955e1-86dd-4d6a-9c25-f3f978b189a9 · Athena
dm:45cf420e-621d-467b-b96c-5bffb62c04f3 · Cally
dm:15a23351-009a-4e69-8d63-45602a805f1c.
