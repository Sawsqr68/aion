# `norn-dev` — one dev brief, delegated and mechanically gated

The single-lane sibling of [`norn-fleet`](../norn-fleet/): one bounded
implementation brief goes to a norn-backed builder over the plain agent seam
(one `String` in, one `String` out — the shape `aion worker agent` serves from
the installed binary, no crate, no rebuild), and the result is admitted or
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
