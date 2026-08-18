# `fleet-dev` — one brief, planned, built in parallel, merged, judged, and gated

The fleet-shaped sibling of [`norn-dev`](../norn-dev/): the division of labour
the single-lane pilot performed by hand at the dispatcher's desk moves into
the workflow. A planning counsel (Claude over ACP) grounds the brief in the
tree, delegated builders (GPT over norn, queue `dev`) build the legs in
parallel — one isolated worktree per builder, fanned out and merged back by
the server — a judging counsel rules on the delivery, and the server's own
gate script — never any agent's account of itself — admits or refuses the
result.

**Status: reviewed, amended, and re-shaped to fan-out (2026-08-14).** The
document review ratified the values below and added the `check_reports`
step; the operator then ruled the fan-out pattern in before first dispatch
("make sure that good pattern is used in this one"), superseding the
sequential v1 — see the first entry under recorded trade-offs.

## The shape

1. **`counsel_control`**, **`dev_control`** — each agent seam is asked a
   question whose answer the dispatcher already knows, before anything real
   is spent on it. A wrong reply refuses the whole run as `unmeasured`.
2. **`plan_legs`** — the planner receives the whole brief in its prompt and
   writes its deliverables INTO THE TREE: `.fleet/plan.md` plus one complete
   builder brief per leg at `.fleet/legs/<id>.md`, each requirement anchored
   file:line.
3. **`check_plan`** — a server-run script verifies every promised leg brief
   exists and is non-empty. The planner's reply is a claim; the tree is the
   fact. A planner that answered eloquently and wrote nothing routes to
   `unplanned` instead of starving every builder downstream.
4. **`cut_trees`** — the server runs the fanout script: one isolated
   worktree and one branch per leg, cut from the planned tree, each carrying
   the planner's `.fleet/` briefs. It refuses loudly rather than deleting
   anything in its way (a leftover tree from a prior attempt is the
   dispatcher's to inspect). Failure → `unforked`, before any builder is
   spent.
5. **`open` → `build_one` → `close`** — a `distribute` region: all legs in
   parallel, each builder in its OWN worktree on its OWN branch
   (`leg.repo`). Each builder's prompt is an ADDRESS: read your brief at
   `.fleet/legs/<id>.md`, write your report to `.fleet/reports/<id>.md`,
   commit to your own branch, stay inside your brief's file zone. The
   builder session transcript is also committed on that leg branch.
6. **`merge`** — the server runs the fanin script: each leg's report carried
   back to the primary tree, then each leg branch merged in dispatch order.
   A conflict names the conflicting files on stdout, aborts clean, and
   routes `unmerged` — a merge conflict is a finding about the plan's leg
   boundaries, never something an agent resolves in passing. A clean merge
   commits the exact leg heads and merge commits to `.fleet/legs-merged.md`.
7. **`check_reports`** — the builders get the planner's treatment: a
   server-run script verifies every promised report exists and is non-empty
   in the merged tree, naming each missing leg on stdout. A builder that
   answered eloquently and wrote nothing leaves the tree in its prior state —
   which can gate GREEN — so this is caught by a script before the gates,
   never left for the judge as a mechanism of last resort. Missing →
   `unreported`.
8. **`gate`** — the server runs the gate script on the merged tree; the real
   exit code routes green/red. Red refuses the run as `gates_red` before any
   judgement is spent on it; green commits the routed 120-line tail before
   judgement.
9. **`judgement`** — the judge reads the merged tree (plan, briefs, reports,
   diff) under the charter carried in its prompt. Exactly the `judge_pass`
   token routes `capture_sessions`; ANY other reply routes `judge_flagged`
   with the verdict text whole. The mismatch direction is deliberate: a
   false flag is possible, a false clear is not.
10. **`capture_sessions`** — before push and teardown, the server copies and
    commits the counsel control, dev control, planner, and judge transcripts
    into the primary tree. An unreachable transcript becomes a committed
    typed absence rather than a silent skip. The result then routes through
    `push_result` and `clean_up` as before.

## What a landed run carries home

- **`.fleet/sessions/`** — `fleet-sessions.sh` writes one transcript per
  named agent session, or `ABSENT-<step>.md` with the exact lookup evidence.
  Builder transcripts are committed in their leg worktrees and ride through
  fanin; fix-round and primary counsel/control transcripts are committed in
  the primary tree before push.
- **`.fleet/gates/green-<n>.md`** — `fleet-gates.sh` commits the same final
  120 lines returned by the green gate probe. Red rounds retain their whole
  numbered logs and stable `RED-LATEST.md` address.
- **`.fleet/legs-merged.md`** — `fleet-fanin.sh` records each leg id, branch,
  pre-merge head, and resulting merge commit after every leg lands, then
  verifies and commits the record before reporting success.

## Why the tree is the blackboard

AWL composes no strings at runtime — constant concatenation exists
(reference §values), runtime concatenation deliberately does not — so no step
can wrap one leg's reply inside another leg's prompt. Every prompt in this
document is therefore dispatcher-built and static for the run, and every
payload that must flow between legs travels through the working tree that
`cwd from repo` gives all of them. The alternatives were considered and
refused: instructing the planner to end its reply with builder-ready prompt
text makes an agent's obedience a wire format, and new string-composition
vocabulary is a language change, not a workflow's private workaround.

## The judge's charter (carried in `judge_prompt`, priced here)

Two duties, in order:

1. **The permitted-choice points.** Every "stated-not-done" the builders
   claimed under the brief's own permissions — the brief's "say which" spots —
   verified as genuinely permitted, genuinely stated, and genuinely the
   better call.
2. **The hard-wall audit.** The delivery checked against the brief's
   forbidden list (untouchable ports and paths, no out-of-fence changes,
   worktree only, never land/merge/push/deploy).

The judge NEVER re-runs gates: a judge re-running them is a colleague
pretending to be a mechanism. The landing re-gate and the landing itself stay
at the operator's desk, after this workflow returns.

## Recorded trade-offs and ruled values (document review, 2026-08-14)

- **`Reply` is now the four-field record**
  `{text, final_message, stop_reason, session_id}` — #241 replacement semantics. `text` remains the
  whole accumulated output and `final_message` is the last completed answer alone. This source
  change is landed, not deployed: no serving process is restarted or swapped by this document
  update. The fleet v3 judge that routes on `final_message` is a separate dispatcher-authored
  change; this revision changes only the declared record shape.
- **`distribute` with per-leg worktrees — TOM'S RULING (2026-08-14),
  superseding the sequential v1.** The review had ratified `sequence`
  (shared tree, no merge phase to go wrong) with the parallel variant as a
  recorded follow-up; before first dispatch the operator ruled the fan-out
  pattern in. The isolation cost is paid honestly, not waved off: `Leg`
  carries its own `repo`, the server-run `fanout` cuts one worktree + branch
  per leg (refusing loudly at anything in its way), and the server-run
  `fanin` merges the branches in dispatch order, routing conflicts to
  `unmerged` with the files named. The corollary lands on the PLANNER's
  charter: briefs must be independent and name disjoint file zones, because
  a merge conflict is now a plan defect the run can end on. Dependent legs
  are the sequential variant's territory; this document no longer serves
  them.
- **Dev harness `concurrency 3` — dispatch-matched, diverges from
  `norn_dev`'s single lane.** The action seam stays byte-compatible
  (`delegate(repo, prompt) -> Reply`, same queue `dev`), so a seated
  norn_dev worker still serves this document's dispatches — but at one leg
  at a time. True parallelism needs a worker seated FROM THIS DOCUMENT,
  whose harness carries the 3-lane value (one lane per leg of the
  declared-actions dispatch). Both workers can share the queue; the server
  spreads dispatches across them.
- **The planner plans the what-and-where, not the how-many.** The leg
  topology is fixed by the dispatcher in `legs` before the run starts; the
  planner authors the CONTENT of each leg's brief and its file:line
  grounding. That keeps the run list at the dispatcher's desk and is the
  right v1 given the language — but read "planner" as brief-author, not
  decomposer.
- **Queue `dev` is reused byte-compatibly** with `norn_dev`'s declaration —
  RATIFIED: the dev queue is a capability class, not a document namespace;
  same contract, same seated worker, and a second queue would buy isolation
  nobody needs at the price of a seat somebody maintains.
- **One `counsel` queue serves both `plan` and `judge`** (serving is
  all-or-nothing per queue); on 0.15.0 the ACP adapter mints a fresh session
  per attempt, which suits these one-shot legs.
- **Every numeric value is RULED, with provenance** — none are guesses:
  builder `timeout 90m` is the pilot's measured register; `retry 3 backoff
  30s..5m` and the reconnect ladder are the proven norn register (provider
  overload defence); workflow `timeout 12h` held from the sequential v1 and
  now carries even more headroom (parallel legs bound the build phase by the
  slowest leg, not the sum); planner/judge `timeout 30m`; ACP `exit_grace 30s`;
  `permission "allow-once"` is necessary because the counsel seats WRITE
  their deliverables into the tree.
- **The authoring workspace resolves home-side when unconfigured.** With
  `authoring.workspace_dir` deliberately absent, the resolver lands on
  `~/.aion/authoring` on this box — not `./aion-authoring` at the server's
  working directory. Seating this document for the console's Authoring panel
  means copying it there; verify with `GET /awl/documents` and a checksum
  against the committed file rather than hunting the repo root.
- **The ACP `command` is this box's absolute npx path**
  (`/opt/homebrew/bin/npx` + `@agentclientprotocol/claude-agent-acp`); the
  portable `{workspace_root}` form waits on per-box harness-arg binding
  (#210).
- **`judge_pass` is an input**, not a constant: the token the judge is
  instructed to reply with and the token the document matches against must be
  one truth, and both live at the dispatcher's hand.
- **The red-routing `otherwise` arms are UNREACHABLE on this server — RULED
  for the v3 rev (2026-08-14, from the first dispatch's live finding).** A
  non-zero exit of a declared command is a retryable activity ERROR
  (DECLARED-COMMANDS.md:368), so a `RunOutcome` never carries a non-zero
  `exit_code` and every red arm that routes on one
  (`unplanned`/`unforked`/`unmerged`/`unreported`/`gates_red`) cannot fire:
  a red script ends the run terminally at that step instead of routing. Two
  things follow, both ruled. The degradation is FAIL-LOUD, not silent — a
  red gate still kills the run before judge spend and leaves the tree
  inspectable — so v2 stands as dispatched. And the v3 rev makes the whole
  document ONE oracle grammar: declared scripts always exit 0 and emit a
  single stdout token, with every routing decision reading stdout equality —
  exactly the register the controls already use. The exit status is not an
  oracle; the document's own lesson, now applied to its own scripts.

## What this does not prove

A green gate proves what the script measures. A judge's clear is one model's
read of the tree, priced by its charter but unverified — every citation in a
verdict must be read before anything acts on it. The control proves each
pipeline can read the tree and follow an exact instruction; it proves nothing
about the quality of any plan, build, or judgement.
