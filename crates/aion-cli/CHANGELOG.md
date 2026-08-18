# Changelog

## Unreleased

### Removed

- **The libSQL store backend is retired** (aion#35). `--store-url` is still
  accepted by the parser and is now REFUSED at startup, naming the flag and
  prescribing `backend = "haematite"` with `store.data_dir` and
  `store.node_cache_budget`; the same refusal covers `backend = "libsql"`,
  `store.url`, `AION_STORE_BACKEND=libsql`, and `AION_STORE_URL`. The
  `libsql-backend` and `haematite-backend` cargo features are gone — haematite
  is linked in every build. This build does not read libSQL database files;
  0.17.0 is the last release that can.

### Changed

- **The installed binary is now named `aion`** (was `aion-cli`). The crate
  name stays `aion-cli`; `cargo install aion-cli` installs the `aion`
  executable. All subcommands and global flags are unchanged.
- **New `aion server` subcommand** runs the full Aion workflow server
  in-process — the Temporal-style unified binary. It exposes exactly the
  surface of the former standalone `aion-server` binary (`--config`,
  `--listen-address`, `--scheduler-threads`, `--drain-timeout`, repeatable
  `--workflow-package`) and preserves its
  operational contract: JSON tracing logs, exit code 2 for configuration
  errors, graceful drain on the first termination signal, and exit code 130
  when a second signal forces immediate exit.
- New `auth` feature forwards to `aion-server/auth` for JWT/JWKS
  authentication in the embedded server. Off by default, matching the
  feature set the standalone server binary shipped with.
- The default cancellation reason for `aion cancel` is now
  `cancelled by aion` (was `cancelled by aion-cli`), matching the binary
  name.
