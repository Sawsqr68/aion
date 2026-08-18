# Runtime-gated tests — the inventory a battery Summary must be read against

A green battery line (`N tests run: N passed`) counts runtime-gated tests as
PASSED. Their skip announcement goes to stderr, and nextest captures stderr on a
passing test — so a gated test is **indistinguishable from a real pass in the
battery record**. This file is the reconciliation: anything listed here proved
nothing in a run where its switch was unset.

House rule this serves: a test that needs an unavailable capability is gated at
runtime with the full assertion body kept armed — never `#[ignore]`, never
deleted, never left permanently red (a known-red on main destroys the estate's
"empty extraction = green" reading of every future battery).

Proving the channel (the announcement is only visible uncaptured):

```
cargo nextest run -p <crate> --test <suite> <test-name-substring> --no-capture
```

| Test | Switch (set to run) | What it proves when it runs |
| --- | --- | --- |
| `aion-server::run_server_outbox_e2e::run_server_outbox_live_reconciliation_rearms_stranded_claims_once` | `AION_E2E_CONCURRENT_STORE_SEAM` | Live reconciliation re-arms a stale outbox claim exactly once on a RUNNING server. Blocked by store capability, not by the test: haematite grants one writer handle per data directory, and `ReadOnlyDatabase` routes reads by key so a second handle cannot read a workflow-co-located outbox row at all. Needs a haematite concurrent seam with ROUTED reads, or a server-side claim-aging seam — the haematite observer-capability item on Tom's docket (via aion#35). Channel proven 2026-08-18 at the steward's gate. |
| `aion-cli::issue36_pause_restart_resume_e2e` (both faces) | `NORN_BIN` (path to a real agent executable) | Issue #36's pause/restart/resume repro against a real agent harness. |

When a switch's capability ships, run the test, delete its row here, and say so
in the landing that closes it.
