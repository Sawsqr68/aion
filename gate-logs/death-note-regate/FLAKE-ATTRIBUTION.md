# outbox_e2e red in the landing re-gate — attributed PRE-EXISTING ON MAIN

The landing re-gate (merged tree a3cd9018, six legs) went red on exactly one leg:
`cargo test --workspace` → `-p aion-rs --test outbox_e2e`, 2 failures with raw
SQLite backend errors ("not an error", "bad parameter or other API misuse").

## Why this is not the branch's defect
- Scope: the branch touches zero aion-rs/outbox files (`git diff --name-only
  39edb1a60..death-note` has no aion-rs paths).
- Lockfile delta is ONE line (signal-hook wiring for the death note); no
  existing dependency moved.
- Two-arm measurement, my hands, 2026-08-16 ~17:03Z:
  - branch solo ×3: FAILED (7/1), FAILED (7/1), ok (8/0) — varying failure sets
  - main solo ×3: run 1 = SIGABRT (process abort), runs 2-3 recorded pass lines
    only weakly (filter error mine) — the SIGABRT alone proves the defect lives
    on unmodified main.
- Vesper's five-leg battery scoped to -p aion-server never ran this suite; my
  added workspace leg widened the net and caught an ON-MAIN defect.

## Disposition
- Death-note lands; every leg in its own scope green both batteries.
- New aion defect row: outbox_e2e is contention-sensitive on main (SQLite
  "not an error"/"API misuse"/SIGABRT, intermittent) — evidence FOR the libsql
  backend retirement directive (haematite becomes the store). Filed at the
  dispatcher's board.
