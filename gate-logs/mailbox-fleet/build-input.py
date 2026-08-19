#!/usr/bin/env python3
"""Build the fleet_dev dispatch input for the MAILBOX RETRY-BUDGET run.

Usage: /usr/bin/python3 build-input.py <primary-worktree-path> > input.json

Payload: docs/briefs/manifold-mailbox-retry-budget.md r1.9 — the retry loop
that spends a rationed resource. Scope is R1 + R2 + R3 ONLY ("The repair —
three requirements"); everything under "The second defect" is superseded by
the brief's own r1.9 header and is NOT in this flight.

The brief is read at build time, so it stays the single source: re-running
this script after an amendment rebuilds the input from the amended bytes.

Controls verified true at manifold main d021100 before this script was
written:
  - crates/manifold-node/Cargo.toml name == "manifold-node"
  - crates/manifold-node/src/mailbox/driver/submit.rs:362 — `pub fn admit`
    lives on `Session` (impl block at :204). NOTE: the brief's prose says
    `MailboxSubmit::admit`; the type was renamed after r1.9. First dispatch
    (eedb4bb9) was REFUSED unmeasured by its own control because this pin
    carried the brief's stale name — the GPT builder read the tree
    correctly and answered `Session`. Pin re-verified at the bytes.
"""
import json
import os
import sys

BRIEF = "/Users/tom/Developer/ablative/docs/briefs/manifold-mailbox-retry-budget.md"
SCRIPTS = "/Users/tom/Developer/ablative/stack/aion/gate-logs/mailbox-fleet"
LEGS = ["L1", "L2"]

WALLS = (
    "=== HARD WALLS (estate law — violating any one is an automatic reject) ===\n"
    "* Worktree only: all work in your own worktree on your own branch. NEVER"
    " push, never merge, never land, never deploy, never touch any checkout"
    " outside your worktree.\n"
    "* The LIVE ESTATE is untouchable: never start, stop, or signal any"
    " running process; never touch ports 4176, 41100-41102, or 9490-9499;"
    " never read or write any `.manifold*` path outside your own worktree"
    " (test fixtures your tests create inside their own temp dirs are fine);"
    " never install anything; nothing you build may become a daemon.\n"
    "* No secrets: no key material, tokens, or unlock phrases in code, logs,"
    " error text, or serialized state.\n"
    "* No suppressions: #[allow(...)], #[expect(...)], #[ignore], _var"
    " renames to silence unused warnings, and cfg-gating dead code are all"
    " bypasses, not fixes. If a lint fires, fix the code.\n"
    "* Red-first: the failing test lands as its OWN COMMIT, its message"
    " containing the word 'red', BEFORE the fix commit. The gate checks this"
    " mechanically.\n"
    "* R2 and R3 are ONE leg and land TOGETHER — the brief's own arithmetic"
    " makes R3 the precondition of R2 (R2 alone raises the retry interval"
    " and silently degrades the only detector that caught this incident)."
    " Never R2 alone, under any decomposition.\n"
    "* Stay inside the file zone your brief names. The leg branches merge"
    " mechanically afterwards: a merge conflict is a plan defect, and an"
    " out-of-zone edit is a reject.\n"
    "* Commit to your own branch as you go; write the report file your brief"
    " names. State what is NOT done as plainly as what is."
)


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <primary-worktree-path>")
    repo = sys.argv[1].rstrip("/")
    trees = os.path.dirname(repo)
    text = open(BRIEF).read()

    plan_prompt = (
        text
        + "\n\n=== YOUR ROLE ===\n\n"
        "You are the PLANNER of a fleet run. Everything above is the settled"
        " brief; you do not re-litigate it. Your working directory is the"
        " target repository worktree (manifold, cut from main d021100).\n\n"
        "SCOPE, EXACTLY: R1, R2, R3 — the section titled 'The repair — three"
        " requirements'. Everything under 'The second defect' is SUPERSEDED"
        " by the brief's own r1.9 header and is NOT in this flight; do not"
        " plan any of it.\n\n"
        "Write .fleet/plan.md — the whole plan, every requirement anchored"
        " file:line INTO THIS TREE (re-derive the anchors; the brief's carried"
        " line numbers are from 2026-08-05 and the tree has moved) — and one"
        " complete builder brief per leg at .fleet/legs/L1.md and"
        " .fleet/legs/L2.md.\n\n"
        "THE SPLIT IS YOURS TO MAKE, on two conditions: the legs run IN"
        " PARALLEL in isolated worktrees and merge mechanically afterwards,"
        " so each brief must be self-contained, independent of the other"
        " leg's output, and must name exactly which files its leg owns; and"
        " R2+R3 stay in ONE leg (the wall below). The natural cut is L1 = R1"
        " (the kernel publishes its shutdown report before tearing down the"
        " spine — crates/manifold-node/src/kernel/ zone) against L2 = R2+R3"
        " (the backoff hoisted to the admit loop WITH the attempt budget —"
        " crates/manifold-node/src/mailbox/ zone; the brief's"
        " `MailboxSubmit::admit` is TODAY `Session::admit` at"
        " src/mailbox/driver/submit.rs:362, impl at :204 — the type was"
        " renamed after the brief's r1.9, one more reason your anchors must"
        " be re-derived from THIS tree). You own the call, but if"
        " any file is wanted by both legs, assign it to EXACTLY ONE and say"
        " so in the plan.\n\n"
        "GROUNDING DUTIES, specific to this package:\n"
        " * Anchor the admit/admit_once loop and the reconnect call whose"
        " backoff resets — quote the exact lines the brief's 'Root cause'"
        " section describes, as they stand in THIS tree.\n"
        " * Anchor where the kernel stages its shutdown report and where"
        " teardown begins — R1 is an ORDERING fix; the plan must show the"
        " two sites whose order is wrong.\n"
        " * Each acceptance test in the brief (staging file absent after a"
        " clean stop; delays advancing toward MAX_BACKOFF across iterations;"
        " exhaustion message naming scope, limit, occupancy, refill window)"
        " must appear in exactly one leg's brief, with its red-first step"
        " stated.\n"
        " * The gate runs `cargo test -p manifold-node -- --test-threads=2`"
        " — bounded threads because the mailbox suite has a known"
        " contention-sensitive flake at default parallelism. Tests the legs"
        " add must pass under that flag.\n\n"
        "Each brief is floors-not-ceilings, anchored, restates the hard walls"
        " below, and names the report file the builder must write"
        " (.fleet/reports/<id>.md). Commit nothing; the .fleet/ files are"
        " your only writes. When done, reply with a short summary of the plan"
        " you wrote.\n\n" + WALLS
    )

    def leg_prompt(leg):
        return (
            f"You are builder leg {leg} of a fleet run, running IN PARALLEL"
            " with the other leg. Your working directory is your OWN isolated"
            f" worktree on your own branch (mailbox-{leg}); the other leg"
            " cannot see your work and you cannot see theirs."
            f" Read .fleet/legs/{leg}.md — that is your complete brief,"
            " written by the run's planner; it is floors, not ceilings. Do"
            " the work it specifies, committing to your own branch as you go"
            " (red test first, as its own commit with 'red' in the message)."
            " Stay inside the file zone your brief names.\n\n"
            "THINGS THIS RUN CHECKS MECHANICALLY, so build for them:\n"
            " * The server-run gate executes, on the MERGED tree:"
            " `cargo fmt -p manifold-node -- --check`,"
            " `cargo clippy -p manifold-node --all-targets -- -D warnings`,"
            " and `cargo test -p manifold-node -- --test-threads=2`."
            " Your work must be green under all three — clippy pedantic"
            " included, no suppressions.\n"
            " * A commit with 'red' in its message must exist on your branch"
            " before your fix commit.\n"
            " * Your report at .fleet/reports/" + leg + ".md is checked for"
            " existence and non-emptiness in the merged tree.\n\n"
            f"Write your full delivery report to .fleet/reports/{leg}.md and"
            " reply with the same report verbatim. State what is NOT done as"
            " plainly as what is.\n\n" + WALLS
        )

    judge_prompt = (
        "You are the JUDGE of a fleet run. Your working directory is the"
        " target repository worktree: the parallel leg branches, already"
        " merged by the server and mechanically gated green"
        " (fmt/clippy/test/red-first). Read .fleet/plan.md, every brief under"
        " .fleet/legs/, every report under .fleet/reports/, and the git log"
        " and diff of this branch against main (the merge commits are the"
        " server's). You have exactly two duties, in order.\n\n"
        "FIRST DUTY — the permitted-choice points: every value or shape the"
        " brief left open (WHERE the backoff lives — hoisted to admit or"
        " threaded into reconnect, both permitted; the budget's derivation"
        " from attach capacity; the exact wording of the exhaustion message)"
        " and every 'stated-not-done' the builders claimed. Verify each is"
        " genuinely permitted by the brief, genuinely stated in a report with"
        " its reason, and genuinely the better call.\n\n"
        "SECOND DUTY — the hard-wall audit, against this list:\n\n" + WALLS
        + "\n\nThree specific walls to audit at the bytes: (1) R2 and R3"
        " landed TOGETHER — if the diff moves the backoff without the attempt"
        " budget, reject, whatever the reports say; (2) the R3 exhaustion"
        " path fails LOUDLY with the record left staged — a silent continue"
        " or a swallowed error is the defect class this brief exists to kill;"
        " (3) no leg touched files outside its declared zone.\n\n"
        "You NEVER re-run gates, builds, or tests — the server has already"
        " run them and their exit codes are not yours to second-guess. Write"
        " your full judgement to .fleet/judgement.md. Then: if BOTH duties"
        " pass with nothing to flag, reply with exactly fleet-judge-pass and"
        " nothing else. Otherwise reply with your findings in full — any"
        " reply other than the exact token refuses the delivery."
    )

    doc = {
        "repo": repo,
        "counsel_control_prompt": (
            "Read crates/manifold-node/Cargo.toml in this repository and"
            " reply with ONLY the value of its \"name\" field under"
            " [package] — the exact string, nothing else. Reply as bare"
            " plain text: no backticks, no quotes, no code formatting — the"
            " value alone."
        ),
        "counsel_control_expect": "manifold-node",
        "control_prompt": (
            "Open crates/manifold-node/src/mailbox/driver/submit.rs in this"
            " repository and find the public function named admit. Reply with"
            " ONLY the name of the struct whose impl block contains it — the"
            " bare type name, nothing else. Reply as bare plain text: no"
            " backticks, no quotes, no code formatting — the name alone."
        ),
        "control_expect": "Session",
        "plan_prompt": plan_prompt,
        "plan_check_script": f"{SCRIPTS}/plan_check.sh",
        "fanout_script": f"{SCRIPTS}/fanout_setup.sh",
        "legs": [
            {
                "id": leg,
                "prompt": leg_prompt(leg),
                "repo": f"{trees}/mailbox-{leg}",
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
