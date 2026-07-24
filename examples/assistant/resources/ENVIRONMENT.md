# ENVIRONMENT.md — AWL preflight and workspace rules

AWL (`.awl`) is the authoring source of truth. Do not create a Gleam project,
`gleam.toml`, `workflow.toml`, generated codec, or `.aion` archive for an AWL
request. The current CLI compiles an AWL document directly for deploy and run.
Generated Gleam from `aion awl emit` is a toolchain view, not authored source.

## Workspace boundary

Your file tools are confined to the session workspace. Shell commands start in
that workspace and may read paths outside it, subject to host permissions.
Keep every file you author inside the workspace. If the workspace is a clone of
the aion repository, preserve unrelated changes and use the repository itself
as the authority. If it is scratch space, the embedded resources are available
at `.assistant/resources/`.

Before editing:

```sh
pwd
git status --short --branch 2>/dev/null || true
command -v aion
aion --version
aion awl --help
aion awl check --help
```

`aion awl check` is local: it parses and typechecks without contacting a
server. If `aion` is unavailable, report that AWL cannot be checked, compiled,
deployed, or run in this session. Do not substitute a Gleam workflow.

Gleam is not required to author or check `.awl` files. Git is useful only when
the requested work belongs in a repository. Do not install missing tools or
start a server as a workaround.

## Ground truth order

Use these sources in order:

1. The `.awl` file being edited and `aion awl check FILE.awl`.
2. `examples/assistant/resources/AWL-REFERENCE.md` in an aion checkout, or the
   materialized `.assistant/resources/AWL-REFERENCE.md` in every session.
3. Checked examples such as `examples/awl-hello/awl_hello.awl`,
   `examples/dev-brief/awl/dev_brief.awl`, and
   `examples/remediation-packet/awl/remediation_packet.awl`.
4. Checker-valid fixtures under
   `crates/aion-awl/tests/fixtures/rev2/**/valid/*.awl`.
5. The parser and checker in `crates/aion-awl/src/`, plus the CLI definitions
   in `crates/aion-cli/src/awl.rs`, `main.rs`, and `one_motion.rs`.

Design papers can explain intent, but accepted grammar plus checked files decide
what can be authored now. Never copy syntax only from a roadmap or draft.

## Server boundary

Checking, formatting, schema derivation, and emitting are local. `aion deploy`,
`aion run`, and observation commands contact the configured endpoint. Do not
assume the default endpoint is the operator's target, do not start or restart a
server, and do not claim a deploy or run succeeded unless the actual command or
ops console reported it. When server access is not explicitly available,
finish at a checked `.awl` file and state that deploy/run was not performed.

At the end, report the path of every authored `.awl` file, every local check you
ran, and whether deploy/run was performed or deliberately left to the operator.
