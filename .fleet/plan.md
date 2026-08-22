# PLAN — policyref-flight1: PolicyRefused class + declared provider fallback

Planning counsel, 2026-08-22. Tree: `fleet/policyrefused-base` @ `ccab2bc7`.
Every coordinate below was re-derived on THIS tree by symbol. The brief's seams
were explored on `89ab967a` (regate/inv87r2); where a coordinate moved it is
named as MOVED with both numbers.

Governing documents, read in this order and in force in this order:

1. `.fleet/BRIEF.md` §CORRECTION (lines 3–27) — binding, outranks the body.
2. `docs/evidence/policy-refused-census-20260822.md` — the measurement.
3. `.fleet/BRIEF.md` body.

The one-line holding: **a provider policy refusal is STOCHASTIC, not
deterministic for its content.** Exhibit A (`4ddee56c`, seq 9 refused / seq 11
completed, same act) and Exhibit B (aion#65 round 3) are independent survivals.

---

## 0. MISSED SITES — where the body still contradicts the correction

The dispatch note says the body has been swept so every downstream restatement
matches the correction. **It has not been swept completely.** Two sites survive,
and both are load-bearing — they are the exact two propositions the correction
names as refuted. Per the standing instruction I name them here and follow the
correction, not the leftover.

### MISSED SITE 1 — `.fleet/BRIEF.md:86-87`, R1(b)

> "The exhaustive `is_deterministic()` match gets its arm decided ON PURPOSE
> (the crate's own doc :190-194 demands exactly this): **deterministic — this
> provider will refuse this content every time.**"

This is the refuted premise (b) restated verbatim as a settled conclusion. The
correction (BRIEF.md:7-8, 16) names (b) as CONTRADICTED. Exhibit A refutes
"every time" directly: the same provider accepted the same content on attempt 2.

**Ruling for this flight:** R1 does NOT get to assume this arm. The
`is_deterministic()` arm for `PolicyRefused` is an OPEN design question the
artifact must decide **with the census in hand**, and per the correction
(BRIEF.md:23-25) a `true` answer must NAME THE MECHANISM by which a refusal that
demonstrably cleared on attempt 2 is deterministic. The measured default is
`false`. See L1-R4.

### MISSED SITE 2 — `.fleet/BRIEF.md:116-118`, R1(d)

> "No chain declared ⇒ today's behavior minus the waste: **fail FAST as
> `policy_refused:` (no in-place retries), not after N identical attempts.**"

This is the refuted proposal (d) verbatim — "no in-place retries" is the
single-attempt collapse. The correction (BRIEF.md:20-23) replaces it: *"with NO
chain declared, a bounded same-provider retry REMAINS, its bound a CONFIGURED
value defaulting to what the act already declares."* R3.3 (BRIEF.md:147-151) was
swept and now reads correctly; R1(d) was not, so the design section and its own
green criterion now disagree with each other.

**Ruling for this flight:** R1(d) is superseded. The no-chain path keeps a
bounded same-provider retry. The bound is `RetrySpec.count` as already authored
(`crates/aion-awl/src/ast/document.rs:561-580`) resolved through
`retry_policy_from_config` (`crates/aion/src/runtime/nif_activity_retry.rs:206`).
**No number is minted anywhere in this flight.** What is removed is the
mislabelling and the burn against a chain that was never consulted — not the
retry.

### Consistent-after-sweep (checked, no action)

Summary ¶ (:29-40, "any same-provider retry bounded by the act's CONFIGURED
budget"), R2 (:125-136), R3.3 (:147-151), closing sentence (:172-176). These
match the correction.

---

## 1. THE FIVE SEAMS, re-derived on this tree

Anchor on the symbol. Line numbers are reported for navigation only.

### Seam 1 — the adapter flattens the provider error to prose

| | Brief | This tree | Status |
|---|---|---|---|
| norn wrap site | `session.rs:289` | `crates/aion-integration-norn/src/session.rs:288-291`, in `response_into_payload` | **holds** |
| `HarnessError::Harness` | `error.rs:78` | `crates/aion-integrations/src/error.rs:79-82` (variant); doc at `:73-78` | holds |
| enum span | `error.rs:16-113` | `error.rs:14-115`; `#[non_exhaustive]` at `:15` | MOVED +2 |

```rust
// crates/aion-integration-norn/src/session.rs:286-292
fn response_into_payload(response: JsonRpcResponse) -> Result<Payload, HarnessError> {
    if let Some(error) = response.error {
        return Err(HarnessError::harness(format!(
            "run/execute failed (code {}): {}",
            error.code, error.message
        )));
    }
```

The brief says the structured object "is flattened to prose and never
inspected." **Confirmed, and it is worse than the brief states** — see §2.

### Seam 2 — `Harness` classifies retryable

- `HarnessError::is_deterministic()` — `crates/aion-integrations/src/error.rs:222`,
  body `:223-232`, `Self::Harness { .. } => false` at `:230`.
  **MOVED** (brief: `:223-233`).
- The exhaustive-match doctrine the brief cites as ":190-194" is at
  `error.rs:190-195` ("EXHAUSTIVE match — legal despite `#[non_exhaustive]` — so
  adding a variant is a compile error at this site"). **MOVED +1.** The doctrine
  is real and is the mechanism that will force `PolicyRefused` to be classified
  on purpose.
- `harness_error_to_outcome()` — `crates/aion-worker/src/runtime/agent.rs:366`,
  body `:366-375`. **MOVED** (brief: `:366-378`). Its stale-able doc block is
  `:335-364` (brief: `:335-364` — holds).
- Catch-all match sites the brief lists as `agent.rs:244-248` / `:327-331`: on
  this tree `deliver_dyn_command` at `:244-248` and `apply_command` at
  `:327-331`. **Both hold.** Both are `Err(HarnessError::CapabilityNotSupported)`
  / `Err(HarnessError::StaleTarget)` arms followed by a wildcard — they are on
  the INTERVENTION path, not the result path, so a new variant is silently
  swallowed there. Confirmed as needing the sweep.

### Seam 3 — the retry loop forecloses per-attempt variation

- `dispatch_with_retries` — `crates/aion/src/runtime/nif_activity_retry_dispatch.rs:247`,
  loop `:260-...`. **MOVED** (brief: `:247-330` — the symbol is at 247, so the
  head holds; the interior moved).
- The clone-and-mutate-only-`attempt`:
  `let mut delivery = request.clone(); delivery.attempt = attempt;` at
  **`:261-262`**. **MOVED −3** (brief: `:264-265`). This is the seam that matters:
  nothing but `attempt` varies between attempts, so a same-provider retry is
  byte-identical except for a counter.
- Brief's negative test `harness.rs:639-645`: on this tree
  `unrecognised_braced_text_is_literal` at
  `crates/aion-integration-norn/src/harness.rs:637-646`. **MOVED −2, and the
  brief's characterisation needs narrowing:** the test does not pin "per-attempt
  provider variation is foreclosed by design" as a policy. It pins that
  `{attempt}` is NOT an argv placeholder — only `{workflow_id}` and
  `{activity_type}` are (`:639-640`). The *effect* the brief describes is real
  (argv cannot carry the attempt number, so the spawned command is identical
  across attempts); the *intent* attributed to the test is not in it. **Report
  as a coordinate + characterisation correction.**

### Seam 4 — `failure_stand_down` wraps exhaustion

- `failure_stand_down` — `nif_activity_retry_dispatch.rs:183`, body `:183-203`.
  **Holds** (brief: `:183-203`).
- The `exhausted:` format string at **`:197-200`** (brief cited `:194` for the
  retry-eligibility line — on this tree `let retry_eligible = expired ||
  is_retryable_reason(reason);` is at **`:194`**, **holds exactly**). This is the
  line the brief says "must not count it retry-eligible", and it is the correct
  coordinate.
- Call site: `:288-293`.

### Seam 5 — the Gleam grammar has no arm

- `parse_error/1` — `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:134`,
  body `:134-162`. **MOVED** (brief: `:134-160`).
- **The brief undercounts the vocabulary.** It names three prefixes; the grammar
  has FIVE: `retryable:` (`:135`), `terminal:` (`:138`), `timeout:` (`:141`),
  `cancelled:` (`:145`), `non_determinism:` (`:151`). The fallthrough to
  `error.ActivityEngineFailure(message: raw)` is at **`:156`**.
- `encode_error/1` — `:178-196` (brief: "`:134-195`" bundled both; the encode
  twin is `:178`). **MOVED.**
- `describe_activity_error/1` — `gleam/aion_flow/src/aion/awl/error.gleam:160`,
  body `:160-175`, `ActivityEngineFailure` arm at `:172-173`. **Holds**
  (brief: `:160-175`).
- `mock.gleam` — `gleam/aion_flow/src/aion/testing/mock.gleam`, encode arms
  `:84-89`. **MOVED** (brief: `:80-90`).

---

## 2. THE LOAD-BEARING FACT: where the structured refusal code is last available

The brief's R1(a) says: *"Prefer the provider's STRUCTURED code (`cyber_policy`
in the sse error event) over substring matching wherever the adapter still holds
the structured object; name where that object is last available."* This is the
one fact the whole design rests on. Here is what this tree actually shows.

### 2.1 `cyber_policy` is not a symbol in this tree — but a tracked log carries the frame

```
grep -rn 'cyber_policy' . → .fleet/*, docs/evidence/…, AND
                            gate-logs/dev-worker-0150.log:589
```

Zero hits in `crates/`, `gleam/`, `sdks/`. The code is a fact about the
**provider's** wire — it is not a symbol this tree can be anchored on, and no
test fixture carries it. **The design leg must not assume the spelling.** It must
key on the structure that carries it.

🔴 **AMENDED 2026-08-22 (re-fly verification).** The original text of this section
said the grep hit "only `.fleet/BRIEF.md` and `docs/evidence/…`". That was
incomplete. `gate-logs/dev-worker-0150.log` is **tracked at HEAD** (298423 bytes)
and is the only in-repo artifact carrying the real frame. The ruling above is
UNCHANGED; what follows is added evidence, and it is directly responsive to what
L1-a asks the artifact to find.

**Exhibit C — the frame, and the prose it becomes.** Two consecutive lines:

```
:589  ERROR norn::provider::openai::sse: provider emitted a standalone
      Responses error event frame={"type":"error","error":{"type":
      "invalid_request","code":"cyber_policy","message":"This content was
      flagged for possible cybersecurity risk. …","param":null},
      "sequence_number":13}
:590  norn: agent error: provider error: invalid request: provider rejected
      the request under its cybersecurity policy
```

`:589` is emitted by `norn::provider::openai::sse` — **inside the norn binary**.
`:590` is what norn hands out. **The string `cyber_policy` does not appear in
`:590`.** So §2.2's conclusion is no longer an inference: the structured code is
consumed inside norn and never crosses the boundary. What reaches
`session.rs:288-291` is prose that has already lost the discriminator.

This gives L1-a decision #3 ("what happens when `error.data` is `None`") a
measured, in-repo answer and the exact substring with provenance that
`.fleet/legs/l1-design.md:59-65` requires: the observed carrier is
`"provider rejected the request under its cybersecurity policy"` at
`gate-logs/dev-worker-0150.log:590`. A prose match ADMITTED and bounded is the
branch the evidence points at; the artifact still has to say so in those words.

**The discriminator exists upstream and is thrown away.** The same log carries
two other SSE error codes — `server_error` (×3: `:82`, `:476`, `:628`) and
`context_length_exceeded` (`:117`). The frame is a stable structured family whose
`code` separates a transient fault, a deterministic input fault, and a policy
refusal. None of that separation survives to aion. That is the defect stated at
its true origin.

**Same-act attempt burn, visible in the log.** Workflow
`192f954f-c4c4-452e-83b1-400cd6857875` ordinal 0 is dispatched at `:586` (attempt
1), `:591` (attempt 2), `:605` (attempt 3), `:641` (attempt 4) — the four-attempt
pattern the census records. Note the limit of this exhibit: the log holds exactly
ONE `cyber_policy` frame and a `server_error` frame at `:628` falls between
attempts 3 and 4, so it does NOT establish that all four attempts were refusals,
and it does not show whether attempt 2 cleared. Cite it for the burn, not for a
survival — Exhibits A and B remain the survival evidence.

### 2.2 There is no "sse error event" surface in this adapter

`crates/aion-integration-norn/src/protocol.rs` is, by its own module doc
(`:1`), *"the ONLY module in the workspace that names"* norn's wire. It declares
`envelope_version` / `stop` / `stop.reason` / `output` / `session_id` and the
`event/*` notification labels (`:66-101`). **It declares no error-object schema
and no refusal code key at all.** aion never sees an OpenAI SSE frame; it sees
norn's JSON-RPC. Whatever norn does with the provider's SSE error happens inside
the norn binary, out of this tree.

**So the brief's phrase "the sse error event" describes a surface that does not
exist at this boundary.** CORRECTION to R1(a).

### 2.3 The structured object that DOES exist, and where it dies

There are exactly **two** points at which a norn failure is still structured, and
both are inside the adapter:

**(A) `JsonRpcError` — the attested path.**

```rust
// crates/aion-integrations/src/jsonrpc/envelope.rs:110-118
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,   // ← :117
}
```

`data` is **parsed and retained** by the transport. It is the only field that
could carry a machine-readable refusal code. And
`crates/aion-integration-norn/src/session.rs:288-291` **drops it on the floor** —
the `format!` reads `error.code` and `error.message` only. `data` is never
inspected, never logged, never persisted.

This is the empirically attested path. `crates/aion-integration-norn/src/advice.rs:273-276`
carries a refusal transcribed off a REAL run:

```
"run/execute failed (code -32603): agent error: session error: … fresh-epoch
 projection; explicit resume approval is required"
```

That is `session.rs:288`'s exact format string, and the code is `-32603` — JSON-RPC
generic internal error. **The JSON-RPC `code` is therefore NOT a discriminator:**
norn reports every application-level failure under `-32603`. Any structured
refusal code must be in `error.data`, or nowhere.

**(B) The stop envelope — the second flattening point.**

```rust
// crates/aion-integration-norn/src/translate/outcome.rs:61-91
let stop = result.get(protocol::STOP_KEY)…;          // :61  — &Value, structured
let reason = stop.get(protocol::STOP_REASON_KEY)…;   // :67  — &str, structured
if reason != protocol::STOP_REASON_COMPLETED {
    let mut message = format!("run stopped without completing: stop: {stop}");  // :83
    …
    return Err(HarnessError::harness(message));       // :91
}
```

`stop` is the whole structured object at `:83` and is `Display`-flattened into a
String on that line. `reason` at `:67` is a typed discriminator.

### 2.4 VERDICT on R1(a) — the brief's location is right, its reason is wrong,
### and its proposed detection seam is structurally incapable of the job

**Confirmed:** the last point at which the structured object is available is
inside the norn adapter, at `session.rs:287` (the `JsonRpcError` binding, whose
`.data` dies at `:288-291`) and at `translate/outcome.rs:61-83` (the `stop`
`&Value`, which dies at `:83`). The brief's `session.rs:289` lands inside the
first of those. **The coordinate holds.**

**Corrected:** the structured thing available there is not an SSE error event; it
is `JsonRpcError { code: i64, message: String, data: Option<Value> }`, whose
`code` is a constant `-32603` and whose only usable structured channel is `data`.

**The finding the brief does not contain, and which changes the design:** R1(a)
proposes `AdvisingHarness` (`crates/aion-integration-norn/src/advice.rs`,
composition root `crates/aion-cli/src/harness/compose.rs:52-59` — **MOVED**,
brief said `:54-59`) as the template for the detection seam. **That decorator
cannot prefer the structured code, because it never sees one.** `advise()` at
`advice.rs:139-141` takes a fully-constructed `HarnessError` and destructures
`HarnessError::Harness { detail }` — a `String`. The decorator sits ABOVE
`session.rs`; by the time it runs, `data` has already been discarded at
`:288-291` and `stop` has already been `format!`-ed at `outcome.rs:83`. **Placing
re-classification in `AdvisingHarness` forces substring matching — the exact
thing the brief says to prefer against.** The template is a template for
*decoration*, not for *detection*.

This is the single most consequential correction in this plan and it is what L1
must design around: **detection belongs inside the adapter, at or before the
flattening lines, not in the decorator above it.**

### 2.5 ACP CAN express a policy refusal — a typed enum variant, not prose

The brief asks to cover the ACP surface *"or state with evidence why ACP cannot
express a policy refusal."* **It can, and far more cleanly than norn.**

- `StopReason::Refusal` is a first-class variant of the ACP protocol's stop
  reason: `crates/aion-integration-acp/src/translate.rs:291` maps it to the label
  `"refusal"`; it is exercised at `:657` and `:678`.
- It is flattened at `crates/aion-integration-acp/src/outcome.rs:169-177`
  (`non_completion`), which folds `Refusal`, `MaxTokens` and `MaxTurnRequests`
  into one prose `HarnessError::harness("the prompt turn did not complete:
  stopReason: {label}")` at `:173`/`:176`. The brief's `outcome.rs:180-204` is
  `prompt_error` — the JSON-RPC-error path, not the stop-reason path.
  **MOVED / MIS-AIMED:** the equivalent surface is `:130-177`, and the load-bearing
  discriminator is `prompt.stop_reason` at `outcome.rs:152-154`.

ACP therefore gives a **typed** detection point (`matches!(prompt.stop_reason,
StopReason::Refusal)`) with no substring matching at all. norn gives, at best,
`error.data`. The census's "provider asymmetry: suggestive, NOT proven" (evidence
doc :70-74) is orthogonal — the *expressive* asymmetry is proven right here in
the tree, and it is in ACP's favour.

---

## 3. FINDINGS THAT ARE NOT IN THE BRIEF

These were observed while re-deriving and materially affect the design. They are
findings, not scope.

### F-1 (BLOCKING for R1(c)) — the worker→engine reason wire is structurally binary

`crates/aion-server/src/worker/bridge.rs:475-490`:

```rust
ActivityCompletionOutcome::Failed(error) => {
    let prefix = if error.is_retryable() { "retryable" } else { "terminal" };
    …
    Err(format!("{prefix}:{}", error.message))
}
```

Every worker-originated reason string is minted **here**, from
`ActivityErrorKind`'s two variants and nothing else. There is no third prefix a
worker can produce. `timeout:`, `parked:`, `lost:`, `superseded:`,
`transport-exhausted:` and `exhausted:` are all synthesised ENGINE-side, never by
a worker.

Consequence: R1(c)'s "either is acceptable" is not symmetric on this tree. A
`policy_refused:` prefix originating at a worker requires **either** a variant on
`ActivityErrorKind` (`crates/aion-core/src/error.rs:9-14`) — a wire + TS-codegen
event, registered at `crates/aion-core/src/generated_types.rs:222`
(`push_type::<ActivityErrorKind>`; brief's ":222" **holds exactly**) — **or** a
change at `bridge.rs:475-490` to mint a third prefix from something other than
`is_retryable()`. The artifact must pick one and say so; it cannot leave it open.

### F-2 (ENABLING) — a structured detail channel already exists end-to-end and is unused

- `ActivityFailure::with_detail(Payload)` — `crates/aion-worker/src/activity.rs:52-57`.
- Survives into `ActivityError.details` — `activity.rs:99-107` (`:104`).
- Crosses the worker protocol — `crates/aion-worker/src/protocol/session.rs:832`.
- Is **durably recorded**: `Event::ActivityFailed { error: ActivityError, attempt }`
  — `crates/aion-core/src/event.rs:469-478` — carries the whole `ActivityError`,
  `details` included.
- And `harness_error_to_outcome` (`agent.rs:366-375`) **never calls it**: both
  arms construct a bare `ActivityFailure::terminal/retryable(error.to_string())`.

So there is a live, durable, type-erased structured channel from the adapter to
history that costs no wire change and no TS regeneration, and today it carries
nothing on the agent path. It is a strong candidate for carrying the refusal
code (and the refusing queue) without touching `ActivityErrorKind`. **Caveat:**
`bridge.rs:490` reads only `error.message`, so `details` reaches history but does
NOT reach the reason string the retry loop and the Gleam grammar parse. L1 must
say how the two are reconciled.

### F-3 (REPORT ONLY — do not fix, per BRIEF.md:134-136) — the fallthrough mislabelling

`activity_dispatch.gleam:156` folds every unrecognised prefix into
`error.ActivityEngineFailure`. The brief names `exhausted:` and
`infrastructure:`. On this tree the actual population is larger:

- `exhausted:` — minted at `nif_activity_retry_dispatch.rs:198`. No Gleam arm.
- `parked:` — `nif_activity_retry.rs:37`, `:49`. No Gleam arm.
- `lost:` — `nif_activity_retry.rs:76`. No Gleam arm.
- `transport-exhausted:` — named at `nif_activity_retry_dispatch.rs:277`. No arm.
- `superseded:server-death` — `nif_activity_retry.rs:382`. No arm.
- `infrastructure:` — **the brief's second example does not exist as a reason
  prefix on this tree.** The only `infrastructure:` strings are
  `crates/aion-server/src/worker/outbox_redrive.rs:390` and
  `outbox_dead_letter.rs:82`, and both are the *message body* of an
  `ActivityError`, not a reason prefix — so they reach the operator as
  `retryable:infrastructure: …` / `terminal:infrastructure: …` via
  `bridge.rs:490`, which DOES parse. **Correction to the brief's example.**

Five engine-side prefixes present to the operator as "activity engine failure".
This deserves its own item and is explicitly out of scope here.

### F-4 — no policy refusal tripped this planning pass

Per the standing instruction: this planning work did not trip a provider policy
refusal. Nothing to report as first-class evidence. Recorded so its absence is
not mistaken for an omission.

---

## 4. THE LEG SPLIT

Design-first is mandatory (R1 lands before implementation). Three legs, strictly
sequential.

```
L1  design      → docs/design/policy-refused-fallback.md          [BLOCKING]
      ↓ consumes: this plan §1–§3
L2  classify    → PolicyRefused class + adapter detection + Gleam arms
      ↓ consumes: L1's committed artifact
L3  route       → AWL `fallback` + engine chain consultation + replay + E2E
```

**Why three, not two.** L2 and L3 touch near-disjoint crate sets — L2 is
`aion-integrations` / `aion-integration-norn` / `aion-integration-acp` /
`aion-worker` / `gleam`, L3 is `aion-awl` / `aion` / `aion-server`. L2 is
independently green-able and carries R3.1 and R3.4 as its own gate. L3 carries
all the replay-determinism risk and is the leg where the F6 measured-red outcome,
if it happens, will happen — isolating it means a red L3 does not strand a
correct L2.

**Why not more.** Splitting L2's two adapters apart would put the shared
`HarnessError::PolicyRefused` variant on two branches at once; the exhaustive
`is_deterministic()` match (`error.rs:222-232`) makes that a guaranteed conflict.

**Landing constraint (from BRIEF.md:127 — "Both halves land together").** L2 and
L3 are separate legs but ONE landing. L3 branches from L2's head; the merge to
the flight base is L3's head, carrying both. Neither half reaches the base alone.

**Blocking hand-offs.**

- L1 → L2: L2 cannot begin until `docs/design/policy-refused-fallback.md` is
  committed. L2 implements the artifact's decisions, and only those.
- L1 → L3: same artifact; L3 additionally needs L1's (d) and (e) decisions.
- L2 → L3: L3 needs `HarnessError::PolicyRefused` and the reason vocabulary to
  exist before it can route on them.

**If L1 concludes measured red** (the fallback hop cannot be made
replay-deterministic within scope): that conclusion, committed with evidence in
the artifact, IS the deliverable. L3 is then descoped to nothing and L2 lands
alone — classification without routing is still the larger half of the cure, and
the correction's core requirement (never burn a same-provider retry budget
mislabelled as transient) is satisfied by L2 alone. Say so explicitly; do not
improvise a partial hop.

---

## 5. WALLS THAT APPLY TO EVERY LEG

Restated so no leg has to re-derive them.

- No lint suppression in any spelling: `#[allow]`, `#[expect]`, `#[deny]`,
  `_var` renames, `#[cfg(any())]`. If clippy fires, fix the code.
- No `#[ignore]` in any spelling. A test needing a live runtime gates it AT
  RUNTIME: read an env var, `tracing::info!` a skip line, `return Ok(())`.
- No file over 500 lines of code. `crates/aion-integration-norn/src/harness.rs`
  is already 1064 lines and `crates/aion/src/runtime/nif_activity_retry.rs` is
  905 — **do not add to either**; new logic goes in new named modules.
  `crates/aion/src/runtime/nif_activity_retry_dispatch.rs` is 609.
- `thiserror` for library errors. Never `.unwrap()` / `.expect()` in library
  code. Lock poison mapped to a typed error.
- `mod.rs` holds only `pub mod` and re-exports.
- Wire/TS changes, if L1 chooses them, are REGENERATED. Never hand-edit
  generated types. The registry at `crates/aion-core/src/generated_types.rs:210-234`
  is hand-maintained and its own module doc (`:3-19`) records the 2026-07-30
  break from forgetting an entry — add the entry if a type is added.
- ⛔ Never one blocking sleep sized to the expected duration. Poll, with a total
  deadline, and timeout as its own distinct outcome.
- 🔴 **Never a test asserting the no-chain path fails after exactly ONE attempt**,
  whatever any text says. That pins as a passing assertion the behaviour that
  would have killed `4ddee56c`. Assert the CLASSIFICATION and that NO chain hop
  was attempted. Assert no particular attempt count.
- 🔴 **Do not mint the bound.** It is configured, defaulting to what the act
  already declares (`RetrySpec.count`, `document.rs:561-580`). "1" is
  affirmatively refuted by measurement; "2" is an assumed default.
- Do NOT widen scope to F-3. Report it; do not fix it.
- `git ls-tree` receipt before any leg declares done.
- If a leg's own work trips a provider policy refusal: report it as a
  first-class finding with what tripped and where. It is evidence for this brief.
  Do not rewrite the work to dodge the trigger.

---

## 6. WHERE THIS PLAN SAYS THE BRIEF IS WRONG — index

| # | Brief | This tree |
|---|---|---|
| M1 | `:86-87` R1(b) asserts `PolicyRefused` is deterministic | Refuted premise, unswept. Arm is OPEN; measured default `false`; a `true` answer must name the mechanism |
| M2 | `:116-118` R1(d) no-chain ⇒ fail fast, no in-place retries | Refuted premise, unswept. Bounded configured retry REMAINS |
| C1 | `:50-51` structured refusal is an "openai sse error event" | No SSE surface at this boundary. `protocol.rs` declares no error schema. The structured carrier is `JsonRpcError.data` (`envelope.rs:117`), dropped at `session.rs:288-291` |
| C1b | — (amendment to THIS plan, 2026-08-22 re-fly) | §2.1's grep result was incomplete. `gate-logs/dev-worker-0150.log` is tracked at HEAD and carries the frame at `:589` and norn's prose at `:590`; `cyber_policy` is absent from `:590`. Ruling unchanged; evidence added. See §2.1 Exhibit C |
| C2 | `:71-75` `AdvisingHarness` is the detection template | It only ever sees `HarnessError::Harness { detail: String }` (`advice.rs:139-141`). It CANNOT prefer a structured code. Detection must live inside the adapter |
| C3 | `:79-81` ACP equivalent surface is `outcome.rs:180-204` | That is `prompt_error` (JSON-RPC path). The refusal surface is `StopReason::Refusal` → `outcome.rs:152-154`, `169-177`. ACP expresses it TYPED |
| C4 | `:60` `harness.rs:639-645` is a negative test on per-attempt provider variation | `:637-646`, `unrecognised_braced_text_is_literal`. Pins that `{attempt}` is not an argv placeholder. Effect is as described; intent is not in the test |
| C5 | `:96-99` wire-vs-prefix "either is acceptable" | Not symmetric. `bridge.rs:475-490` mints prefixes solely from `ActivityErrorKind`. See F-1 |
| C6 | `:135` `infrastructure:` is a fallthrough prefix | Not a reason prefix on this tree; it is a message body and DOES parse. The real unparsed set is `exhausted:`, `parked:`, `lost:`, `transport-exhausted:`, `superseded:` |
| C7 | `:64-65` Gleam grammar has three prefixes | Five: `retryable:`, `terminal:`, `timeout:`, `cancelled:`, `non_determinism:` |
| — | line coordinates | Moved: `error.rs` enum +2, `is_deterministic` body, `agent.rs:366-375`, dispatch clone `:261-262` (−3), `harness.rs:637-646` (−2), `parse_error` `:134-162`, `encode_error` `:178`, `mock.gleam:84-89`, `compose.rs:52-59`. Held exactly: `session.rs:288-291`, `failure_stand_down:183-203`, `:194`, `error.gleam:160`, `generated_types.rs:222`, `agent.rs:244-248`/`:327-331`, `agent.rs:335-364`, `document.rs:540-541` |

---

## 7. LEG BRIEFS

- `.fleet/legs/l1-design.md` — the design artifact. Blocking.
- `.fleet/legs/l2-classify.md` — classification + Gleam grammar.
- `.fleet/legs/l3-route.md` — fallback chain + replay + E2E.
