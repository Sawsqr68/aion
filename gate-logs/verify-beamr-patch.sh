#!/bin/sh
# Witness that a [patch] on beamr ACTUALLY TOOK, before any acceptance run.
#
# WHY THIS EXISTS: a version-mismatched `[patch.crates-io]` fails SILENT.
# Cargo resolves the registry copy instead, everything compiles clean, and a
# green acceptance comes out of a build that never contained the fix. The
# patch LINE is a claim; the LOCK and the resolved dependency graph are facts.
#
# Usage: sh verify-beamr-patch.sh <tree>
set -e
TREE="${1:?usage: verify-beamr-patch.sh <tree>}"
cd "$TREE"

echo "=== 1. What the manifest CLAIMS (a claim, not evidence) ==="
grep -n '^beamr' Cargo.toml || true
grep -n -A 4 '\[patch' Cargo.toml || echo "  (no [patch] section)"

echo
echo "=== 2. EVERY beamr entry in the lock (the fact) ==="
# A lock can carry SEVERAL versions of one crate. Printing only the first is how
# this script lied to its author on its first run: it showed beamr 0.16.3 from a
# transitive edge while the crate under test resolved 0.18.2. Print them ALL,
# then let cargo tree say which one aion actually consumes.
awk '/^name = "beamr"$/{f=1; print "---"} f{print} f&&/^$/{f=0}' Cargo.lock
echo "  ^ if more than one version appears, the lock ALONE cannot settle it —"
echo "    section 3 is what names the version aion-rs resolves."

echo
echo "=== 3. Resolved dependency graph — source must NOT be a bare registry entry ==="
cargo tree -i beamr 2>&1 | head -20

echo
echo "=== VERDICT RULE ==="
echo "PASS only if the lock entry carries a \`source\` naming the git rev or path"
echo "(or carries NO source at all, which is how a path dependency appears)."
echo "A source of \`registry+https://github.com/rust-lang/crates.io-index\` means"
echo "the patch DID NOT TAKE, no matter how clean the build was."
echo
echo "SECOND WITNESS (stronger, use whenever available): reference a public symbol"
echo "that exists ONLY in the patched beamr. If the patch silently dropped, that"
echo "line fails to compile and the build stops before it can produce a"
echo "comforting green. A clean compile is the loudest witness when it could only"
echo "have come from the patched tree."
