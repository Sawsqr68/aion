# Remote-workers cluster — four parallel lanes (Tom's direct order, 2026-08-18 02:16Z)

**Steward/owner:** Vesper Lynd. **Goal (Tom's words):** run workers on different
devices dialing into the server — "we need to just get these done."
**Acceptance for the CLUSTER (after all four land):** a worker on a second
physical device (Dean's box, over SSH setup), dialing into the estate server
over the network, surviving a full day of real work without babysitting.

All four lanes: off main `1e20aea43`, Opus builders, work parallel / land
serial through my gate, red-first tests, extraction battery per
`ablative/docs/GATE-EXTRACTION-STANDARD.md` on each final tree (NORN_BIN
exported, per-leg stderr + Summary kept, no fmt-check ever — autoformat),
both clippy resolutions, REPORT.md at worktree root, no push/no merge.
House rules per CLAUDE.md are law. Charters below are per-lane scope; the
GitHub issue bodies are the diagnosis of record — verify each mechanism at the
bytes before building on it.

---

## Lane 25 (`aion-wt-25-liveness`, branch `25-liveness`) — aion#25

Liminal liveness probe marks healthy gRPC workers permanently ineligible.
Zone: `crates/aion-server` liveness probe / HeartbeatTracker / worker
selection. Fix: a transport's liveness verdict may alter eligibility ONLY for
workers carried by that transport; scope each verdict to the round's actual
liminal targets; empty liminal rounds clear stale liminal verdicts without
touching gRPC workers. The issue's five acceptance criteria are the gates,
including the mixed-transport regression test (both transports in one server,
gRPC dispatch before AND after a probe round). NOTE: the issue text mentions a
reporter-side "local fix" — that is a description, not landed code; verify
what main actually does and build red-first against it.

## Lane 26 (`aion-wt-26-py-heartbeat`, branch `26-py-heartbeat`) — aion#26

Python gRPC worker sends no heartbeat while idle → deregistered every window.
Zone: `sdks/python/aion-worker` (loop.py, session.py). The Rust runtime
(`crates/aion-worker/src/runtime/loop_.rs:459-476`) is the contract of record:
one empty-ID connection heartbeat at quarter-window cadence even with zero
in-flight, then task heartbeats. Cadence derives from the server's RegisterAck
window — never a hardcoded constant (house law). Server accepts empty-ID beats
already (`worker_grpc.rs:383-390`) — do not touch the server. Tests: the
Python suite must pin "idle worker stays registered ≥3 windows without
reconnect" with a short configured window, and the reconnect-as-keepalive
behavior must be red-provable first. Run the Python SDK's own test suite AND
the Rust battery (unchanged tree = unchanged results, but the battery is the
landing proof).

**EXTENDED 2026-08-18 (aion#50, Tom's ruling 04:19Z — same two files, so it
rides this lane):** the Python session ALSO cannot answer the gRPC liveness
ping #197 introduced. `decode_server_message` (session.py) has no
`liveness_ping` branch, so the oneof falls to
`raise TransportError("unsupported server-to-worker message …")` — raised out
of the `_receive_from_stream` async generator, it TEARS THE SESSION DOWN;
the worker reconnects, is pinged again, tears down again, and its probation
never clears (permanent dispatch exclusion on any server that boots the
liminal listener — not opt-in). Fix in the runtime, mirroring the Rust
contract (`crates/aion-worker/src/runtime/loop_.rs:553-557`): decode
`liveness_ping`, answer `WorkerToServer.liveness_answer{liveness_ping:
<echoed sequence>}` from the runtime, never from action code. Both generated
protos already carry the types — no wire change. Red-first: a test that
delivers a liveness ping to the session must reproduce the teardown before
the fix; green must prove echo-with-same-sequence and session survival.
Close BOTH doors in one lane: the idle-heartbeat gap (#26) and the
ping-answer gap (#50-Python). The #50 row stays open for its TypeScript half.

## Lane 12 (`aion-wt-12-payload-limits`, branch `12-payload-limits`) — aion#12

Oversize activity results misreported as lost worker; describe unreadable
above the same 4MiB limit. Three parts, all in scope (never document a
limitation a fix could close):
(a) the worker gRPC service gets a configurable max message size the way the
deploy service already does (`deploy_grpc.rs` pattern); absent key = defer to
tonic's own default — no invented default value;
(b) an oversize result becomes a TYPED, NON-RETRYABLE error naming the payload
size and the limit — never "worker lost", never spending a retry budget on a
size that cannot succeed; fix the classification at
`worker/dispatch.rs:577`'s seam;
(c) history reads (describe/server side) become paging-capable or
payload-omitting so a completed run's history is always retrievable — pick the
design that fits the existing wire contracts (`aion-proto`), state the choice
and why in REPORT.md; the CLI surfaces it.
Red-first for (b): a >limit result must reproduce the misleading
worker-lost classification before the fix. Measured boundary in the issue
(4.0MB ok / 4.5MB fail) is the fixture guide.

## Lane 15 (`aion-wt-15-mint-credentials`, branch `15-mint-credentials`) — aion#15

Authenticated HTTP starts and worker registrations lose credentials during
namespace-mint forwarding. Zone: `crates/aion-server` namespace mint routing
(route.rs, http/workflows.rs, state.rs, worker_grpc.rs, mint_resolve.rs).
Fix per the issue's acceptance criteria: carry allowlisted HTTP auth headers
into forwarded mints; carry WorkerProtocol metadata into the per-registration
minter; never log bearer values; preserve the working gRPC workflow-start
behavior; the owner's typed auth refusal surfaces as itself, not as
NotOwner/unreachable. Tests: multi-node auth-enabled coverage for both paths
on a non-owner node (the auth feature's existing test harness is the venue;
if a true multi-node auth e2e needs infrastructure the tree lacks, build the
smallest honest harness rather than gating on env vars — and if genuinely
impossible in-tree, STOP and report, do not fake it).

---

## Landing-order note (file overlap)

#12 and #15 both touch `worker_grpc.rs` in different functions; #25 touches
worker selection. Landings are serial through my gate with a rebase +
battery-on-true-final-tree for whichever lands second/third. #26 is
Python-only and lands independently. Each landing cleans its target dir in the
same breath (ceremony line, 2026-08-18).
