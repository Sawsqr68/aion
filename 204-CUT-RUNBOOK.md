# Release-cut runbook: the worker bring-up chain (#197 + #200 + #204)

Ruled by Waffles the Terrible, 2026-08-11 10:57Z: the collector does not go
out on 0.13.8 — the deploy rides a fresh release cut carrying all three
landings together, and the #197 truth-in-verdict release-note entry travels in
the same artifact as the change it describes (struck when the #206 mitigation
lands). Tom hears the whole deploy shape from Waffles as one notice; the
assistant-worker restart happens on TOM'S WORD and at his timing.

Chain on `main`: #197 `bc2cf1d3a` · #200 merge `b5ad0bc47` · slice-one merge
`5f7579ed6`. All three CHANGELOG entries are in `Unreleased`.

## Ordering law

Queue moves and document deploys happen BEFORE any worker rebuild or restart.
A worker restarted onto the new binary with the old shape is refused; a
document deployed first means no window in which a default-queue worker is
refused (#200) or an old-flag record wedges a supervised restart (#204).

## Steps

1. **Publish the cut** (all workspace crates, one version — recommendation in
   the envelope; the ladder's attestation rules apply: registry-verify every
   crate, tag, changelog stamped from `Unreleased`).
2. **Install the new binary** on the box (`cargo install` path — do NOT
   restart the running server yet).
3. **Assistant queue move** (before any worker restart):
   `aion assistant document --output assistant.awl` →
   `aion deploy assistant.awl` (loads AND routes) → the routed assistant now
   declares its private queue. The old worker keeps serving old sessions;
   new dispatches go to the private queue.
4. **Re-write Builtin supervision records** that carry deleted launch flags
   (pid 8790's record is the known live instance — its argv carries
   `--harness`, `--acp-command`, `--acp-cwd`, `--acp-permission`,
   `--acp-exit-grace`, `--concurrency`, `--reconnect-*`, all deleted by
   #204). The settings move into the worker document's `harness` section;
   the record's argv shrinks to the document path and the flags that remain
   live on `aion worker agent`.
5. **Verification line (ruled):** after the re-write, a supervised restart of
   a TEST record SUCCEEDS — run one supervised restart against a re-written
   record and record its exit before touching any live worker. The runbook
   proves the migration; it does not assert it.
6. **Worker restarts — ON TOM'S WORD ONLY:**
   - the assistant worker (moves onto the private queue, new binary);
   - the ACP demo worker pid 8790 (new binary, document-declared harness);
   - any collector worker (#196) — the collector serves ONLY on a binary
     carrying #197; the installed 0.13.8 predates the fix, so the collector
     waits for this cut by ruling.
7. **Post-swap ladder:** `aion versions` non-empty from repo root, worker
   re-registration observed with the probation lines (#197's
   `SERVING ITS DISPATCH PROBATION` → eligibility), one live run Completed,
   changelog endpoint shows the new version.

## What this runbook does NOT do

- No worker restart happens on publish or install — steps 6 onward wait for
  Tom's word, delivered through Waffles' single notice.
- Nothing here edits `assistant.awl` content beyond what the embedded
  document already carries — an edit would change the package hash and needs
  Tom's or Waffles' word first.
