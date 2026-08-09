# Declared commands: what they are, and what they are not

An AWL action can carry a body:

```awl
type RunOutcome { exit_code: Int, stdout: String, stderr: String }

worker shell_box
  action stamp(text: String) -> RunOutcome
    run "echo $text"
```

The server executes that command directly. No worker process, no Rust crate.

The result of a body is always those three members — the action's return type
names them, and it may name a subset if the run's exit status or its stderr is
not what the workflow goes on to use.

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
- **no resource limit** — no CPU, memory, file-descriptor, or wall-clock cap of
  any kind. Not "none beyond the timeout": a declared `run "…"` has **no
  external bound at all**. A per-activity `timeout` is carried to the engine as
  `timeout_ms` and read by nothing, and cancellation of a declared body is not
  wired (both stated in full below). Once the command is spawned, the only
  thing that ends it is the command itself.

A declared command can do anything the server's operating-system user can do,
for as long as it likes.

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

### Inside single quotes there is no `$$` escape — and getting it wrong is SILENT

A declared command has three regions, and the dollar means something different
in each:

| Region | `$name` / `${name}` | `$$` |
| --- | --- | --- |
| bare | a parameter | one literal `$` |
| `"double quotes"` | a parameter (the quotes only group the word) | one literal `$` |
| `'single quotes'` | **literal text** — no substitution | **two literal `$`** |

Single quotes take their contents literally, *including the dollar*. So inside
them the escape is not merely unnecessary, it is wrong: `$$` passes through
unchanged, and `sh` then reads it as **its own process id**.

Each row is pinned by a test in `crates/aion-worker/src/shell/template.rs` —
`single_quotes_are_literal_including_dollar_signs` (`echo '$name'` renders the
argument `$name`, unsubstituted) and `a_doubled_dollar_is_a_literal_dollar`
(`echo $$name` renders `$name`). The bare and double-quoted regions share one
code path, `push_parameter`, which is why they behave alike.

That is why the `{workspace_root}` example below writes `"$1/$2"` and not
`"$$1/$$2"`. Those are `sh`'s positional parameters — already literal because
they sit inside single quotes — and they are how a parameter reaches the script:
pass it as a word after the script, then refer to it positionally.

```
run "sh -c 'echo \"$1\"' -- $path"     ✅ $path arrives as sh's $1
run "sh -c 'echo \"$$1\"' -- $path"    ❌ sh reads $$ as its pid, then a bare 1
```

**Nothing catches the second form.** It is valid AWL, it passes `aion awl
check`, and `$$` is valid shell — so the command runs, exits 0, and reports
success while having globbed against a directory named after a process id and
redirected into a file of the same name.

A measured instance is written up in
[`DECLARED-BODY-FINDINGS-2026-08-05.md`](../design/aion-authoring/DECLARED-BODY-FINDINGS-2026-08-05.md):
the run returned `exit_code: 0` with empty output, wrote nothing to the path the
author named, and left a pid-named file in the server's working directory. Its
result payload was byte-identical to the correct run's.

The lesson generalises past this one escape: for a bodied action **the exit
status is not an oracle.** Check the effect the command was written to have.

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

### Changing a body makes a NEW version, and a run in flight keeps its own

Because the command is part of the identity, editing it and deploying again
replaces nothing: content-hash namespacing retains BOTH versions, and both
declare the action. That is not a conflict, because **a body is a property of
the run's package version, not of the task queue.** A run is pinned to exactly
one version for its whole life, and that version's contract is the sole
authority on what its actions execute:

- A run started before the redeploy keeps executing the OLD command, to the end
  — including a run that was asleep for a month. It will never execute the new
  version, so the new version's body is not its business.
- Every run started after the redeploy gets the new command, because the new
  version is the one `aion deploy` makes route-active.

So editing a body and deploying is safe while work is in flight. Deploy when you
need to; nothing already running is disturbed:

```
aion deploy FILE.awl   # the new version becomes route-active; new starts use it
aion run FILE.awl --input …
```

Retiring the superseded version is housekeeping, not a fix. It is `aion unload
<type> <old-hash>`, and the deploy has to come first because **a route-active
version cannot be unloaded** — the deploy API answers `RouteActive`. `aion
deploy` prints `superseded_versions`, so the hashes do not have to be hunted
for. Unloading a version a run is still using will not silently change that
run's body; there is no version left to read one from, and the action is
delegated to a worker instead.

Selection is in `crates/aion-server/src/worker/declared_body_selection.rs`,
pinned there by `an_in_flight_run_keeps_its_own_versions_body_after_a_redeploy`.

#### When the disagreement IS refused

One case remains where two retained versions declaring different bodies stops a
dispatch: when the server cannot name which version the run is pinned to — its
registry handle is gone, or the registry could not be read at all. Without the
run's identity, choosing a body would be exactly the guess the refusal exists to
prevent, so the dispatch refuses terminally and names what to retire (hashes
shortened here for reading; the real message prints them in full, because the
deploy API refuses a short one):

> terminal:action `find_repositories` on task queue `local` declares 2 different
> bodies across 2 retained package versions; refusing to guess which deploy this
> run meant. The route-active version already carries the body new starts use;
> retiring the superseded ones is what leaves a single body. Redeploying adds a
> version rather than removing one, so unload instead, then start the run again:
> `aion unload git_status_sweep 1f2e…`. Kept, because new starts route to it:
> git_status_sweep@ab34….

Two route-active packages that declare the same action name with different
commands are a different situation: there is no superseded version to retire, so
the refusal says so and asks for the documents to be reconciled instead. The
text is generated in
`crates/aion-server/src/worker/declared_body_ambiguity.rs` and each branch is
pinned by a test in that file.

### The command inherits nothing it was not given

- **Environment: cleared.** The server's environment does not cross. `PATH` is
  the sole exception, and only so that a bare program name resolves; an operator
  supplies anything else explicitly. A deployed package cannot read the server's
  credentials with `run "env"`.
- **stdin: closed.** Never the host's. `run "cat"` reads end-of-file
  immediately rather than blocking the activity forever on a terminal.
- **Working directory: the server's own.** A body executed by the server runs in
  whatever directory the server was launched from
  (`crates/aion-server/src/worker/declared_body.rs` builds a `ShellAction` and
  sets no working directory). That is a property of how the operator started the
  process, and a package author cannot see it — so **never write a relative path
  in a declared body.** Use `{workspace_root}` (below) or an absolute path. The
  worker SDK's own `ShellAction` API does let an embedder set a working
  directory; the server-run path does not use it.

### Cancellation of a declared body is NOT wired

Read this one carefully, because the surrounding machinery makes it look
otherwise. The worker SDK builds an `ActivityContext` with a cancellation
handle, and the server's dispatcher constructs one — but **nothing cancels a
single declared-command attempt today**. The handle is held and dropped after
the command returns, deliberately, so that a future wiring cannot race a handle
that died early. The code says so at the drop site.

What this means in practice: a declared body runs to completion (or to its own
failure) once it has started. Cancelling the workflow does not reach into a
running `run "…"`. Do not rely on cancellation to bound a command — bound it in
the command, with the program's own timeout.

**Nor does a declared `timeout` bound it.** A per-activity timeout written in
AWL, or attached in the Gleam SDK, travels to the engine inside the dispatch
config as `timeout_ms` — and nothing on the Rust side reads that field. It is
carried, not consumed. So the two things an author would reach for to put a
ceiling on a declared body — cancellation and a timeout — are both inert, which
is why "bound it in the command" is the whole of the advice and not the
cautious version of it. The workflow-level `timeout_seconds` still bounds the
RUN, but it does not stop a command already spawned.

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

## `{workspace_root}` — how a body learns where to work

A declared body runs with its environment cleared to `PATH` alone, so it cannot
expand `~`, read `$HOME`, or consult a variable to find out where on disk it is
allowed to work. The workflow's start input cannot carry the location either:
where session workspaces live is a property of **the server**, not of any one
run, and putting it in the start contract would mean every caller had to know
it and every recorded history would carry a caller's guess.

So the server states it. A declared command may carry the literal placeholder
`{workspace_root}`, and the server replaces every occurrence with its own
resolved workspace root before the command is parsed:

```awl
type Provisioned { stdout: String }

worker provisioner
  action provision(run_id: String) -> Provisioned
    run "sh -c 'mkdir -p \"$1/$2\" && printf %s \"$1/$2\"' -- {workspace_root} $run_id"
```

That is the teaching form of a real, shipped body. The production one — a full
session-workspace provisioner, with its placement pinned by a test that
compiles the actual documents (`the_shipped_provision_commands_pass_the_placement_guard`)
— is in `crates/aion-server/assistant-embed/assistant.awl`. Copy its shape, not this one's
brevity, when you write your own.

**Where the root is.** `<AION_HOME>/clones` — derived once from the aion home,
with no separate configurable value. The home is already the operator's single
answer to "where does this server's state live", and resolving a second answer
here is exactly how two components come to disagree about one path. Durable
history records workspace paths, so they must survive a reboot: never the OS
temp directory.

**When it resolves.** Once, at server-state construction. Every consumer — the
dispatcher and the startup banner — reads that one value rather than
re-deriving it. A resolution failure does **not** stop the server booting; it is
carried, and surfaces as a terminal dispatch refusal at the first
placeholder-bearing body. A body that does not use the placeholder is untouched
by a failed resolution.

**How to see it.** The server's JSON startup banner carries a `workspace_root`
field. A resolved root appears as its path; a failed one appears as
`unresolvable: <reason>` — never a fabricated value. Reading the banner does
not create the directory.

**The directory.** Created at first expansion, idempotently, owner-only
(`0700`), because it holds operator repositories and agent working trees and
nothing else on the host should get a default read into them. A root that
already exists keeps whatever permissions the operator gave it — those are the
operator's own configuration, not the server's to correct.

### The two refusals, and why they exist

Expansion happens on the raw command **string**, before the template layer
parses it into an argv. That is what makes the placeholder useful — and it is
also why two things are checked rather than assumed.

**The placeholder must stand alone as one whole, unquoted argv word.** These are
all refused (`the declared command carries {workspace_root} …`):

```
run "sh -c '{workspace_root}'"     inside single quotes
run "echo \"{workspace_root}\""    inside double quotes
run "echo x{workspace_root}"       glued to adjacent text
run "echo {workspace_root}/sub"    glued to adjacent text
run "echo ${workspace_root}"       the `$` makes it a parameter reference
```

In every one of those the spliced root would become a *fragment* of some larger
word, so what executed would not be the path the server resolved. Refusing by
name beats running somewhere else quietly. To build a path underneath the root,
pass the root as its own word and join inside the program — as the example
above does with `"$1/$2"`.

**The root itself must not change the command's parsed shape.** A root
containing whitespace (splits a word), `$` (opens a parameter reference), a
quote (opens a quoted region), or a NUL (cannot cross `execve`) is refused
rather than spliced. The root is server-controlled, so this refusal is
theoretical — it is checked because "theoretical" is not "impossible".

Every one of these refusals is **terminal**. The root is a property of the
server's configuration and filesystem; retrying the dispatch cannot change it.

### Why braces, and what happens if expansion is ever skipped

`{name}` is literal text to both the AWL checker and the worker SDK's command
template — `$name` and `${name}` are the only parameter forms either recognises.
So a command carrying `{workspace_root}` checks clean at authoring time, and if
it ever reached an executor unexpanded it would fail loudly trying to use a
nonexistent `{workspace_root}` directory, rather than silently running in the
wrong place.

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
| `{workspace_root}` resolution, expansion, and both refusals | `crates/aion-server/src/worker/workspace_root.rs` |
| Authoring-time checks | `crates/aion-awl/src/checker/bodies.rs` |
| Identity sealing | `crates/aion-package/src/contract.rs` |
| Sealing broken, not read | `crates/aion-awl-package/tests/declared_command_sealing.rs` |
| Live proof | `docs/evidence/awl-declared-command-live-run.md` |
