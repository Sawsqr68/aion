#!/bin/sh
# The estate claim-obligation leg, invoked TREE-RELATIVELY so it travels with
# this repository instead of naming one developer's home directory.
#
# WHY THIS EXISTS. gates.json used to invoke
#   sh /Users/tom/Developer/ablative/tools/gates/hooks/check_must_be_claimed.sh …
# — a macOS absolute path. On any other box that path does not resolve, `sh`
# exits 127, and the battery records the leg as SILENT. Honest, and useless: it
# reads as covered on every future run while enforcing nothing. Measured on
# Arch/rocketfish 2026-08-19 during the aion#84 re-gate, at baseline and after.
# ⚖️ Ruled by Waffles the Terrible: the claim obligation applies wherever an
# aion tree is mutated, so enforcement travels with the repository.
#
# 🔴 WHY THIS IS A SHIM AND NOT A COPY. The canonical gate is 945 lines and is
# THE ONLY INSTRUMENT THAT CAN SEE A SILENT UNCLAIM. Its own header records
# Finding 22 against exactly what a tree-local copy would do: *"one rule, two
# hand-kept copies, and the day they disagree one instrument silently starts
# measuring a different population."* So this file carries NO logic of its own —
# it locates the single definition and hands over. What travels with the repo is
# the OBLIGATION TO ANSWER, not a second implementation of the answer.
#
# 🔴 AND IT REFUSES RATHER THAN SKIPPING. Where the estate ledger cannot be
# reached, the claim obligation is UNMEASURED — not satisfied. Unmeasured exits
# 2, matching the canonical gate's own contract (*"an unmeasured row is not a
# verdict"*), because 127-as-silent is the failure this file was written to end.
# A red here on a box without the estate ledger is TRUE, not inconvenient: that
# box genuinely cannot witness the obligation. Put the ledger within reach, or
# get the obligation re-ruled — do not soften this leg.
#
# Resolution order for the estate root:
#   1. $ABLATIVE_ROOT, if set — used EXCLUSIVELY, with no fallback, so that an
#      explicit override which is wrong fails loudly instead of silently
#      resolving somewhere else. It is also how this script's refusing direction
#      is tested without dismantling a developer's estate.
#   2. Ancestors of this repository (aion sits at <estate>/stack/aion).
#   3. $HOME/Developer/ablative.

set -eu

hook_rel="tools/gates/hooks/check_must_be_claimed.sh"

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

refuse() {
    printf '⛔ estate-guard UNMEASURED — the claim obligation could not be witnessed here.\n' >&2
    printf '   %s\n' "$1" >&2
    printf '   Looked for: <estate-root>/%s\n' "$hook_rel" >&2
    printf '   This is a REFUSAL, not a skip. The canonical gate is the only instrument\n' >&2
    printf '   that can see a silent unclaim, and a box that cannot reach the estate\n' >&2
    printf '   ledger cannot answer for the estate. Set ABLATIVE_ROOT to a checkout of\n' >&2
    printf '   the estate, or have the obligation re-ruled. Do not delete this leg.\n' >&2
    exit 2
}

if [ -n "${ABLATIVE_ROOT:-}" ]; then
    # Named explicitly: honoured exactly, never fallen back from.
    if [ ! -f "$ABLATIVE_ROOT/$hook_rel" ]; then
        refuse "ABLATIVE_ROOT is set to $ABLATIVE_ROOT but carries no estate gate hooks."
    fi
    estate_root=$ABLATIVE_ROOT
else
    estate_root=
    candidate=$repo_root
    while [ "$candidate" != "/" ]; do
        candidate=$(dirname -- "$candidate")
        if [ -f "$candidate/$hook_rel" ]; then
            estate_root=$candidate
            break
        fi
    done
    if [ -z "$estate_root" ] && [ -f "${HOME:-}/Developer/ablative/$hook_rel" ]; then
        estate_root="$HOME/Developer/ablative"
    fi
    if [ -z "$estate_root" ]; then
        refuse "No estate checkout found above $repo_root, nor at \$HOME/Developer/ablative."
    fi
fi

printf 'estate-guard: delegating to the canonical gate at %s\n' "$estate_root/$hook_rel"
exec sh "$estate_root/$hook_rel" "$estate_root"
