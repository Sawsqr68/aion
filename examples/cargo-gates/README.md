# cargo-gates

`cargo-gates` is an AWL-authored TEACHING workflow that fans `cargo check`,
Clippy, tests, and the non-mutating formatter check out over one Rust
workspace. It returns each command's real exit code and only reports `clean`
when all four commands pass.

`cargo_gates.awl` is copied verbatim from the marked exam artifact at
`docs/design/aion-authoring/awl/exam/workbench/cargo_gates.awl`; that workbench
file is the source of record. `src/cargo_gates.gleam` is generated and committed,
never edited by hand. Regenerate it with:

```sh
~/.cargo/bin/aion awl emit cargo_gates.awl --output src/cargo_gates.gleam
```

## Where the runnable version lives

This example's shape was PROMOTED into the estate's first-party gate runner
at [`workflows/gates/`](../../workflows/gates/) (#57): seven legs instead of
four, coverage floors, a manifest derived from the run's own history, and a
liminal worker crate (`workflows/gates/worker/`) that serves the queue on a
live server. The activity worker this directory used to carry was deleted in
that promotion — two gate-worker crates would drift, and did — so to RUN a
gates battery, use `workflows/gates/`; to READ the smallest checked document
of the shape, stay here.

## Build and package

From this directory:

```sh
gleam build
~/.cargo/bin/aion package --build .
```

The package command writes `cargo-gates.aion`. Deploying it requires a worker
serving the `cargo_gates` queue's four actions; the first-party runner in
`workflows/gates/` is the maintained implementation of this family, and its
README carries the full deploy-and-serve story.
