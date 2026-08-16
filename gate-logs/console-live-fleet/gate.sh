# Fleet gate script — aion ops console shows live work (BRIEF-CONSOLE-LIVE).
# Run by the aion server via fleet_dev's `gates` action: `zsh $script`.
#
# AUTHORED PER BRIEF-GATE-SPEAKS (9a89091) FROM BIRTH. Two properties that
# every other gate script in this estate currently lacks, and that this file
# exists to demonstrate as the reference implementation:
#   1. The accumulator keeps NAMES, not just codes. A gate that has discarded
#      the leg names by the time it decides to fail is structurally incapable
#      of saying what failed.
#   2. The failure path SPEAKS ON STDERR, naming every red leg, its code, and
#      the directory holding its full output. aion records a failed run's
#      `failure_reason` from stderr; a gate that exits 1 in silence produces
#      the string "the declared command `zsh` exited 1 with no standard error
#      output" — which is exactly how the previous fleet run reported itself,
#      with three fully-diagnosed red legs sitting on disk.
# The success path stays silent. A green gate has nothing to say.
#
# SCOPE, STATED: this is a console-wiring brief that touches no Rust source,
# so this gate runs the console battery plus a `cargo check -p aion-server`
# (the console bundle is compiled INTO that crate by rust_embed, so a broken
# embed must fail here). It deliberately does NOT run the full aion Rust
# suite. The dispatcher's landing re-gate runs the standing battery on the
# final tree — this gate refuses early, it does not replace that one.
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/console-live
CONSOLE=$WT/apps/aion-ops-console
EMBED=$WT/crates/aion-server/ops-console-embed
LOGS=$WT/gate-logs/console-live
mkdir -p "$LOGS" || { echo "gate FAILED: cannot create log dir $LOGS" >&2; exit 90; }

typeset -a LEG_RESULTS

run_leg() {
  # run_leg <name> <workdir> <command...>
  local name=$1 dir=$2
  shift 2
  ( cd "$dir" && "$@" ) > "$LOGS/gate-$name.log" 2>&1
  local code=$?
  echo "EXIT=$code" >> "$LOGS/gate-$name.log"
  LEG_RESULTS+=("$name=$code")
  echo "$name=$code"
}

# ---------------------------------------------------------------------------
# AMENDMENT 2026-08-17, after run 21b9ea9a — MY HARNESS PRODUCED TWO FALSE REDS.
# `fanout_setup.sh` runs `bun install` in each LEG worktree; nothing ever ran it
# in the PRIMARY tree, which is where this gate executes after fan-in. So run
# 21b9ea9a's typecheck (exit 2) and unit (exit 1) legs failed entirely on
# `Cannot find module 'react-router' / 'bun:test' / 'react/jsx-runtime'` —
# absent node_modules, not delivery defects. 482 tests passed; all 73 failures
# were module resolution.
#
# Two of the three red legs were the INSTRUMENT, not the SUBJECT. Reporting
# "3 of 6 legs failed, the delivery is broken" would have been wrong about two
# of them — the same error I refuse from everyone else, produced by my own
# harness. The census red was the only true one.
# ---------------------------------------------------------------------------
run_leg deps "$CONSOLE" bun install

run_leg typecheck "$CONSOLE" bun run typecheck
run_leg check     "$CONSOLE" bun run check
run_leg unit      "$CONSOLE" bun test

# ---------------------------------------------------------------------------
# AMENDMENT 2026-08-17, mid-run, by Waffles — the planner overturned an
# assumption this file encoded, and it was right to.
#
# This gate originally assumed each LEG would regenerate the committed embed
# under crates/aion-server/ops-console-embed/ and that the census below would
# read a fresh bundle. `.fleet/plan.md` §2.3 forbids that, on grounds I accept
# at the bytes: vite emits CONTENT-HASHED filenames, so two legs regenerating
# in isolated worktrees produce two different hash sets over ~24 files plus two
# different index.html script refs. That is not a conflict git can merge — it
# is a whole-directory collision on generated output where any resolution is a
# coin toss between two bundles, each missing the other leg's source. Worse, a
# bundle built from a partial tree IS the defect being fixed. The same failure
# shape is already on disk at gate-logs/battery-0150/embed.json:5.
#
# So the embed is regenerated exactly ONCE, on the MERGED tree — and this gate
# is that moment. Had this not been fixed, the census would have read the stale
# committed bundle after every successful run and gone red forever: a gate that
# cannot pass is not a gate, and it would have failed for a reason that had
# nothing to do with the delivery.
# ---------------------------------------------------------------------------
run_leg embed "$WT" cargo xtask build-ops-console

# ---------------------------------------------------------------------------
# CENSUS LEG — the brief's own acceptance instrument, run as a gate.
# R1/R2 are mechanical: the shipped bundle either calls the two live endpoints
# or it does not. Runs AFTER the embed leg above, so it reads the bundle THIS
# RUN just built, never the stale committed one.
#
# index.html's reference is read (never a filename guess — the assets dir holds
# several index-*.js files) and reported, because it anchors the canary to the
# artefact actually served.
#
# DENOMINATOR WIDENED 2026-08-17, adopting the planner's improvement over my
# original: counts are summed over EVERY *.js chunk in assets/, not only the
# chunk index.html names. Vite CODE-SPLITS, so a single-chunk grep can produce a
# FALSE ZERO — precisely the error class this census exists to refuse. My
# before-capture read one chunk and got zeros; the planner re-derived across all
# nine and the zeros held, which is what makes the before-capture trustworthy
# rather than lucky.
#
# The canary is load-bearing: if `workflow` does not appear in the hundreds,
# this script is reading the wrong file or the search is broken, and the leg
# FAILS rather than reporting a comfortable zero. An absence claim from a dead
# instrument is worse than no claim at all.
# ---------------------------------------------------------------------------
census_log=$LOGS/gate-census.log
# SUBSHELL, NOT A COMMAND GROUP — deliberate, and I proved the difference before
# relying on it. Under zsh `{ ...; exit 1; ... } > log` runs in the CURRENT
# shell, so an `exit` inside it terminates gate.sh outright: the run would route
# gates_red correctly and print NOTHING to stderr, which is exactly the silent
# failure this whole file exists to eliminate. `( ... )` forks, so `exit` ends
# only the census and `$?` reaches the accumulator below.
(
  ref=$(/usr/bin/grep -o 'assets/index-[A-Za-z0-9_-]*\.js' "$EMBED/index.html" | head -1)
  echo "index.html references: ${ref:-<NONE>}"
  bundle=$EMBED/$ref
  echo "primary bundle path: $bundle"
  if [ -z "$ref" ] || [ ! -s "$bundle" ]; then
    echo "CENSUS FAILED: could not resolve a non-empty bundle from index.html"
    exit 1
  fi
  echo "primary bundle bytes: $(wc -c < "$bundle" | tr -d ' ')"

  typeset -a CHUNKS
  CHUNKS=("$EMBED"/assets/*.js(N))
  echo "chunks in denominator: ${#CHUNKS[@]}"
  if [ "${#CHUNKS[@]}" -eq 0 ]; then
    echo "CENSUS FAILED: no *.js chunks under $EMBED/assets"
    exit 1
  fi

  count_all() {
    /usr/bin/grep -o "$1" "${CHUNKS[@]}" 2>/dev/null | wc -l | tr -d ' '
  }

  canary=$(count_all 'workflow')
  echo "CANARY workflow=$canary (across all chunks)"
  if [ "$canary" -lt 100 ]; then
    echo "CENSUS FAILED: canary $canary is implausibly low — the instrument is not reading the built bundle, so a zero count below would prove nothing"
    exit 1
  fi
  bad=0
  for ep in workflows/describe-live workflows/children; do
    n=$(count_all "$ep")
    echo "$ep=$n"
    if [ "$n" -eq 0 ]; then
      echo "CENSUS RED: the shipped bundle never calls $ep"
      bad=1
    fi
  done
  # Neighbours on the same router, present before this brief. Held as a
  # control: if these went to zero, the change broke existing wiring.
  for ep in workflows/list workflows/describe workflows/history workflows/transcripts; do
    n=$(count_all "$ep")
    echo "control $ep=$n"
    if [ "$n" -eq 0 ]; then
      echo "CENSUS RED: control endpoint $ep dropped to zero — existing wiring was broken by this change"
      bad=1
    fi
  done
  exit $bad
) > "$census_log" 2>&1
census_code=$?
echo "EXIT=$census_code" >> "$census_log"
LEG_RESULTS+=("census=$census_code")
echo "census=$census_code"

run_leg cargo_check "$WT" cargo check -p aion-server

echo "${LEG_RESULTS[*]}"

typeset -a FAILED
for pair in "${LEG_RESULTS[@]}"; do
  if [ "${pair##*=}" -ne 0 ]; then
    FAILED+=("$pair")
  fi
done

if [ "${#FAILED[@]}" -eq 0 ]; then
  exit 0
fi

{
  echo "GATE RED — ${#FAILED[@]} of ${#LEG_RESULTS[@]} legs failed:"
  for pair in "${FAILED[@]}"; do
    echo "  ${pair%%=*} exited ${pair##*=}  (full output: $LOGS/gate-${pair%%=*}.log)"
  done
  echo "all leg results: ${LEG_RESULTS[*]}"
  echo "log directory: $LOGS"
} >&2
exit 1
