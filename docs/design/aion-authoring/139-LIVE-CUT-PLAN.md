# #139 leg C — the live cut plan

Status: written 2026-08-09 alongside the leg-C code (branch
`lane/assistant-embed`). **No live system was touched to write this.** Nothing
here has been run against the operator's server, home, or worker; every
statement about the live box is a prediction with its check named, not an
observation.

## 1. What leg C changed in the product

- The assistant document moved to `crates/aion-server/assistant-embed/assistant.awl`
  and is compiled into the binary with `include_str!`, exactly as the ops-console
  bundle is. There is now ONE copy in the repository; the server, the CLI verbs,
  the example worker, the AWL gate, and the ops-console contract test all read it.
- `aion server` installs it at boot — but only onto a catalog holding **no
  version of the assistant workflow type**. See §2.
- `GET /assistant` describes what the binary carries and where it stands in the
  catalog right now; `GET /assistant/document` serves the document's bytes.
- `aion assistant {document,describe,start,say,end,status}` is the operator verb.
  `document` and `describe` need no server.

## 2. The restart is safe, and that is a designed property

`Engine::load_package` re-points routing for a package's type on every load.
An unconditional boot install would therefore undo an operator's rollback on
every restart. The install claims only the empty case, so:

- **Restarting the live server changes no routing.** The assistant type already
  has resident versions there, so the install stands down and logs
  `operation=assistant.install outcome=deferred` naming `embedded_hash`,
  `routed_hash`, and the verbs that would perform the cut.
- If the embedded hash happens to equal the routed hash, the line reads
  `outcome=already_current` and nothing is loaded.
- On a fresh home (a new install, `cargo install aion-cli`), the line reads
  `outcome=installed` and the assistant is startable immediately.

**The check, before anything else:** restart nothing to learn this. Read the
new binary's own answer:

```
aion assistant describe            # embedded workflow_type + content_hash, no server needed
curl -s http://<http_address>/assistant | jq '.content_hash, .residency'
```

`residency.state` is a live read of the catalog: `routed`, `loaded_not_routed`,
or `not_loaded` with the routed hash named. Compare it with
`aion assistant describe`'s `content_hash`. If they match, the live document and
the embedded document are the same bytes and the cut below is a no-op.

## 3. 🔴 The ordering constraint: documents first, workers last

**Deploy and route the bodied documents BEFORE any worker rebuild or restart.**
This is a standing constraint, not a preference for this cut. Two independent
reasons, both terminal when violated:

1. **Retained contracts must agree on a declared body.** `assistant.awl` and
   `assistant_spike.awl` share task queue `default`, and a queue whose retained
   contracts declare DIFFERENT bodies for one action name refuses every dispatch
   of that action terminally (`worker/declared_body.rs`). A worker brought up
   against a new document while the server still routes the old one is a wedged
   queue, not a degraded one.
2. **The provision body and the worker argv must agree on one path.** The body
   materialises `<workspace_root>/<run_id>/repo`; the agent worker is started
   with `--norn-arg --workspace-root --norn-arg "<workspace_root>/{workflow_id}/repo"`.
   Restart the worker first and it can be confined to a directory the current
   routed document does not provision.

### The order

1. **Land the code.** Build the binary; do not restart anything yet.
2. **Read, do not assume.** `aion assistant describe` (binary) and
   `GET /assistant` (live server) — record both hashes.
3. **If the hashes match:** stop. There is no cut. Restart the server whenever
   convenient; the install will log `already_current`.
4. **If they differ, cut the DOCUMENT first**, from the binary itself — no
   checkout required:
   ```
   aion assistant document --output assistant.awl
   aion deploy assistant.awl          # loads AND routes it
   aion versions --workflow-type assistant
   ```
   `aion deploy` is the operator's deliberate act; the boot install deliberately
   cannot do this for them.
5. **Only then touch the worker.** Rebuild/restart the agent worker for queue
   `default` with the `--workspace-root` argv matching the newly routed body.
6. **Rollback** is `aion route assistant <previous_hash>` — the previous version
   stays resident, and a server restart will not move the route back.

### Sessions parked across the cut

A session parked on `assistant_continue` keeps executing on the version it
started with (content-hash module namespacing). The cut changes what a NEW
session starts on. Nothing needs to be drained.

## 4. The console bundle is FROZEN, and this lane does not need it

No ops-console bundle rebuild is required by leg C.

- `crates/aion-server/ops-console-embed/` is untouched.
- The only console-side edit is the path constant in
  `apps/aion-ops-console/src/features/assistant/__tests__/assistant.contract.test.ts`,
  a vitest contract test that reads the AWL document at TEST time. Test files are
  not part of the built bundle, so the committed bundle stays byte-identical and
  valid.
- If a future change does need the console to consume `/assistant`, that
  rebuild is a separate, last step, taken after the live cut — never bundled
  into it.

## 5. Residues, stated

- `crates/aion/tests/declared_contract_boundary.rs:40` still names the document's
  old path in a doc comment. It is prose, not a read: the test builds its own
  fixture schema. It was left alone deliberately — editing it would pull the
  whole `aion-rs` crate into this lane's gate surface for a comment. The next
  lane touching `aion-rs` should correct it to
  `crates/aion-server/assistant-embed/assistant.awl`.
- `examples/agent-dev/worker` is not a workspace member, so this lane's gates do
  not compile it. Its two references to the document were repointed and each
  relative path was verified to resolve on disk; the crate itself was not built
  here (a separate target directory is a second cold build, and disk headroom is
  the binding constraint on this box).
- The historical design documents under `docs/design/aion-authoring/` still name
  the old path where they are recording what was true at the time. Those are
  records and were left as written.
