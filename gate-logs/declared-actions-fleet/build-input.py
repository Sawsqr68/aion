#!/usr/bin/env python3
"""Build the fleet_dev dispatch input for the declared-actions run.

Usage: python3 build-input.py <worktree-path> > input.json

The one parameter is the worktree Waffles cuts (branch declared-actions off
manifold main >= e58973d). Everything else is derived from the dispatcher
package (docs repo, manifold-click-actions-fleet-20260814.md) read at build
time, so the brief stays the single source: re-running this script after a
package amendment rebuilds the input from the amended bytes.

Controls verified true at e58973d before this script was written:
  - BADGE_MEMBERS is declared at surface/page/src/declared-face.ts:41
  - surface/page/package.json name == "manifold-surface-page"
"""
import json
import sys

BRIEF = "/Users/tom/Developer/ablative/docs/briefs/manifold-click-actions-fleet-20260814.md"
SCRIPTS = "/Users/tom/Developer/ablative/stack/aion/gate-logs/declared-actions-fleet"
LEGS = ["L1", "L2", "L3"]


def section(text, heading, next_heading):
    start = text.index(heading)
    end = text.index(next_heading)
    return text[start:end].rstrip()


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <worktree-path>")
    repo = sys.argv[1]
    text = open(BRIEF).read()

    design = section(text, "## Settled design", "## Prompt content")
    walls = section(text, "## Hard walls", "## Scripts")

    plan_prompt = (
        design
        + "\n\n"
        + walls
        + "\n\nYou are the PLANNER of a fleet run. Your working directory is the"
        " target repository worktree. Write .fleet/plan.md (the whole plan,"
        " every requirement anchored file:line into THIS tree) and one"
        " complete builder brief per leg at .fleet/legs/L1.md, .fleet/legs/L2.md,"
        " .fleet/legs/L3.md. Each brief is floors-not-ceilings, self-contained,"
        " anchored, restates the hard walls above, and names the report file"
        " the builder must write (.fleet/reports/<id>.md). You are planning"
        " CONTENT for this fixed topology, not changing it. Commit nothing;"
        " the .fleet/ files are your only writes. When done, reply with a"
        " short summary of the plan you wrote."
    )

    def leg_prompt(leg):
        return (
            f"You are builder leg {leg} of a fleet run. Your working directory"
            f" is the target repository worktree. Read .fleet/legs/{leg}.md —"
            " that is your complete brief, written by the run's planner; it is"
            " floors, not ceilings. Do the work it specifies, committing as it"
            f" directs. Write your full delivery report to .fleet/reports/{leg}.md"
            " and reply with the same report verbatim.\n\n"
            + walls
        )

    judge_prompt = (
        "You are the JUDGE of a fleet run. Your working directory is the"
        " target repository worktree, already built and mechanically gated"
        " green. Read .fleet/plan.md, every brief under .fleet/legs/, every"
        " report under .fleet/reports/, and the git log and diff of this"
        " branch. You have exactly two duties, in order.\n\n"
        "FIRST DUTY — the permitted-choice points: every \"stated-not-done\""
        " the builders claimed under the brief's own permissions (the brief's"
        " \"say which\" spots). Verify each one is genuinely permitted by the"
        " brief, genuinely stated in the report, and genuinely the better"
        " call.\n\n"
        "SECOND DUTY — the hard-wall audit, against this list:\n\n"
        + walls
        + "\n\nYou NEVER re-run gates, builds, or tests — the server has"
        " already run them and their exit codes are not yours to second-guess."
        " Write your full judgement to .fleet/judgement.md. Then: if BOTH"
        " duties pass with nothing to flag, reply with exactly"
        " fleet-judge-pass and nothing else. Otherwise reply with your"
        " findings in full — any reply other than the exact token refuses"
        " the delivery."
    )

    doc = {
        "repo": repo,
        "counsel_control_prompt": (
            "Read surface/page/src/declared-face.ts and reply with ONLY the"
            " name of the const that lists the members a badge element"
            " permits."
        ),
        "counsel_control_expect": "BADGE_MEMBERS",
        "control_prompt": (
            "Read surface/page/package.json in this repository and reply with"
            " ONLY the value of its \"name\" field — the exact string, nothing"
            " else."
        ),
        "control_expect": "manifold-surface-page",
        "plan_prompt": plan_prompt,
        "plan_check_script": f"{SCRIPTS}/plan_check.sh",
        "legs": [{"id": leg, "prompt": leg_prompt(leg)} for leg in LEGS],
        "judge_prompt": judge_prompt,
        "judge_pass": "fleet-judge-pass",
        "gate_script": f"{SCRIPTS}/gate.sh",
        "report_check_script": f"{SCRIPTS}/report_check.sh",
    }
    json.dump(doc, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
