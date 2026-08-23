# RED-FIRST — the reproduction O1 ordered, and its TRUE rate

> **O1, verbatim:** "reproduce the failing integration test on this tree
> (`cargo test -p aion-server --test recovery_declared_body_e2e`), commit the
> reproduction transcript under `docs/evidence/` per the repo's naming".
>
> **§3.3 of the leg brief, verbatim:** "**State the observed crash rate `n/N`,
> not just 'it crashed.'** ... **A reproduction whose true rate is hidden is
> the same failure mode as the void arm A** — do not repeat it."

## The ordered command, on the pre-fix tree — IT PASSES

Full transcript: `RED-FIRST-ordered-command-prefix-tree.log` (this directory).

```
### PRE-FIX TREE: 45686a3e7cfd46791a875abae7a2d5af8ff065be  2026-08-23T14:14:30+10:00
### command as ordered by O1: cargo test -p aion-server --test recovery_declared_body_e2e
    Finished `test` profile [unoptimized] target(s) in 1m 01s
     Running tests/recovery_declared_body_e2e.rs (target/debug/deps/recovery_declared_body_e2e-056da3913225b1e6)
running 1 test
test adopted_declared_body_dispatch_executes_after_recovery ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.56s
### CARGO-EXIT=0
### 2026-08-23T14:15:33+10:00
```

That is the whole of it. The command O1 names, run on the pre-fix tree
(`HEAD`, no edit of any kind), **completes green in 1.56 s and exits 0.**

Twenty consecutive runs of the binary that command produced
(`recovery_declared_body_e2e-056da3913225b1e6`): **crash 0/20, pass 20/20**
(`D3-build-mode-discrimination.txt`, arm P).

## So what IS red

The artifact `.fleet/gates/red-3.md:7189` names —
`recovery_declared_body_e2e-9990dace6ae08d82`, preserved at
`/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/` —
crashes **20/20** (`D1-armA-preserved-binary.txt`). That is a real, in-tree,
deterministic SIGSEGV and it is not in dispute.

## The true rate, stated as a rate

The crash is **not** a property of the source. It is a property of ONE BUILD
DRAW. Measured, with the source held byte-constant throughout:

| what was held constant | what varied | runs | crash |
|---|---|---|---|
| the preserved round-3 artifact | nothing | 20 | **20** |
| source at `HEAD`, `cargo test --workspace` (the gate's own build) | a fresh link of the same source | 20 | **0** |
| source at `HEAD`, `cargo test -p aion-server --test …` (the ordered command) | — | 20 | **0** |
| source at base `957de0a1` | — | 20 | **0** |

**Per-run rate, given a crashing binary: 20/20.**
**Per-build-draw rate: see `D4-build-draw-lottery.txt` — twenty independent
relinks of one unchanged tree, each one a different binary.**

Stating "the crash reproduces deterministically" without that second row would
repeat exactly the failure mode `.fleet/plan.md` §S3.2 caught in arm A: a
number presented as a reproduction when the thing measured was not the thing
claimed.
