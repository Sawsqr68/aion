# Production leg report

## Summary

Implemented the shipped Gleam failure-classifier fix for issue #63 in the only owned production source file, `crates/aion-toolchain/src/gleam_failure.rs`, and committed it as `efe2496d` (`fix(toolchain): classify package file IO failures`).

The classifier still treats the four existing registry titles unconditionally and still treats `Dependency resolution failed` conditionally. It now also recognizes `error: File IO failure` only when the combined diagnostic contains package-layer evidence. Ordinary project-file IO remains `NotDependencyLayer`.

## Source changes

- Lines 90-97 add the documented `FILE_IO_FAILURE_TITLE` constant for `File IO failure`. Its documentation explains why the title is ambiguous and must not be unconditional.
- Lines 99-108 add the documented `FILE_IO_PACKAGE_MARKERS` constant containing exactly:
  - `failed to unpack`
  - `/build/packages/`
  - `\build\packages\`
  - `/hex/hexpm/packages/`
  - `\hex\hexpm\packages\`
- Lines 137-143 add a title-anchored conditional branch to `classify_gleam_failure`. The title must come from a trimmed `error: <Title>` line, while marker evidence is searched across the full combined diagnostic body, matching the existing resolution-failure shape.
- The four entries in `REGISTRY_FAILURE_TITLES`, the `RESOLUTION_FAILURE_TITLE`/`RESOLUTION_TRANSPORT_MARKERS` pair, the function signature, `#[must_use]`, and the two-variant `GleamFailureLayer` enum were not changed.

## New table-driven pins

Lines 158-216 add one test, `recognised_dependency_layer_diagnostics_are_classified_from_realistic_output`, driven by a single `(name, fixture, expected)` table. Each assertion includes the row name in its failure message.

Rows:

1. P1: `Failed to download package` -> `DependencyLayer`.
2. P2: `Failure opening tar archive` -> `DependencyLayer`.
3. P3: `Hex API failure` -> `DependencyLayer`.
4. P4: `HTTP error` -> `DependencyLayer`.
5. P5: the issue shape, `File IO failure` plus `failed to unpack gleam@uri.erl` -> `DependencyLayer`.
6. P6: `File IO failure` with both `/build/packages/` and the real-character Windows form `\build\packages\` -> `DependencyLayer`.
7. P7: `File IO failure` with both `/hex/hexpm/packages/` and the real-character Windows form `\hex\hexpm\packages\` -> `DependencyLayer`.
8. P8: `File IO failure` over `/home/author/app/src/main.gleam`, with no package marker -> `NotDependencyLayer`.

Thus all five conditional markers are exercised: `failed to unpack` in P5, both project-local path forms in P6, and both global Hex-cache path forms in P7. I did not add an optional ninth row. The required eight rows cover the adjudicated behavior without adding behavior beyond the exact marker specification.

## Preserved tests

All eight pre-existing classifier tests remain as separate tests, unchanged in name and polarity, and all passed:

- `the_hex_outage_that_opened_this_task_is_a_dependency_layer_failure`
- `a_tar_archive_failure_is_a_dependency_layer_failure`
- `a_type_error_is_not_a_dependency_layer_failure`
- `an_unsatisfiable_requirement_is_not_excused_as_infrastructure`
- `a_resolution_failure_that_names_a_transport_fault_is_infrastructure`
- `a_source_snippet_quoting_a_registry_title_does_not_decide_the_classification`
- `download_progress_alone_is_not_a_dependency_layer_failure`
- `empty_output_is_not_a_dependency_layer_failure`

## Mutation check

I temporarily replaced the conditional `File IO failure` branch with an unconditional title match.

A preliminary command with an over-specific filter,

```text
cargo test -p aion-toolchain --lib recognised_dependency_layer_diagnostics_are_classified_from_realistic_output -- --exact
```

exited 0 but selected 0 tests because the exact filter omitted the module-qualified test path. I corrected the command rather than treating that as evidence.

The corrected mutation check was:

```text
cargo test -p aion-toolchain --lib recognised_dependency_layer_diagnostics_are_classified_from_realistic_output
```

It exited 101 and ran one test, which failed exactly on P8:

```text
assertion `left == right` failed: P8 project source file IO failure is not infrastructure
  left: DependencyLayer
 right: NotDependencyLayer

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 27 filtered out
```

The temporary unconditional mutation also produced the expected dead-code warning for `FILE_IO_PACKAGE_MARKERS`. I then restored the conditional branch and inspected `git diff`; the final committed diff contains only the specified conditional implementation and table.

## Verification transcript

### Required toolchain library tests

```text
cargo test -p aion-toolchain --lib
```

Final run: exit 0. Result: `28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. The output named all eight preserved classifier tests listed above and the new table-driven test as passing.

### Complete toolchain crate tests

```text
cargo test -p aion-toolchain
```

Exit 0. Results by target:

- library unit tests: 28 passed, 0 failed;
- `document_entry_e2e`: 4 passed, 0 failed;
- `no_embedded_compiler`: 1 passed, 0 failed;
- `toolchain_e2e`: 5 passed, 0 failed;
- doc tests: 0 tests, success.

### Downstream CLI tests

The brief's literal command was run:

```text
cargo test -p aion-cli --lib
```

Exit 101 with `error: no library targets found in package 'aion-cli'`. This package is a binary crate, so the requested library target does not exist.

I ran the valid binary-unit-test equivalent:

```text
cargo test -p aion-cli --bin aion
```

Exit 0. Result: `293 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. This includes the `generate` module's unit tests in the downstream consumer binary.

I also ran the broader command:

```text
cargo test -p aion-cli
```

Exit 101. The binary unit target passed all 293 tests and `assistant_verb_e2e` passed all 7 tests. `authoring_quality_e2e` passed 10 tests and failed 1 unrelated environment-dependent test, `input_skeleton_round_trips_through_the_workflow_input_codec`, because the installed Erlang lacks EUnit libraries. The observed diagnostic was:

```text
Error: EUnit libraries not found.

Your Erlang installation seems to be incomplete. If you installed Erlang using
a package manager ensure that you have installed the full Erlang
distribution instead of a stripped-down version.
```

No owned source change can install or alter the host Erlang distribution, and changing that integration test was outside the leg's hard wall.

### Clippy

The required workspace command was run:

```text
cargo clippy --workspace --all-targets -- -D warnings
```

Exit 101 due to a pre-existing/out-of-scope lint in `crates/aion-server/src/filesystem.rs:180`: `clippy::unnecessary_wraps` on `backend_path`. The hard wall forbids editing `aion-server`, and no lint bypass was added.

The owned crate was then checked directly:

```text
cargo clippy -p aion-toolchain --all-targets -- -D warnings
```

Exit 0; completed with no warnings.

### Formatting

```text
cargo fmt --check
```

Exit 0; no output.

### Forbidden-pattern scan

```text
grep -n "#\[allow\|#\[expect\|#\[ignore\|#\[deny\|cfg(any())\|unwrap()\|expect(" \
  crates/aion-toolchain/src/gleam_failure.rs
```

Exit 1 with no output, which is grep's expected result for no matches.

### Diff and status checks

```text
git diff --check
```

Exit 0; no whitespace errors.

Before the implementation commit, `git status --porcelain` showed only:

```text
 M crates/aion-toolchain/src/gleam_failure.rs
```

After writing this report and before its commit, the status check showed only the permitted report path. No other repository file was modified.

## Intentionally not done

- Did not edit `crates/aion-toolchain/src/compile.rs`. Its existing exhaustive match at lines 226-228 automatically receives the new `DependencyLayer` result.
- Did not edit `crates/aion-cli/src/generate.rs`. Its existing branch at lines 361-372 automatically receives the new classification; its binary unit tests passed via `cargo test -p aion-cli --bin aion`.
- Did not edit any of the five harness copies owned by the `copies` leg, the separate `aion-package` harness artifact, any integration test, manifest, dependency list, or other crate.
- Did not fix the out-of-scope `aion-server` clippy error or the host's missing Erlang EUnit installation.
- Added no lint bypasses, ignored tests, dependencies, enum variants, or new test files.
