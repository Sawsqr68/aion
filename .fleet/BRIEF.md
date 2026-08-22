# Brief: policyref-l3-pin — retire the editor_assets mirror-pin red

## Standing

You stand on the FULL spent tree of flight policyref-l3-route (fleet/policyref-l3-flagged
= e64031e6). That flight's deliverable — routing policy refusals through authored
fallbacks, including the new AWL `fallback` reserved word — is DONE, COMMITTED, and
judged real work. Its final gate ended with exactly ONE red:

```
test the_zed_pin_names_a_mirror_commit_carrying_this_grammar ... FAILED
crates/aion-awl/tests/editor_assets.rs:283
```

Mechanism (verified first-hand by the dispatcher): the L3 leg extended the grammar
(commits 48047bd2 + 9422491d) and honestly regenerated the in-tree parser, but the Zed
pin in `editors/zed-awl/extension.toml` still records the PREVIOUS mirror commit and
digest. The test recomputes `published_grammar_digest()` from in-tree sources and
fails when the recorded pair lags. The cure requires pushing the external mirror —
an estate-external act a fleet seat cannot and must not perform.

## What has ALREADY been done for you (do not redo)

The dispatcher (Vesper Lynd) has synced the grammar into the published mirror and
pushed it. Verify it yourself, anonymously — this is your first act:

```
git ls-remote https://github.com/tomWhiting/tree-sitter-awl.git refs/heads/main
```

MUST print exactly `4ba65879406dbe9204138e24397917ba60d82aff`. If it prints anything
else, STOP: commit nothing, write the observed value into your report, and end your
leg — the precondition failed and the dispatcher must re-examine.

The mirror commit 4ba65879 carries the fallback grammar byte-identically
(tree-sitter generate byte-stable against the leg's committed sources; tree-sitter
test 18/18 at the dispatcher's hand).

## The task (R1) — exactly two lines in one file

In `editors/zed-awl/extension.toml`, section `[grammars.awl]`:

- `rev = "8d4d63e398bf765dd51772b60555b9dae8ebc106"`
  → `rev = "4ba65879406dbe9204138e24397917ba60d82aff"`
- `# grammar-sha256 = "eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e"`
  → `# grammar-sha256 = "5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859"`

The sha256 value is what the failing test itself printed as the required digest of the
in-tree grammar sources. Do NOT recompute it by improvising a hash command — prove it
by running the test (below); the test IS the oracle.

## Acceptance

1. `cargo test -p aion-awl --test editor_assets` — ALL tests green, including
   `the_zed_pin_names_a_mirror_commit_carrying_this_grammar`.
2. The full gate (`.fleet/gate-entry.sh` via the gates activity) — green.
3. Your diff against base is EXACTLY the two lines above in that one file, plus your
   .fleet report/plan artifacts. Nothing else. No grammar-source edits, no vendored
   query edits, no test edits, no suppressions in any spelling.

## Walls

- NEVER touch tools/tree-sitter-awl/, editors/zed-awl/ beyond the two pin lines,
  editors/nvim-awl/, or any test file.
- NEVER attempt any push to the mirror repository or any external remote.
- If the editor_assets test stays red AFTER the two-line edit, do not widen the
  change: record the verbatim failure in your report and end the leg red — the
  dispatcher reads it from there.
- aion#54 shard-actor timeout is a KNOWN flake on this box (2/24 base rate): a red
  there reproducing on the unmodified base is REPORTED, never chased. NO BUILDS
  during a battery.
