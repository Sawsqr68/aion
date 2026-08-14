# Fleet plan — K8 declared actions (`manifold.facet.actions@v1`)

Planner: Fable, 2026-08-15. Base tree: `declared-actions` at e58973d (clean).
Topology is FIXED (FAN-OUT r1.1): two parallel legs, isolated worktrees,
disjoint file zones, merged mechanically. Green exists only on the merged
tree; L1's red is red BY CONSTRUCTION of the fan.

## What this run builds

The actions half of the component declaration, the same way K6 built the view
half: capability declarations travel as record entries on the registry feed
(never code), read against a closed v1 vocabulary of exactly two primitives
(`respond`, `choose`), rendered as controls on the rendered kind, routed
through the page's EXISTING write path, refusing malformed declarations BY
NAME. K6's proven precedent is the pattern for every design decision below:

- Reader + refusal register: `surface/page/src/declared-face.ts:20` (facet
  constant), `:59-85` (per-element deny-unknown reader, refusal wording),
  `:88-115` (refuse-whole reader), `:188-202` (live resolver over the mounted
  snapshot), `:215-235` (refusals disclosed for a surface to consume).
- Composition seam: `surface/page/src/composition/conversation/mount.ts:332`
  (derived resolver built from `options.mounted`), `:344-346` (carried-first
  composition into the feed's `faces` port).
- Write path & refusal-in-words precedent:
  `surface/page/src/composition/conversation/mount.ts:166-178` (the `place`
  port — the room's ONE write path, handed in), `:317-330` (the poll vote
  no-write-path refusal, in words, where the reader pressed),
  `surface/page/src/poll-face.ts:67-79` (`VotePlacement` — refused/placed),
  `surface/page/src/poll.ts:252-259` (`voteBody` — the sent body is
  `JSON.stringify({shape: <kind-token>, ...members})`).
- The durable entry reference: `surface/page/src/kind-render.ts:50-63`
  (`KindEntry.record` — the conversation's durable sequence, or null; a face
  given null must say so rather than reach for frame numbering).
- Red-first test mechanics: `surface/page/scripts/composition-test.mjs:557-558`
  (per-case-file dynamic import cluster), `:2323-2329` (registration block),
  `:2375-2379` (modules loaded INSIDE claims so a red tree fails the claims,
  not the suite), `surface/page/scripts/composition-declared-face-cases.mjs`
  (whole file — the case-file shape L1 mirrors),
  `surface/page/records/notice-declaration.json` (the fixture shape).
- Gates: `surface/page/package.json:7-12` (typecheck / build /
  test:composition / gate:dist), `scripts/loc-gate.sh:21` (LINE_LIMIT=500
  lines of code per file, not bumped).

Both `options.mounted` and `options.place` are ALREADY wired into the
conversation mount from the page (`surface/page/src/composition/main.ts:987`
and `:995`), so this run needs NO change to `main.ts`, no new port, and zero
new wire surface. If either leg finds otherwise, that is a STOP-REPORT, not
an edit.

## File zones (disjoint; verified against the tree)

| File | Owner |
|---|---|
| `surface/page/scripts/composition-declared-action-cases.mjs` (new) | L1 |
| `surface/page/scripts/composition-test.mjs` (registration lines only) | L1 |
| `surface/page/records/rsvp-declaration.json` (new) | L1 |
| `surface/page/gate-logs/declared-actions/red.log` (new) | L1 |
| `.fleet/reports/L1.md` | L1 |
| `surface/page/src/declared-action.ts` (new) | L2 |
| `surface/page/src/composition/conversation/mount.ts` (seam edit only) | L2 |
| `.fleet/reports/L2.md` | L2 |

No file is shared. The one file the design COULD have pushed both legs toward
is `composition-test.mjs` (L1 registers cases there; L2 might be tempted to
run/edit the suite) — it is assigned to L1 alone; L2 does not touch any file
under `surface/page/scripts/`. Conversely L1 writes nothing under
`surface/page/src/`. `main.ts`, `feed-view.ts`, `kind-render.ts`,
`poll-face.ts`, `poll.ts`, `declared-face.ts` are owned by NOBODY and edited
by nobody.

## THE PINNED CONTRACT (the whole reason this plan exists)

The legs never see each other's work. L1's red cases and L2's module meet
only on the merged tree, so every observable below is pinned here and
restated verbatim in both briefs. Neither leg may "improve" a pinned name.

### C1. The facet

- Name: `manifold.facet.actions@v1`. Body: `{ "actions": { "<kind-token>":
  [ <action>... ] } }`. It rides the component declaration verbatim as an
  unknown facet (`surface/page/src/record-declaration.ts:134-198` already
  passes unknown facets through; `facetNamed` at `:190`, `claimsToken` at
  `:216`). NO core (`crates/`) change; NO `record-declaration.ts` change.

### C2. The two v1 primitives (closed; deny-unknown at every level)

- `{ "primitive": "respond", "label": "<literal>", "kind":
  "<record-kind-token>", "body": { ...literal template... } }` — exactly
  these four members. `label` and `kind` non-empty strings; `body` an object.
  A template member named `shape` or `re` is MALFORMED (it would collide with
  the sent envelope below) and refuses by name.
- `{ "primitive": "choose", "from": "<list-field>", "kind":
  "<record-kind-token>" }` — exactly these three members; `from` and `kind`
  non-empty strings.
- Any other `primitive` value, any unexpected member, any missing member, a
  non-object facet body, a non-object `actions`, a non-array roster, or a
  non-object action refuses BY NAME in the declared-face wording register
  (`declared-face.ts:62-84` is the style: name what was found, name what is
  permitted, say the v1 vocabulary is `"respond"` and `"choose"` and grows
  only with a page release).
- A malformed roster refuses that token's actions WHOLE (mirror of the face's
  refuse-whole, `declared-face.ts:108-113`) and is disclosed via
  `declaredActionRefusals` (C4).
- A `choose` whose `from` field on the ACTED-ON payload is not an array is a
  RENDER-time refusal of THAT ACTION ONLY: a `declared-action-refusal`
  element whose words name the field and say it is not a list; sibling
  actions still render (mirror `declared-face.ts:171-179`).

### C3. The sent entry (press IS the act)

A press hands ONE body string to the room's existing write path and does
nothing else — no composer detour, no second door, no new wire surface.
Body construction (precedent `poll.ts:252-259`):

- respond: `JSON.stringify({ shape: <action.kind>, ...<action.body members>,
  re: <entry.record> })`
- choose, pressed item at index i: `JSON.stringify({ shape: <action.kind>,
  re: <entry.record>, option: i, choice: <the item, verbatim> })`

Member SET and VALUES are the contract; serialization order is not — tests
parse the captured body, never byte-compare it. `entry.record` is
`KindEntry.record` (`kind-render.ts:50-63`); when it is null the press
REFUSES in words (the entry holds no durable name; nothing is sent).

### C4. The module (L2 builds it; L1 imports it by these exact names)

File `surface/page/src/declared-action.ts`, compiled by the existing build to
`dist/declared-action.js` and therefore importable in composition cases as
`/declared-action.js` (exactly as `/declared-face.js` is at
`composition-declared-face-cases.mjs:33`). Exports, exactly:

```ts
export const ACTIONS_FACET = "manifold.facet.actions@v1";

export function readDeclaredActions(
  declaration: RecordDeclaration,
  token: string,
):
  | { kind: "actions"; actions: readonly DeclaredAction[] }  // as declared
  | { kind: "absent" }
  | { kind: "refused"; reason: string };

export function declaredActionBars(
  mounted: () => MountedSnapshot,
  act: ((body: string) => VotePlacement) | null,   // null = no write path
): (token: string) =>
  | ((payload: Readonly<Record<string, unknown>>, entry: KindEntry) => HTMLElement)
  | null;                                           // null = no actions for token

export function declaredActionRefusals(snapshot: MountedSnapshot): readonly string[];
```

`DeclaredAction` is the union of the two C2 shapes (exporting the type is
optional; the returned objects carry exactly the declared members).
`VotePlacement` is imported AS A TYPE from `./poll-face.js` (poll-face.ts:67
— an import touches nothing). `declaredActionBars` resolves against the LIVE
snapshot on every token ask, exactly as `declaredFaces` does
(`declared-face.ts:188-202`), so mount and re-declaration are visible on the
next render. A refused roster answers null from the bar resolver (nothing
renders) and appears in `declaredActionRefusals`, whose sentences mirror
`declaredFaceRefusals` (`declared-face.ts:215-235`): component, mounter, the
faults by name, and that everything else stands untouched. NOTE it derives
the token list from the ACTIONS facet's own keys intersected with
`claimsToken` — a declaration cannot act on a kind it does not render.

### C5. The rendered controls (DOM contract)

The bar render returns ONE element `span.declared-action-bar`. Inside it:

- per respond action: `button.declared-action-respond`, textContent = the
  declared `label`, `data-action-kind` = the declared `kind`.
- per choose action: one `button.declared-action-choice` PER ITEM of the
  payload's `from` list, textContent = the item, `data-option` = its index,
  `data-action-kind` = the declared `kind`.
- action-level render refusals (C2 last bullet; C3 null-record): an element
  `span.declared-action-refusal` carrying the words.
- `act === null` (no write path): every button in the bar rendered
  `disabled`, plus ONE `span.declared-action-refusal` whose words include
  `no write path` and `Nothing was sent.` (register of
  `conversation/mount.ts:322-327`).
- a PLACED press: the pressed button gets `data-pending="true"` and the
  bar's buttons disable until the next redraw (floor; poll's echo-settle
  machinery is ceiling and must not touch poll files). A REFUSED press
  renders the refusal words in a `span.declared-action-refusal` where the
  reader pressed.

Tests assert classes, data attributes, text content and refusal SUBSTRINGS —
never whole-element byte equality — so wording may breathe inside the pinned
register.

### C6. The mount seam (L2) — carried-first, zero new ports

In `conversation/mount.ts`, beside the existing line 332 `derivedFaces`:

- build `const actionBars = options.mounted === undefined ? () => null :
  declaredActionBars(options.mounted, options.place === undefined ? null :
  (body) => options.place(body))` — reusing the EXISTING `place` port
  (`mount.ts:166-178`); when the room has no write path the bar module
  renders disabled-with-words (C5), the exact posture of the poll vote
  refusal at `mount.ts:317-330`.
- compose into the feed's `faces` port (today `mount.ts:346`):
  CARRIED-WINS — if `carriedFaces(token)` answers, return it UNTOUCHED (no
  bar; this is what keeps poll's hand-wired path sovereign for its tokens
  even if someone declares actions over them). Else if `derivedFaces(token)`
  answers AND `actionBars(token)` answers, return a face that appends the
  bar element to the derived face's returned wrap. Else return the derived
  face or null as today.
- RULED EDGE (both legs bound by it): a token with declared actions but NO
  resolvable face renders NO bar in v1. L1 must not write a case expecting
  one; L2 must not reach into `feed-view.ts` to force one. (The rsvp fixture
  declares a face, so the walk is unaffected; the edge is noted in reports.)

### C7. The fixture kind (L1 authors it; the dev walk mounts it)

`surface/page/records/rsvp-declaration.json` — component
`ablative.component.rsvp@v1`, in the exact shape of
`records/notice-declaration.json`, declaring for kind
`ablative.message.rsvp@v1` (fields `prompt` text required, `options`
text-list required min 1):

- `manifold.facet.renders@v1`: types + presentation (title from `prompt`,
  field `options`).
- `manifold.facet.accepts@v1`: the two fields, shape, label `rsvp`.
- `manifold.facet.face@v1`: layout `heading` from `prompt`, `list` from
  `options`.
- `manifold.facet.actions@v1`:
  ```json
  { "actions": { "ablative.message.rsvp@v1": [
    { "primitive": "respond", "label": "Attending",
      "kind": "ablative.message.rsvp.reply@v1",
      "body": { "answer": "attending" } },
    { "primitive": "choose", "from": "options",
      "kind": "ablative.message.rsvp.choice@v1" }
  ] } }
  ```

L1's inline case declarations carry this same actions roster, and one claim
reads the fixture file itself (the `readFile(new URL("../records/…"))`
pattern of `composition-authoring-cases.mjs:340`) through
`readRecordDeclaration` to pin fixture and cases together.

## Leg L1 — `schema-red` (brief: .fleet/legs/L1.md)

New cases file + registration + fixture + the failing run's log, committed
RED with the evidence. Floor cases: facet travels verbatim through the
declaration reader; derivation yields the declared roster; control shape
(respond button + per-item choose buttons, presses captured through a stub
`act` and parsed against C3); refusal-by-name (unknown primitive refuses the
roster whole, disclosed by name, bar answers null); choose-over-non-list
refuses that action and renders the rest; no-write-path renders disabled
with the words. Red evidence: `npm run build` (passes — the leg adds no
TypeScript) then `npm run test:composition` teed to
`surface/page/gate-logs/declared-actions/red.log`, exiting nonzero with the
new claims failing on the missing `/declared-action.js`. Commit message
register: `test(page): pin declared actions red` (precedent eef6428).

## Leg L2 — `derive-build` (brief: .fleet/legs/L2.md)

`declared-action.ts` (≤500 LOC of code, house style of `declared-face.ts`,
exports per C4, vocabulary per C2, bodies per C3, DOM per C5) plus the C6
mount seam. Gated in-leg by `npm run typecheck`, `npm run build`,
`npm run test:composition` (existing suite stays green — the leg never sees
L1's cases), `npm run gate:dist`, and the loc gate. Commit register:
`feat(page): derive declared actions from declarations` (precedent e58973d).

## After the fan (dispatcher's desk, not the legs')

Mechanical merge of both branches (zones are disjoint; a textual conflict is
a fan defect — STOP). On the merged tree the gates prove L1's red turned
green. The dev-estate browser walk (mount `records/rsvp-declaration.json` on
a dev estate, see choose controls render, press one, watch the signed entry
land on the feed, see the refusal-in-words on a no-write-path room, confirm
the DECLARATION changed no dist bytes — the page's built bytes predate the
mount) belongs to Waffles' re-gate desk, where a composed page exists to
walk. Neither leg lands, merges, pushes, or deploys.

## Hard walls (verbatim in both briefs)

⛔ Live estate untouchable: ports 4176/41100/41101/41102/9490s, every
`.manifold*` path — dev estates only. ⛔ No `crates/` changes (STOP-REPORT).
⛔ No `vendor/` edits, no import-map changes, no new served files beyond what
`npm run build` writes from committed sources, no dynamic `import()` in page
source — code delivery is refused by settled design. ⛔ `poll-face.ts` +
`poll.ts` untouched (type-only imports FROM them are fine). ⛔ No lint
bypasses, no test skips. ⛔ Existing pinned cases byte-untouched — a pin
conflict is a STOP-REPORT, never a rewrite. ⛔ Worktree only; never land,
merge, push, or deploy — Waffles re-gates the final tree and lands.
