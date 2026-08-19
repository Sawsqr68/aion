#!/usr/bin/python3
"""Build the five-field norn_dev input for the libsql-retirement lane."""

import json
import sys
from pathlib import Path

BRIEF = Path("/Users/tom/Developer/ablative/stack/aion/gate-logs/libsql-retirement/BRIEF.md")
GATE = "/Users/tom/Developer/ablative/stack/aion/gate-logs/libsql-retirement/gate.sh"


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: build-input.py <worktree-path>")

    repo = sys.argv[1].rstrip("/")
    package = BRIEF.read_bytes().decode("utf-8").rstrip()
    brief = (
        package
        + "\n\n---\n\n"
        "You are the BUILDER for the libsql-retirement lane, working alone in "
        "the workflow repo named above. The package above is your complete "
        "charter. Work only in that named worktree; commit as you go; never push "
        "and never merge. Never touch the live estate, any server or worker "
        "process, or ~/.aion. Follow the brief's red-first ordering exactly: "
        "every demanded committed red must be written, run, and committed "
        "failing before its fix, with the red output quoted in the commit "
        "message. Preserve the control and mutation arms. No lint bypasses, no "
        "#[allow], no #[ignore], and no test skips. Respect every ratified fork, "
        "amendment, scope boundary, and hard wall in the brief.\n\n"
        "Reply with a full delivery report: file-by-file changes; red-then-green "
        "evidence with commit ids; every permitted choice made and why; every "
        "disclosed non-goal; and only verification actually performed."
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
