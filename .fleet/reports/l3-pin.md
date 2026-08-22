# l3-pin report

## PRECONDITION — OBSERVED

Invocation:

```text
git ls-remote https://github.com/tomWhiting/tree-sitter-awl.git refs/heads/main
```

Verbatim output:

```text
4ba65879406dbe9204138e24397917ba60d82aff	refs/heads/main
```

## Diff against base — OBSERVED

Invocation:

```text
git diff 663f1957 -- editors/zed-awl/extension.toml
```

Verbatim output:

```diff
diff --git a/editors/zed-awl/extension.toml b/editors/zed-awl/extension.toml
index e45ea99b..a2eb8274 100644
--- a/editors/zed-awl/extension.toml
+++ b/editors/zed-awl/extension.toml
@@ -21,5 +21,5 @@ language_ids = { AWL = "awl" }
 # recomputes it and fails when the two disagree. Bump both together, and only
 # after `git ls-remote` shows the sha anonymously.
 repository = "https://github.com/tomWhiting/tree-sitter-awl.git"
-rev = "8d4d63e398bf765dd51772b60555b9dae8ebc106"
-# grammar-sha256 = "eb61fb429719c7666618a7dc841266119db320ee9a8cf8557d4364dcb993691e"
+rev = "4ba65879406dbe9204138e24397917ba60d82aff"
+# grammar-sha256 = "5c659cfa6ced3154ad5a69173b320591b83886f80972ed44f42b5d7db405f859"
```

## Test — OBSERVED

Invocation:

```text
cargo test -p aion-awl --test editor_assets
```

Verbatim summary line:

```text
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```
