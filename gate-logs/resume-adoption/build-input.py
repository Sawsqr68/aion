#!/usr/bin/python3
"""Build the five-field norn_dev input for the resume-adoption lane."""

import json
import sys
from pathlib import Path

BRIEF = Path(
    "/Users/tom/Developer/ablative/stack/aion/gate-logs/resume-adoption/BRIEF-DRAFT.md"
)
GATE = "/Users/tom/Developer/ablative/stack/aion/gate-logs/resume-adoption/gate.sh"


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <worktree-path>")

    repo = sys.argv[1].rstrip("/")
    package = BRIEF.read_bytes().decode("utf-8").rstrip()
    brief = (
        package
        + "\n\n---\n\n"
        "You are the BUILDER for the resume-adoption lane, working alone in "
        "the workflow repo named above (a worktree at the v0.17.0 release "
        "commit). The package above is your complete approved charter: a fix "
        "build with SEVEN acceptance legs, all listed in the Acceptance "
        "sketch, plus two riders (exhaustion-must-name-exhaustion; the "
        "false-error resume exit code). Both remedies land: B (recovery "
        "adoption covers remote-agent activities for Running AND Paused "
        "workflows) and A (spawn-gate adoption on self-occupancy), with the "
        "gate as backstop. The foreign-sibling refusal must remain — its "
        "control leg keeps #246's red red.\n\n"
        "Work only in the named worktree; commit as you go; never push and "
        "never merge. Isolation walls are absolute: never touch the live "
        "estate server, ~/.aion, aion-data/, any live worker, or the sibling "
        "worktree .worktrees/libsql-retirement (a live agent occupies it). "
        "All e2e legs run on your own isolated server instance with its own "
        "AION_HOME inside the worktree, on ports you allocate and record. "
        "Follow red-first ordering exactly: every demanded committed red "
        "written, run, and committed failing before its fix, with the red "
        "output quoted in the commit message. No lint bypasses, no "
        "#[allow], no #[ignore], no test skips. For the cross-process "
        "re-attach leg: build the fixture (worker killed under a live "
        "agent, replacement worker's re-dispatch resumes the session, "
        "session-id continuity proves it); if the fixture proves genuinely "
        "prohibitive, an EXPLICIT non-claim in your delivery report is the "
        "only permitted fallback — silence is not an option.\n\n"
        "Reply with a full delivery report: file-by-file changes; "
        "red-then-green evidence with commit ids for every acceptance leg; "
        "which legs are proven and which (if any) carry an explicit "
        "non-claim; every permitted choice made and why; every disclosed "
        "non-goal; and only verification actually performed."
    )

    doc = {
        "repo": repo,
        "control_prompt": (
            "Read the root Cargo.toml in this repository and reply with ONLY "
            "the value of workspace.package.version — the exact string, "
            "nothing else. Reply as bare plain text: no backticks, no quotes, "
            "no code formatting — the value alone."
        ),
        "control_expect": "0.17.0",
        "brief": brief,
        "gate_script": GATE,
    }
    json.dump(doc, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
