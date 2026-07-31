# #119 — a green that is a result about a DIFFERENT PROGRAM

**Status: LANDED.** Clippy clean over all three postures, tests green over all
three, four posture gates green, inverted control watched failing. The OWED list
at the bottom is discharged; kept for the record of what the lane was.

## What #119 actually is

Not a broken cfg. The gating is deliberate and documented — `liminal-transport`
is off by default *"so a default worker build is byte-identical and never links
liminal-sdk."* The defect is that `cargo test -p aion-worker` then exits **0**
having compiled none of the transport. That green is not a weak result; it is a
**result about a different program**, and nobody mistrusts a green for being
about the wrong binary. It is how tonight's P0 reached main: the coverage
existed, and the per-package gate a developer naturally reaches for compiled
none of it.

**Second member, found by enumerating features rather than assuming:**
`aion-proto` has `default = []` with `generated` optional, so
`cargo test -p aion-proto` validates hand-written wire contracts against
NOTHING. That crate has already shipped wire-dead twice against a green suite.

## 🔴 The sharpest finding — the announcement caught it on first run

**The posture depends on WHICH PACKAGES YOU NAME, not only on the flags.**
`aion-worker` depends on `aion-proto` with `features = ["generated"]`, so
cargo's feature unification turns it on whenever both are selected:

```
cargo test -p aion-proto                  -> generated ABSENT   (vacuous)
cargo test -p aion-worker -p aion-proto   -> generated PRESENT
```

Same flags, same crate, opposite coverage — decided by the *other* package on
the command line. This is why the battery pins the ALONE invocation.

## 🔴 Two corrections found while discharging the OWED list

Both were errors in *this lane's own work*, and both were of the exact class the
lane exists to close. Neither was found by review; both were found by checking a
claim against the thing it described.

**1. The self-anchor was itself vacuous.** Clippy's `assertions_on_constants`
fired on both negative arms. It was right: `cfg!` is a compile-time constant, so
`assert!(!cfg!(feature = "x"))` inside `#[cfg(not(feature = "x"))]` is
constant-folded away — *a vacuous assertion inside the module built to stop
vacuous greens*. No `#[allow]`. Both arms now evaluate the claim in a `const`
block, which fails the BUILD if the `cfg` is ever deleted — honest about when it
is decided, and strictly louder than the runtime check it replaced.

**2. The positive arm's NAME asserted coverage the run does not have.** It read
`..._so_its_surface_was_tested`. False: `aion-worker` has exactly ONE integration
target (`tests/grpc_registration.rs`), and it does not drive the liminal dispatch
path. So there is a **second vacuity axis**, independent of feature posture:

```
cargo test -p aion-worker                 -> transport NOT COMPILED
cargo test -p aion-worker --all-features  -> COMPILED, NOT EXERCISED
cargo test -p aion-server                 -> the run that actually proves it
```

The behaviour is proven in `aion-server`, whose dev-dep declares
`aion-worker = { features = ["liminal-transport"] }` and 13 of whose test files
reference the transport. Renamed to
`liminal_transport_compiled_here_but_its_behaviour_is_only_proven_by_aion_server`.

**That axis needed no new gate** — six of those `aion-server` targets name
`aion_worker::LiminalActivityWorker` directly, so dropping the dev-dep feature
breaks their build rather than silently retiring the coverage. Checked before
building anything; the control already existed.

**Also corrected:** both modules claimed "the battery gates never filter." They
do — they filter *to* `build_coverage::` so the announcer is the only thing they
run. They prove the arms track the posture; they cannot make someone else's
ad-hoc narrow filter announce anything. Stated, not papered over.

## The fix

Neither failed nor skipped — **named**. `crates/aion-worker/src/build_coverage.rs`
and `crates/aion-proto/src/build_coverage.rs`: two `cfg`-exclusive test arms, so
exactly one is compiled and printed on every run. A test NAME is the one channel
a runner always prints — no `--nocapture`, no opt-in. Failing the default build
would destroy the byte-identical default the feature protects; `#[ignore]` is
banned by the house rules for making absence invisible.

Positive arms name a type that only exists behind the feature, so they cannot
pass vacuously — if the export moves they stop COMPILING.

**Printed is not checked.** An announcement nothing compares is observability,
not a gate — a run where the wrong arm fires reconciles perfectly because every
count adds up. So `scripts/battery.sh` gained four `posture:*` gates asserting
the posture-appropriate name appeared AND its opposite did not. That external
assertion is also the anchor for the negative arms, which claim little alone.

**Watched failing** — the default-posture expectation run against a
liminal-transport build:
```
POSTURE CONTROL FAILED (INVERTED: default expectation against a liminal build):
expected announcement `liminal_transport_absent_from_this_build` never appeared.
inverted exit=1
```
All four pass in their correct postures.

**Stated limit, not papered over:** a FILTERED run skips the announcer entirely,
and a narrow command is exactly the case this protects — the developer most
likely to be misled is the one most likely to filter. No in-suite mechanism can
close that, and the battery gates do not close it either: they filter *to*
`build_coverage::`, so they prove the arms track the posture but cannot make an
unrelated ad-hoc filter announce anything.

## OWED (all discharged — kept as the record)

1. `cargo fmt --all` (done), then **clippy** `-p aion-worker -p aion-proto
   --all-targets --all-features -- -D warnings`.
2. `cargo test -p aion-worker -p aion-proto --all-targets --all-features
   --no-fail-fast` and the same at default features.
3. Commit + push. Nothing from this lane is committed yet.
4. Update the board: #119 closed by this; note the feature-unification finding
   belongs to #76's class too.

## Block state

BLOCK C declared to Athena with a **derived bound** rather than an invented
range: both packages' graphs are strict subsets of the workspace graph, which
measured 22.77 / 23.07 from clean, so ≤23 at debug scope per posture. That
matters because tonight's three prediction misses were **estimates of my last
error, not of the disk** — a derived bound has no such input.

`target/debug` was ~1.4G mid-lane (mostly incremental reuse), free ~89. Abort 50
(Athena's band; the floor is a band ≥1.5 wide, so a threshold at its edge fires
late). Reclaim at block end; `target/release` NEVER touched (#108, Tom's server).
