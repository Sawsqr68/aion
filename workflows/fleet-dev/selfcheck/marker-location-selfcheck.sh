#!/bin/sh
# O3 controls for #148.
# Arm A fails in the world where the engine claim is inside the synthetic tree.
# Arm B fails in the world where fanin exempts the marker name and accepts real dirt.
set -u

marker=.aion-held-execution.json
base=/home/aion/fleet/runs
name=marker-location-selfcheck-$$
root=$base/$name
repo=$root/repo
bin=$(CDPATH= cd -- "$(dirname "$0")/../bin" && pwd)
confounded=42

cleanup() {
    case "$root" in
        /home/aion/fleet/runs/marker-location-selfcheck-*) rm -rf "$root" ;;
        *) printf 'FAIL: cleanup confinement refused: %s\n' "$root" >&2 ;;
    esac
}
trap cleanup 0 1 2 3 15

confound() { printf 'CONFOUNDED: %s\n' "$1" >&2; exit "$confounded"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
payload_code() { printf '%s' "$1" | jq -er '.exit_code' 2>/dev/null; }

[ $# -ge 1 ] && [ $# -le 2 ] || fail 'usage: marker-location-selfcheck.sh arm-a [claim-path] | arm-b | arm-b-mutant'
arm=$1
[ ! -e "$root" ] || fail "scratch run root already exists: $root"
mkdir -p "$repo" || fail 'cannot create synthetic repo'
git -C "$repo" init -q || fail 'cannot init synthetic repo'
git -C "$repo" config user.name 'marker location selfcheck'
git -C "$repo" config user.email 'marker-location-selfcheck@example.invalid'
if [ "$arm" = confounded ]; then
    printf '%s\n' "$marker" > "$repo/.gitignore"
else
    printf 'synthetic tree; marker intentionally not ignored\n' > "$repo/.gitignore"
fi
printf 'base\n' > "$repo/base.txt"
git -C "$repo" add .gitignore base.txt
git -C "$repo" commit -q -m base || fail 'cannot commit synthetic base'
git -C "$repo" switch -q -c leg/build || fail 'cannot create leg branch'
printf 'leg\n' > "$repo/leg.txt"
git -C "$repo" add leg.txt
git -C "$repo" commit -q -m leg || fail 'cannot commit leg branch'
git -C "$repo" switch -q master || fail 'cannot restore primary branch'

# Preconditions are deliberately first. An ignored marker or script drift makes
# either arm incapable of distinguishing the intended cause, so it is CONFOUNDED.
git -C "$repo" check-ignore -q "$marker"
ignore_rc=$?
[ "$ignore_rc" -eq 1 ] || confound "synthetic marker check-ignore rc=$ignore_rc, expected exact rc 1"
if grep -Fq "$marker" "$repo/.gitignore"; then
    confound 'synthetic .gitignore lists the marker'
fi
[ -f /home/aion/fleet/bin/fleet-lib.sh ] || confound 'deployed fleet-lib.sh absent'
[ -f /home/aion/fleet/bin/fleet-fanin.sh ] || confound 'deployed fleet-fanin.sh absent'
cmp -s /home/aion/fleet/bin/fleet-lib.sh "$bin/fleet-lib.sh" || confound 'deployed fleet-lib.sh differs from in-repo script'
cmp -s /home/aion/fleet/bin/fleet-fanin.sh "$bin/fleet-fanin.sh" || confound 'deployed fleet-fanin.sh differs from in-repo script'
printf 'PRECONDITION: synthetic tree does not ignore %s; deployed scripts are byte-identical\n' "$marker"

fanin=$bin/fleet-fanin.sh
case "$arm" in
    arm-a)
        [ $# -eq 2 ] || fail 'arm-a requires the engine claim path'
        case "$2" in
            WORKTREE) claim=$repo/$marker ;;
            STATE)
                canonical=$(realpath "$repo") || fail 'cannot canonicalize synthetic repo'
                key=$(printf '%s' "$canonical" | sha256sum | awk '{print $1}') || fail 'cannot hash canonical repo'
                claim=$root/state/spawn-claims/$key/$marker
                ;;
            *) claim=$2 ;;
        esac
        mkdir -p "$(dirname "$claim")" || fail 'cannot create claim directory'
        printf '{"control":"engine claim deliberately present"}\n' > "$claim" || fail 'cannot write engine claim'
        output=$(/bin/sh "$fanin" "$base" "$name" build)
        process_rc=$?
        [ "$process_rc" -eq 0 ] || fail "fanin process rc=$process_rc, expected exact rc 0"
        code=$(payload_code "$output") || fail "fanin emitted invalid payload: $output"
        [ "$code" -eq 0 ] || fail "arm A payload rc=$code, expected exact rc 0: $output"
        printf 'PASS arm-a: process_rc=0 payload_rc=0 claim=%s\n' "$claim"
        ;;
    arm-b|arm-b-mutant)
        printf '{"control":"stray worktree dirt"}\n' > "$repo/$marker" || fail 'cannot write stray marker'
        if [ "$arm" = arm-b-mutant ]; then
            mutant=$root/fleet-fanin-mutant.sh
            {
                head -n 14 "$fanin"
                printf '%s\n' 'dirty=$(printf '\''%s\n'\'' "$dirty" | grep -v '\''^?? .aion-held-execution.json$'\'')'
                tail -n +16 "$fanin"
            } > "$mutant" || fail 'cannot create mutant fanin copy'
            fanin=$mutant
        fi
        output=$(/bin/sh "$fanin" "$base" "$name" build)
        process_rc=$?
        [ "$process_rc" -eq 0 ] || fail "fanin process rc=$process_rc, expected exact rc 0"
        code=$(payload_code "$output") || fail "fanin emitted invalid payload: $output"
        [ "$code" -eq 2 ] || fail "arm B payload rc=$code, expected exact rc 2: $output"
        printf 'PASS %s: process_rc=0 payload_rc=2\n' "$arm"
        ;;
    confounded) fail 'confounded fixture unexpectedly passed its preconditions' ;;
    *) fail "unknown arm: $arm" ;;
esac
