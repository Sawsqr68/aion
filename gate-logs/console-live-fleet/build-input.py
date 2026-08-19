#!/usr/bin/env python3
"""Build the fleet_dev dispatch input for the CONSOLE-LIVE run.

Usage: /usr/bin/python3 build-input.py <primary-worktree-path> > input.json

Payload: docs/briefs/console-shows-live-work-20260816.md — the ops console
calls four neighbours on the aion router and skips exactly the two that show
live work (`/workflows/describe-live`, `/workflows/children`). Both are built,
mounted, and one already has a working client in the CLI. This is a wiring
brief, and it answers Tom's own ask: "I want to be able to see the work that's
being done."

The brief is read at build time, so it stays the single source: re-running
this script after an amendment rebuilds the input from the amended bytes.

Controls verified true at aion main cf4510f2e before this script was written:
  - apps/aion-ops-console/package.json name == "aion-ops-console"
  - crates/aion-server/ops-console-embed/index.html references
    "assets/index-CMn6AElm.js"
The second control is deliberately the exact file the gate's census leg reads:
a builder that cannot resolve it cannot be trusted with the census either.
"""
import json
import os
import sys

BRIEF = "/Users/tom/Developer/ablative/docs/briefs/console-shows-live-work-20260816.md"
SCRIPTS = "/Users/tom/Developer/ablative/stack/aion/gate-logs/console-live-fleet"
LEGS = ["L1", "L2"]


def section(text, heading, next_heading):
    start = text.index(heading)
    end = text.index(next_heading)
    return text[start:end].rstrip()


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <primary-worktree-path>")
    repo = sys.argv[1].rstrip("/")
    trees = os.path.dirname(repo)
    text = open(BRIEF).read()

    walls = section(text, "## Hard constraints (estate law)", "## Done means")

    plan_prompt = (
        text
        + "\n\n=== YOUR ROLE ===\n\n"
        "You are the PLANNER of a fleet run. Everything above is the settled"
        " brief; you do not re-litigate it. Your working directory is the"
        " target repository worktree (aion). Write .fleet/plan.md — the whole"
        " plan, every requirement anchored file:line INTO THIS TREE, not from"
        " the brief's carried line numbers — and one complete builder brief"
        " per leg at .fleet/legs/L1.md and .fleet/legs/L2.md.\n\n"
        "THE SPLIT IS YOURS TO MAKE, on one condition: the two legs run IN"
        " PARALLEL, each builder in its own isolated worktree on its own"
        " branch, no leg seeing the other's work, and the branches are merged"
        " mechanically afterwards. So each brief must be fully self-contained"
        " and independent of the other leg's output, and must name exactly"
        " which files its leg owns. The natural cut is R1 (children — a run"
        " shows its legs) against R2/R3/R4 (describe-live — a running row says"
        " what it is doing, liveness including NoLiveOwner, capability-gated"
        " controls), but you own that call. If the design forces both legs"
        " toward the same file — the console's API client layer is the"
        " obvious candidate — say so explicitly in .fleet/plan.md and assign"
        " that file to EXACTLY ONE leg, with the other leg consuming what it"
        " exports. A merge conflict is a finding about your leg boundaries.\n\n"
        "GROUNDING DUTIES, specific to this package:\n"
        " * The CLI at crates/aion-cli/src/tail.rs already posts to"
        " /workflows/children with typed request/response. Anchor it and make"
        " the legs REUSE that vocabulary rather than minting a second one.\n"
        " * The console bundle under crates/aion-server/ops-console-embed/ is"
        " GENERATED. Find and anchor how it is regenerated from"
        " apps/aion-ops-console — and note in the plan that this repository"
        " has no CI workflow performing it (.github/workflows is empty), so"
        " each leg must regenerate and commit the embed itself. A hand-edited"
        " asset is an automatic reject.\n"
        " * R5's poll cadence and R3's transcript window are CALLER-CHOSEN"
        " values. The brief forbids silently inventing a default: whichever"
        " leg owns them must state the number and its reason in its report.\n\n"
        "Each brief is floors-not-ceilings, anchored, restates the hard walls"
        " below, and names the report file the builder must write"
        " (.fleet/reports/<id>.md). Commit nothing; the .fleet/ files are your"
        " only writes. When done, reply with a short summary of the plan you"
        " wrote.\n\n" + walls
    )

    def leg_prompt(leg):
        return (
            f"You are builder leg {leg} of a fleet run, running IN PARALLEL"
            " with the other leg. Your working directory is your OWN isolated"
            f" worktree on your own branch (console-live-{leg}); the other leg"
            " cannot see your work and you cannot see theirs."
            f" Read .fleet/legs/{leg}.md — that is your complete brief,"
            " written by the run's planner; it is floors, not ceilings. Do the"
            " work it specifies, committing to your own branch as you go. Stay"
            " inside the file zone your brief names — the leg branches are"
            " merged mechanically after both legs finish, and a clean merge is"
            " part of your delivery.\n\n"
            "TWO THINGS THIS RUN WILL CHECK MECHANICALLY, so build for them:\n"
            " * The server-run gate re-derives the endpoint census against the"
            " REBUILT bundle, resolved from index.html's own reference, with a"
            " canary. If you change console source and do not regenerate the"
            " committed embed under crates/aion-server/ops-console-embed/, the"
            " census reads the OLD bundle and your leg gates red however good"
            " the source is.\n"
            " * A report is checked for existence and non-emptiness in the"
            " merged tree. An eloquent reply that wrote nothing leaves the"
            " tree in its prior state, which can gate green — so that is"
            " caught by a script, not left to a reader.\n\n"
            f"Write your full delivery report to .fleet/reports/{leg}.md and"
            " reply with the same report verbatim. State what is NOT done as"
            " plainly as what is.\n\n" + walls
        )

    judge_prompt = (
        "You are the JUDGE of a fleet run. Your working directory is the"
        " target repository worktree: the parallel leg branches, already"
        " merged by the server, built, and mechanically gated green. Read"
        " .fleet/plan.md, every brief under .fleet/legs/, every report under"
        " .fleet/reports/, and the git log and diff of this branch (the merge"
        " commits are the server's). You have exactly two duties, in order.\n\n"
        "FIRST DUTY — the permitted-choice points: every value the brief left"
        " to the caller (the poll cadence, the transcript window size) and"
        " every \"stated-not-done\" the builders claimed. Verify each is"
        " genuinely permitted by the brief, genuinely stated in the report"
        " with its reason, and genuinely the better call. The brief"
        " explicitly forbids inventing a default and presenting it as the"
        " truth — check that it was declared, not smuggled.\n\n"
        "SECOND DUTY — the hard-wall audit, against this list:\n\n"
        + walls
        + "\n\nOne specific wall to audit at the bytes: the console bundle"
        " under crates/aion-server/ops-console-embed/ is GENERATED. Confirm it"
        " was regenerated by a build and not hand-edited — a hand-edited asset"
        " is an automatic reject.\n\n"
        "You NEVER re-run gates, builds, or tests — the server has already run"
        " them and their exit codes are not yours to second-guess. A judge"
        " re-running them is a colleague pretending to be a mechanism. Write"
        " your full judgement to .fleet/judgement.md. Then: if BOTH duties"
        " pass with nothing to flag, reply with exactly fleet-judge-pass and"
        " nothing else. Otherwise reply with your findings in full — any reply"
        " other than the exact token refuses the delivery."
    )

    doc = {
        "repo": repo,
        "counsel_control_prompt": (
            "Read apps/aion-ops-console/package.json in this repository and"
            " reply with ONLY the value of its \"name\" field — the exact"
            " string, nothing else. Reply as bare plain text: no backticks, no"
            " quotes, no code formatting — the value alone."
        ),
        "counsel_control_expect": "aion-ops-console",
        "control_prompt": (
            "Read crates/aion-server/ops-console-embed/index.html in this"
            " repository. It references exactly one JavaScript asset. Reply"
            " with ONLY that asset's filename — not its path, just the"
            " filename ending in .js. Reply as bare plain text: no backticks,"
            " no quotes, no code formatting — the filename alone."
        ),
        "control_expect": "index-CMn6AElm.js",
        "plan_prompt": plan_prompt,
        "plan_check_script": f"{SCRIPTS}/plan_check.sh",
        "fanout_script": f"{SCRIPTS}/fanout_setup.sh",
        "legs": [
            {
                "id": leg,
                "prompt": leg_prompt(leg),
                "repo": f"{trees}/console-live-{leg}",
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
