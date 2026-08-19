#!/usr/bin/env python3
"""Build the norn_dev dispatch input for the GATE-SPEAKS run (aion half).

Usage: /usr/bin/python3 build-input.py > input.json

Payload: docs/briefs/gate-failure-speaks-20260816.md plus a dispatch addendum
correcting the brief's population numbers (its 32 was contaminated by worktree
duplicate copies — the find never excluded .worktrees/) and scoping this run to
the aion repository. The brief is read at build time so it stays the single
source.

Controls verified true in the delivery worktree at ab6284f7e before this
script was written:
  - apps/aion-ops-console/package.json name == "aion-ops-console"
  - gate-logs/console-live-fleet/gate.sh present (8,885 bytes, the 9a89091
    reference implementation, contains the GATE RED failure summary)
Red-first proof of the gate: on the untouched worktree it exits 1 and names
census+report on stderr; census log reads population=17, SPEAKS RED x16.
"""
import json
import sys

BRIEF = "/Users/tom/Developer/ablative/docs/briefs/gate-failure-speaks-20260816.md"
WT = "/Users/tom/Developer/ablative/stack/aion/.worktrees/gate-speaks"

ADDENDUM = """

=== DISPATCH ADDENDUM (Waffles, 2026-08-17, supersedes the brief's numbers) ===

POPULATION CORRECTION. The brief's census of 32 was contaminated: its find
never excluded .worktrees/, so worktree duplicate copies were counted. The
true population in THIS repository at your worktree's commit (ab6284f7e) is
17 tracked gate*.sh files — re-derive it yourself inside your worktree and
report your number. My measurement of the defect at dispatch: 17 files, all
parse under zsh -n, 16 contain no stderr write at all, 1 already complies.
The apps/manifold specimen the brief's mechanism section reads is 1 further
file in a DIFFERENT repository — it is OUT of this run's scope and will be
fixed in its own run; your disposition table covers this repository only.

SCOPE OF THIS RUN. Your working directory is an aion worktree on branch
gate-speaks. Fix every gate*.sh in the tree per R1+R2, adapting per file
(R3 — no blind seds). Commit to your branch as you go. Commit nothing outside
the gate scripts and your report.

REFERENCE IMPLEMENTATION, IN YOUR TREE. gate-logs/console-live-fleet/gate.sh
is the 9a89091 shape authored from birth: named accumulator
(LEG_RESULTS+=("$name=$code")), failure summary on stderr naming every red
leg + its code + its log path, silent success. Read it before writing. Note
its subshell-not-command-group comment: under zsh, `{ ...; exit 1; } > log`
runs in the CURRENT shell and an exit inside kills the whole script silently
— the exact defect this brief eliminates. Do not introduce it while fixing.

R4 RULING (dispatcher's). The server-path mechanism (gate stderr →
failure_reason) is already FIELD-PROVEN by run 21b9ea9a on 2026-08-16:
before-text "the declared command `zsh` exited 1 with no standard error
output", after-text "GATE RED — 3 of 6 legs failed: ..." recorded in the
server's own failure_reason. You do not need to re-prove the server path.
Your R4 duty is per-file honesty: for ONE representative script you modified,
run it in a terminal with one leg forced red and capture stdout and stderr
SEPARATELY (> out 2> err), showing the summary lands on fd 2 and names the
leg. Include the capture verbatim in your report.

R5 UNCHANGED. For that same script also show: a green run stays exit 0 with
silent stderr, and the forced-red run's exit code is what it was before your
change.

REPORT. Write gate-logs/gate-speaks-run/report.md in your worktree (the gate
checks it exists and is non-empty): your re-derived population count with its
instrument, the per-file disposition table (every file: fixed / already
compliant / shape-does-not-apply with why), the R4 capture, the R5
demonstration, and your absence-claim search per the brief's REQUIRED
sections. Reply with the same report verbatim.
"""


def main():
    text = open(BRIEF).read()
    doc = {
        "repo": WT,
        "control_prompt": (
            "Read apps/aion-ops-console/package.json in this repository and"
            " reply with ONLY the value of its \"name\" field — the exact"
            " string, nothing else. Reply as bare plain text: no backticks,"
            " no quotes, no code formatting — the value alone."
        ),
        "control_expect": "aion-ops-console",
        "brief": text + ADDENDUM,
        "gate_script": "/Users/tom/Developer/ablative/stack/aion/gate-logs/gate-speaks/gate.sh",
    }
    json.dump(doc, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
