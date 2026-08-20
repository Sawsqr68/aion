# Repository battery workflow

`repo_battery` measures a caller-named repository ref in a run-owned tree. It invokes that tree's committed `scripts/battery.sh` and `scripts/gates/verify-declaration.sh`; it holds no leg list.

## Inputs

- `subject_repo`: git URL or local git repository path.
- `subject_ref`: caller-named ref to check out detached.
- `workspace_root`: venue root under which `<workflow_id>/repo` is created.
- `waivers`: per-run records containing `leg`, `reason`, and `ruled_by`. Anonymous waivers are refused before provisioning.

Naming a ref is authority-bearing: the battery runs with the venue's credentials, network reach, filesystem access, and neighbours. Use only refs reviewed for that venue.

## Serve and run

From the reviewed repository root:

```sh
aion awl check workflows/repo-battery/repo_battery.awl
aion awl check workflows/repo-battery/repo_battery_worker.awl
aion worker awl --check workflows/repo-battery/repo_battery_worker.awl
# At a live engine venue:
aion worker awl workflows/repo-battery/repo_battery_worker.awl
```

The workflow is manual-start only. It defines no cadence, CI trigger, or child workflow.

## Status and receipt

Green and red measured batteries route to workflow success because status answers “did I measure?” while the ordered receipt answers “what did I measure?”. A declared leg missing from `EXECUTED_LEGS` is `unproven` and routes to failure. Missing `gates.json` is the explicit success-status census result `undeclared`, which is not a verdict and must never be treated as pass-by-omission. Zero selected legs is a typed refusal naming `gates.json:.legs`.

The receipt records the frozen HEAD, declaration and battery sha256 values, sorted script witnesses, venue, effective declared environment, declared and executed leg names, per-leg rc/status, verifier output, waivers, and verdict. These witnesses attest which bytes and scope ran; they do not refuse a run because an expected digest differs.

## What a green does not prove

A green does not prove that the venue is trustworthy, that another ref or environment is equivalent, or that a leg's command tests what its name claims. `verify-declaration.sh` compares names only: a leg named `rust:clippy` whose command is `true` passes that scope instrument. The receipt enables later comparison, but this workflow performs no cross-receipt drift check.

No live dispatch is required for the offline proof harness. Engine routing and cancellation proofs remain runnable landing-desk obligations where a listener and seated worker are permitted.
