"""m8: the absence control for the MIR invariance claim, plus unique workflow names.

The three valid harness fixtures all declared `workflow ask_the_agent`, which
`declarations/valid/agent_seam.awl` already uses — and the BC-4 differential loads the
whole covered corpus at once and refuses a duplicate outright (see
`mir/agent_control_tests.rs`, which is why the agent control arm carries its own name).
Four fixtures sharing one name is a hazard this landing introduced; each valid fixture
gets its own name here.
"""

import pathlib

root = pathlib.Path("crates/aion-awl/tests/fixtures/rev2/harness-section/valid")

renames = {
    "acp_harness.awl": "acp_harness_launch",
    "norn_harness.awl": "norn_harness_launch",
    "norn_harness_without_optional_settings.awl": "norn_harness_minimal_launch",
}

for name, workflow in renames.items():
    path = root / name
    text = path.read_text()
    if "workflow ask_the_agent" not in text:
        raise SystemExit(f"{name}: expected the shared name to rename")
    text = text.replace("workflow ask_the_agent", f"workflow {workflow}")
    path.write_text(text)
    print(f"{name}: workflow {workflow}")

# The absence control: `norn_harness_launch` with the `harness` section removed and
# nothing else changed.
source = (root / "norn_harness.awl").read_text()
lines = source.split("\n")
start = next(index for index, line in enumerate(lines) if line == "  harness")
end = next(
    index
    for index in range(start + 1, len(lines))
    if lines[index].startswith("  action ")
)
control_body = lines[:start] + lines[end:]
control = "\n".join(control_body)

narration = """//! THE ABSENCE CONTROL for the harness section's MIR invariance.
//!
//! This is `norn_harness_launch.awl` with the `harness` section removed and
//! nothing else changed. The section is LAUNCH configuration — how a worker
//! process starts an agent — and not flow meaning, so the two must lower to
//! byte-identical MIR. Without this arm the invariance claim rests on three
//! fixtures that all CARRY a section, which cannot tell "the section does not
//! change the lowering" from "these three sections happen to agree".
//!
//! `mir/harness_control_tests.rs` is the comparison. Change one of this pair,
//! change the other.
"""
control = narration + control[control.index("workflow ") :]
control = control.replace(
    "workflow norn_harness_launch", "workflow norn_harness_launch_control"
)
(root / "harness_absent_control.awl").write_text(control)
print("wrote harness_absent_control.awl")

# The ratchet entries follow the file names, which did not change; the new control does.
p = pathlib.Path("crates/aion-awl/src/mir/covered.rs")
s = p.read_text()
s = s.replace(
    '    "harness-section/valid/acp_harness",',
    '    "harness-section/valid/acp_harness",\n'
    '    // The ABSENCE control: the norn fixture with its section removed. Its\n'
    '    // golden must equal the norn fixture\'s, which is what turns "the section\n'
    '    // does not change the lowering" from a claim about three documents that\n'
    '    // all carry one into a comparison against a document that does not.\n'
    '    "harness-section/valid/harness_absent_control",',
)
p.write_text(s)
print("ratchet updated:", "harness_absent_control" in s)
