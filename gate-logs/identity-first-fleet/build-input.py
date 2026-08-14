#!/usr/bin/env python3
"""Build the fleet_dev dispatch input for the K9 identity-first run.

Usage: python3 build-input.py <worktree-path> > input.json

The one parameter is the PRIMARY worktree (branch identity-first off
manifold main >= 46fdbe1). Each leg additionally gets its OWN isolated
worktree (cut by the workflow's fanout action as a sibling directory,
branch identity-first-<leg>) — Tom's fan-out ruling stands. Everything else
is derived from the dispatcher package (docs repo,
manifold-identity-first-fleet-K9-20260815.md) read at build time, so the
brief stays the single source: re-running this script after a package
amendment rebuilds the input from the amended bytes.

Controls verified true at 46fdbe1 before this script was written:
  - BADGE_MEMBERS is declared at surface/page/src/declared-face.ts:41
  - surface/page/package.json name == "manifold-surface-page"
"""
import json
import os
import sys

BRIEF = "/Users/tom/Developer/ablative/docs/briefs/manifold-identity-first-fleet-K9-20260815.md"
SCRIPTS = "/Users/tom/Developer/ablative/stack/aion/gate-logs/identity-first-fleet"
LEGS = ["L1", "L2"]


def section(text, heading, next_heading):
    start = text.index(heading)
    end = text.index(next_heading)
    return text[start:end].rstrip()


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <worktree-path>")
    repo = sys.argv[1].rstrip("/")
    trees = os.path.dirname(repo)
    text = open(BRIEF).read()

    design = section(text, "## Settled design", "## Hard walls")
    walls = section(text, "## Hard walls", "## Provenance")

    plan_prompt = (
        design
        + "\n\n"
        + walls
        + "\n\nYou are the PLANNER of a fleet run. Your working directory is the"
        " target repository worktree. Write .fleet/plan.md (the whole plan,"
        " every requirement anchored file:line into THIS tree) and one"
        " complete builder brief per leg at .fleet/legs/L1.md and"
        " .fleet/legs/L2.md. BEFORE minting ANY new name — wire verb paths,"
        " refusal names, facet or record kinds, key filenames, topic names —"
        " CENSUS the substrate's existing names by searching the crates/ and"
        " surface/ trees for collisions, and record in .fleet/plan.md what"
        " you searched and what you found: the previous fleet run's brief"
        " minted a facet name the registry's own admission door already"
        " owned, and only a live estate caught it. The legs will run IN"
        " PARALLEL, each builder in its own isolated worktree on its own"
        " branch — no leg sees another leg's work, and the branches are"
        " merged mechanically afterwards. Each brief must therefore be fully"
        " self-contained and independent of the other legs' outputs, and"
        " must name exactly which files its leg owns so no two legs write"
        " the same file; if the settled design forces two legs toward the"
        " same file, say so explicitly in .fleet/plan.md and assign that"
        " file to exactly one leg. Each brief is floors-not-ceilings,"
        " anchored, restates the hard walls above, and names the report file"
        " the builder must write (.fleet/reports/<id>.md). You are planning"
        " CONTENT for this fixed topology, not changing it. Commit nothing;"
        " the .fleet/ files are your only writes. When done, reply with a"
        " short summary of the plan you wrote."
    )

    def leg_prompt(leg):
        return (
            f"You are builder leg {leg} of a fleet run, running IN PARALLEL"
            " with the other legs. Your working directory is your OWN"
            f" isolated worktree on your own branch (identity-first-{leg});"
            " the other legs cannot see your work and you cannot see theirs."
            f" Read .fleet/legs/{leg}.md — that is your complete brief,"
            " written by the run's planner; it is floors, not ceilings. Do"
            " the work it specifies, committing to your own branch as you"
            " go. Stay inside the file zone your brief names — the leg"
            " branches are merged mechanically after all legs finish, and a"
            " clean merge is part of your delivery. Write your full delivery"
            f" report to .fleet/reports/{leg}.md and reply with the same"
            " report verbatim.\n\n"
            + walls
        )

    judge_prompt = (
        "You are the JUDGE of a fleet run. Your working directory is the"
        " target repository worktree: the parallel leg branches, already"
        " merged by the server, built, and mechanically gated green. Read"
        " .fleet/plan.md, every brief under .fleet/legs/, every report under"
        " .fleet/reports/, and the git log and diff of this branch (the"
        " merge commits are the server's). You have exactly two duties, in"
        " order.\n\n"
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
            " permits. Reply as bare plain text: no backticks, no quotes,"
            " no code formatting, no punctuation — the name alone."
        ),
        "counsel_control_expect": "BADGE_MEMBERS",
        "control_prompt": (
            "Read surface/page/package.json in this repository and reply with"
            " ONLY the value of its \"name\" field — the exact string, nothing"
            " else. Reply as bare plain text: no backticks, no quotes, no"
            " code formatting — the value alone."
        ),
        "control_expect": "manifold-surface-page",
        "plan_prompt": plan_prompt,
        "plan_check_script": f"{SCRIPTS}/plan_check.sh",
        "fanout_script": f"{SCRIPTS}/fanout_setup.sh",
        "legs": [
            {
                "id": leg,
                "prompt": leg_prompt(leg),
                "repo": f"{trees}/identity-first-{leg}",
            }
            for leg in LEGS
        ],
        "fanin_script": f"{SCRIPTS}/fanin_merge.sh",
        "judge_prompt": judge_prompt,
        "judge_pass": "fleet-judge-pass",
        "gate_script": f"{SCRIPTS}/gate.sh",
        "report_check_script": f"{SCRIPTS}/report_check.sh",
    }
    json.dump(doc, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
