### probe_release_tree(tree, version) -> TreeProbe {ok, refusal, head_commit, workspace_version}
### REPO-AGNOSTIC: a workspace that versions PER CRATE has no version key to read.
### That is a repo SHAPE, not a cut error, and the two must not share a sentence.
set -u
PATH=/home/aion/.cargo/bin:/Users/tom/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin
export PATH
if [ -z "${HOME:-}" ]; then if [ -d /home/aion ]; then HOME=/home/aion; else HOME=/Users/tom; fi; export HOME; fi
t="$1"
v="$2"
em() { printf "{\"ok\":%s,\"refusal\":\"%s\",\"head_commit\":\"%s\",\"workspace_version\":\"%s\"}" "$1" "$2" "$3" "$4"; exit 0; }
if [ ! -d "$t" ]; then em false "tree directory does not exist" "" ""; fi
h=$(git -C "$t" rev-parse HEAD 2>/dev/null)
if [ -z "$h" ]; then em false "not a git work tree or HEAD unreadable" "" ""; fi
dirty=$(git -C "$t" status --porcelain 2>/dev/null | grep -cv "^??")
case "$dirty" in ""|*[!0-9]*) dirty=0 ;; esac
if [ "$dirty" -gt 0 ]; then em false "tree has tracked modifications — a ceremony publishes exactly what landed" "$h" ""; fi
if [ ! -f "$t/Cargo.toml" ]; then em false "no Cargo.toml at the root of $t" "$h" ""; fi
sect() { sed -n "/^\[$1\]/,/^\[/p" "$t/Cargo.toml"; }
readver() { sect "$1" | grep -m1 "^version *= *\"" | sed -n "s/^version *= *\"\([^\"]*\)\".*/\1/p"; }
wv=$(readver "workspace.package")
src="[workspace.package]"
if [ -z "$wv" ]; then wv=$(readver "package"); src="[package]"; fi
if [ -z "$wv" ]; then em true "" "$h" "(per-crate) neither [workspace.package] nor [package] declares a version key in $t/Cargo.toml — this workspace versions per crate, so the cut witness vouches for nothing here and EVERY crate in publish_order is verified against the declared version by publish_and_confirm before it is published"; fi
if [ "$wv" != "$v" ]; then em false "the $src version $wv does not wear the declared release version $v" "$h" "$wv"; fi
em true "" "$h" "$wv"
