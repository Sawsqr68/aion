# RUNBOOK — running remote-gates on a separate gate host

This is the deployment procedure for putting the gate battery on a machine that
is not the author's. The Aion server stays where it is; only the WORKER moves.
The workflow itself is machine-agnostic — nothing in `remote_gates.awl` or the
worker names a host.

Read [README.md](README.md) first for what the workflow does and what comes back.

## What the gate host must have

The worker shells out to these and nothing else. Confirm each one, on the gate
host, before you deploy:

```sh
git --version
cargo --version
rustc -vV
cargo nextest --version     # the tests leg is nextest; there is no fallback
cargo fmt --version
cargo clippy --version
df -P -k /                  # POSIX df, for the free-space reading in Checkout
```

If `cargo nextest` is missing, install it — the nextest leg refuses with a typed
`missing-tool` finding rather than downgrading to `cargo test`:

```sh
cargo install cargo-nextest --locked
```

If the gate host will notify, it also needs the `meridian` CLI on `PATH` and a
working identity; otherwise leave `--notify-channel` unset and the action skips
honestly.

The gate host's toolchain is the one the findings describe. A different `rustc`
or `clippy` there than on the author's machine is a real difference in the
verdict, not noise — stamp `rustc -vV` and `cargo nextest --version` from the
GATE HOST in whatever record the run produces.

## Build and ship the worker

The worker binary is a normal Rust build. Either build it on the gate host from a
checkout of this repository, or build it locally for the host's target and copy
it over. Building ON the gate host is the simpler path and avoids target-triple
mismatches:

```sh
# on the gate host
git clone <this-repository> ~/aion
cd ~/aion/examples/remote-gates
cargo build --release --manifest-path worker/Cargo.toml
```

To build elsewhere and copy, build for the gate host's triple and `scp` the
binary — it is self-contained apart from the tools listed above:

```sh
# on the build machine
cargo build --release --manifest-path worker/Cargo.toml
scp worker/target/release/remote-gates-worker <user>@<gate-host>:~/bin/
```

The workflow package is deployed to the SERVER, not to the gate host:

```sh
# wherever the server is reachable from
cd examples/remote-gates
gleam build
aion package --build .
aion deploy remote-gates.aion
```

## Prepare the workspace root

Pick a directory the worker owns outright. It will hold checkouts and logs, and
it will grow: a warm `target/` for a large workspace is tens of gigabytes.

```sh
# on the gate host
mkdir -p /srv/remote-gates
df -h /srv/remote-gates      # confirm the headroom before the first run
```

The `Checkout` returned by every run carries `free_disk_bytes` read from that
filesystem, so a gate host that is filling up says so in the run's own data.

## Start the worker

Foreground first, always — one real run under your eye before you detach
anything. This is the exact production argv:

```sh
# on the gate host
remote-gates-worker \
  --address <server-host>:50061 \
  --task-queue remote_gates \
  --workspace-dir /srv/remote-gates \
  --progress-interval 30
```

Add `--notify-channel <channel>` if this host should post verdicts. Repeat
`--address` to give the worker redial candidates across a server cluster; it
cycles them with bounded backoff and re-registers on whichever answers.

Every switch that describes the host is required and has no default. The worker
refuses to start without `--address`, `--task-queue`, `--workspace-dir`, and
`--progress-interval`.

## Keep it up

There is no daemonization in the binary and no launchd unit — deliberately. To
keep the worker alive across your logout, detach it with `nohup` and `disown`,
which survives a session ending:

```sh
# on the gate host
nohup remote-gates-worker \
  --address <server-host>:50061 \
  --task-queue remote_gates \
  --workspace-dir /srv/remote-gates \
  --progress-interval 30 \
  >> /srv/remote-gates/worker.log 2>&1 &
disown
```

Confirm it registered before you walk away:

```sh
tail -n 20 /srv/remote-gates/worker.log     # look for "connection registered"
pgrep -fl remote-gates-worker
```

`--ready-file /srv/remote-gates/.ready` gives a file that appears only after the
first successful registration, if you want something to poll rather than a log
line to read.

To stop it:

```sh
pkill -f remote-gates-worker
```

An in-flight leg is a real child process group. Cancelling the workflow takes
that group down — `SIGTERM`, a bounded grace, then `SIGKILL`, with the group
confirmed empty before the activity reports cancelled — so a cancelled run does
not orphan a Cargo build on the gate host. Killing the WORKER, by contrast,
leaves its children to `kill_on_drop`; check with `pgrep -fl cargo` afterwards.

## Start a run

From wherever the server is reachable:

```sh
aion start remote_gates \
  --input '{"repo":"https://github.com/ablative/aion.git","branch":"main"}'
```

The `repo` must be a remote the gate host can reach with the credentials the gate
host has. A private remote needs the gate host's own SSH key or credential
helper — the worker passes the remote to `git` unchanged and holds no secrets.

## Reading a run

Everything the run learned is in its outcome payload: per-leg exit codes,
structured findings with file/line/column/lint, denominators, and the absolute
path of each leg's log ON THE GATE HOST. To read a full log:

```sh
ssh <user>@<gate-host> cat /srv/remote-gates/logs/<repo-name>/<leg>-<key>.log
```

Each log holds the exact `argv`, the working directory, both output streams, and
the exit code.

## When something is wrong

| symptom | what it means | what to do |
| --- | --- | --- |
| worker exits with `WORKER_CONTRACT_MISMATCH` | the deployed package declares actions this binary does not advertise, or a different queue | rebuild the worker from the same commit as the deployed package; check `--task-queue` matches the `worker` block in the `.awl` |
| leg result with `missing-tool` | `cargo nextest` is absent on the gate host | `cargo install cargo-nextest --locked` |
| leg result with `stream-integrity` | the leg's stream yielded nothing at all — a broken invocation, not a green | read the leg's log at `log_path`; the exit code is in the result |
| leg result with `spawn-failure` | `cargo` could not be started in the checkout | confirm `cargo` is on the worker's `PATH`, not just your login shell's |
| clone step fails with "already holds a checkout of" | the checkout directory belongs to a different remote | remove or rename that directory on the gate host, deliberately |
| runs get slower every time | the checkout's ignored build cache was wiped | confirm nothing else is running `git clean -x` in the workspace root |
| free disk falling in `Checkout` | warm `target/` directories are accumulating | remove whole checkouts under `<workspace-dir>/checkouts/`, not their contents |
