import sys, pathlib
p = pathlib.Path("crates/aion/src/runtime/nif_continue_as_new.rs")
src = p.read_text()
start = src.index("    if !engine_tasks.is_epoch_open() {") if "    if !engine_tasks.is_epoch_open() {" in src else -1
if start == -1:
    sys.exit("NON-RUN: mutation target literal not found — nothing was mutated")
end_marker = "    }\n    recorder\n        .record_workflow_continued_as_new("
end = src.index(end_marker, start) if end_marker in src[start:] else -1
if end == -1:
    sys.exit("NON-RUN: end-of-gate literal not found — nothing was mutated")
mutated = src[:start] + src[end + len("    }\n"):]
if mutated == src:
    sys.exit("NON-RUN: mutation produced identical text")
p.write_text(mutated)
print("MUTATED: the epoch gate is deleted (the literal pre-fix state)")
