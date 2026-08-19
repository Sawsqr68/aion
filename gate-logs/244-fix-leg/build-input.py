#!/usr/bin/env python3
"""Assemble the norn_dev input for the #244 fix leg.

The brief is the Opus landing review verbatim (read from the worktree at
build time — never a paraphrase) wrapped with the builder charter. The
control is the workspace version, verified at the worktree's own bytes
before this script is trusted (0.15.0 at b0214c0ef, checked 2026-08-15
18:1xZ at Vesper's hand).
"""
import json
import sys

WT = "/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility"
REVIEW = f"{WT}/gate-logs/244-child-visibility/OPUS-REVIEW-20260816.md"
GATE = "/Users/tom/Developer/ablative/stack/aion/gate-logs/244-fix-leg/gate.sh"

with open(REVIEW, encoding="utf-8") as fh:
    review = fh.read()

if "VERDICT: DO-NOT-LAND" not in review or "### B6" not in review:
    sys.exit("review artifact does not carry the expected verdict sections")

charter = """
---

You are the BUILDER for the #244 fix leg, working alone in the worktree
named above, on its existing branch `244-child-visibility` (tip
b0214c0ef). The Opus landing review above is your complete charter: the
verdict is DO-NOT-LAND and every finding is mandatory — all six BLOCKERS
(B1-B6), all eight IMPORTANT items (I1-I8), and every LOWER SEVERITY
item including the `mod.rs` doc omission. Nothing is optional, nothing
is deferred, per house law.

Ground rules:

- Work only in that worktree. Commit as you go; never push, never
  merge, never rebase or rewrite existing history — the reviewed range
  051b547a6..b0214c0ef is append-only ground.
- Never touch the live estate: no server or worker process, no
  ~/.aion, no other worktree.
- Red-first for every blocker that is pinnable by a test (B1's stray
  stdout `null`, B2's error-path rendering, B3's pending-child
  liveness, B4's attach-window loss, B5's truncation message, B6's
  unknown-run 200): write the failing test, run it, commit it red with
  the failure output quoted in the commit body, then fix. Where the
  reviewer supplies a fix shape (B1's print_json exemption precedent,
  B4's subscribe_workflow_from + run-scoped continued-as-new guard),
  follow it or better it with your reasons in the delivery report.
- The Scope-discipline and Commit-chain sections are addressed with
  NEW commits whose bodies carry the missing justifications (the test
  extraction rationale; the ebb0d6b70 red-pin relocation note) — never
  by history surgery. If you judge an extraction unjustifiable, say so
  in the delivery report rather than reverting reviewed ground.
- House law on the mpsc 256 constant: no invented defaults — derive
  it, make it configuration with no default, or justify the chosen
  bound in a doc comment that names its reasoning; state which in the
  report.
- No lint bypasses, no #[allow], no #[expect], no #[ignore], no test
  skips. cargo clippy --workspace --all-targets -- -D warnings AND the
  per-crate resolution must both pass; fmt clean.
- The server-run gate that follows your leg runs: JS deps, fmt, both
  clippy resolutions, aion-cli/aion-server/aion-client suites, console
  check/typecheck/test, then the full standing battery. Leave the tree
  in a state where every leg passes.

Reply with a full delivery report: finding-by-finding disposition
(B1-B6, I1-I8, each lower item — what changed, where, commit id);
red-then-green evidence with commit ids for every pinned blocker; every
permitted choice made and why; every disclosed non-goal; and only
verification actually performed.
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
