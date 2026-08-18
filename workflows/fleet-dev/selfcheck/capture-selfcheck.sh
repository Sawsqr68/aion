#!/bin/sh
# Behavioural checks for fleet-dev's three durable-capture mechanisms.
set -u

base=/home/aion/fleet/runs
name=selfcheck-$$
root=$base/$name
repo=$root/repo
bin=$(CDPATH= cd -- "$(dirname "$0")/../bin" && pwd)
failed=0

ok() { printf 'ok: %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1"; failed=1; }
probe_code() { printf '%s' "$1" | jq -er '.exit_code' 2>/dev/null; }
cleanup() {
    case "$root" in /home/aion/fleet/runs/selfcheck-*) rm -rf "$root" ;; *) printf 'FAIL: cleanup confinement refused: %s\n' "$root" >&2 ;; esac
}
trap cleanup 0 1 2 3 15

if [ ! -f /home/aion/fleet/bin/fleet-lib.sh ]; then
    printf 'FAIL: required deployed helper absent: /home/aion/fleet/bin/fleet-lib.sh\n' >&2
    exit 1
fi
if [ -e "$root" ]; then
    printf 'FAIL: scratch run root already exists; refusing to touch it: %s\n' "$root" >&2
    exit 1
fi
mkdir -p "$repo" "$root/norn" "$root/claude/project" || exit 1
export FLEET_NORN_STORE=$root/norn
export FLEET_CLAUDE_PROJECTS=$root/claude

git -C "$repo" init -q
git -C "$repo" config user.name 'fleet capture selfcheck'
git -C "$repo" config user.email 'fleet-capture-selfcheck@example.invalid'
printf 'base\n' > "$repo/base.txt"
cat > "$repo/gate-red.sh" <<'EOF'
#!/bin/sh
printf 'RED SELF-CHECK MARKER\n'
exit 1
EOF
cat > "$repo/gate-green.sh" <<'EOF'
#!/bin/sh
printf 'GREEN SELF-CHECK MARKER\n'
exit 0
EOF
git -C "$repo" add base.txt gate-red.sh gate-green.sh
git -C "$repo" commit -q -m 'selfcheck: base'

fanout=$(sh "$bin/fleet-fanout.sh" "$base" "$name" alpha,beta)
[ "$(probe_code "$fanout")" = 0 ] || { printf 'FAIL: setup fanout: %s\n' "$fanout" >&2; exit 1; }
printf 'alpha\n' > "$root/legs/alpha/alpha.txt"
git -C "$root/legs/alpha" add alpha.txt
git -C "$root/legs/alpha" commit -q -m 'leg alpha'
printf 'beta\n' > "$root/legs/beta/beta.txt"
git -C "$root/legs/beta" add beta.txt
git -C "$root/legs/beta" commit -q -m 'leg beta'

leg_sid=11111111-1111-1111-1111-111111111111
printf '{"norn_session_format":2}\n{"event":"leg transcript"}\n' > "$FLEET_NORN_STORE/$leg_sid.jsonl"
printf 'leave staged\n' > "$root/legs/alpha/staged.txt"
git -C "$root/legs/alpha" add staged.txt
one=$(sh "$bin/fleet-sessions.sh" one "$base" "$name" "$root/legs/alpha" alpha "$leg_sid")
if [ "$(probe_code "$one")" = 0 ] && git -C "$root/legs/alpha" cat-file -e "HEAD:.fleet/sessions/alpha-$leg_sid.jsonl" 2>/dev/null; then ok '1 session one captures and commits on the leg branch'; else fail '1 session one captures and commits on the leg branch'; fi
if git -C "$root/legs/alpha" diff --cached --name-only | grep -qx staged.txt && ! git -C "$root/legs/alpha" cat-file -e HEAD:staged.txt 2>/dev/null; then ok '2 path-limited capture leaves unrelated staged content uncommitted'; else fail '2 path-limited capture leaves unrelated staged content uncommitted'; fi
again=$(sh "$bin/fleet-sessions.sh" one "$base" "$name" "$root/legs/alpha" alpha "$leg_sid")
if [ "$(probe_code "$again")" = 0 ] && printf '%s' "$again" | jq -er '.stdout | contains("already-committed")' >/dev/null 2>&1; then ok '3 repeated session capture is idempotent'; else fail '3 repeated session capture is idempotent'; fi

counsel_sid=22222222-2222-2222-2222-222222222222
dev_sid=33333333-3333-3333-3333-333333333333
plan_sid=44444444-4444-4444-4444-444444444444
judge_sid=55555555-5555-5555-5555-555555555555
printf '{"sessionId":"%s","event":"counsel"}\n' "$counsel_sid" > "$FLEET_CLAUDE_PROJECTS/project/$counsel_sid.jsonl"
printf '{"norn_session_format":2}\n{"event":"dev control"}\n' > "$FLEET_NORN_STORE/$dev_sid.jsonl"
primary=$(sh "$bin/fleet-sessions.sh" primary "$base" "$name" "$counsel_sid" "$dev_sid" "$plan_sid" "$judge_sid")
primary_ok=1
[ "$(probe_code "$primary")" = 1 ] || primary_ok=0
for path in \
    ".fleet/sessions/counsel_control-$counsel_sid.jsonl" \
    ".fleet/sessions/dev_control-$dev_sid.jsonl" \
    .fleet/sessions/ABSENT-plan_legs.md \
    .fleet/sessions/ABSENT-judgement.md; do
    git -C "$repo" cat-file -e "HEAD:$path" 2>/dev/null || primary_ok=0
done
for path in .fleet/sessions/ABSENT-plan_legs.md .fleet/sessions/ABSENT-judgement.md; do
    text=$(git -C "$repo" show "HEAD:$path")
    printf '%s' "$text" | grep -Fq "$FLEET_NORN_STORE" || primary_ok=0
    printf '%s' "$text" | grep -Fq "$FLEET_CLAUDE_PROJECTS" || primary_ok=0
done
if [ "$primary_ok" -eq 1 ]; then ok '4 primary capture commits present transcripts and typed absences naming both roots'; else fail '4 primary capture commits present transcripts and typed absences naming both roots'; fi
bad_tree=$(sh "$bin/fleet-sessions.sh" one "$base" "$name" /etc bad "$leg_sid")
bad_id=$(sh "$bin/fleet-sessions.sh" one "$base" "$name" "$repo" bad ../escape)
if [ "$(probe_code "$bad_tree")" = 2 ] && [ "$(probe_code "$bad_id")" = 2 ]; then ok '5 tree and session-id confinement guards refuse unsafe inputs'; else fail '5 tree and session-id confinement guards refuse unsafe inputs'; fi

red=$(sh "$bin/fleet-gates.sh" "$base" "$name" gate-red.sh)
if [ "$(probe_code "$red")" = 1 ] && git -C "$repo" cat-file -e HEAD:.fleet/gates/red-1.md 2>/dev/null && git -C "$repo" cat-file -e HEAD:.fleet/gates/RED-LATEST.md 2>/dev/null; then ok '6 red gate commits numbered and stable findings'; else fail '6 red gate commits numbered and stable findings'; fi
green=$(sh "$bin/fleet-gates.sh" "$base" "$name" gate-green.sh)
if [ "$(probe_code "$green")" = 0 ] && git -C "$repo" cat-file -e HEAD:.fleet/gates/green-2.md 2>/dev/null && [ -s "$repo/.fleet/gates/green-2.md" ] && grep -q 'GREEN SELF-CHECK MARKER' "$repo/.fleet/gates/green-2.md"; then ok '7 green gate commits its numbered non-empty routed tail'; else fail '7 green gate commits its numbered non-empty routed tail'; fi

alpha_head=$(git -C "$repo" rev-parse leg/alpha)
beta_head=$(git -C "$repo" rev-parse leg/beta)
fanin=$(sh "$bin/fleet-fanin.sh" "$base" "$name" alpha,beta)
if [ "$(probe_code "$fanin")" = 0 ] && git -C "$repo" cat-file -e HEAD:.fleet/legs-merged.md 2>/dev/null; then ok '8 fanin commits the leg branch-ref record'; else fail '8 fanin commits the leg branch-ref record'; fi
record=$(git -C "$repo" show HEAD:.fleet/legs-merged.md 2>/dev/null)
if printf '%s\n' "$record" | grep -Fq "| alpha | leg/alpha | $alpha_head |" && printf '%s\n' "$record" | grep -Fq "| beta | leg/beta | $beta_head |"; then ok '9 leg-ref rows preserve every pre-merge branch head'; else fail '9 leg-ref rows preserve every pre-merge branch head'; fi
sha_ok=1
sha_count=0
for sha in $(printf '%s\n' "$record" | tr '|' '\n' | tr -d ' ' | grep -E '^[0-9a-f]{40}$'); do
    sha_count=$((sha_count + 1))
    git -C "$repo" cat-file -e "$sha^{commit}" 2>/dev/null || sha_ok=0
done
if [ "$sha_ok" -eq 1 ] && [ "$sha_count" -eq 4 ]; then ok '10 every recorded leg and merge sha resolves to a commit'; else fail '10 every recorded leg and merge sha resolves to a commit'; fi

exit "$failed"
