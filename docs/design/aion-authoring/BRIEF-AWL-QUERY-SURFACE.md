# Brief: the AWL query surface (task #96)

## Why this exists

On 2026-07-30 a workflow that had been parked for twenty-eight days was proven
alive by one command:

```
POST /workflows/query {"query_name":"assistant_status"}
→ {"result":{"phase":"awaiting_operator","round":2}}
```

That is the single most valuable operator affordance the engine has: it reads
live in-memory workflow state, appends no event, and answers instantly.

**The workflow that answered it was the Gleam `assistant`. The AWL port of the
same workflow cannot answer it, because AWL has no query surface at all** —
there is no `query` token in `crates/aion-awl/src/lexer/tokens.rs`, and none of
the 55 keywords is `query`.

So the AWL port is a capability *regression* against the Gleam original on the
one axis an operator cares about most: can I ask a running workflow what it is
doing. Closing that is this brief.

## What already exists (do not rebuild any of it)

The whole runtime path is built, shipped, and proven in production. This is a
LANGUAGE gap, not an engine gap.

- **Engine**: `crates/aion/src/query/service.rs` (`QueryService::query`,
  `query_process`), `crates/aion/src/engine/delegated.rs:293`.
- **Transport**: `POST /workflows/query`, and the gRPC equivalent.
- **Gleam SDK**: `gleam/aion_flow/src/aion/query.gleam` —
  `query.handler(name, codec, fn() -> value)` and `query.dispatch(name, codec)`,
  with the Erlang pump in `aion_flow_query_pump.erl`.
- **A worked reference**: `examples/assistant/src/assistant.gleam:212-231`.

Read that reference before designing anything. The load-bearing detail is that
the Gleam workflow **re-registers the handler at every stage**
(`set_status(phase, round)`), so the closure captures the state as of that
point. Queries are answered at engine yield points from whatever was last
registered.

## The design (ruled — build this, do not re-litigate the shape)

Two additions, each with an exact existing precedent in the language.

### 1. `query <name>: <Type>` — a header declaration

Sibling of `signal <name>: <Type>`. Follow `SignalDecl` in
`crates/aion-awl/src/ast/document.rs:80-96` exactly: same parse position (the
workflow header), same span capture, same duplicate-name checking, same
appearance in the derived schema and the printed canonical form.

The declared `Type` is the query's ANSWER type. It must resolve to a declared
record type in the document, the same way a signal payload type does.

### 2. `answer <name>(field: expr, …)` — a statement

This is the re-registration. It is a statement inside a step body, and it is
the direct analogue of `set_status(phase, round)` in the Gleam reference.

The closest existing precedent in the codebase is `send`, which was added for
task #24 and lowers in `crates/aion-awl/src/emitter/sends.rs`. Follow its
shape: a statement that names a header-declared name, constructs a record from
named arguments over bindings in scope, and lowers to one runtime call.

Semantics, stated so the checker can enforce them:

- **The answer value is computed AT THE STATEMENT, from bindings in scope** —
  not at query time. This is what keeps AWL deterministic: no closure over
  mutable state, no evaluation at an arbitrary yield point, nothing for replay
  to disagree about. The statement evaluates a pure expression and registers
  the resulting VALUE.
- **`answer` may appear anywhere a statement may**, including inside loops.
  Inside a loop it re-registers each iteration, which is exactly the live-state
  behaviour the Gleam version gets from per-stage re-registration.
- **`answer` records no event.** It is not an effect and must not appear in
  history.
- **A query declared but never answered** must be a CHECKER ERROR, not a
  runtime surprise. An operator querying an unanswered name gets `UnknownQuery`
  from the engine, which reads as "this workflow does not have that query" when
  the truth is "the author declared it and never set it". Refuse at check time.
- **A query answered but not declared** is likewise a checker error.

### Naming

`answer` is the proposed keyword. If — and only if — you find a concrete
grammar conflict with the existing 55 keywords or with a field/binding name in
any checked-in `.awl` fixture, propose an alternative WITH the conflict
evidence rather than switching silently.

## What "done" means

1. Lexer, parser, AST, checker, canonical printer, schema derivation, and the
   direct-BEAM emitter all handle both constructs. A partial vertical slice is
   not done.
2. `aion awl fmt` round-trips a document using both, byte-identically.
3. `aion awl schema` includes the query name and its answer type.
4. The checker refuses: declared-never-answered, answered-never-declared,
   duplicate query names, an answer whose field set does not match the declared
   type, and an answer referencing a binding not in scope at that point.
5. **`examples/assistant/awl/assistant.awl` gains its status query**, matching
   the Gleam original's `assistant_status` in name and in answer shape
   (`{phase, round}`). This is the acceptance case — the port stops being a
   regression.
6. Tests assert the INVARIANT, not an enumeration. Where a test could pass
   vacuously, include the control: a document WITHOUT a query must produce no
   query registration, and that must be asserted, or the positive test only
   proves the code can fire, not that it discriminates.

## Constraints (house rules, non-negotiable)

- No `#[allow]`, `#[expect]`, `#[ignore]`, `_var` renames, or `#[cfg(any())]`.
  If clippy fires, fix the code.
- No file over 500 lines of non-test, non-comment code. `mod.rs` holds only
  `pub mod` and re-exports.
- No `.unwrap()` / `.expect()` / `panic!` in library code.
- No backwards-compatibility shims. Replace, do not add alongside.
- No arbitrary limits or assumed defaults. If a value wants configuring, raise
  it rather than picking one.

## Gates before you report

Run and report the exact exit codes, never an `echo` after the command:

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Report the PASSED/FAILED counts, not "green". If a gate fails, say so with the
output. **Commit your work** — an uncommitted green tree is not a landing.

## Working rules

- Work directly in `/Users/tom/Developer/ablative/stack/aion` on `main`. NO
  worktrees.
- `git commit -- <paths> -m msg` FAILS in this repo. Use `git commit -F <file>`
  or a heredoc.
- Do not restart or kill the running server (pid 99844) or worker (pid 35591).
  A live AWL assistant session is served by them.
- Do not rebuild `target/release/aion`.
