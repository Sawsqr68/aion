#!/usr/bin/env zsh
# repo-gates — run the three checks over ONE repository and report every
# finding at full resolution, as machine-readable JSON.
#
# This is Tom's gate command, verbatim in its three cargo invocations and
# verbatim in the jq that extracts from them. Nothing here re-derives what a
# finding is: cargo's own `--message-format=json` says it, jq selects it, and
# this script's only additions are (a) running them against a NAMED repository
# rather than whatever directory it was launched in, and (b) assembling the
# three streams into one document with a verdict.
#
# WHY FULL RESOLUTION. An exit code says a gate went red. It does not say which
# file, which line, which rule, or what to do. A downstream agent handed an exit
# code has to re-run the gate itself to learn anything, and a downstream agent
# handed a tail of log text has to parse prose. The findings list is the thing
# that can be acted on directly, and it is what this returns.
#
# WHY THE `note` FIELDS ARE NOT CARRIED. rustc's child notes on a lint are
# mostly instructions for SILENCING it — `#[allow(...)]` and friends. Handing
# those to an agent whose job is to fix the code is handing it the bypass
# alongside the defect. The verdict carries `instruction` instead, and it says
# the opposite.
#
# WHY IT ALWAYS EXITS 0. The caller is an AWL `run json` action: under that
# capture a non-zero exit is a FAILURE and its stdout is discarded, so a script
# that exited non-zero on a red gate would throw away the findings at exactly
# the moment they matter. Red is a VALUE here — `passed: false` with the list —
# never an exit status. A failure to RUN the gates at all is different, and does
# exit non-zero: see the refusals below.
#
# THE ENVIRONMENT IS CLEARED BEFORE THIS RUNS. A declared command inherits only
# PATH by design (`aion_worker::shell::action` — the host's environment does not
# cross into a declared command). cargo needs HOME to find its registry and
# toolchain, so this script establishes HOME from the account it is running as
# rather than assuming one was passed. That is a deliberate, named
# reconstruction, not an inherited value.
set -u
set -o pipefail

readonly SCRIPT_NAME="${0:t}"

fail() {
  print -ru2 -- "${SCRIPT_NAME}: $1"
  exit 1
}

if (( $# != 1 )); then
  fail "usage: ${SCRIPT_NAME} <repository-path>"
fi

repo="$1"
[[ -d "$repo" ]] || fail "no such directory: ${repo}"
cd "$repo" || fail "cannot enter: ${repo}"
repo="$PWD"
[[ -f Cargo.toml ]] || fail "not a cargo workspace (no Cargo.toml): ${repo}"

# HOME, reconstructed — see the header. `~user` expansion does not itself need
# HOME, so this works in the cleared environment the caller hands us.
if [[ -z "${HOME:-}" ]]; then
  account="$(id -un)" || fail "cannot determine the account this is running as"
  eval "resolved=~${account}" 2>/dev/null || fail "cannot resolve the home directory of ${account}"
  [[ -n "${resolved:-}" && -d "$resolved" ]] \
    || fail "the home directory of ${account} does not exist; cargo cannot run without one"
  export HOME="$resolved"
fi

for tool in cargo jq; do
  whence -p "$tool" >/dev/null 2>&1 \
    || fail "${tool} is not on PATH; the gates cannot run and will not be reported as passing"
done

work="$(mktemp -d)" || fail "cannot create a working directory"
trap 'rm -rf -- "$work"' EXIT INT TERM

# ---------------------------------------------------------------------------
# WHICH TREE THIS MEASURED — captured FIRST, before any gate runs.
#
# A gate run against a dirty tree is evidence about that dirty tree, and a
# report that carries only a PATH cannot say which one. So the commit and the
# working-tree status are read here, at the start, and travel in the report.
#
# Untracked mode is `normal`, not `-uall`, and the reason is measured: `-uall`
# expands every untracked directory to every path beneath it, which on this box
# was 78,867 lines / 8.8 MB — a thing that cannot be carried in a report.
# `normal` names every dirty tracked file individually and every untracked
# directory by its own path.
#
# A DIRECTORY THAT IS NOT A GIT REPOSITORY IS A LEGITIMATE TARGET. Gating a
# cargo workspace that nobody has put under version control is a real thing to
# want, so that case records ITSELF — `head_commit: null` with `vcs: "none"` —
# rather than being silently omitted (which would read as "clean") or refused
# (which would forbid a valid use). What is NOT tolerated is git being present
# and FAILING: that is a broken measurement and refuses, because a report that
# quietly downgrades "git broke" to "no git here" is the same lie by a longer
# route.
if whence -p git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git rev-parse HEAD >"${work}/head" 2>"${work}/head.err" \
    || fail "the repository has no readable HEAD; refusing to report gates against a tree that cannot be named (see ${work}/head.err)"
  git status --porcelain --untracked-files=normal >"${work}/porcelain" 2>"${work}/porcelain.err" \
    || fail "git could not read the working-tree status; refusing to report a clean tree it never measured (see ${work}/porcelain.err)"
  vcs=git
else
  : >"${work}/porcelain"
  vcs=none
fi

# ---------------------------------------------------------------------------
# The extractor, verbatim from the gate command. One JSON object per finding:
# gate, level, file, line, column, lint, message. Child notes are not selected.
#
# A SPAN-LESS DIAGNOSTIC IS STILL A FINDING. Most error/warning messages carry
# at least one span, but not all: the linker's own warnings arrive under
# `#[warn(linker_messages)]` with no source location at all, and they are real.
# So the location is encoded rather than required — `file: "unknown"` with
# `line: 0` and `column: 0` means the compiler attached no place to it. Zero
# cannot collide with a real reading: rustc's lines and columns are 1-based.
# Dropping these would be a narrowing that hides a whole class of warning; a
# null would be a shape the calling document's decoder cannot take.
#
# rustc's own tallies ("N warnings emitted", "aborting due to N previous
# errors") are NOT in this stream — measured on this box, cargo emits them only
# as human text, and the one span-less JSON entry seen there is level
# `failure-note`, which this filter does not select. So the count here is a
# count of diagnostics, not of diagnostics plus their own summary.
# ---------------------------------------------------------------------------
readonly COMPILER_FILTER='
  select(.reason == "compiler-message")
  | select(.message.level == "error" or .message.level == "warning")
  | {
      gate: $gate,
      level: .message.level,
      file: (
        [.message.spans[]? | select(.is_primary)][0].file_name
        // .message.spans[0]?.file_name
        // "unknown"
      ),
      line: (
        [.message.spans[]? | select(.is_primary)][0].line_start
        // .message.spans[0]?.line_start
        // 0
      ),
      column: (
        [.message.spans[]? | select(.is_primary)][0].column_start
        // .message.spans[0]?.column_start
        // 0
      ),
      lint: (.message.code.code // "compile-error"),
      message: .message.message
    }
'

# nextest'"'"'s failed event carries the test'"'"'s name and everything it printed
# — which, for a panicking test, is the whole assertion block including the
# source file and line rustc panicked at. That output IS the finding; there is
# no summary to write beside it.
readonly TEST_FILTER='
  select(.type == "test" and .event == "failed")
  | {
      test: .name,
      stdout: (.stdout // "")
    }
'

# Run one cargo gate: capture its JSON stream, record CARGO's exit code (not
# jq's — the pipeline would otherwise report the extractor's status), and
# extract findings into <name>.findings.
#
# stderr goes to a file rather than /dev/null: a gate that could not run at all
# is a different fact from a gate that ran and found nothing, and the second
# must never be reported when the first is true.
run_cargo_gate() {
  local name="$1"; shift
  local raw="${work}/${name}.raw" err="${work}/${name}.err"

  # NOT named `status`: in zsh that identifier is an alias for `$?` itself, and
  # assigning to it would be assigning to the exit status of the shell.
  "$@" >"$raw" 2>"$err"
  local gate_status=$?
  print -r -- "$gate_status" >"${work}/${name}.exit"

  jq -c --arg gate "$name" "$COMPILER_FILTER" <"$raw" >"${work}/${name}.findings" 2>"${work}/${name}.jqerr"
  if (( $? != 0 )); then
    fail "the ${name} gate produced output jq could not read; refusing to report a finding count that measured nothing (see ${work}/${name}.jqerr)"
  fi
}

run_cargo_gate check \
  cargo check --workspace --all-targets --message-format=json --keep-going

run_cargo_gate clippy \
  cargo clippy --workspace --all-targets --message-format=json --keep-going -- -D warnings

# nextest speaks its own JSON. Its absence is a refusal, never a silent skip:
# a battery that quietly did not run its tests reads as a battery that passed
# them.
whence -p cargo-nextest >/dev/null 2>&1 \
  || fail "cargo-nextest is not installed; the test gate cannot run and will not be reported as passing (cargo install cargo-nextest --locked)"

NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
  cargo nextest run --workspace --all-targets --cargo-quiet \
    --message-format libtest-json-plus --no-fail-fast \
    >"${work}/nextest.raw" 2>"${work}/nextest.err"
print -r -- "$?" >"${work}/nextest.exit"

jq -c "$TEST_FILTER" <"${work}/nextest.raw" >"${work}/nextest.findings" 2>"${work}/nextest.jqerr"
if (( $? != 0 )); then
  fail "the test gate produced output jq could not read; refusing to report a test result that measured nothing (see ${work}/nextest.jqerr)"
fi

# ---------------------------------------------------------------------------
# One document. `passed` is computed from what actually ran: every gate exited
# zero AND no findings came back. The two conditions are both required because
# either alone can be true while the other is false — `--keep-going` can leave
# a zero exit beside real diagnostics, and a gate can die non-zero having
# emitted nothing.
# ---------------------------------------------------------------------------
readonly INSTRUCTION='Fix every finding at its cause. Do not silence, allow, expect, ignore, rename to _, or otherwise bypass any of them — a bypass is not a fix, and this report deliberately omits the compiler notes that suggest one.'

jq -n \
  --arg repo "$repo" \
  --arg vcs "$vcs" \
  --arg head_commit "$([[ "$vcs" == git ]] && cat -- "${work}/head" || print -n '')" \
  --rawfile porcelain "${work}/porcelain" \
  --arg instruction "$INSTRUCTION" \
  --arg check_exit    "$(cat -- "${work}/check.exit")" \
  --arg clippy_exit   "$(cat -- "${work}/clippy.exit")" \
  --arg nextest_exit  "$(cat -- "${work}/nextest.exit")" \
  --slurpfile check   "${work}/check.findings" \
  --slurpfile clippy  "${work}/clippy.findings" \
  --slurpfile tests   "${work}/nextest.findings" \
  '
  # `--all-targets` compiles the lib AND the test target of a crate, and rustc emits
  # the SAME diagnostic once for each — so one defect arrives twice, per gate.
  # Collapsing byte-identical entries is not a narrowing: `unique` only removes
  # duplicates that are equal in every field, so no distinct finding can be lost,
  # and `raw_finding_count` keeps the collapse visible rather than silent. Two
  # copies of one defect is not more exact, it is a count that misrepresents how
  # much is wrong.
  (($check + $clippy) | unique) as $diagnostics
  | {
      repo: $repo,
      # WHICH TREE THIS MEASURED. `vcs` is "git" or "none" — never absent, so a
      # reader never has to tell "not under version control" from "this report
      # predates the field". `head_commit` is the empty string exactly when
      # `vcs` is "none". `porcelain` is git status --porcelain verbatim, empty
      # for a clean tree; it is carried as the LINES themselves rather than a
      # count, because a count cannot attribute and a hash has no preimage a
      # reader holds.
      vcs: $vcs,
      head_commit: $head_commit,
      porcelain: $porcelain,
      gates: [
        { gate: "check",   exit_code: ($check_exit   | tonumber), finding_count: ($check  | unique | length) },
        { gate: "clippy",  exit_code: ($clippy_exit  | tonumber), finding_count: ($clippy | unique | length) },
        { gate: "test",    exit_code: ($nextest_exit | tonumber), finding_count: ($tests  | length) }
      ],
      raw_finding_count: (($check + $clippy) | length),
      errors:       [ $diagnostics[] | select(.level == "error")   ] | length,
      warnings:     [ $diagnostics[] | select(.level == "warning") ] | length,
      findings:     $diagnostics,
      failed_tests: $tests,
      passed: (
        ($check_exit | tonumber) == 0 and
        ($clippy_exit | tonumber) == 0 and
        ($nextest_exit | tonumber) == 0 and
        ($diagnostics | length) == 0 and
        ($tests | length) == 0
      ),
      instruction: $instruction
    }
  ' || fail "the report could not be assembled"
