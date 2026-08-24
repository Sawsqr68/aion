#!/usr/bin/env python3
"""Compose the liminal22_dev start input from the rig's parts.

Usage: build-input.py <docs-repo-root> > input.json

Refuses (exit 2) while OPERATING-GROUND.md still carries the TBD-LINUX-TESTS
slot — the brief must state the box's MEASURED test-leg truth (Artemis's
record run) before any dispatch composes; a brief predicting where it could
cite is the fault the beamr#27 ground rule exists to prevent.
"""
import json
import sys
from pathlib import Path

docs = Path(sys.argv[1])
rig = docs / "dispatch-rigs" / "liminal22"
round2 = len(sys.argv) > 2 and sys.argv[2] == "--round2"

ground = (rig / "OPERATING-GROUND.md").read_text()
if "TBD-LINUX-TESTS" in ground:
    sys.stderr.write(
        "REFUSED: OPERATING-GROUND.md still carries TBD-LINUX-TESTS — fill it "
        "with the box's measured record-run truth before composing.\n"
    )
    sys.exit(2)

lane_brief = (docs / "briefs" / "liminal-22-reply-fifo-poison-brief-20260819.md").read_text()

if round2:
    control_prompt = (rig / "control-prompt-round2.txt").read_text()
    fix_round = (rig / "FIX-ROUND-BRIEF.md").read_text()
    brief = (
        ground
        + "\n"
        + fix_round
        + "\n=== THE ORIGINAL COMMISSIONED BRIEF, for the defect model and fix "
        + "shape (its legs were round 1's; THIS round's deliverable contract is "
        + "the FIX ROUND section above) ===\n\n"
        + lane_brief
    )
else:
    control_prompt = (rig / "control-prompt.txt").read_text()
    brief = (
        ground
        + "\n=== END OPERATING GROUND — the commissioned brief follows verbatim. "
        + "Where its run-shape or instruments sections name estate-box mechanics "
        + "(the instruments file path, battery-queue line claims, host-reaper "
        + "detachment, worktrees, seat-side merging), the OPERATING GROUND above "
        + "OVERRIDES them for this venue. ===\n\n"
        + lane_brief
    )

json.dump(
    {
        "control_prompt": control_prompt,
        "control_expect": "0.6.1",
        "brief": brief,
        "gate_script": "/home/rocketfish/gate-liminal22.sh",
    },
    sys.stdout,
)
