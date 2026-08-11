#!/bin/zsh
# Reviewer's INDEPENDENT sweep, v2 — with a control on BOTH halves.
# v1 had exactly the defect this lane keeps meeting: the specials half was
# controlled, the EXTRACTION half was not, so it extracted zero names and
# reported "no collisions" — a tool failing into the comfortable answer.
emulate -L zsh
setopt extendedglob
set -u
RUNNER="$1"

typeset -a specials
for name in ${(k)parameters}; do
  [[ ${parameters[$name]} == *special* ]] && specials+=$name
done
print "HALF 1 — zsh special table: ${#specials} names"
for control in path status argv; do
  (( ${specials[(Ie)$control]} )) \
    && print "  control OK: \`$control\` is in the table" \
    || { print "  CONTROL FAILED: \`$control\` missing"; exit 1 }
done

extract() {
  sed -nE 's/^[[:space:]]*(local|typeset|readonly)[[:space:]]+//p' "$1" \
  | tr ' \t' '\n\n' \
  | sed -E '/^-/d; s/=.*$//; s/\[.*$//; /^$/d' \
  | sort -u
}

typeset -a declared
declared=(${(f)"$(extract "$RUNNER")"})
print "HALF 2 — declared names extracted: ${#declared}"
if (( ${#declared} == 0 )); then
  print "  CONTROL FAILED: extracted zero names — an empty intersection would mean nothing"
  exit 1
fi
print "  ${(j:, :)declared}"

# Extraction control: a fixture that DOES shadow must be seen.
local fixture=$(mktemp)
print 'f() {\n  local tool="$1" path version\n  typeset -a status\n}' > $fixture
typeset -a fixture_names
fixture_names=(${(f)"$(extract $fixture)"})
rm -f $fixture
if (( ${fixture_names[(Ie)path]} && ${fixture_names[(Ie)status]} )); then
  print "  control OK: the extractor SEES a shadowing fixture (found: ${(j:, :)fixture_names})"
else
  print "  CONTROL FAILED: extractor blind to a known shadowing fixture (found: ${(j:, :)fixture_names})"
  exit 1
fi

typeset -a collisions
for nm in $declared; do
  (( ${specials[(Ie)$nm]} )) && collisions+=$nm
done
if (( ${#collisions} )); then
  print "RESULT: COLLISIONS — ${(j:, :)collisions}"
  exit 1
fi
print "RESULT: no local/typeset/readonly in $RUNNER shadows a zsh special parameter."
