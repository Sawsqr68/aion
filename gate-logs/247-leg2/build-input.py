#!/usr/bin/env python3
"""Assemble the norn_dev input for the #247 leg 2 (rebase + four gaps).

The brief is the ORIGINAL ratified brief verbatim plus the leg-1 builder's
own delivery report verbatim (its "Not ready to land" self-audit is the
charter's backbone), wrapped with the leg-2 charter. Control is the
workspace version at the worktree's own bytes (0.15.0, unchanged through
e8c1b28bf).
"""
import json
import sys

WT = "/Users/tom/Developer/ablative/stack/aion/.worktrees/247-transcript-spool"
BRIEF = "/Users/tom/Developer/ablative/stack/aion/gate-logs/247-transcript-spool/BRIEF.md"
REPORT = "/Users/tom/Developer/ablative/stack/aion/gate-logs/247-transcript-spool/DEV-DELIVERY-REPORT.md"
GATE = "/Users/tom/Developer/ablative/stack/aion/gate-logs/247-leg2/gate.sh"

with open(BRIEF, encoding="utf-8") as fh:
    brief = fh.read()
with open(REPORT, encoding="utf-8") as fh:
    report = fh.read()

if "Ratified requirements not satisfied" not in report:
    sys.exit("delivery report does not carry the four-gap section")

charter = """
---

You are the BUILDER for #247 LEG 2, working alone in the worktree named
above, on its existing branch `247-transcript-spool`. Above you have (a)
the original ratified brief and (b) the leg-1 delivery report, whose
"Ratified requirements not satisfied" section names exactly four gaps.
Your charter has two phases, in order:

PHASE 1 — REBASE. The branch holds three commits (29863a6ca red,
6fb7d909a spool implementation, ca9360f2f collision-safe dedup) on the
stale base 7109c1f6d. Rebase them onto current aion main `e8c1b28bf`
(`git rebase --onto e8c1b28bf 7109c1f6d 247-transcript-spool` or
equivalent). This rebase is EXPLICITLY AUTHORIZED for these three
commits only: they were never pushed and never reviewed at their current
shas, and the terminal adjudication ruled rebase as the cure for the two
base-inherited gate reds (fleet_dev.awl `agent` Reply-shape corpus red;
SDK-closure freshness red) — both already fixed on main. Resolve
conflicts preserving the slice's semantics; run the touched-crate suite
after the rebase and record the result before starting phase 2.

PHASE 2 — discharge all four gaps from the report, red-first where
pinnable, each as its own commit(s):

1. Server-authored worker-death gap: wire the server-side worker-death
   detector to a transcript-gap publisher recording dispatch identity,
   last accepted seq, and the death window ("stream X: last accepted
   event seq S at T1, worker incarnation died ~T2, events between are
   unrecoverable").
2. The chartered live outage e2e: a live streaming command, a real
   server kill + restart, worker re-registration observed, the #246
   outage/recovery rig shared (the outage-recovery e2e infrastructure in
   crates/aion-server tests), adopted-survivor replay proven under a
   re-armed generation token. The FakeSink unit fixtures stay; they are
   not the proof.
3. Malformed-entry recording must not depend on a later live event: when
   spool.front() cannot decode and the producer channel is closed, a
   typed stream event still records the loss.
4. The disk-exhaustion diagnostic must not be losable while
   disconnected: record_spool_failure's diagnostic is retained/retried
   until publishable, never discarded on a failed DrainStep.

Ground rules:

- Work only in that worktree. Commit as you go; never push, never
  merge. After the phase-1 rebase, history is append-only again.
- Never touch the live estate: no server or worker process, no ~/.aion,
  no other worktree.
- No lint bypasses of any kind. cargo clippy --workspace --all-targets
  -- -D warnings AND the per-crate resolution must both pass; fmt clean.
- Commit bodies carry REAL newlines (write them from a file or heredoc).
- No invented defaults/caps (the ratified non-goals in the brief stand).
- The server-run gate that follows: js-deps ×2, fmt, both clippy
  resolutions, aion-worker suite, aion-server suite, full standing
  battery. Leave the tree so every leg passes on a truthful base.

Reply with a full delivery report: the rebase record (commands, conflict
list, post-rebase suite result, new shas); gap-by-gap disposition with
red-then-green evidence and commit ids; every choice made and why; every
disclosed non-goal; and only verification actually performed.
"""

payload = {
    "repo": WT,
    "control_prompt": (
        "Read the root Cargo.toml in this repository and reply with ONLY "
        "the value of workspace.package.version — the exact string, "
        "nothing else. Reply as bare plain text: no backticks, no quotes, "
        "no code formatting — the value alone."
    ),
    "control_expect": "0.15.0",
    "brief": brief + "\n\n---\n\n# Leg-1 delivery report (verbatim)\n\n" + report + charter,
    "gate_script": GATE,
}

json.dump(payload, sys.stdout, indent=2)
