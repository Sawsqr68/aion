# libsql-retirement pre-removal census

Measured at baseline `00064c41c7c023ea86f940a06b7ace37b439596e` before any production or dependency removal.

## Instrument control

`census/canary-proof.txt` records five planted, temporary canaries: one each for Cargo manifests, Rust source, docs, workflows, and config. The exact sweep found all five; the canaries were then removed. They are not part of the change.

## Dependency graph

`census/cargo-tree-inverse.txt` is the unedited output of `cargo tree -i ... --workspace` for `aion-store-libsql`, `libsql`, `libsql-sys`, and `libsql_replication`. It shows the backend crate as the sole production root for the libsql family, plus direct test-only edges from `aion-server` and `aion-rs`.

## Text surface and dispositions

`census/text-hit-dispositions.tsv` lists every tracked file matching `libsql`, `store.url`, `AION_STORE_URL`, `--store-url`, or `slim build` (Cargo.lock excluded because it is regenerated after dependency removal). Each row has its disposition. Historical design/evidence records remain history; normative, production, config, test, release, and assistant-facing claims are updated or deleted.

`census/expected-red-hits.txt` separately inventories #85/#229 mentions on gate/runbook/release surfaces. Historical attestations remain historical. Live instructions that tell future batteries to classify or excuse #85/#229 are removed or explicitly retired in the landing motion.

## Data-file boundary

`census/repository-db-files.txt` contains no paths after its heading: no sqlite/db fixture or live-data file exists inside this worktree. Per dispatch Amendment 2, the independent live-server population check was already performed outside this builder lane and found the loaded backend was haematite. This builder did not inspect or touch the live estate or `~/.aion`.

## Wire and excluded surfaces

A source sweep found no `.proto` store-backend selection edge; this lane does not touch wire definitions. Beamr, liminal, and SDKs have no store-selection source edge and remain out of scope.
