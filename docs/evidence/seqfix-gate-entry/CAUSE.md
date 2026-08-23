# CAUSE — round 4's red: `gate-entry.sh: line 3: HOME: unbound variable`

`.fleet/gates/RED-LATEST.md` is one line. It is not a battery failure: the gate
process died in its own prologue, before the deliverable checks, before `fmt`,
before `clippy`, before the suite. Nothing about the tree was measured in round 4.

## The cause

`.fleet/gate-entry.sh` line 3 was:

```sh
export PATH=$HOME/.cargo/bin:$PATH
```

with `set -u` on line 2. `set -u` makes the expansion of an unset variable a
fatal error. The harness (`fleet/bin/fleet-gates.sh`) runs the gate as
`sh "$gate_entry"` in an environment that does not export `HOME`, so line 3
aborted the script with status 1 and the harness recorded that single line as
the round's findings.

Reproduced verbatim in `RED-FIRST-home-unbound.log` §A0. §A0b shows the same
prologue passing when `HOME` *is* exported — which is why rounds 1-3 and the
prior leg's two `GATE-GREEN` runs never saw it. The defect is in the gate's
assumption about its own invocation environment, not in the workspace.

## Why "drop the line" is not the fix

The line exists to put the **rustup shim** ahead of any system `cargo`, so that
`rust-toolchain.toml` (`channel = "1.97.1"`) is honoured. On this venue a distro
cargo is also installed:

```
/home/rocketfish/.cargo/bin/cargo  -> 1.97.1   (rustup shim, pinned)
/usr/bin/cargo                     -> 1.94.0   (Arch system rust)
```

Skipping the line when `HOME` is absent leaves `/usr/bin/cargo 1.94.0` to answer.
The gate would then run to completion and print `GATE-GREEN` on a toolchain the
workspace does not pin — three minor versions off, with different clippy lints
and a different `rustfmt`. That is a **false green**, which is worse than the
crash: the crash is loud, a green earned on the wrong compiler is silent. This
was measured, not assumed — the first attempt at this fix was exactly that
skip, and it was caught running the battery on 1.94.0 and abandoned.

## The fix

Resolve the rustup bin directory rather than dereferencing `HOME`:
`CARGO_HOME/bin`, else `HOME/.cargo/bin`, else the invoking user's home from the
passwd database (`getent passwd "$(id -un)"`), which is authoritative and
independent of the environment. When the passwd path is used, `HOME` is exported
to its true value so cargo, rustup and every test binary downstream agree.

Then **prove** the resolution worked: compare `cargo --version` against the
`channel` in `rust-toolchain.toml` and refuse to proceed on a mismatch. Three
named causes, each with a negative control in the transcript:

| cause | condition | control |
|---|---|---|
| `toolchain_absent` | no cargo via `CARGO_HOME`, `HOME` or passwd | §E |
| `toolchain_mismatch` | resolved cargo is not the pinned channel | §D |
| `toolchain_unpinned` | `rust-toolchain.toml` declares no channel | §F |

## What was NOT changed

The four checks the gate already performed are byte-identical — §G diffs
everything after the prologue against `HEAD` and it is unchanged. No
`--exclude`, no target filter, `--no-fail-fast` intact, no `#[ignore]`, no env
gate, no `#[allow]`. `BASE` was **not** touched: the prior leg's §6.1 noted that
`O3`'s parenthetical cites `957de0a1` while the script says `c590712f`, and
re-measurement here (§H) confirms the mis-citation is inert — both bases yield
the identical presence verdict (38 `crates/*.rs`, 40 `docs/evidence/`). Changing
what the gate measures would be a bypass, not a fix.

The prologue is strictly larger than it was: every pre-existing check survives,
and three new ways to fail loudly were added. No source path under `crates/`
was touched; this red never reached the workspace.
