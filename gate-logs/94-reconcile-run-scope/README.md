# Provenance of the two records in this directory

`HYPOTHESIS.md` and `RESULT.md` were written on **2026-08-20** on the branch
`investigate-workflow` at commit **`0acceb684`**, which is **not an ancestor** of
`fix/94-reconcile-run-scope`. The fix commit `f83e8167a` cites
`gate-logs/94-reconcile-run-scope/RESULT.md` as its full record, so without this
recovery main would cite a record main does not hold. Recovered verbatim with
`git show 0acceb684:<path>` — content unmodified; this file is the only addition.

`HYPOTHESIS.md` is kept deliberately: it records what was believed **before** the
defect was measured, and the falsifiers it named. `RESULT.md` supersedes it.

## ⚠️ One staleness caveat — the counts are a record of their own commit

`RESULT.md` reports `709/709` (lib) and `1198/1198` (all targets), and says the
diff *"adds exactly 2 `#[test]` functions and removes none, so 1196 → 1198 is
accounted"*.

**That describes the branch as it stood at `0acceb684`.** Two further tests were
added afterwards in `2ee53285d` — `reconcile_refuses_a_run_absent_from_the_history`
and `run_not_in_history_maps_to_typed_backend_on_both_halves` — closing an error
path (`EngineError::RunNotInHistory`) that had no test at any of its three
surfaces. Those numbers are correct as a record of the commit that produced them
and **must not be read as the current denominator**. The current denominator is
the gate log: `gate-logs/94-reconcile/battery-20260820-on-main.log`, frozen HEAD
`2ee53285d`, with the four tests and their line numbers tabulated in
`gate-logs/94-reconcile/README.md`.

Corrections to the fix commit's own prose are in
`gate-logs/94-reconcile/CORRECTIONS.md`.
