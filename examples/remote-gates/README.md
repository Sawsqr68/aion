# remote-gates

`remote-gates` is an AWL-authored workflow that takes a repository and a branch,
clones them onto a machine that is not yours, and runs the whole Rust gate
battery over that checkout — `cargo check`, Clippy, `cargo nextest`, and the
non-mutating formatter check — returning each leg's real exit code together with
the diagnostics that produced it.

It reports `clean` only when all four legs pass. Both outcomes carry every leg
result, because the VM cannot build a failed-only sublist from named-fork
bindings.

For deployment onto a separate gate host, see [RUNBOOK-REMOTE.md](RUNBOOK-REMOTE.md).

## What comes back

Each leg returns a `LegResult`:

| field | meaning |
| --- | --- |
| `leg` | `check`, `clippy`, `nextest`, or `fmt` |
| `exit_code` | the real process exit code; `-1` when no process could be observed |
| `passed` | true only when `exit_code` is 0 **and** the stream was intact |
| `findings` | every diagnostic the leg's own stream yielded, never truncated |
| `denominator` | how many processed-unit records that same stream yielded |
| `log_path` | absolute path of the leg's log on the gate host |
| `output_tail` | last 40 lines of that log |

A `Finding` carries `leg`, `level`, `file`, `line`, `column`, `code`, and
`message`. `line` and `column` are absent — not null, not zero — when the tool
reported no span; `file` is then `unknown` and `code` is `compile-error`.

### The denominator rule

A leg that reports no findings has only proved something if its stream also
proves it looked at anything. So every leg counts the processed-unit records on
its own stream:

- `check` and `clippy` count `compiler-artifact` records.
- `nextest` counts test events that reached a verdict (`ok` plus `failed`).
- `fmt` counts the targets `cargo fmt --verbose` handed to rustfmt.

**A leg whose stream yielded neither a finding nor a processed unit fails.** It
gets a `stream-integrity` finding saying so, and `passed` is false even when the
exit code was 0. A silent stream is a broken stream, never a green gate.

### Findings are extracted natively

There is no `jq` on the gate host. The worker decodes each stream with
`serde_json`:

- `cargo check` / `cargo clippy` run with `--message-format json`. A
  `compiler-message` at `error` or `warning` level becomes a finding; the
  primary span supplies `file`/`line`/`column`, falling back to the first span
  and then to `unknown`; `code.code` falls back to `compile-error`.
- `cargo nextest` runs with `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1` and
  `--message-format libtest-json-plus`. A `failed` test event becomes a finding
  whose `code` is the test's own name and whose `message` is its captured
  output. **The tests leg is nextest.** A gate host without it gets a typed
  `missing-tool` failure naming `cargo install cargo-nextest --locked` — the leg
  never quietly falls back to `cargo test`.
- `cargo fmt --all --verbose --check` prints `Diff in <file>:<line>:` before each
  hunk; each becomes a `rustfmt` finding carrying the hunk. (`cargo fmt
  --message-format json` is a nightly-only surface, so the verbose check stream
  is the machine-readable one available on the stable toolchain this repository
  pins.)

## Checkouts and caches

`--workspace-dir` is the root the worker owns on the gate host. It builds:

```
<workspace-dir>/checkouts/<repo-name>/   one working tree per repository
<workspace-dir>/logs/<repo-name>/        one log file per command per attempt
```

If `checkouts/<repo-name>` already exists **and its `origin` matches the
requested remote**, the worker fetches, `checkout -B`s the branch onto the remote
tip, `reset --hard`s to it, and runs `git clean -fd`. Note the absent `-x`:
`.gitignore`d build caches (`target/`) survive, so the gates stay incremental and
a second run is minutes rather than tens of minutes.

If the path exists with a **different** remote, the activity refuses and names
both remotes. It never re-clones over somebody else's checkout.

Every command's log holds its exact `argv`, its working directory, both output
streams, and its exit code. Nothing is discarded.

## Build

From this directory:

```sh
cargo build --release --manifest-path worker/Cargo.toml
```

`src/remote_gates.gleam` is generated and committed, never edited by hand;
regenerate it after any change to the `.awl`:

```sh
aion awl check remote_gates.awl
aion awl fmt remote_gates.awl
aion awl emit remote_gates.awl --output src/remote_gates.gleam
```

The worker is a separate binary. Both the deployed workflow and a running worker
must be present for the workflow to make progress.

## Deploy and run

Deploy the `.awl` document DIRECTLY — `aion deploy` compiles and deploys it with
its queue contract intact. Do **not** deploy via `gleam build` +
`aion package --build`: the zip-package path drops the `.awl`'s queue
declaration, and every activity dispatch is then refused at start with
`NO_QUEUE_DECLARATION` (proven live 2026-07-29). Then start the worker, then
start a run:

```sh
aion deploy remote_gates.awl

remote-gates-worker \
  --address 127.0.0.1:50061 \
  --task-queue remote_gates \
  --workspace-dir /srv/remote-gates \
  --progress-interval 30

aion start remote_gates --input '{"repo":"https://github.com/ablative/aion.git","branch":"main"}'
```

Every switch that describes the gate host is required and has no default:

| switch | required | meaning |
| --- | --- | --- |
| `--address` | yes | liminal worker endpoint; repeat it to give redial candidates |
| `--task-queue` | yes | queue to serve; the deployed workflow declares `remote_gates` |
| `--workspace-dir` | yes | root the worker owns for checkouts and logs |
| `--progress-interval` | yes | seconds between progress heartbeats during a leg |
| `--notify-channel` | no | side channel for the verdict; unset means notify skips |
| `--identity` | no | worker identity reported at registration |
| `--ready-file` | no | file written once the first registration succeeds |

`--progress-interval` is what lets the server tell a grinding 45-minute build
from a dead worker: while a leg's child process is alive, the activity attaches
its elapsed time to its liveness record on that cadence.

## The advisory notify

`notify_result` is declared `advisory` in the `.awl`, so the engine routes its
faults around the step's fault arm: an unreachable notifier warns, and the gate
run still reaches its verdict. There are exactly two honest shapes:

- **No `--notify-channel`**: a deliberate operator choice. The action returns
  `delivered: false` with `skipped: no channel configured` — an honest skip, not
  silence.
- **A channel configured**: the post either succeeds, or the action FAILS. A
  `delivered: false` "success" would hide a real delivery failure behind a
  boolean, which is exactly what the advisory class exists to absorb.

## Timeouts

Per-action timeouts are authored in `remote_gates.awl`, on each action's config
line — never in Rust:

```
action run_nextest(path: String) -> LegResult
  node shell, timeout 45m
```

Clone 10m, check 15m, clippy 15m, nextest 45m, fmt 5m, notify 2m. An operator who
needs different bounds edits the `.awl`, re-emits, and re-deploys the document.

## Scope

The legs are Rust. Multi-language leg detection is deliberately out of scope for
this version; the leg set is data in the `.awl`, which is where that seam opens.
