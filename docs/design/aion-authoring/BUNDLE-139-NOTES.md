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

## Cross-references

- Live assistant is 67128ff6 (current document), upgraded from d9774285
  on 2026-08-04; rollback via `aion route`.
- The gitignored `examples/assistant/assistant.aion` archive is STALE
  (249e7489, pre-dates the Jul 30–31 document edits) — do not resurrect.
- setup.sh lessons that must carry into any verb: owner-only home before
  the path gate fires; readiness = `/build` BODY, never a status code;
  addresses derived from the startup banner, never assumed; worker
  identity is per-home/per-address, not per-box.
