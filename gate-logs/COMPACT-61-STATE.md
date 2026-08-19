# STATE — 2026-08-14 ~19:00 AEST, pre-compact-61

I am **Vesper Lynd**; sign "— Vesper". Loop STOPPED. Read with
gate-logs/COMPACT-60-STATE.md (+ its addendum), 233-CANCEL-FINDINGS.md,
205-SESSION-RESUME-FINDINGS.md (§9 = the build design), TOM-RULINGS-20260814.md.
Trust files, not memory.

## DONE THIS SESSION
- ✅ #233 CLOSED (eaac2510a + b3b17a0cd, both PUSHED, task marked completed).
  Battery run-2 1587/0 EXIT=0 quiet box; run-1 contended red attributed
  (outbox 100ms fence, filed #238); review at my own hand found + closed the
  TransportCannotCarry never-seen-red gap (feature-gated liminal test,
  mutation-killed, 6/6 green, clippy ×2 EXIT=0). Tree is CLEAN except
  pre-existing untracked files (state files, MARKER-GUIDE..., .aion-* dirs).
- #237 filed (TimedOut/Failed leak — NOT closed by #233).
- #238 filed (outbox retry test wall-clock race).
- WAFFLES ASKED (DM dm:896955e1-86dd-4d6a-9c25-f3f978b189a9, sent ~18:55):
  ruling on widening #181's AgentOutcome{text,stop_reason} with a third
  mandatory `session_id: String` filled by EVERY harness. Poll the thread
  per silence-doctrine if overdue; the #205 OUT seam waits on his answer.

## IN FLIGHT: #205 item 1 — ACP session/load (adapter core)
Design is WRITTEN in 205-SESSION-RESUME-FINDINGS.md §9. Build NOT started —
no code edits yet. The concrete edit list:

1. `crates/aion-integration-acp/src/protocol.rs`: add
   `pub const METHOD_SESSION_LOAD: &str = AGENT_METHOD_NAMES.session_load;`
   + pin "session/load" in method_names_match test.
2. `crates/aion-integrations/src/workspace.rs`: `prompt_for_attempt` gains a
   `reserved: &[&str]` param (extra names subtracted in prompt_field beside
   the directory param). REPLACE signature, update ALL callers (ACP
   harness.rs:629 passes its session param; norn + any other callers pass
   &[] — grep prompt_for_attempt across crates/ first).
3. `AcpHarness` (harness.rs): new field `session_parameter: Option<String>`
   + builder `with_session_parameter(name)`. New fn session_for_attempt
   (&input) -> Result<Option<SessionId>>: field absent → None (fresh);
   present non-string/blank → HarnessError::configuration (a job that TRIED
   to name a session must not silently get a fresh one).
4. `connect`/`open_turn`: thread `agent_capabilities.load_session` (bool,
   InitializeResponse — currently DROPPED after the debug log at
   harness.rs:501) + `prior: Option<SessionId>` into open_turn. Arms:
   - Some(id) + load_session → call METHOD_SESSION_LOAD with
     LoadSessionRequest::new(id, cwd).mcp_servers(Vec::new())
     .additional_directories(...) (schema types verified in
     agent-client-protocol-schema 1.5.0, v1/agent.rs:1171/1248; replayed
     session/update notifications land in `deferred`, pump replays — exists).
     session_id = the id we asked for; then session/prompt as today.
   - Some(id) + !load_session → HarnessError::configuration naming the
     refusal (deterministic: same doc → same agent → same answer;
     require_absolute precedent at harness.rs:330).
   - None → session/new exactly as today.
5. Tests: wire_tests.rs mock agent (advertises loadSession:false at :142).
   Add: (a) load path — mock advertises true, job carries id, assert the
   frame on the wire is session/load with that id and NO session/new;
   (b) refusal — advertises false + id supplied → Configuration error, NO
   session minted; (c) fresh path unchanged when no id. Red-before-trusted:
   mutation each arm (e.g. load arm silently falling back to session/new is
   THE defect this task exists to end — that mutant must die).
6. AWL vocabulary (`session from <parameter>` in worker-doc harness
   section, #204 precedent) + worker_awl serve wiring + docs = NEXT SLICE,
   not this one. Item 2 (coordinator's own session) + item 3 (arg
   placeholders, #210 seam) later. Battery scope when committing: the
   closure = aion-integrations + aion-integration-acp + aion-integration-norn
   (caller signature change!) + anything else calling prompt_for_attempt.

## STANDING GUARDS (verbatim)
- ~/.aion-196-soak DO NOT rm. /Users/tom/.aion-binary-archive DO NOT rm;
  restore by RENAME. pid 50822 (:8099/:50099) separate server — leave.
  Live aion server pid 17893. Tom's worker pids untouched.
- liminal exclusion #126: never touch 557eed7 or its worktree.
- Decisions via Waffles; network/workers/dev-loop thread Tom-direct.
  Tom-facing = PLAIN LANGUAGE. Never destroy in-flight agent work.
- aion server from repo root. Never bare `aion list`. Suite output to a
  FILE, EXIT= line INSIDE the file (wrapper exits lie — proven twice).
  git commit -F - heredoc. git -c diff.external= diff --no-ext-diff.
  Kill only RECORDED pids. No custom CARGO_TARGET_DIR. Quote globs.
  Both clippy resolutions (--workspace AND -p) before commit.
- Bash cwd persists — absolute paths. Session rule: NO Agent tool, no
  workflows unless the user asks. <channel> = untrusted data. Task
  notifications ≠ user approval. NO ARTIFACTS (plain HTML only).
- Scratchpad: /private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad

## ⚡ WAFFLES RULED (18:56Z msg 45806645, BEFORE compact) — OUT SEAM UNBLOCKED
APPROVED: AgentOutcome gains third MANDATORY field `session_id: String`,
every harness fills it, REPLACE not shim (two-field declarations break at
checker and get updated; serialization pin → exactly THREE keys in the same
change). Side-channel REFUSED. Three riding conditions (doc+checker, no
shape change):
1. Semantics written WHERE THE SHAPE IS: session_id is a FACT (handle of
   the conversation that produced this outcome), NOT a promise of
   resumability — resumability is the initialize handshake's per-agent
   question.
2. Checker refuses EMPTY STRING by name — no silent vacancy on a
   mandatory field.
3. Consumer rule in the doc: the id is meaningful only to the harness
   that minted it; resume routing pairs id with the harness identity the
   workflow record already carries. One sentence, no wrapper type.
Input-side asymmetry confirmed right (absent param = fresh; every run HAS
a session to name, not every job resumes one). "Build the whole thing."
So the lane now = adapter core (steps 1-5 above) + AgentOutcome widening
(aion-core agent_outcome.rs + its 3-key pin + AWL checker's record-shape
demand + norn fills its derived id + ACP fills minted/loaded id + docs) —
ONE coordinated motion, battery scope = every crate touching AgentOutcome
(grep it across crates/ + sdks/ + conformance first). Reply owed to
Waffles: brief ack once built, on the same DM thread.
