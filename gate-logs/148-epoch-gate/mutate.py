#!/usr/bin/env python3
"""Apply or restore one textual mutation, without lying to the build system.

Rules this instrument enforces, each purchased with a real failure:

- A mutation whose target text is not found is a NON-RUN, not a survivor.
- REFUSE TO CLOBBER AN EXISTING BACKUP. A backup left behind means a previous
  run was killed before it restored — the file on disk may BE a mutation, and
  overwriting the backup with it destroys the only good copy. That happened
  once: an interrupted run left mutation A live, the next run backed the
  mutated file up over the good backup, and its "restored byte-identical"
  check passed against a corrupted baseline.
- 🔴 RESTORE BY WRITING BYTES, NEVER BY mv. `mv backup file` carries the
  backup's OLD mtime — older than the binary cargo compiled from the MUTANT —
  so cargo judges the build fresh and every later `cargo test` runs the stale
  mutant binary while the source reads clean. That happened too: Review 21
  observed five "nondeterministic" failures whose signature exactly matched a
  mutation kill, because they WERE the mutation, served from target/ after an
  mv-restore. Writing the bytes gives the file a fresh mtime and forces the
  rebuild. The caller must still end every mutation session with a green
  re-run of the rebuilt tree — that re-run is the only thing that proves the
  BINARY matches the SOURCE again.
"""
import os
import sys

path, mode = sys.argv[1], sys.argv[2]
backup = path + ".orig"

if mode == "apply":
    target, replacement = sys.argv[3], sys.argv[4]
    with open(path, "r", encoding="utf-8") as handle:
        text = handle.read()
    count = text.count(target)
    if count != 1:
        print(f"NON-RUN: target found {count} times, expected exactly 1")
        sys.exit(2)
    if os.path.exists(backup):
        print("NON-RUN: a backup already exists — a previous run did not restore. "
              "Recover the baseline from it FIRST; do not let this run overwrite it.")
        sys.exit(3)
    with open(backup, "w", encoding="utf-8") as handle:
        handle.write(text)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text.replace(target, replacement))
    print("APPLIED")
elif mode == "restore":
    if not os.path.exists(backup):
        print("RESTORE FAILED: no backup exists")
        sys.exit(2)
    with open(backup, "r", encoding="utf-8") as handle:
        original = handle.read()
    # Write (fresh mtime, so cargo rebuilds), then remove the consumed backup.
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(original)
    os.remove(backup)
    print("RESTORED")
else:
    print("bad mode")
    sys.exit(2)
