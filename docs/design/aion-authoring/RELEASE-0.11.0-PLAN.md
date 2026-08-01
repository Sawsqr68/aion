# RELEASE 0.11.0 — plan of record (task #138)

**Authority: Tom's GO, 06:35Z 2026-08-01** ("kind of released definitely …
you can release it and it's all good"), in response to the 0.10.0/main gap
the #133 docs lane proved by running the published binary. Purpose: Benji's
binary matches Benji's docs.

## Published-state facts (verified)

- Workspace version on main: **0.10.0** (root Cargo.toml). Published crate:
  0.10.0 — proven missing: declared `run` bodies, `agent` marker, `send`,
  `query`/`answer`, `aion awl scaffold`, `aion worker agent`,
  `aion run --timeout`. beamr pin: **0.16.2**.
- CHANGELOG Unreleased is current through 54ae9a43 (docs lane caught it up).

## Sequence (strict order)

1. **Final tree forms**: #131/#134 console-debt lane lands (in flight —
   clears the whole check baseline by refactor + run-mode button fix +
   pin). #131's release-cut backstop is thereby PAID before the cut.
2. **Version cut** (seat's edit): workspace 0.10.0 → 0.11.0 + any
   inter-crate version refs; CHANGELOG `## Unreleased` → `## 0.11.0` dated;
   RELEASE-0.11.0 notes if the house pattern wants them. Commit + push.
3. **Battery at the FINAL tree, on Dean's box** (attestation law: one
   battery at the final release tree; Athena's cycle arithmetic refused the
   local loop). Tom pastes the blocks (his preferred division), numbers come
   back, I verify remote HEAD over SSH + scp logs to gate-logs/. Battery =
   `cargo test --workspace --no-fail-fast` + `cargo clippy --workspace
   --all-targets -- -D warnings` + `cargo fmt --check` (fmt/clippy skew on
   Dean's newer rustc = forward-compat signal; check locally before
   reacting, don't chase phantom findings). Supersedes/absorbs the staged
   DEANS-BOX-135-BLOCK.md (re-issue blocks at the final tree; battery
   re-runs on new tree — law).
4. **Console gates local** (cheap, no cargo): bun run check absolute-green
   expected post-#131, bun test, bun run build.
5. **Publish, dependency order** — 21 crates (derived from cargo metadata,
   normal deps, topo order; NOT the remembered "19"):
   aion-core → aion-package → aion-awl → aion-awl-lsp → aion-awl-package →
   aion-proto-generated → aion-proto → aion-store → aion-rs → aion-client →
   aion-integrations → aion-integration-acp → aion-integration-norn →
   aion-darwin-acl → aion-store-haematite → aion-store-libsql →
   aion-toolchain → aion-worker → aion-server → aion-cli →
   aion-integration-cli.
   PRICED (Athena 06:46Z): verify SHARES the workspace target (measured:
   aion-core dry-run +0.104 GiB, one dep compiled). Ceiling 1 GiB/hard 1.5
   PER CRATE, anchors both nouns per crate, stop-on-anomaly vs the 0.104
   leaf actual; expect the first high-family crate (verify resolving
   published aion deps from crates.io, not path) to be the shape-breaker if
   any. Authority = TOM'S GO 06:35Z (Athena withdrew her per-crate-word
   gate herself, 06:46Z). --no-verify off the table. Publish only after the
   Dean's-box battery is green at the release tree.
6. **Post-publish verify**: install 0.11.0 and re-run the docs lane's
   probe corpus (the fixtures that failed on 0.10.0 must now check clean).
   Venue for the install build: price with Athena (or Dean's box).
   6b. **Strike the release-boundary notes** (docs/AWL.md:27-30,
   docs/GETTING-STARTED.md:51-58, AWL-REFERENCE.md:10-12) — true until
   publish, false after. Docs-only commit; release notes state the battery
   tree, which this commit does not move.
7. **Assistant worker rebuild** (examples/agent-dev worker embeds the 5
   teaching files via include_str!): rebuild so the LIVE assistant serves
   the new content. Local cargo build — priced with Athena.
8. **Tom's server** (#108): offer the clean swap to the new binary at his
   word — his live server (pid 99844, NEVER killed by anyone) still runs
   the deleted binary; the deployed read-only view and everything since
   only shows on his screen after HIS restart.

## Fences and laws in force

- Per-lane disk ceiling ≤2 GiB / hard stop 3 vs own fire anchor on ANY
  local cargo work; watchdog 30; anchors both nouns at fire instant.
- Battery re-runs on new tree; summarized exits never citable; count
  verdict lines.
- Release docs claim nothing unmeasured: the 0.11.0 notes state what the
  battery ran and where.
