# #244 remediation — the tail that went quiet while curing quiet

Remediation of the FAIL verdict on branch `244-child-visibility` @ dd5073bf2
(OPUS-REVIEW.md, 2026-08-15, appended below in full — it is part of this
charter). Work continues ON THE SAME worktree and branch:
`.worktrees/244-child-visibility`, commits atop dd5073bf2. The #241
remediation precedent applies: the review's "What must happen before this
lands" list is the charter, every item compulsory, nothing deferrable —
including the two NOTE-severity items.

## The core defect (why the verdict is FAIL, not refused-with-minors)

`aion tail --follow` decodes a WebSocket frame shape the server has never
sent — the CLI expects `{activity_event, error}` while the server sends
`{"kind":"activity_event","event":{…}}` — and serde's ignore-unknown-fields
makes the mismatch SILENT: after backfill the terminal sits quiet forever
while legs stream. That is the exact defect class #244 exists to remove,
reintroduced in the verb built to cure it. It shipped because the brief's
required R3 e2e was never written: nothing in the diff ever connected
`aion tail` to a running server.

## Charter (OPUS-REVIEW.md "What must happen before this lands", verbatim
## obligations; finding numbers refer to that document)

1. **The R3 red comes FIRST, committed failing (F2).** Two layers:
   (a) a decoder test that feeds a genuine `StreamedActivityEvent` JSON
   frame — the real server shape, taken from the server's serializer, not
   hand-typed from memory — to the CLI decoder, committed failing against
   the current decoder; (b) an e2e that runs `aion tail --follow` against
   a live server with a parent-with-children fixture and asserts leg lines
   APPEAR on the CLI's output.

   **Amendment — red for the right reason (Waffles, ratified 05:01Z):**
   the defect kills only the LIVE path — backfill prints fine. An e2e
   whose fixture events all exist before `tail --follow` attaches would
   see its leg lines arrive via the working backfill and pass against
   the broken decoder: a husk test one layer up, the same class the
   review itself diagnosed. The committed red's asserted leg lines must
   originate from events emitted AFTER the tail is attached, so they can
   only travel the live socket path. Pin the sequencing in the fixture:
   attach, THEN drive the child legs, THEN assert. A backfill arm beside
   it as a control is welcome — but the RED is the post-attach arm.

   Both reds committed before any fix, with
   the failing output quoted in the commit message. This is the discipline
   whose absence shipped the defect; it is the landing's spine.
   (Note for the hunt: there is no F14 in the review — findings jump F13
   to F15. F19 is disclosed benign churn and carries no obligation.)
2. **F1/F1b** — decode the real transcript frame; unknown/undecodable
   frames are LOUD (named notice on the operator's terminal, never a
   silent discard); `transcript_lagged` is per-leg recoverable, not
   session-fatal.
3. **F3, F5, F7, F8** — every quiet path says why it is quiet: the legs
   section surfaces the transcript's own `socketError`/`backfillError`
   channels; not-yet-started legs are shown as such, never silently
   omitted; the three `let _ = sender.send(...)` swallows in the CLI live
   path are replaced with handled/propagated sends. No swallowed sends
   anywhere in the new surface.
4. **F4** — pin the component that actually ships (the test must exercise
   the shipping artifact, not a lookalike).
5. **F6** — the CLI notices fanout that happens after it starts:
   parent-socket fanout discovery matching the console's dd5073bf2
   mechanism.
6. **F9, F10, F11** — typed status rendering (no `Debug` formatting, no
   invented `Pending` state), API.md + CLI docs updated to the real
   contract, strict field validation at the decode boundary.
7. **F13** — every child history read gates through the same per-workflow
   authorization its sibling routes use, and the contract SETTLES what an
   unauthorized child looks like in a children listing (named decision,
   written down, not an accident of implementation).
8. **F15, F16** — the 2000-event backfill cap either announces its
   truncation to the operator or becomes a settled argument (house law:
   no invented constants — surface the value decision, don't bury it);
   the HTTP endpoint derives from `DEFAULT_HTTP_ADDRESS` and gains the
   same override story the other connection settings have.
9. **F17** — the transcript subscription moves to `aion-client` and the
   CLI consumes it there: ONE implementation of this wire contract, one
   place to test it. The review names this the cleanest fix for F1 —
   build it as the mechanism, not as an extra.
10. **F12** — settle the read-shape question (whole-history reads vs
    K9-scale histories) before this runs against production-sized runs;
    a written decision in the module doc counts, an unexamined
    `read_history` loop does not.
11. **F18** — `main.rs` does not grow: the `Tail` variant and its
    dispatch arm live beside `tail.rs`.

## Verification floor

- Both R3 reds demonstrably red first (committed), then green by the fix.
- Full workspace battery to a committed log file, exit codes recorded.
  THREE reds are PRE-EXISTING CONFIRMED on base 7109c1f6d and expected:
  sdk_closure_freshness, slim-libsql feature-matrix clippy, and
  aion-cli awl_corpus_gate. Any FOURTH red is this diff's and blocks.
- `cargo clippy --workspace --all-targets -- -D warnings` AND the
  `-p`-scoped run (both resolutions required), `cargo fmt --check`,
  console suite if console files are touched.
- Record box load beside any red.

## House laws

Worktree-only, commit-as-you-go, never push, red-first with quoted reds,
no lint bypasses, no `#[allow]`/`#[ignore]`/`let _ =` suppressions,
≤500 LOC per file, no invented defaults. Waffles' re-gate before landing;
his desk lands. The full OPUS-REVIEW.md follows below — read it whole
before the first edit; the finding numbers above resolve there.
