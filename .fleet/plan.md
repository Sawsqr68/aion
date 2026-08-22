# PLAN — policyref-l3-pin: record the pushed mirror in the Zed pin

Planning counsel, 2026-08-22. Tree: `fleet/policyref-l3-pin-base` @ `663f1957`.
Single leg, fixed at dispatch: **`l3-pin`**. No other split is proposed and none
is admissible.

This flight stands on the FULL spent tree of `policyref-l3-route`. That flight's
deliverable — routing policy refusals through authored fallbacks, including the
AWL `fallback` reserved word — is landed, committed, and judged real. Its gate
ended with exactly one red, and retiring that one red is this flight's whole
content.

Governing documents, in force in this order:

1. `.fleet/BRIEF.md` — complete and self-sufficient. It names the file, the two
   lines, both old values, and both new values.
2. `crates/aion-awl/tests/editor_assets.rs` — **the oracle**. The digest is
   whatever that test computes, never what a hand-rolled `sha256sum` pipeline
   computes. Where the brief and the test could ever disagree, the test wins and
   the leg reports the disagreement.
3. This plan — the precondition, the mechanism re-derived on this tree, the
   battery, and the walls.

The previous flight's plan is not superseded in substance; it is simply not this
flight's plan. It stays recoverable as `git show 663f1957:.fleet/plan.md`.

---

## 0. Precondition — the estate-external half, and how it is proved

The cure has two halves. The external half — pushing the published tree-sitter
mirror so the sha the pin will name actually exists anonymously — is **already
done at dispatcher hands** and must not be attempted, repeated, or "verified" by
any write. The leg's half is the two-line record.

The leg's FIRST act, before any edit:

```
git ls-remote https://github.com/tomWhiting/tree-sitter-awl.git refs/heads/main
```

MUST print exactly:

```
4ba65879406dbe9204138e24397917ba60d82aff	refs/heads/main
```

Counsel ran this read-only at plan time (2026-08-22) and observed exactly that
sha. The leg re-runs it anyway: the mirror is an external moving object and a
plan-time observation is not a build-time fact.

**If it prints anything else — a different sha, an error, a prompt — STOP.**
Commit no source change, write the observed bytes verbatim into
`.fleet/reports/l3-pin.md`, and end the leg. The precondition failed and the
dispatcher must re-examine. There is no fallback path, no "pin it anyway", no
digest-only edit.

## 1. The mechanism, re-derived on this tree

`editors/zed-awl/extension.toml` is 25 lines. Its `[grammars.awl]` table
(`:14`) carries three load-bearing values:

- `repository = "https://github.com/tomWhiting/tree-sitter-awl.git"` (`:23`)
- `rev = "8d4d63e398bf765dd51772b60555b9dae8ebc106"` (`:24`)
- `# grammar-sha256 = "eb61fb4297…"` (`:25`)

Zed does not build the grammar from this repository — it fetches that mirror at
that exact rev. The `# grammar-sha256` comment is what makes a stale pin
detectable offline; the header comment at `:15-22` says so in the file itself.

`crates/aion-awl/tests/editor_assets.rs`:

- `PUBLISHED_GRAMMAR` (`:199`) — the four files a mirror commit must carry:
  `tools/tree-sitter-awl/grammar.js`, `src/grammar.json`, `src/node-types.json`,
  `src/parser.c`.
- `published_grammar_digest()` (`:210`) — SHA-256 over, per file in that fixed
  order: the relative path bytes, a NUL, the length as 8 little-endian bytes,
  then the contents. Path and length are folded in so a rename or a shift of
  content between files cannot leave the digest unchanged.
- `manifest_value()` (`:236`) — reads a `key = "value"` from the manifest,
  **scoped to the `[grammars.awl]` table** and stopping at the next `[`, with a
  leading `#` stripped. Two consequences the leg must respect: the sha line
  **keeps its `# ` prefix**, and neither line may be moved out of that table or
  below another table header.
- `the_zed_pin_names_a_mirror_commit_carrying_this_grammar()` (`:264`, asserts
  at `:283`) — the red. It also asserts `rev.len() == 40`, so the replacement
  must be the full 40-character sha, never abbreviated.
- `the_recorded_pin_is_present_on_the_public_mirror()` (`:305`) — the network
  half. It is gated at runtime on `AION_VERIFY_MIRROR_PIN=1` (never `#[ignore]`)
  and prints `SKIPPED:` when it does not run. When it runs it asserts the
  mirror's `refs/heads/main` head **equals** the recorded `rev` — not merely
  contains it. Its own doc comment directs that it be run "before landing any
  change to `editors/zed-awl/extension.toml`", which makes it part of this
  leg's battery, not an optional extra.

The red, verbatim from `.fleet/gates/RED-LATEST.md:1063-1078`:

```
thread 'the_zed_pin_names_a_mirror_commit_carrying_this_grammar' (952447) panicked at crates/aion-awl/tests/editor_assets.rs:283:5:
assertion `left == right` failed:
...
  4. set `rev` to that sha and `# grammar-sha256` to 5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859

  left: "5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859"
 right: "eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e"
```

The test itself printed the required digest. That printed value is the one the
brief carries and the one the leg writes. Counsel additionally reproduced the
digest construction read-only over the four in-tree files and got
`5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859` — a
cross-check only. **The leg does not derive the digest by any means; it copies
the two values the brief states and lets the test confirm them.**

That gate round's only failing target was `-p aion-awl --test editor_assets`
(`.fleet/gates/RED-LATEST.md:8080-8081`); the aion#54 shard-actor flake did not
fire in it.

## 2. The edit — exactly two lines, one file

`editors/zed-awl/extension.toml`, inside `[grammars.awl]`, both lines replaced
in place, order and indentation untouched:

| line | from | to |
|---|---|---|
| `:24` | `rev = "8d4d63e398bf765dd51772b60555b9dae8ebc106"` | `rev = "4ba65879406dbe9204138e24397917ba60d82aff"` |
| `:25` | `# grammar-sha256 = "eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e"` | `# grammar-sha256 = "5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859"` |

Bump both together. A rev bump without the digest leaves the staleness fence
disarmed; a digest bump without the rev records that the mirror was pushed when
it was not — the exact falsification the test's own message forbids, and the
one the previous flight correctly refused to commit.

No other file in the tree carries either old value outside `.fleet/` prose.
`editors/nvim-awl/` installs the grammar from a local
`location = 'tools/tree-sitter-awl'` and carries no pin, so it is untouched by
construction as well as by wall.

## 3. Acceptance battery — in this order

1. **Precondition.** `git ls-remote …` = `4ba65879406dbe9204138e24397917ba60d82aff` (§0).
2. **The edit.** Two lines, one file.
3. **`cargo test -p aion-awl --test editor_assets`** — 5 passed, 0 failed,
   including `the_zed_pin_names_a_mirror_commit_carrying_this_grammar`.
4. **`AION_VERIFY_MIRROR_PIN=1 cargo test -p aion-awl --test editor_assets`** —
   same 5 green with the network half actually running rather than printing
   `SKIPPED:`. This is what proves the pinned sha resolves from the anonymous
   fetch a Zed user performs. Note the asymmetry: run with the env var
   **before** the edit this test fails by design (mirror head `4ba65879` ≠
   pinned `8d4d63e3`) — that is the disease, not a new defect.
5. **The full gate** — `.fleet/gate-entry.sh` via the gates activity: `cargo fmt
   --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; the
   integrations/norn/worker suites twice; `cargo test --workspace
   --no-fail-fast`. Green, with `GATE-GREEN` on the last line. The workspace run
   reaches `editor_assets` and leaves the network half skipped, which is why
   step 4 exists separately.
6. **The diff.** `git diff <base>..HEAD --stat` shows
   `editors/zed-awl/extension.toml` with exactly 2 insertions and 2 deletions,
   plus `.fleet/` artifacts. Nothing else, in any file, for any reason.

## 4. Walls — binding

- **The diff is the two lines plus `.fleet/` artifacts.** No grammar-source
  edits, no vendored query edits, no test edits, no manifest reflow, no
  comment rewording, no formatting drive-by.
- **NEVER touch** `tools/tree-sitter-awl/`, `editors/nvim-awl/`, any file under
  `editors/zed-awl/` other than the two pin lines, or any test file.
- **No suppression in any spelling** — `#[ignore]`, `#[allow]`, `#[expect]`,
  `#[cfg(any())]`, a skip env var, a weakened assertion, a deleted test. The
  test is the oracle; disarming the oracle is the one failure mode this whole
  flight exists to have avoided.
- **NEVER push to the mirror or any external remote.** The external half is
  done. `git ls-remote` (read-only) is the only permitted contact with it, plus
  whatever step 4's test performs on its own.
- **Red after the edit ⇒ report and stop.** If
  `the_zed_pin_names_a_mirror_commit_carrying_this_grammar` is still red after
  the two-line edit, do not widen the change by one character. Record the
  verbatim failure — both digests as printed — in `.fleet/reports/l3-pin.md`
  and end the leg red. The dispatcher reads it from there.
- **aion#54 shard-actor timeout is a KNOWN flake on this box** (2/24 base
  rate). A red there that reproduces on the unmodified base is REPORTED, never
  chased. **No builds during a battery.**

## 5. What this flight is not

It is not a re-opening of `policyref-l3-route`. The routing work, the `fallback`
reserved word, the settlement predicate, the event and recorder changes — all
landed, all judged, none re-derived, none reviewed here. If the leg finds
something it believes is wrong in that landed work, it **reports it and changes
nothing**: this tree's diff is two lines.
