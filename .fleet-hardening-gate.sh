#!/bin/sh
# .fleet-hardening-gate.sh — the gate entry for the fleet_dev capture-hardening
# run. Static and box-scoped, and it dispatches no fleet: it proves the two AWL
# documents check, every fleet-dev operation script parses, and the committed
# capture self-check passes. Every failing command is named in the output and
# the gate exits non-zero.
#
# THE EXIT CODE IS THE VERDICT HERE, deliberately unlike the operation scripts.
# This file is run by `fleet-gates.sh` as `sh "$gate_entry"` (bin/fleet-gates.sh:19),
# and THAT script is the one that turns this exit code into typed data
# (bin/fleet-gates.sh:20,27). The exit-zero-always law binds the OpProbe-faced
# scripts, not the thing they measure.
#
# PATH/HOME NOTE — the first red of tonight's flight, fixed for the #63 gate in
# 360d1dc1 ("fix(gates): bind box toolchain home"): the gate runner's HOME and
# PATH are not this box account's, so bind them explicitly rather than assume
# them. `aion` is at $HOME/.cargo/bin/aion on this box (`command -v aion` →
# /home/aion/.cargo/bin/aion); $HOME/.local/bin holds only the claude symlink,
# so $CARGO_HOME/bin is the directory that must be on PATH.
set -u

: "${FLEET_GATE_HOME:=/home/aion}"
HOME="$FLEET_GATE_HOME"
: "${CARGO_HOME:=$HOME/.cargo}"
export HOME CARGO_HOME

# Preserve an already usable runner PATH. If `aion` is not discoverable, add the
# box-scoped installation explicitly and fail clearly if it is absent.
if ! command -v aion >/dev/null 2>&1; then
    if [ ! -x "$CARGO_HOME/bin/aion" ]; then
        echo "fleet-hardening gate: aion not found at $CARGO_HOME/bin/aion or on PATH" >&2
        exit 127
    fi
    PATH="$CARGO_HOME/bin${PATH:+:$PATH}"
    export PATH
fi

failed=0

# Run one gate command, naming it before it runs and again if it fails, so the
# committed tail identifies the failing command without inference.
gate_run() {
    echo "gate: $*"
    if "$@"; then
        return 0
    fi
    echo "GATE FAILED: $*" >&2
    failed=1
    return 0
}

if [ ! -d workflows/fleet-dev/bin ]; then
    echo "fleet-hardening gate: workflows/fleet-dev/bin is absent — this gate must be run from the repository root" >&2
    exit 2
fi

gate_run aion awl check workflows/fleet-dev/fleet_dev.awl
gate_run aion awl check workflows/fleet-dev/fleet_ops_worker.awl

matched=0
for script in workflows/fleet-dev/bin/*.sh; do
    [ -f "$script" ] || continue
    matched=$((matched + 1))
    gate_run sh -n "$script"
done
if [ "$matched" -eq 0 ]; then
    echo "fleet-hardening gate: workflows/fleet-dev/bin/*.sh matched no file — the operation scripts are missing" >&2
    exit 2
fi
echo "gate: parsed $matched operation scripts"

# The behavioural half: the committed self-check exercises all three capture
# gaps against a scratch tree and scratch transcript stores, and is what turns
# red when a gap regresses. A missing self-check is a red, never a skip.
selfcheck=workflows/fleet-dev/selfcheck/capture-selfcheck.sh
if [ -f "$selfcheck" ]; then
    gate_run sh "$selfcheck"
else
    echo "GATE FAILED: sh $selfcheck — the capture self-check is not in the tree" >&2
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    echo "fleet-hardening gate: RED" >&2
    exit 1
fi
echo "fleet-hardening gate: GREEN"
