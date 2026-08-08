# #136 AWL template notes

## Decisions

- Added `Template::Awl` with id `awl` and made it the `aion new` default.
- Reused the existing shared `.gitignore` + `aion.toml` subset by naming it `SHARED_AWL_FILES`; the AWL family does not fork either shared file.
- Kept the scaffold minimal: `.gitignore`, `aion.toml`, `<name>.awl`, and `README.md`. It emits no Gleam manifest, workflow descriptor, schema directory, or source directory.
- The AWL document has typed `String`/`Bool` input, a typed `Greeting` output, three pure steps, and one `when`/`otherwise` decision. It declares no worker or actions and runs to completion in the native AWL VM.
- Extracted the existing inline scaffold/template test modules into `scaffold_tests.rs` and `template_tests.rs`; this keeps every touched Rust file below 500 lines while preserving the prior tests.

## Name substitution and validation

`scaffold.rs` does not sanitize or transform names. It validates the supplied name, then substitutes it byte-for-byte with `.replace("{{name}}", name)` in both manifest paths and contents. The existing common rule is: the first character must be an ASCII lowercase letter; every remaining character must be an ASCII lowercase letter, ASCII digit, or underscore; the exact name `gleam` is also refused. For the AWL template, the rendered name must additionally lex as one `TokenKind::Identifier`; AWL reserved words such as `workflow`, `step`, and `type` are refused before the target directory is created, with an error that names the AWL lowercase-snake-case/non-reserved-word rule.

## Checker cross-check

The in-process workspace `aion-awl` parser/checker reports zero diagnostics for the embedded template after `{{name}} -> demo_flow`, and its canonical printer is byte-identical to the embedded bytes. Installed `aion 0.11.0` also accepted the hand substitution `{{name}} -> installed_demo`; there is no checker divergence. See `installed-aion-awl-check.log`.

## Gate observations

- `cargo fmt --all` and `cargo fmt --all --check`: green.
- `cargo clippy -p aion-cli --all-targets -- -D warnings`: green on the final rerun.
- `cargo clippy --workspace --all-targets -- -D warnings`: green.
- The new live AWL e2e is green and proves the no-flag default scaffold, three ephemeral config addresses, direct deploy, one-motion run, `greeted` completion, and `{"message":"Hello, Ada!"}` output.
- The full `cargo test -p aion-cli` gate reached and passed all 217 library tests, including the new in-process AWL canonical/check test, AWL reserved-word refusal, and worker refusal parity. It then failed in the pre-existing `dev_hot_reload_e2e`: that test invokes `aion new flow` without `--template hello-world` while its remaining logic assumes a Gleam project. The isolated rerun failed identically (`no new loaded version appeared within the reload deadline`). This is a direct stale-old-default assumption, not load contention. Other pre-existing files found with the same old-default assumption include `new_hello_world_e2e.rs` and `generate_types_first_e2e.rs`.
- The brief fences `crates/aion-cli/tests/` to the new `new_awl_e2e.rs` only, so those existing e2es were not edited. They need explicit `--template hello-world` (and the types-first template intended by the generate gate) in the lane that owns those files. Logs preserve the full failure and isolated rerun evidence.

No design ruling was intentionally deviated from in production code or the new regression tests.
