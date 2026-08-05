# Seat resume — marker AL, 5 August 2026

Supersedes AK. **Latest letter wins.** AK's record stands (it holds #84, the
R2.4 amendment, and the #101 re-measurement); everything below is newer.

`origin/main` = `2afd537db`. **The working tree is DIRTY with unfinished #102
work.** Nothing of #102 is committed.

## 🔴 IN FLIGHT — #102, uncommitted, PARTLY GATED

One file changed: `crates/aion-server/src/api/http/payload.rs`.

### The defect, confirmed at the code

HTTP `POST /workflows/start` and `/signal` take an optional `input`. If it is
envelope-shaped (`content_type` + `bytes` keys) it was decoded as
`ProtoPayload`, whose `content_type` is the **gRPC wire string**
`"application/json"`. But HTTP *output* (history, describe) emits
`aion_core::Payload`, whose serde form is **`"Json"`**. So a payload read off a
run could not be handed back — `content_type_from_wire`
(`crates/aion-proto/src/convert.rs:459`) accepts exactly `"application/json"`
and refuses everything else.

**Scope is narrower than task #102 claims.** gRPC/proto and the Python SDKs are
internally consistent on `"application/json"`; this is an **HTTP-surface-only**
asymmetry. Evidence that `"Json"` is canonical on HTTP: it is the output form
by explicit design (`describe_response_to_ops_console` doc comment — "so the
wire matches the generated TypeScript bindings field-for-field, no
protobuf-derived `{content_type, data}` payload wrappers"); the generated TS
declares `ContentType`; `apps/aion-ops-console/src/features/workflow-detail/
lib/payload.ts:38` checks `'Json'`; `crates/aion-cli/src/output.rs:155` checks
`"Json"`. **Nothing first-party sends an envelope**, so the blast radius of
narrowing it is measured, not assumed.

**Bonus finding:** the old envelope path accepted arbitrary content-type
strings (its test asserted `"application/json; charset=utf-8"` passes through)
— but the next layer refuses anything but exact `"application/json"`. That test
asserted a pass-through that cannot survive: a seam test proving nothing
end-to-end. The fix narrows the accepted set to what the system can actually
process.

### The fix as written

`http_input_payload`'s envelope branch now decodes `aion_core::Payload` and
converts with the public `impl From<aion_core::Payload> for ProtoPayload`
(`convert.rs:211`). Refusal message updated to show the `"Json"` envelope.
Three tests, replacing the old seam test:

- `http_start_input_normalization_accepts_plain_json_and_an_envelope`
- `a_payload_serialised_the_way_this_api_emits_it_is_accepted_as_input` — the
  round-trip. **Its oracle is the emitted form itself**, serialised from a real
  `aion_core::Payload`, not a literal I chose, so it cannot pass by my writing
  the same spelling on both sides. Carries a guard-the-guard assertion: if the
  emitted spelling ever stops being `"Json"`, it fails loudly rather than
  quietly testing something else.
- `the_grpc_wire_spelling_is_refused_as_an_http_envelope`

**Measured so far:** `cargo test -p aion-server --lib api::http::payload` →
**3 passed / 0 failed**. **Mutation-checked**: reverting the branch to the old
`ProtoPayload` decode killed exactly the two new tests, with the failure
message *being* the defect (`left: "Json", right: "application/json"`).
Restored from a file copy and verified by `diff` + grep.

### NEXT STEPS, in order

1. `cargo fmt --all`.
2. `cargo clippy -p aion-server --all-targets -- -D warnings`.
3. `cargo test -p aion-server` — **baseline to beat is 951 passed / 0 failed /
   0 ignored across 46 targets** (at `7c7d2be9`). Expect 951 + 2 = 953 (three
   new tests replace one old one). ⚠️ `cargo test` STOPS at the first failing
   target, so a red run's totals are truncated and NOT comparable.
4. **The API.md correction is UNFINISHED and UNVERIFIED — do not write it from
   this marker.** `docs/operations/API.md` line 55 describes the start `input`
   envelope as a `ProtoPayload` with `content_type` (now wrong — it is the
   `Payload` serde form). Lines 111–131 document `POST /workflows/describe` as
   returning `{"payload": {"content_type": "application/json", "data": {…}}}`
   envelopes with status at `summary.payload.data.status`. That looks wrong:
   `describe_workflow` (`crates/aion-server/src/api/http/workflows.rs:203-212`)
   calls `describe_response_to_ops_console`, which DECODES those envelopes into
   plain `aion_core::WorkflowSummary` / `Event`. **I was interrupted before
   confirming the served shape.** Read the test
   `describe_decodes_json_payloads_for_http`
   (`crates/aion-server/src/api/http/workflows.rs:846`) and confirm against the
   handler before rewriting a single line of that doc — correcting a doc to
   something false is worse than leaving it.
5. Commit + push, update task #102.

**Reversible exit:** drop the change —
`git checkout -- crates/aion-server/src/api/http/payload.rs` (it is TRACKED, so
checkout works here, unlike an untracked file) — and re-open #102 with this
analysis. Nothing depends on it.

## Board

- ✅ #84 (`e314caf1`), #144, #145 closed. ✅ #101 closed at my seat
  (`2afd537db`) — its ergonomics claim was REFUTED; the route-retire gap is
  **Waffles' to take to Tom**, not mine to build.
- ✅ R2.4 discharged by Amendment 1 in `BRIEF-139-LEG-A-REWORK.md`
  (`cfc4ac42`), on Waffles' ruling.
- 🔧 #102 in flight, uncommitted, partly gated (this document).
- ⛔ **#94 examined and deliberately NOT taken.** Its intervals are configured
  values needing discussion, and it must land as one policy with #58 — taking
  half unilaterally is the fight the task warns about. Route it, don't build
  it.
- Held on Tom: the #139 live cut, #142, the 0.11.1 decision. Queued: #141.

## Three method rules earned today — all now memories

1. **A mutation check only measures what the test actually executes.** My own
   marker AJ specified a mutation on a path its named test never reached; it
   would have killed nothing and I'd have recorded a pass. Ruled at Waffles'
   seat.
2. **One truth kept in two places, equal only because a person keeps it equal,
   has drifted or it will.** The R2.4 shape.
3. **Confidence is what stops you measuring, so the confident claim is the one
   that most needs it.** #101's headline claim was false for six weeks and had
   been passed to Waffles as evidence; `git log -S` settles such a claim in two
   commands. Ruled at Waffles' seat with my name on it.

Waffles' framing, worth keeping whole: *carefulness at the moment of writing
does not survive into the moment of execution — only mechanisms do.*

## Unchanged standing constraints

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker, and
restarting an already-built worker is not a rebuild. **Editing aion-server
source is not a worker rebuild, but do not restart Tom's live processes.**
🔴 Tom's `~/.aion/config.toml` `[ops_console] FileSystem` line must stay until he
installs a binary built from `b20d2da1` or later. 🔴 beamr JIT blocker with
Artemis. ⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
Backticked commit messages via `git commit -F -`. Do not edit `CLAUDE.md`.

Live pids, untouched all session: server **45425**, worker **48399**, both from
the repo root. Server log: scratchpad `server.log`.
