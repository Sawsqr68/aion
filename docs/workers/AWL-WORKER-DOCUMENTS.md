# Worker documents (`worker <queue>.awl`)

A `.awl` file whose opening declaration is `worker` rather than `workflow` is a
**worker document**: the implementation of one task queue. It declares the
actions that queue serves, each with the command that runs it, and it declares
no flow at all.

```awl
//! Serve the `greeter` queue.
worker greeter
  action greet(name: String) -> Greeting
    run json "greeter-tool --json --name $name"
  action shout(text: String) -> String
    run text "tr a-z A-Z"

type Greeting { message: String }
```

Serve it against a running server:

```sh
aion worker awl greeter.awl --concurrency 4 \
    --reconnect-initial-backoff 0.1 --reconnect-max-backoff 5 \
    --reconnect-max-attempts 1000
```

This is the fourth way an action can be served, alongside a declared `run`
body inside a workflow document (the server runs it), `aion worker shell` (a
TOML manifest wires host commands), and a worker SDK build. Compared with the
manifest it is **typed by the language** rather than by a schema-less TOML
file, and compared with a declared body it runs **on the worker's host**, not
on the server's.

Ground truth for everything below: `crates/aion-awl/src/checker/family.rs`,
`crates/aion-awl/src/worker_document.rs`, `crates/aion-cli/src/worker_awl/`,
and the corpus under
`crates/aion-awl/tests/fixtures/rev2/worker-documents/`.

## The two families

One `.awl` file is one family, and the opening declaration is what says which:

| Opening declaration | Family | What it is |
|---|---|---|
| `workflow <name>` | workflow | A flow: typed start contract, outcomes, steps, and the `worker` blocks it REQUIRES. |
| `worker <queue>` | worker | An implementation: the actions one task queue serves, each carrying the body that runs it. |

Both parse through one grammar, so a declaration belonging to the other family
is a well-formed parse that `aion awl check` refuses **by name, with the
remedy**. In a worker document that means:

- a `step`, a `child`, or a `subflow` — a worker document runs no flow;
- a `const` — a worker document has no expressions for one to feed;
- a second `worker` block — one document serves one queue, the one its opening
  declaration names;
- an action with **no body** — a bodyless action is a requirement on an
  out-of-band worker, which is a thing a workflow document declares;
- a delivery key on an action (`timeout`, `retry`, `node`, `advisory`,
  `agent`) — every one of those configures how the CALLER dispatches the
  action, and the caller is the workflow document. Stating it here as well
  would put one fact in two files with nothing holding them equal.

And in the other direction, a workflow document may not declare a capture
selector (`run text`, `run json`): its declared bodies are executed by the
server, whose dispatcher produces the command's outcome record and nothing
else.

`aion awl check` reports a clean worker document as
`ok: <file> (worker `<queue>`, N actions)` — it has no steps to count.

## The three body forms

A body's **capture** decides what a successful command's result is. It decides
nothing else: failure classification is the executor's and is identical across
all three, so there are not three failure vocabularies to learn.

| Form | Result | Declared return type |
|---|---|---|
| `run "…"` | The command's outcome record: `{ exit_code: Int, stdout: String, stderr: String }` | A record naming any subset of those three members, with those types |
| `run text "…"` | Trimmed stdout, as one JSON string | `String` |
| `run json "…"` | Trimmed stdout, parsed as JSON | Any declared type except `Nil` — there would be nothing for the decoded output to become |

The checker enforces each rule at the declaration. What it cannot check is
whether the command's actual output matches the declared type under `run
json`: that is enforced at runtime by the calling workflow's decoder, exactly
as it is for any worker's result.

Failures, for every form:

- a **non-zero exit** is a **retryable** failure carrying the command's own
  stderr; the calling document's declared `retry` policy decides whether
  another attempt is scheduled;
- a program that could not be spawned, and a **cancellation**, are
  **terminal**;
- stdout that is not valid JSON under `run json` is **terminal** — re-running
  the same command cannot make the same output parse.

Parameter interpolation (`$name`, `${name}`, `$$` for a literal dollar) is the
same in every form, and so are its rules: the program a command executes must
be written out rather than named by a parameter, and a command may reference
only the action's own declared parameters. See
[`DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md) for the full interpolation
table, including the single-quote trap.

## How a worker document and its caller stay honest

The workflow document declares what it needs:

```awl
worker greeter
  action greet(name: String) -> Greeting
    timeout 30s, retry 3 every 10s
```

The worker document declares what it serves — the same name, the same
parameters, the same result type, plus the body. Two files now state one
interface, and what holds them equal is **admission**, not diligence.

The advertised surface is derived through the same contract derivation the
deploy path uses, so the worker's advertisement and the server's stored
contract for the queue are two independent derivations of two documents. On
registration the server compares them field by field
(`crates/aion-server/src/worker/contracts.rs`) and refuses a worker whose
surface has drifted, naming the package version, the action, the field, what
was expected and what was advertised. A worker document that has fallen behind
the workflow that calls it is refused at the door, in the same way and with
the same message as a stale SDK worker build.

Two consequences worth stating plainly:

- **A clean `aion awl check` of a worker document proves it is internally
  consistent, not that it matches any deployed contract.** Run
  `aion worker awl <file> --check` to see the exact surface it would
  advertise before dialling, and read
  `POST /awl/workers/availability {"namespace":"…","task_queue":"…"}` —
  specifically `connected_workers` — to confirm it arrived.
- **Serving is all-or-nothing by construction.** The document IS the served
  set, so a partially wired queue — the failure the manifest worker has to
  check for — cannot be expressed here.

## The verb

```
aion --endpoint http://127.0.0.1:50051 worker awl <FILE.awl> [flags]
```

| Flag | Variable | Meaning |
|---|---|---|
| `--identity` | `AION_WORKER_IDENTITY` | The identity the worker registers under. **Derived** as `<queue>-awl-worker` when absent — a reading of the document, not an invented default. |
| `--concurrency` | `AION_WORKER_CONCURRENCY` | Maximum activities served at once. Required. |
| `--reconnect-initial-backoff` | `AION_RECONNECT_INITIAL_BACKOFF_SECONDS` | Seconds before the first reconnect attempt. Required. |
| `--reconnect-max-backoff` | `AION_RECONNECT_MAX_BACKOFF_SECONDS` | Reconnect backoff cap, in seconds. Required. |
| `--reconnect-max-attempts` | `AION_RECONNECT_MAX_ATTEMPTS` | Cumulative session-drop budget. Required. |
| `--check` | — | Print the advertised descriptors (canonical JSON — diff them against the deployed contract) and the command each action runs, then exit without connecting. |

Nothing is guessed: the worker SDK's builder refuses to build without those
four settings because they are operator policy, so this verb demands them too.
A flag wins over its variable, and every missing setting is named at once
rather than one per correction.

The endpoint is the global gRPC `--endpoint`, and a scheme-less one is
normalized to `http://` — unlike `aion worker shell`, which passes it verbatim
and silently never connects.

## THERE IS NO SANDBOX

A worker document's commands are **host commands** on the machine running
`aion worker awl`. Read the security section of
[`DECLARED-COMMANDS.md`](DECLARED-COMMANDS.md) in full: there is no allowlist,
no denylist, no filesystem confinement, no network restriction, no privilege
drop and no resource limit. A declared `timeout` travels to the engine and
stops nothing; cancellation is the one thing that ends a running command from
outside it, and it terminates the whole process group.

What a worker document does give you, and the manifest does not:

1. **The environment is CLEARED, not inherited.** A command runs with only
   `PATH` from the host — the same rule the server applies to a workflow
   document's declared bodies, because the host process legitimately holds
   credentials and a document must not be able to read them by running `env`.
   The manifest shell worker inherits the worker's whole environment.
2. **A parameter value can never become code.** The command is parsed once
   into an argv whose elements are literal text or named parameter holes, and
   a value is substituted as one WHOLE argv element that never re-enters a
   parser. There is no shell between the declaration and `execve`.
3. **The program is always written out.** Unlike a manifest, which may take
   its program element from the input, a body that names a parameter in
   program position is a check error.

The command runs in whatever working directory the worker was started in, and
the document is a local file: whoever can edit it chooses what runs on the
next restart. Protect it like the scripts it is, and if you need containment,
run the worker as a constrained user, in a container, or on a host that has
nothing else on it.
