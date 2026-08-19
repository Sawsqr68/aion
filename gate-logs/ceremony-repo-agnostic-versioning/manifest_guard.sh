### The manifest guard spliced into publish_and_confirm, BEFORE any index read
### or publish. publish_one hands EVERY crate in publish_order the SINGLE
### ceremony version, so the crate's own manifest must already wear it: cargo
### publishes whatever the manifest says, and a disagreement would push one
### version while the index confirmation looks for another.
### Args: tree crate version   ->   prints a CrateProbe refusal and exits 0, or is silent.
set -u
t="$1"
c="$2"
v="$3"
cr() { printf "{\"crate_name\":\"%s\",\"skipped\":false,\"live\":false,\"checksum\":\"\",\"rc\":%s,\"log_path\":\"\",\"refusal\":\"%s\"}" "$c" "$1" "$2"; exit 0; }
m=""
for f in $(find "$t" -name Cargo.toml -not -path "*/target/*" 2>/dev/null); do
if sed -n "/^\[package\]/,/^\[/p" "$f" | grep -q "^name *= *\"$c\" *$"; then m="$f"; break; fi
done
if [ -z "$m" ]; then cr 94 "no manifest under $t declares a [package] section named <$c> — publish_order names a crate this tree does not contain"; fi
mv=$(sed -n "/^\[package\]/,/^\[/p" "$m" | grep -m1 "^version *= *\"" | sed -n "s/^version *= *\"\([^\"]*\)\".*/\1/p")
if [ -z "$mv" ]; then
if sed -n "/^\[package\]/,/^\[/p" "$m" | grep -q "^version *\.* *workspace *= *true\|^version *= *{ *workspace *= *true"; then
mv=$(sed -n "/^\[workspace.package\]/,/^\[/p" "$t/Cargo.toml" | grep -m1 "^version *= *\"" | sed -n "s/^version *= *\"\([^\"]*\)\".*/\1/p")
if [ -z "$mv" ]; then cr 94 "$c inherits its version from the workspace ($m says version.workspace = true) but $t/Cargo.toml declares no version under [workspace.package] — nothing supplies a version for this crate"; fi
else
cr 94 "$m declares no version for $c, and does not inherit one from the workspace"
fi
fi
if [ "$mv" != "$v" ]; then cr 94 "$c wears $mv in $m but the ceremony declares $v — cargo publishes what the manifest says, so this would push $mv while the index confirmation looked for $v"; fi
