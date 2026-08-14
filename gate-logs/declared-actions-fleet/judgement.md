# Fleet judgement — K8 declared actions (`manifold.facet.actions@v1`)

Judge: Fable, 2026-08-15. Tree judged: `declared-actions` at 9e1bcb8 (server
fan-in of L1 834cf24 and L2 65d56ef over base e58973d). Server gates on the
merged tree: composition 642 claims all green (EXIT=0), dist gate EXIT=0
(141 served files byte-identical, none tracked), loc gate EXIT=0 (nothing
new over 500). Gates were read, not re-run — their exit codes are the
server's. VERDICT: **PASS** on both duties; nothing to flag.

## Duty 1 — permitted-choice points (stated-not-done / judgment calls)

Every judgment call each report claims was checked three ways: permitted by
the brief, actually stated in the report, and the better call on the tree.

### L1 (`schema-red`)

1. **`/declared-action.js` imported inside `page.evaluate` per claim** —
   mandated by the brief's red-first mechanic, stated, and done: the shared
   `draw` helper (cases file line 62) and the standalone claims 2 and 4 all
   import inside the evaluate. The red.log shows exactly the six intended
   claims failing on `Failed to fetch dynamically imported module:
   …/declared-action.js` with every pre-existing claim green — red by
   construction, as designed.
2. **Parse-not-byte-compare on sent bodies** — mandated by C3, stated, done
   (`JSON.parse` of captured bodies, member-set + value assertions at cases
   lines 180–208).
3. **Boolean for bar availability across the browser boundary** — a genuine
   in-floor call (a renderer function cannot serialize out of
   `page.evaluate`); stated; the better call — the claim needs only
   null/not-null, and the `!== null` probe (line 229) pins exactly that.
4. **Fresh snapshots / fresh draws per measurement** — stated; matches the
   brief's re-declaration precedent and keeps presses independent.
5. **Verbatim-travel claim passing in the red tree** — disclosed; the brief
   explicitly anticipated and permitted this ("the red is the suite, not
   every claim"). 6 of 7 new claims red, suite exit 1 — consistent.
6. **Floor claims only (7), no extras** — the brief's floor is "add more if
   you see gaps; never fewer"; all seven floors delivered with their
   sub-assertions intact (pending state, whole-bar disable, disclosure
   sentence, reserved-`re` refusal, scalar non-leak, nothing-sent on
   disabled press). Permitted.

### L2 (`derive-build`)

1. **Refusal wording** — the brief leaves wording free inside the pinned
   register/substrings. Verified in `declared-action.ts`: unknown primitive
   names the found primitive and says the v1 vocabulary is `"respond"` and
   `"choose"` "and grows only with a page release" (lines 65–70); the
   no-write-path span carries both pinned substrings `no write path` and
   `Nothing was sent.` (line 238); choose-over-non-list names the field and
   says "not a list" (lines 206–210) without leaking the value; the
   disclosure sentence names component, mounter (`@by`), the faults by
   name, and "everything else stands untouched" (lines 280–283). All stated
   in the report, all inside the register.
2. **Durable-name timing (press-time refusal for null `entry.record`)** —
   the brief lists null-record under both render-time and press-time
   refusals; the pinned contract (C3) says "when it is null the press
   REFUSES in words … nothing is sent," and L2 chose press-time
   (`wirePress`, lines 171–177), disclosed it, and never calls the write
   path on a null record. The better call: it matches C3's letter, keeps
   controls honest until pressed, and no L1 case pins the other reading
   (all case entries carry `record: 4`) — no conflict on the merged tree.
3. **No-face ruled edge** — honored at the seam (`mount.ts` line 357:
   derived null ⇒ null, no bar), noted in the module doc comment (lines
   8–9) and the report, `feed-view.ts` untouched. As ruled.
4. **Pending state as floor** — pressed button `data-pending="true"`, whole
   bar disabled (`settlePress`), no persistent ledger, poll's echo-settle
   machinery read but not extended. Stated; matches the floor exactly.
5. **Choice labels via `words()`** — strings verbatim, other JSON values
   readable-converted, body always carries the choice verbatim (line 219).
   Stated; consistent with the declared-face posture.
6. **`options.place!` non-null assertion** — stated. Verified: it sits only
   inside the branch that has established `options.place !== undefined`
   (mount.ts lines 334–339) and exists solely because TypeScript cannot
   narrow a property across the callback while preserving the
   planner-pinned seam shape. It is a language-level narrowing, not a lint
   bypass; the minimal deviation from the pinned snippet. Accepted.
7. **Seam restructure of the `faces` lambda** — the one-liner
   `carried ?? derived` became an explicit block to interleave the bar.
   Checked against the type: `carriedFaces` is `KindFaces` =
   `(token) => KindFace | null` (kind-render.ts:97), so the `!== null`
   check is exhaustive; carried answers return UNTOUCHED; the bar is
   appended to the derived face's wrap from the same `(payload, entry)`
   the face received. C6 satisfied precisely; carried-wins preserved.

Cross-leg contract meet: exports match C4 name-for-name; the fixture
matches C7 field-for-field (and L1's claim 2 pins fixture-to-cases through
`readRecordDeclaration` from disk); bodies match C3 (member set + values,
`re` from `entry.record`); DOM matches C5 (classes, data attributes,
substrings); token derivation intersects the facet's keys with
`claimsToken` (declaredActionBars line 254, actionTokens for disclosure).
L1's report footer (642 claims, 6 red) and L2's (635 claims, all green)
are mutually consistent: 635 + 7 new = 642, one of which (verbatim travel)
was green pre-merge. The merged tree's 642-all-green is exactly L1's red
turned green by L2's module.

## Duty 2 — hard-wall audit

Audited against the full diff `e58973d..HEAD` (8 files, matching the
plan's zone table exactly) and each leg commit's file list.

- ⛔ **Live estate** — no `.manifold*` path, no port 4176/41100/41101/
  41102/9490s config anywhere in the diff or either leg's actions.
  UNTOUCHED. (The untracked root `gate-logs/declared-actions/` is the
  server's post-merge gate output — dispatcher-desk artifact, not a leg's
  write.)
- ⛔ **No `crates/` changes** — none in the diff; both reports affirm.
  HELD.
- ⛔ **No `vendor/`, import-map, new served files, dynamic `import()` in
  page source** — no vendor or import-map paths in the diff; the dist gate
  proved served files regenerate byte-identically from committed sources
  with none tracked; `grep import(` over `declared-action.ts` and
  `mount.ts` is clean (the mount imports `./declared-action.js`
  statically). The dynamic imports in the cases file live inside
  `page.evaluate` in a test script — the brief-mandated red-first
  mechanic, not page source, following the existing declared-face
  precedent in the same scripts directory. HELD.
- ⛔ **`poll-face.ts` + `poll.ts` untouched** — absent from the diff;
  `VotePlacement` is imported `import type` (declared-action.ts:13), the
  expressly permitted type-only import. HELD.
- ⛔ **No lint bypasses, no test skips** — diff grep for
  eslint-disable/ts-ignore/ts-expect-error/.skip/.only/nolint is clean.
  The non-null assertion is narrowing, not a bypass (see Duty 1, L2 #6).
  HELD.
- ⛔ **Existing pinned cases byte-untouched** — the only touched existing
  test file is `composition-test.mjs`, and its diff is 9 added lines (one
  import beside the declared-face import, one registration block beside
  the declared-face block) with zero modified or deleted lines. Every
  pre-existing claim was green in L1's red run and on the merged tree.
  HELD.
- ⛔ **Worktree only; never land, merge, push, or deploy** — each leg made
  exactly one commit on its own branch containing only its zone
  (834cf24: L1's five files; 65d56ef: L2's three); the fan-in merges
  32af936/9e1bcb8 are the server's, per the run design. No push, no
  deploy, no landing by either leg. HELD.

Nothing to flag on either duty. The dev-estate browser walk and landing
remain with Waffles' re-gate desk, as planned.
