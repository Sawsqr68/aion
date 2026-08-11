#!/bin/zsh
# gate-runner.zsh — the shell half of the `gates` workflow (workflows/gates/).
#
# Invoked by the `lane-gates-worker` crate (workflows/gates/worker/), which
# spawns it as `zsh -f <this script> <subcommand> ...` with the environment
# cleared to an operator-named PATH (docs/workers/DECLARED-COMMANDS.md states
# the executor contract this mirrors; `-f` suppresses zsh startup files so no
# host dotfile can widen the environment underneath the record). Everything
# this script needs beyond PATH it derives or refuses over, out loud.
#
# CONTRACT — gate subcommands (fmt, clippy_workspace, clippy_server, test,
# bun, embed, gates_worker): stdout carries EXACTLY ONE JSON object, the
# GateResult
#   { gate, exit_code, passed, output_tail }
# and the script exits 0 even when the gate is RED — a red gate is a RESULT
# the workflow routes on, carrying the gate command's real exit code as data.
# `passed` is true exactly when the command exited 0 AND the leg's coverage
# floor (below) held. The script itself exits non-zero only when the
# MEASUREMENT broke — a required tool absent, a workspace it cannot enter, a
# gate command that could not be found (exit 127), an unwritable log — which
# the worker surfaces as a failed activity: an unmeasured gate is never
# reported as a result, in either direction.
#
# COVERAGE FLOORS — an exit code is a claim about failure, not about
# coverage. The `test` leg re-applies the estate's target/result PAIRING
# control (ported from scripts/battery.sh `paired_test`; the naive
# header-count-vs-verdict-count comparison was MEASURED there to false-fire
# on green runs, so the pairing form is the control) plus a nonzero-targets
# floor; the `bun` leg requires a nonzero "Ran N tests" line. A floor
# violation is a RED leg with a distinguishable "COVERAGE CONTROL FAILED"
# reason at the head of its output_tail — never a green. A floor that cannot
# be EVALUATED is a broken measurement, not a red leg.
#
# KNOWN HOLE, DOCKETED, NOT CLOSED HERE: the pairing control is blind to a
# target that starts and reports `running 0 tests` — targets=1, unpaired=0,
# floor holds, green over nothing. This is a hole in the estate's SHARED
# control (`scripts/battery.sh paired_test` has it too, which is why porting
# it faithfully reproduced it), so closing it is a joint docket item against
# both instruments and not a change either may make alone. The trap in the
# naive fix is written down in docs/operations/lane-gates.md.
#
# WHAT WAS MEASURED IS CAPTURED AT THE LEG. Every gate leg writes
#   <workspace>/gate-logs/gates-runs/<workflow_id>/identity/<gate>.json
# — the workspace HEAD and this script's own sha256, read before the gate
# command runs — and the manifest derivation binds itself to those
# observations, refusing when they disagree with each other or with what it
# reads at derivation time. Sampling the tree and the instrument in the
# manifest leg alone is a twelve-to-thirty-minute window in which either can
# move (the lane's r2a run is the demonstration: a release landed mid-run).
#
# FULL LOGS ARE RETAINED. Every leg's complete combined output is kept at
#   <workspace>/gate-logs/gates-runs/<workflow_id>/<gate>.log
# (gitignored — stated home on the measuring box, named in the manifest; the
# bounded tails in workflow history are for routing and reading, the retained
# log is for the #118 class of question the tail cannot answer).
#
# CONTRACT — manifest subcommand: derives the committed-manifest artifact
# MECHANICALLY from the run's own recorded history (`aion describe`), the
# server's own build surface (GET /build), the measured checkout (`git`), the
# legs' own identity observations, and this script's own bytes (the
# instrument block), writes it to the named path, and emits a ManifestReceipt
# JSON on stdout. See the F2 ruling in the README. The expected leg count is
# passed in BY THE WORKFLOW — the one place the legs are declared — and a
# history that does not hold exactly that many completed gates refuses the
# derivation rather than recording a different battery. The tree's porcelain
# status is INLINED into the manifest, not pinned by reference to a file the
# record does not carry.
#
# Measurement-failure exit codes (sysexits-flavoured, each named at the exit
# site): 64 usage, 65 refused data, 69 required tool/service unavailable,
# 70 internal step failed, 71 workspace cannot be entered, 72 gate command
# not found (exit 127).

emulate -L zsh
set -u
setopt pipe_fail

# Belt-and-braces only: zsh itself sets HOME from the passwd database before
# this script runs, even under `-f` with a cleared environment (measured
# 2026-08-11). cargo (the rustup proxy) and bun both need HOME, so this block
# is the explicit guarantee the toolchain rests on — not the mechanism that
# normally supplies it.
if [[ -z "${HOME:-}" ]]; then
  HOME="$(eval echo "~$(id -un)")"
  export HOME
fi

# Tail length carried over from the promoted examples/cargo-gates shape (its
# worker bounded output to the final 40 combined lines).
readonly TAIL_LINES=40

# This script's own resolved path, captured at FILE scope: inside a function
# zsh's $0 is the function name (FUNCTION_ARGZERO), so the instrument block
# must read this instead.
readonly RUNNER_SELF="${0:A}"

# --------------------------------------------------------------------------
# The gate command table — the ONE home of every command this battery runs.
# run_gate executes these arrays verbatim, and the manifest's instrument
# block prints them from the same arrays, so the record and the execution
# cannot drift apart.
# --------------------------------------------------------------------------
typeset -ga CMD_FMT CMD_CLIPPY_WORKSPACE CMD_CLIPPY_SERVER CMD_TEST CMD_BUN
typeset -ga EMBED_STAGE_1 EMBED_STAGE_2 EMBED_STAGE_3 EMBED_STAGE_4
typeset -ga WORKER_STAGE_1 WORKER_STAGE_2 WORKER_STAGE_3
CMD_FMT=(cargo fmt --all -- --check)
# `--workspace` resolves serde_json/preserve_order ON via feature
# unification; the `-p aion-server` leg resolves it OFF. Both resolutions are
# real programs and both are required (the #76 posture record) — which is why
# clippy is two legs, not one.
CMD_CLIPPY_WORKSPACE=(cargo clippy --workspace --all-targets -- -D warnings)
CMD_CLIPPY_SERVER=(cargo clippy -p aion-server --all-targets -- -D warnings)
# `--no-fail-fast` is load-bearing: without it the first failing TARGET
# un-runs every target after it (the #118 P0 record in scripts/battery.sh).
CMD_TEST=(cargo test --workspace --no-fail-fast)
CMD_BUN=(bun test)
# The embed gate is the NON-MUTATING decomposition of
# `cargo xtask verify-ops-console`: the xtask verb regenerates the ts-rs wire
# types in-tree (AION_UPDATE_BINDINGS=1), which rewrites tracked files on the
# stale path, and this battery is ruled to mutate nothing. Wire types are
# ASSERTED (write-nothing mode), the console is rebuilt into its gitignored
# dist/, and the rebuild is diffed against the committed embed bundle. The
# diff excludes dotfiles and *.map on BOTH sides — strictly more permissive
# than the xtask compare, which filters the committed side only.
EMBED_STAGE_1=(cargo test -p aion-core export_ops_console_wire_types)
EMBED_STAGE_2=(bun install --frozen-lockfile)
EMBED_STAGE_3=(bun run build)
EMBED_STAGE_4=(diff -r -x '.*' -x '*.map' --)
# The battery gates its own instrument: the lane-gates-worker crate is its
# own cargo workspace (root membership is a named follow-up), so the
# workspace-wide legs above never touch it. These three stages hold it to the
# same bar (deny-level lints in its Cargo.toml).
WORKER_STAGE_1=(cargo fmt --check)
WORKER_STAGE_2=(cargo clippy --all-targets -- -D warnings)
WORKER_STAGE_3=(cargo test)

# The named home of a run's full leg logs, derived (never invented) from the
# measured workspace plus the WORKFLOW's durable id — the value the document
# passes (`workflow.id`) and the value the manifest derivation rebuilds this
# path from, so all three names agree with the value (r2 minor 5).
run_log_dir() {
  local ws="$1" workflow_id="$2"
  print -r -- "$ws/gate-logs/gates-runs/$workflow_id"
}

# Every tool a subcommand needs, checked before any work: a missing toolchain
# is a BROKEN MEASUREMENT, not a red gate and not a skip (scripts/battery.sh
# states the incident history; B1 of the r1 review measured the hole this
# closes).
require_tools() {
  local tool missing=0
  for tool in "$@"; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      print -u2 "gate-runner: required tool \`$tool\` is not on PATH — the measurement cannot run, and an unmeasured gate must never be reported"
      missing=1
    fi
  done
  (( missing == 0 )) || return 69
}

# The `cargo` BINARY existing does not establish that `cargo fmt` or
# `cargo clippy` can run — those are the separate `rustfmt` and `clippy`
# components, and a toolchain can carry cargo without either. Measured on this
# box: `cargo <subcommand-that-does-not-exist>` exits **101**, not 127 — the
# SAME code clippy uses for a genuine `-D warnings` finding. So the 127 guard
# in run_gate cannot see it, and without this probe a box lacking rustfmt or
# clippy (the moment slice two runs on another host) writes
# `{gate: "clippy_workspace", exit_code: 101, passed: false}` into the record:
# a broken measurement wearing a red gate's clothes. Each subcommand answers
# its own `--version` before its leg, and a silent one is refused as a broken
# measurement (r2 M-r2-4).
require_cargo_subcommands() {
  local subcommand component
  for subcommand in "$@"; do
    case "$subcommand" in
      fmt)    component="rustfmt" ;;
      clippy) component="clippy" ;;
      *)      component="$subcommand" ;;
    esac
    if ! cargo "$subcommand" --version >/dev/null 2>&1; then
      print -u2 "gate-runner: \`cargo $subcommand --version\` does not answer — the \`$component\` component is not installed for this toolchain. A missing cargo subcommand exits 101, the same code a real clippy finding uses, so this is a BROKEN MEASUREMENT and no gate result is reported"
      return 69
    fi
  done
}

# M2 — the identity of what is being measured, captured AT THE LEG rather
# than sampled by the manifest leg twelve to thirty minutes later. Every gate
# leg writes its own observation before it runs its command; the manifest
# derivation binds itself to those observations and REFUSES when they
# disagree with each other or with what it reads at derivation time. Without
# this the record cannot state that the tree it names is the tree the fmt leg
# measured, nor that the runner it hashed is the runner that executed the
# legs — and the lane's own r2a run proved the window is not theoretical: the
# 0.13.8 release landed mid-run and HEAD moved underneath the battery.
#
# A workspace whose HEAD git cannot read is a broken measurement, not a leg
# that quietly measures an unnameable tree.
#
# Known blind spot (m-r3-3): a RETRIED leg overwrites its own observation
# file, so a tree or instrument move between two attempts of the SAME leg is
# invisible to the cross-leg check unless another leg straddles the move.
# The observed-vs-derivation comparison still bounds the window.
capture_identity() {
  local ws="$1" gate="$2" identity_dir head_commit runner_digest observed_at git_output
  identity_dir="$RUN_LOG_DIR/identity"
  mkdir -p -- "$identity_dir" || {
    print -u2 "gate-runner: cannot create the identity directory $identity_dir — the measurement cannot record what it measured"
    return 70
  }
  git_output="$(git -C "$ws" rev-parse HEAD 2>&1)" || {
    print -u2 "gate-runner: \`git -C $ws rev-parse HEAD\` failed for gate \`$gate\` — a leg that cannot name the commit it measures reports nothing. git said:"
    print -u2 -r -- "$git_output"
    return 65
  }
  head_commit="$git_output"
  runner_digest="$(shasum -a 256 -- "$RUNNER_SELF" | cut -d" " -f1)" || {
    print -u2 "gate-runner: cannot hash the runner ($RUNNER_SELF) for gate \`$gate\` — the record must name the instrument that ran"
    return 70
  }
  observed_at="$(date '+%Y-%m-%dT%H:%M:%S%z')"
  jq -n --arg gate "$gate" --arg observed_at "$observed_at" --arg head_commit "$head_commit" \
        --arg runner_path "$RUNNER_SELF" --arg runner_sha256 "sha256:$runner_digest" \
        '{gate: $gate, observed_at: $observed_at, head_commit: $head_commit,
          runner_path: $runner_path, runner_sha256: $runner_sha256}' >"$identity_dir/$gate.json" || {
    print -u2 "gate-runner: writing the identity observation for gate \`$gate\` failed"
    return 70
  }
}

# Emit one GateResult. `passed` is exit_code == 0 AND an empty floor verdict;
# a non-empty floor verdict is prepended to the tail so the reason is the
# first thing a reader sees.
emit_result() {
  local gate="$1" code="$2" log="$3" floor="$4"
  local passed=false
  (( code == 0 )) && [[ -z "$floor" ]] && passed=true
  {
    [[ -n "$floor" ]] && printf '%s\n' "$floor"
    tail -n "$TAIL_LINES" -- "$log"
  } | jq -Rs --arg gate "$gate" --argjson code "$code" --argjson passed "$passed" \
      '{gate: $gate, exit_code: $code, passed: $passed, output_tail: .}' || {
    print -u2 "gate-runner: failed to encode the GateResult for gate \`$gate\` (real exit code was $code)"
    return 70
  }
}

# The estate's cargo-test coverage control, ported from scripts/battery.sh
# `paired_test` (the #118/#120 record). Asserts the PAIRING — every started
# target reports a result before the next target starts — plus a nonzero
# target count. The obvious header-count == verdict-count comparison was
# MEASURED there to false-fire on green runs (child re-invocations print
# nested blocks), so the pairing form is the control. Prints a verdict line
# on violation; prints nothing when the floor holds.
#
# A floor VIOLATION is a red leg (the run measured too little). A floor that
# cannot be EVALUATED — the scan itself failing — is a BROKEN MEASUREMENT and
# returns non-zero, which run_gate turns into a non-zero runner exit and the
# worker into a terminal activity failure: "we could not verify this leg" is
# not "this leg failed", and recording it as a red gate would attribute the
# fault to the code under test (r2 minor 2; this is also what makes run_gate's
# `|| return 70` a live path rather than dead code, r2 minor 3).
coverage_floor_cargo_test() {
  local log="$1" counts targets unpaired
  counts=$(awk '
    /^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (started) unpaired++; started = 1; targets++ }
    /^test result:/ { started = 0 }
    END { if (started) unpaired++; print targets + 0, unpaired + 0 }
  ' "$log") || {
    print -u2 "gate-runner: the target/result pairing scan over $log failed — this leg's coverage cannot be verified, and an unverifiable measurement is broken, not red"
    return 70
  }
  targets=${counts%% *}
  unpaired=${counts##* }
  if (( targets == 0 )); then
    print -r -- "COVERAGE CONTROL FAILED: the run started ZERO test targets. An exit code from a run that tested nothing is not a result about this code."
  elif (( unpaired != 0 )); then
    print -r -- "COVERAGE CONTROL FAILED: $unpaired of $targets target(s) started and never reported a result. The run was TRUNCATED; its tally is not a coverage claim, whatever it says."
  fi
  return 0
}

# The bun suite's floor: a nonzero "Ran N tests" line. An absent line is a
# violation too — a runner that cannot see the count must refuse, not assume.
# A scan that cannot run at all is a broken measurement, as above.
coverage_floor_bun() {
  local log="$1" ran
  ran=$(sed -nE 's/.*Ran ([0-9]+) tests? across.*/\1/p' "$log" | sed -n '$p') || {
    print -u2 "gate-runner: the bun test-count scan over $log failed — this leg's coverage cannot be verified, and an unverifiable measurement is broken, not red"
    return 70
  }
  if [[ -z "$ran" ]]; then
    print -r -- "COVERAGE CONTROL FAILED: bun's own 'Ran N tests' line never appeared — this leg cannot claim any test ran."
  elif (( ran == 0 )); then
    print -r -- "COVERAGE CONTROL FAILED: bun ran ZERO tests. An exit code from a run that tested nothing is not a result about this code."
  fi
  return 0
}

# Run one gate command in a directory, retain its full combined output, apply
# the gate's floor, and emit the GateResult. The gate's exit code is DATA;
# this function returns non-zero only when the measurement machinery itself
# broke (unenterable workspace, command not found, unwritable log).
run_gate() {
  local gate="$1" dir="$2" floor_fn="$3"
  shift 3
  local log="$RUN_LOG_DIR/$gate.log"

  # The workspace is checked SEPARATELY from the gate command: a directory
  # the measurement cannot enter is a broken measurement, and recording it as
  # a red gate would be indistinguishable in the manifest from the gate
  # genuinely failing (B1).
  if ! ( cd -- "$dir" ) 2>"$log"; then
    print -u2 "gate-runner: cannot enter \`$dir\` for gate \`$gate\` — the measurement is broken, no result is reported:"
    cat -- "$log" >&2
    return 71
  fi

  ( cd -- "$dir" && "$@" ) >"$log" 2>&1
  local code=$?
  if (( code == 127 )); then
    print -u2 "gate-runner: gate \`$gate\` exited 127 (command not found) — an absent tool is a broken measurement, not a red gate:"
    tail -n "$TAIL_LINES" -- "$log" >&2
    return 72
  fi

  # The floor is evaluated only on a GREEN command, deliberately: the
  # dangerous direction is the vacuous green (an exit code from a run that
  # measured nothing), and a red leg is already red. The cost is stated in the
  # README (r2 minor 4): a RED `test` leg therefore carries NO coverage
  # verdict, so "exit 101" on that leg must not be read as "a test failed" —
  # it may be a truncation, and answering which is a hand pairing check over
  # the retained full log, not something the workflow has answered.
  #
  # A floor function returning non-zero means the floor could not be
  # EVALUATED; that is a broken measurement and propagates as one.
  local floor=""
  if (( code == 0 )) && [[ -n "$floor_fn" ]]; then
    floor="$($floor_fn "$log")" || {
      print -u2 "gate-runner: the coverage floor for gate \`$gate\` could not be evaluated — no result is reported for a leg whose coverage cannot be verified"
      return 70
    }
  fi
  emit_result "$gate" "$code" "$log" "$floor"
}

# The embed gate's stages, in order, each announced so the tail names the
# failing stage. Commands come from the shared table above.
embed_stages() {
  local ws="$1"
  print "=== embed 1/4: wire-type freshness — ${EMBED_STAGE_1[*]} (assert mode, writes nothing) ==="
  ( cd -- "$ws" && "${EMBED_STAGE_1[@]}" ) || return $?
  print "=== embed 2/4: ${EMBED_STAGE_2[*]} (refuses, never rewrites, a drifted lockfile) ==="
  ( cd -- "$ws/apps/aion-ops-console" && "${EMBED_STAGE_2[@]}" ) || return $?
  print "=== embed 3/4: ${EMBED_STAGE_3[*]} ==="
  ( cd -- "$ws/apps/aion-ops-console" && "${EMBED_STAGE_3[@]}" ) || return $?
  print "=== embed 4/4: rebuilt dist/ matches committed crates/aion-server/ops-console-embed/ ==="
  "${EMBED_STAGE_4[@]}" "$ws/apps/aion-ops-console/dist" "$ws/crates/aion-server/ops-console-embed"
}

# The instrument-gate stages: fmt, clippy, tests of the lane-gates-worker
# crate itself, which the workspace-wide legs cannot reach (it is its own
# cargo workspace with its own target/).
#
# These three do NOT short-circuit (r2 minor 6). They are three independent
# questions about the same crate, and stopping at the first red hides the
# other two answers behind it — the same truncated-diagnosis shape as running
# cargo test without `--no-fail-fast`, one level up. All three run; the leg's
# exit code is the FIRST failure's, so the verdict is unchanged and only the
# diagnosis is complete. (The embed stages, by contrast, are genuinely
# sequential — you cannot diff a build that did not happen — so they keep
# their short-circuit.)
gates_worker_stages() {
  local crate="$1" first_failure=0 stage_status
  print "=== gates_worker 1/3: ${WORKER_STAGE_1[*]} (in workflows/gates/worker) ==="
  ( cd -- "$crate" && "${WORKER_STAGE_1[@]}" )
  stage_status=$?
  (( first_failure == 0 )) && first_failure=$stage_status
  print "=== gates_worker 2/3: ${WORKER_STAGE_2[*]} (run even if 1/3 was red — every stage answers) ==="
  ( cd -- "$crate" && "${WORKER_STAGE_2[@]}" )
  stage_status=$?
  (( first_failure == 0 )) && first_failure=$stage_status
  print "=== gates_worker 3/3: ${WORKER_STAGE_3[*]} ==="
  ( cd -- "$crate" && "${WORKER_STAGE_3[@]}" )
  stage_status=$?
  (( first_failure == 0 )) && first_failure=$stage_status
  return $first_failure
}

# One gate's command, as the manifest records it — read from the same arrays
# run_gate executes.
gate_command_text() {
  case "$1" in
    fmt)              print -r -- "${CMD_FMT[*]}" ;;
    clippy_workspace) print -r -- "${CMD_CLIPPY_WORKSPACE[*]}" ;;
    clippy_server)    print -r -- "${CMD_CLIPPY_SERVER[*]}" ;;
    test)             print -r -- "${CMD_TEST[*]} [+ coverage floor: battery.sh target/result pairing, nonzero targets]" ;;
    bun)              print -r -- "(cd apps/aion-ops-console) ${CMD_BUN[*]} [+ coverage floor: nonzero 'Ran N tests']" ;;
    embed)            print -r -- "1/4: ${EMBED_STAGE_1[*]}; 2/4+3/4 (cd apps/aion-ops-console): ${EMBED_STAGE_2[*]}; ${EMBED_STAGE_3[*]}; 4/4: ${EMBED_STAGE_4[*]} <dist> <ops-console-embed>" ;;
    gates_worker)     print -r -- "(cd workflows/gates/worker) 1/3: ${WORKER_STAGE_1[*]}; 2/3: ${WORKER_STAGE_2[*]}; 3/3: ${WORKER_STAGE_3[*]}" ;;
    *)                return 64 ;;
  esac
}

# One tool's identity for the instrument block: resolved path plus the first
# line of its own version report.
#
# The local is `tool_location`, NOT `path`: `path` is the zsh special array
# tied to `PATH`, so `local path` empties PATH for the whole function — every
# `command -v` then misses and this function degrades to nine nulls while the
# gates themselves run fine (measured; the r2 review caught it in the
# committed record). `status` is the same class of trap, already paid for
# above. No local in this script may take a special parameter's name.
tool_identity() {
  local tool="$1" tool_location version
  tool_location="$(command -v -- "$tool")" || {
    print -u2 "gate-runner: \`$tool\` did not resolve on PATH while deriving the instrument block"
    print -r -- '{"path": null, "version": null}'
    return 0
  }
  version="$("$tool" --version 2>&1 | sed -n 1p)"
  jq -n --arg location "$tool_location" --arg version "$version" '{path: $location, version: $version}'
}

# manifest <workflow_id> <server_grpc> <server_http> <out_path> <expected_legs> <workspace>
#            [--unobserved-instrument-post-mortem]
#
# The run's event history is the INSTRUMENT for the verdicts; this derivation
# is the RECORD (the F2 ruling). Verdicts come only from the server's own
# answers (`aion describe`), server identity from GET /build, workspace
# identity from the legs' own captured observations reconciled against git,
# and the instrument block from this script's own bytes and the resolved
# tools — never from a hand-maintained copy.
#
# The trailing acknowledgment is for POST-MORTEM derivation only: a run whose
# legs left no identity observations (they predate the capture, or the log
# directory was cleared) cannot have its measured tree and instrument named
# by this derivation. The flag does not fill the gap in — it records the gap,
# and the manifest then carries `head_commit`/`runner_sha256` as NULL with
# only the at-derivation readings beside them. The workflow never passes it.
cmd_manifest() {
  if (( $# < 6 || $# > 7 )); then
    print -u2 "gate-runner: manifest expects <workflow_id> <server_grpc> <server_http> <out_path> <expected_legs> <workspace> [--unobserved-instrument-post-mortem], got $# argument(s)"
    return 64
  fi
  local wid="$1" grpc="$2" http="$3" out="$4" expected="$5" ws="$6" acknowledged=false
  if (( $# == 7 )); then
    if [[ "$7" != "--unobserved-instrument-post-mortem" ]]; then
      print -u2 "gate-runner: the only seventh argument manifest accepts is \`--unobserved-instrument-post-mortem\`, got \`$7\`"
      return 64
    fi
    acknowledged=true
  fi
  require_tools jq aion curl shasum git date mkdir sed || return $?

  local describe describe_err
  describe="$(mktemp)" || { print -u2 "gate-runner: mktemp failed"; return 70; }
  describe_err="$describe.err"

  # stderr is kept and repeated on failure: the reason a derivation failed is
  # part of the failure (r1 minor 7).
  if ! aion --endpoint "$grpc" describe "$wid" >"$describe" 2>"$describe_err"; then
    print -u2 "gate-runner: \`aion --endpoint $grpc describe $wid\` failed — no history, no manifest. Its stderr:"
    cat -- "$describe_err" >&2
    rm -f -- "$describe" "$describe_err"
    return 69
  fi

  # ONE selection, used for both the leg table and the tails digest, so the
  # two can never disagree about which activities are the gates: every
  # completed activity whose recorded result carries the GateResult members,
  # in activity-id (dispatch) order.
  local gate_filter='[.history[] | select(.type == "ActivityCompleted") | .data
    | select(.result | type == "object" and has("gate") and has("exit_code") and has("passed") and has("output_tail"))]
    | sort_by(.activity_id)'

  local legs count
  legs="$(jq "$gate_filter"' | map({gate: .result.gate, activity_id: .activity_id, exit_code: .result.exit_code, passed: .result.passed})' "$describe")" || {
    print -u2 "gate-runner: jq failed reading the describe history"; rm -f -- "$describe" "$describe_err"; return 70; }
  count="$(jq 'length' <<<"$legs")" || { rm -f -- "$describe" "$describe_err"; return 70; }

  # Refused in BOTH directions, each named as itself (r1 minor 6): fewer
  # completed gates than the document dispatched is a leg summarised away;
  # more is a record of legs the document never declared.
  if [[ "$count" != "$expected" ]]; then
    if (( count < expected )); then
      print -u2 "gate-runner: the history holds $count completed gate leg(s) but the workflow dispatched $expected — refusing to derive a manifest that summarises a leg away"
    else
      print -u2 "gate-runner: the history holds $count completed gate leg(s) but the workflow declares only $expected — refusing to record legs the document never dispatched"
    fi
    rm -f -- "$describe" "$describe_err"
    return 65
  fi

  # The tails digest: sha256 over the output tails in the SAME order as the
  # leg table, joined with the unit separator so adjacent tails cannot be
  # re-split into a colliding concatenation. (`jq -r` appends one trailing
  # newline, which is part of the digested bytes — the verifier must match.)
  local digest
  digest="$(jq -r "$gate_filter"' | map(.result.output_tail) | join("\u001f")' "$describe" | shasum -a 256 | cut -d" " -f1)" || {
    print -u2 "gate-runner: computing the tails digest failed"; rm -f -- "$describe" "$describe_err"; return 70; }

  # `status` itself is a read-only zsh special ($? alias), so the run's
  # status gets a named variable of its own. When this leg runs inside the
  # run it is always "Running" by construction; the docs state that, and a
  # post-mortem re-derivation shows the terminal status.
  local run_id package_version workflow_type started_at run_status
  run_id="$(jq -r '.history[] | select(.type == "WorkflowStarted") | .data.run_id' "$describe")"
  package_version="$(jq -r '.history[] | select(.type == "WorkflowStarted") | .data.package_version' "$describe")"
  workflow_type="$(jq -r '.summary.workflow_type' "$describe")"
  started_at="$(jq -r '.summary.started_at' "$describe")"
  run_status="$(jq -r '.summary.status' "$describe")"
  if [[ -z "$run_id" || "$run_id" == "null" ]]; then
    print -u2 "gate-runner: the describe history carries no WorkflowStarted run_id — refusing to fabricate one"
    rm -f -- "$describe" "$describe_err"
    return 65
  fi

  # Server identity from the server's own build surface — never asserted.
  # --max-time 30: a manifest against a hung HTTP endpoint must fail loudly,
  # not stall to the workflow deadline (r1 minor 8; 30s is generous for a
  # local /build and named here as the operator-visible bound).
  local build_json
  if ! build_json="$(curl -sf --max-time 30 "$http/build")"; then
    print -u2 "gate-runner: GET $http/build failed — the manifest must carry the server's own identity, not a guess"
    rm -f -- "$describe" "$describe_err"
    return 69
  fi

  # What was measured: the workspace's commit and its porcelain status.
  # git's own exit codes are checked at every call and the raw output is the
  # record — a line count cannot attribute, and a failed git would count 0,
  # the most comfortable possible answer (r1 M3).
  #
  # THE STATUS PREIMAGE IS INLINED, not pinned by reference (r2 M-r2-2). The
  # earlier shape wrote the status to a file under `gate-logs/gates-runs/`,
  # which `gate-logs/.gitignore` excludes — so the committed manifest carried
  # a sha256 whose preimage was never committed and was deleted with the logs:
  # an unverifiable hash attributes exactly as poorly as the count it
  # replaced. The lines now live IN the manifest, so the hash is checkable
  # from committed bytes alone.
  #
  # UNTRACKED MODE IS `normal`, NOT `-uall`, and the reason is measured, not
  # assumed: on this box `-uall` expands the standing local snapshot
  # directories to 78,867 lines / 8.8 MB (measured 2026-08-11), which can be
  # neither inlined nor committed, while `normal` names every dirty tracked
  # file individually and every untracked DIRECTORY by its own path — 16
  # lines / 532 bytes on the same tree. Attribution comes from recording the
  # LINES rather than a count; expanding a snapshot directory to eighty
  # thousand paths adds no attribution any reader can use.
  #
  # Timing dependency, stated (r2 minor 8): this read happens AFTER the legs
  # have written their full logs and identity observations under
  # `gate-logs/gates-runs/`, so the tree state recorded here is downstream of
  # the runner's own artifacts. It is inert only while `gate-logs/.gitignore`
  # keeps `gates-runs/` untracked; the manifest's own directory is NOT
  # ignored, so a manifest committed beside earlier manifests will see them
  # in this listing — which is honest and intended, but it is why this record
  # must never be read as "the tree before the battery touched it".
  local head_commit_at_derivation logs_dir status_sha git_err porcelain_file untracked_mode
  git_err="$describe.giterr"
  head_commit_at_derivation="$(git -C "$ws" rev-parse HEAD 2>"$git_err")" || {
    print -u2 "gate-runner: \`git -C $ws rev-parse HEAD\` failed — the manifest cannot name what was measured. git stderr:"
    cat -- "$git_err" >&2
    rm -f -- "$describe" "$describe_err" "$git_err"
    return 65
  }
  logs_dir="$(run_log_dir "$ws" "$wid")"
  mkdir -p -- "$logs_dir" || { rm -f -- "$describe" "$describe_err" "$git_err"; return 70; }
  porcelain_file="$describe.porcelain"
  untracked_mode="normal — every dirty tracked file named individually, every untracked directory named by its own path (\`git status --porcelain=v1\`); \`-uall\` expands standing local snapshot directories to a preimage that cannot be committed, and the record is the LINES, not a count"
  if ! git -C "$ws" status --porcelain=v1 >"$porcelain_file" 2>"$git_err"; then
    print -u2 "gate-runner: \`git -C $ws status --porcelain=v1\` failed — refusing to record a tree state git did not report. git stderr:"
    cat -- "$git_err" >&2
    rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
    return 65
  fi
  status_sha="$(shasum -a 256 -- "$porcelain_file" | cut -d" " -f1)" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }

  # The instrument block (r1 B3): the record names WHAT ran — this script by
  # path and content hash, every gate command from the same table run_gate
  # executes, the resolved toolchain, and the PATH it all resolved under.
  local runner_path runner_sha gate_commands tool_versions
  runner_path="$RUNNER_SELF"
  runner_sha="$(shasum -a 256 -- "$runner_path" | cut -d" " -f1)" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }

  # M2 — bind the record to what the LEGS observed, not to what this leg can
  # sample now. Each gate leg wrote `<logs_dir>/identity/<gate>.json` before
  # running its command; there must be exactly as many observations as the
  # document declares legs, they must agree with each other, and they must
  # agree with the readings above. Any disagreement means the tree or the
  # instrument moved underneath the battery — the r2a shape, where the 0.13.8
  # release landed mid-run — and a record that cannot say which tree it
  # measured is refused rather than written with the later of the two.
  local identity_dir observation_count observations observed_head observed_runner_sha identity_note
  local -a identity_files
  identity_dir="$logs_dir/identity"
  identity_files=("$identity_dir"/*.json(N))
  observation_count=${#identity_files}
  observed_head=""
  observed_runner_sha=""
  if (( observation_count == 0 )); then
    if [[ "$acknowledged" != true ]]; then
      print -u2 "gate-runner: no per-leg identity observations under $identity_dir — this derivation cannot state which tree the legs measured or which runner executed them. Re-derive with --unobserved-instrument-post-mortem to record the run WITH that gap named, or run the battery again on a runner that captures them"
      rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
      return 65
    fi
    # m-r3-2: the note's two causes are distinguishable — a cleared log
    # directory takes the sibling `<gate>.log` files with it, so their
    # survival excludes that arm. The record states which world it is in
    # instead of naming both.
    local -a sibling_logs
    sibling_logs=("$logs_dir"/*.log(N))
    local absence_cause
    if (( ${#sibling_logs} > 0 )); then
      absence_cause="${#sibling_logs} gate log(s) survive beside this derivation, so the log directory was NOT cleared: either the legs predate the identity capture, or the identity directory alone was removed after the run"
    else
      absence_cause="no gate logs survive beside this derivation either — the run's log directory was cleared"
    fi
    identity_note="NO per-leg identity observation exists for this run ($absence_cause). The operator acknowledged this explicitly. head_commit and runner_sha256 are therefore NULL: this record does NOT claim to name the tree the legs measured or the runner that executed them. Everything else in this block — gate_commands, tool_versions, path — and the workspace's status_porcelain_at_derivation are readings taken AT DERIVATION from the runner and the box as they stand now, not a claim about what executed the legs; the runner's command table may even have changed shape since. The VERDICTS are unaffected: the leg table, legs_failed and tails_digest come from the run's own recorded history."
  else
    if (( observation_count != expected )); then
      print -u2 "gate-runner: $observation_count identity observation(s) under $identity_dir but the workflow dispatched $expected leg(s) — refusing to derive a record whose legs cannot all be shown to have measured one tree with one instrument"
      rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
      return 65
    fi
    observations="$(jq -s 'sort_by(.gate)' "${identity_files[@]}")" || {
      print -u2 "gate-runner: reading the identity observations under $identity_dir failed"
      rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
      return 70
    }
    local distinct_heads distinct_runners
    distinct_heads="$(jq -r '[.[].head_commit] | unique | length' <<<"$observations")" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }
    distinct_runners="$(jq -r '[.[].runner_sha256] | unique | length' <<<"$observations")" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }
    if [[ "$distinct_heads" != 1 || "$distinct_runners" != 1 ]]; then
      print -u2 "gate-runner: the legs did not all measure one tree with one instrument ($distinct_heads distinct HEAD(s), $distinct_runners distinct runner hash(es)) — the tree or the runner MOVED during the battery, so no single tree can be named. Observations:"
      jq -r '.[] | "  \(.gate): head=\(.head_commit) runner=\(.runner_sha256) at \(.observed_at)"' <<<"$observations" >&2
      rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
      return 65
    fi
    observed_head="$(jq -r '.[0].head_commit' <<<"$observations")"
    observed_runner_sha="$(jq -r '.[0].runner_sha256' <<<"$observations")"
    if [[ "$observed_head" != "$head_commit_at_derivation" ]]; then
      print -u2 "gate-runner: the legs measured HEAD $observed_head but this derivation reads $head_commit_at_derivation — the tree moved between the battery and its record. Refusing; re-derive as a post-mortem if the run is to be recorded with that gap named"
      rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
      return 65
    fi
    if [[ "$observed_runner_sha" != "sha256:$runner_sha" ]]; then
      print -u2 "gate-runner: the legs ran runner $observed_runner_sha but this derivation hashes sha256:$runner_sha — the instrument changed between the battery and its record (a mid-run edit is exactly what a mutation probe does). Refusing"
      rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
      return 65
    fi
    identity_note="Every leg captured the tree and the runner before running its command; all $observation_count agree with each other and with the derivation-time reading — so gate_commands below are the commands that ran, printed from the same arrays, out of bytes proven identical. tool_versions and path are still readings taken at derivation: a toolchain can be upgraded mid-run and this record would not see it."
  fi
  local identity_block
  identity_block="$(jq -n --argjson observed "$( (( observation_count > 0 )) && print true || print false )" \
      --arg note "$identity_note" \
      --argjson observations "${observations:-[]}" \
      '{observed: $observed, note: $note, observations: $observations}')" || {
    rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }
  local observed_head_json observed_runner_json
  observed_head_json="$(jq -n --arg value "$observed_head" 'if $value == "" then null else $value end')"
  observed_runner_json="$(jq -n --arg value "$observed_runner_sha" 'if $value == "" then null else $value end')"
  gate_commands="$(jq -n \
    --arg fmt "$(gate_command_text fmt)" \
    --arg clippy_workspace "$(gate_command_text clippy_workspace)" \
    --arg clippy_server "$(gate_command_text clippy_server)" \
    --arg test "$(gate_command_text test)" \
    --arg bun "$(gate_command_text bun)" \
    --arg embed "$(gate_command_text embed)" \
    --arg gates_worker "$(gate_command_text gates_worker)" \
    '{fmt: $fmt, clippy_workspace: $clippy_workspace, clippy_server: $clippy_server, test: $test, bun: $bun, embed: $embed, gates_worker: $gates_worker}')" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }
  tool_versions="$(jq -n \
    --argjson zsh "$(tool_identity zsh)" \
    --argjson cargo "$(tool_identity cargo)" \
    --argjson rustc "$(tool_identity rustc)" \
    --argjson bun "$(tool_identity bun)" \
    --argjson jq "$(tool_identity jq)" \
    --argjson git "$(tool_identity git)" \
    --argjson aion "$(tool_identity aion)" \
    --argjson curl "$(tool_identity curl)" \
    --argjson shasum "$(tool_identity shasum)" \
    '{zsh: $zsh, cargo: $cargo, rustc: $rustc, bun: $bun, jq: $jq, git: $git, aion: $aion, curl: $curl, shasum: $shasum}')" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }

  local derived_at
  derived_at="$(date '+%Y-%m-%dT%H:%M:%S%z')"

  mkdir -p -- "${out:h}" || { rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"; return 70; }
  if ! jq -n \
      --arg schema "aion.gates.manifest.v3" \
      --arg derived_at "$derived_at" \
      --arg workflow_id "$wid" \
      --arg workflow_type "$workflow_type" \
      --arg run_id "$run_id" \
      --arg package_version "$package_version" \
      --arg started_at "$started_at" \
      --arg status_at_derivation "$run_status" \
      --arg grpc "$grpc" \
      --arg http "$http" \
      --argjson build "$build_json" \
      --arg workspace "$ws" \
      --argjson head_commit "$observed_head_json" \
      --arg head_commit_at_derivation "$head_commit_at_derivation" \
      --arg status_porcelain_sha256 "sha256:$status_sha" \
      --arg untracked_mode "$untracked_mode" \
      --rawfile status_porcelain "$porcelain_file" \
      --arg logs_dir "$logs_dir" \
      --arg runner_path "$runner_path" \
      --argjson runner_sha256 "$observed_runner_json" \
      --arg runner_sha256_at_derivation "sha256:$runner_sha" \
      --argjson identity_capture "$identity_block" \
      --argjson gate_commands "$gate_commands" \
      --argjson tool_versions "$tool_versions" \
      --arg resolved_path "$PATH" \
      --argjson legs "$legs" \
      --arg tails_digest "sha256:$digest" \
      '{schema: $schema,
        derived_at: $derived_at,
        workflow: {workflow_id: $workflow_id, workflow_type: $workflow_type, run_id: $run_id,
                   package_version: $package_version, started_at: $started_at,
                   status_at_derivation: $status_at_derivation},
        server: {grpc: $grpc, http: $http, build: $build},
        workspace: {path: $workspace,
                    head_commit: $head_commit,
                    head_commit_at_derivation: $head_commit_at_derivation,
                    status_porcelain_untracked_mode: $untracked_mode,
                    status_porcelain_sha256: $status_porcelain_sha256,
                    status_porcelain_at_derivation:
                      ($status_porcelain | if . == "" then [] else rtrimstr("\n") | split("\n") end)},
        instrument: {runner_path: $runner_path,
                     runner_sha256: $runner_sha256,
                     runner_sha256_at_derivation: $runner_sha256_at_derivation,
                     identity_capture: $identity_capture,
                     gate_commands: $gate_commands, tool_versions: $tool_versions,
                     path: $resolved_path},
        logs_dir: $logs_dir,
        legs: $legs,
        legs_recorded: ($legs | length),
        legs_failed: ($legs | map(select(.passed | not)) | length),
        tails_digest: $tails_digest}' >"$out"; then
    print -u2 "gate-runner: writing the manifest to $out failed"
    rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"
    return 70
  fi
  rm -f -- "$describe" "$describe_err" "$git_err" "$porcelain_file"

  jq -n --arg path "$out" --arg run_id "$run_id" --arg tails_digest "sha256:$digest" \
      --argjson legs_recorded "$count" \
      '{path: $path, run_id: $run_id, tails_digest: $tails_digest, legs_recorded: $legs_recorded}'
}

if (( $# < 1 )); then
  print -u2 "usage: gate-runner.zsh <fmt|clippy_workspace|clippy_server|test|bun|embed|gates_worker> <workspace> <workflow_id>"
  print -u2 "       gate-runner.zsh manifest <workflow_id> <server_grpc> <server_http> <out_path> <expected_legs> <workspace> [--unobserved-instrument-post-mortem]"
  exit 64
fi

subcommand="$1"
shift

case "$subcommand" in
  fmt|clippy_workspace|clippy_server|test|bun|embed|gates_worker)
    if (( $# != 2 )); then
      print -u2 "gate-runner: \`$subcommand\` expects <workspace> <workflow_id>, got $# argument(s)"
      exit 64
    fi
    workspace="$1"
    # The WORKFLOW id (not the run id: a workflow's retries share one log
    # home, and the document passes `workflow.id`) becomes a directory name
    # under the workspace; anything outside the durable-id alphabet is
    # refused rather than joined into a path.
    workflow_id="$2"
    if [[ -z "$workflow_id" || "$workflow_id" == *[^A-Za-z0-9-]* ]]; then
      print -u2 "gate-runner: workflow id \`$workflow_id\` is not a plain durable identifier — refusing to build a log path from it"
      exit 64
    fi
    # git and shasum are required by every leg, not just the manifest: each
    # leg captures the tree and the instrument it is about to measure (M2).
    require_tools jq tail mktemp mkdir git shasum date || exit $?
    # The workspace must already exist: `mkdir -p` on the log path below
    # would otherwise CREATE a mistyped workspace and let the gate fail
    # inside an empty directory — a red result about nothing (B1).
    if [[ ! -d "$workspace" ]]; then
      print -u2 "gate-runner: workspace \`$workspace\` does not exist or is not a directory — the measurement is broken, no result is reported"
      exit 71
    fi
    RUN_LOG_DIR="$(run_log_dir "$workspace" "$workflow_id")"
    mkdir -p -- "$RUN_LOG_DIR" || { print -u2 "gate-runner: cannot create log directory $RUN_LOG_DIR"; exit 70; }
    capture_identity "$workspace" "$subcommand" || exit $?
    ;;
esac

case "$subcommand" in
  fmt)
    require_tools cargo || exit $?
    require_cargo_subcommands fmt || exit $?
    run_gate fmt "$workspace" "" "${CMD_FMT[@]}"
    ;;
  clippy_workspace)
    require_tools cargo || exit $?
    require_cargo_subcommands clippy || exit $?
    run_gate clippy_workspace "$workspace" "" "${CMD_CLIPPY_WORKSPACE[@]}"
    ;;
  clippy_server)
    require_tools cargo || exit $?
    require_cargo_subcommands clippy || exit $?
    run_gate clippy_server "$workspace" "" "${CMD_CLIPPY_SERVER[@]}"
    ;;
  test)
    require_tools cargo awk || exit $?
    run_gate test "$workspace" coverage_floor_cargo_test "${CMD_TEST[@]}"
    ;;
  bun)
    require_tools bun sed || exit $?
    run_gate bun "$workspace/apps/aion-ops-console" coverage_floor_bun "${CMD_BUN[@]}"
    ;;
  embed)
    require_tools cargo bun diff || exit $?
    run_gate embed "$workspace" "" embed_stages "$workspace"
    ;;
  gates_worker)
    require_tools cargo || exit $?
    # This leg runs fmt AND clippy AND test of the worker crate, so both
    # components must answer before it can report anything.
    require_cargo_subcommands fmt clippy || exit $?
    run_gate gates_worker "$workspace/workflows/gates/worker" "" gates_worker_stages "$workspace/workflows/gates/worker"
    ;;
  manifest)
    cmd_manifest "$@"
    ;;
  *)
    print -u2 "gate-runner: unknown subcommand \`$subcommand\`"
    exit 64
    ;;
esac
