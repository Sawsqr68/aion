import sys, pathlib
p = pathlib.Path("crates/aion/src/runtime/nif_timer_fire.rs")
src = p.read_text()
LIT = """            Self::WheelTornDown {
                timer_id,
                cancelled: true,
            } => wheel_torn_down_before_cancelling(&timer_id),
            Self::WheelTornDown {
                timer_id,
                cancelled: false,
            } => wheel_torn_down_after_firing(&timer_id),
"""
if LIT not in src:
    sys.exit("NON-RUN: the split dispatch arms were not found; nothing was mutated")
MUT = """            Self::WheelTornDown { timer_id, .. } => wheel_torn_down_after_firing(&timer_id),
"""
pathlib.Path("/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad/fa-restore.rs").write_text(src)
p.write_text(src.replace(LIT, MUT, 1))
print("MUTATED: into_seam_error now discards the outcome discriminant (the pre-fix behaviour)")
