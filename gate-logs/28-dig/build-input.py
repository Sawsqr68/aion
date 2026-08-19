#!/usr/bin/python3
"""Build the five-field norn_dev input for the 28-dig lane."""

import json
import sys
from pathlib import Path

BRIEF = Path("/Users/tom/Developer/ablative/stack/aion/gate-logs/28-dig/BRIEF.md")
GATE = "/Users/tom/Developer/ablative/stack/aion/gate-logs/28-dig/gate.sh"


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <worktree-path>")

    repo = sys.argv[1].rstrip("/")
    package = BRIEF.read_bytes().decode("utf-8").rstrip()
    brief = (
        package
        + "\n\n---\n\n"
        "You are the BUILDER for the 28-dig lane, working alone in the "
        "workflow repo named above. The package above is your complete "
        "charter: a defect dig with a conditional fix, not a feature build. "
        "Work only in that named worktree; commit as you go; never push and "
        "never merge. The isolation walls in Amendment B are absolute: never "
        "touch the live estate server, ~/.aion, aion-data/, any live worker, "
        "or the sibling worktree .worktrees/libsql-retirement. All "
        "reproduction runs on your own isolated server instance with its own "
        "AION_HOME inside the worktree, on ports you allocate and record. "
        "If you land a fix, follow red-first ordering exactly: every "
        "demanded committed red written, run, and committed failing before "
        "its fix, with the red output quoted in the commit message. No lint "
        "bypasses, no #[allow], no #[ignore], and no test skips.\n\n"
        "Reply with a full delivery report: what reproduced and what did "
        "not (with run counts and exact faces); which hypothesis the "
        "instruments support and the evidence; file-by-file changes; "
        "red-then-green evidence with commit ids if a fix landed; every "
        "permitted choice made and why; every disclosed non-goal; and only "
        "verification actually performed."
    )

    doc = {
        "repo": repo,
        "control_prompt": (
            "Read the root Cargo.toml in this repository and reply with ONLY "
            "the value of workspace.package.version — the exact string, "
            "nothing else. Reply as bare plain text: no backticks, no quotes, "
            "no code formatting — the value alone."
        ),
        "control_expect": "0.16.0",
        "brief": brief,
        "gate_script": GATE,
    }
    json.dump(doc, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
