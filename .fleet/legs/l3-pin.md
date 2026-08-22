# LEG `l3-pin` — record the pushed mirror in the Zed pin

**Leg id:** `l3-pin` (the only leg; the split is fixed at dispatch)
**Branch from:** `fleet/policyref-l3-pin-base` @ `663f1957`
**Depends on:** the dispatcher's mirror push, already done (§0).
**Blocks:** nothing.

Read, in this order, before touching anything:

1. `.fleet/BRIEF.md` — complete and self-sufficient; it names the file, the two
   lines, and all four values.
2. `.fleet/plan.md` — the precondition, the mechanism re-derived by symbol on
   this tree, the battery, and the walls.
3. This file.

---

## 1. Scope

One file. Two lines. `editors/zed-awl/extension.toml`, in `[grammars.awl]`.

Everything else in this repository is out of scope, including everything the
previous flight (`policyref-l3-route`) landed. That work is done and judged; you
consume the tree as it stands and never re-derive, re-review, or "improve" it.

---

## 2. 🔴 NON-NEGOTIABLES

### 2.1 The precondition — your first act, before any edit

```
git ls-remote https://github.com/tomWhiting/tree-sitter-awl.git refs/heads/main
```

MUST print exactly `4ba65879406dbe9204138e24397917ba60d82aff`.

If it prints anything else — a different sha, a network error, an auth prompt —
**STOP**. Commit nothing. Write the observed output verbatim into
`.fleet/reports/l3-pin.md` and end the leg. The precondition failed; the
dispatcher re-examines. Do not pin anyway, do not bump the digest alone, do not
retry with credentials.

### 2.2 The test is the oracle

`crates/aion-awl/tests/editor_assets.rs` computes the required digest from
in-tree sources (`published_grammar_digest`, `:210`) and asserts it at `:283`.
You do **not** derive the digest — not with `sha256sum`, not with a script, not
by reading it out of a build log you regenerate. You copy the two values the
brief states and let the test confirm them. If the test ever disagrees with the
brief, the test wins and you report the disagreement rather than reconciling it.

### 2.3 Both lines move together

A `rev` bump without the digest disarms the staleness fence. A digest bump
without the `rev` records that the mirror was pushed when it was not — the
falsification the test's own message forbids, and the thing the previous flight
correctly refused to commit. Two lines, one commit, or neither.

### 2.4 Red after the edit ⇒ report and stop

If `the_zed_pin_names_a_mirror_commit_carrying_this_grammar` is still red after
the two-line edit, **do not widen the change by one character**. Record the
verbatim failure — both digests exactly as printed — in
`.fleet/reports/l3-pin.md` and end the leg red.

### 2.5 The walls

- **NEVER touch** `tools/tree-sitter-awl/`, `editors/nvim-awl/`, anything under
  `editors/zed-awl/` beyond the two pin lines, or any test file.
- **NEVER push** to the mirror repository or any external remote. `git
  ls-remote` is read-only and is the only contact you initiate.
- **No suppression in any spelling** — `#[ignore]`, `#[allow]`, `#[expect]`,
  `#[cfg(any())]`, a skip env var, a weakened assertion, a deleted test.
- **No reflow, no reword, no formatting drive-by** anywhere, including in the
  manifest you are editing. Its `:15-22` comment block stays byte-identical.
- **aion#54 shard-actor timeout is a KNOWN flake** on this box (2/24 base rate).
  A red there reproducing on unmodified base is REPORTED, never chased. **No
  builds during a battery.**

---

## 3. R1 — the two lines

In `editors/zed-awl/extension.toml`, section `[grammars.awl]`:

**Line 24**

```toml
rev = "8d4d63e398bf765dd51772b60555b9dae8ebc106"
```

becomes

```toml
rev = "4ba65879406dbe9204138e24397917ba60d82aff"
```

**Line 25**

```toml
# grammar-sha256 = "eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e"
```

becomes

```toml
# grammar-sha256 = "5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859"
```

Mechanical constraints, all enforced by the test — verify you have not tripped
one before you run anything:

- The sha line **keeps its leading `# `**. `manifest_value` (`:236`) strips one
  `#` and would not find the key without it; the line is a comment on purpose.
- Both lines stay **inside `[grammars.awl]`** and above any next `[` table
  header. The reader stops at the next table, and `repository` is also a
  top-level key naming the aion repo itself — moving these lines would send the
  network check at the wrong remote and pass.
- The `rev` is the **full 40 characters**. The test asserts `rev.len() == 40`;
  an abbreviated or symbolic revision is not a fence.
- Nothing else on either line changes: same key, same spacing, same quoting.

---

## 4. R2 — the acceptance battery, in order

1. **Precondition** — §2.1. Record the exact ls-remote output in your report.
2. **The edit** — §3.
3. `cargo test -p aion-awl --test editor_assets`
   → **5 passed; 0 failed**, including
   `the_zed_pin_names_a_mirror_commit_carrying_this_grammar`.
4. `AION_VERIFY_MIRROR_PIN=1 cargo test -p aion-awl --test editor_assets`
   → same 5 green, and `the_recorded_pin_is_present_on_the_public_mirror`
   actually **runs** rather than printing its `SKIPPED:` line. This is the
   network half: it asserts the mirror's `refs/heads/main` head equals the
   recorded `rev`, which is what a Zed user's anonymous fetch does. The test's
   own doc comment requires it before landing any change to this manifest.
   Expect this test to FAIL if you run it before the edit — that is the disease
   (head `4ba65879` ≠ pin `8d4d63e3`), not a new defect.
5. **The full gate** — `.fleet/gate-entry.sh` via the gates activity. Green,
   ending `GATE-GREEN`. The workspace run reaches `editor_assets` with the
   network half skipped, which is why step 4 stands separately.
6. **Diff proof** — `git diff <base>..HEAD --stat` shows
   `editors/zed-awl/extension.toml | 2 +- 2 -` (2 insertions, 2 deletions) and
   `.fleet/` artifacts. Nothing else. Paste the stat into your report.

Any step failing stops the leg at that step. Report what you observed, verbatim.

---

## 5. Report

Write `.fleet/reports/l3-pin.md`. It must carry, verbatim rather than
paraphrased:

- the `git ls-remote` output;
- the `editor_assets` result line for both runs (with and without
  `AION_VERIFY_MIRROR_PIN=1`), and for step 4 the line proving the network test
  ran rather than skipped;
- the gate's final line;
- the `git diff --stat` against base;
- anything you noticed and deliberately did not touch.

If you ended red, the verbatim failure block is the report's centre — both
digests as printed, and the assertion's file:line.
