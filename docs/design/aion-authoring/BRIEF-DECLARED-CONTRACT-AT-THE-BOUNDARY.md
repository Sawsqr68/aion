# Brief: enforce the declared contract at the boundary (tasks #95 + #97-A)

## These are one defect, not two

An AWL document declares its input type and its signal payload types. Both
declarations reach the package contract. **Neither is checked where the value
enters the system**, and the consequences land in two different places:

- **#95 — start.** A workflow's declared `input_schema` may not be enforced at
  start, so a start with wrong-shaped input is admitted and fails later, inside
  the run.
- **#97-A — signal.** A signal payload is recorded opaquely and decoded later,
  *inside the workflow process*. A decode failure fails the run.

Same shape: the contract is known at the boundary and consulted past it. Fix
them together or the second one will be rediscovered.

## Why #97-A is the severe one

Traced end to end and cited in `ASSISTANT-AWL-PORT.md` §3:

- `ConcreteSignalRouter::route` (`crates/aion/src/signal/router.rs:35-60`)
  records the payload opaquely — no declared-type table on the path.
- The HTTP handler
  (`crates/aion-server/src/api/handlers/workflows.rs:166-204`) and the CLI
  (`crates/aion-cli/src/main.rs:686-709`) validate only that the payload is
  well-formed JSON.
- **The arrival is marked consumed BEFORE the decode is attempted**
  (`crates/aion/src/runtime/nif_signal.rs:338-341`). It is gone either way.
- The decode failure becomes `Error(AwlSignalFailed(...))` and `TryBind`
  short-circuits the step. **The run fails.**

So: **an operator typo kills a live durable workflow.** A session that has been
parked for months — the exact thing this engine exists to make possible — is
destroyed by a malformed JSON body, and the body is consumed on the way out so
there is nothing to retry.

The Gleam SDK's own assistant module states the opposite as a requirement:

> A payload that does not decode at all is ALSO tolerated as a no-op — an
> operator typo must never kill a live session.

AWL cannot reproduce that today. The only lever is a step-level `on failure`,
and `emit_with_failure` (`crates/aion-awl/src/emitter/failure.rs:102-107`)
matches `Error(_)` indiscriminately — it would swallow a genuine agent-round
failure just as readily. **Trading a loud failure for a silent one is worse
than the problem**, which is why the assistant port deliberately did not use
it.

## The design (ruled)

**Validate the declared payload against the declared type AT THE BOUNDARY,
before anything is recorded or consumed.**

The signal types are already in `PackageContract::signals`, and the input
schema is already in `PackageContract::input_schema`. The server resolves the
package version on both paths. So the data is present; it is simply not
consulted.

Concretely:

1. **Signal**: before `ConcreteSignalRouter::route` records anything, resolve
   the target run's package contract, find the declared type for
   `signal_name`, and validate the payload against it. A mismatch is a typed
   refusal returned to the CALLER — `POST /workflows/signal` answers 400 with
   the field that did not match — and **nothing is recorded and nothing is
   consumed**. The run stays parked exactly where it was, and the operator sees
   their own mistake at the moment they made it.
2. **Start**: the same check against `input_schema`, so a wrong-shaped start is
   refused at the boundary rather than dying inside the run. Verify first
   whether this is genuinely unenforced — #95 is stated as "may not be", not
   "is not". **Establish which it is before building; if it is already
   enforced, say so and close #95 rather than adding a second check.**
3. **An unknown signal name** for the resolved contract is likewise a refusal,
   not a silently recorded no-op.

### The rule this must obey

A refusal must never be able to consume the arrival. The current ordering —
consume, then decode — is what makes the failure unrecoverable. Whatever the
implementation, assert that ordering in a test: a rejected payload leaves the
run's history byte-identical and the workflow still parked on the same wait.

### Explicitly NOT in scope

Do not add a blanket `on failure` catch, and do not change
`emit_with_failure`'s matching. A typed failure-class catch in AWL is a
reasonable future feature and is a separate decision; it is not the fix for
this, and reaching for it here would convert run-killing failures into silent
ones.

## Acceptance

Not a green test — a demonstrated behaviour:

1. Start an AWL workflow that parks on a signal.
2. Send a malformed payload for a declared signal. The API refuses with a
   message naming the mismatch. **Read the run's history: unchanged. Query the
   run: still parked.**
3. Send a valid payload. It proceeds normally.
4. Send an undeclared signal name. Refused, history unchanged.

Do this against a real server and quote the recorded bytes, the way
`ASSISTANT-AWL-LIVE-PROOF.md` does. `examples/assistant/awl/assistant.awl` is
the natural subject — it parks on exactly one signal.

## Constraints

House rules in `CLAUDE.md` are binding: no `#[allow]`/`#[expect]`/`#[ignore]`,
no `unwrap`/`expect`/`panic` in library code, no file over 500 non-test lines,
no compat shims, no arbitrary limits or assumed defaults. Every error handled
or propagated — a validation path that swallows its own failure is the same
class of bug as the one being fixed.

Gates: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace`. Report exact exit codes and PASSED/FAILED
counts. Never end a gate command with `echo` — you would capture the echo's
status.

## Working rules

- Work directly in `/Users/tom/Developer/ablative/stack/aion` on `main`. NO
  worktrees. **Only one lane may hold `crates/aion-awl` at a time** — check
  before starting.
- `git commit -- <paths> -m msg` FAILS here. Use `git commit -F <file>`.
- Do not restart the server (pid 99844) or worker (pid 35591).

---

## Sibling, NOT in this lane: #97-B, optional fields are write-only

Recorded here so it is not lost, and kept separate because it is a type-system
change rather than a boundary change.

`is present` does not narrow a bare optional scalar. Verified against the
checker (`ASSISTANT-AWL-PORT.md` §4):

| Expression | Checker says |
| --- | --- |
| `nudge.end` where `end: Bool?` | `` `until` needs a Bool condition, found Bool? `` |
| `nudge.end is present and nudge.end` | `` `and` needs Bool operands, found Bool? `` |
| `nudge.message == ""` where `message: String?` | `` `==` needs matching operands, found String? and String `` |

Field access *through* an optional record unwraps one level, which is why the
`guard_optional_wait.awl` fixture appears to narrow — but a bare optional
scalar stays optional forever. The `View` type does carry a `narrow` slot
(`crates/aion-awl/src/checker/exprs.rs:14-23`), and `operators.rs` populates it
for one name, so the machinery half-exists; `blocks.rs:102` and
`walk.rs:291` pass `narrow: None`, so it never reaches a statement body.

Consequence: `T?` is effectively write-only. The assistant port had to declare
both signal fields REQUIRED, which changed the operator contract — `{"end":
true}` alone, valid against the Gleam original, now fails the round.
