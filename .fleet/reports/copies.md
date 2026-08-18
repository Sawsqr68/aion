# Leg `copies` report

## Changes

Code commit: `c759dd7f` (`test: classify package file IO failures`).

Changed exactly the five owned classifier copies:

- `crates/aion/tests/test_support/gleam.rs`
- `crates/aion-cli/tests/test_support/gleam.rs`
- `crates/aion-awl-package/tests/test_support/gleam.rs`
- `crates/aion-server/tests/test_support/gleam.rs`
- `crates/aion-awl/tests/test_support/gleam.rs`

In the four byte-identical Variant A files, I added `FILE_IO_FAILURE_TITLE` and `FILE_IO_PACKAGE_MARKERS` at lines 81-95, added the marker-qualified branch to `classify_text` at lines 364-370, and added the eight-row table-driven pin `dependency_failure_titles_are_classified_by_layer` at lines 521-575. The four pre-existing unconditional registry titles and the conditional dependency-resolution handling are unchanged. The table asserts through `is_dependency_layer_failure` and names each row in its failure message.

In Variant B (`aion-awl`), I added the same semantics in its own vocabulary: constants at lines 60-74 describe generated-project IO and AWL emitter faults, the conditional classifier branch is in `classify_text`, and the eight-row pin asserts via `matches!(classify_text(...), BuildFailure::Registry)`. It remains deliberately divergent in API and prose; it was not copied over from Variant A.

The five File-IO markers are covered as follows in both variants:

- `failed to unpack`: row `failed package unpack`, using the issue shape `failed to unpack gleam@uri.erl`.
- `/build/packages/` and `\build\packages\`: row `project package cache paths` (called `generated-project package cache paths` in Variant B), with POSIX and actual Windows-shaped path characters in one realistic diagnostic.
- `/hex/hexpm/packages/` and `\hex\hexpm\packages\`: row `global Hex package cache paths`, again with POSIX and Windows-shaped cache paths.
- The negative row is `ordinary source file` / `ordinary generated source file`; it contains `error: File IO failure` over `src/awl_hello.gleam` and no package marker, and expects Sources.

All five existing classification tests remain present and unchanged, including both dependency-resolution polarities.

## Verification transcript

### Shared-copy identity

Command:

```text
md5sum crates/aion/tests/test_support/gleam.rs \
  crates/aion-cli/tests/test_support/gleam.rs \
  crates/aion-awl-package/tests/test_support/gleam.rs \
  crates/aion-server/tests/test_support/gleam.rs
```

Exit 0. All four hashes were identical:

```text
337c4565f47a8adbffb79058f7d82fd3
```

### Required integration-test includers

`cargo test -p aion-awl --test emitter` — exit 0: **41 passed, 0 failed, 0 ignored**. The new table pin and all five existing classifier pins passed. Gleam was available; the compile proofs actually ran and printed successful `COMPILED ... under real gleam build` messages.

`cargo test -p aion --test hello_world_e2e` — exit 101 before compilation because the brief's package identifier is stale: Cargo reported that package `aion` does not exist. `cargo metadata` showed that `crates/aion/Cargo.toml` is package `aion-rs`, so I ran the corrected command below.

`cargo test -p aion-rs --test hello_world_e2e` — exit 101: **9 passed, 1 failed**. Every Gleam-support test passed, including:

- `dependency_failure_titles_are_classified_by_layer`
- `the_shared_gleam_support_copies_have_not_diverged`
- `every_gleam_support_file_is_classified`

The unrelated `hello_world_runs_end_to_end` failed because the runtime rejected `gleam@dynamic@decode...` with `BEAM module validation failed: instruction 392: Y register index 5 is outside frame size 1`. I did not edit runtime/BEAM code because it is outside this leg's five owned files.

`cargo test -p aion-cli --test generate_e2e` — exit 0: **11 passed, 0 failed, 0 ignored**. The new table pin, both copy/classification guard tests, and all existing classifier pins passed.

`cargo test -p aion-server --test deploy_api_e2e` — exit 0: **16 passed, 0 failed, 0 ignored**. The new table pin, both copy/classification guard tests, and all existing classifier pins passed.

`cargo test -p aion-awl-package --test sdk_closure_freshness` — exit 101: **12 passed, 1 failed**. Every Gleam-support test passed, including the new table and both copy/classification guard tests. The unrelated `committed_closure_matches_a_fresh_oracle_harvest` failed because committed BEAM closure bytes differ from the current toolchain harvest while the wire surface matches. Its message asks for `scripts/regen-awl-sdk-closure.sh`; I did not regenerate or commit closure artifacts because they are outside this leg's allowed files.

After reverting the mutation check, I also ran the new pin directly in both variants:

```text
cargo test -p aion-awl --test emitter gleam_test_support::dependency_failure_titles_are_classified_by_layer -- --exact
cargo test -p aion-cli --test generate_e2e gleam_test_support::dependency_failure_titles_are_classified_by_layer -- --exact
```

Both exited 0 with **1 passed, 0 failed** (40 and 10 filtered out respectively).

### P8 mutation check

I temporarily changed `File IO failure` to unconditional Registry in both variants and ran the exact new table test. Both commands exited 101 as required.

Variant B observed failure:

```text
assertion `left == right` failed: classification row `ordinary generated source file` returned the wrong layer
  left: true
 right: false
```

Variant A observed failure:

```text
assertion `left == right` failed: classification row `ordinary source file` returned the wrong layer
  left: true
 right: false
```

I restored the marker-qualified branches, recopied Variant A to its three shared siblings, re-ran both exact pins successfully, ran `git diff --check`, and re-proved the identical MD5 hashes before committing.

### Formatting and lint

`cargo fmt --check` — initial exit 1 identified one rustfmt line-wrap in Variant B; I applied it. Final `cargo fmt --check` exited 0.

`cargo clippy --workspace --all-targets -- -D warnings` — exit 101 on an unrelated pre-existing/out-of-scope diagnostic:

```text
error: this function's return value is unnecessarily wrapped by `Result`
  --> crates/aion-server/src/filesystem.rs:180:5
180 | pub(crate) fn backend_path(&self) -> io::Result<PathBuf>
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
error: could not compile `aion-server` (lib) due to 1 previous error
```

I did not alter `crates/aion-server/src/filesystem.rs`; the brief permits changes only to the five test-support copies and this report, and forbids lint suppressions.

The brief's exact forbidden-attribute grep exited 0 with matches only in existing documentation comments saying ``never `#[ignore]` `` (Variant A line 181 and Variant B line 96). Inspection confirms no `#[allow]`, `#[expect]`, `#[ignore]`, `#[deny]`, or `cfg(any())` attribute was added or exists as code in the five files. I did not rewrite that pre-existing prose merely to force the literal grep to return empty, because doing so is unrelated churn in all shared copies.

`git diff --check` — exit 0.

Before the code commit, `git status --short` and `git diff --name-only` showed only the five owned source files. The final status after this report commit is clean.

## Deliberately not done

- I did not touch `crates/aion-toolchain/src/gleam_failure.rs`; it belongs to the production leg.
- I did not touch `crates/aion-package/tests/test_support/gleam.rs`, any copy-classification list, runner/retry gate, closure artifact, runtime code, or server filesystem code.
- I did not converge the `aion-awl` ancestor onto Variant A.
- I did not edit the stale sentence in `DIVERGENT_COPIES` claiming the `aion-awl` file has no dependency-layer classification. That sentence is inaccurate, while the signature and emitter-vocabulary rationale remains accurate; the brief explicitly says to preserve the prose and record the stale statement here.
- The file-length validator warned that the already-large shared test-support file exceeds its configured 500-LOC preference. Extracting a module or adding a file is explicitly prohibited by this leg, so no structural refactor was attempted.
