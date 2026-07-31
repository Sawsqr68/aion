# `aion server --endpoint` was accepted and never read (#114)

Found 2026-07-31 while standing up an isolated server for the #97 live proof —
the same launch that surfaced [#113](AION-HOME-ISOLATION-INVERSION.md). Fixed
and live-proven the same day.

## What happened

`--endpoint` is a **global** CLI flag, so it is advertised in `aion server
--help` and accepted by `aion server`. It is the address a *client dials*. The
server's bind comes from `[server].grpc_address`, and nothing in
`crates/aion-cli/src/server.rs` ever read `--endpoint`.

So an operator who ran

```
aion --endpoint 127.0.0.1:51099 server
```

to move the bind off a busy port got a server that bound **50051** anyway. On
this box, 50051 is Tom's live server. The failure mode is not a confusing
message — it is a second process starting on a port that already has an owner,
or, worse, starting successfully and reaching for state the operator believed
they had moved away from.

I hit this personally: my proof server died with `grpc transport failed at
127.0.0.1:50051` while I was passing `--endpoint 127.0.0.1:51099`.

## Why refusing beats honouring

`--endpoint` could have been wired to the bind. That would be a category error:
a dial target and a bind address are different things (a dial target carries a
scheme and may name a remote host; a bind address must be local). Conflating
them would produce a flag that works until the day someone dials a hostname.

There was also nothing to point an operator at — `aion server` had
`--listen-address` for HTTP and **no gRPC equivalent at all**. The refusal
would have had to say "use the config file". So the fix is two things, and
neither is optional:

1. **`aion server --grpc-address <ADDR>`** now exists, symmetric with
   `--listen-address`, threaded through `CliOverrides.grpc_address` into
   `[server].grpc_address`.
2. **`aion server` refuses** when `--endpoint` is passed, naming
   `--grpc-address`, `AION_SERVER_GRPC_ADDRESS`, and `[server].grpc_address`.
   Exit code 2, the established config-failure code.

A refusal that names the wrong replacement is barely better than the silence it
replaces, so the message is a tested value, not a string literal buried in a
dispatch arm.

## The default had to move

`--endpoint` carried a clap `default_value`, which meant the parsed value could
not distinguish "the operator named an endpoint" from "nobody said". The server
needs exactly that distinction to refuse. The field is now `Option<String>` with
the default applied at the point of use (`Cli::endpoint()`), so parsing no longer
destroys the fact the refusal depends on.

This is the same shape as #113 and #83: **the information existed and was
discarded one layer above the decision that needed it.** Three findings in two
days with one root pattern is worth naming.

## Live proof — both findings, one launch

Run from the repository root, which holds the 6.4G legacy `aion-data`, so #113
and #114 are exercised together on the production path.

**The refusal, on the real binary, at the exact argv that burned me:**

```
$ AION_HOME=… ./target/debug/aion --endpoint 127.0.0.1:51099 server
refusing to start: `--endpoint 127.0.0.1:51099` names an address a CLIENT dials,
and `aion server` cannot bind it. Use `--grpc-address <ADDR>` (or
AION_SERVER_GRPC_ADDRESS, or [server].grpc_address) to choose the gRPC listener,
and `--listen-address <ADDR>` for HTTP.
EXIT=2
```

**The positive control** — same directory, correct flags:

```
WARN  Aion home legacy directory ignored because AION_HOME is set
      notice=… legacy store data directory `/Users/tom/Developer/ablative/stack/aion/aion-data`
      exists but this server uses `/Users/tom/.aion-proof114-vesper/data`; the legacy
      directory is untouched …
INFO  aion-server data root resolved   data_root=/Users/tom/.aion-proof114-vesper/data
INFO  aion-server startup banner       grpc_address=127.0.0.1:51099 http_address=127.0.0.1:8099
```

**Bound sockets, not just a banner** — a startup banner reports configuration,
which is not the same claim as a listening socket:

```
aion  13637  tom  37u  TCP 127.0.0.1:8099  (LISTEN)
aion  13637  tom  80u  TCP 127.0.0.1:51099 (LISTEN)
aion  99844  tom  86u  TCP 127.0.0.1:50051 (LISTEN)   ← Tom's server, untouched
```

`nc` succeeded on both; `GET /health` returned **200**.

**Isolation confirmed after teardown, from the filesystem rather than the log:**
`aion-data` mtime remained `2026-07-30T12:54:59+1000` — the day before. The
proof server never wrote there. Proof home removed, both proof pids gone
(SIGTERM, exit 144), ports released, Tom's pid 99844 still serving.

## Gates

| Gate | Result |
|---|---|
| `cargo test -p aion-cli --bin aion` | **207 passed / 0 failed** |
| `cargo test -p aion-server --lib config::load::home_tests` | **11 passed / 0 failed** |
| Live binary refusal + positive control | above |

New pins:

- `the_server_refuses_a_client_endpoint_instead_of_binding_the_default` — the
  refusal exists and names the working flags.
- `a_server_run_without_endpoint_is_not_refused_and_clients_keep_the_default` —
  the control. An unconditional refusal would satisfy the first test and break
  every other verb; this catches it, and also holds the client default.
- `the_grpc_bind_override_reaches_the_server_config` — compares against a
  measured baseline rather than a hardcoded default port, and asserts the two
  differ, so it cannot pass vacuously if the override becomes a no-op.
- `server_subcommand_converts_all_overrides` extended with `--grpc-address`.
