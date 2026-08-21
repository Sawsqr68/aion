# investigate — the issue-investigation loop, on stock workers

The ratified design (`docs/design/issue-investigation-workflow-20260818.md`,
estate docs repo) built with zero bespoke crates: the estate server
orchestrates, two stock workers on the investigation box serve, every run is
in the operator's console.

| Piece | Served by |
|---|---|
| `investigate.awl` | the workflow — deploy to the estate server |
| `investigate_worker.awl` (queue `investigate_ops`) | `aion worker awl` on the box — clone / read-report / file-report / teardown |
| the `investigator` agent seam (in `investigate.awl`) | `aion worker agent` on the box, norn harness |
| `bin/*.sh` | the sealed scripts the worker document's commands name at `/home/aion/fleet/bin/` |

## The scripts are sealed by path — pin the bytes

Worker-document `run` bodies admit no shell variables, so all logic lives in
these scripts, exactly as gates seals `gate-runner.zsh`. This folder is the
source of truth; provisioning copies `bin/*.sh` to `/home/aion/fleet/bin/`
and records each file's sha256 in the dispatch record. A drifted script is a
different instrument.

Every script exits ZERO on anticipated faces and returns its refusal as
typed JSON — the workflow routes on refusals; a nonzero exit is
retryable-then-terminal and reserved for genuine breakage.

## Deployment (this deployment: aion-workflows, `aion@192.168.50.205`)

Per-deployment values, named here because the documents cannot carry them
portably: workspace root `/home/aion/fleet/runs` (in the harness
`--workspace-root` arg AND the `workspace_root` input AND teardown.sh's
root guard — all three must agree); estate server `192.168.50.40`
(gRPC 50051 for the awl worker, liminal 50061 for the agent worker).

```sh
# on the box, from the repo checkout
aion worker awl workflows/investigate/investigate_worker.awl \
    --endpoint http://192.168.50.40:50051 --identity investigate-ops-workflows
aion worker agent workflows/investigate/investigate.awl \
    --liminal-address 192.168.50.40:50061 --identity investigator-workflows
```

## Dispatch

The dispatcher builds two prompts: `control_prompt` — quote a literal at a
known path in the clone, reply exactly; `investigate_prompt` — the issue, the
threshold that fired, and the deliverable path
`repo/.investigation/report.json` RELATIVE TO THE SESSION CWD (the clone).
The workflow appends the complete prose contract and the derived
`InvestigationReport` schema to `investigate_prompt` at compile time, so the
dispatcher does not carry or reproduce the field contract.

Deterministic-id idempotency is NOT expressible on today's start surface
(stated gap in the document header): carry
`investigate/<repo-slug>/<issue>/<threshold>` in `--name`.

## Red-first receipts

Script faces proven 2026-08-18 (see the dispatch record): clone
dest-exists + id-shape refusals and real-clone success; read-report
absent/unparseable/parses-typed; teardown root-guard + id-guard; file-report
absent-report + gh-failure faces with render proven. Workflow-level refusal
arms are proven by dispatched runs before the first real investigation —
`unsupported_tracker`, `unprovisioned`, `unmeasured` at minimum — and the
receipts land beside this folder in gate-logs.
