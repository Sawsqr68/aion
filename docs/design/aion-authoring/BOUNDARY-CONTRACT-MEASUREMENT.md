# The declared contract at the boundary: what is enforced today

Measured 2026-07-31 at main `46bbdced`, against
`BRIEF-DECLARED-CONTRACT-AT-THE-BOUNDARY.md`, which requires the start side to
be established before anything is built: "#95 is stated as 'may not be', not
'is not'. Establish which it is before building; if it is already enforced, say
so and close #95 rather than adding a second check."

Established. They are not the same state.

## #95 — START input: ALREADY ENFORCED. Close it, do not build it.

`crates/aion/src/lifecycle/start_admission.rs::admit_declared_input` measures
the caller's input against `contract.entry_input_schema(workflow_type)` — the
schema of the exact package identity the start resolved to — and returns
`EngineError::StartInputRefused` naming the mismatch.

It runs where the brief asked for it. `lifecycle/start.rs:154`:

```rust
if options.input_admission == InputAdmission::Declared {
    admission::admit_declared_input(loaded, &input)?;
}
let (workflow_id, initial_head) = admission::workflow_identity(…).await?;
…
let mut recorder = Recorder::resume_at(…);
```

Before the id is minted, before the recorder is constructed, before any
process is spawned. A refused start records nothing.

The gate is not dark. `InputAdmission::Declared` is set at
`crates/aion/src/engine/api.rs:293` — the single external start path, which
every transport (HTTP, gRPC, WebSocket, CLI, in-process client) reaches. The
only `Trusted` uses are `lifecycle/continue_as_new.rs:140` and
`lifecycle/completion.rs:250`, both engine-generated re-entries whose input was
already admitted at the original boundary. That is correct, not a hole:
re-admitting an engine-authored value would refuse runs for a package whose
schema changed under a live workflow.

Checked specifically because "the gate exists and has never run" is a defect
this codebase has produced twice (#71, #100). It runs.

**One real gap remains on this side, and it is task #106, not #95**: a declared
schema that cannot COMPILE takes the `AdmissionError::UnusableSchema` branch,
which logs `tracing::warn!` and admits the value
(`start_admission.rs:100-108`). The reasoning given there is sound as far as it
goes — refusing would blame the caller for a package defect and make every
start of that package impossible — but the effect is that validation is absent
for exactly the packages whose declarations are broken, and a warn line is not
a refusal. #106 owns that; it is not this brief.

## #97-A — SIGNAL payload: read on first pass as NOT enforced

> **WRONG, and corrected in the VERIFIED section below.** This section is what
> the HANDLER path shows, and the handler path is genuinely bare — everything
> stated here about `workflows.rs::signal` and `ConcreteSignalRouter::route` is
> accurate. What it misses is that admission was moved UP, into
> `engine/delegated.rs::signal`, above both of those. Reading the two sites the
> brief named and concluding from their absence was the error: the brief was
> written before the fix existed, and I checked the brief's citations rather
> than the current call path. Kept rather than deleted, because the shape of
> the mistake is worth more than a clean document — a stale brief cites stale
> lines, and a defect can be fixed at a site the brief never mentions.

`crates/aion-server/src/api/handlers/workflows.rs::signal` (166-204) resolves
the run, checks terminal status, and calls `engine.signal(…)`. There is no
schema consulted anywhere on the path — the handler's only payload check is
`required_payload`, which establishes presence, not shape.

`ConcreteSignalRouter::route` (`crates/aion/src/signal/router.rs:35-60`) then
takes the recorder lock, checks for terminal history, and calls
`record_signal_received(…)` with the payload opaque. The declared type is never
looked at.

So the two boundaries are asymmetric today: start has an admission step and
signal has none. That asymmetry is the whole of #97-A, and it is why the brief
insisted the two be considered together — the start side already shows the
shape the signal side needs.

### What the signal side can reuse

- `aion_package::admit_value` / `declares_nothing` — the same functions the
  start side uses, already a dependency of `aion`.
- `PackageContract::signals: Vec<SignalContract>`
  (`crates/aion-package/src/contract.rs:40`), each carrying its own
  `input_schema` (encoded at `contract.rs:120`, so it is part of the contract
  identity hash).

### What it cannot reuse, and must solve

Start resolves its contract from `workflow_type` before the run exists. A
signal targets a run that is ALREADY RUNNING, so the contract has to be
resolved from the run's pinned package version rather than from a routed
lookup — signalling a parked run must measure against the version that run was
started on, not whatever is route-active now. A run parked for weeks across a
redeploy is precisely the case this exists to protect, and resolving the wrong
version would refuse valid payloads on exactly those runs.

That is the open design question for the build, and it is the reason this is
not a five-line change.

---

# VERIFIED — #97-A was already built, by a lane that was cut off

Continued 2026-07-31 at main `46bbdced`.

The measurement above said #97-A "is the work". That was wrong the moment it
was written, and the correction matters more than the original reading: the
signal boundary IS built. The engine's `signal` path calls
`crate::signal::admission::admit_against_handle` (resident) or
`admit_against_history` (non-resident) before every record path
(`engine/delegated.rs:247-312`), and `crates/aion/src/signal/admission.rs`
resolves the declaration from the identity the run is PINNED to — the live
handle's `loaded_version` when resident, the recorded `WorkflowStarted`'s
`package_version` when not. That is the right ruler: a run parked across a
redeploy is measured against the version it started on, not whatever is
route-active now, which is the one case this exists to protect.

**The whole of it sits under a single commit titled `wip(boundary):
declared-contract-at-the-boundary lane, agent cut off pre-commit`
(`16ba5157`).** So it arrived on main as unverified work wearing the shape of
finished work. It was verified here rather than assumed.

## What was checked, and how

**It runs, and it is not vacuous.** `cargo test -p aion-rs --test
declared_contract_boundary` — exit 0, 7 passed, 0 failed. Then, with BOTH
admission calls removed from `engine/delegated.rs::signal`, the same binary
gives exit 101 and **exactly the three signal tests fail**:

```
an_undeclared_signal_name_is_refused_without_recording ... FAILED
a_refused_signal_payload_leaves_the_history_byte_identical_and_the_run_parked ... FAILED
an_undecodable_payload_is_refused_without_recording ... FAILED
```

The other four stay green, correctly — they are the admits-everything cases and
the start-side test, none of which that code decides. A suite that had been
written to its own answer would not have moved. This one moves precisely where
it should.

**The ordering rule the brief made binding is the thing the tests assert.**
`a_refused_signal_payload_leaves_the_history_byte_identical_and_the_run_parked`
is the acceptance criterion by name.

**The caller sees it as their own mistake.** `EngineError::SignalRefused` →
`error.rs:616` → `declared_contract_wire` → `WireError::invalid_input` →
`WireErrorCode::InvalidInput` → `StatusCode::BAD_REQUEST`
(`api/http/error.rs:30`). HTTP 400, as ruled, with the reason string carried.

**An unknown signal name is a refusal, not a silent no-op** —
`SignalRefusalReason::Undeclared` at `catalog_admission.rs:115-119`, and it
names the signals that ARE declared, so the operator can see their typo.

## The deliberate admit-anyway branches, and what they cost

Three paths admit rather than refuse, each logged. They are defensible and they
are also the honest limit of this fix:

1. Contract declares no signals at all → admit (`catalog_admission.rs:112`).
2. The run's pinned identity is no longer retained in the catalog → `debug!`
   and admit (`:94-101`). A run parked long enough for its version to be
   unloaded loses this protection.
3. The recorded package version is not a canonical content hash → `warn!` and
   admit (`admission.rs:81-92`).

Each trades protection for availability, and each is the right trade in
isolation — refusing would strand a live run over something the caller cannot
influence. Worth stating plainly: **the protection is strongest exactly where
runs are young and weakest exactly where they are old**, which is the inverse
of where the original defect hurt most. Not a reason to change it now; a reason
to know it.

## What is NOT done

The brief's acceptance is explicit that a green test is not enough: "Not a
green test — a demonstrated behaviour… Do this against a real server and quote
the recorded bytes." **That live demonstration has not been run.** Everything
above is bench evidence — strong bench evidence, including a mutation check,
but not the thing the brief asked for.

Recorded as owed rather than claimed.

---

# The hole under the fix: AWL's signal declarations were ungated

Found while verifying the above, 2026-07-31.

`crates/aion/src/signal/admission.rs` is only as good as what reaches
`PackageContract::signals`. For an AWL document the ONLY thing that puts
anything there is one `.map()` in `compile.rs` over `document.signals`.

**Nothing gated that mapping.** The single contract golden in `aion-awl`
(`retry_counting.rs`, `contract-goldens/retry_three_every_1s.hex`) pins a
document that declares NO signal, so the signal bytes it commits are empty
either way.

Measured, not argued. With the mapping collapsed to `Vec::new()`:

```
cargo test -p aion-awl   ->  exit 101
test result: FAILED. 0 passed; 3 failed
```

and those 3 are the three tests added here. **Every other AWL binary stayed
green** — 44 of them, 489 tests — with signal declarations compiling to nothing
at all.

That is the full failure chain, and it is silent end to end: the mapping
regresses → an AWL package deploys with `signals: []` →
`catalog_admission.rs:112` reads an empty vector as "this package declares
nothing" and returns `Ok(None)` → every payload is admitted → the P0 is back,
with no test anywhere going red and no log line saying so. The protection that
exists to stop an operator's typo from killing a durable run would have been
switched off by a green build.

## The gate

`crates/aion-awl/tests/declared_signal_contract.rs`, three pins that assert the
PROPERTY rather than a shape — a hardcoded copy of the derived schema would
pass just as well if the schema stopped discriminating:

1. **every_declared_signal_reaches_the_contract** — the contract's signal names
   equal the PARSED document's, compared against the document itself rather
   than a hardcoded list, plus a non-vacuity guard that the fixture declares at
   least one signal.
2. **the_declared_schema_admits_conforming_and_refuses_malformed** — the schema
   discriminates: a conforming payload is admitted and one missing the required
   `end` field is REFUSED. A schema that were present but vacuous would satisfy
   pin 1 and still pass every typo through.
3. **each_signal_carries_its_own_declared_schema** — two declarations do not
   share a ruler. `ping`'s payload must NOT satisfy `control`'s schema, which
   is only observable if the per-signal mapping is genuinely per-signal.

All three verified failing at my own hand by the injection above, and green on
the restored tree: `cargo test -p aion-awl` exit 0, 45 binaries, 492 passed, 0
failed.

---

# 🔴 RESUME HERE — 2026-07-31 ~05:15Z

State: `origin/main` at `656a3327` plus ONE UNCOMMITTED addition — the gate
above (`crates/aion-awl/tests/declared_signal_contract.rs`) and this section.
Working tree otherwise clean; `RELEASE-0.9.2.md` is pre-existing untracked.

Done this session: #110 closed and pushed (`46bbdced`); #95 closed by
measurement; #97-A verified as built-by-another-lane and holding; the
`aion-server` debt discharged (exit 0, 863 passed, 0 failed).

**OWED, and held by ruling — do not just run it.** The brief's acceptance for
#97 is a live demonstration against a real server quoting recorded bytes. It
needs `cargo build -p aion-cli`, a full debug link at minutes-scale, which sits
past the freeze's allowed class. Cally ruled it QUEUED behind Waffles's
all-clear, as its own named item, with a fresh disk reading taken first. The
debt survives the hold; #97 closes on quoted bytes and not before.

Do NOT take the shortcut: `target/debug/aion` postdates the fix and carries the
refusal strings, but its provenance is unknown, and a binary's presence on disk
is not a citation.

Also do not restart Tom's server (#108 — it is running from a deleted binary
and cannot be restarted).

Next after the all-clear, in order: (1) the #97 live proof; (2) task #106, the
one real remaining gap on the start side — a declared schema that cannot
COMPILE warns and admits, so validation is absent for exactly the packages
whose declarations are broken. The fix shape worth considering is refusing at
DEPLOY time, when the schema is derived, rather than admitting at start: a
package should not be deployable with an uncompilable declared schema.
