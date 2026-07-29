# Declared commands: what they are, and what they are not

An AWL action can carry a body:

```
worker shell_box
  action stamp(text: String) -> RunOutcome
    run "echo $text"
```

The server executes that command directly. No worker process, no Rust crate.

This document exists because that is a **capability**, and a capability that
looks like a sandbox and is not one is worse than no sandbox at all — an
operator who believes they are protected takes risks they would never take
knowingly. So the boundaries are written down here in full, including the ones
that do not exist.

## THERE IS NO SANDBOX

`run "rm -rf /"` is legal. So is `run "curl https://example.com/x | sh"` — not
because a pipe works (it does not; see below) but because `curl` and `sh` are
programs and nothing stops you naming them.

There is:

- **no allowlist** of permitted programs;
- **no denylist** of forbidden ones;
- **no filesystem confinement** — a declared command reads and writes anything
  the server process can;
- **no network restriction**;
- **no user or privilege drop** — the command runs as whoever runs the server;
- **no resource limit** — no CPU, memory, file-descriptor, or wall-clock cap
  beyond the activity's own timeout and cancellation.

A declared command can do anything the server's operating-system user can do.

**What protects you is review, not runtime enforcement.** A declared command is
source code in a `.awl` file. It is read at authoring, it is checked at compile,
and it is sealed into the package's identity at deploy. The control is that you
can see exactly what will run before you deploy it, and that nobody can change
it afterwards without producing a different package. That is a real and strong
control. It is not containment.

If you need containment, put it where containment belongs: run the server as a
constrained user, in a container, or on a host that has nothing else on it.

## What IS guaranteed

These are properties the implementation enforces, each with a test that would
fail if it regressed.

### A parameter value can never become code

A declared command is parsed ONCE into an argv whose elements are literal text
or named parameter holes. A value is substituted as one WHOLE argv element and
never re-enters a parser.

```
run "echo $text"      with text = "; rm -rf /"
                      → echo receives ONE argument: "; rm -rf /"
```

`rm` does not run. Neither does anything else a value contains. There is no
shell between the declaration and `execve`, so a value has nothing to escape
into. Nothing is blacklisted because nothing needs to be.

The corollary is worth stating plainly: **shell syntax in the COMMAND does not
work either.** `run "a | b"` passes `|` and `b` as arguments to `a`; it does not
build a pipe. `run "x > f"` does not redirect. An author who wants a shell must
name one — `run "sh -c 'a | b'"` — and doing so is visible in review, which is
the point.

### A caller can never choose which program runs

`run "$program arg"` is refused at check time, and refused again by the
executor. The program is always literal text.

### The command is sealed into package identity

The command text is committed into the package's content hash under the
`aion.package.version.worker-contract.v5` domain. Rewriting a deployed
archive's command, changing one character of it, or deleting the body to
downgrade the action into one a worker serves — all three make the package
fail to load with an integrity refusal.

That is proven by breaking it, not by reading the encoding:
`crates/aion-awl-package/tests/declared_command_sealing.rs` assembles a real
archive, edits `contract.json` inside it, and requires the load to refuse — with
a no-edit repack as the control, so a refusal cannot be an artefact of
repacking.

### The command inherits nothing it was not given

- **Environment: cleared.** The server's environment does not cross. `PATH` is
  the sole exception, and only so that a bare program name resolves; an operator
  supplies anything else explicitly. A deployed package cannot read the server's
  credentials with `run "env"`.
- **stdin: closed.** Never the host's. `run "cat"` reads end-of-file
  immediately rather than blocking the activity forever on a terminal.
- **Working directory: explicit or inherited.** Set it whenever a command names
  a relative path — absent, the command inherits the host's cwd, which is a
  property of how the server was launched and not something a package author can
  see.

### Cancellation kills the whole tree

The command runs in its own process group. Cancelling the activity terminates
that group (SIGTERM, then SIGKILL after a grace period), so a command that
spawned children does not leave them running.

### Values are delivered exactly, or refused

Newlines, tabs, empty strings, leading and trailing whitespace, quotes, and
backslashes all arrive as exactly one argument. A NUL byte is **refused**
rather than truncated, because a NUL would silently cut the argument short at
the kernel boundary and change what the program receives. A null, array, or
object value is refused rather than guessed at.

### One thing the executor deliberately does NOT do

A value that begins with `-` is delivered faithfully as one argv element — and
the program's own argument parser decides whether to read it as a flag.

```
run "printf %s $value"        with value = "-n"
                              → printf decides; the executor does not
run "printf -- [%s] $value"   with value = "-n"
                              → unambiguously a value
```

The executor cannot fix this without silently rewriting what the author wrote.
The author closes it by writing `--` in the declaration, exactly as they would
in any script. Both halves are pinned in the test suite so the behaviour is
recorded rather than discovered.

## What happens on failure

A non-zero exit is **retryable by default** — the common causes (a busy
resource, an unreachable host, a transient permission state) succeed on a
second attempt — and the failure carries the command's own stderr, because a
failing command's own words are the most useful thing an operator can be handed.
An action whose command must never run twice declares its non-zero exits
terminal instead.

A missing parameter, an unrepresentable value, a program that cannot be spawned,
and a cancellation are all **terminal**: retrying cannot change any of them, and
retrying a command whose termination could not be confirmed risks running it
twice at once.

## Where the code is

| Piece | File |
|---|---|
| Command parsing and argv substitution | `crates/aion-worker/src/shell/template.rs` |
| Execution, environment, failure classification | `crates/aion-worker/src/shell/action.rs` |
| Registration as an activity | `crates/aion-worker/src/shell/registry.rs` |
| Server-side execution of a deployed body | `crates/aion-server/src/worker/declared_body.rs` |
| Authoring-time checks | `crates/aion-awl/src/checker/bodies.rs` |
| Identity sealing | `crates/aion-package/src/contract.rs` |
| Sealing broken, not read | `crates/aion-awl-package/tests/declared_command_sealing.rs` |
| Live proof | `docs/evidence/awl-declared-command-live-run.md` |
