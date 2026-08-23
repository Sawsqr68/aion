#!/bin/sh
# AWL-CUT1 gate — staged by Waffles 2026-08-23, implementing the owner grill's
# staging note (Vesper, brief 9ec0a8f) and the toolchain-pinning standard on
# the venue where the pin-ignored defect was live until today.
#
# Order is load-bearing and is NOT to be reordered:
#   1 toolchain (named causes, pin-honored discrimination)
#   2 tree receipt   3 paths census   4 suite (clippy, new-test control, nextest)
#
# ⛔ NO fmt gate — Tom's estate gate ruling (docs 2ab9689, canonical in
#    GATE-EXTRACTION-STANDARD.md): format is autoformat discipline, never a leg.
#
# Discipline carried from 2026-08-23's paid-for failures:
#   - every rc captured DIRECTLY, never through a pipe
#   - --no-ext-diff on every diff a grep reads
#   - the zero-state is redded FIRST, then a nonzero count required
#   - the receipt names the tree it measured (head + clean/dirty)
#   - a version match alone cannot prove a pin is honored — the SOURCE of the
#     active toolchain is asserted, because this venue ran 1.94.0 against a
#     1.97.1 pin silently until 2026-08-23 (system cargo ignores the pin file)

set -u

BASE=5e2fb5a3ed32b53d48dbc585aa914acd056458c6   # aion main, post-seqfix-relanding, ls-remote verified at staging
# ── WALL AMENDMENT (build leg, FINDING F-RIG-1) ─────────────────────────────
# ONE alternative added to the staged wall, and nothing else:
#     examples/assistant/resources/AWL-REFERENCE\.md
#
# WHY, at the bytes. The staged wall admits `crates/aion-awl` as a prefix,
# which includes the packaged reference copy
# `crates/aion-awl/guide-embed/AWL-REFERENCE.md`. But
# `crates/aion-awl/tests/guide_reference_gate.rs:221` — a PRE-EXISTING test in
# the crate the wall admits — pins that embed byte-identical to the authored
# twin at `examples/assistant/resources/AWL-REFERENCE.md`
# (`crates/aion-awl/src/guide/entry.rs:75`), which the staged wall does NOT
# admit. Editing the admitted file therefore mechanically forces an edit to an
# unadmitted one, so the wall as staged is internally inconsistent.
#
# WHY IT IS FATAL rather than inconvenient. Three pre-existing gates compose:
#   1. guide_reference_gate.rs:204 asserts guide::all().len() ==
#      Keyword::ALL.len() + POSITIONAL_WORDS.len() BY EQUALITY, so any new word
#      forces a glossary entry;
#   2. guide/lookup.rs is an exhaustive match, so a new word fails to compile
#      until its entry is written;
#   3. guide_reference_gate.rs:126 requires the cited section's BODY TEXT to
#      use the word, whole-word matched.
# Measured against the reference's sixteen sections at this base, the words
# `option`, `operand`, `env`, `admits`, `variadic` and `hardening` occur in NO
# section. So within the wall as staged, NO new AWL word can land at all —
# which takes R1, R2, R3 and R7 with it.
#
# RIG.md states its own precedence: "Sources govern the brief; the brief
# governs this rig." R7 is BINDING and the wall is subordinate to it. This is
# one line to revert; the ruling is Waffles' at re-gate and Vesper's at
# landing. Reported as F-RIG-1, first section of .fleet/reports/build.md.
# ────────────────────────────────────────────────────────────────────────────
WALL='^(crates/aion-awl|apps/aion-ops-console/src/features/authoring/assets/awl-guide\.json|examples/assistant/resources/AWL-REFERENCE\.md|Cargo\.toml|Cargo\.lock|\.fleet/)'
PKG_HINT='^crates/aion-awl/'

red() { echo "GATE-RED: $1"; exit 1; }

# ---------------------------------------------------------------- 1. TOOLCHAIN
echo "== 1. toolchain =="
command -v rustc >/dev/null 2>&1 || red "rustc is not on PATH (named cause: toolchain_absent)"
command -v cargo >/dev/null 2>&1 || red "cargo is not on PATH (named cause: toolchain_absent)"
command -v rustup >/dev/null 2>&1 || red "rustup is not on PATH — without rustup the pin file is silently ignored, which is this venue's own measured defect (named cause: toolchain_absent)"

[ -f rust-toolchain.toml ] || red "rust-toolchain.toml absent at repo root (named cause: toolchain_unpinned)"
PIN=$(grep -E '^channel *= *"' rust-toolchain.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
[ -n "${PIN}" ] || red "rust-toolchain.toml carries no channel (named cause: toolchain_unpinned)"
echo "  pinned: ${PIN}"

ACTIVE=$(rustc --version 2>&1) || red "rustc --version failed: ${ACTIVE} (named cause: toolchain_absent)"
echo "  active: ${ACTIVE}"
ACTIVE_V=$(echo "${ACTIVE}" | awk '{print $2}')
[ "${ACTIVE_V}" = "${PIN}" ] || red "active rustc ${ACTIVE_V} but rust-toolchain.toml pins ${PIN} (named cause: toolchain_mismatch)"

# Version equality is NECESSARY but not SUFFICIENT: the venue's stable default
# could coincide with the pin. Assert the SOURCE — rustup must name the
# toolchain file as the override origin (pin-honored), not an environment
# variable or a directory default that happens to match (default-coincides).
SRC=$(rustup show active-toolchain 2>&1) || red "rustup show active-toolchain failed: ${SRC} (named cause: toolchain_absent)"
echo "  source: ${SRC}"
case "${SRC}" in
  *rust-toolchain.toml*) echo "  pin HONORED: active toolchain is sourced from the pin file" ;;
  *) red "active toolchain matches the pin by VERSION but is not sourced from rust-toolchain.toml (${SRC}) — default-coincides is not pin-honored (named cause: pin_unhonored)" ;;
esac

# ----------------------------------------------------------- 2. TREE RECEIPT
echo "== 2. tree receipt =="
HEAD_SHA=$(git rev-parse HEAD) || red "cannot resolve HEAD"
echo "  HEAD: ${HEAD_SHA}"
DIRTY=$(git status --porcelain)
if [ -n "${DIRTY}" ]; then
  echo "  TREE: DIRTY"
  echo "${DIRTY}" | sed 's/^/    /'
  red "working tree is dirty at gate time — a green here would attest bytes that are not committed (named cause: tree_dirty)"
fi
echo "  TREE: CLEAN"

# ---------------------------------------------------------- 3. PATHS CENSUS
echo "== 3. paths census =="
CHANGED=$(git diff --no-ext-diff --name-only "${BASE}...HEAD")
[ -n "${CHANGED}" ] || red "no files changed against base ${BASE} (named cause: empty_diff)"

ESCAPEES=$(echo "${CHANGED}" | grep -Ev "${WALL}" || true)
if [ -n "${ESCAPEES}" ]; then
  echo "  outside the wall:"
  echo "${ESCAPEES}" | sed 's/^/    /'
  red "files changed outside the wall (named cause: wall_breach)"
fi

# Positive control: the deliverable is the AWL crate family — a rig-only or
# lockfile-only tree must not pass the census vacuously.
DELIV=$(echo "${CHANGED}" | grep -cE "${PKG_HINT}" || true)
[ "${DELIV}" -gt 0 ] || red "no changed file under crates/aion-awl/ — the census would have passed vacuously (named cause: deliverable_absent)"
echo "  census OK ($(echo "${CHANGED}" | wc -l | tr -d ' ') files within the wall, ${DELIV} in the deliverable crate family)"

# ------------------------------------------------- 4. SUITE (clippy → tests)
echo "== 4a. clippy =="
cargo clippy --workspace --all-targets -- -D warnings
CLIPPY_RC=$?
[ ${CLIPPY_RC} -eq 0 ] || red "cargo clippy --workspace --all-targets -- -D warnings rc=${CLIPPY_RC} (named cause: clippy_red)"
echo "  clippy clean"

echo "== 4b. new-test control =="
# Red-first evidence duty, measured mechanically: a cut that lands a parser,
# an emitter, and a refusal vocabulary without one new test is not this brief.
NEWTESTS=$(git diff --no-ext-diff "${BASE}...HEAD" -- 'crates/aion-awl*' | grep -cE '^\+.*#\[(test|tokio::test)\]' || true)
[ "${NEWTESTS}" -gt 0 ] || red "the diff adds ZERO test annotations under crates/aion-awl* (named cause: suite_no_new_tests)"
echo "  new test annotations in diff: ${NEWTESTS}"

echo "== 4c. suite =="
# Zero-state first: a suite that names no tests proves nothing by passing.
# rc captured DIRECTLY (a pipe would report grep's status, not nextest's);
# the '::' count approximates the test census — it is a >0 presence control,
# and the authoritative count is nextest run's own summary below.
LIST_OUT=$(cargo nextest list --workspace 2>&1)
LIST_RC=$?
[ ${LIST_RC} -eq 0 ] || { echo "${LIST_OUT}" | tail -20 | sed 's/^/    /'; red "cargo nextest list failed rc=${LIST_RC} (named cause: suite_unlistable)"; }
LISTED=$(echo "${LIST_OUT}" | grep -c '::' || true)
[ "${LISTED}" -gt 0 ] || red "cargo nextest list names ZERO tests (named cause: suite_empty)"
echo "  suite names ~${LISTED} test(s)"

cargo nextest run --workspace
SUITE_RC=$?
[ ${SUITE_RC} -eq 0 ] || red "cargo nextest run --workspace rc=${SUITE_RC} (named cause: suite_red)"

echo "GATE-GREEN: toolchain ${PIN} pin-honored from rust-toolchain.toml; tree CLEAN at ${HEAD_SHA}; census within wall with deliverable present; clippy -D warnings clean; ${NEWTESTS} new test annotations; ${LISTED} tests named and green against base ${BASE}"
