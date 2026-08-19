### verify_install(tree, crate_name, version) -> InstallProbe {green, rc, reported_version, log_path, refusal}
### REPO-AGNOSTIC: the install probe assumed the release version names a crate
### that ships a BINARY. A lib-only crate cannot satisfy it and never could, so
### "no binary landed" read as a failed install when the truth is the probe does
### not apply. Absence is now named, never dressed as a red.
set -u
PATH=/home/aion/.cargo/bin:/Users/tom/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin
export PATH
if [ -z "${HOME:-}" ]; then if [ -d /home/aion ]; then HOME=/home/aion; else HOME=/Users/tom; fi; export HOME; fi
ulimit -n 65536 2>/dev/null || true
t="$1"
c="$2"
v="$3"
ip() { printf "{\"green\":%s,\"rc\":%s,\"reported_version\":\"%s\",\"log_path\":\"%s\",\"refusal\":\"%s\"}" "$1" "$2" "$3" "$4" "$5"; exit 0; }
if [ -z "$c" ]; then ip false 0 "" "" "declared-none: the ceremony declares no install_probe_crate, so no from-index install was attempted. This is DECLARED absence, not a failed install - the release is not gated on it and the record says so rather than wearing a tick it did not earn"; fi
mkdir -p "$t/.release" 2>/dev/null
l="$t/.release/install-verify.log"
d="$t/.release/install-root"
rm -rf "$d" 2>/dev/null
cargo install "$c" --version "$v" --root "$d" > "$l" 2>&1
rc=$?
if [ "$rc" != "0" ]; then
if grep -q "there is nothing to install" "$l" 2>/dev/null; then ip false 0 "" "$l" "not-applicable: $c $v ships no binary target, so a from-index install probe cannot apply to it. Declare install_probe_crate as the crate that carries the binary, or as an empty string when the release ships none. This is NOT a failed install"; fi
ip false "$rc" "" "$l" "from-index install red - read the log"
fi
b=$(ls "$d/bin" 2>/dev/null | head -1)
if [ -z "$b" ]; then ip false 93 "" "$l" "not-applicable: cargo install reported success for $c $v but placed no binary in the install root, which is the shape of a lib-only crate. Declare install_probe_crate as the crate that carries the binary, or as an empty string when the release ships none"; fi
rv=$("$d/bin/$b" --version 2>/dev/null | head -1 | tr -d "\"")
g=false
printf "%s" "$rv" | grep -q "$v" && g=true
r=""
if [ "$g" = "false" ]; then r="the installed binary reports <$rv>, which does not carry the released version $v"; fi
ip "$g" 0 "$rv" "$l" "$r"
