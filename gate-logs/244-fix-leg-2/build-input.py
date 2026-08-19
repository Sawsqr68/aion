#!/usr/bin/env python3
"""Assemble the norn_dev input for the #244 fix leg 2.

The brief is the Opus pass-2 ruling verbatim (read from the file Waffles'
reviewer wrote — never a paraphrase) wrapped with the builder charter. The
control is the workspace version, verified at the worktree's own bytes
before this script is trusted (0.15.0 at 0328e67a2, checked 2026-08-16 at
Vesper's hand). Both load-bearing findings (NEW-B1's ungated fall-through
in monitor_source; NEW-2's silent empty-backfill path) were independently
re-verified at the bytes by Vesper before dispatch.
"""
import json
import sys

WT = "/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility"
REVIEW = "/Users/tom/Developer/ablative/stack/aion/gate-logs/244-fix-leg/OPUS-REVIEW-2-20260816.md"
GATE = "/Users/tom/Developer/ablative/stack/aion/gate-logs/244-fix-leg-2/gate.sh"

with open(REVIEW, encoding="utf-8") as fh:
    review = fh.read()

if "VERDICT: DO-NOT-LAND" not in review or "NEW-B1" not in review or "NEW-2" not in review:
    sys.exit("review artifact does not carry the expected verdict sections")

charter = """
---

You are the BUILDER for the #244 fix leg 2, working alone in the worktree
named above, on its existing branch `244-child-visibility` (tip
0328e67a2). The Opus pass-2 ruling above is your complete charter. The
verdict is DO-NOT-LAND on exactly two findings; both are mandatory,
nothing else in the ruling asks for code:

1. NEW-B1 (BLOCKER): in `crates/aion-cli/src/tail/live.rs`
   `monitor_source`, gate parent child-discovery AND `observe_attempt` on
   `selected_run_seen` so that events replayed from generations before the
   selected run's `WorkflowStarted` are not attributed to the selected
   run. History is seq-ordered, so everything before the selected run's
   own `WorkflowStarted` is prior-generation by construction. Red-pin
   with a two-generation fixture (the server-side children tests show how
   to build one; the tail e2e fixtures show the socket side), committed
   red-first with the failure output quoted in the commit body, then fix.
   The pin must fail on the current tip's behavior: a prior-generation
   `ChildWorkflowStarted` must NOT be announced/followed and a
   prior-generation `ActivityStarted` must NOT print "superseded" or
   poison the current-attempt map (a genuine later attempt of the
   selected run must still be announced after replay).

2. NEW-2 (must fix with NEW-B1): in `crates/aion-cli/src/tail.rs`
   `backfill_notice`, an empty `events` with `head_seq > 0` must emit a
   truthful notice (retained records trimmed; head at durable seq N)
   instead of returning None. Pin it in `backfill_notice`'s own tests,
   red-first.

Ground rules (unchanged from leg 1):

- Work only in that worktree. Commit as you go; never push, never merge,
  never rebase or rewrite existing history — everything through
  0328e67a2 is append-only ground.
- Never touch the live estate: no server or worker process, no ~/.aion,
  no other worktree.
- No lint bypasses, no #[allow], no #[expect], no #[ignore], no test
  skips, no suppressions of any kind. cargo clippy --workspace
  --all-targets -- -D warnings AND cargo clippy -p aion-cli must both
  pass; fmt clean.
- Commit bodies carry REAL newlines — the pass-2 ruling notes leg 1's
  bodies contained literal \\n\\n escape sequences (a tooling artifact);
  do not repeat it. Write the commit message from a file or heredoc so
  the body renders as actual paragraphs.
- The server-run gate that follows your leg runs: fmt, clippy workspace,
  clippy -p aion-cli, the full aion-cli suite, and the tail e2e. Leave
  the tree in a state where every leg passes.

Reply with a full delivery report: disposition of NEW-B1 and NEW-2 (what
changed, where, commit ids); red-then-green evidence with commit ids and
the quoted red output for both pins; any choice you made inside the
specified fix shape and why; and only verification actually performed.
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
    "brief": review + charter,
    "gate_script": GATE,
}

json.dump(payload, sys.stdout, indent=2)
