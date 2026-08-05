# The fresh-machine test of `scripts/setup.sh` — 5 August 2026

Tom asked whether the assistant would actually work when someone else ran it
on their own machine, and whether we had a clear command for it. He then
approved running the setup script the way a stranger would.

**The answer was no.** The script could not finish on any machine running the
published CLI it tells you to install — including Tom's own box. This record
is how that was established and what changed.

## Why the test had to be isolated, and how

The whole reason this test exists is a measurement law we have already paid
for once: *an environment that carries the thing under test cannot measure
whether it is required.* `setup.sh` was live-proven for weeks while being
unable to work on a fresh machine, because every proof ran on a home that
already had what it needed.

So the harness (`harness.sh`, kept beside the run) **subtracts** rather than
adds, and then asserts the subtraction landed, refusing to run otherwise:

| assertion | why it exists |
|---|---|
| `aion` resolves nowhere on the test PATH | if it resolves, `setup.sh` skips the install step and the run measures nothing |
| `git`, `cargo`, `norn` do resolve | otherwise a failure would be my isolation's fault, not a finding |
| no `config.toml` in the Aion home | the fresh-config path is one of the things under test |
| every ancestor of the store is owner-only | see "the first attempt" below |
| ports differ from the live 8080 / 50051 / 50061 and are free | Tom's server must not be touched |

PATH was `<install root>:<shim>:/usr/bin:/bin:/usr/sbin:/sbin`. The shim
carries only the Rust toolchain and `norn` — **homebrew is deliberately
absent**, so any homebrew-only build dependency would surface as the finding it
would be for a stranger. None did: `aws-lc-sys`, `zstd-sys` and `blake3` all
built from the stripped PATH.

Tom's live server (pid 85467, ports 8080/50051/50061) was never touched.
`CARGO_INSTALL_ROOT` was redirected so his `~/.cargo/bin/aion` was never
overwritten.

## The first attempt, and what it taught

The isolated store was placed under the session scratchpad in `/private/tmp`.
The server refused to start:

> unsafe store.data_dir …: ancestor `/private/tmp` is not owner-controlled:
> mode 1777 grants group/world write access

That is the server being **right**, and it is not a `setup.sh` defect — it is
my harness putting a store somewhere no store may live. The whole isolated
world moved to an owner-only path, and the harness gained the ancestor
assertion above, watched refusing against `/private/tmp` before it was trusted
to pass.

### But it exposed a real defect on the way

`setup.sh` waited the **full 120 seconds** and would then have said only
*"server did not become ready — see the log"*. The server had already exited,
one second in, with a precise reason.

And the same file already knew better: the **worker** startup loop checks
whether its process is still alive and says so. The server loop did not. One
script, two loops, one of them carrying a rule the other lacked — the same
shape as a rule known in two places with nothing forcing agreement.

**Fixed**: the server's pid is captured, the wait loop exits the moment the
process is gone, and the failure quotes the server's own last error.

- Before: 120s, no cause.
- After: **2s, with the cause and the remedy.** Measured both ways.

A defect in my own fix was caught the same way: the first version used a greedy
`.*` to pull the error out of the JSON log line and dragged the following
fields into the message. Watching what it actually printed, rather than that it
printed, is what caught it.

## The finding

With the store on a legal path, the server came up on the isolated port and
`setup.sh` stopped hard:

```
setup: startup banner carries no workspace_root
```

Verified, three ways:

1. The published 0.11.0 server's startup banner has no `workspace_root` field
   (read from the isolated server's own log).
2. `crates/aion-server/src/run.rs:190` emits it — **on main**. The commit is
   `fe702945`, #139 leg A, which landed *after* `v0.11.0` was published.
3. **Tom's live server's banner does not carry it either.**

`setup.sh` on main requires that field. `setup.sh` installs the CLI with
`cargo install aion-cli`, which gets 0.11.0. So the script demanded of the
server a field that the binary it had just installed could not produce.

**Anyone following the documented path got a working server and then a hard
stop.** Tom's own machine included.

### One trap avoided

I first tested whether the clone contained `fe702945` with
`git merge-base --is-ancestor`, which answered "no". That answer was worthless:
the clone is `--depth 1`, so the commit does not exist in it as an object and
the question cannot be answered. The reliable check was the **content** — the
clone's own `run.rs` emits `workspace_root`, and is byte-identical to main's.
Identity is content, not label; a shallow clone will lie to a graph query and
tell the truth to a diff.

## The fix

The cause is not the missing field. The cause is that **the script and the
server were two copies of one contract with nothing forcing them to agree** —
the script ships in the repository, the binary came from crates.io. Bumping a
version fixes this instance and leaves the mechanism in place.

So the fix is derivation, not reconciliation: **`setup.sh` now installs the CLI
from the checkout it already has** (`cargo install --path
$REPO_ROOT/crates/aion-cli`). The script and the server it drives now come from
one tree by construction, and cannot skew again.

For the case the install branch never reaches — an older `aion` already on
PATH — the banner failure now names the mismatch, the binary in use, its
version, and the exact command to end it, instead of reporting an absent field.

## A second thing the fix bought, unplanned

Installing from the checkout restores the server's ability to say what code it
is. Compare the two servers running side by side on this box:

```
published 0.11.0:        "commit":"unknown",  "dirty":"unknown"
built from the checkout: "commit":"369b2dab…", "dirty":"false"
```

A crates.io install has no repository to read, so its `/build` endpoint reports
`unknown` — the server cannot identify its own source. That is the capability
#123 exists to provide, silently absent on every published install. Building
from the checkout gives it back, with a clean-tree flag alongside it.

Worth stating plainly: this is a consequence of the change, not a reason it was
made. The reason was the skew.

## Status

- Server-startup failure reporting: **fixed and measured** (120s→2s, cause
  named, control run both ways).
- Version skew: **fixed and verified**. The install-from-checkout run went the
  whole way.
- **The full path passes.** `==> Done`; console on 127.0.0.1:18080; assistant
  deployed (`content_hash ae8b0ea1…`, `route_changed: true`); worker pid 14366
  running with `--address 127.0.0.1:18061`; one registration recorded
  server-side.
- Tom's server (pid 85467) answering on 8080 throughout, his worker (pid 66189)
  still on 50061. Neither touched.
