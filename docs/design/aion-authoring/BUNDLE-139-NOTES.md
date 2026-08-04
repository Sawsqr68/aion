# #139 — the assistant inside the aion distribution (working notes)

Tom's target (2026-08-04, his words): "the experience for installing for
somebody else where you can't help." End state: `cargo install aion-cli`
and one verb gives a running server, the assistant deployed, and the
assistant answering — no repository clone, no Rust build of an example
crate. `scripts/setup.sh` (4c2c11ee, proven) is the interim answer and the
measuring stick: everything it does must become the product's own motion.

## What blocks repo-free today

1. **The document**: `examples/assistant/awl/assistant.awl` lives only in
   the repo. The published binary carries no assistant.
2. **The worker**: `examples/agent-dev/worker` is a standalone crate. Its
   surface is two actions on queue `default`:
   - `assistant_provision` — clones `repo_path` into
     `<aion-home>/clones/{workflow_id}/repo` and returns the opening
     prompt built from the embedded teaching resources (include_str! of
     `examples/assistant/resources/*`).
   - `assistant` — one norn round in the clone (session pinned to
     `{workflow_id}-{activity_type}`, `--resume-if-exists`).
   `aion worker agent <awl>` (shipped in 0.11.0) serves agent actions from
   a document — but `assistant_provision` is not an agent action, so the
   crate cannot simply be replaced by the verb yet.

## Proposed shape (MVP order)

1. **Provision becomes document-declared.** #60 shipped server-run command
   bodies: AWL declares the command, the server runs it, no worker serves
   it ("an action WITH a body is run by the server from the deployed
   contract"). Rewrite the provision step as a declared body (git clone +
   prompt assembly). The teaching resources then must ride the package
   (#139's original framing; #91 already made archives carry authored
   AWL) or be emitted by the declared body from packaged resource files.
2. **The agent action is served by `aion worker agent`** — document-
   declared queue, `--harness norn`, no crate. The session-pin argv the
   crate builds (`norn_invocation.rs`) must be reproduced by the built-in
   harness path; verify with `--check` before trusting.
3. **The document ships in the binary.** `aion new` template or an
   embedded document + a verb (`aion assistant up` or an extension of the
   setup story) that: prepares the home, starts/finds the server, deploys
   the embedded document, starts `aion worker agent` for it. #27
   (server-supervised workers) is the endgame — the server starts the
   worker itself and the verb collapses further.
4. **Prereq floor**: norn stays a prerequisite until the ACP harness or a
   bundled agent changes that; say so plainly in the verb's output.

## Scoped build plan (scout ratified 2026-08-04, seat's decisions)

The scout's decisive findings, verified with cites:

- **A declared body cannot return `Workspace{path, opening_prompt}`** —
  bodied-action results are capped at a subset of `{exit_code: Int,
  stdout: Str, stderr: Str}` (checker/bodies.rs:24-28). So provision
  SPLITS: prompt composition moves into the document (consts +
  triple-quoted raw strings + `+` concat — proven by
  ergonomics/valid/flow_vocab_b1.awl), and the body only creates the
  workspace.
- **Declared bodies run with cwd inherited, env = PATH ONLY** (no HOME,
  no AION_*; shell/action.rs:161-181) → the workflow gains a
  `workspace_root` input; the workspace path is composed in AWL by
  string concat.
- **Resources cannot ride the package today** (no files/ archive family
  — unknown prefixes are refused or silently dropped on rewrite,
  package.rs:364-442; nothing materializes packages on disk). MVP: the
  prompt points at `examples/assistant/resources/` inside the clone
  (attached-repo case) or at the public clone command (scratch case).
  No curl. The files/ family is the proper later build.
- **`aion worker agent --harness norn` builds argv `--protocol jsonrpc`
  and NOTHING else** — no session pin, no account, no workspace-root
  confinement (compose.rs:46-54). Ten missing elements vs the crate's
  norn_invocation.rs.

**Leg A — the documents + body (no Rust):** assistant.awl and
assistant_spike.awl (shared queue `default` — bodies must match
byte-for-byte or dispatch wedges terminally, declared_body.rs:242-247):
provision becomes `run "sh -c '<script>' -- $workspace_root $run_id
$repo_path"` with rename-aside collision discipline reproduced IN the
script (retry-wedge risk: clone-into-existing fails and bodies retry by
default); prompt ported from opening_prompt.rs into consts; the
two-branch workspace/ground-truth lines resolved either by when/otherwise
binding or by the body echoing the branch fragment on stdout —
`aion awl check` is the oracle, checker binding rules decide the shape.
State the posture change plainly: provision now runs as the server's
user, no sandbox (DECLARED-COMMANDS contract).

**Leg B — the built-in norn harness reaches parity (Rust):**
`--norn-arg` repeatable pass-through (allow_hyphen_values, mirror
--acp-arg exactly, settings.rs:149-159) threaded into build_norn;
`--norn-unset-env` repeatable (generic, operator-controlled — covers the
OPENAI_API_KEY force-OAuth case without baking policy in); promote
AdvisingHarness's degraded-session advice out of the example into
aion-integration-norn so the built-in path gives the same route-out.
Parity proven by a test asserting the COMPOSED argv (--check does not
show argv; the --account lesson, norn_invocation.rs:6-8).

**Leg C (after A+B):** embed the document in the binary + the verb;
#27 endgame unchanged.

## 2026-08-04 late: Tom's two rulings reshape leg A

Mid-demo, Tom hit the start refusal (`max_rounds`/`max_nudges` required)
and ruled, verbatim force: hand-supplying those caps is "ridiculous", and
**"there should be no max rounds"**. Actions taken at once:

- **Hotfix deployed + routed**: `assistant` version `6f43c809` — both cap
  inputs REMOVED from the workflow; the loops carry `max 1000000` (AWL
  demands a literal bound on every loop; this is the formality set beyond
  reach, not a budget). Start input is now `objective` + `repo_path` only.
  Verified live: empty-input probe names exactly those two fields.
  `examples/assistant/awl/assistant.awl` in-tree IS this deployed version.
  Old `67128ff6` stays resident; rollback is `aion route`.
- **Leg A is REWORKED, not landed.** The bodied-provision drafts added a
  `workspace_root` input (forced by the PATH-only body environment) — a
  third required start field, which the same ruling forbids. The drafts
  are parked byte-for-byte in `139-drafts/` beside this file. Before leg A
  can land, the workspace location must come from somewhere that is not
  the operator's start input. Candidates, undecided (discuss values before
  implementing): the body's inherited cwd (bodies inherit the server's
  cwd), or a server-side setting the body can see. The coupling to the
  worker's `--norn-arg --workspace-root` argv must move with it — the
  provision body and the worker argv must agree on one path.
- The example crate's `descriptors_tests.rs` fixes (contract-derived
  assertions) stay — they hold for any document shape.

Known risks carried: cancellation of declared bodies is held-not-wired
(a big clone won't stop); the live route cut needs `aion route` planning
(old 67128 stays resident); both documents change identically.

## Cross-references

- Live assistant is 67128ff6 (current document), upgraded from d9774285
  on 2026-08-04; rollback via `aion route`.
- The gitignored `examples/assistant/assistant.aion` archive is STALE
  (249e7489, pre-dates the Jul 30–31 document edits) — do not resurrect.
- setup.sh lessons that must carry into any verb: owner-only home before
  the path gate fires; readiness = `/build` BODY, never a status code;
  addresses derived from the startup banner, never assumed; worker
  identity is per-home/per-address, not per-box.

## 2026-08-05: review verdicts and open questions

Recorded from the leg-A Opus review and the seat's rulings (fixes brief
`BRIEF-139-LEG-A-FIXES.md`); each is a decision NOT taken in code:

1. **Provision has exactly one attempt.** No `retry` clause is declared, so a
   non-zero exit fails the dispatch and reaches the operator. Whether the
   action SHOULD declare a retry policy is an undecided value — the old
   worker-side handler classified these failures terminal, so one-attempt is
   behaviour-preserving, not a regression.
2. **AWL cannot mark a run-body failure terminal vs retryable.** The language
   has no construct for it; the executor's default classification decides.
   Language gap, recorded — not worked around.
3. **Superseded workspace directories accumulate unboundedly by design**
   ("renamed aside — never deleted"). A retention/reaping policy is an
   undecided value, not an omission to patch silently.
4. **The root may be an operator-planted symlink and is followed.** That is
   the operator's own configuration; the banner reports the configured path,
   and no symlink refusal is imposed.
5. **`state.rs` and `run.rs` exceed the 500-line law pre-existingly.**
   Standing debt, not grown materially by this lane (the lane added a
   handful of comment lines and moved banner logic OUT of `run.rs`).
6. **`mv` collision on a same-second-same-pid superseded name** is
   effectively unreachable (one process cannot mint two identical
   `<stamp>-<pid>` suffixes for one run id in one second through this path);
   if forced, `mv` nests the directory rather than failing. Recorded.
