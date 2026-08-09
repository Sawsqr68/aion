# awl-hello — the AWL end-to-end runway

The smallest complete AWL-authored workflow. `awl_hello.awl` (rev-2 surface,
verbatim the spec's worked example 1) is the source of truth and the only file
you edit; a tiny Rust activity worker (`worker/`) serves the two activities.

`aion deploy` and `aion run` compile the `.awl` document directly — there is no
Gleam build, no `aion package`, and no `.aion` archive in the authoring loop.
The committed `src/awl_hello.gleam` and `workflow.toml` exist only for the
legacy Gleam-target route described at the bottom of this file; they are
generated output and are never edited by hand.

## The workflow

```awl
//! Greet a name, then shout it — the first workflow written in AWL and run for real.
workflow awl_hello
  input name: String
  outcome shouted: type Shouted, route success

type Greeting { greeting: String }
type Shouted  { text: String }

worker awl_hello
  action greet(name: String) -> Greeting
  action shout(text: String) -> Shouted

step greet_and_shout
  name |> greet |> .greeting |> shout |> route shouted
```

One step, one pipe chain: the input threads through `greet`, the `.greeting`
field access, and `shout`, and the piped value becomes the `shouted`
outcome's payload. The run ends as
`{"outcome": "shouted", "payload": {"text": "HELLO, <NAME>!!"}}`
(see `schemas/output.json`).

## Activity contract

Both activities are served by `worker/` on ONE liminal connection:

| Activity | Task queue  | Input                | Output                                      |
|----------|-------------|----------------------|---------------------------------------------|
| `greet`  | `awl_hello` | `{"name": String}`   | `{"greeting": String}` — `Hello, <name>!`   |
| `shout`  | `awl_hello` | `{"text": String}`   | `{"text": String}` — uppercased + `!`       |

The server routes a pushed activity by (namespace, task_queue, node) only —
never by activity type. In rev-2 the `worker awl_hello` block name IS the
task queue, so the generated module pins `task_queue("awl_hello")` on every
call. The actions declare no `node` config, so dispatches are node-unpinned
and match any worker on the queue — including this worker, which registers
node `hello` (an action would add `node hello` on its config line to
require it). The authoritative queue string lives twice and MUST agree:
`TASK_QUEUE` in `worker/src/main.rs`, `worker awl_hello` in `awl_hello.awl`.

## Running it

1. **Check the document.** After every edit to `awl_hello.awl`:

   ```sh
   aion awl check awl_hello.awl
   ```

   A clean check prints `ok: awl_hello.awl (1 step)`. `aion awl fmt
   awl_hello.awl` canonicalises it in place.

2. **Start a server** with its `[deploy]` surface enabled (see
   [`docs/operations/operations.md`](../../docs/operations/operations.md)).

3. **Start the worker**, which serves `greet` and `shout` on the `awl_hello`
   queue:

   ```sh
   cargo run --manifest-path worker/Cargo.toml -- --address <liminal addr>
   ```

   `--ready-file <path>` writes a readiness marker on first connect.

4. **Compile, deploy, start, and await in one motion:**

   ```sh
   aion --endpoint <host:port> run awl_hello.awl --input '{"name":"Ada"}'
   ```

   `run` validates the input against the derived start contract before it
   opens the deploy connection, then waits for the terminal result. Add
   `--timeout 2m` to bound how long the *command* waits; the run itself keeps
   going on the server either way.

   To deploy without starting a run, use `aion deploy awl_hello.awl` and start
   it later with `aion start awl_hello --input '{"name":"Ada"}'`.

## The legacy Gleam-target route

`aion awl emit` lowers the document to Gleam source — the stopgap execution
target, and a toolchain view rather than authored source. It is not part of the
authoring loop above and is only needed when you specifically want a packaged
`.aion` archive:

```sh
aion awl emit awl_hello.awl --output src/awl_hello.gleam
gleam build
aion package .            # → awl-hello.aion, per workflow.toml
```

Never edit the emitted module, and never treat it as the source of truth. For
direct BEAM bytecode instead, `aion awl emit awl_hello.awl --target beam
--output <path>.beam` uses the same compiler seam the deploy path does.
