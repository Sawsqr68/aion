#!/bin/bash
# RED-FIRST, discharged as a MUTATION MATRIX.
#
# WHY THIS SHAPE, STATED PLAINLY. Two of this flight's reds are
# chronological and are committed verbatim beside this script
# (`r1-first-run.txt`, `r6-first-run.txt`): the R1 suite's first run failed
# on a fixture defect, and the R6 suite's first run failed on an absent
# ledger and on a self-matching kill-scope sweep. Those are real reds and
# they are what a first run actually produced.
#
# But a chronological red only proves that ONE test could fail once. The
# property that matters is that EVERY clause's refusal specimen is
# LOAD-BEARING — that no specimen passes for a reason other than the clause
# it is supposed to exercise. A mutation matrix proves that directly and
# reproducibly: neutralize exactly one clause, and the specimen keyed to it
# must go RED. A clause whose mutation leaves the suite green has a
# specimen that was never testing it.
#
# Each mutation makes one clause function return `Ok(())` before it can
# refuse. Nothing else changes. The source is restored after every run.
set -u
root=$(git rev-parse --show-toplevel)
cd "$root" || exit 1
clauses="$root/crates/aion-fleet-plan/src/validate/clauses.rs"
shape="$root/crates/aion-fleet-plan/src/validate/shape.rs"
verify="$root/crates/aion-fleet-plan/src/verify.rs"
backup=$(mktemp -d) || exit 1
cp "$clauses" "$backup/clauses.rs"; cp "$shape" "$backup/shape.rs"; cp "$verify" "$backup/verify.rs"
restore() { cp "$backup/clauses.rs" "$clauses"; cp "$backup/shape.rs" "$shape"; cp "$backup/verify.rs" "$verify"; }
trap 'restore; rm -rf "$backup"' EXIT

survivors=0
total=0

mutate_fn() { # $1 = file, $2 = fn name, $3 = injected first statement
  python3 - "$1" "$2" "$3" <<'PY'
import sys, re
path, fn, inject = sys.argv[1], sys.argv[2], sys.argv[3]
src = open(path).read()
m = re.search(r'(pub fn ' + re.escape(fn) + r'\([^)]*\)[^{]*\{)', src, re.S)
if not m:
    sys.exit("no such fn: " + fn)
src = src[:m.end()] + "\n    " + inject + src[m.end():]
open(path, 'w').write(src)
PY
}

check() { # $1 = label, $2 = file, $3 = fn, $4 = test filter, $5 = injected stmt
  total=$((total+1))
  restore
  mutate_fn "$2" "$3" "$5" || { echo "SKIP  $1 (could not mutate)"; return; }
  if cargo test -p aion-fleet-plan --test "$4" >/dev/null 2>&1; then
    echo "SURVIVED  $1 — the suite stayed GREEN with this clause neutralized"
    survivors=$((survivors+1))
  else
    echo "killed    $1"
  fi
}

echo "=== mutation matrix: neutralize one clause, the suite must go RED ==="
OK='return Ok(());'
check "charset clause (stage + leg ids)"        "$clauses" check_id_charset                    r1_plan_refusals "$OK"
check "clause 2  — stage id nonempty"           "$clauses" check_stage_id_nonempty             r1_plan_refusals "$OK"
check "clause 2  — stage id uniqueness"         "$clauses" check_stage_id_uniqueness           r1_plan_refusals "$OK"
check "clause 3  — leg uniqueness"              "$clauses" check_leg_uniqueness                r1_plan_refusals "$OK"
check "clauses 4+5 — dispatch leg cover"        "$clauses" check_dispatch_leg_cover            r1_plan_refusals "$OK"
check "clause 6  — stage has legs"              "$clauses" check_stage_has_legs                r1_plan_refusals "$OK"
check "clause 10 — conclusion owner"            "$clauses" check_conclusion_leg_membership     r1_plan_refusals "$OK"
check "clause 11 — multi-leg vocabulary"        "$clauses" check_multi_leg_conclusion_count    r1_plan_refusals "$OK"
check "clause 12 — multi-value legs"            "$clauses" check_multi_value_leg_count         r1_plan_refusals "$OK"
check "clause 15 — distinct conclusion paths"   "$clauses" check_conclusion_path_distinctness  r1_plan_refusals "$OK"
check "clause 7  — predecessors"                "$shape"   check_predecessors                  r1_plan_refusals "$OK"
check "clause 9  — acyclicity"                  "$shape"   check_acyclic                       r1_plan_refusals "$OK"
check "clause 8  — chain cardinality"           "$shape"   check_chain_cardinality             r1_plan_refusals "$OK"
check "clauses 13+14 — admit references"        "$shape"   check_admit_references              r1_plan_refusals "$OK"
check "clause 16 — advance"                     "$shape"   check_advance                       r1_plan_refusals "$OK"

echo
echo "=== the R3 seal: neutralize the exact-object-id gate ==="
total=$((total+1))
restore
python3 - "$verify" <<'PY'
import sys
path = sys.argv[1]
src = open(path).read()
old = """        let ok = candidate.len() == 40
            && candidate
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));"""
new = "        let ok = true;"
assert old in src, "seal anchor not found"
open(path, 'w').write(src.replace(old, new))
PY
if cargo test -p aion-fleet-plan --test r3_conclusion_verifier >/dev/null 2>&1; then
  echo "SURVIVED  R3 seal — a ref NAME was accepted as an exact object id and nothing failed"
  survivors=$((survivors+1))
else
  echo "killed    R3 seal — a ref name is refused before any git process starts"
fi

echo
echo "=== the R3 token contract: accept anything as a well-formed token ==="
total=$((total+1))
restore
python3 - "$verify" <<'PY'
import sys
path = sys.argv[1]
src = open(path).read()
old = "    let (last, body) = bytes.split_last()?;"
new = "    return String::from_utf8(bytes.to_vec()).ok();\n    #[allow(unreachable_code)]\n    let (last, body) = bytes.split_last()?;"
assert old in src
open(path, 'w').write(src.replace(old, new, 1))
PY
if cargo test -p aion-fleet-plan --test r3_conclusion_verifier >/dev/null 2>&1; then
  echo "SURVIVED  R3 token contract — malformed bytes verified and nothing failed"
  survivors=$((survivors+1))
else
  echo "killed    R3 token contract"
fi

echo
echo "=== the chain decision function: R5 base law and R6/R8 ordering ==="
decide="$root/crates/aion-fleet-plan/src/chain/decide.rs"
cp "$decide" "$backup/decide.rs"
restore_decide() { cp "$backup/decide.rs" "$decide"; }

chain_mutation() { # $1 = label, $2 = python patch heredoc marker file, $3 = test
  total=$((total+1))
  restore_decide
  python3 "$2" "$decide" || { echo "SKIP  $1"; return; }
  if cargo test -p aion-fleet-plan --test "$3" >/dev/null 2>&1; then
    echo "SURVIVED  $1"
    survivors=$((survivors+1))
  else
    echo "killed    $1"
  fi
  restore_decide
}

patch_base=$(mktemp); cat > "$patch_base" <<'PYEOF'
import sys
p = sys.argv[1]; s = open(p).read()
old = """    ledger
        .last_accepted_commit()
        .unwrap_or(root_base_commit)
        .to_owned()"""
new = """    root_base_commit.to_owned()"""
assert old in s, "base_for anchor not found"
open(p, 'w').write(s.replace(old, new))
PYEOF
chain_mutation "R5 clause 4 — successor base = accepted SHA" "$patch_base" r8_exemplar

patch_admit=$(mktemp); cat > "$patch_admit" <<'PYEOF'
import sys
p = sys.argv[1]; s = open(p).read()
old = """            if holds {"""
new = """            if true {"""
assert old in s, "admission anchor not found"
open(p, 'w').write(s.replace(old, new, 1))
PYEOF
chain_mutation "R8 §9.4 — admission evaluated from recorded history" "$patch_admit" r8_exemplar

patch_hold=$(mktemp); cat > "$patch_hold" <<'PYEOF'
import sys
p = sys.argv[1]; s = open(p).read()
old = """        let Some(conclusion) = ledger.conclusion_of(&stage.id) else {"""
new = """        let Some(conclusion) = ledger.conclusion_of(&stage.id).or(Some("")) else {"""
assert old in s, "verify-before-accept anchor not found"
open(p, 'w').write(s.replace(old, new, 1))
PYEOF
chain_mutation "R3/R4 §2.6 — verify BEFORE accept-or-hold" "$patch_hold" r8_exemplar

rm -f "$patch_base" "$patch_admit" "$patch_hold"
restore; restore_decide
echo
echo "mutations: $total   survivors: $survivors"
if [ "$survivors" -eq 0 ]; then
  echo "MUTATION MATRIX: PASS — every clause is load-bearing; neutralizing any one turns the suite RED"
  exit 0
fi
echo "MUTATION MATRIX: FAIL — $survivors clause(s) have specimens that do not test them"
exit 1
