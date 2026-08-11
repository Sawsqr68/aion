# `norn-dev` — one dev brief, delegated and mechanically gated

The single-lane sibling of [`norn-fleet`](../norn-fleet/): one bounded
implementation brief goes to a norn-backed builder over the plain agent seam
(one `String` prompt in, the canonical agent-outcome record out —
`{text, stop_reason}`, the shape `aion worker agent` serves from the
installed binary, no crate, no rebuild), and the result is admitted or
refused by a gate script the **server** runs, not by the builder's own account
of itself.

## The shape

1. **`control`** — the builder is asked a question whose answer the dispatcher
   already knows (quote a literal at a known path). A wrong reply refuses the
   whole run as `unmeasured`: a pipeline that cannot read the tree produces
   confident reports indistinguishable from real ones, and the distinction is
   made here or not at all.
2. **`build`** — the brief goes to the session verbatim; the reply comes back
   verbatim as the build report. Retries absorb provider overload, not build
   failures — a failed build is a reply, not an error.
3. **`gate`** — a `run`-bodied action executes the gate script on the server.
   The real exit code routes the workflow: `0` → `done`, anything else →
   `gates_red` with both streams attached. The builder cannot route this fork.

## Serving the `dev` queue

`norn_dev.awl` declares one agent-served action on queue `dev`, and it does
**not** yet carry a `harness` section — so `aion worker agent` refuses it by
name until one is written. That is not an oversight to work around with flags:
the flags are gone, and every field of the section is operator policy with no
default (how many activities this box serves at once, how long a reconnect
budget is worth, exactly which environment variables the builder agent may
see). Nobody can write those on an operator's behalf, least of all a file
committed to this repository.

Add the section under `worker dev`, above its actions:

```awl
worker dev
  harness
    kind norn
    concurrency <n>
    reconnect_initial_backoff <n>s
    reconnect_max_backoff <n>s
    reconnect_max_attempts <n>
    env_pass "PATH", "HOME"
    args "--account", "<account>"
```

then serve it — the command line now carries only the connection:

```sh
aion worker agent workflows/norn-dev/norn_dev.awl \
    --liminal-address <host:port> \
    --identity norn-dev-builder
```

`aion awl check workflows/norn-dev/norn_dev.awl` names every setting the
section is missing, scoped to the kind it declares, so the document can be
brought up against the checker rather than against this README.

`env_pass` is the one that changes behaviour rather than just moving house: it
is the builder agent's **entire** environment. The document names the
variables, this machine supplies their values, and a variable that is not named
is absent in the agent whatever your shell exported. Nothing is inherited —
which is exactly what stops an agent's own `CLAUDECODE` reaching a nested agent
and killing the run. See [AGENT-WORKERS.md](../../docs/workers/AGENT-WORKERS.md)
for the whole section.

## What the dispatcher owes the run

- **`brief`**: complete and self-contained — the task, the file fence, the
  house standards, and the evidence the builder must produce. The builder sees
  nothing else.
- **`gate_script`**: a path to an executable script that runs every gate the
  lane owes (tests, clippy, fmt, mutations) and exits non-zero on any failure.
  The script is the lane's measurement; writing it honestly is dispatch work.
- **`control_prompt` / `control_expect`**: the known-answer question and its
  exact expected reply.

## What this does not prove

A green gate proves what the script measures, nothing more. Review of the
diff — the judgement half of the delegation loop — happens after this
workflow returns, by a reviewer reading the tree, not the report.
