#!/usr/bin/env python3
"""Build the norn_dev dispatch input for the #207 delivery-verdict re-judge.

Usage: build-input.py <worktree-path> > input.json

The one parameter is the worktree (branch delivery-rejudge off manifold
main >= 46fdbe1). Everything else derives from the dispatcher package
(docs repo, manifold-delivery-verdict-rejudge-20260815.md) read at build
time — re-running after a package amendment rebuilds from the amended
bytes. Single dev run on the norn_dev document (repo travels with the
dispatch, `cwd from repo`).

Control verified true at 46fdbe1 (K8/K9 precedent, re-verified for K9):
  surface/page/package.json name == "manifold-surface-page"
"""
import json
import sys

BRIEF = "/Users/tom/Developer/ablative/docs/briefs/manifold-delivery-verdict-rejudge-20260815.md"
SCRIPTS = "/Users/tom/Developer/ablative/stack/aion/gate-logs/delivery-rejudge"


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <worktree-path>")
    repo = sys.argv[1].rstrip("/")
    package = open(BRIEF).read().rstrip()

    brief = (
        package
        + "\n\n---\n\n"
        "You are the BUILDER of this fix, working alone. The package above"
        " is your complete brief: Hermes's contract anchors are the ruled"
        " ground, the judge-site census names your exact targets, the"
        " requirements R1-R5 are floors, and the hard walls are absolute."
        " Your working directory is an isolated worktree on branch"
        " delivery-rejudge; commit to it as you go, never push, never merge,"
        " never touch the live estate or any .manifold* path.\n\n"
        "Order of work: R5's RED comes FIRST — write the test that enrolls"
        " a signer AFTER a long-lived seat has attached, delivers a message"
        " from that signer, and pins the current wrong verdict; run it, and"
        " COMMIT IT FAILING with the red output quoted in the commit"
        " message. Then build R1-R4 until that same test turns green. The"
        " R5 test and the two-process re-judge proof must be TESTS THE"
        " CARGO BATTERY RUNS (spawn the second process from the test if the"
        " proof needs one) — the run's gate is `cargo test --workspace` on"
        " this worktree and it is the only mechanical measure of your work,"
        " so a proof living outside the battery proves nothing here. The"
        " dispatcher's desk re-runs the battery and performs the live"
        " two-process walk on a scratch estate before landing.\n\n"
        "The other pre_record sites (history, replay, session, manifest,"
        " scheduler) are named, disclosed non-goals unless the same seam"
        " lifts them for free — if you take one, say which and why; if you"
        " leave them, say so explicitly. No lint bypasses, no test skips,"
        " no #[allow]/#[ignore]; every touched file stays <=500 LOC or is"
        " split. Reply with your full delivery report: what changed"
        " file-by-file, the R5 red-then-green evidence (commit ids), every"
        " permitted choice you made and why, and every disclosed non-goal."
    )

    doc = {
        "repo": repo,
        "control_prompt": (
            "Read surface/page/package.json in this repository and reply with"
            " ONLY the value of its \"name\" field — the exact string, nothing"
            " else. Reply as bare plain text: no backticks, no quotes, no"
            " code formatting — the value alone."
        ),
        "control_expect": "manifold-surface-page",
        "brief": brief,
        "gate_script": f"{SCRIPTS}/gate.sh",
    }
    json.dump(doc, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
